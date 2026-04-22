//! Weather module — displays current conditions from Open-Meteo.
//!
//! HTTP fetches run in a background `tokio` task so `update()` never blocks.
//! The task result is communicated back via an `Arc<Mutex<Option<String>>>`.
//! Open-Meteo is used by default because it requires no API key.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Deserialize;

use hyprdeck_core::{
    ConfigField, ConfigFieldType, EventResult, InputEvent, ModuleConfigSchema, PanelModule, Pixmap,
    Rect, Size, ThemeContext, UpdateContext,
};

use crate::render_utils;

// ── Config ────────────────────────────────────────────────────────────────────

/// Temperature unit for weather display.
#[derive(Debug, Default, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum TemperatureUnit {
    #[default]
    Celsius,
    Fahrenheit,
}

/// Configuration for the weather module.
#[derive(Debug, Default, Deserialize)]
pub struct WeatherConfig {
    /// Location as "lat,lon" (e.g. "51.5,-0.1").  Required; module shows "—"
    /// if absent.
    pub location: Option<String>,
    /// Temperature unit.
    #[serde(default)]
    pub unit: TemperatureUnit,
    /// How often to refresh weather data, in minutes.
    #[serde(default = "default_refresh_minutes")]
    pub refresh_minutes: u64,
}

fn default_refresh_minutes() -> u64 {
    30
}

// ── Module ────────────────────────────────────────────────────────────────────

/// Runtime state for the weather module.
pub struct WeatherModule {
    config: WeatherConfig,
    /// Latest display text derived from the most recent successful fetch.
    display_text: String,
    /// Shared data slot written by the background fetch task.
    shared: Arc<Mutex<Option<String>>>,
    /// When the most recent fetch was started.
    last_fetch: Option<Instant>,
    /// Whether a background fetch task is currently running.
    fetch_in_flight: bool,
}

impl WeatherModule {
    pub fn new(config: WeatherConfig) -> Self {
        if config.location.is_none() {
            tracing::warn!(
                "Weather module: no location configured. \
                 Set `location = \"lat,lon\"` in the weather module config."
            );
        }
        WeatherModule {
            config,
            display_text: "—".to_owned(),
            shared: Arc::new(Mutex::new(None)),
            last_fetch: None,
            fetch_in_flight: false,
        }
    }

    fn poll_due(&self) -> bool {
        match self.last_fetch {
            None => true,
            Some(t) => t.elapsed() >= Duration::from_secs(self.config.refresh_minutes * 60),
        }
    }
}

impl PanelModule for WeatherModule {
    fn id(&self) -> &str {
        "weather"
    }

    fn desired_size(&self, theme: &ThemeContext) -> Size {
        let sample = if self.display_text == "—" {
            "☀ 00°C"
        } else {
            &self.display_text
        };
        let h = theme.fonts.size + theme.padding.top + theme.padding.bottom;
        let font_size = render_utils::effective_font_size(h, theme.fonts.size);
        let w = render_utils::estimate_text_width(sample, font_size)
            + theme.padding.left
            + theme.padding.right;
        Size::new(w, h)
    }

    fn update(&mut self, _ctx: &UpdateContext<'_>) -> bool {
        // Check if the background task produced a new result.
        let new_text = self.shared.try_lock().ok().and_then(|g| g.clone());

        if let Some(text) = new_text {
            if text != self.display_text {
                self.display_text = text;
                self.fetch_in_flight = false;
                // Schedule the next fetch after the full interval from now.
                self.last_fetch = Some(Instant::now());
                return true;
            } else {
                self.fetch_in_flight = false;
                self.last_fetch = Some(Instant::now());
            }
        }

        // Spawn a new fetch if the poll interval has elapsed.
        if self.poll_due() && !self.fetch_in_flight {
            let Some(location) = self.config.location.clone() else {
                return false;
            };
            let unit = self.config.unit.clone();
            let shared = Arc::clone(&self.shared);

            self.last_fetch = Some(Instant::now());
            self.fetch_in_flight = true;

            // Reset shared slot before spawning so stale data is not re-read.
            if let Ok(mut g) = shared.lock() {
                *g = None;
            }

            tokio::spawn(async move {
                match fetch_weather(&location, &unit).await {
                    Ok(text) => {
                        if let Ok(mut g) = shared.lock() {
                            *g = Some(text);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Weather fetch error: {e}");
                        // Put an error placeholder so the module knows the fetch finished.
                        if let Ok(mut g) = shared.lock() {
                            *g = Some("?°".to_owned());
                        }
                    }
                }
            });
        }

        false
    }

    fn render(&self, canvas: &mut Pixmap, theme: &ThemeContext, bounds: Rect) {
        let font_size = render_utils::effective_font_size(bounds.height, theme.fonts.size);
        let font = theme
            .fonts
            .bold_family
            .as_deref()
            .unwrap_or(&theme.fonts.family);
        render_utils::draw_text_centered(
            canvas,
            &self.display_text,
            bounds,
            font,
            font_size,
            theme.colors.foreground,
        );
    }

    fn handle_event(&mut self, _event: &InputEvent, _bounds: Rect) -> EventResult {
        EventResult::Ignored
    }

    fn config_schema(&self) -> ModuleConfigSchema {
        ModuleConfigSchema {
            module_id: self.id().to_owned(),
            fields: vec![
                ConfigField {
                    key: "location".to_owned(),
                    label: "Location".to_owned(),
                    description: "Latitude and longitude as \"lat,lon\", e.g. \"51.5,-0.1\"."
                        .to_owned(),
                    field_type: ConfigFieldType::Text {
                        default: String::new(),
                    },
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

// ── HTTP fetch ────────────────────────────────────────────────────────────────

/// Parse a "lat,lon" string into (latitude, longitude).
fn parse_location(location: &str) -> Option<(f64, f64)> {
    let mut parts = location.splitn(2, ',');
    let lat: f64 = parts.next()?.trim().parse().ok()?;
    let lon: f64 = parts.next()?.trim().parse().ok()?;
    Some((lat, lon))
}

/// Fetch current weather from Open-Meteo and return a formatted display string.
async fn fetch_weather(location: &str, unit: &TemperatureUnit) -> Result<String, String> {
    let (lat, lon) =
        parse_location(location).ok_or_else(|| format!("Invalid location string: '{location}'"))?;

    let temp_unit = match unit {
        TemperatureUnit::Celsius => "celsius",
        TemperatureUnit::Fahrenheit => "fahrenheit",
    };

    let url = format!(
        "https://api.open-meteo.com/v1/forecast\
         ?latitude={lat}&longitude={lon}\
         &current=temperature_2m,weather_code\
         &temperature_unit={temp_unit}"
    );

    let resp: serde_json::Value = reqwest::get(&url)
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let current = &resp["current"];
    let temperature = current["temperature_2m"].as_f64().unwrap_or(0.0);
    let wmo_code = current["weather_code"].as_i64().unwrap_or(0);

    let icon = wmo_icon(wmo_code);
    let unit_str = match unit {
        TemperatureUnit::Celsius => "°C",
        TemperatureUnit::Fahrenheit => "°F",
    };

    Ok(format!("{icon} {temperature:.0}{unit_str}"))
}

/// Map a WMO weather interpretation code to a display emoji.
fn wmo_icon(code: i64) -> &'static str {
    match code {
        0 => "☀",
        1..=3 => "⛅",
        45 | 48 => "🌫",
        51..=67 => "🌧",
        71..=77 => "❄",
        80..=82 => "🌧",
        85 | 86 => "❄",
        95..=99 => "⛈",
        _ => "☁",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyprdeck_core::HyprState;

    #[test]
    fn wmo_icon_covers_all_ranges() {
        assert_eq!(wmo_icon(0), "☀");
        assert_eq!(wmo_icon(2), "⛅");
        assert_eq!(wmo_icon(45), "🌫");
        assert_eq!(wmo_icon(55), "🌧");
        assert_eq!(wmo_icon(73), "❄");
        assert_eq!(wmo_icon(80), "🌧");
        assert_eq!(wmo_icon(85), "❄");
        assert_eq!(wmo_icon(95), "⛈");
        assert_eq!(wmo_icon(200), "☁");
    }

    #[test]
    fn parse_location_valid() {
        let (lat, lon) = parse_location("51.5,-0.1").unwrap();
        assert!((lat - 51.5).abs() < 1e-9);
        assert!((lon - (-0.1)).abs() < 1e-9);
    }

    #[test]
    fn parse_location_with_spaces() {
        let r = parse_location(" 48.8 , 2.3 ");
        assert!(r.is_some());
    }

    #[test]
    fn parse_location_invalid_returns_none() {
        assert!(parse_location("not_a_location").is_none());
        assert!(parse_location("").is_none());
    }

    #[test]
    fn no_location_logs_warning_and_returns_dash() {
        let m = WeatherModule::new(WeatherConfig::default());
        assert_eq!(m.display_text, "—");
    }

    #[test]
    fn update_does_not_block_without_location() {
        let mut m = WeatherModule::new(WeatherConfig::default());
        let state = HyprState::default();
        let ctx = UpdateContext {
            now: chrono::Local::now(),
            hypr_state: &state,
            output_name: "",
        };
        // Should return false without blocking since no location is set.
        let changed = m.update(&ctx);
        assert!(!changed);
    }

    #[test]
    fn desired_size_is_nonzero() {
        let m = WeatherModule::new(WeatherConfig::default());
        use hyprdeck_core::{ColorPalette, FontConfig, Padding, ResolvedModuleStyles};
        let theme = ThemeContext {
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
            module_styles: ResolvedModuleStyles::default(),
        };
        let sz = m.desired_size(&theme);
        assert!(sz.width > 0.0);
        assert!(sz.height > 0.0);
    }
}
