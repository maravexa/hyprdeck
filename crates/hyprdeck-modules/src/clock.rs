//! Digital clock module — displays the current time.
//!
//! Format is a `strftime`-compatible string (via [`chrono::format`]).
//! `update()` returns `true` only when the formatted string changes, so a
//! `"%H:%M"` format (no seconds) triggers redraws once per minute.

use chrono::TimeZone as _;
use serde::Deserialize;

use hyprdeck_core::{
    ConfigField, ConfigFieldType, EventResult, InputEvent, ModuleConfigSchema, PanelModule, Pixmap,
    PopupContent, PopupEventResult, Rect, Size, ThemeContext, UpdateContext,
};

use crate::render_utils;

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
    /// Stable representative string used for width estimation in `desired_size`.
    width_sample: String,
}

impl ClockModule {
    pub fn new(config: ClockConfig) -> Self {
        let width_sample = make_width_sample(&config.format);
        ClockModule {
            config,
            cached_text: String::new(),
            width_sample,
        }
    }
}

/// Format a fixed reference time with `fmt` to get a stable representative
/// string for width calculations (avoids jitter as digits change).
fn make_width_sample(fmt: &str) -> String {
    // 2000-01-20 20:30:00 — wide digits in common positions.
    match chrono::Local.with_ymd_and_hms(2000, 1, 20, 20, 30, 0) {
        chrono::LocalResult::Single(dt) => dt.format(fmt).to_string(),
        _ => "00:00 PM".to_owned(),
    }
}

impl PanelModule for ClockModule {
    fn id(&self) -> &str {
        "clock"
    }

    fn desired_size(&self, theme: &ThemeContext) -> Size {
        let sample = if self.width_sample.is_empty() {
            "00:00"
        } else {
            &self.width_sample
        };
        let h = theme.fonts.size + theme.padding.top + theme.padding.bottom;
        let font_size = render_utils::effective_font_size(h, theme.fonts.size);
        let w = render_utils::estimate_text_width(sample, font_size)
            + theme.padding.left
            + theme.padding.right;
        Size::new(w, h)
    }

    fn update(&mut self, ctx: &UpdateContext<'_>) -> bool {
        let new_text = ctx.now.format(&self.config.format).to_string();
        if new_text != self.cached_text {
            self.cached_text = new_text;
            true
        } else {
            false
        }
    }

    fn render(&self, canvas: &mut Pixmap, theme: &ThemeContext, bounds: Rect) {
        let font_size = render_utils::effective_font_size(bounds.height, theme.fonts.size);
        render_utils::draw_text_centered(
            canvas,
            &self.cached_text,
            bounds,
            &theme.fonts.family,
            font_size,
            theme.colors.foreground,
        );
    }

    fn handle_event(&mut self, _event: &InputEvent, _bounds: Rect) -> EventResult {
        EventResult::Ignored
    }

    fn has_popup(&self) -> bool {
        true
    }

    fn popup_content(&self) -> Option<Box<dyn PopupContent>> {
        Some(Box::new(ClockPopup))
    }

    fn config_schema(&self) -> ModuleConfigSchema {
        ModuleConfigSchema {
            module_id: self.id().to_owned(),
            fields: vec![
                ConfigField {
                    key: "format".to_owned(),
                    label: "Time format".to_owned(),
                    description: "strftime format string, e.g. \"%H:%M\" or \"%I:%M %p\"."
                        .to_owned(),
                    field_type: ConfigFieldType::Text {
                        default: default_format(),
                    },
                },
                ConfigField {
                    key: "secondary_timezone".to_owned(),
                    label: "Secondary time zone".to_owned(),
                    description: "Optional IANA time-zone name shown below the primary clock."
                        .to_owned(),
                    field_type: ConfigFieldType::Text {
                        default: String::new(),
                    },
                },
            ],
        }
    }
}

// ── Clock popup ───────────────────────────────────────────────────────────────

/// Popup content for the clock module — shows HH:MM:SS with timezone.
///
/// `update()` returns `true` every tick so the seconds counter stays live.
pub struct ClockPopup;

impl PopupContent for ClockPopup {
    fn desired_size(&self, _theme: &ThemeContext) -> Size {
        Size::new(240.0, 80.0)
    }

    fn render(&self, canvas: &mut Pixmap, theme: &ThemeContext, bounds: Rect) {
        let bold = theme
            .fonts
            .bold_family
            .as_deref()
            .unwrap_or(&theme.fonts.family);
        let now = chrono::Local::now();

        // Large HH:MM:SS
        let time_str = now.format("%H:%M:%S").to_string();
        let time_rect = Rect::new(bounds.x, bounds.y, bounds.width, 46.0);
        render_utils::draw_text_centered(canvas, &time_str, time_rect, bold, 30.0, theme.colors.foreground);

        // Timezone label (dimmed)
        let tz_str = now.format("%Z (UTC%:z)").to_string();
        let dim = [
            theme.colors.foreground[0],
            theme.colors.foreground[1],
            theme.colors.foreground[2],
            (theme.colors.foreground[3] as f32 * 0.6) as u8,
        ];
        let tz_rect = Rect::new(bounds.x, bounds.y + 50.0, bounds.width, 24.0);
        render_utils::draw_text_centered(canvas, &tz_str, tz_rect, &theme.fonts.family, 12.0, dim);
    }

    fn handle_event(&mut self, _event: &InputEvent, _bounds: Rect) -> PopupEventResult {
        PopupEventResult::Ignored
    }

    fn update(&mut self) -> bool {
        true // always redraw so the seconds counter ticks
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyprdeck_core::{ColorPalette, FontConfig, HyprState, Padding};

    fn theme() -> ThemeContext {
        use hyprdeck_core::ResolvedModuleStyles;
        ThemeContext {
            colors: ColorPalette {
                background: [30, 30, 30, 230],
                foreground: [255, 255, 255, 255],
                accent: [80, 160, 255, 255],
                urgent: [255, 80, 80, 255],
                separator: [128, 128, 128, 128],
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
        }
    }

    #[test]
    fn default_format_is_hhmm() {
        assert_eq!(default_format(), "%H:%M");
    }

    #[test]
    fn desired_size_is_nonzero() {
        let m = ClockModule::new(ClockConfig::default());
        let sz = m.desired_size(&theme());
        assert!(sz.width > 0.0, "width should be > 0");
        assert!(sz.height > 0.0, "height should be > 0");
    }

    #[test]
    fn update_returns_true_when_text_changes() {
        let mut m = ClockModule::new(ClockConfig {
            format: "%S".to_owned(),
            secondary_timezone: None,
        });
        let state = HyprState::default();
        let ctx1 = UpdateContext {
            now: chrono::Local.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap(),
            hypr_state: &state,
            output_name: "",
        };
        assert!(m.update(&ctx1), "first update should always return true");
        // Same second — no change.
        assert!(!m.update(&ctx1), "same second should return false");
    }

    #[test]
    fn update_returns_false_when_minute_unchanged_without_seconds() {
        let mut m = ClockModule::new(ClockConfig::default()); // "%H:%M"
        let state = HyprState::default();
        let t1 = chrono::Local
            .with_ymd_and_hms(2000, 1, 1, 12, 30, 0)
            .unwrap();
        let t2 = chrono::Local
            .with_ymd_and_hms(2000, 1, 1, 12, 30, 45)
            .unwrap();
        let ctx1 = UpdateContext {
            now: t1,
            hypr_state: &state,
            output_name: "",
        };
        let ctx2 = UpdateContext {
            now: t2,
            hypr_state: &state,
            output_name: "",
        };
        assert!(m.update(&ctx1));
        assert!(!m.update(&ctx2), "same H:M should not trigger redraw");
    }

    #[test]
    fn handle_event_always_ignored() {
        let mut m = ClockModule::new(ClockConfig::default());
        let result = m.handle_event(
            &InputEvent::MousePress {
                x: 0.0,
                y: 0.0,
                button: hyprdeck_core::MouseButton::Left,
            },
            Rect::new(0.0, 0.0, 100.0, 32.0),
        );
        assert!(matches!(result, EventResult::Ignored));
    }

    #[test]
    fn config_schema_contains_expected_keys() {
        let m = ClockModule::new(ClockConfig::default());
        let schema = m.config_schema();
        let keys: Vec<&str> = schema.fields.iter().map(|f| f.key.as_str()).collect();
        assert!(keys.contains(&"format"));
        assert!(keys.contains(&"secondary_timezone"));
    }
}
