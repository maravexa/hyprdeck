//! Hyprland IPC — the reactive foundation of HyprDeck.
//!
//! HyprDeck talks to Hyprland over two UNIX sockets:
//!
//! 1. **Event socket** (`.socket2.sock`) — persistent stream of newline
//!    delimited events, consumed by [`socket::start_event_listener`].
//! 2. **Command socket** (`.socket.sock`) — request/response, used for
//!    state hydration on startup and for dispatching user actions.
//!    See [`command`].
//!
//! Most callers should use [`HyprIpc`], which wires both sockets together,
//! owns the background reader task, exposes a shared
//! `Arc<RwLock<HyprState>>`, and hands out broadcast subscriptions for
//! the typed event stream.
//!
//! # Example
//!
//! ```no_run
//! # async fn example() -> Result<(), hyprdeck_core::ipc::IpcError> {
//! use hyprdeck_core::ipc::HyprIpc;
//!
//! let ipc = HyprIpc::connect().await?;
//! let state = ipc.state();
//! let mut rx = ipc.subscribe();
//! while let Ok(event) = rx.recv().await {
//!     println!("Hyprland event: {event:?}");
//!     let snapshot = state.read().await;
//!     println!("Active workspace: {}", snapshot.active_workspace);
//! }
//! # Ok(()) }
//! ```

pub mod command;
pub mod event;
pub mod socket;

use std::path::PathBuf;
use std::sync::Arc;

use thiserror::Error;
use tokio::sync::{RwLock, broadcast};
use tokio::task::JoinHandle;
use tracing::info;

pub use command::CommandClient;
pub use event::{
    HyprEvent, HyprState, MonitorInfo, WindowInfo, Workspace, WorkspaceRef, parse_event,
};
pub use socket::{EventSocket, start_event_listener};

/// Errors returned by the IPC layer.
#[derive(Debug, Error)]
pub enum IpcError {
    /// `$HYPRLAND_INSTANCE_SIGNATURE` was not set. HyprDeck only runs
    /// under an active Hyprland session.
    #[error("HYPRLAND_INSTANCE_SIGNATURE not set — is Hyprland running?")]
    NoHyprland,

    /// `$XDG_RUNTIME_DIR` was not set. This is required to locate the
    /// Hyprland sockets.
    #[error("XDG_RUNTIME_DIR not set")]
    NoXdgRuntime,

    /// Failed to open a UNIX stream at the given socket path.
    #[error("failed to connect to Hyprland socket at {path}: {source}", path = path.display())]
    Connect {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Low-level socket I/O error after the connection was established.
    #[error("socket I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Failed to deserialize a Hyprland command socket response.
    #[error("failed to deserialize Hyprland response: {0}")]
    Deserialize(#[from] serde_json::Error),

    /// Hyprland's command socket returned an error string.
    #[error("Hyprland command failed: {0}")]
    CommandFailed(String),
}

/// The main IPC handle.
///
/// Owns the background event-socket reader task and provides access to
/// the shared [`HyprState`] snapshot plus a broadcast subscription API
/// for the typed event stream.
pub struct HyprIpc {
    state: Arc<RwLock<HyprState>>,
    event_tx: broadcast::Sender<HyprEvent>,
    command: CommandClient,
    // Kept alive so the background task is aborted when `HyprIpc` drops.
    _event_task: JoinHandle<()>,
}

impl HyprIpc {
    /// Connect to Hyprland, hydrate initial state, and start the
    /// background event listener.
    ///
    /// Returns an error if Hyprland is not running or if the initial
    /// state query fails. Once this returns, the event listener is
    /// running and will reconnect automatically on failure.
    pub async fn connect() -> Result<Self, IpcError> {
        let command_path = command_socket_path()?;
        let event_path = event_socket_path()?;

        info!(
            command = %command_path.display(),
            event = %event_path.display(),
            "Connecting to Hyprland IPC",
        );

        // Hydrate state up front so the first subscriber sees a fully
        // populated snapshot, not an empty default.
        let initial_state = command::hydrate_state(&command_path).await?;
        let state = Arc::new(RwLock::new(initial_state));

        let (event_tx, event_task) =
            socket::start_event_listener(event_path, command_path.clone(), Arc::clone(&state))
                .await?;

        Ok(HyprIpc {
            state,
            event_tx,
            command: CommandClient::new(command_path),
            _event_task: event_task,
        })
    }

    /// Get a shared handle to the aggregated state.
    ///
    /// Modules should hold this `Arc` and take read locks in their
    /// `update()` implementation. Writes are performed only by the
    /// background event task.
    pub fn state(&self) -> Arc<RwLock<HyprState>> {
        Arc::clone(&self.state)
    }

    /// Subscribe to the live event stream.
    ///
    /// New subscribers receive events from the moment of subscription
    /// — historical events are not replayed. Subscribers that fall
    /// behind the buffer will see
    /// [`broadcast::error::RecvError::Lagged`] and should fall back to
    /// reading the shared state directly.
    pub fn subscribe(&self) -> broadcast::Receiver<HyprEvent> {
        self.event_tx.subscribe()
    }

    /// Borrow the [`CommandClient`] for dispatching actions or running
    /// ad-hoc queries.
    pub fn command(&self) -> &CommandClient {
        &self.command
    }
}

impl std::fmt::Debug for HyprIpc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HyprIpc")
            .field("command", &self.command)
            .finish_non_exhaustive()
    }
}

/// Resolve the path to Hyprland's command socket (`.socket.sock`).
pub fn command_socket_path() -> Result<PathBuf, IpcError> {
    socket_path(".socket.sock")
}

/// Resolve the path to Hyprland's event socket (`.socket2.sock`).
pub fn event_socket_path() -> Result<PathBuf, IpcError> {
    socket_path(".socket2.sock")
}

fn socket_path(name: &str) -> Result<PathBuf, IpcError> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").ok_or(IpcError::NoXdgRuntime)?;
    let signature = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").ok_or(IpcError::NoHyprland)?;
    let mut path = PathBuf::from(runtime);
    path.push("hypr");
    path.push(signature);
    path.push(name);
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_path_no_env_returns_error() {
        // SAFETY: env manipulation in tests. Stash and restore.
        let prev_runtime = std::env::var_os("XDG_RUNTIME_DIR");
        let prev_sig = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE");
        // SAFETY: single-threaded test context.
        unsafe {
            std::env::remove_var("XDG_RUNTIME_DIR");
            std::env::remove_var("HYPRLAND_INSTANCE_SIGNATURE");
        }

        assert!(matches!(command_socket_path(), Err(IpcError::NoXdgRuntime)));

        // SAFETY: restoring prior env.
        unsafe {
            if let Some(v) = prev_runtime {
                std::env::set_var("XDG_RUNTIME_DIR", v);
            }
            if let Some(v) = prev_sig {
                std::env::set_var("HYPRLAND_INSTANCE_SIGNATURE", v);
            }
        }
    }
}
