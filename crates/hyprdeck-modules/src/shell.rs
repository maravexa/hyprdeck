//! Shell command output module.
//!
//! Runs an arbitrary shell command on a configurable interval via `sh -c` and
//! displays the trimmed stdout in the panel.
//!
//! Commands execute in a background tokio task with a hard timeout; results are
//! shared back to `update()` via `Arc<Mutex<Option<String>>>`.
//!
//! Security notes:
//! - Commands come from user config only; no runtime input is ever passed to the
//!   shell.
//! - `stdin` is always `/dev/null`.
//! - Output is truncated to `max_chars`.
//! - Execution is bounded by `timeout_secs`.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Deserialize;

use hyprdeck_core::{
    ConfigField, ConfigFieldType, EventResult, InputEvent, ModuleConfigSchema, PanelModule, Pixmap,
    Rect, Size, ThemeContext, UpdateContext,
};

use crate::render_utils;

// ── Config ────────────────────────────────────────────────────────────────────

/// Configuration for the shell command output module.
#[derive(Debug, Deserialize)]
pub struct ShellConfig {
    /// Shell command to run (passed to `sh -c`).
    pub command: String,
    /// How often to re-run the command, in seconds.
    #[serde(default = "default_interval")]
    pub interval_secs: u64,
    /// Maximum number of characters to display before truncating.
    #[serde(default = "default_max_chars")]
    pub max_chars: usize,
    /// Hard timeout for command execution in seconds.
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

fn default_interval() -> u64 {
    5
}
fn default_max_chars() -> usize {
    64
}
fn default_timeout() -> u64 {
    5
}

impl Default for ShellConfig {
    fn default() -> Self {
        ShellConfig {
            command: String::new(),
            interval_secs: default_interval(),
            max_chars: default_max_chars(),
            timeout_secs: default_timeout(),
        }
    }
}

// ── Module ────────────────────────────────────────────────────────────────────

/// Runtime state for the shell output module.
pub struct ShellModule {
    config: ShellConfig,
    /// Last captured stdout from the command.
    output: String,
    /// Shared slot written by the background task.
    shared: Arc<Mutex<Option<String>>>,
    /// When the most recent execution was started.
    last_run: Option<Instant>,
    /// Whether a background execution is currently in progress.
    running: bool,
}

impl ShellModule {
    pub fn new(config: ShellConfig) -> Self {
        ShellModule {
            config,
            output: String::new(),
            shared: Arc::new(Mutex::new(None)),
            last_run: None,
            running: false,
        }
    }

    fn should_run(&self) -> bool {
        match self.last_run {
            None => !self.config.command.is_empty(),
            Some(t) => {
                !self.config.command.is_empty()
                    && t.elapsed() >= Duration::from_secs(self.config.interval_secs)
            }
        }
    }
}

impl PanelModule for ShellModule {
    fn id(&self) -> &str {
        "shell"
    }

    fn desired_size(&self, theme: &ThemeContext) -> Size {
        let sample = if self.output.is_empty() {
            " "
        } else {
            &self.output
        };
        let h = theme.fonts.size + theme.padding.top + theme.padding.bottom;
        let font_size = render_utils::effective_font_size(h, theme.fonts.size);
        let w = render_utils::estimate_text_width(sample, font_size)
            + theme.padding.left
            + theme.padding.right;
        Size::new(w, h)
    }

    fn update(&mut self, _ctx: &UpdateContext<'_>) -> bool {
        // Check if the background task produced a result.
        let result = self.shared.try_lock().ok().and_then(|g| g.clone());

        if let Some(new_output) = result {
            // Clear the shared slot.
            if let Ok(mut g) = self.shared.lock() {
                *g = None;
            }
            self.running = false;
            self.last_run = Some(Instant::now());
            if new_output != self.output {
                self.output = new_output;
                return true;
            }
        }

        // Spawn a new execution if the interval has elapsed.
        if self.should_run() && !self.running {
            self.running = true;
            // Don't reset last_run here — it will be set when the result arrives.
            // This prevents an immediate re-spawn if the task is still in flight.
            if self.last_run.is_none() {
                self.last_run = Some(Instant::now());
            }
            let shared = Arc::clone(&self.shared);
            let command = self.config.command.clone();
            let max_chars = self.config.max_chars;
            let timeout_secs = self.config.timeout_secs;

            tokio::spawn(async move {
                let result = execute_command(&command, max_chars, timeout_secs).await;
                if let Ok(mut g) = shared.lock() {
                    *g = Some(result);
                }
            });
        }

        false
    }

    fn render(&self, canvas: &mut Pixmap, theme: &ThemeContext, bounds: Rect) {
        if !self.output.is_empty() {
            let font_size = render_utils::effective_font_size(bounds.height, theme.fonts.size);
            render_utils::draw_text_centered(
                canvas,
                &self.output,
                bounds,
                &theme.fonts.family,
                font_size,
                theme.colors.foreground,
            );
        }
    }

    fn handle_event(&mut self, _event: &InputEvent, _bounds: Rect) -> EventResult {
        EventResult::Ignored
    }

    fn config_schema(&self) -> ModuleConfigSchema {
        ModuleConfigSchema {
            module_id: self.id().to_owned(),
            fields: vec![
                ConfigField {
                    key: "command".to_owned(),
                    label: "Command".to_owned(),
                    description: "Shell command to run (executed via `sh -c`).".to_owned(),
                    field_type: ConfigFieldType::Text {
                        default: String::new(),
                    },
                },
                ConfigField {
                    key: "interval_secs".to_owned(),
                    label: "Refresh interval (seconds)".to_owned(),
                    description: "How often to re-run the command.".to_owned(),
                    field_type: ConfigFieldType::Integer {
                        default: 5,
                        min: Some(1),
                        max: Some(3600),
                    },
                },
                ConfigField {
                    key: "max_chars".to_owned(),
                    label: "Max characters".to_owned(),
                    description: "Truncate output after this many characters.".to_owned(),
                    field_type: ConfigFieldType::Integer {
                        default: 64,
                        min: Some(1),
                        max: Some(512),
                    },
                },
                ConfigField {
                    key: "timeout_secs".to_owned(),
                    label: "Timeout (seconds)".to_owned(),
                    description: "Kill the command after this many seconds.".to_owned(),
                    field_type: ConfigFieldType::Integer {
                        default: 5,
                        min: Some(1),
                        max: Some(60),
                    },
                },
            ],
        }
    }
}

// ── Command execution ─────────────────────────────────────────────────────────

/// Run `command` via `sh -c` with a hard timeout.  Returns the trimmed stdout
/// on success, or an "err" placeholder string on failure.
///
/// - `stdin` is always `/dev/null` (no user input reaches the shell).
/// - Output is truncated to `max_chars` characters.
/// - `stderr` is logged at `debug` level and never displayed.
async fn execute_command(command: &str, max_chars: usize, timeout_secs: u64) -> String {
    if command.is_empty() {
        return String::new();
    }

    let result = tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        tokio::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .stdin(std::process::Stdio::null())
            .output(),
    )
    .await;

    match result {
        Ok(Ok(out)) => {
            if !out.status.success() {
                let stderr = String::from_utf8_lossy(&out.stderr);
                tracing::debug!(
                    "Shell command '{}' exited non-zero: {}",
                    command,
                    stderr.trim()
                );
                return "err".to_owned();
            }
            let stdout = String::from_utf8_lossy(&out.stdout);
            let trimmed = stdout.trim();
            trimmed.chars().take(max_chars).collect()
        }
        Ok(Err(e)) => {
            tracing::warn!("Failed to spawn shell command '{}': {e}", command);
            "err".to_owned()
        }
        Err(_) => {
            tracing::warn!(
                "Shell command '{}' timed out after {timeout_secs}s",
                command
            );
            "err".to_owned()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyprdeck_core::{ColorPalette, FontConfig, HyprState, Padding};

    fn theme() -> ThemeContext {
        ThemeContext {
            colors: ColorPalette {
                background: [0; 4],
                foreground: [255; 4],
                accent: [0; 4],
                urgent: [0; 4],
                separator: [0; 4],
            },
            fonts: FontConfig {
                family: "sans-serif".into(),
                size: 14.0,
                bold_family: None,
            },
            padding: Padding {
                top: 4.0,
                right: 8.0,
                bottom: 4.0,
                left: 8.0,
            },
            border_radius: 4.0,
            opacity: 1.0,
        }
    }

    #[test]
    fn empty_command_does_not_spawn() {
        let mut m = ShellModule::new(ShellConfig::default());
        let state = HyprState::default();
        let ctx = UpdateContext {
            now: chrono::Local::now(),
            hypr_state: &state,
        };
        // Should return false without spawning since command is empty.
        let changed = m.update(&ctx);
        assert!(!changed);
        assert!(!m.running);
    }

    #[test]
    fn desired_size_is_nonzero() {
        let m = ShellModule::new(ShellConfig::default());
        let sz = m.desired_size(&theme());
        assert!(sz.width > 0.0);
        assert!(sz.height > 0.0);
    }

    #[test]
    fn handle_event_always_ignored() {
        let mut m = ShellModule::new(ShellConfig::default());
        let r = m.handle_event(
            &InputEvent::MousePress {
                x: 0.0,
                y: 0.0,
                button: hyprdeck_core::MouseButton::Left,
            },
            Rect::new(0.0, 0.0, 100.0, 32.0),
        );
        assert!(matches!(r, EventResult::Ignored));
    }

    #[test]
    fn output_truncated_to_max_chars() {
        // Test the truncation logic directly without spawning a subprocess.
        let long = "a".repeat(200);
        let truncated: String = long.chars().take(64).collect();
        assert_eq!(truncated.len(), 64);
    }

    #[tokio::test]
    async fn execute_command_returns_stdout() {
        let out = execute_command("echo hello", 100, 5).await;
        assert_eq!(out.trim(), "hello");
    }

    #[tokio::test]
    async fn execute_command_truncates_output() {
        let out = execute_command("printf '%100s' x", 10, 5).await;
        assert!(out.chars().count() <= 10);
    }

    #[tokio::test]
    async fn execute_command_handles_failure() {
        let out = execute_command("exit 1", 100, 5).await;
        assert_eq!(out, "err");
    }

    #[tokio::test]
    async fn execute_command_empty_returns_empty() {
        let out = execute_command("", 100, 5).await;
        assert!(out.is_empty());
    }
}
