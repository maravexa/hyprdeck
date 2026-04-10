use serde::Deserialize;
use hyprdeck_core::Pixmap;

use hyprdeck_core::{
    ConfigField, ConfigFieldType, EventResult, InputEvent, ModuleConfigSchema, PanelModule, Rect,
    Size, ThemeContext, UpdateContext,
};

/// Configuration for the workspace switcher module.
#[derive(Debug, Default, Deserialize)]
pub struct WorkspacesConfig {
    /// Show workspace names instead of numbers.
    #[serde(default)]
    pub show_names: bool,
    /// Only show workspaces that have open windows.
    #[serde(default)]
    pub hide_empty: bool,
    /// Highlight workspaces with urgent windows.
    #[serde(default = "default_true")]
    pub highlight_urgent: bool,
}

fn default_true() -> bool {
    true
}

/// Runtime state for the workspace switcher.
pub struct WorkspacesModule {
    config: WorkspacesConfig,
    /// Cached workspace list from the last update tick.
    workspaces: Vec<hyprdeck_core::Workspace>,
    active_id: i32,
}

impl WorkspacesModule {
    pub fn new(config: WorkspacesConfig) -> Self {
        WorkspacesModule {
            config,
            workspaces: Vec::new(),
            active_id: 1,
        }
    }
}

impl PanelModule for WorkspacesModule {
    fn id(&self) -> &str {
        "workspaces"
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
        todo!()
    }

    fn config_schema(&self) -> ModuleConfigSchema {
        ModuleConfigSchema {
            module_id: self.id().to_owned(),
            fields: vec![
                ConfigField {
                    key: "show_names".to_owned(),
                    label: "Show workspace names".to_owned(),
                    description: "Display workspace names instead of numbers.".to_owned(),
                    field_type: ConfigFieldType::Boolean { default: false },
                },
                ConfigField {
                    key: "hide_empty".to_owned(),
                    label: "Hide empty workspaces".to_owned(),
                    description: "Only show workspaces that contain at least one window.".to_owned(),
                    field_type: ConfigFieldType::Boolean { default: false },
                },
                ConfigField {
                    key: "highlight_urgent".to_owned(),
                    label: "Highlight urgent".to_owned(),
                    description: "Use the urgent colour for workspaces with urgent windows.".to_owned(),
                    field_type: ConfigFieldType::Boolean { default: true },
                },
            ],
        }
    }
}
