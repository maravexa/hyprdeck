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

impl PanelModule for LunarModule {
    fn id(&self) -> &str {
        "lunar"
    }

    fn desired_size(&self, theme: &ThemeContext) -> Size {
        // One emoji glyph (square) plus optional label.
        let h = theme.fonts.size + theme.padding.top + theme.padding.bottom;
        let font_size = render_utils::effective_font_size(h, theme.fonts.size);
        let icon_w = font_size + theme.padding.left + theme.padding.right;
        let label_w = if self.config.show_label && !self.cached_name.is_empty() {
            render_utils::estimate_text_width(&self.cached_name, font_size) + 4.0 // gap
        } else {
            0.0
        };
        Size::new(icon_w + label_w, h)
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
        let emoji = moon_emoji(self.cached_phase);
        let font_size = render_utils::effective_font_size(bounds.height, theme.fonts.size);
        let icon_rect = Rect::new(
            bounds.x,
            bounds.y,
            font_size + theme.padding.left + theme.padding.right,
            bounds.height,
        );

        render_utils::draw_text_centered(
            canvas,
            emoji,
            icon_rect,
            &theme.fonts.family,
            font_size,
            theme.colors.foreground,
        );

        if self.config.show_label && !self.cached_name.is_empty() {
            let label_x = icon_rect.x + icon_rect.width + 4.0;
            let label_rect = Rect::new(
                label_x,
                bounds.y,
                bounds.width - label_x + bounds.x,
                bounds.height,
            );
            render_utils::draw_text(
                canvas,
                &self.cached_name,
                label_rect,
                &theme.fonts.family,
                font_size,
                theme.colors.foreground,
            );
        }
        // TODO: future enhancement — draw the moon phase using Canvas primitives
        // (filled circle + shadow overlay) rather than a Unicode glyph.
    }

    fn handle_event(&mut self, _event: &InputEvent, _bounds: Rect) -> EventResult {
        EventResult::Ignored
    }

    fn has_popup(&self) -> bool {
        true
    }

    fn popup_content(&self) -> Option<Box<dyn PopupContent>> {
        let illumination = {
            // cos((phase - 0.5) * 2π) maps 0=new(0%) 0.5=full(100%) 1=new(0%)
            let angle = (self.cached_phase - 0.5) * 2.0 * std::f64::consts::PI;
            ((angle.cos() + 1.0) / 2.0 * 100.0).round()
        };
        Some(Box::new(LunarPopup {
            phase_emoji: moon_emoji(self.cached_phase).to_owned(),
            body_name: self.config.body.clone(),
            illumination_percent: illumination,
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
            ],
        }
    }
}

// ── Lunar popup ───────────────────────────────────────────────────────────────

/// Popup content for the lunar module — shows large emoji, body name, and illumination.
pub struct LunarPopup {
    phase_emoji: String,
    body_name: String,
    illumination_percent: f64,
    phase_name: String,
}

impl PopupContent for LunarPopup {
    fn desired_size(&self, _theme: &ThemeContext) -> Size {
        Size::new(220.0, 148.0)
    }

    fn render(&self, canvas: &mut Pixmap, theme: &ThemeContext, bounds: Rect) {
        let font = &theme.fonts.family;

        // Large moon emoji centred at top
        let emoji_rect = Rect::new(bounds.x, bounds.y, bounds.width, 64.0);
        render_utils::draw_text_centered(canvas, &self.phase_emoji, emoji_rect, font, 44.0, theme.colors.foreground);

        // Body name (capitalised)
        let mut body_display = self.body_name.clone();
        if let Some(c) = body_display.get_mut(0..1) {
            c.make_ascii_uppercase();
        }
        let name_rect = Rect::new(bounds.x, bounds.y + 68.0, bounds.width, 26.0);
        render_utils::draw_text_centered(canvas, &body_display, name_rect, font, 16.0, theme.colors.foreground);

        // Phase name (dimmed)
        let dim = dim_color(theme.colors.foreground, 0.65);
        let phase_rect = Rect::new(bounds.x, bounds.y + 96.0, bounds.width, 22.0);
        render_utils::draw_text_centered(canvas, &self.phase_name, phase_rect, font, 13.0, dim);

        // Illumination percentage (dimmed)
        let illum = format!("{:.0}% illuminated", self.illumination_percent);
        let illum_rect = Rect::new(bounds.x, bounds.y + 118.0, bounds.width, 22.0);
        render_utils::draw_text_centered(canvas, &illum, illum_rect, font, 13.0, dim);
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
