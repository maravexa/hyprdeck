//! Hyprland event types and state aggregation.
//!
//! This module defines [`HyprEvent`] — a typed representation of each event
//! Hyprland emits on `.socket2.sock` — along with [`HyprState`], which holds
//! the aggregated compositor state that the rest of HyprDeck reads from.
//!
//! Event parsing is handled by [`parse_event`]; state mutation is handled by
//! [`HyprState::apply_event`]. Both functions are infallible from the caller's
//! perspective: unknown or malformed data is logged at debug level and skipped.

use tracing::debug;

/// Typed events received from Hyprland's `.socket2.sock` event stream.
///
/// Not every Hyprland event is represented here — unknown events are silently
/// dropped by [`parse_event`] to preserve forward compatibility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HyprEvent {
    /// Active workspace changed to the given workspace ID.
    WorkspaceChanged { id: i32 },
    /// A workspace was created (`createworkspace`).
    WorkspaceAdded { id: i32 },
    /// A workspace was destroyed (`destroyworkspace`).
    WorkspaceDestroyed { id: i32 },
    /// A workspace was moved to a different monitor.
    WorkspaceMoved { id: i32, monitor: String },
    /// The active window changed (`activewindow`).
    ActiveWindow { class: String, title: String },
    /// The active window address (`activewindowv2`).
    ActiveWindowV2 { address: String },
    /// The focused monitor changed (`focusedmon`).
    ActiveMonitor { name: String, workspace_id: i32 },
    /// Fullscreen state of the focused window toggled.
    Fullscreen { enter: bool },
    /// A monitor was hot-plugged.
    MonitorAdded {
        id: Option<i32>,
        name: String,
        description: Option<String>,
    },
    /// A monitor was unplugged.
    MonitorRemoved { name: String },
    /// A window wants attention (urgency hint).
    Urgent { address: String },
    /// A new window was opened.
    WindowOpened {
        address: String,
        workspace_name: String,
        class: String,
        title: String,
    },
    /// An existing window was closed.
    WindowClosed { address: String },
    /// A window moved to a different workspace.
    WindowMoved {
        address: String,
        workspace_name: String,
    },
    /// The keyboard layout changed for a device.
    LayoutChanged { keyboard: String, layout: String },
    /// A submap (keybinding mode) was entered or exited.
    SubMap { name: String },
    /// Floating state of a window toggled.
    FloatingMode { address: String, floating: bool },
    /// A window's title changed (`windowtitle`, address only).
    WindowTitle { address: String },
    /// A window's title changed (`windowtitlev2`, address + new title).
    WindowTitleV2 { address: String, title: String },
}

/// Aggregated Hyprland compositor state.
///
/// Built at startup via the command socket and kept live by applying
/// [`HyprEvent`]s from the event socket. Modules take read locks on this
/// to render.
#[derive(Debug, Default, Clone)]
pub struct HyprState {
    /// All known workspaces.
    pub workspaces: Vec<Workspace>,
    /// The currently active workspace ID.
    pub active_workspace: i32,
    /// The currently focused window, if any.
    pub active_window: Option<WindowInfo>,
    /// All connected monitors.
    pub monitors: Vec<MonitorInfo>,
    /// All open windows.
    pub windows: Vec<WindowInfo>,
    /// Name of the currently focused monitor (updated by `focusedmon` events).
    pub focused_monitor: String,
}

impl HyprState {
    /// Apply a single event, mutating state in-place.
    ///
    /// Unknown addresses, workspaces, or monitors are logged at `debug` level
    /// and skipped — this method never panics.
    pub fn apply_event(&mut self, event: &HyprEvent) {
        match event {
            HyprEvent::WorkspaceChanged { id } => {
                self.active_workspace = *id;
                // Update the focused monitor's active workspace immediately so
                // per-monitor rendering reflects the change without waiting for
                // a subsequent `focusedmon` event.
                if let Some(mon) = self
                    .monitors
                    .iter_mut()
                    .find(|m| m.name == self.focused_monitor)
                {
                    mon.active_workspace = *id;
                }
                // Switching to a workspace acknowledges any pending alert.
                if let Some(ws) = self.workspaces.iter_mut().find(|ws| ws.id == *id) {
                    ws.has_urgent = false;
                }
            }
            HyprEvent::WorkspaceAdded { id } => {
                if !self.workspaces.iter().any(|ws| ws.id == *id) {
                    self.workspaces.push(Workspace {
                        id: *id,
                        name: id.to_string(),
                        monitor: String::new(),
                        has_urgent: false,
                    });
                }
            }
            HyprEvent::WorkspaceDestroyed { id } => {
                self.workspaces.retain(|ws| ws.id != *id);
            }
            HyprEvent::WorkspaceMoved { id, monitor } => {
                if let Some(ws) = self.workspaces.iter_mut().find(|ws| ws.id == *id) {
                    ws.monitor = monitor.clone();
                } else {
                    debug!(workspace_id = id, "moveworkspace for unknown workspace");
                }
            }
            HyprEvent::ActiveWindow { class, title } => {
                // Clear focus on every window, then set on the match.
                for win in &mut self.windows {
                    win.is_focused = false;
                }
                let matched = self
                    .windows
                    .iter_mut()
                    .find(|w| w.class == *class && w.title == *title);
                if let Some(win) = matched {
                    win.is_focused = true;
                    self.active_window = Some(win.clone());
                } else {
                    // Fallback: construct a synthetic record so modules can at
                    // least show the title. The real entry will arrive via
                    // openwindow or the next hydration.
                    self.active_window = Some(WindowInfo {
                        address: String::new(),
                        title: title.clone(),
                        class: class.clone(),
                        workspace_id: self.active_workspace,
                        is_focused: true,
                        is_fullscreen: false,
                    });
                }
            }
            HyprEvent::ActiveWindowV2 { address } => {
                let normalized = normalize_address(address);
                for win in &mut self.windows {
                    win.is_focused = win.address == normalized;
                }
                if let Some(win) = self.windows.iter().find(|w| w.address == normalized) {
                    self.active_window = Some(win.clone());
                } else {
                    debug!(address = %normalized, "activewindowv2 for unknown window");
                }
            }
            HyprEvent::WindowOpened {
                address,
                workspace_name,
                class,
                title,
            } => {
                let address = normalize_address(address);
                let workspace_id = workspace_id_from_name(workspace_name, &self.workspaces);
                // If we already know about this window (race with hydration),
                // update instead of duplicating.
                if let Some(existing) = self.windows.iter_mut().find(|w| w.address == address) {
                    existing.title = title.clone();
                    existing.class = class.clone();
                    existing.workspace_id = workspace_id;
                } else {
                    self.windows.push(WindowInfo {
                        address,
                        title: title.clone(),
                        class: class.clone(),
                        workspace_id,
                        is_focused: false,
                        is_fullscreen: false,
                    });
                }
            }
            HyprEvent::WindowClosed { address } => {
                let normalized = normalize_address(address);
                let before = self.windows.len();
                self.windows.retain(|w| w.address != normalized);
                if self.windows.len() == before {
                    debug!(address = %normalized, "closewindow for unknown window");
                }
                if let Some(active) = &self.active_window {
                    if active.address == normalized {
                        self.active_window = None;
                    }
                }
            }
            HyprEvent::WindowMoved {
                address,
                workspace_name,
            } => {
                let normalized = normalize_address(address);
                let workspace_id = workspace_id_from_name(workspace_name, &self.workspaces);
                if let Some(win) = self.windows.iter_mut().find(|w| w.address == normalized) {
                    win.workspace_id = workspace_id;
                } else {
                    debug!(address = %normalized, "movewindow for unknown window");
                }
            }
            HyprEvent::Urgent { address } => {
                let normalized = normalize_address(address);
                let ws_id = self
                    .windows
                    .iter()
                    .find(|w| w.address == normalized)
                    .map(|w| w.workspace_id);
                match ws_id {
                    Some(ws_id) => {
                        // Suppress the alert if the workspace is already the
                        // active one on some monitor — the user can see it.
                        let visible = self.monitors.iter().any(|m| m.active_workspace == ws_id);
                        if !visible {
                            if let Some(ws) = self.workspaces.iter_mut().find(|ws| ws.id == ws_id) {
                                ws.has_urgent = true;
                            }
                        }
                    }
                    None => {
                        debug!(address = %normalized, "urgent for unknown window");
                    }
                }
            }
            HyprEvent::MonitorAdded {
                id: _,
                name,
                description: _,
            } => {
                if !self.monitors.iter().any(|m| m.name == *name) {
                    self.monitors.push(MonitorInfo {
                        name: name.clone(),
                        width: 0,
                        height: 0,
                        active_workspace: 0,
                    });
                }
            }
            HyprEvent::MonitorRemoved { name } => {
                self.monitors.retain(|m| m.name != *name);
            }
            HyprEvent::ActiveMonitor { name, workspace_id } => {
                self.focused_monitor = name.clone();
                if let Some(mon) = self.monitors.iter_mut().find(|m| m.name == *name) {
                    mon.active_workspace = *workspace_id;
                } else {
                    debug!(monitor = %name, "focusedmon for unknown monitor");
                }
                self.active_workspace = *workspace_id;
                // The newly focused workspace is in view — clear any alert.
                if let Some(ws) = self.workspaces.iter_mut().find(|ws| ws.id == *workspace_id) {
                    ws.has_urgent = false;
                }
            }
            HyprEvent::Fullscreen { enter } => {
                let addr = self.active_window.as_mut().map(|active| {
                    active.is_fullscreen = *enter;
                    active.address.clone()
                });
                if let Some(addr) = addr {
                    if !addr.is_empty() {
                        if let Some(win) = self.windows.iter_mut().find(|w| w.address == addr) {
                            win.is_fullscreen = *enter;
                        }
                    }
                }
            }
            HyprEvent::WindowTitle { address } => {
                // windowtitle (v1) carries no new title — consumers must
                // re-query. We only note the event here.
                debug!(address = %normalize_address(address), "windowtitle event (v1, no payload)");
            }
            HyprEvent::WindowTitleV2 { address, title } => {
                let normalized = normalize_address(address);
                if let Some(win) = self.windows.iter_mut().find(|w| w.address == normalized) {
                    win.title = title.clone();
                } else {
                    debug!(address = %normalized, "windowtitlev2 for unknown window");
                }
                if let Some(active) = self.active_window.as_mut() {
                    if active.address == normalized {
                        active.title = title.clone();
                    }
                }
            }
            HyprEvent::LayoutChanged { .. } | HyprEvent::SubMap { .. } => {
                // No state fields tracked for these yet; they're still
                // broadcast so interested modules can react.
            }
            HyprEvent::FloatingMode { .. } => {
                // Floating state isn't tracked in WindowInfo at 1.0.
            }
        }
    }
}

/// A Hyprland workspace descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    pub id: i32,
    pub name: String,
    pub monitor: String,
    pub has_urgent: bool,
}

/// A Hyprland window descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowInfo {
    /// Hyprland window address, normalised without the `0x` prefix.
    pub address: String,
    pub title: String,
    pub class: String,
    pub workspace_id: i32,
    pub is_focused: bool,
    pub is_fullscreen: bool,
}

/// A connected monitor as reported by Hyprland.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorInfo {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub active_workspace: i32,
}

/// Strip a leading `0x`/`0X` from a Hyprland window address, if present.
///
/// Hyprland's JSON output includes the prefix, while `.socket2.sock` events
/// do not — we normalise on the unprefixed form internally.
pub(crate) fn normalize_address(address: &str) -> String {
    address
        .strip_prefix("0x")
        .or_else(|| address.strip_prefix("0X"))
        .unwrap_or(address)
        .to_owned()
}

/// Look up a workspace ID from its name, falling back to parsing the name
/// as an integer for numeric workspaces.
fn workspace_id_from_name(name: &str, workspaces: &[Workspace]) -> i32 {
    if let Some(ws) = workspaces.iter().find(|w| w.name == name) {
        return ws.id;
    }
    name.parse::<i32>().unwrap_or(0)
}

/// Parse a single raw event line from Hyprland's `.socket2.sock`.
///
/// Returns `None` for unrecognised or malformed events. Unknown event names
/// are logged at `debug` level so new Hyprland events can be discovered
/// during development.
pub fn parse_event(line: &str) -> Option<HyprEvent> {
    let line = line.trim_end_matches(['\r', '\n']);
    let (name, data) = line.split_once(">>")?;

    let event = match name {
        "workspace" | "workspacev2" => {
            // workspacev2 is ID,NAME — we only care about the ID.
            let id_str = data.split(',').next()?;
            HyprEvent::WorkspaceChanged {
                id: parse_workspace_id(id_str)?,
            }
        }
        "createworkspace" | "createworkspacev2" => {
            let id_str = data.split(',').next()?;
            HyprEvent::WorkspaceAdded {
                id: parse_workspace_id(id_str)?,
            }
        }
        "destroyworkspace" | "destroyworkspacev2" => {
            let id_str = data.split(',').next()?;
            HyprEvent::WorkspaceDestroyed {
                id: parse_workspace_id(id_str)?,
            }
        }
        "moveworkspace" | "moveworkspacev2" => {
            let mut parts = data.splitn(2, ',');
            let id = parse_workspace_id(parts.next()?)?;
            let monitor = parts.next()?.to_owned();
            HyprEvent::WorkspaceMoved { id, monitor }
        }
        "focusedmon" => {
            let (name, ws) = data.split_once(',')?;
            HyprEvent::ActiveMonitor {
                name: name.to_owned(),
                workspace_id: parse_workspace_id(ws)?,
            }
        }
        "activewindow" => {
            // Split on the FIRST comma only — titles may contain commas.
            let (class, title) = data.split_once(',').unwrap_or((data, ""));
            HyprEvent::ActiveWindow {
                class: class.to_owned(),
                title: title.to_owned(),
            }
        }
        "activewindowv2" => HyprEvent::ActiveWindowV2 {
            address: data.to_owned(),
        },
        "fullscreen" => {
            let enter = match data.trim() {
                "1" => true,
                "0" => false,
                other => {
                    debug!(value = %other, "unknown fullscreen payload");
                    return None;
                }
            };
            HyprEvent::Fullscreen { enter }
        }
        "monitoradded" => HyprEvent::MonitorAdded {
            id: None,
            name: data.to_owned(),
            description: None,
        },
        "monitoraddedv2" => {
            let mut parts = data.splitn(3, ',');
            let id = parts.next()?.parse::<i32>().ok();
            let name = parts.next()?.to_owned();
            let description = parts.next().map(str::to_owned);
            HyprEvent::MonitorAdded {
                id,
                name,
                description,
            }
        }
        "monitorremoved" => HyprEvent::MonitorRemoved {
            name: data.to_owned(),
        },
        "openwindow" => {
            // openwindow>>ADDRESS,WORKSPACENAME,CLASS,TITLE
            // Class is comma-free, title may contain commas.
            let mut parts = data.splitn(4, ',');
            let address = parts.next()?.to_owned();
            let workspace_name = parts.next()?.to_owned();
            let class = parts.next()?.to_owned();
            let title = parts.next().unwrap_or("").to_owned();
            HyprEvent::WindowOpened {
                address,
                workspace_name,
                class,
                title,
            }
        }
        "closewindow" => HyprEvent::WindowClosed {
            address: data.to_owned(),
        },
        "movewindow" | "movewindowv2" => {
            let mut parts = data.splitn(2, ',');
            let address = parts.next()?.to_owned();
            let workspace_name = parts.next().unwrap_or("").to_owned();
            HyprEvent::WindowMoved {
                address,
                workspace_name,
            }
        }
        "urgent" => HyprEvent::Urgent {
            address: data.to_owned(),
        },
        "submap" => HyprEvent::SubMap {
            name: data.to_owned(),
        },
        "activelayout" => {
            let (keyboard, layout) = data.split_once(',')?;
            HyprEvent::LayoutChanged {
                keyboard: keyboard.to_owned(),
                layout: layout.to_owned(),
            }
        }
        "changefloatingmode" => {
            let (address, flag) = data.split_once(',')?;
            let floating = flag.trim() == "1";
            HyprEvent::FloatingMode {
                address: address.to_owned(),
                floating,
            }
        }
        "windowtitle" => HyprEvent::WindowTitle {
            address: data.to_owned(),
        },
        "windowtitlev2" => {
            // windowtitlev2>>ADDRESS,TITLE
            let (address, title) = data.split_once(',').unwrap_or((data, ""));
            HyprEvent::WindowTitleV2 {
                address: address.to_owned(),
                title: title.to_owned(),
            }
        }
        other => {
            debug!(event = %other, payload = %data, "unknown Hyprland event");
            return None;
        }
    };

    Some(event)
}

fn parse_workspace_id(s: &str) -> Option<i32> {
    s.trim().parse::<i32>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ws(id: i32, name: &str) -> Workspace {
        Workspace {
            id,
            name: name.to_owned(),
            monitor: String::new(),
            has_urgent: false,
        }
    }

    fn win(addr: &str, class: &str, title: &str, ws_id: i32) -> WindowInfo {
        WindowInfo {
            address: addr.to_owned(),
            title: title.to_owned(),
            class: class.to_owned(),
            workspace_id: ws_id,
            is_focused: false,
            is_fullscreen: false,
        }
    }

    // --- parse_event ---

    #[test]
    fn parses_workspace_changed() {
        assert_eq!(
            parse_event("workspace>>3"),
            Some(HyprEvent::WorkspaceChanged { id: 3 }),
        );
    }

    #[test]
    fn parses_workspacev2_uses_id_field() {
        assert_eq!(
            parse_event("workspacev2>>5,five"),
            Some(HyprEvent::WorkspaceChanged { id: 5 }),
        );
    }

    #[test]
    fn parses_negative_workspace_id() {
        assert_eq!(
            parse_event("workspace>>-99"),
            Some(HyprEvent::WorkspaceChanged { id: -99 }),
        );
    }

    #[test]
    fn parses_create_and_destroy_workspace() {
        assert_eq!(
            parse_event("createworkspace>>7"),
            Some(HyprEvent::WorkspaceAdded { id: 7 }),
        );
        assert_eq!(
            parse_event("destroyworkspace>>7"),
            Some(HyprEvent::WorkspaceDestroyed { id: 7 }),
        );
    }

    #[test]
    fn parses_moveworkspace() {
        assert_eq!(
            parse_event("moveworkspace>>4,DP-1"),
            Some(HyprEvent::WorkspaceMoved {
                id: 4,
                monitor: "DP-1".to_owned(),
            }),
        );
    }

    #[test]
    fn parses_focusedmon() {
        assert_eq!(
            parse_event("focusedmon>>DP-1,2"),
            Some(HyprEvent::ActiveMonitor {
                name: "DP-1".to_owned(),
                workspace_id: 2,
            }),
        );
    }

    #[test]
    fn parses_activewindow_with_comma_in_title() {
        assert_eq!(
            parse_event("activewindow>>kitty,~/my project, the cool one"),
            Some(HyprEvent::ActiveWindow {
                class: "kitty".to_owned(),
                title: "~/my project, the cool one".to_owned(),
            }),
        );
    }

    #[test]
    fn parses_activewindow_empty_title() {
        assert_eq!(
            parse_event("activewindow>>firefox,"),
            Some(HyprEvent::ActiveWindow {
                class: "firefox".to_owned(),
                title: String::new(),
            }),
        );
    }

    #[test]
    fn parses_activewindowv2_address() {
        assert_eq!(
            parse_event("activewindowv2>>5649f0a1e030"),
            Some(HyprEvent::ActiveWindowV2 {
                address: "5649f0a1e030".to_owned(),
            }),
        );
    }

    #[test]
    fn parses_fullscreen_enter_and_exit() {
        assert_eq!(
            parse_event("fullscreen>>1"),
            Some(HyprEvent::Fullscreen { enter: true }),
        );
        assert_eq!(
            parse_event("fullscreen>>0"),
            Some(HyprEvent::Fullscreen { enter: false }),
        );
    }

    #[test]
    fn parses_monitor_added_and_removed() {
        assert_eq!(
            parse_event("monitoradded>>DP-2"),
            Some(HyprEvent::MonitorAdded {
                id: None,
                name: "DP-2".to_owned(),
                description: None,
            }),
        );
        assert_eq!(
            parse_event("monitorremoved>>DP-2"),
            Some(HyprEvent::MonitorRemoved {
                name: "DP-2".to_owned(),
            }),
        );
    }

    #[test]
    fn parses_monitoraddedv2() {
        assert_eq!(
            parse_event("monitoraddedv2>>1,DP-2,Dell U2720Q"),
            Some(HyprEvent::MonitorAdded {
                id: Some(1),
                name: "DP-2".to_owned(),
                description: Some("Dell U2720Q".to_owned()),
            }),
        );
    }

    #[test]
    fn parses_openwindow_all_fields() {
        assert_eq!(
            parse_event("openwindow>>5649f0a1e030,1,kitty,~/code/hyprdeck"),
            Some(HyprEvent::WindowOpened {
                address: "5649f0a1e030".to_owned(),
                workspace_name: "1".to_owned(),
                class: "kitty".to_owned(),
                title: "~/code/hyprdeck".to_owned(),
            }),
        );
    }

    #[test]
    fn parses_openwindow_title_with_commas() {
        assert_eq!(
            parse_event("openwindow>>aabb,2,firefox,GitHub, Inc., Mozilla Firefox"),
            Some(HyprEvent::WindowOpened {
                address: "aabb".to_owned(),
                workspace_name: "2".to_owned(),
                class: "firefox".to_owned(),
                title: "GitHub, Inc., Mozilla Firefox".to_owned(),
            }),
        );
    }

    #[test]
    fn parses_closewindow() {
        assert_eq!(
            parse_event("closewindow>>5649f0a1e030"),
            Some(HyprEvent::WindowClosed {
                address: "5649f0a1e030".to_owned(),
            }),
        );
    }

    #[test]
    fn parses_movewindow() {
        assert_eq!(
            parse_event("movewindow>>aabb,3"),
            Some(HyprEvent::WindowMoved {
                address: "aabb".to_owned(),
                workspace_name: "3".to_owned(),
            }),
        );
    }

    #[test]
    fn parses_urgent() {
        assert_eq!(
            parse_event("urgent>>aabb"),
            Some(HyprEvent::Urgent {
                address: "aabb".to_owned(),
            }),
        );
    }

    #[test]
    fn parses_submap() {
        assert_eq!(
            parse_event("submap>>resize"),
            Some(HyprEvent::SubMap {
                name: "resize".to_owned(),
            }),
        );
    }

    #[test]
    fn parses_activelayout() {
        assert_eq!(
            parse_event("activelayout>>AT Translated Set 2 keyboard,English (US)"),
            Some(HyprEvent::LayoutChanged {
                keyboard: "AT Translated Set 2 keyboard".to_owned(),
                layout: "English (US)".to_owned(),
            }),
        );
    }

    #[test]
    fn parses_changefloatingmode() {
        assert_eq!(
            parse_event("changefloatingmode>>aabb,1"),
            Some(HyprEvent::FloatingMode {
                address: "aabb".to_owned(),
                floating: true,
            }),
        );
    }

    #[test]
    fn parses_windowtitle_and_v2() {
        assert_eq!(
            parse_event("windowtitle>>aabb"),
            Some(HyprEvent::WindowTitle {
                address: "aabb".to_owned(),
            }),
        );
        assert_eq!(
            parse_event("windowtitlev2>>aabb,New, title"),
            Some(HyprEvent::WindowTitleV2 {
                address: "aabb".to_owned(),
                title: "New, title".to_owned(),
            }),
        );
    }

    #[test]
    fn empty_workspace_payload_fails() {
        assert_eq!(parse_event("workspace>>"), None);
    }

    #[test]
    fn unknown_event_returns_none() {
        assert_eq!(parse_event("somefutureevent>>data"), None);
    }

    #[test]
    fn line_without_separator_returns_none() {
        assert_eq!(parse_event("no separator here"), None);
    }

    #[test]
    fn trims_trailing_newline() {
        assert_eq!(
            parse_event("workspace>>2\n"),
            Some(HyprEvent::WorkspaceChanged { id: 2 }),
        );
    }

    // --- apply_event ---

    fn base_state() -> HyprState {
        HyprState {
            workspaces: vec![ws(1, "1"), ws(2, "2")],
            active_workspace: 1,
            active_window: None,
            monitors: vec![MonitorInfo {
                name: "DP-1".to_owned(),
                width: 1920,
                height: 1080,
                active_workspace: 1,
            }],
            windows: vec![
                win("aabb", "kitty", "shell", 1),
                win("ccdd", "firefox", "GitHub", 2),
            ],
            focused_monitor: "DP-1".to_owned(),
        }
    }

    #[test]
    fn apply_workspace_changed_updates_active() {
        let mut s = base_state();
        s.apply_event(&HyprEvent::WorkspaceChanged { id: 2 });
        assert_eq!(s.active_workspace, 2);
    }

    #[test]
    fn apply_workspace_added_and_destroyed() {
        let mut s = base_state();
        s.apply_event(&HyprEvent::WorkspaceAdded { id: 9 });
        assert!(s.workspaces.iter().any(|w| w.id == 9));
        s.apply_event(&HyprEvent::WorkspaceDestroyed { id: 9 });
        assert!(!s.workspaces.iter().any(|w| w.id == 9));
    }

    #[test]
    fn apply_workspace_added_is_idempotent() {
        let mut s = base_state();
        let before = s.workspaces.len();
        s.apply_event(&HyprEvent::WorkspaceAdded { id: 1 });
        assert_eq!(s.workspaces.len(), before);
    }

    #[test]
    fn apply_window_opened_adds_to_list() {
        let mut s = base_state();
        s.apply_event(&HyprEvent::WindowOpened {
            address: "eeff".to_owned(),
            workspace_name: "1".to_owned(),
            class: "alacritty".to_owned(),
            title: "bash".to_owned(),
        });
        assert!(s.windows.iter().any(|w| w.address == "eeff"));
    }

    #[test]
    fn apply_window_opened_strips_address_prefix() {
        let mut s = base_state();
        s.apply_event(&HyprEvent::WindowOpened {
            address: "0xeeff".to_owned(),
            workspace_name: "1".to_owned(),
            class: "alacritty".to_owned(),
            title: "bash".to_owned(),
        });
        assert!(s.windows.iter().any(|w| w.address == "eeff"));
    }

    #[test]
    fn apply_window_closed_removes_from_list() {
        let mut s = base_state();
        s.apply_event(&HyprEvent::WindowClosed {
            address: "aabb".to_owned(),
        });
        assert!(!s.windows.iter().any(|w| w.address == "aabb"));
    }

    #[test]
    fn apply_window_moved_updates_workspace() {
        let mut s = base_state();
        s.apply_event(&HyprEvent::WindowMoved {
            address: "aabb".to_owned(),
            workspace_name: "2".to_owned(),
        });
        let w = s.windows.iter().find(|w| w.address == "aabb").unwrap();
        assert_eq!(w.workspace_id, 2);
    }

    #[test]
    fn apply_window_moved_unknown_address_is_ignored() {
        let mut s = base_state();
        s.apply_event(&HyprEvent::WindowMoved {
            address: "zzzz".to_owned(),
            workspace_name: "2".to_owned(),
        });
        // Base state unchanged
        assert_eq!(s.windows.len(), 2);
    }

    #[test]
    fn apply_urgent_flags_correct_workspace() {
        let mut s = base_state();
        s.apply_event(&HyprEvent::Urgent {
            address: "ccdd".to_owned(),
        });
        let ws2 = s.workspaces.iter().find(|w| w.id == 2).unwrap();
        assert!(ws2.has_urgent);
        let ws1 = s.workspaces.iter().find(|w| w.id == 1).unwrap();
        assert!(!ws1.has_urgent);
    }

    #[test]
    fn apply_urgent_unknown_window_is_ignored() {
        let mut s = base_state();
        s.apply_event(&HyprEvent::Urgent {
            address: "zzzz".to_owned(),
        });
        assert!(s.workspaces.iter().all(|w| !w.has_urgent));
    }

    #[test]
    fn apply_urgent_skips_active_workspace() {
        // The window "aabb" lives on workspace 1, which is active on DP-1.
        // The user is already viewing it, so no alert should be raised.
        let mut s = base_state();
        s.apply_event(&HyprEvent::Urgent {
            address: "aabb".to_owned(),
        });
        let ws1 = s.workspaces.iter().find(|w| w.id == 1).unwrap();
        assert!(!ws1.has_urgent);
    }

    #[test]
    fn workspace_changed_clears_urgent() {
        let mut s = base_state();
        s.apply_event(&HyprEvent::Urgent {
            address: "ccdd".to_owned(),
        });
        assert!(s.workspaces.iter().find(|w| w.id == 2).unwrap().has_urgent);
        s.apply_event(&HyprEvent::WorkspaceChanged { id: 2 });
        assert!(!s.workspaces.iter().find(|w| w.id == 2).unwrap().has_urgent);
    }

    #[test]
    fn focused_mon_clears_urgent_on_target_workspace() {
        let mut s = base_state();
        s.workspaces
            .iter_mut()
            .find(|w| w.id == 2)
            .unwrap()
            .has_urgent = true;
        s.monitors.push(MonitorInfo {
            name: "DP-2".to_owned(),
            width: 1920,
            height: 1080,
            active_workspace: 1,
        });
        s.apply_event(&HyprEvent::ActiveMonitor {
            name: "DP-2".to_owned(),
            workspace_id: 2,
        });
        assert!(!s.workspaces.iter().find(|w| w.id == 2).unwrap().has_urgent);
    }

    #[test]
    fn apply_active_window_v2_sets_focus() {
        let mut s = base_state();
        s.apply_event(&HyprEvent::ActiveWindowV2 {
            address: "ccdd".to_owned(),
        });
        assert_eq!(s.active_window.as_ref().unwrap().address, "ccdd");
        assert!(
            s.windows
                .iter()
                .find(|w| w.address == "ccdd")
                .unwrap()
                .is_focused
        );
        assert!(
            !s.windows
                .iter()
                .find(|w| w.address == "aabb")
                .unwrap()
                .is_focused
        );
    }

    #[test]
    fn apply_monitor_added_and_removed() {
        let mut s = base_state();
        s.apply_event(&HyprEvent::MonitorAdded {
            id: Some(1),
            name: "DP-2".to_owned(),
            description: None,
        });
        assert!(s.monitors.iter().any(|m| m.name == "DP-2"));
        s.apply_event(&HyprEvent::MonitorRemoved {
            name: "DP-2".to_owned(),
        });
        assert!(!s.monitors.iter().any(|m| m.name == "DP-2"));
    }

    #[test]
    fn apply_active_monitor_updates_workspace() {
        let mut s = base_state();
        s.apply_event(&HyprEvent::ActiveMonitor {
            name: "DP-1".to_owned(),
            workspace_id: 2,
        });
        assert_eq!(s.active_workspace, 2);
        assert_eq!(s.monitors[0].active_workspace, 2);
    }

    #[test]
    fn apply_fullscreen_flags_active_window() {
        let mut s = base_state();
        s.apply_event(&HyprEvent::ActiveWindowV2 {
            address: "aabb".to_owned(),
        });
        s.apply_event(&HyprEvent::Fullscreen { enter: true });
        assert!(s.active_window.as_ref().unwrap().is_fullscreen);
        let w = s.windows.iter().find(|w| w.address == "aabb").unwrap();
        assert!(w.is_fullscreen);
    }

    #[test]
    fn apply_windowtitlev2_updates_title() {
        let mut s = base_state();
        s.apply_event(&HyprEvent::WindowTitleV2 {
            address: "aabb".to_owned(),
            title: "new title".to_owned(),
        });
        let w = s.windows.iter().find(|w| w.address == "aabb").unwrap();
        assert_eq!(w.title, "new title");
    }

    #[test]
    fn normalize_address_strips_prefix() {
        assert_eq!(normalize_address("0xdeadbeef"), "deadbeef");
        assert_eq!(normalize_address("deadbeef"), "deadbeef");
    }
}
