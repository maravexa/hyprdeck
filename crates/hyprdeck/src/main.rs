use std::collections::{HashMap, HashSet};
use std::os::unix::io::{AsFd, AsRawFd, RawFd};
use std::time::{Duration, Instant};

use clap::Parser;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat, delegate_shm,
    output::{OutputHandler, OutputState as SctkOutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
    seat::pointer::{
        BTN_LEFT, BTN_MIDDLE, BTN_RIGHT, PointerEvent, PointerEventKind, PointerHandler,
    },
    seat::{Capability, SeatHandler, SeatState},
    shell::{
        WaylandSurface,
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
    },
    shm::{Shm, ShmHandler, slot::SlotPool},
};
use tracing::{debug, error, info, trace, warn};
use wayland_client::{
    Connection, EventQueue, Proxy, QueueHandle,
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
};

mod instance;
mod notifications;

use hyprdeck_core::ipc::HyprIpc;
use hyprdeck_core::{
    App, Edge, InputEvent, InputResult, MouseButton, NOTIFICATION_HEIGHT, Notification,
    NotificationCenter, PopupEventResult, Rect, dispatch_action, keymod, notification_placement,
    render_notification,
};
use tokio::sync::mpsc;

use crate::instance::{InstanceClaim, InstanceControl};
use crate::notifications::NotificationCommand;

const POPUP_LEAVE_GRACE: Duration = Duration::from_millis(300);

#[derive(Default)]
struct PopupCloseTracker {
    pending: HashMap<(String, usize), Instant>,
}

impl PopupCloseTracker {
    fn schedule(&mut self, output_name: &str, panel_idx: usize, now: Instant) {
        self.pending
            .insert((output_name.to_owned(), panel_idx), now + POPUP_LEAVE_GRACE);
    }

    fn cancel(&mut self, output_name: &str, panel_idx: usize) {
        self.pending.remove(&(output_name.to_owned(), panel_idx));
    }

    fn take_expired(&mut self, now: Instant) -> Vec<(String, usize)> {
        let expired = self
            .pending
            .iter()
            .filter(|(_, deadline)| **deadline <= now)
            .map(|(owner, _)| owner.clone())
            .collect::<Vec<_>>();
        for owner in &expired {
            self.pending.remove(owner);
        }
        expired
    }

    fn clear(&mut self) {
        self.pending.clear();
    }
}

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
    /// The active Wayland keyboard object.  `None` until a seat with keyboard
    /// capability is seen.
    keyboard: Option<wl_keyboard::WlKeyboard>,
    /// Surface that currently holds keyboard focus (a panel or popup surface).
    /// `None` while no HyprDeck surface is focused.
    keyboard_focus_surface: Option<wl_surface::WlSurface>,
    /// Current modifier state as [`keymod`] bitflags, updated from
    /// `update_modifiers` and attached to every dispatched key press.
    modifiers: u32,
    /// Path to the Hyprland command socket, used when dispatching actions from popup events.
    hypr_socket: std::path::PathBuf,
    /// Notifications received by the D-Bus service. Surfaces are owned here
    /// because the Wayland queue must be manipulated on this event-loop task.
    notification_center: NotificationCenter,
    notification_surfaces: HashMap<u32, NotificationSurface>,
    notification_output: Option<String>,
    /// Popups scheduled to close after the pointer leaves their surface.
    /// A short grace period lets the pointer cross panel/popup surface seams.
    popup_close: PopupCloseTracker,
}

/// One configured notification overlay. It intentionally does not reuse the
/// module-popup state: notifications have independent lifetime and stacking.
struct NotificationSurface {
    notification: Notification,
    output_name: String,
    layer_surface: LayerSurface,
    canvas: hyprdeck_core::Canvas,
    pool: SlotPool,
    width: u32,
    height: u32,
    configured: bool,
}

impl NotificationSurface {
    fn render(&mut self, theme: &hyprdeck_core::ThemeContext) {
        if !self.configured {
            return;
        }
        render_notification(&mut self.canvas, &self.notification, theme);
        let stride = self.width as i32 * 4;
        let (buffer, data) = match self.pool.create_buffer(
            self.width as i32,
            self.height as i32,
            stride,
            wl_shm::Format::Argb8888,
        ) {
            Ok(pair) => pair,
            Err(error) => {
                error!(?error, "failed to allocate notification buffer");
                return;
            }
        };
        let source = self.canvas.data();
        for index in 0..(data.len().min(source.len()) / 4) {
            data[index * 4] = source[index * 4 + 2];
            data[index * 4 + 1] = source[index * 4 + 1];
            data[index * 4 + 2] = source[index * 4];
            data[index * 4 + 3] = source[index * 4 + 3];
        }
        let surface = self.layer_surface.wl_surface();
        if let Err(error) = buffer.attach_to(surface) {
            error!(?error, "failed to attach notification buffer");
            return;
        }
        surface.damage_buffer(0, 0, self.width as i32, self.height as i32);
        surface.commit();
    }
}

/// Pack SCTK keyboard modifiers into [`keymod`] bitflags.
///
/// Lock states (caps/num lock) are deliberately not represented.
fn pack_modifiers(m: &Modifiers) -> u32 {
    let mut bits = 0;
    if m.shift {
        bits |= keymod::SHIFT;
    }
    if m.ctrl {
        bits |= keymod::CTRL;
    }
    if m.alt {
        bits |= keymod::ALT;
    }
    if m.logo {
        bits |= keymod::LOGO;
    }
    bits
}

impl AppState {
    fn reload_bar(
        &mut self,
        config_path: &std::path::Path,
        state: &hyprdeck_core::HyprState,
        qh: &QueueHandle<AppState>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let config = hyprdeck_core::Config::load(config_path)?;
        let theme = hyprdeck_themes::load_theme(&config.theme)?;
        info!(theme = %theme.name, "refreshing existing HyprDeck instance");

        self.close_all_popups();
        self.popup_close.clear();
        self.notification_surfaces.clear();
        self.notification_output = None;
        self.app.reload(config, theme);
        self.reconcile_output_topology(state, qh);
        self.app.tick_modules(chrono::Local::now(), state);
        self.sync_notification_surfaces(state, qh, true);
        Ok(())
    }

    /// Reconcile binary-owned Wayland panels with the latest authoritative
    /// Hyprland monitor snapshot.
    ///
    /// Socket monitor-add events contain no dimensions, and topology can
    /// change while the event socket reconnects. Running this against the
    /// hydrated state avoids creating 0x0 panels and repairs missed add/remove
    /// events once both Hyprland and Wayland advertise the output.
    fn reconcile_output_topology(
        &mut self,
        state: &hyprdeck_core::HyprState,
        qh: &QueueHandle<AppState>,
    ) -> bool {
        let desired_names: HashSet<&str> = state
            .monitors
            .iter()
            .map(|monitor| monitor.name.as_str())
            .collect();
        let stale = self
            .app
            .outputs
            .keys()
            .filter(|name| !desired_names.contains(name.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        let mut changed = !stale.is_empty();
        for name in stale {
            self.app.remove_output(&name);
        }

        let available = state
            .monitors
            .iter()
            .filter(|monitor| {
                monitor.width > 0
                    && monitor.height > 0
                    && self.wl_outputs.contains_key(&monitor.name)
            })
            .map(|monitor| (monitor.name.clone(), monitor.width, monitor.height))
            .collect::<Vec<_>>();
        for (name, width, height) in available {
            let needs_rebuild = self
                .app
                .outputs
                .get(&name)
                .is_none_or(|output| output.width != width || output.height != height);
            if needs_rebuild {
                self.app.remove_output(&name);
                self.app.add_output(name.clone(), width, height);
                self.create_surfaces_for_output(&name, qh);
                changed = true;
            }
        }
        changed
    }

    fn process_notification(&mut self, command: NotificationCommand) -> bool {
        let config = &self.app.config.notifications;
        match command {
            NotificationCommand::Notify(request) => {
                let change =
                    self.notification_center
                        .notify(request, config, std::time::Instant::now());
                debug!(?change, "notification queue updated");
                true
            }
            NotificationCommand::Close(id) => self.notification_center.close(id).is_some(),
        }
    }

    fn expire_notifications(&mut self) -> bool {
        let expired = self.notification_center.expire(std::time::Instant::now());
        if !expired.is_empty() {
            debug!(?expired, "expired notifications removed");
        }
        !expired.is_empty()
    }

    /// Resolve `focused`, `primary`, or an explicitly named target output.
    fn notification_target(&self, state: &hyprdeck_core::HyprState) -> Option<String> {
        let selected = self.app.config.notifications.monitor.as_str();
        let target = match selected {
            "focused" => state.focused_monitor.as_str(),
            // Hyprland does not expose an independent primary-output flag in
            // its monitor JSON. Its first monitor is the stable primary
            // fallback used by HyprDeck's daemon.
            "primary" => state
                .monitors
                .first()
                .map(|monitor| monitor.name.as_str())?,
            output => output,
        };
        self.app
            .outputs
            .contains_key(target)
            .then(|| target.to_owned())
    }

    fn sync_notification_surfaces(
        &mut self,
        state: &hyprdeck_core::HyprState,
        qh: &QueueHandle<AppState>,
        force: bool,
    ) {
        let target = self.notification_target(state);
        if !force && target == self.notification_output {
            return;
        }

        self.notification_surfaces.clear();
        self.notification_output = target.clone();
        let Some(output_name) = target else {
            return;
        };
        let Some(output) = self.app.outputs.get(&output_name) else {
            return;
        };
        let wl_output = self.wl_outputs.get(&output_name).cloned();
        let output_width = output.width;
        let config = self.app.config.notifications.clone();
        let notifications: Vec<Notification> = self
            .notification_center
            .visible(config.max_visible)
            .cloned()
            .collect();

        for (index, notification) in notifications.into_iter().enumerate() {
            let placement =
                notification_placement(&config, output_width, index, NOTIFICATION_HEIGHT);
            let surface = self.compositor_state.create_surface(qh);
            let layer = self.layer_shell.create_layer_surface(
                qh,
                surface,
                Layer::Overlay,
                Some("hyprdeck-notification"),
                wl_output.as_ref(),
            );
            let anchor = match placement.anchor {
                hyprdeck_core::NotificationAnchor::TopLeft
                | hyprdeck_core::NotificationAnchor::TopCenter => Anchor::TOP | Anchor::LEFT,
                hyprdeck_core::NotificationAnchor::TopRight => Anchor::TOP | Anchor::RIGHT,
                hyprdeck_core::NotificationAnchor::BottomLeft
                | hyprdeck_core::NotificationAnchor::BottomCenter => Anchor::BOTTOM | Anchor::LEFT,
                hyprdeck_core::NotificationAnchor::BottomRight => Anchor::BOTTOM | Anchor::RIGHT,
            };
            layer.set_anchor(anchor);
            layer.set_size(config.width, NOTIFICATION_HEIGHT);
            layer.set_exclusive_zone(-1);
            layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            layer.set_margin(
                placement.top,
                placement.right,
                placement.bottom,
                placement.left,
            );
            layer.commit();
            let pool_size = (config.width as usize * NOTIFICATION_HEIGHT as usize * 4).max(4096);
            let pool = match SlotPool::new(pool_size, &self.shm) {
                Ok(pool) => pool,
                Err(error) => {
                    error!(?error, "failed to allocate notification shared memory");
                    continue;
                }
            };
            self.notification_surfaces.insert(
                notification.id,
                NotificationSurface {
                    notification,
                    output_name: output_name.clone(),
                    layer_surface: layer,
                    canvas: hyprdeck_core::Canvas::new(config.width, NOTIFICATION_HEIGHT),
                    pool,
                    width: config.width,
                    height: NOTIFICATION_HEIGHT,
                    configured: false,
                },
            );
        }
    }

    fn render_notification_surface(&mut self, id: u32) {
        let Some(output_name) = self
            .notification_surfaces
            .get(&id)
            .map(|surface| surface.output_name.clone())
        else {
            return;
        };
        let Some(theme) = self
            .app
            .outputs
            .get(&output_name)
            .and_then(|output| output.panels.first())
            .map(|panel| &panel.theme_ctx)
        else {
            return;
        };
        if let Some(surface) = self.notification_surfaces.get_mut(&id) {
            surface.render(theme);
        }
    }

    /// Create a Wayland `Overlay`-layer surface for a panel's popup.
    ///
    /// Called after `Panel::handle_input` returns [`InputResult::OpenPopup`].
    /// Reads the desired size from `panel.popup.content`, creates the surface,
    /// positions it adjacent to the panel edge, performs the initial empty commit
    /// to trigger a `configure`, and calls `Panel::attach_popup_surface` to hand
    /// over ownership.
    ///
    fn create_popup_surface_for_panel(
        &mut self,
        output_name: &str,
        panel_idx: usize,
        module_id: &str,
        module_bounds: Rect,
        qh: &QueueHandle<AppState>,
    ) {
        // ── Read desired size from popup content ──────────────────────────────
        let (width, height, edge, panel_w, panel_h, output_w, output_h) = {
            let Some(output) = self.app.outputs.get(output_name) else {
                return;
            };
            let panel = &output.panels[panel_idx];
            let Some(content) = &panel.popup.content else {
                warn!(
                    "open_popup called but popup.content is None for '{}'",
                    module_id
                );
                return;
            };
            let size = content.desired_size(&panel.theme_ctx);
            let (max_width, max_height) = match panel.edge {
                Edge::Top | Edge::Bottom => (
                    output.width.saturating_sub(8).max(1),
                    output
                        .height
                        .saturating_sub(panel.surface_height + 8)
                        .max(1),
                ),
                Edge::Left | Edge::Right => (
                    output.width.saturating_sub(panel.surface_width + 8).max(1),
                    output.height.saturating_sub(8).max(1),
                ),
            };
            let w = (size.width.ceil() as u32).clamp(1, max_width);
            let h = (size.height.ceil() as u32).clamp(1, max_height);
            (
                w,
                h,
                panel.edge,
                panel.surface_width,
                panel.surface_height,
                output.width,
                output.height,
            )
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
        // Use -1 so this surface is NOT repositioned to accommodate other surfaces'
        // exclusive zones (e.g. the panel's own reserved strip).  With zone=0 the
        // compositor would add the panel's exclusive-zone offset ON TOP of our margin,
        // placing the popup too far from the bar.  With zone=-1 our margins are always
        // measured from the raw output edge, which is what we want.
        layer.set_exclusive_zone(-1);

        // OnDemand keyboard focus so popups can take key input (e.g. Esc to close).
        layer.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);

        // Anchor and margin place the popup flush against the panel surface,
        // with the popup's cross-axis centre aligned on the triggering module.
        // Clamp to keep the popup fully on-screen (4px margin from output edge).
        const EDGE_MARGIN: f32 = 4.0;
        match edge {
            Edge::Top => {
                layer.set_anchor(Anchor::TOP | Anchor::LEFT);

                let module_center_x = module_bounds.x + module_bounds.width / 2.0;
                let mut popup_left = module_center_x - width as f32 / 2.0;
                let min_left = EDGE_MARGIN;
                let max_left = (output_w as f32) - width as f32 - EDGE_MARGIN;
                popup_left = popup_left.clamp(min_left, max_left.max(min_left));

                let margin_top = panel_h as i32;
                let margin_left = popup_left as i32;
                info!(
                    "Popup position: top bar, module_center_x={:.0}, popup_left={:.0}, margin_top={}, margin_left={}",
                    module_center_x, popup_left, margin_top, margin_left
                );
                layer.set_margin(margin_top, 0, 0, margin_left);
            }
            Edge::Bottom => {
                layer.set_anchor(Anchor::BOTTOM | Anchor::LEFT);

                let module_center_x = module_bounds.x + module_bounds.width / 2.0;
                let mut popup_left = module_center_x - width as f32 / 2.0;
                let min_left = EDGE_MARGIN;
                let max_left = (output_w as f32) - width as f32 - EDGE_MARGIN;
                popup_left = popup_left.clamp(min_left, max_left.max(min_left));

                let margin_bottom = panel_h as i32;
                let margin_left = popup_left as i32;
                info!(
                    "Popup position: bottom bar, popup_left={:.0}, margin_bottom={}, margin_left={}",
                    popup_left, margin_bottom, margin_left
                );
                layer.set_margin(0, 0, margin_bottom, margin_left);
            }
            Edge::Left => {
                layer.set_anchor(Anchor::LEFT | Anchor::TOP);

                let module_center_y = module_bounds.y + module_bounds.height / 2.0;
                let mut popup_top = module_center_y - height as f32 / 2.0;
                let min_top = EDGE_MARGIN;
                let max_top = (output_h as f32) - height as f32 - EDGE_MARGIN;
                popup_top = popup_top.clamp(min_top, max_top.max(min_top));

                let margin_left = panel_w as i32;
                let margin_top = popup_top as i32;
                info!(
                    "Popup position: left bar, popup_top={:.0}, margin_top={}, margin_left={}",
                    popup_top, margin_top, margin_left
                );
                layer.set_margin(margin_top, 0, 0, margin_left);
            }
            Edge::Right => {
                layer.set_anchor(Anchor::RIGHT | Anchor::TOP);

                let module_center_y = module_bounds.y + module_bounds.height / 2.0;
                let mut popup_top = module_center_y - height as f32 / 2.0;
                let min_top = EDGE_MARGIN;
                let max_top = (output_h as f32) - height as f32 - EDGE_MARGIN;
                popup_top = popup_top.clamp(min_top, max_top.max(min_top));

                let margin_right = panel_w as i32;
                let margin_top = popup_top as i32;
                info!(
                    "Popup position: right bar, popup_top={:.0}, margin_top={}, margin_right={}",
                    popup_top, margin_top, margin_right
                );
                layer.set_margin(margin_top, margin_right, 0, 0);
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
            InputResult::OpenPopup {
                module_id,
                module_bounds,
            } => {
                self.create_popup_surface_for_panel(
                    output_name,
                    panel_idx,
                    &module_id,
                    module_bounds,
                    qh,
                );
            }
            InputResult::ClosePopup => {
                // PopupState::close() already dropped the LayerSurface.
                // The connection flush in the main loop will send the destroy.
                debug!("Popup closed for panel {} on '{}'", panel_idx, output_name);
            }
            InputResult::Action(action) => {
                info!("Module action: {:?}", action);
                let hypr_socket = self.hypr_socket.clone();
                tokio::spawn(async move {
                    if let Err(e) = dispatch_action(&action, &hypr_socket).await {
                        warn!("Module action dispatch failed: {}", e);
                    }
                });
            }
            InputResult::None => {}
        }
    }

    /// Find the (output_name, panel_index) pair that owns the given `wl_surface`.
    ///
    /// Searches panel layer surfaces only (not popup surfaces).  Used in
    /// [`PointerHandler::pointer_frame`] to route events to the correct panel.
    fn find_panel_for_surface(&self, surface: &wl_surface::WlSurface) -> Option<(String, usize)> {
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

    /// Find the (output_name, panel_index) pair whose popup surface matches the given `wl_surface`.
    ///
    /// Used in [`PointerHandler::pointer_frame`] to route pointer events that land
    /// on a popup overlay surface to the correct panel's popup content.
    fn find_popup_owner(&self, surface: &wl_surface::WlSurface) -> Option<(String, usize)> {
        let target_id = surface.id();
        for (output_name, output) in &self.app.outputs {
            for (panel_idx, panel) in output.panels.iter().enumerate() {
                if panel.popup.surface_id() == Some(target_id.clone()) {
                    return Some((output_name.clone(), panel_idx));
                }
            }
        }
        None
    }

    /// Close every open popup across all outputs and panels.
    fn close_all_popups(&mut self) {
        self.popup_close.clear();
        for output in self.app.outputs.values_mut() {
            for panel in &mut output.panels {
                if panel.popup.active_module.is_some() {
                    panel.popup.close();
                }
            }
        }
    }

    fn close_expired_popups(&mut self) {
        for (output_name, panel_idx) in self.popup_close.take_expired(Instant::now()) {
            let Some(panel) = self
                .app
                .outputs
                .get_mut(&output_name)
                .and_then(|output| output.panels.get_mut(panel_idx))
            else {
                continue;
            };
            let dragging = panel
                .popup
                .content
                .as_ref()
                .is_some_and(|content| content.is_dragging());
            if !dragging {
                debug!(%output_name, panel_idx, "closing popup after pointer-leave grace period");
                panel.popup.close();
            }
        }
    }

    /// Dispatch an [`InputEvent`] to the active popup content of the specified panel.
    ///
    /// Constructs bounds from the popup's current pixel dimensions and forwards
    /// the event via `PopupState::handle_event`.  If the popup returns an
    /// [`PopupEventResult::Action`], the popup is closed and the action is
    /// dispatched in a background tokio task.
    fn dispatch_popup_event(&mut self, output_name: &str, panel_idx: usize, event: InputEvent) {
        let hypr_socket = self.hypr_socket.clone();

        let Some(output) = self.app.outputs.get_mut(output_name) else {
            return;
        };
        let Some(panel) = output.panels.get_mut(panel_idx) else {
            return;
        };

        let bounds = Rect::new(
            0.0,
            0.0,
            panel.popup.width as f32,
            panel.popup.height as f32,
        );

        let result = panel.popup.handle_event(&event, bounds);

        match result {
            Some(PopupEventResult::Action(action)) => {
                info!("Popup action: {:?}", action);
                panel.popup.close();
                tokio::spawn(async move {
                    if let Err(e) = dispatch_action(&action, &hypr_socket).await {
                        warn!("Popup action dispatch failed: {}", e);
                    }
                });
            }
            Some(PopupEventResult::Close) => panel.popup.close(),
            Some(PopupEventResult::Handled | PopupEventResult::Ignored) | None => {}
        }
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

            // OnDemand: the compositor grants keyboard focus when the user
            // clicks the panel and returns it to normal windows on click-away.
            layer.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);

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
            output.panels[*idx].surface_configured = false;
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
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            match self.seat_state.get_keyboard(qh, &seat, None) {
                Ok(kbd) => {
                    info!("SEAT Created keyboard {:?}", kbd.id());
                    self.keyboard = Some(kbd);
                }
                Err(e) => {
                    error!("SEAT Failed to create keyboard: {}", e);
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
        if capability == Capability::Keyboard {
            self.keyboard = None;
            self.keyboard_focus_surface = None;
            self.modifiers = 0;
        }
    }

    fn remove_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, seat: wl_seat::WlSeat) {
        info!("SEAT Seat removed: {:?}", seat.id());
        self.pointer = None;
        self.keyboard = None;
        self.keyboard_focus_surface = None;
        self.modifiers = 0;
    }
}

impl KeyboardHandler for AppState {
    fn enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _serial: u32,
        _raw: &[u32],
        _keysyms: &[Keysym],
    ) {
        debug!("KEYBOARD Enter surface {:?}", surface.id());
        self.keyboard_focus_surface = Some(surface.clone());
    }

    fn leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _serial: u32,
    ) {
        debug!("KEYBOARD Leave surface {:?}", surface.id());
        self.keyboard_focus_surface = None;
    }

    fn press_key(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        debug!("KEYBOARD Press keysym {:?}", event.keysym);
        let Some(surface) = self.keyboard_focus_surface.clone() else {
            return;
        };
        let input = InputEvent::KeyPress {
            key: event.keysym.raw(),
            modifiers: self.modifiers,
        };

        if let Some((output_name, panel_idx)) = self.find_popup_owner(&surface) {
            // Esc closes the focused popup; everything else goes to its content.
            if event.keysym == Keysym::Escape {
                if let Some(panel) = self
                    .app
                    .outputs
                    .get_mut(&output_name)
                    .and_then(|o| o.panels.get_mut(panel_idx))
                {
                    panel.popup.close();
                }
                return;
            }
            self.dispatch_popup_event(&output_name, panel_idx, input);
        } else if let Some((output_name, panel_idx)) = self.find_panel_for_surface(&surface) {
            let result = {
                let output = self.app.outputs.get_mut(&output_name).unwrap();
                output.panels[panel_idx].handle_input(input)
            };
            self.handle_input_result(result, &output_name, panel_idx, qh);
        }
    }

    fn release_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        _event: KeyEvent,
    ) {
        // Key releases are not delivered to modules (no KeyRelease variant).
    }

    fn repeat_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        _event: KeyEvent,
    ) {
        // Key repeat is not requested (plain get_keyboard, no repeat source).
    }

    fn update_modifiers(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        modifiers: Modifiers,
        _raw_modifiers: RawModifiers,
        _layout: u32,
    ) {
        self.modifiers = pack_modifiers(&modifiers);
        trace!("KEYBOARD Modifiers {:#06b}", self.modifiers);
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
                PointerEventKind::Enter { .. } => {
                    if let Some((output_name, panel_idx)) =
                        self.find_panel_for_surface(&event.surface)
                    {
                        self.popup_close.cancel(&output_name, panel_idx);
                        let output = self.app.outputs.get_mut(&output_name).unwrap();
                        output.panels[panel_idx].on_cursor_enter();
                    } else if let Some((output_name, panel_idx)) =
                        self.find_popup_owner(&event.surface)
                    {
                        self.popup_close.cancel(&output_name, panel_idx);
                    }
                }

                PointerEventKind::Leave { .. } => {
                    if let Some((output_name, panel_idx)) =
                        self.find_panel_for_surface(&event.surface)
                    {
                        let output = self.app.outputs.get_mut(&output_name).unwrap();
                        output.panels[panel_idx].on_cursor_leave();
                    } else if let Some((output_name, panel_idx)) =
                        self.find_popup_owner(&event.surface)
                    {
                        // Do not close immediately: separate Wayland surfaces can
                        // emit a brief leave while the pointer crosses their seam.
                        let is_dragging = self
                            .app
                            .outputs
                            .get(&output_name)
                            .and_then(|o| o.panels.get(panel_idx))
                            .and_then(|p| p.popup.content.as_ref())
                            .map(|c| c.is_dragging())
                            .unwrap_or(false);
                        if !is_dragging {
                            self.popup_close
                                .schedule(&output_name, panel_idx, Instant::now());
                        }
                    }
                }

                PointerEventKind::Motion { .. } => {
                    if let Some((output_name, panel_idx)) =
                        self.find_panel_for_surface(&event.surface)
                    {
                        let input = InputEvent::MouseMove {
                            x: event.position.0 as f32,
                            y: event.position.1 as f32,
                        };
                        let output = self.app.outputs.get_mut(&output_name).unwrap();
                        let _ = output.panels[panel_idx].handle_input(input);
                    } else if let Some((output_name, panel_idx)) =
                        self.find_popup_owner(&event.surface)
                    {
                        self.popup_close.cancel(&output_name, panel_idx);
                        let input = InputEvent::MouseMove {
                            x: event.position.0 as f32,
                            y: event.position.1 as f32,
                        };
                        self.dispatch_popup_event(&output_name, panel_idx, input);
                    }
                }

                PointerEventKind::Press { button, .. } => {
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
                        if let Some((output_name, panel_idx)) =
                            self.find_panel_for_surface(&event.surface)
                        {
                            let input = InputEvent::MousePress {
                                x: event.position.0 as f32,
                                y: event.position.1 as f32,
                                button: mb,
                            };
                            let result = {
                                let output = self.app.outputs.get_mut(&output_name).unwrap();
                                output.panels[panel_idx].handle_input(input)
                            };
                            // Close any popup that wasn't toggled by this click.
                            if matches!(result, InputResult::None | InputResult::Action(_)) {
                                self.close_all_popups();
                            }
                            self.handle_input_result(result, &output_name, panel_idx, qh);
                        } else if let Some((output_name, panel_idx)) =
                            self.find_popup_owner(&event.surface)
                        {
                            let input = InputEvent::MousePress {
                                x: event.position.0 as f32,
                                y: event.position.1 as f32,
                                button: mb,
                            };
                            self.dispatch_popup_event(&output_name, panel_idx, input);
                        } else {
                            // Click landed on the desktop or an unrelated surface.
                            self.close_all_popups();
                            warn!(
                                "POINTER Press: surface {:?} not matched to any panel",
                                event.surface.id()
                            );
                        }
                    }
                }

                PointerEventKind::Release { button, .. } => {
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
                        } else if let Some((output_name, panel_idx)) =
                            self.find_popup_owner(&event.surface)
                        {
                            let input = InputEvent::MouseRelease {
                                x: event.position.0 as f32,
                                y: event.position.1 as f32,
                                button: mb,
                            };
                            self.dispatch_popup_event(&output_name, panel_idx, input);
                        }
                    }
                }

                PointerEventKind::Axis {
                    horizontal,
                    vertical,
                    ..
                } => {
                    // SCTK already aggregates all axis protocol events in this
                    // pointer frame.  Prefer pixel deltas for touchpads, then
                    // high-resolution wheel units, then legacy discrete steps.
                    let axis_delta = |axis: &smithay_client_toolkit::seat::pointer::AxisScroll| {
                        if axis.absolute != 0.0 {
                            axis.absolute as f32
                        } else if axis.value120 != 0 {
                            axis.value120 as f32 / 120.0
                        } else {
                            axis.discrete as f32
                        }
                    };
                    let dx = axis_delta(horizontal);
                    let dy = axis_delta(vertical);
                    if dx == 0.0 && dy == 0.0 {
                        continue;
                    }
                    let input = InputEvent::Scroll { dx, dy };
                    if let Some((output_name, panel_idx)) =
                        self.find_panel_for_surface(&event.surface)
                    {
                        let result = {
                            let output = self.app.outputs.get_mut(&output_name).unwrap();
                            output.panels[panel_idx].handle_input(input)
                        };
                        self.handle_input_result(result, &output_name, panel_idx, qh);
                    } else if let Some((output_name, panel_idx)) =
                        self.find_popup_owner(&event.surface)
                    {
                        self.dispatch_popup_event(&output_name, panel_idx, input);
                    }
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

        if let Some(id) = self.notification_surfaces.iter().find_map(|(id, surface)| {
            (surface.layer_surface.wl_surface().id() == surface_id).then_some(*id)
        }) {
            self.notification_surfaces.remove(&id);
            return;
        }

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
                    info!(
                        "Popup surface closed by compositor for module {:?}",
                        panel.popup.active_module
                    );
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
        debug!(
            "Layer configure event for surface {:?}, size {:?}",
            surface_id, configure.new_size
        );

        if let Some(id) = self.notification_surfaces.iter().find_map(|(id, surface)| {
            (surface.layer_surface.wl_surface().id() == surface_id).then_some(*id)
        }) {
            let (width, height) = configure.new_size;
            if let Some(surface) = self.notification_surfaces.get_mut(&id) {
                if width != 0 && height != 0 && (width != surface.width || height != surface.height)
                {
                    surface.width = width;
                    surface.height = height;
                    surface.canvas.resize(width, height);
                    let needed = (width as usize * height as usize * 4).max(4096);
                    if needed > surface.pool.len()
                        && let Err(error) = surface.pool.resize(needed)
                    {
                        error!(?error, "failed to resize notification shared memory");
                    }
                }
                surface.configured = true;
            }
            self.render_notification_surface(id);
            return;
        }

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
            info!(
                "Popup configure {}x{} for panel {} on '{}'",
                w, h, panel_idx, output_name
            );
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

            panel.popup.configured = true;
            panel.popup.dirty = true;
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
        panel.surface_configured = true;
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
delegate_keyboard!(AppState);
delegate_registry!(AppState);

// ── CLI ───────────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "hyprdeck", version)]
struct Cli {
    /// Print the versioned built-in module configuration schema as JSON and exit.
    #[arg(long)]
    print_config_schema: bool,

    /// Validate a HyprDeck TOML file, print JSON diagnostics, and exit.
    #[arg(long, value_name = "PATH")]
    validate_config: Option<std::path::PathBuf>,
}

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

fn editor_config_schema() -> Result<hyprdeck_core::ConfigSchema, hyprdeck_themes::ThemeLoadError> {
    let themes = hyprdeck_themes::embedded_theme_names()
        .into_iter()
        .map(|id| {
            let theme = hyprdeck_themes::load_theme(id)?;
            let mut modules = Vec::new();
            for module in theme.panels.iter().flat_map(|panel| {
                panel
                    .modules_start
                    .iter()
                    .chain(&panel.modules_center)
                    .chain(&panel.modules_end)
            }) {
                if !modules.contains(module) {
                    modules.push(module.clone());
                }
            }
            Ok(hyprdeck_core::ThemeMetadata {
                id: id.to_owned(),
                name: theme.name,
                description: theme.description,
                modules,
            })
        })
        .collect::<Result<Vec<_>, hyprdeck_themes::ThemeLoadError>>()?;
    Ok(hyprdeck_modules::builtin_config_schema().with_themes(themes))
}

// ── Main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // ── 0. Parse CLI (handles --version / -V and exits early) ────────────────
    let cli = Cli::parse();
    if cli.print_config_schema {
        println!(
            "{}",
            serde_json::to_string_pretty(&editor_config_schema()?)?
        );
        return Ok(());
    }
    if let Some(path) = cli.validate_config {
        let config = hyprdeck_core::Config::load(&path)?;
        let diagnostics = config.validate_with_schema(&editor_config_schema()?);
        println!("{}", serde_json::to_string_pretty(&diagnostics)?);
        if diagnostics
            .iter()
            .any(hyprdeck_core::ConfigDiagnostic::is_error)
        {
            return Err(hyprdeck_core::ConfigError::Validation(diagnostics).into());
        }
        return Ok(());
    }

    // ── 1. Initialize logging ─────────────────────────────
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new("hyprdeck=info,hyprdeck_core=info")
            }),
        )
        .init();

    info!("HyprDeck starting");

    let instance_control = match InstanceControl::claim_default().await? {
        InstanceClaim::Primary(control) => control,
        InstanceClaim::RefreshedExisting => {
            info!("existing HyprDeck instance refreshed; exiting duplicate");
            return Ok(());
        }
    };

    // ── 2. Load config ────────────────────────────────────
    let config_path = hyprdeck_core::default_config_path()?;
    info!("Loading config from {:?}", config_path);
    let config = hyprdeck_core::Config::load(&config_path)?;
    info!("Theme: {}", config.theme);

    // The service is opt-in so HyprDeck never unexpectedly races an existing
    // notification daemon for the well-known D-Bus name.
    let (notification_tx, mut notification_rx) = mpsc::unbounded_channel();
    let _notification_dbus = if config.notifications.enabled {
        match notifications::start_notification_service(notification_tx).await {
            Ok(connection) => {
                info!("Serving org.freedesktop.Notifications");
                Some(connection)
            }
            Err(error) => {
                warn!(
                    ?error,
                    "could not claim org.freedesktop.Notifications; notifications disabled"
                );
                None
            }
        }
    } else {
        info!("Desktop notification daemon disabled by configuration");
        None
    };
    let notification_service_active = _notification_dbus.is_some();

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
    let hypr_socket = ipc.command().socket_path().to_path_buf();
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
        keyboard: None,
        keyboard_focus_surface: None,
        modifiers: 0,
        hypr_socket,
        notification_center: NotificationCenter::default(),
        notification_surfaces: HashMap::new(),
        notification_output: None,
        popup_close: PopupCloseTracker::default(),
    };

    // Initial roundtrip: enumerates Wayland outputs (populates wl_outputs).
    event_queue.roundtrip(&mut app_state)?;

    // ── 7. Add outputs and create Wayland surfaces ────────
    {
        let state = hypr_state.read().await;
        app_state.reconcile_output_topology(&state, &qh);
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

    let mut popup_close_interval = tokio::time::interval(Duration::from_millis(50));
    popup_close_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

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

            refresh = instance_control.receive_refresh() => {
                match refresh {
                    Ok(true) => {
                        let state = hypr_state.read().await;
                        if let Err(error) = app_state.reload_bar(&config_path, &state, &qh) {
                            warn!(?error, "could not refresh HyprDeck configuration");
                        }
                    }
                    Ok(false) => warn!("ignored unknown HyprDeck control command"),
                    Err(error) => warn!(?error, "HyprDeck control socket failed"),
                }
            }

            command = notification_rx.recv(), if notification_service_active => {
                if let Some(command) = command {
                    if app_state.process_notification(command) {
                        let state = hypr_state.read().await;
                        app_state.sync_notification_surfaces(&state, &qh, true);
                    }
                }
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

                        // HyprState was already updated by the socket reader task
                        // (state update happens before broadcast). Run an immediate
                        // module tick so panels reflect the change without waiting
                        // for the periodic fallback interval.
                        {
                            let now = chrono::Local::now();
                            let state = hypr_state.read().await;
                            let topology_changed =
                                app_state.reconcile_output_topology(&state, &qh);
                            app_state.app.tick_modules(now, &state);
                            app_state.sync_notification_surfaces(
                                &state,
                                &qh,
                                topology_changed,
                            );
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
                let topology_changed = app_state.reconcile_output_topology(&state, &qh);
                app_state.app.tick_modules(now, &state);
                if app_state.expire_notifications() || topology_changed {
                    app_state.sync_notification_surfaces(&state, &qh, true);
                } else {
                    app_state.sync_notification_surfaces(&state, &qh, false);
                }
            }

            _ = popup_close_interval.tick() => {
                app_state.close_expired_popups();
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

#[cfg(test)]
mod tests {
    use super::*;

    fn modifiers(shift: bool, ctrl: bool, alt: bool, logo: bool) -> Modifiers {
        Modifiers {
            shift,
            ctrl,
            alt,
            logo,
            ..Modifiers::default()
        }
    }

    #[test]
    fn pack_modifiers_maps_each_flag() {
        assert_eq!(
            pack_modifiers(&modifiers(true, false, false, false)),
            keymod::SHIFT
        );
        assert_eq!(
            pack_modifiers(&modifiers(false, true, false, false)),
            keymod::CTRL
        );
        assert_eq!(
            pack_modifiers(&modifiers(false, false, true, false)),
            keymod::ALT
        );
        assert_eq!(
            pack_modifiers(&modifiers(false, false, false, true)),
            keymod::LOGO
        );
        assert_eq!(pack_modifiers(&modifiers(false, false, false, false)), 0);
        assert_eq!(
            pack_modifiers(&modifiers(true, true, true, true)),
            keymod::SHIFT | keymod::CTRL | keymod::ALT | keymod::LOGO
        );
    }

    #[test]
    fn pack_modifiers_ignores_lock_states() {
        let m = Modifiers {
            caps_lock: true,
            num_lock: true,
            ..Modifiers::default()
        };
        assert_eq!(pack_modifiers(&m), 0);
    }

    #[test]
    fn popup_close_grace_can_be_cancelled_by_reentry() {
        let start = Instant::now();
        let mut tracker = PopupCloseTracker::default();
        tracker.schedule("DP-1", 2, start);
        assert!(
            tracker
                .take_expired(start + POPUP_LEAVE_GRACE - Duration::from_millis(1))
                .is_empty()
        );

        tracker.cancel("DP-1", 2);
        assert!(tracker.take_expired(start + POPUP_LEAVE_GRACE).is_empty());
    }

    #[test]
    fn popup_close_grace_expires_for_pointer_exit() {
        let start = Instant::now();
        let mut tracker = PopupCloseTracker::default();
        tracker.schedule("DP-1", 2, start);
        assert_eq!(
            tracker.take_expired(start + POPUP_LEAVE_GRACE),
            vec![("DP-1".to_owned(), 2)]
        );
    }
}
