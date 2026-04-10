use chrono::{DateTime, Local};
use tiny_skia::Pixmap;

use crate::action::Action;
use crate::geometry::{Rect, Size};
use crate::ipc::event::HyprState;
use crate::panel::{ColorPalette, FontConfig, Padding};

// ── Update context ─────────────────────────────────────────────────────────────

/// Context snapshot provided to modules on each update tick.
pub struct UpdateContext<'a> {
    /// Wall-clock time at the start of this tick.
    pub now: DateTime<Local>,
    /// Reference to the latest aggregated Hyprland state.
    pub hypr_state: &'a HyprState,
}

// ── Theme context ──────────────────────────────────────────────────────────────

/// Resolved theme values passed to a module's `render` and `desired_size` methods.
pub struct ThemeContext {
    pub colors: ColorPalette,
    pub fonts: FontConfig,
    pub padding: Padding,
    pub border_radius: f32,
    pub opacity: f32,
}

// ── Input events ───────────────────────────────────────────────────────────────

/// An input event dispatched to the module under the cursor.
#[derive(Debug, Clone)]
pub enum InputEvent {
    MousePress { x: f32, y: f32, button: MouseButton },
    MouseRelease { x: f32, y: f32, button: MouseButton },
    MouseMove { x: f32, y: f32 },
    Scroll { dx: f32, dy: f32 },
    KeyPress { key: u32, modifiers: u32 },
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

// ── Config schema (HyprCube integration) ──────────────────────────────────────

/// Self-description of a module's configurable options.
///
/// Returned by [`PanelModule::config_schema`] so HyprCube can auto-generate
/// settings UI without knowing anything about the module at compile time.
pub struct ModuleConfigSchema {
    /// Unique module type identifier (matches the module's `id()` return value).
    pub module_id: String,
    pub fields: Vec<ConfigField>,
}

/// A single configurable option declared by a module.
pub struct ConfigField {
    /// TOML key used to set this field in `config.toml`.
    pub key: String,
    /// Short human-readable label for the settings UI.
    pub label: String,
    /// Longer explanatory text shown in the settings UI.
    pub description: String,
    pub field_type: ConfigFieldType,
}

/// Declares the widget type and constraints for a [`ConfigField`].
#[derive(Debug)]
pub enum ConfigFieldType {
    Text { default: String },
    Integer { default: i64, min: Option<i64>, max: Option<i64> },
    Float { default: f64, min: Option<f64>, max: Option<f64> },
    Boolean { default: bool },
    Choice { options: Vec<String>, default: String },
    Color { default: String },
}

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
}
