/// Typed events received from Hyprland's `/.socket2.sock` event stream.
#[derive(Debug, Clone)]
pub enum HyprEvent {
    WorkspaceChanged { id: i32 },
    WorkspaceAdded { id: i32 },
    WorkspaceDestroyed { id: i32 },
    ActiveWindow { class: String, title: String },
    ActiveMonitor { name: String, workspace_id: i32 },
    Fullscreen { enter: bool },
    MonitorAdded { name: String },
    MonitorRemoved { name: String },
    Urgent { address: String },
    WindowOpened {
        address: String,
        workspace_id: i32,
        class: String,
        title: String,
    },
    WindowClosed { address: String },
    WindowMoved { address: String, workspace_id: i32 },
    LayoutChanged { keyboard: String, layout: String },
    SubMap { name: String },
}

/// Aggregated Hyprland compositor state, built by replaying [`HyprEvent`]s and
/// issuing initial queries via the command socket at startup.
#[derive(Debug, Default)]
pub struct HyprState {
    pub workspaces: Vec<Workspace>,
    pub active_workspace: i32,
    pub active_window: Option<WindowInfo>,
    pub monitors: Vec<MonitorInfo>,
    pub windows: Vec<WindowInfo>,
}

impl HyprState {
    /// Apply a single event, mutating state in-place.
    ///
    /// Returns `true` if the mutation is likely to require a panel redraw.
    pub fn apply(&mut self, event: HyprEvent) -> bool {
        todo!()
    }
}

/// A Hyprland workspace descriptor.
#[derive(Debug, Clone)]
pub struct Workspace {
    pub id: i32,
    pub name: String,
    pub monitor: String,
    pub has_urgent: bool,
}

/// A Hyprland window descriptor.
#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub address: String,
    pub title: String,
    pub class: String,
    pub workspace_id: i32,
    pub is_focused: bool,
}

/// A connected monitor as reported by Hyprland.
#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub active_workspace: i32,
}
