use hyprdeck_core::Pixmap;
use serde::Deserialize;

use hyprdeck_core::{
    ConfigField, ConfigFieldType, EventResult, InputEvent, ModuleConfigSchema, PanelModule, Rect,
    Size, ThemeContext, UpdateContext,
};

/// Temperature unit for weather display.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TemperatureUnit {
    #[default]
    Celsius,
    Fahrenheit,
}

/// Configuration for the weather module.
#[derive(Debug, Default, Deserialize)]
pub struct WeatherConfig {
    /// OpenMeteo location override as "lat,lon" (auto-detected from IP if absent).
    pub location: Option<String>,
    /// Temperature unit to display.
    #[serde(default)]
    pub unit: TemperatureUnit,
    /// How often to refresh weather data, in minutes.
    #[serde(default = "default_refresh_minutes")]
    pub refresh_minutes: u64,
}

fn default_refresh_minutes() -> u64 {
    30
}

/// Cached weather snapshot.
#[derive(Debug, Default)]
struct WeatherSnapshot {
    temperature: f32,
    condition_code: u8,
    condition_text: String,
    fetched_at: Option<std::time::Instant>,
}

/// Runtime state for the weather module.
pub struct WeatherModule {
    config: WeatherConfig,
    snapshot: WeatherSnapshot,
    /// Pending async fetch handle.
    fetch_handle: Option<tokio::task::JoinHandle<Result<WeatherSnapshot, String>>>,
}

impl WeatherModule {
    pub fn new(config: WeatherConfig) -> Self {
        WeatherModule {
            config,
            snapshot: WeatherSnapshot::default(),
            fetch_handle: None,
        }
    }
}

impl PanelModule for WeatherModule {
    fn id(&self) -> &str {
        "weather"
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
                    key: "location".to_owned(),
                    label: "Location".to_owned(),
                    description: "Latitude and longitude as \"lat,lon\", e.g. \"51.5,-0.1\". Leave empty to auto-detect.".to_owned(),
                    field_type: ConfigFieldType::Text { default: String::new() },
                },
                ConfigField {
                    key: "unit".to_owned(),
                    label: "Temperature unit".to_owned(),
                    description: "Display temperature in Celsius or Fahrenheit.".to_owned(),
                    field_type: ConfigFieldType::Choice {
                        options: vec!["celsius".to_owned(), "fahrenheit".to_owned()],
                        default: "celsius".to_owned(),
                    },
                },
                ConfigField {
                    key: "refresh_minutes".to_owned(),
                    label: "Refresh interval (minutes)".to_owned(),
                    description: "How often to fetch new weather data.".to_owned(),
                    field_type: ConfigFieldType::Integer {
                        default: 30,
                        min: Some(5),
                        max: Some(120),
                    },
                },
            ],
        }
    }
}
