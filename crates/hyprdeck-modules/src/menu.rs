use serde::Deserialize;
use hyprdeck_core::Pixmap;

use hyprdeck_core::{
    Action, ConfigField, ConfigFieldType, EventResult, InputEvent, ModuleConfigSchema, PanelModule,
    Rect, Size, ThemeContext, UpdateContext,
};

/// Configuration for the application menu / start button module.
#[derive(Debug, Deserialize)]
pub struct MenuConfig {
    /// Label text displayed on the button; empty string shows icon only.
    #[serde(default = "default_label")]
    pub label: String,
    /// XDG icon name for the button icon.
    #[serde(default = "default_icon")]
    pub icon: String,
    /// Action triggered when the button is clicked.
    #[serde(default = "default_action")]
    pub action: Action,
}

fn default_label() -> String {
    String::new()
}

fn default_icon() -> String {
    "start-here".to_owned()
}

fn default_action() -> Action {
    Action::Exec {
        command: "wofi".to_owned(),
        args: vec!["--show".to_owned(), "drun".to_owned()],
    }
}

impl Default for MenuConfig {
    fn default() -> Self {
        MenuConfig {
            label: default_label(),
            icon: default_icon(),
            action: default_action(),
        }
    }
}

/// Runtime state for the menu button module.
pub struct MenuModule {
    config: MenuConfig,
    /// Whether the button is currently in a pressed visual state.
    pressed: bool,
}

impl MenuModule {
    pub fn new(config: MenuConfig) -> Self {
        MenuModule { config, pressed: false }
    }
}

impl PanelModule for MenuModule {
    fn id(&self) -> &str {
        "menu_button"
    }

    fn desired_size(&self, theme: &ThemeContext) -> Size {
        todo!()
    }

    fn update(&mut self, ctx: &UpdateContext<'_>) -> bool {
        false // Stateless apart from pressed state.
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
                    key: "label".to_owned(),
                    label: "Button label".to_owned(),
                    description: "Text shown on the menu button. Empty string for icon-only mode.".to_owned(),
                    field_type: ConfigFieldType::Text { default: String::new() },
                },
                ConfigField {
                    key: "icon".to_owned(),
                    label: "Icon name".to_owned(),
                    description: "XDG icon name for the button (e.g. \"start-here\", \"distributor-logo-arch\").".to_owned(),
                    field_type: ConfigFieldType::Text { default: "start-here".to_owned() },
                },
            ],
        }
    }
}
