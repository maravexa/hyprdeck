use tokio::net::UnixStream;

use super::event::HyprEvent;

type HyprResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Persistent connection to Hyprland's event socket (`/.socket2.sock`).
///
/// Drives the reactive event loop: call [`Self::next_event`] in a `tokio::select!`
/// arm alongside render ticks and timer callbacks.
pub struct EventSocket {
    stream: UnixStream,
}

impl EventSocket {
    /// Connect to Hyprland's event socket.
    ///
    /// Reads `$HYPRLAND_INSTANCE_SIGNATURE` from the environment to locate the socket.
    pub async fn connect() -> HyprResult<Self> {
        todo!()
    }

    /// Read and parse the next event from the socket.
    ///
    /// Blocks until an event is available.  Returns an error on socket disconnect.
    pub async fn next_event(&mut self) -> HyprResult<HyprEvent> {
        todo!()
    }
}
