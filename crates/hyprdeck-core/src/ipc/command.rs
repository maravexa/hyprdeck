use std::path::PathBuf;

use super::event::HyprState;

type HyprResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// One-shot client for Hyprland's command socket (`/.socket.sock`).
///
/// Used for initial state hydration and for dispatching user-triggered actions
/// (e.g. workspace switches, window focus changes).
pub struct CommandClient {
    socket_path: PathBuf,
}

impl CommandClient {
    /// Construct a client pointing at the given socket path.
    ///
    /// The path is typically `$XDG_RUNTIME_DIR/hypr/$HYPRLAND_INSTANCE_SIGNATURE/.socket.sock`.
    pub fn new(socket_path: PathBuf) -> Self {
        CommandClient { socket_path }
    }

    /// Build a [`HyprState`] snapshot by querying workspaces, windows, and monitors.
    pub async fn query_state(&self) -> HyprResult<HyprState> {
        todo!()
    }

    /// Send a raw `hyprctl dispatch` command string.
    pub async fn dispatch(&self, command: &str) -> HyprResult<()> {
        todo!()
    }

    /// Send a raw `hyprctl` request and return the response string.
    pub async fn request(&self, payload: &str) -> HyprResult<String> {
        todo!()
    }
}
