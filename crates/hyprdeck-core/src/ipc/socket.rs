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

use std::path::{Path, PathBuf};
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
/// Coalesce event bursts before correcting the optimistic socket state from
/// Hyprland's command socket. This bounds query traffic while ensuring a bad
/// or missed event cannot desynchronise panels permanently.
const RECONCILE_INTERVAL: Duration = Duration::from_millis(500);

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
    let mut reconcile_tick = tokio::time::interval(RECONCILE_INTERVAL);
    reconcile_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut reconcile_needed = false;
    // Consume interval's immediate first tick so the first refresh is caused
    // by actual state activity, not merely connecting.
    reconcile_tick.tick().await;
    let mut line = Vec::new();

    loop {
        // `read_until` is cancellation-safe, unlike `read_line`; the timer
        // branch below keeps this buffer so it can resume a partial event
        // without losing bytes while coalescing a reconciliation refresh.
        tokio::select! {
            read = reader.read_until(b'\n', &mut line) => match read {
                Ok(0) => {
                    warn!("Hyprland event socket closed (EOF); reconnecting");
                }
                Ok(_) => {
                    // Successful read resets the backoff.
                    backoff = INITIAL_BACKOFF;
                    let raw_line = String::from_utf8_lossy(&line);
                    if let Some(event) = parse_event(&raw_line) {
                        reconcile_needed |= event.requires_reconciliation();
                        {
                            let mut guard = state.write().await;
                            guard.apply_event(&event);
                        }
                        // Broadcast failure just means no subscribers — not
                        // an error we should log loudly.
                        let _ = tx.send(event);
                    } else if !raw_line.trim().is_empty() {
                        // An unsupported or malformed line can still represent
                        // state HyprDeck needs. Reconcile rather than leaving a
                        // stale taskbar/workspace model forever.
                        reconcile_needed = true;
                    }
                    line.clear();
                    continue;
                }
                Err(err) => {
                    warn!(error = %err, "Hyprland event socket read failed; reconnecting");
                    line.clear();
                }
            },
            _ = reconcile_tick.tick(), if reconcile_needed => {
                match command::hydrate_state(&command_socket_path).await {
                    Ok(fresh) => {
                        let mut guard = state.write().await;
                        guard.reconcile_authoritative(fresh);
                        reconcile_needed = false;
                        debug!("HyprState reconciled after event activity");
                    }
                    Err(err) => {
                        // Keep the dirty marker so the next interval retries.
                        warn!(error = %err, "Failed to reconcile HyprState after event activity");
                    }
                }
                continue;
            }
        }

        // Reconnect with exponential backoff, re-hydrating state on success.
        line.clear();
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
        reconcile_needed = false;
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
    event_socket_path: &Path,
    command_socket_path: &Path,
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
                        guard.reconcile_authoritative(fresh);
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
