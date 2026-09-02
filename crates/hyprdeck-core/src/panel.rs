use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shell::wlr_layer::LayerSurface;
use smithay_client_toolkit::shm::slot::SlotPool;
use wayland_client::protocol::wl_shm;

use crate::action::Action;
use crate::autohide::{AnimPhase, AutoHideMode, AutoHideState};
use crate::geometry::{DisplayGeometry, Edge, Point, Rect, Size};
use crate::ipc::event::HyprEvent;
use crate::layout::{LayoutEngine, LayoutResult, ModuleGroups, ModuleSizeProvider};
use crate::module::{
    EventResult, InputEvent, MouseButton, PanelModule, ThemeContext, UpdateContext,
};
use crate::popup::PopupState;
use crate::render::Canvas;

/// RGBA colour stored as four bytes (0–255).
pub type Color = [u8; 4];

/// A single panel instance — one Wayland `zwlr_layer_surface_v1` surface.
///
/// Each connected output may host one or more panels as declared by the active theme.
/// Owns a canvas, layout engine, modules, and auto-hide state machine.
pub struct Panel {
    /// Screen edge this panel is anchored to.
    pub edge: Edge,
    /// Ordered list of modules rendered inside this panel.
    pub modules: Vec<Box<dyn PanelModule>>,
    /// Module grouping for layout (start, center, end).
    pub groups: ModuleGroups,
    /// Layout engine responsible for assigning per-module bounding boxes.
    pub layout: LayoutEngine,
    /// Last computed layout result.
    pub last_layout: Option<LayoutResult>,
    /// Rendering surface.
    pub canvas: Canvas,
    /// Auto-hide state machine for this panel.
    pub auto_hide: AutoHideState,
    /// Fully-resolved visual style (theme defaults + user overrides).
    pub style: ResolvedStyle,
    /// Theme context passed to modules for rendering.
    pub theme_ctx: ThemeContext,
    /// Active popup dropdown for this panel (at most one at a time).
    pub popup: PopupState,
    /// Module currently under the cursor, tracked from `MouseMove` hit tests.
    /// Keyboard events are routed here when no popup is open.
    pub hovered_module: Option<String>,
    /// Whether this panel needs to be redrawn.
    pub dirty: bool,
    /// Whether the panel surface needs to be resized.
    pub needs_resize: bool,
    /// Current surface width in pixels.
    pub surface_width: u32,
    /// Current surface height in pixels.
    pub surface_height: u32,
    /// SCTK layer surface handle. `None` until Wayland surfaces are created.
    pub layer_surface: Option<LayerSurface>,
    /// Whether the compositor has acknowledged the current layer surface.
    /// Buffers must not be attached before its first configure event.
    pub surface_configured: bool,
    /// SHM slot pool for buffer allocation. `None` until Wayland surfaces are created.
    pub pool: Option<SlotPool>,
}

impl std::fmt::Debug for Panel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Panel")
            .field("edge", &self.edge)
            .field("module_count", &self.modules.len())
            .field("dirty", &self.dirty)
            .field("surface_width", &self.surface_width)
            .field("surface_height", &self.surface_height)
            .finish_non_exhaustive()
    }
}

// ── InputResult ───────────────────────────────────────────────────────────────

/// Return value from [`Panel::handle_input`].
///
/// The binary crate checks this after every input event to decide whether to
/// create or destroy a popup Wayland surface.
#[derive(Debug)]
pub enum InputResult {
    /// Event was consumed or ignored with no further side effects.
    None,
    /// A module produced an action that should be dispatched.
    Action(Action),
    /// A popup was toggled **open** for the given module.
    ///
    /// `module_bounds` is the triggering module's rect within the panel's
    /// output-space coordinate system, used to centre the popup on the module.
    ///
    /// The binary crate must:
    /// 1. Read `panel.popup.content` to get the desired size.
    /// 2. Create a `Layer::Overlay` surface with the compositor and layer shell.
    /// 3. Call `panel.attach_popup_surface(layer_surface, pool, width, height)`.
    /// 4. Flush the Wayland connection.
    OpenPopup {
        module_id: String,
        module_bounds: Rect,
    },
    /// The active popup was toggled **closed**.
    ///
    /// `PopupState::close()` has already been called (which drops the
    /// `LayerSurface`). The binary crate only needs to flush the connection
    /// so the compositor receives the destroy request.
    ClosePopup,
}

impl Panel {
    /// Create a new panel from a theme definition on a given output.
    ///
    /// Initialises the canvas, layout engine, module groups, and auto-hide state.
    /// Modules should be added separately via [`Panel::set_modules`].
    pub fn new(
        edge: Edge,
        style: ResolvedStyle,
        auto_hide_mode: AutoHideMode,
        layout: LayoutEngine,
        groups: ModuleGroups,
        surface_width: u32,
        surface_height: u32,
    ) -> Self {
        let theme_ctx = ThemeContext {
            colors: style.colors.clone(),
            fonts: style.fonts.clone(),
            padding: style.padding,
            border_radius: style.border_radius,
            opacity: style.background_opacity,
            // Icon-only status modules are square and occupy the panel's
            // padded cross-axis thickness. This stays correct for vertical
            // panels, whose cross axis is horizontal.
            icon_slot_size: match edge {
                Edge::Top | Edge::Bottom => {
                    (style.bar_height as f32 - style.padding.top - style.padding.bottom).max(1.0)
                }
                Edge::Left | Edge::Right => {
                    (style.bar_height as f32 - style.padding.left - style.padding.right).max(1.0)
                }
            },
            icon_padding: style.icon_padding,
            verbose_text_padding: style
                .verbose_text_padding
                .unwrap_or(style.bar_height as f32 / 8.0),
            module_styles: style.module_styles.clone(),
        };

        Self {
            edge,
            modules: Vec::new(),
            groups,
            layout,
            last_layout: None,
            canvas: Canvas::new(surface_width, surface_height),
            auto_hide: AutoHideState::from_mode(auto_hide_mode),
            style,
            theme_ctx,
            popup: PopupState::new(),
            hovered_module: None,
            dirty: true,
            needs_resize: false,
            surface_width,
            surface_height,
            layer_surface: None,
            surface_configured: false,
            pool: None,
        }
    }

    /// Replace the module list and ensure the dock layout has the correct icon count.
    pub fn set_modules(&mut self, modules: Vec<Box<dyn PanelModule>>) {
        let icon_count = modules.len();
        self.modules = modules;
        self.layout.ensure_icon_count(icon_count);
        self.dirty = true;
    }

    // ── Update Cycle ──────────────────────────────────────

    /// Run module updates. Called on a timer tick.
    /// Returns true if any module needs redraw.
    pub fn update_modules(&mut self, ctx: &UpdateContext<'_>) -> bool {
        let mut needs_redraw = false;
        for module in &mut self.modules {
            if module.update(ctx) {
                needs_redraw = true;
            }
        }
        // Popup content is independent of the panel module render pass.  It
        // still needs the same regular update cadence so open dropdowns can
        // observe shared module state (for example external audio changes).
        if self.popup.update() {
            needs_redraw = true;
        }
        if needs_redraw {
            self.dirty = true;
        }
        needs_redraw
    }

    /// Notify the panel that a Hyprland IPC event occurred.
    ///
    /// Uses the "modules poll shared state" model: we simply mark the panel
    /// dirty for events that commonly affect modules. Modules will re-read
    /// `HyprState` on their next `update()` call.
    pub fn handle_hypr_event(&mut self, event: &HyprEvent) -> bool {
        let needs_redraw = matches!(
            event,
            HyprEvent::WorkspaceChanged { .. }
                | HyprEvent::WorkspaceMoved { .. }
                | HyprEvent::WorkspaceRenamed { .. }
                | HyprEvent::ActiveMonitor { .. }
                | HyprEvent::ActiveWindow { .. }
                | HyprEvent::ActiveWindowV2 { .. }
                | HyprEvent::WindowOpened { .. }
                | HyprEvent::WindowClosed { .. }
                | HyprEvent::WindowMoved { .. }
                | HyprEvent::WindowTitle { .. }
                | HyprEvent::WindowTitleV2 { .. }
                | HyprEvent::Urgent { .. }
                | HyprEvent::Fullscreen { .. }
                | HyprEvent::WorkspaceAdded { .. }
                | HyprEvent::WorkspaceDestroyed { .. }
                | HyprEvent::MonitorAdded { .. }
                | HyprEvent::MonitorRemoved { .. }
        );
        if needs_redraw {
            self.dirty = true;
        }
        needs_redraw
    }

    // ── Input Handling ────────────────────────────────────

    /// Handle an input event on this panel.
    ///
    /// Determines which module the event targets based on layout bounds,
    /// then forwards to that module. Returns an [`InputResult`] describing
    /// what happened:
    ///
    /// - [`InputResult::Action`] — the module produced an action to dispatch.
    /// - [`InputResult::OpenPopup`] — a popup was toggled open; the caller
    ///   must create a Wayland surface and call [`Panel::attach_popup_surface`].
    /// - [`InputResult::ClosePopup`] — the popup was closed; the caller
    ///   should flush the Wayland connection.
    /// - [`InputResult::None`] — event consumed or ignored with no side effects.
    pub fn handle_input(&mut self, event: InputEvent) -> InputResult {
        // Update auto-hide state
        match &event {
            InputEvent::MousePress { .. } | InputEvent::MouseRelease { .. } => {}
            InputEvent::MouseMove { x, y } => {
                let cursor = Point::new(*x, *y);
                if self.layout.update_cursor(Some(cursor)) {
                    self.dirty = true;
                    self.needs_resize = true;
                }
            }
            InputEvent::Scroll { .. } | InputEvent::KeyPress { .. } => {}
        }

        // Hit test: find which module the event lands on
        let Some(layout) = &self.last_layout else {
            tracing::warn!(
                "Panel::handle_input: no layout available for hit test — panel has not rendered yet"
            );
            return InputResult::None;
        };

        // Keyboard events carry no position: route to the module owning the
        // open popup if any, else the hovered module.
        if let InputEvent::KeyPress { .. } = &event {
            let target_id = self
                .popup
                .active_module
                .clone()
                .or_else(|| self.hovered_module.clone());
            let Some(target_id) = target_id else {
                tracing::trace!("KeyPress with no open popup or hovered module — dropped");
                return InputResult::None;
            };
            let Some(bounds) = layout
                .module_bounds
                .iter()
                .find(|(id, _)| *id == target_id)
                .map(|(_, b)| *b)
            else {
                tracing::trace!("KeyPress target '{}' has no layout bounds", target_id);
                return InputResult::None;
            };
            if let Some(module) = self.modules.iter_mut().find(|m| m.id() == target_id) {
                match module.handle_event(&event, bounds) {
                    EventResult::Action(action) => return InputResult::Action(action),
                    EventResult::Handled => {
                        self.dirty = true;
                        return InputResult::None;
                    }
                    EventResult::Ignored => {}
                }
            }
            return InputResult::None;
        }

        // Scroll has no pointer coordinates.  The Wayland integration keeps
        // `hovered_module` current from motion events, so route it there.
        if matches!(&event, InputEvent::Scroll { .. }) {
            let Some(target_id) = self.hovered_module.clone() else {
                return InputResult::None;
            };
            let Some(bounds) = layout
                .module_bounds
                .iter()
                .find(|(id, _)| *id == target_id)
                .map(|(_, b)| *b)
            else {
                return InputResult::None;
            };
            if let Some(module) = self.modules.iter_mut().find(|m| m.id() == target_id) {
                match module.handle_event(&event, bounds) {
                    EventResult::Action(action) => return InputResult::Action(action),
                    EventResult::Handled => {
                        self.dirty = true;
                    }
                    EventResult::Ignored => {}
                }
            }
            return InputResult::None;
        }

        let point = match &event {
            InputEvent::MousePress { x, y, .. }
            | InputEvent::MouseRelease { x, y, .. }
            | InputEvent::MouseMove { x, y } => Some(Point::new(*x, *y)),
            InputEvent::Scroll { .. } | InputEvent::KeyPress { .. } => None,
        };

        if let Some(pt) = point {
            let mut hit_any = false;
            for (module_id, bounds) in &layout.module_bounds {
                if bounds.contains(pt) {
                    hit_any = true;
                    if matches!(&event, InputEvent::MouseMove { .. }) {
                        self.hovered_module = Some(module_id.clone());
                    }
                    // Left-click on a popup-capable module toggles its popup.
                    if let InputEvent::MousePress {
                        button: MouseButton::Left,
                        ..
                    } = &event
                    {
                        let has_popup = self
                            .modules
                            .iter()
                            .find(|m| m.id() == module_id.as_str())
                            .map(|m| m.has_popup())
                            .unwrap_or(false);
                        if has_popup {
                            // Record whether this module's popup was already open
                            // so we can distinguish open vs close after the toggle.
                            let was_open =
                                self.popup.active_module.as_deref() == Some(module_id.as_str());
                            let content = self
                                .modules
                                .iter()
                                .find(|m| m.id() == module_id.as_str())
                                .and_then(|m| m.popup_content());
                            let Some(content) = content else {
                                tracing::warn!(
                                    "Module '{}' advertises a popup but returned no content",
                                    module_id
                                );
                                return InputResult::None;
                            };
                            self.popup.toggle(module_id, || content);
                            self.dirty = true;
                            if was_open {
                                // Toggle closed the popup.
                                return InputResult::ClosePopup;
                            } else {
                                // Toggle opened (or switched to) a new popup.
                                return InputResult::OpenPopup {
                                    module_id: module_id.clone(),
                                    module_bounds: *bounds,
                                };
                            }
                        }
                    }

                    if let Some(module) = self
                        .modules
                        .iter_mut()
                        .find(|m| m.id() == module_id.as_str())
                    {
                        match module.handle_event(&event, *bounds) {
                            EventResult::Action(action) => return InputResult::Action(action),
                            EventResult::Handled => {
                                self.dirty = true;
                                return InputResult::None;
                            }
                            EventResult::Ignored => {}
                        }
                    }
                }
            }
            if !hit_any && matches!(&event, InputEvent::MouseMove { .. }) {
                self.hovered_module = None;
            }
        }

        InputResult::None
    }

    /// Attach a freshly-created popup Wayland surface to this panel.
    ///
    /// Called by the binary crate after it processes an [`InputResult::OpenPopup`]
    /// and creates the surface via `CompositorState` / `LayerShell`.
    pub fn attach_popup_surface(
        &mut self,
        layer_surface: smithay_client_toolkit::shell::wlr_layer::LayerSurface,
        pool: smithay_client_toolkit::shm::slot::SlotPool,
        width: u32,
        height: u32,
    ) {
        self.popup
            .attach_surface(layer_surface, pool, width, height);
    }

    /// Notify the panel that the cursor entered its surface.
    pub fn on_cursor_enter(&mut self) {
        self.auto_hide.on_cursor_enter();
        if self.auto_hide.tick(0.0) {
            self.dirty = true;
        }
    }

    /// Notify the panel that the cursor left its surface.
    pub fn on_cursor_leave(&mut self) {
        self.auto_hide.on_cursor_leave();
        self.hovered_module = None;
        // Reset dock magnification
        if self.layout.update_cursor(None) {
            self.dirty = true;
        }
    }

    // ── Auto-Hide ─────────────────────────────────────────

    /// Tick auto-hide animation. Returns true if still animating.
    pub fn tick_auto_hide(&mut self, dt: f32) -> bool {
        let animating = self.auto_hide.tick(dt);
        if animating {
            self.dirty = true;
        }
        animating
    }

    /// Returns true if the auto-hide state machine is currently animating.
    pub fn is_auto_hide_animating(&self) -> bool {
        matches!(self.auto_hide.phase, AnimPhase::Showing | AnimPhase::Hiding)
    }

    // ── Layout & Render ───────────────────────────────────

    /// Run layout and render if dirty. Returns true if a frame was produced.
    pub fn frame(&mut self, display: &DisplayGeometry) -> bool {
        if self.layer_surface.is_some() && !self.surface_configured {
            return false;
        }

        // Tick dock animation
        let dock_animating = self.layout.tick_animation(1.0 / 60.0);
        if dock_animating {
            self.dirty = true;
        }

        if !self.dirty {
            return false;
        }
        self.dirty = false;

        // Run layout
        let sizes = ModuleSizeAdapter(&self.modules, &self.theme_ctx);
        let layout = self
            .layout
            .layout(&self.groups, &sizes, &self.style, display);

        // Check if surface needs resize (dock magnification)
        let needed_width = (layout.total_size.width.ceil() as u32).max(1);
        let needed_height = (layout.total_size.height.ceil() as u32).max(1);
        if needed_width != self.surface_width || needed_height != self.surface_height {
            self.surface_width = needed_width;
            self.surface_height = needed_height;
            self.canvas.resize(needed_width, needed_height);
            self.needs_resize = true;
        }

        // Render
        crate::render::render_panel(
            &mut self.canvas,
            &layout,
            &self.modules,
            &self.style,
            &self.theme_ctx,
        );

        // Apply auto-hide offset
        if !matches!(self.auto_hide.mode, AutoHideMode::Disabled) {
            self.apply_auto_hide_offset();
        }

        self.last_layout = Some(layout);

        // Submit buffer to Wayland surface
        self.submit_buffer();

        // Render popup to its own surface if configured, attached, and dirty.
        // `configured` must be true before we may attach any buffer: the Wayland
        // protocol requires the compositor's first configure to arrive first.
        if self.popup.dirty && self.popup.configured && self.popup.layer_surface.is_some() {
            self.popup.frame(&self.theme_ctx);
        }

        true
    }

    /// Submit the canvas pixmap data to the Wayland compositor via an SHM buffer.
    ///
    /// No-ops gracefully when the panel has no Wayland surface (e.g. in tests or
    /// headless mode).  When a surface is present the function:
    ///
    /// 1. Allocates a slot from the SHM pool.
    /// 2. Copies the tiny-skia RGBA pixels, byte-swapping R↔B so they match
    ///    Wayland's `ARGB8888` little-endian layout (`[B, G, R, A]`).
    /// 3. Attaches the buffer, damages the full surface, and commits.
    fn submit_buffer(&mut self) {
        if self.layer_surface.is_none() {
            tracing::trace!(
                "Panel frame ready (no surface): {}x{} ({} bytes)",
                self.surface_width,
                self.surface_height,
                self.canvas.data().len(),
            );
            return;
        }

        let w = self.surface_width;
        let h = self.surface_height;

        // ── Phase 1: allocate a buffer slot and fill it ───────────────────
        //
        // This block borrows `self.pool` (mutably) and `self.canvas` (immutably).
        // Both borrows are released when the block exits, before Phase 2 touches
        // `self.layer_surface`.
        let buffer = {
            let Some(pool) = self.pool.as_mut() else {
                tracing::warn!("Panel has layer_surface but no shm pool");
                return;
            };

            let stride = w as i32 * 4;
            let (buffer, canvas_data) =
                match pool.create_buffer(w as i32, h as i32, stride, wl_shm::Format::Argb8888) {
                    Ok(pair) => pair,
                    Err(e) => {
                        tracing::error!("Failed to create shm buffer: {:?}", e);
                        return;
                    }
                };

            // Convert tiny-skia premultiplied RGBA [R, G, B, A]
            // → Wayland ARGB8888 little-endian bytes   [B, G, R, A].
            let src = self.canvas.data();
            let copy_len = canvas_data.len().min(src.len());
            for i in 0..(copy_len / 4) {
                canvas_data[i * 4] = src[i * 4 + 2]; // B
                canvas_data[i * 4 + 1] = src[i * 4 + 1]; // G
                canvas_data[i * 4 + 2] = src[i * 4]; // R
                canvas_data[i * 4 + 3] = src[i * 4 + 3]; // A
            }

            buffer
            // canvas_data and pool borrows released here
        };

        // ── Phase 2: attach the buffer to the wl_surface and commit ──────
        let layer = self.layer_surface.as_ref().unwrap();
        let wl_surface = layer.wl_surface();

        // `attach_to` marks the slot active until the compositor releases it.
        if let Err(e) = buffer.attach_to(wl_surface) {
            tracing::error!("Failed to attach buffer to surface: {:?}", e);
            return;
        }
        wl_surface.damage_buffer(0, 0, w as i32, h as i32);
        wl_surface.commit();

        tracing::debug!("Submitted buffer {}x{}", w, h);
    }

    /// Adjust layer surface margins to slide the panel off-screen
    /// during auto-hide animation.
    fn apply_auto_hide_offset(&mut self) {
        let Some(layer) = &self.layer_surface else {
            return;
        };

        let offset = match self.edge {
            Edge::Top | Edge::Bottom => {
                -(self.auto_hide.progress * self.surface_height as f32) as i32
            }
            Edge::Left | Edge::Right => {
                -(self.auto_hide.progress * self.surface_width as f32) as i32
            }
        };

        // Slide the panel by adjusting its margin.
        match self.edge {
            Edge::Top => layer.set_margin(offset, 0, 0, 0),
            Edge::Bottom => layer.set_margin(0, 0, offset, 0),
            Edge::Left => layer.set_margin(0, 0, 0, offset),
            Edge::Right => layer.set_margin(0, offset, 0, 0),
        }

        // Release the exclusive zone when the panel is fully hidden so
        // windows can use that screen area.
        let zone = if self.auto_hide.progress == 0.0 {
            -1 // no exclusive zone while hidden
        } else {
            match self.edge {
                Edge::Top | Edge::Bottom => self.surface_height as i32,
                Edge::Left | Edge::Right => self.surface_width as i32,
            }
        };
        layer.set_exclusive_zone(zone);
    }
}

/// Adapter to satisfy [`ModuleSizeProvider`] from a slice of modules.
struct ModuleSizeAdapter<'a>(&'a [Box<dyn PanelModule>], &'a ThemeContext);

impl ModuleSizeProvider for ModuleSizeAdapter<'_> {
    fn desired_size(&self, module_id: &str) -> Size {
        self.0
            .iter()
            .find(|m| m.id() == module_id)
            .map(|m| m.desired_size(self.1))
            .unwrap_or(Size::new(0.0, 0.0))
    }
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
    /// Inset between an icon-only module slot and its drawn content.
    pub icon_padding: f32,
    /// Separator line styling between adjacent modules.
    pub separator: ResolvedSeparator,
    /// Blank space between adjacent module slots in logical pixels.
    pub module_gap: f32,
    /// Gap between the icon half and text half in verbose display mode, in
    /// logical pixels.  Kept optional because the default depends on
    /// `bar_height`, which `create_panel()` overrides after style resolution;
    /// the final value is resolved into `ThemeContext` in `Panel::new`.
    pub verbose_text_padding: Option<f32>,
    /// Per-module color overrides for this panel.
    pub module_styles: ResolvedModuleStyles,
}

// ── Per-module resolved styles ────────────────────────────────────────────────

/// Resolved (color-parsed) per-module style overrides for a panel.
#[derive(Debug, Clone)]
pub struct ResolvedModuleStyles {
    pub window_list: ResolvedWindowListStyle,
    pub workspaces: ResolvedWorkspacesStyle,
}

/// Resolved colors for the `window_list` module.
#[derive(Debug, Clone)]
pub struct ResolvedWindowListStyle {
    pub active_background: Color,
    pub active_foreground: Color,
    pub inactive_background: Color,
    pub inactive_foreground: Color,
}

/// Resolved colors for the `workspaces` module.
#[derive(Debug, Clone)]
pub struct ResolvedWorkspacesStyle {
    pub active_background: Color,
    pub active_foreground: Color,
    pub inactive_background: Color,
    pub inactive_foreground: Color,
    /// Background for a workspace owned by another output.
    pub remote_background: Color,
    /// Foreground for a workspace owned by another output.
    pub remote_foreground: Color,
    /// Deliberately muted urgency background for another output.
    pub remote_urgent_background: Color,
    /// Foreground for a muted urgent workspace on another output.
    pub remote_urgent_foreground: Color,
}

impl Default for ResolvedModuleStyles {
    fn default() -> Self {
        Self {
            window_list: ResolvedWindowListStyle {
                active_background: [80, 160, 255, 200],
                active_foreground: [30, 30, 30, 255],
                inactive_background: [255, 255, 255, 10],
                inactive_foreground: [255, 255, 255, 255],
            },
            workspaces: ResolvedWorkspacesStyle {
                active_background: [80, 160, 255, 255],
                active_foreground: [30, 30, 30, 255],
                inactive_background: [255, 255, 255, 80],
                inactive_foreground: [255, 255, 255, 255],
                remote_background: [128, 128, 128, 80],
                remote_foreground: [192, 192, 192, 190],
                remote_urgent_background: [150, 96, 96, 190],
                remote_urgent_foreground: [245, 225, 225, 220],
            },
        }
    }
}

impl ResolvedModuleStyles {
    /// Build default module styles from a resolved color palette.
    ///
    /// Used as the starting point before per-panel overrides are applied.
    pub fn from_palette(colors: &ColorPalette) -> Self {
        let mut inactive_wl = colors.foreground;
        inactive_wl[3] = 10;

        let mut active_wl = colors.accent;
        active_wl[3] = 200;

        let mut inactive_ws = colors.foreground;
        inactive_ws[3] = 80;
        let remote_ws = muted_color(colors.foreground, 96, 80);
        let remote_foreground = muted_color(colors.foreground, 160, 190);
        let remote_urgent = muted_color(colors.urgent, 112, 190);

        Self {
            window_list: ResolvedWindowListStyle {
                active_background: active_wl,
                active_foreground: colors.background,
                inactive_background: inactive_wl,
                inactive_foreground: colors.foreground,
            },
            workspaces: ResolvedWorkspacesStyle {
                active_background: colors.accent,
                active_foreground: colors.background,
                inactive_background: inactive_ws,
                inactive_foreground: colors.foreground,
                remote_background: remote_ws,
                remote_foreground,
                remote_urgent_background: remote_urgent,
                remote_urgent_foreground: remote_foreground,
            },
        }
    }
}

/// Desaturate `color` toward a neutral value while retaining enough alpha for
/// the workspace indicator to read as deliberately remote rather than absent.
fn muted_color(color: Color, neutral: u8, alpha: u8) -> Color {
    let mix = |channel: u8| ((u16::from(channel) + u16::from(neutral)) / 2) as u8;
    [mix(color[0]), mix(color[1]), mix(color[2]), alpha]
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
    /// Optional monospace family for fixed-width rendering; falls back to `"monospace"` if `None`.
    pub mono_family: Option<String>,
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
            visible: false,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autohide::AutoHideMode;
    use crate::layout::{HorizontalLayout, LayoutEngine, ModuleGroups};
    use crate::module::{EventResult, InputEvent, ModuleConfigSchema, MouseButton};
    use crate::popup::{PopupContent, PopupEventResult};
    use tiny_skia::Pixmap;

    /// Minimal test module for unit tests.
    struct TestModule {
        name: String,
        size: Size,
    }

    impl TestModule {
        fn new(name: &str, width: f32, height: f32) -> Self {
            Self {
                name: name.to_owned(),
                size: Size::new(width, height),
            }
        }
    }

    impl PanelModule for TestModule {
        fn id(&self) -> &str {
            &self.name
        }

        fn desired_size(&self, _theme: &ThemeContext) -> Size {
            self.size
        }

        fn update(&mut self, _ctx: &UpdateContext<'_>) -> bool {
            false
        }

        fn render(
            &self,
            _canvas: &mut Pixmap,
            _theme: &ThemeContext,
            _bounds: crate::geometry::Rect,
        ) {
        }

        fn handle_event(
            &mut self,
            event: &InputEvent,
            _bounds: crate::geometry::Rect,
        ) -> EventResult {
            match event {
                InputEvent::MousePress { .. } => EventResult::Handled,
                _ => EventResult::Ignored,
            }
        }

        fn config_schema(&self) -> ModuleConfigSchema {
            ModuleConfigSchema {
                module_id: self.name.clone(),
                fields: vec![],
            }
        }
    }

    struct TestPopupContent;

    impl PopupContent for TestPopupContent {
        fn desired_size(&self, _theme: &ThemeContext) -> Size {
            Size::new(160.0, 120.0)
        }

        fn render(&self, _canvas: &mut Pixmap, _theme: &ThemeContext, _bounds: Rect) {}

        fn handle_event(&mut self, _event: &InputEvent, _bounds: Rect) -> PopupEventResult {
            PopupEventResult::Ignored
        }

        fn update(&mut self) -> bool {
            false
        }
    }

    struct TestPopupModule;

    impl PanelModule for TestPopupModule {
        fn id(&self) -> &str {
            "popup"
        }

        fn desired_size(&self, _theme: &ThemeContext) -> Size {
            Size::new(60.0, 32.0)
        }

        fn update(&mut self, _ctx: &UpdateContext<'_>) -> bool {
            false
        }

        fn render(&self, _canvas: &mut Pixmap, _theme: &ThemeContext, _bounds: Rect) {}

        fn handle_event(&mut self, _event: &InputEvent, _bounds: Rect) -> EventResult {
            EventResult::Ignored
        }

        fn config_schema(&self) -> ModuleConfigSchema {
            ModuleConfigSchema {
                module_id: self.id().to_owned(),
                fields: vec![],
            }
        }

        fn has_popup(&self) -> bool {
            true
        }

        fn popup_content(&self) -> Option<Box<dyn PopupContent>> {
            Some(Box::new(TestPopupContent))
        }
    }

    fn test_style() -> ResolvedStyle {
        ResolvedStyle {
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
            bar_height: 32,
            padding: Padding {
                top: 4.0,
                right: 8.0,
                bottom: 4.0,
                left: 8.0,
            },
            border_radius: 0.0,
            background_opacity: 0.9,
            icon_padding: 2.0,
            separator: ResolvedSeparator::default(),
            module_gap: 0.0,
            verbose_text_padding: None,
            module_styles: ResolvedModuleStyles::default(),
        }
    }

    fn test_panel() -> Panel {
        let groups = ModuleGroups {
            start: vec!["a".into(), "b".into()],
            center: vec![],
            end: vec!["c".into()],
        };
        let mut panel = Panel::new(
            Edge::Top,
            test_style(),
            AutoHideMode::Disabled,
            LayoutEngine::Horizontal(HorizontalLayout::new()),
            groups,
            1920,
            32,
        );
        let modules: Vec<Box<dyn PanelModule>> = vec![
            Box::new(TestModule::new("a", 100.0, 32.0)),
            Box::new(TestModule::new("b", 80.0, 32.0)),
            Box::new(TestModule::new("c", 60.0, 32.0)),
        ];
        panel.set_modules(modules);
        panel
    }

    #[test]
    fn panel_starts_dirty() {
        let panel = test_panel();
        assert!(panel.dirty);
    }

    /// Shared log of keysyms received by a [`KeyRecorder`].
    type KeyLog = std::sync::Arc<std::sync::Mutex<Vec<u32>>>;

    /// Test module that records every KeyPress it receives.
    struct KeyRecorder {
        name: String,
        size: Size,
        received: KeyLog,
    }

    impl PanelModule for KeyRecorder {
        fn id(&self) -> &str {
            &self.name
        }

        fn desired_size(&self, _theme: &ThemeContext) -> Size {
            self.size
        }

        fn update(&mut self, _ctx: &UpdateContext<'_>) -> bool {
            false
        }

        fn render(
            &self,
            _canvas: &mut Pixmap,
            _theme: &ThemeContext,
            _bounds: crate::geometry::Rect,
        ) {
        }

        fn handle_event(
            &mut self,
            event: &InputEvent,
            _bounds: crate::geometry::Rect,
        ) -> EventResult {
            match event {
                InputEvent::KeyPress { key, .. } => {
                    self.received.lock().unwrap().push(*key);
                    EventResult::Handled
                }
                _ => EventResult::Ignored,
            }
        }

        fn config_schema(&self) -> ModuleConfigSchema {
            ModuleConfigSchema {
                module_id: self.name.clone(),
                fields: vec![],
            }
        }
    }

    /// Panel with two key-recording modules "a" and "b"; returns the panel and
    /// the per-module key logs.
    fn key_test_panel() -> (Panel, KeyLog, KeyLog) {
        let groups = ModuleGroups {
            start: vec!["a".into(), "b".into()],
            center: vec![],
            end: vec![],
        };
        let mut panel = Panel::new(
            Edge::Top,
            test_style(),
            AutoHideMode::Disabled,
            LayoutEngine::Horizontal(HorizontalLayout::new()),
            groups,
            1920,
            32,
        );
        let keys_a = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let keys_b = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let modules: Vec<Box<dyn PanelModule>> = vec![
            Box::new(KeyRecorder {
                name: "a".into(),
                size: Size::new(100.0, 32.0),
                received: keys_a.clone(),
            }),
            Box::new(KeyRecorder {
                name: "b".into(),
                size: Size::new(80.0, 32.0),
                received: keys_b.clone(),
            }),
        ];
        panel.set_modules(modules);
        // Force a layout so last_layout is populated.
        let display = DisplayGeometry {
            bounds: crate::geometry::Rect::new(0.0, 0.0, 1920.0, 1080.0),
            usable_region: None,
            edge_path: None,
        };
        panel.frame(&display);
        (panel, keys_a, keys_b)
    }

    #[test]
    fn key_press_routes_to_hovered_module() {
        let (mut panel, keys_a, _keys_b) = key_test_panel();
        // Hover module "a" (its slot starts at the left edge).
        let _ = panel.handle_input(InputEvent::MouseMove { x: 10.0, y: 16.0 });
        assert_eq!(panel.hovered_module.as_deref(), Some("a"));

        let result = panel.handle_input(InputEvent::KeyPress {
            key: 42,
            modifiers: 0,
        });
        assert!(matches!(result, InputResult::None));
        assert_eq!(*keys_a.lock().unwrap(), vec![42]);
    }

    #[test]
    fn key_press_without_hover_or_popup_is_dropped() {
        let (mut panel, keys_a, keys_b) = key_test_panel();
        let result = panel.handle_input(InputEvent::KeyPress {
            key: 42,
            modifiers: 0,
        });
        assert!(matches!(result, InputResult::None));
        assert!(keys_a.lock().unwrap().is_empty());
        assert!(keys_b.lock().unwrap().is_empty());
    }

    #[test]
    fn key_press_prefers_open_popup_over_hover() {
        let (mut panel, keys_a, keys_b) = key_test_panel();
        let _ = panel.handle_input(InputEvent::MouseMove { x: 10.0, y: 16.0 });
        panel.popup.active_module = Some("b".into());

        let _ = panel.handle_input(InputEvent::KeyPress {
            key: 7,
            modifiers: 0,
        });
        assert!(keys_a.lock().unwrap().is_empty());
        assert_eq!(*keys_b.lock().unwrap(), vec![7]);
    }

    #[test]
    fn cursor_leave_clears_hovered_module() {
        let (mut panel, _keys_a, _keys_b) = key_test_panel();
        let _ = panel.handle_input(InputEvent::MouseMove { x: 10.0, y: 16.0 });
        assert!(panel.hovered_module.is_some());
        panel.on_cursor_leave();
        assert!(panel.hovered_module.is_none());
    }

    #[test]
    fn theme_ctx_uses_explicit_verbose_text_padding() {
        let mut style = test_style();
        style.verbose_text_padding = Some(3.0);
        let panel = Panel::new(
            Edge::Top,
            style,
            AutoHideMode::Disabled,
            LayoutEngine::Horizontal(HorizontalLayout::new()),
            ModuleGroups::default(),
            1920,
            32,
        );
        assert_eq!(panel.theme_ctx.verbose_text_padding, 3.0);
    }

    #[test]
    fn theme_ctx_defaults_verbose_text_padding_to_bar_height_eighth() {
        let mut style = test_style();
        style.verbose_text_padding = None;
        style.bar_height = 40;
        let panel = Panel::new(
            Edge::Top,
            style,
            AutoHideMode::Disabled,
            LayoutEngine::Horizontal(HorizontalLayout::new()),
            ModuleGroups::default(),
            1920,
            40,
        );
        assert_eq!(panel.theme_ctx.verbose_text_padding, 5.0);
    }

    #[test]
    fn theme_ctx_derives_icon_slot_from_cross_axis_content() {
        let mut horizontal = test_style();
        horizontal.bar_height = 40;
        horizontal.padding.top = 3.0;
        horizontal.padding.bottom = 5.0;
        let panel = Panel::new(
            Edge::Top,
            horizontal,
            AutoHideMode::Disabled,
            LayoutEngine::Horizontal(HorizontalLayout::new()),
            ModuleGroups::default(),
            1920,
            40,
        );
        assert_eq!(panel.theme_ctx.icon_slot_size, 32.0);

        let mut vertical = test_style();
        vertical.bar_height = 48;
        vertical.padding.left = 6.0;
        vertical.padding.right = 8.0;
        let panel = Panel::new(
            Edge::Left,
            vertical,
            AutoHideMode::Disabled,
            LayoutEngine::Vertical(super::super::layout::VerticalLayout::new()),
            ModuleGroups::default(),
            48,
            1080,
        );
        assert_eq!(panel.theme_ctx.icon_slot_size, 34.0);
    }

    #[test]
    fn module_size_adapter_returns_correct_sizes() {
        let panel = test_panel();
        let adapter = ModuleSizeAdapter(&panel.modules, &panel.theme_ctx);
        let size_a = adapter.desired_size("a");
        assert_eq!(size_a.width, 100.0);
        assert_eq!(size_a.height, 32.0);
    }

    #[test]
    fn module_size_adapter_returns_zero_for_unknown() {
        let panel = test_panel();
        let adapter = ModuleSizeAdapter(&panel.modules, &panel.theme_ctx);
        let size = adapter.desired_size("nonexistent");
        assert_eq!(size.width, 0.0);
        assert_eq!(size.height, 0.0);
    }

    #[test]
    fn handle_input_returns_none_outside_all_modules() {
        let mut panel = test_panel();
        // Force a layout so last_layout is populated
        let display = DisplayGeometry {
            bounds: crate::geometry::Rect::new(0.0, 0.0, 1920.0, 1080.0),
            usable_region: None,
            edge_path: None,
        };
        panel.frame(&display);

        // Click far outside any module bounds
        let result = panel.handle_input(InputEvent::MousePress {
            x: 5000.0,
            y: 5000.0,
            button: MouseButton::Left,
        });
        assert!(matches!(result, InputResult::None));
    }

    #[test]
    fn right_aligned_module_hover_and_click_open_its_popup() {
        let groups = ModuleGroups {
            start: vec![],
            center: vec![],
            end: vec!["popup".into()],
        };
        let mut panel = Panel::new(
            Edge::Top,
            test_style(),
            AutoHideMode::Disabled,
            LayoutEngine::Horizontal(HorizontalLayout::new()),
            groups,
            400,
            32,
        );
        panel.set_modules(vec![Box::new(TestPopupModule)]);
        panel.frame(&DisplayGeometry {
            bounds: Rect::new(0.0, 0.0, 400.0, 32.0),
            usable_region: None,
            edge_path: None,
        });

        let bounds = panel.last_layout.as_ref().unwrap().module_bounds[0].1;
        assert!(bounds.x > 300.0, "end group should remain right-aligned");
        let x = bounds.x + bounds.width / 2.0;
        let y = bounds.y + bounds.height / 2.0;

        assert!(matches!(
            panel.handle_input(InputEvent::MouseMove { x, y }),
            InputResult::None
        ));
        assert_eq!(panel.hovered_module.as_deref(), Some("popup"));

        let result = panel.handle_input(InputEvent::MousePress {
            x,
            y,
            button: MouseButton::Left,
        });
        assert!(matches!(
            result,
            InputResult::OpenPopup {
                ref module_id,
                module_bounds
            } if module_id == "popup"
                && module_bounds.x == bounds.x
                && module_bounds.y == bounds.y
                && module_bounds.width == bounds.width
                && module_bounds.height == bounds.height
        ));
        assert_eq!(panel.popup.active_module.as_deref(), Some("popup"));
        assert!(panel.popup.content.is_some());
    }

    #[test]
    fn handle_hypr_event_marks_dirty_for_workspace_change() {
        let mut panel = test_panel();
        panel.dirty = false;

        let event = crate::ipc::event::HyprEvent::WorkspaceChanged {
            workspace: crate::ipc::event::WorkspaceRef::Id(2),
        };
        let needs_redraw = panel.handle_hypr_event(&event);
        assert!(needs_redraw);
        assert!(panel.dirty);
    }

    #[test]
    fn handle_hypr_event_ignores_layout_changed() {
        let mut panel = test_panel();
        panel.dirty = false;

        let event = crate::ipc::event::HyprEvent::LayoutChanged {
            keyboard: "kb".into(),
            layout: "us".into(),
        };
        let needs_redraw = panel.handle_hypr_event(&event);
        assert!(!needs_redraw);
        assert!(!panel.dirty);
    }

    #[test]
    fn panel_frame_clears_dirty_flag() {
        let mut panel = test_panel();
        assert!(panel.dirty);

        let display = DisplayGeometry {
            bounds: crate::geometry::Rect::new(0.0, 0.0, 1920.0, 1080.0),
            usable_region: None,
            edge_path: None,
        };
        let rendered = panel.frame(&display);
        assert!(rendered);
        assert!(!panel.dirty);
    }

    #[test]
    fn panel_frame_returns_false_when_not_dirty() {
        let mut panel = test_panel();
        let display = DisplayGeometry {
            bounds: crate::geometry::Rect::new(0.0, 0.0, 1920.0, 1080.0),
            usable_region: None,
            edge_path: None,
        };
        panel.frame(&display);
        assert!(!panel.dirty);

        let rendered = panel.frame(&display);
        assert!(!rendered);
    }

    #[test]
    fn auto_hide_disabled_panel_is_not_animating() {
        let panel = test_panel();
        assert!(!panel.is_auto_hide_animating());
    }

    #[test]
    fn set_modules_marks_dirty() {
        let groups = ModuleGroups::default();
        let mut panel = Panel::new(
            Edge::Bottom,
            test_style(),
            AutoHideMode::Disabled,
            LayoutEngine::Horizontal(HorizontalLayout::new()),
            groups,
            1920,
            32,
        );
        panel.dirty = false;
        panel.set_modules(vec![Box::new(TestModule::new("x", 50.0, 32.0))]);
        assert!(panel.dirty);
    }
}
