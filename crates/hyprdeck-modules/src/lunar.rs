//! Lunar phase display module.
//!
//! Uses the `fn0rd` crate for orbital-mechanics phase calculations.
//! The phase is re-computed at most once per calendar day; intraday updates
//! are skipped.
//!
//! For 1.0, the phase is rendered as a Unicode moon-phase emoji (🌑 … 🌕 …).
//! A Canvas-drawn crescent implementation is noted as a future enhancement.

use chrono::NaiveDate;
use serde::Deserialize;

use fn0rd_lib::moon::calc::{Body, phase_angle, phase_name_for_angle};

use hyprdeck_core::{
    ConfigField, ConfigFieldType, EventResult, InputEvent, ModuleConfigSchema, PanelModule, Pixmap,
    PopupContent, PopupEventResult, Rect, Size, ThemeContext, UpdateContext,
};

use crate::render_utils;

// ── Config ────────────────────────────────────────────────────────────────────

/// How the moon phase icon is rendered.
///
/// Only `Canvas` is implemented for 1.0; the other variants exist as
/// forward-compatibility hooks for future theme-driven icon sets.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LunarRenderMode {
    #[default]
    Canvas,  // tiny-skia drawn (default, always works)
    Icons,   // pre-rendered PNG set from theme (future)
    Emoji,   // Unicode emoji fallback (future)
    Ascii,   // ASCII art via fn0rd (future)
}

/// Configuration for the lunar phase display module.
#[derive(Debug, Default, Deserialize)]
pub struct LunarConfig {
    /// Show the phase name as text next to the icon.
    #[serde(default)]
    pub show_label: bool,
    /// Celestial body to track.  Accepts any name accepted by `fn0rd::moon::calc::Body::parse`,
    /// e.g. "luna", "phobos".  Defaults to "luna".
    #[serde(default = "default_body")]
    pub body: String,
    /// Calendar locale for phase names (BCP-47 tag, e.g. "en", "zh-TW").
    #[serde(default = "default_locale")]
    pub locale: String,
    /// Rendering mode for the moon icon.
    #[serde(default)]
    pub render_mode: LunarRenderMode,
}

fn default_body() -> String {
    "luna".to_owned()
}

fn default_locale() -> String {
    "en".to_owned()
}

// ── Module ────────────────────────────────────────────────────────────────────

/// Runtime state for the lunar phase display module.
pub struct LunarModule {
    config: LunarConfig,
    /// Phase angle in [0.0, 1.0) cached from the last update.
    cached_phase: f64,
    /// Human-readable phase name.
    cached_name: String,
    /// Date on which the cache was last filled.
    last_date: Option<NaiveDate>,
}

impl LunarModule {
    pub fn new(config: LunarConfig) -> Self {
        LunarModule {
            config,
            cached_phase: 0.0,
            cached_name: String::new(),
            last_date: None,
        }
    }

    /// Resolve the configured body string to a `fn0rd` `Body`.  Falls back to
    /// `Body::Luna` if the name is unrecognised, logging a warning once.
    fn resolve_body(name: &str) -> Body {
        Body::parse(name).unwrap_or_else(|| {
            tracing::warn!(
                "Lunar module: unrecognised body '{}', falling back to Luna",
                name
            );
            Body::Luna
        })
    }
}

/// Return the Unicode moon-phase emoji for a phase angle in [0.0, 1.0).
///
/// 0.0 = new moon, 0.5 = full moon.
pub fn moon_emoji(phase: f64) -> &'static str {
    let name = phase_name_for_angle(phase);
    name.emoji()
}

/// Return a scalable Unicode geometric symbol representing the moon phase.
///
/// Unlike color emoji, these symbols are drawn by the text font at whatever
/// size cosmic-text is asked for.  0.0 = new moon (dark), 0.5 = full moon (bright).
pub fn moon_phase_symbol(fraction: f64) -> &'static str {
    let phase_idx = ((fraction * 8.0).round() as u8) % 8;
    match phase_idx {
        0 => "●", // new moon — fully dark
        1 => "◗", // waxing crescent
        2 => "◐", // first quarter
        3 => "◒", // waxing gibbous
        4 => "○", // full moon — fully bright
        5 => "◓", // waning gibbous
        6 => "◑", // last quarter
        7 => "◖", // waning crescent
        _ => "●",
    }
}

impl PanelModule for LunarModule {
    fn id(&self) -> &str {
        "lunar"
    }

    fn desired_size(&self, theme: &ThemeContext) -> Size {
        let height = theme.fonts.size * 2.0;
        if self.config.show_label && !self.cached_name.is_empty() {
            let text_width = render_utils::estimate_text_width(&self.cached_name, theme.fonts.size);
            Size::new(height + text_width + 4.0, height)
        } else {
            Size::new(height, height)
        }
    }

    fn update(&mut self, ctx: &UpdateContext<'_>) -> bool {
        let today = ctx.now.date_naive();
        if self.last_date == Some(today) {
            return false;
        }
        self.last_date = Some(today);
        let body = Self::resolve_body(&self.config.body);
        let new_phase = phase_angle(body, today);
        let new_name = phase_name_for_angle(new_phase).label().to_owned();

        let changed = (new_phase - self.cached_phase).abs() > 1e-9 || new_name != self.cached_name;
        self.cached_phase = new_phase;
        self.cached_name = new_name;
        changed
    }

    fn render(&self, canvas: &mut Pixmap, theme: &ThemeContext, bounds: Rect) {
        let icon_size = bounds.height - 2.0;
        let icon_bounds = Rect::new(bounds.x + 1.0, bounds.y + 1.0, icon_size, icon_size);

        let lit_color = theme.colors.foreground;
        let dark_color = dim_color(theme.colors.background, 0.3);
        render_utils::draw_moon_phase(canvas, icon_bounds, self.cached_phase, lit_color, dark_color);

        if self.config.show_label && !self.cached_name.is_empty() {
            let label_x = bounds.x + icon_size + 4.0;
            let label_rect =
                Rect::new(label_x, bounds.y, bounds.width - icon_size - 4.0, bounds.height);
            let bold = theme.fonts.bold_family.as_deref().unwrap_or(&theme.fonts.family);
            let text_size = render_utils::effective_font_size(bounds.height, theme.fonts.size);
            render_utils::draw_text(canvas, &self.cached_name, label_rect, bold, text_size, theme.colors.foreground);
        }
    }

    fn handle_event(&mut self, _event: &InputEvent, _bounds: Rect) -> EventResult {
        EventResult::Ignored
    }

    fn has_popup(&self) -> bool {
        tracing::debug!("{} has_popup called → true", self.id());
        true
    }

    fn popup_content(&self) -> Option<Box<dyn PopupContent>> {
        tracing::debug!("{} popup_content called", self.id());
        let illumination_fraction = {
            let angle = (self.cached_phase - 0.5) * 2.0 * std::f64::consts::PI;
            (angle.cos() + 1.0) / 2.0
        };
        Some(Box::new(LunarPopup {
            phase_fraction: self.cached_phase,
            body_name: self.config.body.clone(),
            illumination_fraction,
            phase_name: self.cached_name.clone(),
        }))
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
                    key: "body".to_owned(),
                    label: "Celestial body".to_owned(),
                    description:
                        "Which moon to track (luna, phobos, deimos, io, europa, ganymede, titan, triton).".to_owned(),
                    field_type: ConfigFieldType::Text { default: "luna".to_owned() },
                },
                ConfigField {
                    key: "locale".to_owned(),
                    label: "Locale".to_owned(),
                    description:
                        "BCP-47 language tag for phase name localisation (e.g. \"en\", \"zh-TW\").".to_owned(),
                    field_type: ConfigFieldType::Text { default: "en".to_owned() },
                },
                ConfigField {
                    key: "render_mode".to_owned(),
                    label: "Render mode".to_owned(),
                    description: "How the moon icon is drawn (canvas, icons, emoji, ascii).".to_owned(),
                    field_type: ConfigFieldType::Choice {
                        options: vec!["canvas".to_owned(), "icons".to_owned(), "emoji".to_owned(), "ascii".to_owned()],
                        default: "canvas".to_owned(),
                    },
                },
            ],
        }
    }
}

// ── Lunar popup ───────────────────────────────────────────────────────────────

/// Popup content for the lunar module — shows canvas-drawn moon, body name, and illumination.
pub struct LunarPopup {
    phase_fraction: f64,
    body_name: String,
    illumination_fraction: f64,
    phase_name: String,
}

impl PopupContent for LunarPopup {
    fn desired_size(&self, _theme: &ThemeContext) -> Size {
        Size::new(220.0, 180.0)
    }

    fn render(&self, canvas: &mut Pixmap, theme: &ThemeContext, bounds: Rect) {
        let moon_size = 80.0;
        let moon_bounds = Rect::new(
            bounds.x + (bounds.width - moon_size) / 2.0,
            bounds.y,
            moon_size,
            moon_size,
        );
        let lit_color = theme.colors.foreground;
        let dark_color = dim_color(theme.colors.foreground, 0.15);
        render_utils::draw_moon_phase(canvas, moon_bounds, self.phase_fraction, lit_color, dark_color);

        let bold = theme.fonts.bold_family.as_deref().unwrap_or(&theme.fonts.family);
        let font = &theme.fonts.family;

        // Body name (capitalised)
        let mut body_display = self.body_name.clone();
        if let Some(c) = body_display.get_mut(0..1) {
            c.make_ascii_uppercase();
        }
        let name_rect = Rect::new(bounds.x, bounds.y + 88.0, bounds.width, 24.0);
        render_utils::draw_text_centered(canvas, &body_display, name_rect, bold, 16.0, theme.colors.foreground);

        // Phase name (dimmed)
        let dim = dim_color(theme.colors.foreground, 0.8);
        let phase_rect = Rect::new(bounds.x, bounds.y + 112.0, bounds.width, 22.0);
        render_utils::draw_text_centered(canvas, &self.phase_name, phase_rect, font, 14.0, dim);

        // Illumination percentage (more dimmed)
        let dim2 = dim_color(theme.colors.foreground, 0.6);
        let illum_text = format!("{:.1}% illuminated", self.illumination_fraction * 100.0);
        let illum_rect = Rect::new(bounds.x, bounds.y + 134.0, bounds.width, 22.0);
        render_utils::draw_text_centered(canvas, &illum_text, illum_rect, font, 13.0, dim2);
    }

    fn handle_event(&mut self, _event: &InputEvent, _bounds: Rect) -> PopupEventResult {
        PopupEventResult::Ignored
    }

    fn update(&mut self) -> bool {
        false
    }
}

fn dim_color(color: [u8; 4], opacity: f32) -> [u8; 4] {
    let a = (color[3] as f32 * opacity.clamp(0.0, 1.0)) as u8;
    [color[0], color[1], color[2], a]
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone as _;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn luna_reference_new_moon() {
        // Jan 6, 2000 is the reference new-moon date in fn0rd.
        let phase = phase_angle(Body::Luna, date(2000, 1, 6));
        assert!(
            !(0.05..=0.95).contains(&phase),
            "expected near-zero phase at reference new moon, got {phase}"
        );
    }

    #[test]
    fn luna_known_full_moon() {
        // Jan 20, 2000 ≈ full moon.
        let phase = phase_angle(Body::Luna, date(2000, 1, 20));
        assert!(
            (0.47..=0.53).contains(&phase),
            "expected full moon, got {phase}"
        );
    }

    #[test]
    fn moon_emoji_covers_all_8_phases() {
        let phases = [0.0, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875];
        for p in phases {
            let e = moon_emoji(p);
            assert!(!e.is_empty(), "emoji should not be empty for phase {p}");
        }
    }

    #[test]
    fn moon_emoji_new_is_dark() {
        assert_eq!(moon_emoji(0.0), "🌑");
    }

    #[test]
    fn moon_emoji_full_is_bright() {
        assert_eq!(moon_emoji(0.5), "🌕");
    }

    #[test]
    fn update_skips_same_day() {
        let mut m = LunarModule::new(LunarConfig::default());
        let state = hyprdeck_core::HyprState::default();
        let t = chrono::Local
            .with_ymd_and_hms(2024, 1, 1, 12, 0, 0)
            .unwrap();
        let ctx = UpdateContext {
            now: t,
            hypr_state: &state,
            output_name: "",
        };
        assert!(m.update(&ctx), "first update should return true");
        assert!(!m.update(&ctx), "same day should return false");
    }

    #[test]
    fn resolve_unknown_body_falls_back_to_luna() {
        let body = LunarModule::resolve_body("unknown_planet");
        assert_eq!(body, Body::Luna);
    }
}
