use serde::{Deserialize, Serialize};

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
    /// Forward a named action to a specific module.
    ModuleAction { module: String, action: String },
    /// Execute multiple actions sequentially.
    Chain { actions: Vec<Action> },
}
