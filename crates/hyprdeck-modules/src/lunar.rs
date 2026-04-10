use serde::Deserialize;
use hyprdeck_core::Pixmap;

use hyprdeck_core::{
    ConfigField, ConfigFieldType, EventResult, InputEvent, ModuleConfigSchema, PanelModule, Rect,
    Size, ThemeContext, UpdateContext,
};

/// Configuration for the lunar phase module.
///
/// Lunar phase calculations are provided by the `fn0rd` crate, shared with HyprSaver.
#[derive(Debug, Default, Deserialize)]
pub struct LunarConfig {
    /// Show the phase name as text next to the icon.
    #[serde(default)]
    pub show_label: bool,
    /// Calendar locale for phase names (BCP-47 tag, e.g. "en", "zh-TW").
    #[serde(default = "default_locale")]
    pub locale: String,
}

fn default_locale() -> String {
    "en".to_owned()
}

/// Runtime state for the lunar phase display module.
pub struct LunarModule {
    config: LunarConfig,
    /// Phase angle in degrees [0, 360) cached from the last update.
    cached_phase_deg: f64,
    last_date: Option<chrono::NaiveDate>,
}

impl LunarModule {
    pub fn new(config: LunarConfig) -> Self {
        LunarModule {
            config,
            cached_phase_deg: 0.0,
            last_date: None,
        }
    }
}

impl PanelModule for LunarModule {
    fn id(&self) -> &str {
        "lunar"
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
                    key: "show_label".to_owned(),
                    label: "Show phase label".to_owned(),
                    description: "Display the moon phase name next to the icon.".to_owned(),
                    field_type: ConfigFieldType::Boolean { default: false },
                },
                ConfigField {
                    key: "locale".to_owned(),
                    label: "Locale".to_owned(),
                    description: "BCP-47 language tag for phase name localisation (e.g. \"en\", \"zh-TW\").".to_owned(),
                    field_type: ConfigFieldType::Text { default: "en".to_owned() },
                },
            ],
        }
    }
}
