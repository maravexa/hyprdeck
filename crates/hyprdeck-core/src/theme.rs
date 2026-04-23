use serde::Deserialize;

use crate::autohide::AutoHideMode;
use crate::geometry::Edge;

/// A fully parsed theme definition, loaded from `theme.toml`.
#[derive(Debug, Deserialize)]
pub struct ThemeDefinition {
    /// Human-readable theme name (shown in HyprCube theme picker).
    pub name: String,
    /// One-line theme description.
    pub description: String,
    /// One or more panel configurations this theme creates.
    pub panels: Vec<PanelDefinition>,
    /// Optional base style block; merged with per-panel and user overrides.
    #[serde(default)]
    pub style: Option<StyleDefinition>,
}

/// Configuration for a single panel surface within a theme.
#[derive(Debug, Deserialize)]
pub struct PanelDefinition {
    /// Screen edge this panel is anchored to.
    pub edge: Edge,
    /// Panel height in logical pixels (meaningful for top/bottom edges).
    pub height: Option<u32>,
    /// Panel width in logical pixels (meaningful for left/right edges).
    pub width: Option<u32>,
    /// Auto-hide behaviour.
    pub auto_hide: AutoHideMode,
    /// Layout engine to use for arranging modules.
    #[serde(default)]
    pub layout: LayoutType,
    /// IDs of modules placed in the start (left/top) slot.
    #[serde(default)]
    pub modules_start: Vec<String>,
    /// IDs of modules placed in the center slot.
    #[serde(default)]
    pub modules_center: Vec<String>,
    /// IDs of modules placed in the end (right/bottom) slot.
    #[serde(default)]
    pub modules_end: Vec<String>,
    /// Dock-specific parameters; required when `layout = "dock"`.
    #[serde(default)]
    pub dock: Option<DockConfig>,
    /// Per-module color overrides for this panel.
    #[serde(default)]
    pub module_styles: ModuleStyleMap,
}

// ── Per-module style definitions ───────────────────────────────────────────────

/// Raw (unparsed) per-module color overrides within a panel definition.
///
/// Each field maps to a module ID and holds optional hex color strings.
/// Missing fields fall back to the panel's base color palette during resolution.
#[derive(Debug, Default, Deserialize, Clone)]
pub struct ModuleStyleMap {
    /// Color overrides for the `window_list` module.
    #[serde(default)]
    pub window_list: Option<WindowListStyleDef>,
    /// Color overrides for the `workspaces` module.
    #[serde(default)]
    pub workspaces: Option<WorkspacesStyleDef>,
    /// Color overrides for `menu_button` and similar single-button modules.
    #[serde(default)]
    pub menu_button: Option<ButtonStyleDef>,
}

/// Raw window-list color strings parsed from `[panels.module_styles.window_list]`.
#[derive(Debug, Deserialize, Clone)]
pub struct WindowListStyleDef {
    /// Background of the focused/active window button (`#rrggbb` or `#rrggbbaa`).
    pub active_background: Option<String>,
    /// Text color of the focused/active window button.
    pub active_foreground: Option<String>,
    /// Background of an unfocused window button.
    pub inactive_background: Option<String>,
    /// Text color of an unfocused window button.
    pub inactive_foreground: Option<String>,
}

/// Raw workspaces-switcher color strings parsed from `[panels.module_styles.workspaces]`.
#[derive(Debug, Deserialize, Clone)]
pub struct WorkspacesStyleDef {
    /// Background of the active workspace indicator.
    pub active_background: Option<String>,
    /// Label color of the active workspace indicator.
    pub active_foreground: Option<String>,
    /// Background of inactive workspace indicators.
    pub inactive_background: Option<String>,
    /// Label color of inactive workspace indicators.
    pub inactive_foreground: Option<String>,
}

/// Raw button color strings parsed from `[panels.module_styles.menu_button]`.
#[derive(Debug, Deserialize, Clone)]
pub struct ButtonStyleDef {
    /// Button background color.
    pub background: Option<String>,
    /// Button foreground/text color.
    pub foreground: Option<String>,
}

/// Selects the layout algorithm used by a panel.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LayoutType {
    #[default]
    Horizontal,
    Vertical,
    Dock,
}

/// Dock layout tuning parameters, parsed from `[panels.dock]`.
#[derive(Debug, Deserialize)]
pub struct DockConfig {
    /// Baseline icon size in logical pixels.
    pub icon_base_size: f32,
    /// Maximum scale multiplier at peak magnification.
    pub icon_max_scale: f32,
    /// Cursor distance (logical px) over which magnification is applied.
    pub magnification_radius: f32,
    /// Animation lerp speed.
    pub animation_speed: f32,
    /// Padding inside the dock background pill.
    pub padding: f32,
    /// Corner radius of the dock background pill.
    pub background_radius: f32,
    /// Background pill opacity (0.0 – 1.0).
    pub background_opacity: f32,
}

/// Visual style definition parsed from a `[style]` block in `theme.toml`.
///
/// All fields are optional so themes can partially override a base style.
#[derive(Debug, Default, Deserialize)]
pub struct StyleDefinition {
    /// Panel background colour as `#rrggbb` or `#rrggbbaa`.
    pub background_color: Option<String>,
    /// Primary text colour.
    pub foreground_color: Option<String>,
    /// Accent / highlight colour (active workspace, clock, etc.).
    pub accent_color: Option<String>,
    /// Urgent indicator colour.
    pub urgent_color: Option<String>,
    /// Separator line colour.
    pub separator_color: Option<String>,
    /// Font family name.
    pub font_family: Option<String>,
    /// Monospace font family name (used by calendar popup and other fixed-width elements).
    pub mono_font_family: Option<String>,
    /// Base font size in points.
    pub font_size: Option<f32>,
    /// Global corner radius applied to the panel background.
    pub border_radius: Option<f32>,
    /// Panel background opacity (0.0 = fully transparent, 1.0 = fully opaque).
    pub opacity: Option<f32>,
    /// Gap inserted between the icon half and text half in verbose display mode,
    /// in logical pixels. Defaults to `bar_height / 8` when absent.
    pub verbose_text_padding: Option<f32>,
}

impl ThemeDefinition {
    /// Parse a theme from a TOML string.
    pub fn from_toml(src: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(src)
    }
}
