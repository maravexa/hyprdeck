use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use tiny_skia::Pixmap;

pub use hyprdeck_config::{ConfigField, ConfigFieldType, ModuleConfigSchema};

use crate::action::Action;
use crate::geometry::{Rect, Size};
use crate::ipc::event::HyprState;
use crate::panel::{ColorPalette, FontConfig, Padding, ResolvedModuleStyles};

// ── Display mode ───────────────────────────────────────────────────────────────

/// Controls whether a module renders as a single icon square or as a
/// double-wide icon-plus-readout widget.
///
/// `Icon` is the default and preserves the module's pre-existing square
/// geometry. `Verbose` doubles the width: the icon occupies the left half and
/// a numeric readout occupies the right half.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DisplayMode {
    #[default]
    Icon,
    Verbose,
}

// ── Update context ─────────────────────────────────────────────────────────────

/// Context snapshot provided to modules on each update tick.
pub struct UpdateContext<'a> {
    /// Wall-clock time at the start of this tick.
    pub now: DateTime<Local>,
    /// Reference to the latest aggregated Hyprland state.
    pub hypr_state: &'a HyprState,
    /// Wayland output name this panel belongs to (e.g. `"DP-2"`).
    ///
    /// Used by per-monitor modules (workspaces, window list) to read the
    /// active workspace for *this* output rather than the globally focused one.
    pub output_name: &'a str,
}

// ── Theme context ──────────────────────────────────────────────────────────────

/// Resolved theme values passed to a module's `render` and `desired_size` methods.
pub struct ThemeContext {
    pub colors: ColorPalette,
    pub fonts: FontConfig,
    pub padding: Padding,
    pub border_radius: f32,
    pub opacity: f32,
    /// Side length of an icon-only module slot, resolved from this panel's
    /// content thickness. All built-in icon-only status modules request this
    /// size so adjacent indicators have a consistent footprint.
    pub icon_slot_size: f32,
    /// Inset between an icon-only module slot and its drawn icon content.
    /// Resolved from the theme's optional `style.icon_padding` key.
    pub icon_padding: f32,
    /// Gap between the icon half and text half in verbose display mode, in
    /// logical pixels.  Resolved from the theme's `verbose_text_padding` key,
    /// defaulting to `bar_height / 8` when the key is absent.
    pub verbose_text_padding: f32,
    /// Per-module color overrides resolved from the panel's `module_styles` section.
    pub module_styles: ResolvedModuleStyles,
}

// ── Input events ───────────────────────────────────────────────────────────────

/// An input event dispatched to the module under the cursor.
#[derive(Debug, Clone)]
pub enum InputEvent {
    MousePress {
        x: f32,
        y: f32,
        button: MouseButton,
    },
    MouseRelease {
        x: f32,
        y: f32,
        button: MouseButton,
    },
    MouseMove {
        x: f32,
        y: f32,
    },
    Scroll {
        dx: f32,
        dy: f32,
    },
    /// A key press while the panel (or its popup) has keyboard focus.
    ///
    /// `key` is the xkb keysym value; `modifiers` is a bitmask of [`keymod`]
    /// flags.  Delivered to the module owning the open popup if any, else the
    /// hovered module.  Key release and key repeat are not delivered.
    KeyPress {
        key: u32,
        modifiers: u32,
    },
}

/// Modifier bitflags for [`InputEvent::KeyPress`]'s `modifiers` field.
pub mod keymod {
    pub const SHIFT: u32 = 1 << 0;
    pub const CTRL: u32 = 1 << 1;
    pub const ALT: u32 = 1 << 2;
    pub const LOGO: u32 = 1 << 3;
}

/// Pointer button identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

// ── Event result ───────────────────────────────────────────────────────────────

/// Return value from [`PanelModule::handle_event`].
pub enum EventResult {
    /// The event was not handled by this module.
    Ignored,
    /// The event was consumed; no further propagation needed.
    Handled,
    /// The event triggered an action that the panel should execute.
    Action(Action),
}

// Schema types live in `hyprdeck-config` so settings editors can consume the
// same versioned contract without linking the Wayland/rendering runtime.

// ── Core module trait ──────────────────────────────────────────────────────────

/// Core trait implemented by every built-in and third-party panel module.
///
/// All methods are called from the panel's main thread.  Modules that need
/// background work should spawn `tokio` tasks internally and communicate via
/// channels, storing results for the next `update()` call.
pub trait PanelModule: Send {
    /// Stable, lowercase identifier for this module type (e.g. `"clock"`).
    fn id(&self) -> &str;

    /// Preferred bounding-box size given the current theme.
    ///
    /// The panel may grant less space than requested if the bar is too narrow.
    fn desired_size(&self, theme: &ThemeContext) -> Size;

    /// Advance internal state by one tick.
    ///
    /// Returns `true` if the module's visual output has changed and a redraw
    /// should be scheduled.
    fn update(&mut self, ctx: &UpdateContext<'_>) -> bool;

    /// Render the module into `canvas` within the given `bounds`.
    ///
    /// The pixmap is shared across all modules in the panel; modules must
    /// restrict all drawing to the provided `bounds`.
    fn render(&self, canvas: &mut Pixmap, theme: &ThemeContext, bounds: Rect);

    /// Handle a pointer or keyboard event that landed within this module's bounds.
    fn handle_event(&mut self, event: &InputEvent, bounds: Rect) -> EventResult;

    /// Describe all configurable options for HyprCube GUI integration.
    fn config_schema(&self) -> ModuleConfigSchema;

    /// Whether this module shows a popup dropdown when left-clicked.
    ///
    /// When `true` the panel intercepts left-clicks and calls
    /// [`popup_content`][PanelModule::popup_content] instead of forwarding
    /// the event to [`handle_event`][PanelModule::handle_event].
    fn has_popup(&self) -> bool {
        false
    }

    /// Return the popup content for this module.
    ///
    /// Called by the panel on left-click when [`has_popup`][PanelModule::has_popup]
    /// returns `true`.  Should never return `None` when `has_popup()` is `true`.
    fn popup_content(&self) -> Option<Box<dyn crate::popup::PopupContent>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_mode_default_is_icon() {
        assert_eq!(DisplayMode::default(), DisplayMode::Icon);
    }

    #[test]
    fn display_mode_serde_roundtrip() {
        let icon: DisplayMode = serde_json::from_str("\"icon\"").unwrap();
        assert_eq!(icon, DisplayMode::Icon);
        let verbose: DisplayMode = serde_json::from_str("\"verbose\"").unwrap();
        assert_eq!(verbose, DisplayMode::Verbose);
    }

    #[test]
    fn display_mode_serialize() {
        assert_eq!(
            serde_json::to_string(&DisplayMode::Icon).unwrap(),
            "\"icon\""
        );
        assert_eq!(
            serde_json::to_string(&DisplayMode::Verbose).unwrap(),
            "\"verbose\""
        );
    }

    #[test]
    fn display_mode_unknown_value_fails() {
        let result: Result<DisplayMode, _> = serde_json::from_str("\"compact\"");
        assert!(
            result.is_err(),
            "unknown variant 'compact' should fail to deserialize"
        );
    }
}
