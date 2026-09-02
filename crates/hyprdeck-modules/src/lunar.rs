//! Lunar phase display module.
//!
//! Phase calculations use a built-in synodic-period approximation.
//! The phase is re-computed at most once per calendar day; intraday updates
//! are skipped.
//!
//! By default the phase is rendered as a canvas-drawn disc with a terminator
//! curve (`render_utils::draw_moon_phase`).  The `render_mode` config key
//! selects alternative representations: `emoji` (Unicode moon-phase emoji,
//! 🌑 … 🌕) or `ascii` (scalable Unicode geometric symbols).  `icons`
//! (pre-rendered theme icon sets) is reserved and currently falls back to
//! canvas.

use chrono::NaiveDate;
use serde::Deserialize;

// ── Orbital mechanics (built-in approximation) ────────────────────────────────

/// A celestial body whose phase we can approximate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Body {
    Luna,
    Phobos,
    Deimos,
    Io,
    Europa,
    Ganymede,
    Titan,
    Triton,
}

impl Body {
    pub fn parse(s: &str) -> Option<Body> {
        match s.to_lowercase().as_str() {
            "luna" | "moon" => Some(Body::Luna),
            "phobos" => Some(Body::Phobos),
            "deimos" => Some(Body::Deimos),
            "io" => Some(Body::Io),
            "europa" => Some(Body::Europa),
            "ganymede" => Some(Body::Ganymede),
            "titan" => Some(Body::Titan),
            "triton" => Some(Body::Triton),
            _ => None,
        }
    }

    fn orbital_period(self) -> f64 {
        match self {
            Body::Luna => 29.530_59,
            Body::Phobos => 0.318_91,
            Body::Deimos => 1.262_44,
            Body::Io => 1.769_14,
            Body::Europa => 3.551_82,
            Body::Ganymede => 7.154_55,
            Body::Titan => 15.945_42,
            Body::Triton => 5.876_85,
        }
    }

    fn reference_new_moon(self) -> NaiveDate {
        match self {
            Body::Luna => NaiveDate::from_ymd_opt(2000, 1, 6).unwrap(),
            _ => NaiveDate::from_ymd_opt(2000, 1, 1).unwrap(),
        }
    }

    fn retrograde(self) -> bool {
        matches!(self, Body::Triton)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PhaseName {
    NewMoon,
    WaxingCrescent,
    FirstQuarter,
    WaxingGibbous,
    FullMoon,
    WaningGibbous,
    LastQuarter,
    WaningCrescent,
}

impl PhaseName {
    fn label(self) -> &'static str {
        match self {
            PhaseName::NewMoon => "New Moon",
            PhaseName::WaxingCrescent => "Waxing Crescent",
            PhaseName::FirstQuarter => "First Quarter",
            PhaseName::WaxingGibbous => "Waxing Gibbous",
            PhaseName::FullMoon => "Full Moon",
            PhaseName::WaningGibbous => "Waning Gibbous",
            PhaseName::LastQuarter => "Last Quarter",
            PhaseName::WaningCrescent => "Waning Crescent",
        }
    }

    fn emoji(self) -> &'static str {
        match self {
            PhaseName::NewMoon => "🌑",
            PhaseName::WaxingCrescent => "🌒",
            PhaseName::FirstQuarter => "🌓",
            PhaseName::WaxingGibbous => "🌔",
            PhaseName::FullMoon => "🌕",
            PhaseName::WaningGibbous => "🌖",
            PhaseName::LastQuarter => "🌗",
            PhaseName::WaningCrescent => "🌘",
        }
    }
}

fn phase_angle(body: Body, target: NaiveDate) -> f64 {
    let days = (target - body.reference_new_moon()).num_days() as f64;
    let raw = days.rem_euclid(body.orbital_period()) / body.orbital_period();
    if body.retrograde() {
        (1.0 - raw).rem_euclid(1.0)
    } else {
        raw
    }
}

fn phase_name_for_angle(angle: f64) -> PhaseName {
    let a = angle.rem_euclid(1.0);
    if !(0.03..0.97).contains(&a) {
        PhaseName::NewMoon
    } else if a < 0.22 {
        PhaseName::WaxingCrescent
    } else if a < 0.28 {
        PhaseName::FirstQuarter
    } else if a < 0.47 {
        PhaseName::WaxingGibbous
    } else if a < 0.53 {
        PhaseName::FullMoon
    } else if a < 0.72 {
        PhaseName::WaningGibbous
    } else if a < 0.78 {
        PhaseName::LastQuarter
    } else {
        PhaseName::WaningCrescent
    }
}

use hyprdeck_core::{
    ConfigField, ConfigFieldType, DisplayMode, EventResult, InputEvent, ModuleConfigSchema,
    PanelModule, Pixmap, PopupContent, PopupEventResult, Rect, Size, ThemeContext, UpdateContext,
};

use crate::render_utils;

// ── Config ────────────────────────────────────────────────────────────────────

/// How the moon phase icon is rendered.
///
/// `Icons` is reserved for future theme-driven icon sets (themes have no
/// asset-path schema yet); selecting it logs a warning at module creation
/// and renders as `Canvas`.
#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LunarRenderMode {
    /// tiny-skia drawn disc with terminator curve (default, always works).
    #[default]
    Canvas,
    /// Pre-rendered icon set from the theme (reserved; falls back to Canvas).
    Icons,
    /// Unicode moon-phase emoji (🌑 … 🌕).  Requires a color-emoji font for
    /// color output; renders the font's monochrome fallback otherwise.
    Emoji,
    /// Unicode geometric symbols (● ◐ ○ …) drawn by the regular text font,
    /// scalable to any size.
    Ascii,
}

/// Configuration for the lunar phase display module.
#[derive(Debug, Default, Deserialize)]
pub struct LunarConfig {
    /// Show the phase name as text next to the icon.
    #[serde(default)]
    pub show_label: bool,
    /// Celestial body to track.  Accepts "luna", "phobos", "deimos", "io",
    /// "europa", "ganymede", "titan", or "triton".  Defaults to "luna".
    #[serde(default = "default_body")]
    pub body: String,
    /// Calendar locale for phase names (BCP-47 tag, e.g. "en", "zh-TW").
    #[serde(default = "default_locale")]
    pub locale: String,
    /// Rendering mode for the moon icon.
    #[serde(default)]
    pub render_mode: LunarRenderMode,
    /// Layout mode: `icon` renders a square icon only; `verbose` renders the
    /// icon in the left half and the integer illumination percentage in the right
    /// half.
    #[serde(default)]
    pub display: DisplayMode,
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
    /// Illumination percentage (0–100), cached alongside phase.
    cached_illumination_pct: u8,
    /// Date on which the cache was last filled.
    last_date: Option<NaiveDate>,
}

impl LunarModule {
    pub fn new(config: LunarConfig) -> Self {
        if config.render_mode == LunarRenderMode::Icons {
            tracing::warn!(
                "lunar: render_mode = \"icons\" is not yet implemented; falling back to canvas"
            );
        }
        LunarModule {
            config,
            cached_phase: 0.0,
            cached_name: String::new(),
            cached_illumination_pct: 0,
            last_date: None,
        }
    }

    /// Draw the moon-phase icon into `icon_rect` according to the configured
    /// render mode.
    fn draw_phase_icon(&self, canvas: &mut Pixmap, theme: &ThemeContext, icon_rect: Rect) {
        match self.config.render_mode {
            // `Icons` falls back to canvas until themes can ship icon sets;
            // the fallback is announced once in `LunarModule::new`.
            LunarRenderMode::Canvas | LunarRenderMode::Icons => {
                let lit_color = theme.colors.foreground;
                let dark_color = render_utils::dim_color(theme.colors.background, 0.3);
                render_utils::draw_moon_phase(
                    canvas,
                    icon_rect,
                    self.cached_phase,
                    lit_color,
                    dark_color,
                );
            }
            LunarRenderMode::Emoji => {
                render_utils::draw_text_centered(
                    canvas,
                    moon_emoji(self.cached_phase),
                    icon_rect,
                    &theme.fonts.family,
                    icon_rect.height * 0.9,
                    theme.colors.foreground,
                );
            }
            LunarRenderMode::Ascii => {
                render_utils::draw_text_centered(
                    canvas,
                    moon_phase_symbol(self.cached_phase),
                    icon_rect,
                    &theme.fonts.family,
                    icon_rect.height,
                    theme.colors.foreground,
                );
            }
        }
    }

    /// Resolve the configured body string to a `Body`.  Falls back to
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
        let slot = theme.icon_slot_size;
        match self.config.display {
            DisplayMode::Verbose => Size::new(slot * 2.0, slot),
            DisplayMode::Icon => {
                if self.config.show_label && !self.cached_name.is_empty() {
                    let text_width =
                        render_utils::estimate_text_width(&self.cached_name, theme.fonts.size);
                    Size::new(slot + text_width + theme.icon_padding.max(2.0), slot)
                } else {
                    Size::new(slot, slot)
                }
            }
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

        let illum_angle = (new_phase - 0.5) * 2.0 * std::f64::consts::PI;
        let illum_frac = (illum_angle.cos() + 1.0) / 2.0;
        let new_illum_pct = (illum_frac * 100.0).round() as u8;

        let changed = (new_phase - self.cached_phase).abs() > 1e-9 || new_name != self.cached_name;
        self.cached_phase = new_phase;
        self.cached_name = new_name;
        self.cached_illumination_pct = new_illum_pct;
        changed
    }

    fn render(&self, canvas: &mut Pixmap, theme: &ThemeContext, bounds: Rect) {
        match self.config.display {
            DisplayMode::Icon => {
                let icon_slot = if self.config.show_label && !self.cached_name.is_empty() {
                    Rect::new(
                        bounds.x,
                        bounds.y,
                        theme.icon_slot_size.min(bounds.width),
                        bounds.height,
                    )
                } else {
                    bounds
                };
                let icon_bounds = render_utils::icon_content_rect(icon_slot, theme.icon_padding);
                self.draw_phase_icon(canvas, theme, icon_bounds);
                if self.config.show_label && !self.cached_name.is_empty() {
                    let label_x = icon_slot.x + icon_slot.width + theme.icon_padding.max(2.0);
                    let label_rect = Rect::new(
                        label_x,
                        bounds.y,
                        bounds.width - (label_x - bounds.x),
                        bounds.height,
                    );
                    let bold = theme
                        .fonts
                        .bold_family
                        .as_deref()
                        .unwrap_or(&theme.fonts.family);
                    let text_size =
                        render_utils::effective_font_size(bounds.height, theme.fonts.size);
                    render_utils::draw_text(
                        canvas,
                        &self.cached_name,
                        label_rect,
                        bold,
                        text_size,
                        theme.colors.foreground,
                    );
                }
            }
            DisplayMode::Verbose => {
                let readout = format!("{}%", self.cached_illumination_pct);
                render_utils::draw_verbose(
                    canvas,
                    bounds,
                    theme,
                    &readout,
                    theme.colors.foreground,
                    |canvas, icon_rect| {
                        self.draw_phase_icon(canvas, theme, icon_rect);
                    },
                );
            }
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
                    key: "display".to_owned(),
                    label: "Display mode".to_owned(),
                    description: "Icon-only square or double-wide icon + illumination percentage."
                        .to_owned(),
                    field_type: ConfigFieldType::LabeledChoice {
                        options: vec!["icon".to_owned(), "verbose".to_owned()],
                        labels: vec!["Icon only".to_owned(), "Icon + value".to_owned()],
                        default: "icon".to_owned(),
                    },
                },
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
                    description: "How the moon icon is drawn: canvas (vector disc, default), emoji (color-emoji font required for color output), ascii (Unicode geometric symbols), icons (reserved; falls back to canvas).".to_owned(),
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
        let dark_color = render_utils::dim_color(theme.colors.foreground, 0.15);
        render_utils::draw_moon_phase(
            canvas,
            moon_bounds,
            self.phase_fraction,
            lit_color,
            dark_color,
        );

        let bold = theme
            .fonts
            .bold_family
            .as_deref()
            .unwrap_or(&theme.fonts.family);
        let font = &theme.fonts.family;

        // Body name (capitalised)
        let mut body_display = self.body_name.clone();
        if let Some(c) = body_display.get_mut(0..1) {
            c.make_ascii_uppercase();
        }
        let name_rect = Rect::new(bounds.x, bounds.y + 88.0, bounds.width, 24.0);
        render_utils::draw_text_centered(
            canvas,
            &body_display,
            name_rect,
            bold,
            16.0,
            theme.colors.foreground,
        );

        // Phase name (dimmed)
        let dim = render_utils::dim_color(theme.colors.foreground, 0.8);
        let phase_rect = Rect::new(bounds.x, bounds.y + 112.0, bounds.width, 22.0);
        render_utils::draw_text_centered(canvas, &self.phase_name, phase_rect, font, 14.0, dim);

        // Illumination percentage (more dimmed)
        let dim2 = render_utils::dim_color(theme.colors.foreground, 0.6);
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone as _;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn luna_reference_new_moon() {
        // Jan 6, 2000 is the reference new-moon date for Luna.
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

    #[test]
    fn moon_phase_symbol_new_is_dark_full_is_bright() {
        assert_eq!(moon_phase_symbol(0.0), "●");
        assert_eq!(moon_phase_symbol(0.5), "○");
    }

    #[test]
    fn moon_phase_symbol_covers_all_8_phases() {
        let phases = [0.0, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875];
        for p in phases {
            assert!(!moon_phase_symbol(p).is_empty(), "empty symbol for {p}");
        }
    }

    #[test]
    fn render_mode_deserializes_all_variants() {
        for (toml, expected) in [
            ("render_mode = \"canvas\"", LunarRenderMode::Canvas),
            ("render_mode = \"icons\"", LunarRenderMode::Icons),
            ("render_mode = \"emoji\"", LunarRenderMode::Emoji),
            ("render_mode = \"ascii\"", LunarRenderMode::Ascii),
        ] {
            let config: LunarConfig = toml::from_str(toml).unwrap();
            assert_eq!(config.render_mode, expected, "for input {toml:?}");
        }
    }

    fn test_theme() -> ThemeContext {
        use hyprdeck_core::{ColorPalette, FontConfig, Padding, ResolvedModuleStyles};
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
                size: 13.0,
                bold_family: None,
                mono_family: None,
            },
            padding: Padding::default(),
            border_radius: 0.0,
            opacity: 1.0,
            icon_slot_size: 24.0,
            icon_padding: 2.0,
            verbose_text_padding: 4.0,
            module_styles: ResolvedModuleStyles::default(),
        }
    }

    /// Every render mode must draw without panicking in both display modes.
    /// Pixel assertions are limited to Canvas (and the Icons fallback): the
    /// Emoji/Ascii modes depend on fonts installed on the test machine.
    #[test]
    fn all_render_modes_render_without_panic() {
        let theme = test_theme();
        let state = hyprdeck_core::HyprState::default();
        let t = chrono::Local
            .with_ymd_and_hms(2024, 1, 20, 12, 0, 0)
            .unwrap();
        let ctx = UpdateContext {
            now: t,
            hypr_state: &state,
            output_name: "",
        };

        for mode_toml in ["canvas", "icons", "emoji", "ascii"] {
            for display_toml in ["icon", "verbose"] {
                let config: LunarConfig = toml::from_str(&format!(
                    "render_mode = \"{mode_toml}\"\ndisplay = \"{display_toml}\""
                ))
                .unwrap();
                let mut m = LunarModule::new(config);
                m.update(&ctx);
                let mut pm = Pixmap::new(64, 32).unwrap();
                m.render(&mut pm, &theme, Rect::new(0.0, 0.0, 64.0, 32.0));

                if mode_toml == "canvas" || mode_toml == "icons" {
                    let painted = pm.data().chunks_exact(4).any(|px| px[3] > 0);
                    assert!(painted, "canvas-backed mode {mode_toml} painted nothing");
                }
            }
        }
    }
}
