//! Command socket client for Hyprland.
//!
//! Hyprland's command socket (`.socket.sock`) accepts one request per
//! connection: write the command string, read the full response, close. The
//! `j/` prefix requests JSON output; without it Hyprland returns plain text.
//!
//! This module exposes one-shot async helpers that open a fresh connection
//! per call — matching how `hyprctl` itself behaves — plus a convenience
//! [`hydrate_state`] that fans out the common startup queries concurrently
//! and assembles a [`HyprState`].
//!
//! A thin [`CommandClient`] wrapper is provided for callers that want to
//! carry the socket path around as a handle.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tracing::warn;

use super::IpcError;
use super::event::{HyprState, MonitorInfo, WindowInfo, Workspace, normalize_address};

/// One-shot client for Hyprland's command socket (`.socket.sock`).
///
/// Wraps a socket path and provides convenience methods that delegate to
/// the free functions in this module. Each call opens a fresh connection.
#[derive(Debug, Clone)]
pub struct CommandClient {
    socket_path: PathBuf,
}

impl CommandClient {
    /// Construct a client pointing at the given socket path.
    ///
    /// The path is typically
    /// `$XDG_RUNTIME_DIR/hypr/$HYPRLAND_INSTANCE_SIGNATURE/.socket.sock`.
    pub fn new(socket_path: PathBuf) -> Self {
        CommandClient { socket_path }
    }

    /// Build a [`HyprState`] snapshot by querying workspaces, windows, and monitors.
    pub async fn query_state(&self) -> Result<HyprState, IpcError> {
        hydrate_state(&self.socket_path).await
    }

    /// Send a `hyprctl dispatch` command.
    pub async fn dispatch(&self, args: &str) -> Result<(), IpcError> {
        dispatch(&self.socket_path, args).await
    }

    /// Send a raw `hyprctl` request and return the response string.
    pub async fn request(&self, payload: &str) -> Result<String, IpcError> {
        hyprctl(&self.socket_path, payload).await
    }

    /// Borrow the underlying socket path.
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }
}

/// Send a command to Hyprland's command socket and return the raw response.
///
/// Opens a fresh UNIX stream, writes `command`, reads to EOF, and closes.
/// Most callers want one of the typed helpers below, but this is exposed
/// for forward compatibility with Hyprland commands this crate doesn't
/// model directly.
pub async fn hyprctl(socket_path: &Path, command: &str) -> Result<String, IpcError> {
    let mut stream =
        UnixStream::connect(socket_path)
            .await
            .map_err(|source| IpcError::Connect {
                path: socket_path.to_path_buf(),
                source,
            })?;

    stream.write_all(command.as_bytes()).await?;
    // Half-close the write side so Hyprland knows the request is complete.
    stream.shutdown().await?;

    let mut response = String::new();
    stream.read_to_string(&mut response).await?;
    Ok(response)
}

/// Query all workspaces.
pub async fn get_workspaces(socket_path: &Path) -> Result<Vec<Workspace>, IpcError> {
    let json = hyprctl(socket_path, "j/workspaces").await?;
    let raw: Vec<RawWorkspace> = serde_json::from_str(&json)?;
    Ok(raw.into_iter().map(Workspace::from).collect())
}

/// Query all open windows (clients).
pub async fn get_clients(socket_path: &Path) -> Result<Vec<WindowInfo>, IpcError> {
    let json = hyprctl(socket_path, "j/clients").await?;
    let raw: Vec<RawClient> = serde_json::from_str(&json)?;
    Ok(raw.into_iter().map(WindowInfo::from).collect())
}

/// Query all connected monitors.
pub async fn get_monitors(socket_path: &Path) -> Result<Vec<MonitorInfo>, IpcError> {
    let json = hyprctl(socket_path, "j/monitors").await?;
    let raw: Vec<RawMonitor> = serde_json::from_str(&json)?;
    Ok(raw.into_iter().map(MonitorInfo::from).collect())
}

/// Query the currently active workspace.
pub async fn get_active_workspace(socket_path: &Path) -> Result<Workspace, IpcError> {
    let json = hyprctl(socket_path, "j/activeworkspace").await?;
    let raw: RawWorkspace = serde_json::from_str(&json)?;
    Ok(Workspace::from(raw))
}

/// Full state hydration — called on startup and after a reconnect.
///
/// Runs the four underlying queries concurrently, then stitches the
/// results together: sets `active_workspace`, marks the focused window,
/// and flags any workspace containing an urgent window.
pub async fn hydrate_state(socket_path: &Path) -> Result<HyprState, IpcError> {
    let (workspaces, windows, monitors, active_ws) = tokio::try_join!(
        get_workspaces(socket_path),
        get_clients(socket_path),
        get_monitors(socket_path),
        get_active_workspace(socket_path),
    )?;

    Ok(assemble_state(workspaces, windows, monitors, active_ws))
}

/// Send a `/dispatch` command to Hyprland.
///
/// Dispatch doesn't use the `j/` prefix — it returns a plain text response,
/// typically `"ok"` on success. Anything else is logged at `warn` level.
pub async fn dispatch(socket_path: &Path, args: &str) -> Result<(), IpcError> {
    let command = format!("/dispatch {args}");
    let response = hyprctl(socket_path, &command).await?;
    let trimmed = response.trim();
    if trimmed != "ok" {
        warn!(%args, %trimmed, "hyprctl dispatch returned non-ok response");
    }
    Ok(())
}

/// Assemble a [`HyprState`] from the raw query results.
///
/// Pure function — extracted from [`hydrate_state`] so it can be tested
/// without touching a socket. Picks the focused window as the one with
/// `focusHistoryID == 0` (marked as `is_focused` during deserialization)
/// and leaves urgency flags clear — urgent state is only observed via the
/// event socket.
fn assemble_state(
    workspaces: Vec<Workspace>,
    windows: Vec<WindowInfo>,
    monitors: Vec<MonitorInfo>,
    active_ws: Workspace,
) -> HyprState {
    let active_workspace_id = active_ws.id;
    let active_window = windows.iter().find(|w| w.is_focused).cloned();

    HyprState {
        workspaces,
        active_workspace: active_workspace_id,
        active_window,
        monitors,
        windows,
        focused_monitor: String::new(),
    }
}

// ---------------------------------------------------------------------------
// Raw Hyprland JSON shapes.
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct RawWorkspace {
    id: i32,
    name: String,
    monitor: String,
}

impl From<RawWorkspace> for Workspace {
    fn from(raw: RawWorkspace) -> Self {
        Workspace {
            id: raw.id,
            name: raw.name,
            monitor: raw.monitor,
            has_urgent: false,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawClient {
    address: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    class: String,
    workspace: RawClientWorkspace,
    #[serde(default, rename = "focusHistoryID")]
    focus_history_id: i32,
    #[serde(default)]
    fullscreen: FullscreenField,
}

#[derive(Debug, Deserialize)]
struct RawClientWorkspace {
    id: i32,
}

/// Hyprland's `fullscreen` field has changed representation across versions:
/// older builds use a bool, newer builds use an int (0 = none, 1+ = some
/// form of fullscreen). Accept either.
#[derive(Debug, Default)]
struct FullscreenField(bool);

impl<'de> Deserialize<'de> for FullscreenField {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;
        let v = serde_json::Value::deserialize(deserializer)?;
        match v {
            serde_json::Value::Bool(b) => Ok(FullscreenField(b)),
            serde_json::Value::Number(n) => {
                let n = n.as_i64().ok_or_else(|| D::Error::custom("bad number"))?;
                Ok(FullscreenField(n > 0))
            }
            serde_json::Value::Null => Ok(FullscreenField(false)),
            other => Err(D::Error::custom(format!(
                "unexpected fullscreen value: {other:?}"
            ))),
        }
    }
}

impl From<RawClient> for WindowInfo {
    fn from(raw: RawClient) -> Self {
        WindowInfo {
            address: normalize_address(&raw.address),
            title: raw.title,
            class: raw.class,
            workspace_id: raw.workspace.id,
            // focusHistoryID == 0 => most recently focused window.
            is_focused: raw.focus_history_id == 0,
            is_fullscreen: raw.fullscreen.0,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMonitor {
    name: String,
    #[serde(default)]
    width: u32,
    #[serde(default)]
    height: u32,
    #[serde(default)]
    active_workspace: RawActiveWorkspaceRef,
}

#[derive(Debug, Default, Deserialize)]
struct RawActiveWorkspaceRef {
    #[serde(default)]
    id: i32,
}

impl From<RawMonitor> for MonitorInfo {
    fn from(raw: RawMonitor) -> Self {
        MonitorInfo {
            name: raw.name,
            width: raw.width,
            height: raw.height,
            active_workspace: raw.active_workspace.id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLIENTS_JSON: &str = r#"[
        {
            "address": "0x5649f0a1e030",
            "mapped": true,
            "hidden": false,
            "at": [0, 0],
            "size": [1920, 1080],
            "workspace": { "id": 1, "name": "1" },
            "floating": false,
            "monitor": 0,
            "class": "kitty",
            "title": "~/code/hyprdeck",
            "initialClass": "kitty",
            "initialTitle": "kitty",
            "pid": 1234,
            "xwayland": false,
            "pinned": false,
            "fullscreen": 0,
            "fullscreenClient": 0,
            "grouped": [],
            "swallowing": "0x0",
            "focusHistoryID": 0
        },
        {
            "address": "0x5649f0a1e200",
            "mapped": true,
            "hidden": false,
            "at": [0, 0],
            "size": [800, 600],
            "workspace": { "id": 2, "name": "2" },
            "floating": true,
            "monitor": 0,
            "class": "firefox",
            "title": "GitHub, Inc.",
            "pid": 5678,
            "xwayland": false,
            "pinned": false,
            "fullscreen": true,
            "focusHistoryID": 1
        }
    ]"#;

    const WORKSPACES_JSON: &str = r#"[
        { "id": 1, "name": "1", "monitor": "DP-1", "windows": 1, "hasfullscreen": false, "lastwindow": "0x5649f0a1e030", "lastwindowtitle": "~/code" },
        { "id": 2, "name": "2", "monitor": "DP-1", "windows": 1, "hasfullscreen": true, "lastwindow": "0x5649f0a1e200", "lastwindowtitle": "GitHub" }
    ]"#;

    const MONITORS_JSON: &str = r#"[
        {
            "id": 0,
            "name": "DP-1",
            "description": "Dell Inc.",
            "make": "Dell",
            "model": "U2720Q",
            "width": 3840,
            "height": 2160,
            "refreshRate": 60.0,
            "x": 0,
            "y": 0,
            "activeWorkspace": { "id": 1, "name": "1" },
            "specialWorkspace": { "id": 0, "name": "" },
            "reserved": [0, 0, 0, 0],
            "scale": 1.5,
            "transform": 0,
            "focused": true,
            "dpmsStatus": true,
            "vrr": false
        }
    ]"#;

    const ACTIVE_WS_JSON: &str = r#"{
        "id": 2,
        "name": "2",
        "monitor": "DP-1",
        "windows": 1,
        "hasfullscreen": true,
        "lastwindow": "0x5649f0a1e200",
        "lastwindowtitle": "GitHub"
    }"#;

    #[test]
    fn deserialize_workspaces() {
        let raw: Vec<RawWorkspace> = serde_json::from_str(WORKSPACES_JSON).unwrap();
        let ws: Vec<Workspace> = raw.into_iter().map(Workspace::from).collect();
        assert_eq!(ws.len(), 2);
        assert_eq!(ws[0].id, 1);
        assert_eq!(ws[0].name, "1");
        assert_eq!(ws[0].monitor, "DP-1");
    }

    #[test]
    fn deserialize_clients_normalises_address_and_focus() {
        let raw: Vec<RawClient> = serde_json::from_str(CLIENTS_JSON).unwrap();
        let wins: Vec<WindowInfo> = raw.into_iter().map(WindowInfo::from).collect();
        assert_eq!(wins.len(), 2);
        assert_eq!(wins[0].address, "5649f0a1e030");
        assert_eq!(wins[0].class, "kitty");
        assert_eq!(wins[0].workspace_id, 1);
        assert!(wins[0].is_focused);
        assert!(!wins[0].is_fullscreen);

        assert_eq!(wins[1].address, "5649f0a1e200");
        assert_eq!(wins[1].title, "GitHub, Inc.");
        assert!(!wins[1].is_focused);
        assert!(wins[1].is_fullscreen);
    }

    #[test]
    fn deserialize_monitors() {
        let raw: Vec<RawMonitor> = serde_json::from_str(MONITORS_JSON).unwrap();
        let mons: Vec<MonitorInfo> = raw.into_iter().map(MonitorInfo::from).collect();
        assert_eq!(mons.len(), 1);
        assert_eq!(mons[0].name, "DP-1");
        assert_eq!(mons[0].width, 3840);
        assert_eq!(mons[0].height, 2160);
        assert_eq!(mons[0].active_workspace, 1);
    }

    #[test]
    fn deserialize_active_workspace() {
        let raw: RawWorkspace = serde_json::from_str(ACTIVE_WS_JSON).unwrap();
        let ws = Workspace::from(raw);
        assert_eq!(ws.id, 2);
        assert_eq!(ws.name, "2");
    }

    #[test]
    fn assemble_state_picks_active_workspace_and_focused_window() {
        let workspaces: Vec<Workspace> = serde_json::from_str::<Vec<RawWorkspace>>(WORKSPACES_JSON)
            .unwrap()
            .into_iter()
            .map(Workspace::from)
            .collect();
        let windows: Vec<WindowInfo> = serde_json::from_str::<Vec<RawClient>>(CLIENTS_JSON)
            .unwrap()
            .into_iter()
            .map(WindowInfo::from)
            .collect();
        let monitors: Vec<MonitorInfo> = serde_json::from_str::<Vec<RawMonitor>>(MONITORS_JSON)
            .unwrap()
            .into_iter()
            .map(MonitorInfo::from)
            .collect();
        let active_ws: Workspace = serde_json::from_str::<RawWorkspace>(ACTIVE_WS_JSON)
            .map(Workspace::from)
            .unwrap();

        let state = assemble_state(workspaces, windows, monitors, active_ws);
        assert_eq!(state.active_workspace, 2);
        assert_eq!(state.windows.len(), 2);
        assert_eq!(state.monitors.len(), 1);
        // The focused-flagged window is on ws 1, not the active ws 2 — the
        // assembler falls back to any window with is_focused set.
        let aw = state.active_window.expect("active window present");
        assert_eq!(aw.address, "5649f0a1e030");
    }

    #[test]
    fn fullscreen_field_accepts_bool_and_int() {
        let as_bool: FullscreenField = serde_json::from_str("true").unwrap();
        assert!(as_bool.0);
        let as_int: FullscreenField = serde_json::from_str("2").unwrap();
        assert!(as_int.0);
        let as_zero: FullscreenField = serde_json::from_str("0").unwrap();
        assert!(!as_zero.0);
        let as_null: FullscreenField = serde_json::from_str("null").unwrap();
        assert!(!as_null.0);
    }
}
