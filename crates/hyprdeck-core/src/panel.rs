use crate::action::Action;
use crate::autohide::{AnimPhase, AutoHideMode, AutoHideState};
use crate::geometry::{DisplayGeometry, Edge, Point, Size};
use crate::ipc::event::HyprEvent;
use crate::layout::{LayoutEngine, LayoutResult, ModuleGroups, ModuleSizeProvider};
use crate::module::{EventResult, InputEvent, PanelModule, ThemeContext, UpdateContext};
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
    /// Whether this panel needs to be redrawn.
    pub dirty: bool,
    /// Whether the panel surface needs to be resized.
    pub needs_resize: bool,
    /// Current surface width in pixels.
    pub surface_width: u32,
    /// Current surface height in pixels.
    pub surface_height: u32,
    // Wayland handles will be stored here once SCTK integration lands:
    // pub layer_surface: LayerSurface,
    // pub shm_pool: SlotPool,
    // pub wl_surface: wl_surface::WlSurface,
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
            dirty: true,
            needs_resize: false,
            surface_width,
            surface_height,
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
    /// then forwards to that module. Returns any [`Action`] to execute.
    pub fn handle_input(&mut self, event: InputEvent) -> Option<Action> {
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
            return None;
        };

        let point = match &event {
            InputEvent::MousePress { x, y, .. }
            | InputEvent::MouseRelease { x, y, .. }
            | InputEvent::MouseMove { x, y } => Some(Point::new(*x, *y)),
            InputEvent::Scroll { .. } | InputEvent::KeyPress { .. } => None,
        };

        if let Some(pt) = point {
            for (module_id, bounds) in &layout.module_bounds {
                if bounds.contains(pt) {
                    if let Some(module) = self.modules.iter_mut().find(|m| m.id() == module_id) {
                        match module.handle_event(&event, *bounds) {
                            EventResult::Action(action) => return Some(action),
                            EventResult::Handled => {
                                self.dirty = true;
                                return None;
                            }
                            EventResult::Ignored => {}
                        }
                    }
                }
            }
        }

        None
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

        true
    }

    /// Submit the canvas pixmap data to the Wayland surface via shm.
    ///
    /// This is a placeholder that will be wired up once SCTK layer surface
    /// handles are stored on the Panel.
    fn submit_buffer(&mut self) {
        // SCTK buffer submission will go here:
        // 1. Get a buffer from the shm pool matching canvas dimensions
        // 2. Copy canvas.data() into the buffer
        // 3. Attach buffer to wl_surface
        // 4. Mark damage region
        // 5. Commit the surface
        //
        // This is intentionally a no-op until Wayland handles are added.
        tracing::trace!(
            "Panel frame ready: {}x{} ({} bytes)",
            self.surface_width,
            self.surface_height,
            self.canvas.data().len(),
        );
    }

    /// Adjust layer surface margins to slide the panel off-screen
    /// during auto-hide animation.
    fn apply_auto_hide_offset(&mut self) {
        let _offset = match self.edge {
            Edge::Top | Edge::Bottom => {
                -(self.auto_hide.progress * self.surface_height as f32) as i32
            }
            Edge::Left | Edge::Right => {
                -(self.auto_hide.progress * self.surface_width as f32) as i32
            }
        };

        // SCTK margin adjustment will go here:
        // self.layer_surface.set_margin(top, right, bottom, left);
        // Toggle exclusive zone based on phase.
        //
        // This is intentionally a no-op until Wayland handles are added.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autohide::AutoHideMode;
    use crate::layout::{HorizontalLayout, LayoutEngine, ModuleGroups};
    use crate::module::{EventResult, InputEvent, ModuleConfigSchema, MouseButton};
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
            separator: ResolvedSeparator::default(),
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
        assert!(result.is_none());
    }

    #[test]
    fn handle_hypr_event_marks_dirty_for_workspace_change() {
        let mut panel = test_panel();
        panel.dirty = false;

        let event = crate::ipc::event::HyprEvent::WorkspaceChanged { id: 2 };
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
