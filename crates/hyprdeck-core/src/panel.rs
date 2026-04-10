use crate::autohide::AutoHideState;
use crate::geometry::Edge;
use crate::layout::LayoutEngine;
use crate::module::PanelModule;

/// RGBA colour stored as four bytes (0–255).
pub type Color = [u8; 4];

/// A single panel instance — one Wayland `zwlr_layer_surface_v1` surface.
///
/// Each connected output may host one or more panels as declared by the active theme.
pub struct Panel {
    /// Screen edge this panel is anchored to.
    pub edge: Edge,
    /// Ordered list of modules rendered inside this panel.
    pub modules: Vec<Box<dyn PanelModule>>,
    /// Layout engine responsible for assigning per-module bounding boxes.
    pub layout: LayoutEngine,
    /// Auto-hide state machine for this panel.
    pub auto_hide: AutoHideState,
    /// Fully-resolved visual style (theme defaults + user overrides).
    pub style: ResolvedStyle,
    // TODO: smithay LayerSurface handle goes here once Wayland integration lands.
}

/// Fully resolved visual style after merging theme defaults with user overrides.
///
/// All colour and typography values are ready to use directly during rendering —
/// no further lookup or parsing required.
#[derive(Debug, Clone)]
pub struct ResolvedStyle {
    pub colors: ColorPalette,
    pub fonts: FontConfig,
    /// Panel thickness in physical pixels (height for top/bottom, width for left/right).
    pub bar_height: u32,
    pub padding: Padding,
    pub border_radius: f32,
    pub background_opacity: f32,
    /// Separator line styling between adjacent modules.
    pub separator: ResolvedSeparator,
}

/// RGBA colour palette for a panel.
///
/// Each colour is stored as `[r, g, b, a]` bytes (0–255).
#[derive(Debug, Clone)]
pub struct ColorPalette {
    pub background: [u8; 4],
    pub foreground: [u8; 4],
    pub accent: [u8; 4],
    pub urgent: [u8; 4],
    pub separator: [u8; 4],
}

/// Font configuration after theme/override resolution.
#[derive(Debug, Clone)]
pub struct FontConfig {
    pub family: String,
    pub size: f32,
    /// Optional separate family for bold text; falls back to `family` if `None`.
    pub bold_family: Option<String>,
}

/// Inset padding applied inside the panel background, in logical pixels.
#[derive(Debug, Clone, Copy, Default)]
pub struct Padding {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

/// Resolved separator styling between modules within a group.
#[derive(Debug, Clone)]
pub struct ResolvedSeparator {
    /// Whether separators are drawn at all.
    pub visible: bool,
    /// Line thickness in logical pixels.
    pub width: f32,
    /// Space on each side of the separator line.
    pub margin: f32,
    /// Separator line colour.
    pub color: Color,
}

impl Default for ResolvedSeparator {
    fn default() -> Self {
        Self {
            visible: true,
            color: [128, 128, 128, 128],
            width: 1.0,
            margin: 4.0,
        }
    }
}

impl ColorPalette {
    /// Parse an `#rrggbb` or `#rrggbbaa` hex string into an RGBA byte array.
    pub fn parse_hex(s: &str) -> Option<[u8; 4]> {
        let s = s.strip_prefix('#')?;
        match s.len() {
            6 => {
                let r = u8::from_str_radix(&s[0..2], 16).ok()?;
                let g = u8::from_str_radix(&s[2..4], 16).ok()?;
                let b = u8::from_str_radix(&s[4..6], 16).ok()?;
                Some([r, g, b, 255])
            }
            8 => {
                let r = u8::from_str_radix(&s[0..2], 16).ok()?;
                let g = u8::from_str_radix(&s[2..4], 16).ok()?;
                let b = u8::from_str_radix(&s[4..6], 16).ok()?;
                let a = u8::from_str_radix(&s[6..8], 16).ok()?;
                Some([r, g, b, a])
            }
            _ => None,
        }
    }
}
