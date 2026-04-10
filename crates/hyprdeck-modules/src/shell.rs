use serde::Deserialize;
use hyprdeck_core::Pixmap;

use hyprdeck_core::{
    ConfigField, ConfigFieldType, EventResult, InputEvent, ModuleConfigSchema, PanelModule, Rect,
    Size, ThemeContext, UpdateContext,
};

/// Configuration for the shell command output module.
///
/// Runs an arbitrary shell command on an interval and displays its stdout in the panel.
/// Useful for custom status indicators not covered by built-in modules.
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
}

fn default_interval() -> u64 {
    5
}

fn default_max_chars() -> usize {
    64
}

impl Default for ShellConfig {
    fn default() -> Self {
        ShellConfig {
            command: String::new(),
            interval_secs: default_interval(),
            max_chars: default_max_chars(),
        }
    }
}

/// Runtime state for the shell output module.
pub struct ShellModule {
    config: ShellConfig,
    /// Last captured stdout from the command.
    output: String,
    /// Handle to the in-flight command task, if any.
    pending: Option<tokio::task::JoinHandle<std::io::Result<String>>>,
    last_run: Option<std::time::Instant>,
}

impl ShellModule {
    pub fn new(config: ShellConfig) -> Self {
        ShellModule {
            config,
            output: String::new(),
            pending: None,
            last_run: None,
        }
    }
}

impl PanelModule for ShellModule {
    fn id(&self) -> &str {
        "shell"
    }

    fn desired_size(&self, theme: &ThemeContext) -> Size {
        todo!()
    }

    fn update(&mut self, ctx: &UpdateContext<'_>) -> bool {
        todo!()
    }

    fn render(&self, canvas: &mut Pixmap, theme: &ThemeContext, bounds: Rect) {
        todo!()
    }

    fn handle_event(&mut self, event: &InputEvent, bounds: Rect) -> EventResult {
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
                    field_type: ConfigFieldType::Text { default: String::new() },
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
            ],
        }
    }
}
