//! Per-user single-instance control channel.

use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

const REFRESH_COMMAND: &[u8] = b"refresh\n";

/// Result of attempting to claim HyprDeck's runtime control socket.
pub enum InstanceClaim {
    /// This process is the primary instance and owns the listener.
    Primary(InstanceControl),
    /// An existing process accepted a refresh request; this process should exit.
    RefreshedExisting,
}

/// Runtime listener owned for the lifetime of the primary HyprDeck process.
pub struct InstanceControl {
    listener: UnixListener,
    path: PathBuf,
}

impl InstanceControl {
    /// Claim the default per-user control socket or refresh its current owner.
    pub async fn claim_default() -> io::Result<InstanceClaim> {
        let runtime = std::env::var_os("XDG_RUNTIME_DIR").ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "XDG_RUNTIME_DIR is not set; cannot enforce single-instance operation",
            )
        })?;
        Self::claim_at(PathBuf::from(runtime).join("hyprdeck/control.sock")).await
    }

    async fn claim_at(path: PathBuf) -> io::Result<InstanceClaim> {
        let parent = path.parent().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "control socket has no parent")
        })?;
        fs::create_dir_all(parent)?;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;

        match UnixListener::bind(&path) {
            Ok(listener) => Self::primary(listener, path),
            Err(error) if error.kind() == io::ErrorKind::AddrInUse => {
                match UnixStream::connect(&path).await {
                    Ok(mut stream) => {
                        stream.write_all(REFRESH_COMMAND).await?;
                        stream.shutdown().await?;
                        Ok(InstanceClaim::RefreshedExisting)
                    }
                    Err(connect_error)
                        if matches!(
                            connect_error.kind(),
                            io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
                        ) =>
                    {
                        // The previous process did not shut down cleanly and left
                        // its filesystem entry behind. Remove only this exact socket.
                        match fs::remove_file(&path) {
                            Ok(()) => {}
                            Err(remove_error) if remove_error.kind() == io::ErrorKind::NotFound => {
                            }
                            Err(remove_error) => return Err(remove_error),
                        }
                        let listener = UnixListener::bind(&path)?;
                        Self::primary(listener, path)
                    }
                    Err(connect_error) => Err(connect_error),
                }
            }
            Err(error) => Err(error),
        }
    }

    fn primary(listener: UnixListener, path: PathBuf) -> io::Result<InstanceClaim> {
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        Ok(InstanceClaim::Primary(Self { listener, path }))
    }

    /// Wait for and validate one refresh request.
    pub async fn receive_refresh(&self) -> io::Result<bool> {
        let (mut stream, _) = self.listener.accept().await?;
        let mut buffer = [0_u8; 64];
        let read = tokio::time::timeout(Duration::from_millis(500), stream.read(&mut buffer))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "control request timed out"))??;
        Ok(buffer[..read].trim_ascii() == b"refresh")
    }
}

impl Drop for InstanceControl {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn socket_path(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!(
                "hyprdeck-instance-{name}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ))
            .join("control.sock")
    }

    #[tokio::test]
    async fn second_claim_refreshes_primary_instead_of_becoming_an_instance() {
        let path = socket_path("refresh");
        let primary = match InstanceControl::claim_at(path.clone()).await.unwrap() {
            InstanceClaim::Primary(control) => control,
            InstanceClaim::RefreshedExisting => panic!("first claim must become primary"),
        };

        assert!(matches!(
            InstanceControl::claim_at(path.clone()).await.unwrap(),
            InstanceClaim::RefreshedExisting
        ));
        assert!(primary.receive_refresh().await.unwrap());

        drop(primary);
        let _ = fs::remove_dir(path.parent().unwrap());
    }

    #[tokio::test]
    async fn stale_socket_is_reclaimed() {
        let path = socket_path("stale");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let stale = std::os::unix::net::UnixListener::bind(&path).unwrap();
        drop(stale);

        let primary = match InstanceControl::claim_at(path.clone()).await.unwrap() {
            InstanceClaim::Primary(control) => control,
            InstanceClaim::RefreshedExisting => panic!("stale socket must be reclaimed"),
        };
        drop(primary);
        let _ = fs::remove_dir(path.parent().unwrap());
    }

    #[test]
    fn socket_parent_is_private() {
        let path = socket_path("permissions");
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let primary = match runtime
            .block_on(InstanceControl::claim_at(path.clone()))
            .unwrap()
        {
            InstanceClaim::Primary(control) => control,
            InstanceClaim::RefreshedExisting => panic!("first claim must become primary"),
        };
        let mode = fs::metadata(path.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o700);
        drop(primary);
        let _ = fs::remove_dir(path.parent().unwrap());
    }
}
