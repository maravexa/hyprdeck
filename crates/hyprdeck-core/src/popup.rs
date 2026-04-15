//! Popup dropdown system for panel modules.
//!
//! Provides the [`PopupContent`] trait that modules implement to supply
//! popup window content, and [`PopupState`] that the panel uses to track
//! the single active popup.
//!
//! The Wayland layer-surface lifecycle (creating / destroying the overlay
//! surface) is handled by the binary crate's SCTK delegate; this module
//! carries only the data-layer state.

use tiny_skia::Pixmap;

use crate::action::Action;
use crate::geometry::{Rect, Size};
use crate::module::{InputEvent, ThemeContext};

// ── Popup event result ─────────────────────────────────────────────────────────

/// Return value from [`PopupContent::handle_event`].
pub enum PopupEventResult {
    /// Event was not handled by the popup content.
    Ignored,
    /// Event was consumed; a redraw may be needed.
    Handled,
    /// Event triggered an action the panel should execute.
    Action(Action),
    /// The popup should close itself (e.g. item was selected).
    Close,
}

// ── PopupContent trait ─────────────────────────────────────────────────────────

/// Content rendered inside a popup window.
///
/// Modules that support a popup implement this trait and return an instance
/// from [`crate::module::PanelModule::popup_content`].
pub trait PopupContent: Send {
    /// Desired size of the popup window in logical pixels.
    fn desired_size(&self, theme: &ThemeContext) -> Size;

    /// Render the popup content into `canvas` within `bounds`.
    ///
    /// This is called with the popup's own pixmap, not the panel pixmap.
    /// `bounds` is typically `Rect::new(0.0, 0.0, width, height)`.
    fn render(&self, canvas: &mut Pixmap, theme: &ThemeContext, bounds: Rect);

    /// Handle an input event inside the popup.
    fn handle_event(&mut self, event: &InputEvent, bounds: Rect) -> PopupEventResult;

    /// Advance popup state by one tick. Returns `true` if a redraw is needed.
    fn update(&mut self) -> bool;
}

// ── PopupState ─────────────────────────────────────────────────────────────────

/// Manages the single active popup for a panel.
///
/// Only one popup may be open at a time. Opening a new one closes the
/// previous one automatically.
pub struct PopupState {
    /// ID of the module whose popup is open. `None` = no popup.
    pub active_module: Option<String>,
    /// Popup content renderer.
    pub content: Option<Box<dyn PopupContent>>,
    /// Whether the popup content has changed and needs repainting.
    pub dirty: bool,
}

impl PopupState {
    /// Create a new empty popup state (no popup open).
    pub fn new() -> Self {
        Self {
            active_module: None,
            content: None,
            dirty: false,
        }
    }

    /// Close the active popup, if any.
    pub fn close(&mut self) {
        tracing::info!("Popup close called, active={:?}", self.active_module);
        self.active_module = None;
        self.content = None;
        self.dirty = false;
    }

    /// Open a popup for `module_id`, closing any existing popup first.
    pub fn open(&mut self, module_id: String, content: Box<dyn PopupContent>) {
        tracing::info!("Opening popup for '{}'", module_id);
        self.close();
        self.active_module = Some(module_id);
        self.content = Some(content);
        self.dirty = true;
    }

    /// Toggle: close if `module_id`'s popup is already open, otherwise open it.
    ///
    /// `content_fn` is only called when opening a new popup; it is not called
    /// when closing.
    pub fn toggle(
        &mut self,
        module_id: &str,
        content_fn: impl FnOnce() -> Box<dyn PopupContent>,
    ) {
        tracing::info!("popup.toggle called for '{}'", module_id);
        if self.active_module.as_deref() == Some(module_id) {
            tracing::info!("Closing popup for '{}'", module_id);
            self.close();
        } else {
            tracing::info!("Opening popup for '{}'", module_id);
            self.open(module_id.to_string(), content_fn());
        }
    }

    /// Returns `true` if a popup is currently open.
    pub fn is_open(&self) -> bool {
        self.active_module.is_some()
    }

    /// Tick the popup content. Returns `true` if a redraw is needed.
    pub fn update(&mut self) -> bool {
        if let Some(content) = &mut self.content {
            if content.update() {
                self.dirty = true;
                return true;
            }
        }
        false
    }

    /// Render the popup content into `canvas` if dirty. Clears the dirty flag.
    pub fn render(&mut self, canvas: &mut Pixmap, theme: &ThemeContext, bounds: Rect) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        if let Some(content) = &self.content {
            content.render(canvas, theme, bounds);
        }
    }

    /// Forward an input event to the active popup content.
    ///
    /// Returns `None` if no popup is open. If the result is [`PopupEventResult::Close`]
    /// the popup is automatically closed before returning.
    pub fn handle_event(
        &mut self,
        event: &InputEvent,
        bounds: Rect,
    ) -> Option<PopupEventResult> {
        let content = self.content.as_mut()?;
        let result = content.handle_event(event, bounds);
        match &result {
            PopupEventResult::Handled => {
                self.dirty = true;
            }
            PopupEventResult::Close => {
                self.close();
            }
            _ => {}
        }
        Some(result)
    }
}

impl Default for PopupState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NullPopup;

    impl PopupContent for NullPopup {
        fn desired_size(&self, _theme: &ThemeContext) -> Size {
            Size::new(100.0, 100.0)
        }
        fn render(&self, _canvas: &mut Pixmap, _theme: &ThemeContext, _bounds: Rect) {}
        fn handle_event(&mut self, _event: &InputEvent, _bounds: Rect) -> PopupEventResult {
            PopupEventResult::Ignored
        }
        fn update(&mut self) -> bool {
            false
        }
    }

    #[test]
    fn popup_state_starts_closed() {
        let state = PopupState::new();
        assert!(!state.is_open());
        assert!(state.active_module.is_none());
    }

    #[test]
    fn open_sets_active_module() {
        let mut state = PopupState::new();
        state.open("clock".into(), Box::new(NullPopup));
        assert!(state.is_open());
        assert_eq!(state.active_module.as_deref(), Some("clock"));
        assert!(state.dirty);
    }

    #[test]
    fn close_clears_state() {
        let mut state = PopupState::new();
        state.open("clock".into(), Box::new(NullPopup));
        state.close();
        assert!(!state.is_open());
        assert!(state.active_module.is_none());
        assert!(!state.dirty);
    }

    #[test]
    fn toggle_opens_when_closed() {
        let mut state = PopupState::new();
        state.toggle("clock", || Box::new(NullPopup));
        assert!(state.is_open());
        assert_eq!(state.active_module.as_deref(), Some("clock"));
    }

    #[test]
    fn toggle_closes_when_same_module_open() {
        let mut state = PopupState::new();
        state.toggle("clock", || Box::new(NullPopup));
        state.toggle("clock", || Box::new(NullPopup));
        assert!(!state.is_open());
    }

    #[test]
    fn toggle_switches_to_different_module() {
        let mut state = PopupState::new();
        state.toggle("clock", || Box::new(NullPopup));
        state.toggle("network", || Box::new(NullPopup));
        assert!(state.is_open());
        assert_eq!(state.active_module.as_deref(), Some("network"));
    }

    #[test]
    fn handle_event_returns_none_when_closed() {
        let mut state = PopupState::new();
        let event = InputEvent::MouseMove { x: 0.0, y: 0.0 };
        assert!(state.handle_event(&event, Rect::default()).is_none());
    }
}
