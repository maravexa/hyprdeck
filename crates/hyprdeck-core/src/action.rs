use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::ipc::command;

/// Unified action system for all user interactions across modules and themes.
///
/// Actions are serialisable so they can be declared inline in theme/config TOML.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// Launch an external program.
    Exec {
        command: String,
        #[serde(default)]
        args: Vec<String>,
    },
    /// Send a `hyprctl dispatch` command to Hyprland.
    HyprDispatch { dispatch: String },
    /// Execute the command stored in a Hyprland `$variable`.
    HyprlandExec {
        variable: String,
        #[serde(default)]
        fallback: Option<String>,
    },
    /// Forward a named action to a specific module.
    ModuleAction { module: String, action: String },
    /// Execute multiple actions sequentially.
    Chain { actions: Vec<Action> },
}

/// Errors that can occur while executing an [`Action`].
#[derive(Debug, thiserror::Error)]
pub enum ActionError {
    /// A child process could not be spawned.
    #[error("failed to spawn '{command}': {source}")]
    SpawnFailed {
        command: String,
        source: std::io::Error,
    },
    /// A Hyprland dispatch command failed.
    #[error("hyprland dispatch '{dispatch}' failed: {detail}")]
    HyprDispatchFailed { dispatch: String, detail: String },
    /// A requested Hyprland command variable was unavailable.
    #[error("Hyprland variable '${variable}' is not defined and has no fallback")]
    HyprlandVariableMissing { variable: String },
}

/// Execute an action. Called by the main loop when a module returns an [`Action`].
pub async fn dispatch_action(action: &Action, hypr_socket: &Path) -> Result<(), ActionError> {
    match action {
        Action::Exec { command: cmd, args } => {
            tracing::info!("Executing: {} {:?}", cmd, args);
            spawn_detached(cmd, args)
        }

        Action::HyprDispatch { dispatch } => {
            tracing::debug!("Hyprland dispatch: {}", dispatch);
            command::dispatch(hypr_socket, dispatch).await.map_err(|e| {
                tracing::error!("Hyprland dispatch failed: {}", e);
                ActionError::HyprDispatchFailed {
                    dispatch: dispatch.clone(),
                    detail: e.to_string(),
                }
            })
        }

        Action::HyprlandExec { variable, fallback } => {
            let command = resolve_hyprland_variable(variable)
                .or_else(|| fallback.clone())
                .ok_or_else(|| ActionError::HyprlandVariableMissing {
                    variable: variable.trim_start_matches('$').to_owned(),
                })?;
            tracing::info!(variable, %command, "executing Hyprland command variable");
            spawn_detached("sh", &["-lc".to_owned(), format!("exec {command}")])
        }

        Action::ModuleAction { module, action } => {
            // Module-internal actions are handled by the module itself during
            // handle_event(). If one reaches the dispatcher it means the module
            // wants the panel to handle it. Log and ignore for now.
            tracing::debug!("Module action: {}::{}", module, action);
            Ok(())
        }

        Action::Chain { actions } => {
            for a in actions {
                if let Err(e) = dispatch_action_boxed(a, hypr_socket).await {
                    tracing::warn!("Action in chain failed: {}. Continuing.", e);
                }
            }
            Ok(())
        }
    }
}

fn spawn_detached(command: &str, args: &[String]) -> Result<(), ActionError> {
    let result = tokio::process::Command::new(command)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();

    match result {
        Ok(_child) => Ok(()),
        Err(source) => Err(ActionError::SpawnFailed {
            command: command.to_owned(),
            source,
        }),
    }
}

fn resolve_hyprland_variable(variable: &str) -> Option<String> {
    let path = hyprland_config_path()?;
    let source = std::fs::read_to_string(path).ok()?;
    variable_from_config(&source, variable)
}

fn hyprland_config_path() -> Option<PathBuf> {
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|value| !value.is_empty())
                .map(|home| PathBuf::from(home).join(".config"))
        })?;
    Some(config_home.join("hypr/hyprland.conf"))
}

fn variable_from_config(source: &str, variable: &str) -> Option<String> {
    let wanted = variable.trim().trim_start_matches('$');
    source
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.starts_with('#') {
                return None;
            }
            let (name, value) = line.split_once('=')?;
            (name.trim().strip_prefix('$')? == wanted)
                .then(|| value.trim().to_owned())
                .filter(|value| !value.is_empty())
        })
        .next_back()
}

/// Box the recursive call to avoid infinite future size.
fn dispatch_action_boxed<'a>(
    action: &'a Action,
    hypr_socket: &'a Path,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), ActionError>> + Send + 'a>> {
    Box::pin(dispatch_action(action, hypr_socket))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_exec_serializes_correctly() {
        let action = Action::Exec {
            command: "firefox".to_owned(),
            args: vec!["--new-window".to_owned()],
        };
        let json = serde_json::to_string(&action).expect("serialize");
        assert!(json.contains("\"type\":\"exec\""));
        assert!(json.contains("\"command\":\"firefox\""));
    }

    #[test]
    fn action_chain_serializes() {
        let action = Action::Chain {
            actions: vec![
                Action::Exec {
                    command: "a".into(),
                    args: vec![],
                },
                Action::HyprDispatch {
                    dispatch: "workspace 2".into(),
                },
            ],
        };
        let json = serde_json::to_string(&action).expect("serialize");
        assert!(json.contains("\"type\":\"chain\""));
    }

    #[test]
    fn hyprland_exec_roundtrips() {
        let action = Action::HyprlandExec {
            variable: "menu".into(),
            fallback: Some("wofi --show drun".into()),
        };
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("\"type\":\"hyprland_exec\""));
        assert!(matches!(
            serde_json::from_str::<Action>(&json).unwrap(),
            Action::HyprlandExec { variable, .. } if variable == "menu"
        ));
    }

    #[test]
    fn config_variable_parser_uses_last_active_definition() {
        let config = r#"
            # $menu = ignored
            $menu = fuzzel
            $terminal = kitty
            $menu = wofi --show drun
        "#;
        assert_eq!(
            variable_from_config(config, "$menu").as_deref(),
            Some("wofi --show drun")
        );
        assert_eq!(variable_from_config(config, "missing"), None);
    }

    #[test]
    fn action_module_action_roundtrip() {
        let action = Action::ModuleAction {
            module: "clock".into(),
            action: "toggle_format".into(),
        };
        let json = serde_json::to_string(&action).expect("serialize");
        let back: Action = serde_json::from_str(&json).expect("deserialize");
        match back {
            Action::ModuleAction { module, action } => {
                assert_eq!(module, "clock");
                assert_eq!(action, "toggle_format");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[tokio::test]
    async fn dispatch_exec_nonexistent_command_returns_error() {
        let socket = Path::new("/nonexistent/socket");
        let action = Action::Exec {
            command: "/nonexistent/binary/xyzzy".into(),
            args: vec![],
        };
        let result = dispatch_action(&action, socket).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ActionError::SpawnFailed { command, .. } => {
                assert_eq!(command, "/nonexistent/binary/xyzzy");
            }
            other => panic!("wrong error variant: {:?}", other),
        }
    }

    #[tokio::test]
    async fn dispatch_module_action_succeeds() {
        let socket = Path::new("/nonexistent/socket");
        let action = Action::ModuleAction {
            module: "clock".into(),
            action: "toggle".into(),
        };
        let result = dispatch_action(&action, socket).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn dispatch_chain_continues_on_failure() {
        let socket = Path::new("/nonexistent/socket");
        let action = Action::Chain {
            actions: vec![
                // This will fail (nonexistent binary)
                Action::Exec {
                    command: "/nonexistent/binary/aaa".into(),
                    args: vec![],
                },
                // This should still run and succeed
                Action::ModuleAction {
                    module: "test".into(),
                    action: "ok".into(),
                },
            ],
        };
        let result = dispatch_action(&action, socket).await;
        // Chain itself succeeds even though one sub-action failed
        assert!(result.is_ok());
    }

    #[test]
    fn action_error_display() {
        let err = ActionError::SpawnFailed {
            command: "test".into(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
        };
        let msg = err.to_string();
        assert!(msg.contains("test"));
        assert!(msg.contains("not found"));

        let err2 = ActionError::HyprDispatchFailed {
            dispatch: "workspace 5".into(),
            detail: "connection refused".into(),
        };
        let msg2 = err2.to_string();
        assert!(msg2.contains("workspace 5"));
        assert!(msg2.contains("connection refused"));
    }
}
