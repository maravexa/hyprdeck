use serde::Deserialize;
use hyprdeck_core::Pixmap;

use hyprdeck_core::{
    ConfigField, ConfigFieldType, EventResult, InputEvent, ModuleConfigSchema, PanelModule, Rect,
    Size, ThemeContext, UpdateContext, WindowInfo,
};

/// How to display windows in the list.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WindowListStyle {
    /// Text buttons with window title (classic taskbar).
    #[default]
    Buttons,
    /// Icon-only compact view.
    Icons,
    /// Icon plus truncated title.
    IconLabel,
}

/// Configuration for the window list / taskbar module.
#[derive(Debug, Default, Deserialize)]
pub struct WindowListConfig {
    #[serde(default)]
    pub style: WindowListStyle,
    /// Limit list to windows on the current workspace only.
    #[serde(default = "default_true")]
    pub current_workspace_only: bool,
    /// Maximum button width in logical pixels before title is truncated.
    #[serde(default = "default_max_width")]
    pub max_button_width: f32,
}

fn default_true() -> bool {
    true
}

fn default_max_width() -> f32 {
    200.0
}

/// Runtime state for the window list module.
pub struct WindowListModule {
    config: WindowListConfig,
    /// Cached window list from the last update.
    windows: Vec<WindowInfo>,
    /// Index of the button currently hovered.
    hovered: Option<usize>,
}

impl WindowListModule {
    pub fn new(config: WindowListConfig) -> Self {
        WindowListModule {
            config,
            windows: Vec::new(),
            hovered: None,
        }
    }
}

impl PanelModule for WindowListModule {
    fn id(&self) -> &str {
        "window_list"
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
                    key: "style".to_owned(),
                    label: "Display style".to_owned(),
                    description: "How window buttons are rendered.".to_owned(),
                    field_type: ConfigFieldType::Choice {
                        options: vec![
                            "buttons".to_owned(),
                            "icons".to_owned(),
                            "iconlabel".to_owned(),
                        ],
                        default: "buttons".to_owned(),
                    },
                },
                ConfigField {
                    key: "current_workspace_only".to_owned(),
                    label: "Current workspace only".to_owned(),
                    description: "Only show windows from the active workspace.".to_owned(),
                    field_type: ConfigFieldType::Boolean { default: true },
                },
                ConfigField {
                    key: "max_button_width".to_owned(),
                    label: "Max button width".to_owned(),
                    description: "Maximum logical pixel width of each window button.".to_owned(),
                    field_type: ConfigFieldType::Float {
                        default: 200.0,
                        min: Some(60.0),
                        max: Some(600.0),
                    },
                },
            ],
        }
    }
}
