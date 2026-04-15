use std::collections::HashMap;
use std::os::unix::io::{AsFd, AsRawFd, RawFd};
use std::time::Duration;

use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_pointer, delegate_registry,
    delegate_seat, delegate_shm,
    output::{OutputHandler, OutputState as SctkOutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{Capability, SeatHandler, SeatState},
    seat::pointer::{BTN_LEFT, BTN_MIDDLE, BTN_RIGHT, PointerEvent, PointerEventKind, PointerHandler},
    shell::{
        WaylandSurface,
        wlr_layer::{
            Anchor, Layer, LayerShell, LayerShellHandler, LayerSurface, LayerSurfaceConfigure,
        },
    },
    shm::{Shm, ShmHandler, slot::SlotPool},
};
use tracing::{debug, error, info, trace, warn};
use wayland_client::{
    Connection, EventQueue, Proxy, QueueHandle,
    globals::registry_queue_init,
    protocol::{wl_output, wl_pointer, wl_seat, wl_surface},
};

use hyprdeck_core::ipc::HyprIpc;
use hyprdeck_core::{App, Edge, InputEvent, InputResult, MouseButton};

// ── Application state ─────────────────────────────────────────────────────────

/// Combined application + Wayland state, passed to every SCTK delegate callback.
struct AppState {
    /// HyprDeck application logic.
    app: App,

    // ── SCTK state ────────────────────────────────────────────────────────────
    registry_state: RegistryState,
    compositor_state: CompositorState,
    output_state: SctkOutputState,
    shm: Shm,
    layer_shell: LayerShell,
    seat_state: SeatState,

    /// Map from Wayland output connector name (e.g. "DP-2") to the WlOutput
    /// handle.  Populated as Wayland outputs are advertised.  Used to direct
    /// each panel's layer surface to the correct physical output.
    wl_outputs: HashMap<String, wl_output::WlOutput>,

    /// The active Wayland pointer object.  `None` until a seat with pointer
    /// capability is seen.
    pointer: Option<wl_pointer::WlPointer>,
}

impl AppState {
    /// Create a Wayland `Overlay`-layer surface for a panel's popup.
    ///
    /// Called after [`Panel::handle_input`] returns [`InputResult::OpenPopup`].
    /// Reads the desired size from `panel.popup.content`, creates the surface,
    /// positions it adjacent to the panel edge, performs the initial empty commit
    /// to trigger a `configure`, and calls [`Panel::attach_popup_surface`] to hand
    /// over ownership.
    ///
    fn create_popup_surface_for_panel(
        &mut self,
        output_name: &str,
        panel_idx: usize,
        module_id: &str,
        qh: &QueueHandle<AppState>,
    ) {
        // ── Read desired size from popup content ──────────────────────────────
        let (width, height, edge, panel_w, panel_h) = {
            let Some(output) = self.app.outputs.get(output_name) else {
                return;
            };
            let panel = &output.panels[panel_idx];
            let Some(content) = &panel.popup.content else {
                warn!("open_popup called but popup.content is None for '{}'", module_id);
                return;
            };
            let size = content.desired_size(&panel.theme_ctx);
            let w = (size.width.ceil() as u32).max(1);
            let h = (size.height.ceil() as u32).max(1);
            (w, h, panel.edge, panel.surface_width, panel.surface_height)
        };

        let wl_output = self.wl_outputs.get(output_name).cloned();

        // ── Create the wl_surface and layer surface ───────────────────────────
        let surface = self.compositor_state.create_surface(qh);
        let layer = self.layer_shell.create_layer_surface(
            qh,
            surface,
            Layer::Overlay,
            Some("hyprdeck-popup"),
            wl_output.as_ref(),
        );

        // Fixed size — no exclusive zone (overlay must not push other surfaces).
        layer.set_size(width, height);
        layer.set_exclusive_zone(0);

        // Anchor and margin place the popup flush against the panel surface.
        // The margin on the panel-side equals the panel thickness, so the popup
        // appears just outside the panel rather than overlapping it.
        match edge {
            Edge::Bottom => {
                layer.set_anchor(Anchor::BOTTOM | Anchor::LEFT);
                layer.set_margin(0, 0, panel_h as i32, 0);
            }
            Edge::Top => {
                layer.set_anchor(Anchor::TOP | Anchor::LEFT);
                layer.set_margin(panel_h as i32, 0, 0, 0);
            }
            Edge::Left => {
                layer.set_anchor(Anchor::LEFT | Anchor::TOP);
                layer.set_margin(0, 0, 0, panel_w as i32);
            }
            Edge::Right => {
                layer.set_anchor(Anchor::RIGHT | Anchor::TOP);
                layer.set_margin(0, panel_w as i32, 0, 0);
            }
        }

        // Initial empty commit → compositor sends configure.
        layer.commit();
        info!(
            "Created popup surface {}x{} for module '{}' on '{}'",
            width, height, module_id, output_name
        );

        // ── Allocate SHM pool ─────────────────────────────────────────────────
        let pool_size = ((width * height * 4) as usize).max(4096);
        let pool = match SlotPool::new(pool_size, &self.shm) {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to allocate popup shm pool: {}", e);
                return;
            }
        };

        // ── Hand ownership to the panel's PopupState ──────────────────────────
        let output = self.app.outputs.get_mut(output_name).unwrap();
        output.panels[panel_idx].attach_popup_surface(layer, pool, width, height);
    }

    /// Process an [`InputResult`] that was returned by a panel's `handle_input`.
    ///
    /// Creates or destroys popup Wayland surfaces as needed and flushes the
    /// Wayland connection.  Returns the event queue so the caller can flush.
    ///
    fn handle_input_result(
        &mut self,
        result: InputResult,
        output_name: &str,
        panel_idx: usize,
        qh: &QueueHandle<AppState>,
    ) {
        match result {
            InputResult::OpenPopup { module_id } => {
                self.create_popup_surface_for_panel(
                    output_name,
                    panel_idx,
                    &module_id,
                    qh,
                );
            }
            InputResult::ClosePopup => {
                // PopupState::close() already dropped the LayerSurface.
                // The connection flush in the main loop will send the destroy.
                debug!("Popup closed for panel {} on '{}'", panel_idx, output_name);
            }
            InputResult::Action(_) | InputResult::None => {}
        }
    }

    /// Find the (output_name, panel_index) pair that owns the given `wl_surface`.
    ///
    /// Searches panel layer surfaces only (not popup surfaces).  Used in
    /// [`PointerHandler::pointer_frame`] to route events to the correct panel.
    fn find_panel_for_surface(
        &self,
        surface: &wl_surface::WlSurface,
    ) -> Option<(String, usize)> {
        let target_id = surface.id();
        for (output_name, output) in &self.app.outputs {
            for (panel_idx, panel) in output.panels.iter().enumerate() {
                if let Some(layer) = &panel.layer_surface {
                    if layer.wl_surface().id() == target_id {
                        return Some((output_name.clone(), panel_idx));
                    }
                }
            }
        }
        None
    }

    /// Create Wayland layer surfaces for all panels of a newly-added output.
    ///
    /// Panels that already have a surface are skipped.  A `SlotPool` is
    /// allocated for each new surface.  The initial empty commit is performed
    /// here to signal the compositor that the surface is ready, so it will
    /// send a `configure` event.
    fn create_surfaces_for_output(&mut self, name: &str, qh: &QueueHandle<AppState>) {
        // Collect the info we need without holding a borrow on app.outputs.
        let Some(output) = self.app.outputs.get(name) else {
            return;
        };
        // Clone the WlOutput handle so we don't hold a borrow on self.wl_outputs
        // while also borrowing other fields.
        let wl_output = self.wl_outputs.get(name).cloned();

        let panel_info: Vec<(usize, Edge, u32, u32)> = output
            .panels
            .iter()
            .enumerate()
            .filter(|(_, p)| p.layer_surface.is_none())
            .map(|(i, p)| (i, p.edge, p.surface_width, p.surface_height))
            .collect();

        // All borrows of self.app.outputs end here (panel_info owns Copy data).

        if panel_info.is_empty() {
            return;
        }

        // Create surfaces (uses self.compositor_state / self.layer_shell / self.shm).
        let mut new_surfaces: Vec<(LayerSurface, SlotPool)> = Vec::new();
        for &(_, edge, w, h) in &panel_info {
            let surface = self.compositor_state.create_surface(qh);

            let layer = self.layer_shell.create_layer_surface(
                qh,
                surface,
                Layer::Top,
                Some("hyprdeck"),
                wl_output.as_ref(),
            );

            // Anchor to the appropriate screen edge; span the full perpendicular axis.
            let anchor = match edge {
                Edge::Top => Anchor::TOP | Anchor::LEFT | Anchor::RIGHT,
                Edge::Bottom => Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
                Edge::Left => Anchor::LEFT | Anchor::TOP | Anchor::BOTTOM,
                Edge::Right => Anchor::RIGHT | Anchor::TOP | Anchor::BOTTOM,
            };
            layer.set_anchor(anchor);

            // For horizontal panels set width=0 (fill available) and a fixed
            // height.  For vertical panels set a fixed width and height=0.
            let (set_w, set_h) = match edge {
                Edge::Top | Edge::Bottom => (0, h),
                Edge::Left | Edge::Right => (w, 0),
            };
            layer.set_size(set_w, set_h);

            let excl = match edge {
                Edge::Top | Edge::Bottom => h as i32,
                Edge::Left | Edge::Right => w as i32,
            };
            layer.set_exclusive_zone(excl);

            // CRITICAL: initial empty commit — tells the compositor the surface
            // is ready and triggers the configure event.
            layer.commit();

            info!("Created layer surface for {} edge {:?}", name, edge);
            debug!("Initial empty commit for surface");

            let pool_size = ((w * h * 4) as usize).max(4096);
            let pool = SlotPool::new(pool_size, &self.shm).expect("Failed to allocate shm pool");

            new_surfaces.push((layer, pool));
        }

        // Attach the new surfaces to their panels.
        let output = self.app.outputs.get_mut(name).unwrap();
        for ((idx, _, _, _), (layer, pool)) in panel_info.iter().zip(new_surfaces) {
            output.panels[*idx].layer_surface = Some(layer);
            output.panels[*idx].pool = Some(pool);
        }
    }
}

// ── SCTK delegate trait implementations ──────────────────────────────────────

impl SeatHandler for AppState {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, seat: wl_seat::WlSeat) {
        info!("SEAT New seat: {:?}", seat.id());
    }

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        info!("SEAT New capability: {:?}", capability);
        if capability == Capability::Pointer && self.pointer.is_none() {
            match self.seat_state.get_pointer(qh, &seat) {
                Ok(ptr) => {
                    info!("SEAT Created pointer {:?}", ptr.id());
                    self.pointer = Some(ptr);
                }
                Err(e) => {
                    error!("SEAT Failed to create pointer: {}", e);
                }
            }
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        info!("SEAT Capability removed: {:?}", capability);
        if capability == Capability::Pointer {
            self.pointer = None;
        }
    }

    fn remove_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, seat: wl_seat::WlSeat) {
        info!("SEAT Seat removed: {:?}", seat.id());
        self.pointer = None;
    }
}

impl PointerHandler for AppState {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            match &event.kind {
                // ── Stage 1: trace every raw Wayland pointer event ────────
                PointerEventKind::Enter { .. } => {
                    info!(
                        "POINTER Enter surface {:?} at ({:.1}, {:.1})",
                        event.surface.id(),
                        event.position.0,
                        event.position.1
                    );
                    if let Some((output_name, panel_idx)) =
                        self.find_panel_for_surface(&event.surface)
                    {
                        let output = self.app.outputs.get_mut(&output_name).unwrap();
                        output.panels[panel_idx].on_cursor_enter();
                    } else {
                        debug!(
                            "POINTER Enter: surface {:?} not matched to any panel",
                            event.surface.id()
                        );
                    }
                }

                PointerEventKind::Leave { .. } => {
                    info!("POINTER Leave surface {:?}", event.surface.id());
                    if let Some((output_name, panel_idx)) =
                        self.find_panel_for_surface(&event.surface)
                    {
                        let output = self.app.outputs.get_mut(&output_name).unwrap();
                        output.panels[panel_idx].on_cursor_leave();
                    }
                }

                PointerEventKind::Motion { .. } => {
                    trace!(
                        "POINTER Motion ({:.1}, {:.1})",
                        event.position.0,
                        event.position.1
                    );
                    if let Some((output_name, panel_idx)) =
                        self.find_panel_for_surface(&event.surface)
                    {
                        let input = InputEvent::MouseMove {
                            x: event.position.0 as f32,
                            y: event.position.1 as f32,
                        };
                        let output = self.app.outputs.get_mut(&output_name).unwrap();
                        let _ = output.panels[panel_idx].handle_input(input);
                    }
                }

                PointerEventKind::Press { button, .. } => {
                    info!(
                        "POINTER Press button={:#x} at ({:.1}, {:.1})",
                        button,
                        event.position.0,
                        event.position.1
                    );
                    let mb = match *button {
                        BTN_LEFT => Some(MouseButton::Left),
                        BTN_RIGHT => Some(MouseButton::Right),
                        BTN_MIDDLE => Some(MouseButton::Middle),
                        other => {
                            debug!("POINTER Press unknown button={:#x}, ignoring", other);
                            None
                        }
                    };
                    if let Some(mb) = mb {
                        // ── Stage 2: route to panel ───────────────────────
                        if let Some((output_name, panel_idx)) =
                            self.find_panel_for_surface(&event.surface)
                        {
                            let input = InputEvent::MousePress {
                                x: event.position.0 as f32,
                                y: event.position.1 as f32,
                                button: mb,
                            };
                            info!(
                                "Routing click to panel on output '{}', panel_idx={}",
                                output_name, panel_idx
                            );
                            let result = {
                                let output =
                                    self.app.outputs.get_mut(&output_name).unwrap();
                                let r = output.panels[panel_idx].handle_input(input);
                                info!("handle_input returned: {:?}", r);
                                r
                            };
                            self.handle_input_result(result, &output_name, panel_idx, qh);
                        } else {
                            warn!(
                                "POINTER Press: surface {:?} not matched to any panel",
                                event.surface.id()
                            );
                        }
                    }
                }

                PointerEventKind::Release { button, .. } => {
                    info!(
                        "POINTER Release button={:#x} at ({:.1}, {:.1})",
                        button,
                        event.position.0,
                        event.position.1
                    );
                    let mb = match *button {
                        BTN_LEFT => Some(MouseButton::Left),
                        BTN_RIGHT => Some(MouseButton::Right),
                        BTN_MIDDLE => Some(MouseButton::Middle),
                        _ => None,
                    };
                    if let Some(mb) = mb {
                        if let Some((output_name, panel_idx)) =
                            self.find_panel_for_surface(&event.surface)
                        {
                            let input = InputEvent::MouseRelease {
                                x: event.position.0 as f32,
                                y: event.position.1 as f32,
                                button: mb,
                            };
                            let output = self.app.outputs.get_mut(&output_name).unwrap();
                            let _ = output.panels[panel_idx].handle_input(input);
                        }
                    }
                }

                PointerEventKind::Axis { .. } => {
                    debug!("POINTER Scroll");
                }
            }
        }
    }
}

impl CompositorHandler for AppState {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for AppState {
    fn output_state(&mut self) -> &mut SctkOutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        if let Some(info) = self.output_state.info(&output) {
            if let Some(name) = &info.name {
                debug!("Wayland output advertised: {}", name);
                self.wl_outputs.insert(name.clone(), output);
            }
        }
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        // Update the name→WlOutput map when output properties change.
        if let Some(info) = self.output_state.info(&output) {
            if let Some(name) = &info.name {
                self.wl_outputs.entry(name.clone()).or_insert(output);
            }
        }
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        self.wl_outputs.retain(|_, v| v != &output);
    }
}

impl LayerShellHandler for AppState {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, layer: &LayerSurface) {
        let surface_id = layer.wl_surface().id();
        info!("Layer surface closed ({:?})", surface_id);

        for output in self.app.outputs.values_mut() {
            // Check whether this is a panel surface being closed.
            let was_panel = output.panels.iter().any(|p| {
                p.layer_surface
                    .as_ref()
                    .is_some_and(|l| l.wl_surface().id() == surface_id)
            });
            if was_panel {
                output.panels.retain(|p| {
                    p.layer_surface
                        .as_ref()
                        .is_none_or(|l| l.wl_surface().id() != surface_id)
                });
                continue;
            }

            // Otherwise check whether it is a popup surface being closed by
            // the compositor (e.g. the user dismissed it externally).
            for panel in &mut output.panels {
                if panel.popup.surface_id() == Some(surface_id.clone()) {
                    info!("Popup surface closed by compositor for module {:?}", panel.popup.active_module);
                    panel.popup.close();
                    break;
                }
            }
        }
    }

    /// Called by SCTK **after** it has already sent `ack_configure`.
    ///
    /// Handles both panel surfaces and popup overlay surfaces. For panel
    /// surfaces: resize canvas if needed, mark dirty, render immediately.
    /// For popup surfaces: mark popup dirty and render the first frame.
    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        let surface_id = layer.wl_surface().id();
        debug!("Layer configure event for surface {:?}, size {:?}", surface_id, configure.new_size);

        // Search for either a panel surface or a popup surface matching this id.
        let mut panel_target: Option<(String, usize)> = None;
        let mut popup_target: Option<(String, usize)> = None;
        'outer: for (name, output) in &self.app.outputs {
            for (i, panel) in output.panels.iter().enumerate() {
                if panel
                    .layer_surface
                    .as_ref()
                    .is_some_and(|l| l.wl_surface().id() == surface_id)
                {
                    panel_target = Some((name.clone(), i));
                    break 'outer;
                }
                if panel.popup.surface_id() == Some(surface_id.clone()) {
                    popup_target = Some((name.clone(), i));
                    break 'outer;
                }
            }
        }

        // ── Popup configure ───────────────────────────────────────────────────
        if let Some((output_name, panel_idx)) = popup_target {
            let (w, h) = configure.new_size;
            info!("Popup configure: {}x{} for panel {} on '{}'", w, h, panel_idx, output_name);

            let output = self.app.outputs.get_mut(&output_name).unwrap();
            let panel = &mut output.panels[panel_idx];

            // If the compositor assigned different dimensions than we requested,
            // update the popup canvas.
            if w != 0 && h != 0 {
                let pw = panel.popup.width;
                let ph = panel.popup.height;
                if w != pw || h != ph {
                    panel.popup.width = w;
                    panel.popup.height = h;
                    if let Some(canvas) = &mut panel.popup.canvas {
                        canvas.resize(w, h);
                    }
                    if let Some(pool) = &mut panel.popup.pool {
                        let needed = (w * h * 4) as usize;
                        if needed > pool.len() {
                            if let Err(e) = pool.resize(needed) {
                                error!("Failed to resize popup shm pool: {}", e);
                            }
                        }
                    }
                }
            }

            panel.popup.dirty = true;
            debug!("Rendering popup on configure");
            // Borrow panel.theme_ctx and panel.popup separately (different fields).
            panel.popup.frame(&panel.theme_ctx);
            return;
        }

        // ── Panel configure ───────────────────────────────────────────────────
        let Some((output_name, panel_idx)) = panel_target else {
            warn!("Configure for UNMATCHED surface {:?}", surface_id);
            return;
        };

        let output = self.app.outputs.get_mut(&output_name).unwrap();
        let display = output.display_geometry.clone();
        let panel = &mut output.panels[panel_idx];

        // Compositor may send (0, 0) meaning "you choose".  Fall back to the
        // panel's desired dimensions.
        let (w, h) = configure.new_size;
        let w = if w == 0 { panel.surface_width } else { w };
        let h = if h == 0 { panel.surface_height } else { h };

        info!("Configure: {}x{}", w, h);

        if w != panel.surface_width || h != panel.surface_height {
            panel.surface_width = w;
            panel.surface_height = h;
            panel.canvas.resize(w, h);

            // Grow the SHM pool if needed.
            if let Some(pool) = &mut panel.pool {
                let needed = (w * h * 4) as usize;
                if needed > pool.len() {
                    if let Err(e) = pool.resize(needed) {
                        error!("Failed to resize shm pool: {}", e);
                    }
                }
            }
        }

        panel.dirty = true;
        debug!("Rendering panel on configure");
        panel.frame(&display);
    }
}

impl ShmHandler for AppState {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for AppState {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers![SctkOutputState, SeatState];
}

delegate_compositor!(AppState);
delegate_output!(AppState);
delegate_layer!(AppState);
delegate_shm!(AppState);
delegate_seat!(AppState);
delegate_pointer!(AppState);
delegate_registry!(AppState);

// ── Wayland fd helper ─────────────────────────────────────────────────────────

/// Wraps a raw Wayland fd for use with `tokio::io::unix::AsyncFd` without
/// taking ownership of (and thus closing) the fd.
///
/// # Safety
/// The caller must ensure the underlying `Connection` outlives this wrapper.
struct WaylandFdRef(RawFd);

impl std::os::unix::io::AsRawFd for WaylandFdRef {
    fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}

// ── Main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // ── 1. Initialize logging ─────────────────────────────
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new("hyprdeck=info,hyprdeck_core=info")
            }),
        )
        .init();

    info!("HyprDeck starting");

    // ── 2. Load config ────────────────────────────────────
    let config_path = user_config_path();
    info!("Loading config from {:?}", config_path);
    let config = hyprdeck_core::Config::load(&config_path)?;
    info!("Theme: {}", config.theme);

    // ── 3. Resolve theme ──────────────────────────────────
    let theme_def = hyprdeck_themes::load_theme(&config.theme)?;
    info!(
        "Loaded theme '{}' with {} panel(s)",
        theme_def.name,
        theme_def.panels.len(),
    );

    // ── 4. Connect to Hyprland IPC ────────────────────────
    let ipc = HyprIpc::connect().await?;
    let hypr_state = ipc.state();
    let mut hypr_rx = ipc.subscribe();
    let _hypr_socket = ipc.command().socket_path().to_path_buf();
    info!("Connected to Hyprland IPC");

    // ── 5. Build App state (no outputs yet) ───────────────
    let app = App::new(config, theme_def, hyprdeck_modules::create_module);

    // ── 6. Connect to Wayland and set up SCTK ────────────
    let conn = Connection::connect_to_env().expect("Could not connect to Wayland display");
    info!("Connected to Wayland display");

    let (globals, mut event_queue): (_, EventQueue<AppState>) = registry_queue_init(&conn)?;
    let qh = event_queue.handle();

    let compositor_state = CompositorState::bind(&globals, &qh)?;
    let output_state = SctkOutputState::new(&globals, &qh);
    let shm = Shm::bind(&globals, &qh)?;
    let layer_shell = LayerShell::bind(&globals, &qh)?;
    let registry_state = RegistryState::new(&globals);
    let seat_state = SeatState::new(&globals, &qh);

    let mut app_state = AppState {
        app,
        registry_state,
        compositor_state,
        output_state,
        shm,
        layer_shell,
        seat_state,
        wl_outputs: HashMap::new(),
        pointer: None,
    };

    // Initial roundtrip: enumerates Wayland outputs (populates wl_outputs).
    event_queue.roundtrip(&mut app_state)?;

    // ── 7. Add outputs and create Wayland surfaces ────────
    {
        let state = hypr_state.read().await;
        for monitor in &state.monitors {
            app_state
                .app
                .add_output(monitor.name.clone(), monitor.width, monitor.height);
            app_state.create_surfaces_for_output(&monitor.name, &qh);
        }
    }
    debug!("Flushed Wayland connection");
    event_queue.flush()?;

    // Second roundtrip: compositor processes surface creation and sends
    // configure events; LayerShellHandler::configure renders the first frame.
    event_queue.roundtrip(&mut app_state)?;
    // Flush the buffer-attach and commit requests produced during configure.
    event_queue.flush()?;

    // ── 8. Initial module update ──────────────────────────
    {
        let now = chrono::Local::now();
        let state = hypr_state.read().await;
        app_state.app.tick_modules(now, &state);
    }
    // Render any panels dirtied by the module tick.
    app_state.app.render_dirty();
    event_queue.flush()?;

    // ── 9. Signal handling ────────────────────────────────
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;

    // ── 10. Async Wayland fd ──────────────────────────────
    //
    // Borrow the raw fd from the Connection for use with tokio's reactor.
    // Safety: `conn` is declared above and outlives `wayland_async_fd`.
    let wayland_raw_fd: RawFd = conn.as_fd().as_raw_fd();
    let wayland_async_fd = tokio::io::unix::AsyncFd::new(WaylandFdRef(wayland_raw_fd))?;

    // ── 11. Main event loop ───────────────────────────────
    //
    // Four concurrent event sources:
    //   a) Wayland compositor events (configure, close, frame callbacks)
    //   b) Hyprland IPC events (workspace changes, window focus, hotplug)
    //   c) Module update tick (250 ms — safety net for polling modules such as
    //      clock, weather, network; IPC events trigger an immediate update too)
    //   d) Animation frame (16 ms / 60 fps, only while animating)

    let mut tick_interval = tokio::time::interval(Duration::from_millis(250));
    tick_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    let mut frame_interval = tokio::time::interval(Duration::from_millis(16));
    frame_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    let mut last_frame_time = tokio::time::Instant::now();
    let mut animating = app_state.app.is_animating();

    info!("Entering main event loop");

    loop {
        tokio::select! {
            // Signal handling
            _ = sigterm.recv() => {
                info!("Received SIGTERM, shutting down");
                app_state.app.shutdown = true;
            }
            _ = sigint.recv() => {
                info!("Received SIGINT, shutting down");
                app_state.app.shutdown = true;
            }

            // Wayland compositor events
            result = wayland_async_fd.readable() => {
                let mut guard = result?;
                guard.clear_ready();

                // Read incoming bytes from the Wayland socket.
                // Errors here (including WouldBlock) are tolerated; any real
                // protocol error will surface through dispatch_pending below.
                if let Some(read_guard) = event_queue.prepare_read() {
                    let _ = read_guard.read();
                }
                event_queue.dispatch_pending(&mut app_state)?;
                trace!("Dispatched Wayland events");
            }

            // Hyprland IPC events
            event = hypr_rx.recv() => {
                match event {
                    Ok(ev) => {
                        trace!("Hyprland event: {:?}", ev);

                        // Handle monitor hotplug.
                        match &ev {
                            hyprdeck_core::HyprEvent::MonitorAdded { name, .. } => {
                                let state = hypr_state.read().await;
                                if let Some(mon) = state.monitors.iter().find(|m| &m.name == name) {
                                    app_state.app.add_output(
                                        mon.name.clone(),
                                        mon.width,
                                        mon.height,
                                    );
                                    app_state.create_surfaces_for_output(&mon.name, &qh);
                                    // Flush so the compositor receives the new surfaces.
                                    event_queue.flush()?;
                                }
                            }
                            hyprdeck_core::HyprEvent::MonitorRemoved { name } => {
                                app_state.app.remove_output(name);
                            }
                            _ => {}
                        }

                        // HyprState was already updated by the socket reader task
                        // (state update happens before broadcast). Run an immediate
                        // module tick so panels reflect the change without waiting
                        // for the periodic fallback interval.
                        {
                            let now = chrono::Local::now();
                            let state = hypr_state.read().await;
                            app_state.app.tick_modules(now, &state);
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!("Hyprland event receiver lagged by {} events", n);
                        // Mark all panels dirty so they re-read current state on
                        // the next tick rather than rendering stale content.
                        for output in app_state.app.outputs.values_mut() {
                            for panel in &mut output.panels {
                                panel.dirty = true;
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        error!("Hyprland IPC channel closed");
                        app_state.app.shutdown = true;
                    }
                }
            }

            // Module update tick (250 ms fallback)
            _ = tick_interval.tick() => {
                let now = chrono::Local::now();
                let state = hypr_state.read().await;
                app_state.app.tick_modules(now, &state);
            }

            // Animation frame (16 ms, only while animating)
            _ = frame_interval.tick(), if animating => {
                let now = tokio::time::Instant::now();
                let dt = (now - last_frame_time).as_secs_f32();
                last_frame_time = now;
                trace!("Animation frame dt={:.4}s", dt);
                app_state.app.tick_animations(dt);
            }
        }

        // After any event, render dirty panels and flush submissions.
        app_state.app.render_dirty();
        event_queue.flush()?;

        // Update animating flag.
        animating = app_state.app.is_animating();
        if !animating {
            last_frame_time = tokio::time::Instant::now();
        }

        if app_state.app.shutdown {
            break;
        }
    }

    info!("HyprDeck shutting down");
    Ok(())
}

/// Return the path to `~/.config/hypr/hyprdeck.toml`.
fn user_config_path() -> std::path::PathBuf {
    let config_dir = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").unwrap_or_default();
            std::path::Path::new(&home).join(".config")
        });
    config_dir.join("hypr").join("hyprdeck.toml")
}
