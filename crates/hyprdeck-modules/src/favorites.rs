use hyprdeck_core::Pixmap;
use serde::Deserialize;

use hyprdeck_core::{
    Action, ConfigField, ConfigFieldType, EventResult, InputEvent, ModuleConfigSchema, PanelModule,
    Rect, Size, ThemeContext, UpdateContext,
};

/// A single pinned application or action entry.
#[derive(Debug, Deserialize)]
pub struct FavoriteEntry {
    /// Displayed tooltip / label.
    pub label: String,
    /// XDG icon name used to look up the app icon.
    pub icon: String,
    /// Action executed on left-click.
    pub action: Action,
}

/// Configuration for the pinned-apps / favorites module.
#[derive(Debug, Default, Deserialize)]
pub struct FavoritesConfig {
    /// Ordered list of pinned entries.
    #[serde(default)]
    pub entries: Vec<FavoriteEntry>,
    /// Icon size in logical pixels (overrides theme default).
    pub icon_size: Option<f32>,
    /// Show running-indicator dots under pinned icons that have open windows.
    #[serde(default = "default_true")]
    pub show_running_indicator: bool,
}

fn default_true() -> bool {
    true
}

/// Runtime state for the favorites / dock shortcut module.
pub struct FavoritesModule {
    config: FavoritesConfig,
    /// Index of the icon slot currently under the pointer (-1 = none).
    hovered_index: Option<usize>,
}

impl FavoritesModule {
    pub fn new(config: FavoritesConfig) -> Self {
        FavoritesModule {
            config,
            hovered_index: None,
        }
    }
}

impl PanelModule for FavoritesModule {
    fn id(&self) -> &str {
        "favorites"
    }

    fn desired_size(&self, theme: &ThemeContext) -> Size {
        todo!()
    }

    fn update(&mut self, ctx: &UpdateContext<'_>) -> bool {
        false
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
                    key: "icon_size".to_owned(),
                    label: "Icon size".to_owned(),
                    description: "Override icon size in logical pixels.".to_owned(),
                    field_type: ConfigFieldType::Float {
                        default: 24.0,
                        min: Some(16.0),
                        max: Some(64.0),
                    },
                },
                ConfigField {
                    key: "show_running_indicator".to_owned(),
                    label: "Show running indicator".to_owned(),
                    description: "Show a dot under icons with open windows.".to_owned(),
                    field_type: ConfigFieldType::Boolean { default: true },
                },
            ],
        }
    }
}
