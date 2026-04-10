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

use fn0rd_lib::moon::calc::{phase_angle, phase_name_for_angle, Body};

use hyprdeck_core::{
    ConfigField, ConfigFieldType, EventResult, InputEvent, ModuleConfigSchema, PanelModule, Pixmap,
    Rect, Size, ThemeContext, UpdateContext,
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
        let icon_w = theme.fonts.size + theme.padding.left + theme.padding.right;
        let label_w = if self.config.show_label && !self.cached_name.is_empty() {
            render_utils::estimate_text_width(&self.cached_name, theme.fonts.size)
                + 4.0 // gap
        } else {
            0.0
        };
        let h = theme.fonts.size + theme.padding.top + theme.padding.bottom;
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

        let changed = (new_phase - self.cached_phase).abs() > 1e-9
            || new_name != self.cached_name;
        self.cached_phase = new_phase;
        self.cached_name = new_name;
        changed
    }

    fn render(&self, canvas: &mut Pixmap, theme: &ThemeContext, bounds: Rect) {
        let emoji = moon_emoji(self.cached_phase);
        let icon_size = theme.fonts.size;
        let icon_rect = Rect::new(bounds.x, bounds.y, icon_size + theme.padding.left + theme.padding.right, bounds.height);

        render_utils::draw_text_centered(
            canvas,
            emoji,
            icon_rect,
            &theme.fonts.family,
            icon_size,
            theme.colors.foreground,
        );

        if self.config.show_label && !self.cached_name.is_empty() {
            let label_x = icon_rect.x + icon_rect.width + 4.0;
            let label_rect = Rect::new(label_x, bounds.y, bounds.width - label_x + bounds.x, bounds.height);
            render_utils::draw_text(
                canvas,
                &self.cached_name,
                label_rect,
                &theme.fonts.family,
                theme.fonts.size,
                theme.colors.foreground,
            );
        }
        // TODO: future enhancement — draw the moon phase using Canvas primitives
        // (filled circle + shadow overlay) rather than a Unicode glyph.
    }

    fn handle_event(&mut self, _event: &InputEvent, _bounds: Rect) -> EventResult {
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
        assert!((0.47..=0.53).contains(&phase), "expected full moon, got {phase}");
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
        let t = chrono::Local.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let ctx = UpdateContext { now: t, hypr_state: &state };
        assert!(m.update(&ctx), "first update should return true");
        assert!(!m.update(&ctx), "same day should return false");
    }

    #[test]
    fn resolve_unknown_body_falls_back_to_luna() {
        let body = LunarModule::resolve_body("unknown_planet");
        assert_eq!(body, Body::Luna);
    }
}
