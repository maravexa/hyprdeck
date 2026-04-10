use serde::Deserialize;
use hyprdeck_core::Pixmap;

use hyprdeck_core::{
    ConfigField, ConfigFieldType, EventResult, InputEvent, ModuleConfigSchema, PanelModule, Rect,
    Size, ThemeContext, UpdateContext,
};

/// Configuration for the digital clock module.
#[derive(Debug, Deserialize)]
pub struct ClockConfig {
    /// `strftime`-compatible format string.
    #[serde(default = "default_format")]
    pub format: String,
    /// Show a second time zone below the primary clock.
    #[serde(default)]
    pub secondary_timezone: Option<String>,
}

fn default_format() -> String {
    "%H:%M".to_owned()
}

impl Default for ClockConfig {
    fn default() -> Self {
        ClockConfig {
            format: default_format(),
            secondary_timezone: None,
        }
    }
}

/// Runtime state for the clock module.
pub struct ClockModule {
    config: ClockConfig,
    /// Formatted time string cached from the last update tick.
    cached_text: String,
}

impl ClockModule {
    pub fn new(config: ClockConfig) -> Self {
        ClockModule {
            config,
            cached_text: String::new(),
        }
    }
}

impl PanelModule for ClockModule {
    fn id(&self) -> &str {
        "clock"
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
                    key: "format".to_owned(),
                    label: "Time format".to_owned(),
                    description: "strftime format string, e.g. \"%H:%M\" or \"%I:%M %p\".".to_owned(),
                    field_type: ConfigFieldType::Text { default: default_format() },
                },
                ConfigField {
                    key: "secondary_timezone".to_owned(),
                    label: "Secondary time zone".to_owned(),
                    description: "Optional IANA time-zone name shown below the primary clock.".to_owned(),
                    field_type: ConfigFieldType::Text { default: String::new() },
                },
            ],
        }
    }
}
