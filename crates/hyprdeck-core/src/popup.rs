//! Popup dropdown system for panel modules.
//!
//! Provides the [`PopupContent`] trait that modules implement to supply
//! popup window content, and [`PopupState`] that the panel uses to track
//! the single active popup.
//!
//! The Wayland layer-surface lifecycle (creating / destroying the overlay
//! surface) is managed here: [`PopupState`] owns the [`LayerSurface`] and
//! SHM pool. Surface *creation* is delegated to the binary crate (which
//! has the compositor/layer_shell handles); once created the surface is
//! handed to [`PopupState::attach_surface`].

use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shell::wlr_layer::LayerSurface;
use smithay_client_toolkit::shm::slot::SlotPool;
use tiny_skia::Pixmap;
use wayland_client::Proxy;
use wayland_client::backend::ObjectId;
use wayland_client::protocol::wl_shm;

use crate::action::Action;
use crate::geometry::{Rect, Size};
use crate::module::{InputEvent, ThemeContext};
use crate::render::Canvas;

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
///
/// The Wayland surface for the popup lives here. Surface *creation* is done
/// by the binary crate (which owns `CompositorState` / `LayerShell`); once
/// the surface exists it is handed in via [`PopupState::attach_surface`].
/// Dropping this struct (or calling [`PopupState::close`]) automatically
/// destroys the Wayland surface.
pub struct PopupState {
    /// ID of the module whose popup is open. `None` = no popup.
    pub active_module: Option<String>,
    /// Popup content renderer.
    pub content: Option<Box<dyn PopupContent>>,
    /// Whether the popup content has changed and needs repainting.
    pub dirty: bool,
    /// The `zwlr_layer_surface_v1` for this popup on the `Overlay` layer.
    /// `None` until [`attach_surface`][Self::attach_surface] is called.
    pub layer_surface: Option<LayerSurface>,
    /// SHM slot pool used to allocate pixel buffers for the popup surface.
    pub pool: Option<SlotPool>,
    /// Rendering canvas for the popup's pixel data.
    pub canvas: Option<Canvas>,
    /// Popup surface width in pixels (set by `attach_surface`).
    pub width: u32,
    /// Popup surface height in pixels (set by `attach_surface`).
    pub height: u32,
}

impl PopupState {
    /// Create a new empty popup state (no popup open).
    pub fn new() -> Self {
        Self {
            active_module: None,
            content: None,
            dirty: false,
            layer_surface: None,
            pool: None,
            canvas: None,
            width: 0,
            height: 0,
        }
    }

    /// Close the active popup, destroying its Wayland surface if present.
    pub fn close(&mut self) {
        tracing::info!("Popup close called, active={:?}", self.active_module);
        if let Some(ls) = self.layer_surface.take() {
            tracing::info!("Destroying popup Wayland surface");
            drop(ls);
        }
        self.pool = None;
        self.canvas = None;
        self.active_module = None;
        self.content = None;
        self.dirty = false;
        self.width = 0;
        self.height = 0;
    }

    /// Open a popup for `module_id`, closing any existing popup first.
    ///
    /// This stores the content but does **not** create a Wayland surface.
    /// Surface creation is done by the binary crate via [`attach_surface`][Self::attach_surface]
    /// once the content's desired size is known and compositor handles are available.
    pub fn open(&mut self, module_id: String, content: Box<dyn PopupContent>) {
        tracing::info!("Opening popup for '{}'", module_id);
        // close() destroys any existing Wayland surface before we replace content.
        self.close();
        self.active_module = Some(module_id);
        self.content = Some(content);
        self.dirty = true;
    }

    // ── Wayland surface management ────────────────────────────────────────────

    /// Attach a newly-created Wayland layer surface to this popup.
    ///
    /// Called by the binary crate after it has created the surface with the
    /// compositor and layer shell. The popup takes ownership and will destroy
    /// the surface when [`close`][Self::close] is called.
    pub fn attach_surface(
        &mut self,
        layer_surface: LayerSurface,
        pool: SlotPool,
        width: u32,
        height: u32,
    ) {
        // Drop any previously attached surface first.
        if let Some(old) = self.layer_surface.take() {
            tracing::warn!("Popup: replacing existing surface before attach");
            drop(old);
        }
        self.layer_surface = Some(layer_surface);
        self.pool = Some(pool);
        self.canvas = Some(Canvas::new(width, height));
        self.width = width;
        self.height = height;
        self.dirty = true;
        tracing::info!("Popup surface attached: {}x{}", width, height);
    }

    /// Returns the Wayland `ObjectId` of the popup surface, if one exists.
    ///
    /// Used by the binary crate's `LayerShellHandler::configure` to route
    /// configure events to the correct surface.
    pub fn surface_id(&self) -> Option<ObjectId> {
        self.layer_surface.as_ref().map(|ls| ls.wl_surface().id())
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

    // ── Rendering ─────────────────────────────────────────────────────────────

    /// Render the popup to its own Wayland surface if dirty.
    ///
    /// Clears the `dirty` flag, renders the content into the internal canvas,
    /// then submits an SHM buffer to the compositor. Returns `true` if a frame
    /// was produced. No-ops when:
    /// - the popup is not dirty, or
    /// - no Wayland surface has been attached yet.
    pub fn frame(&mut self, theme: &ThemeContext) -> bool {
        if !self.dirty || self.layer_surface.is_none() {
            return false;
        }
        self.dirty = false;

        let w = self.width;
        let h = self.height;
        let bounds = Rect::new(0.0, 0.0, w as f32, h as f32);

        if let (Some(canvas), Some(content)) = (self.canvas.as_mut(), self.content.as_ref()) {
            canvas.clear();
            content.render(canvas.pixmap_mut(), theme, bounds);
        }

        self.submit_buffer();
        true
    }

    /// Submit the popup canvas pixels to the Wayland compositor via an SHM buffer.
    ///
    /// Structured identically to `Panel::submit_buffer`: pool borrow ends before
    /// the layer surface is used, avoiding overlapping mutable borrows.
    fn submit_buffer(&mut self) {
        let w = self.width;
        let h = self.height;

        // ── Phase 1: fill SHM buffer ──────────────────────────────────────────
        let buffer = {
            let Some(pool) = self.pool.as_mut() else {
                tracing::warn!("Popup has layer_surface but no shm pool");
                return;
            };
            let stride = w as i32 * 4;
            let (buffer, canvas_data) =
                match pool.create_buffer(w as i32, h as i32, stride, wl_shm::Format::Argb8888) {
                    Ok(pair) => pair,
                    Err(e) => {
                        tracing::error!("Failed to create popup shm buffer: {:?}", e);
                        return;
                    }
                };

            // tiny-skia premultiplied RGBA [R, G, B, A]
            // → Wayland ARGB8888 little-endian     [B, G, R, A]
            if let Some(canvas) = &self.canvas {
                let src = canvas.data();
                let copy_len = canvas_data.len().min(src.len());
                for i in 0..(copy_len / 4) {
                    canvas_data[i * 4] = src[i * 4 + 2]; // B
                    canvas_data[i * 4 + 1] = src[i * 4 + 1]; // G
                    canvas_data[i * 4 + 2] = src[i * 4]; // R
                    canvas_data[i * 4 + 3] = src[i * 4 + 3]; // A
                }
            }
            buffer
            // canvas_data and pool borrows released here
        };

        // ── Phase 2: attach and commit ────────────────────────────────────────
        let Some(layer_surface) = &self.layer_surface else {
            return;
        };
        let wl_surface = layer_surface.wl_surface();
        if let Err(e) = buffer.attach_to(wl_surface) {
            tracing::error!("Failed to attach popup buffer: {:?}", e);
            return;
        }
        wl_surface.damage_buffer(0, 0, w as i32, h as i32);
        wl_surface.commit();
        tracing::debug!("Submitted popup buffer {}x{}", w, h);
    }

    /// Render the popup content into an external `canvas` (for off-screen compositing).
    ///
    /// This variant does not use the popup's own Wayland surface. Used when the
    /// popup content is composited directly onto the panel pixmap rather than a
    /// separate overlay surface.
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
