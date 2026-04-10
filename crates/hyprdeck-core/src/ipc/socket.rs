//! Persistent reader for Hyprland's event socket (`.socket2.sock`).
//!
//! Connects to the event socket, reads newline-delimited events, updates
//! the shared [`HyprState`] under a write lock, and broadcasts the typed
//! [`HyprEvent`] to any subscribers.
//!
//! On disconnect, the reader loop re-hydrates state via the command socket
//! and reconnects with exponential backoff. This is silent to the rest of
//! the application — modules hold an `Arc<RwLock<HyprState>>` and simply
//! see state snap back into place when Hyprland comes back.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::{RwLock, broadcast};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use super::IpcError;
use super::command;
use super::event::{HyprEvent, HyprState, parse_event};

/// Size of the broadcast channel used for events.
///
/// Slow receivers lose old events once the buffer is full. That's fine:
/// modules can always recover by re-reading the shared [`HyprState`],
/// which is updated by the reader before each broadcast.
const EVENT_CHANNEL_CAPACITY: usize = 64;

/// Lower bound for the reconnect backoff, doubled on each failure.
const INITIAL_BACKOFF: Duration = Duration::from_secs(1);
/// Upper bound for the reconnect backoff.
const MAX_BACKOFF: Duration = Duration::from_secs(30);

/// Persistent connection to Hyprland's event socket.
///
/// This is a thin legacy wrapper around the reader task spawned by
/// [`start_event_listener`], kept around so callers that want a
/// pull-based API can still use `next_event()`. Most code should prefer
/// [`crate::ipc::HyprIpc`] instead.
pub struct EventSocket {
    reader: BufReader<UnixStream>,
}

impl EventSocket {
    /// Connect to Hyprland's event socket.
    ///
    /// Resolves the socket path from `$XDG_RUNTIME_DIR` and
    /// `$HYPRLAND_INSTANCE_SIGNATURE`.
    pub async fn connect() -> Result<Self, IpcError> {
        let path = super::event_socket_path()?;
        let stream = UnixStream::connect(&path)
            .await
            .map_err(|source| IpcError::Connect { path, source })?;
        Ok(EventSocket {
            reader: BufReader::new(stream),
        })
    }

    /// Read and parse the next event from the socket.
    ///
    /// Blocks until a full line is available. Returns an error on socket
    /// disconnect. Lines that don't parse cleanly are skipped and logged at
    /// debug level.
    pub async fn next_event(&mut self) -> Result<HyprEvent, IpcError> {
        let mut line = String::new();
        loop {
            line.clear();
            let n = self.reader.read_line(&mut line).await?;
            if n == 0 {
                return Err(IpcError::Io(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "Hyprland event socket closed",
                )));
            }
            if let Some(event) = parse_event(&line) {
                return Ok(event);
            }
        }
    }
}

/// Connect to the event socket and spawn a background reader task.
///
/// Returns the broadcast [`Sender`] and the [`JoinHandle`] for the task.
/// The task owns the reader loop, including state updates and automatic
/// reconnection with exponential backoff.
///
/// The caller is expected to hold onto the `JoinHandle` (or a
/// [`crate::ipc::HyprIpc`] wrapping it) — dropping it aborts the task.
///
/// [`Sender`]: broadcast::Sender
pub async fn start_event_listener(
    event_socket_path: PathBuf,
    command_socket_path: PathBuf,
    state: Arc<RwLock<HyprState>>,
) -> Result<(broadcast::Sender<HyprEvent>, JoinHandle<()>), IpcError> {
    // Open the initial connection before returning so the caller sees any
    // immediate errors (e.g., socket doesn't exist) synchronously.
    let initial_stream = UnixStream::connect(&event_socket_path)
        .await
        .map_err(|source| IpcError::Connect {
            path: event_socket_path.clone(),
            source,
        })?;

    let (tx, _rx) = broadcast::channel::<HyprEvent>(EVENT_CHANNEL_CAPACITY);
    let tx_task = tx.clone();

    let handle = tokio::spawn(async move {
        reader_loop(
            initial_stream,
            event_socket_path,
            command_socket_path,
            state,
            tx_task,
        )
        .await;
    });

    Ok((tx, handle))
}

/// The main reader loop. Reads lines, updates state, broadcasts events,
/// and reconnects on failure.
async fn reader_loop(
    initial_stream: UnixStream,
    event_socket_path: PathBuf,
    command_socket_path: PathBuf,
    state: Arc<RwLock<HyprState>>,
    tx: broadcast::Sender<HyprEvent>,
) {
    let mut reader = BufReader::new(initial_stream);
    let mut backoff = INITIAL_BACKOFF;

    loop {
        let mut line = String::new();
        match reader.read_line(&mut line).await {
            Ok(0) => {
                warn!("Hyprland event socket closed (EOF); reconnecting");
            }
            Ok(_) => {
                // Successful read resets the backoff.
                backoff = INITIAL_BACKOFF;
                if let Some(event) = parse_event(&line) {
                    {
                        let mut guard = state.write().await;
                        guard.apply_event(&event);
                    }
                    // Broadcast failure just means no subscribers — not
                    // an error we should log loudly.
                    let _ = tx.send(event);
                }
                continue;
            }
            Err(err) => {
                warn!(error = %err, "Hyprland event socket read failed; reconnecting");
            }
        }

        // Reconnect with exponential backoff, re-hydrating state on success.
        reader = match reconnect(
            &event_socket_path,
            &command_socket_path,
            &state,
            &mut backoff,
        )
        .await
        {
            Some(stream) => BufReader::new(stream),
            None => {
                error!("Hyprland event listener giving up — task exiting");
                return;
            }
        };
    }
}

/// Attempt to reconnect to the event socket, sleeping with exponential
/// backoff between failures. On success, also re-hydrates [`HyprState`]
/// via the command socket before returning the new stream.
///
/// Returns `None` only on unrecoverable errors (currently: never — this
/// loop retries forever). The `None` branch is kept so callers don't
/// have to handle cancellation as a separate code path.
async fn reconnect(
    event_socket_path: &PathBuf,
    command_socket_path: &PathBuf,
    state: &Arc<RwLock<HyprState>>,
    backoff: &mut Duration,
) -> Option<UnixStream> {
    loop {
        tokio::time::sleep(*backoff).await;

        match UnixStream::connect(event_socket_path).await {
            Ok(stream) => {
                info!("Reconnected to Hyprland event socket");
                // Re-hydrate state so no events are missed during downtime.
                match command::hydrate_state(command_socket_path).await {
                    Ok(fresh) => {
                        let mut guard = state.write().await;
                        *guard = fresh;
                        debug!("HyprState re-hydrated after reconnect");
                    }
                    Err(err) => {
                        warn!(error = %err, "Failed to re-hydrate state after reconnect");
                    }
                }
                *backoff = INITIAL_BACKOFF;
                return Some(stream);
            }
            Err(err) => {
                warn!(
                    error = %err,
                    backoff_secs = backoff.as_secs(),
                    "Reconnect to Hyprland event socket failed",
                );
                *backoff = (*backoff * 2).min(MAX_BACKOFF);
            }
        }
    }
}
