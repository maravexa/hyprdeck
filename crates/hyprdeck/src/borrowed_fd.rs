//! Readiness registration that cannot close or replace its borrowed descriptor.

use std::io;
use std::os::fd::BorrowedFd;
use tokio::io::unix::{AsyncFd, AsyncFdReadyGuard};

pub(crate) struct BorrowedAsyncFd<'fd> {
    inner: AsyncFd<BorrowedFd<'fd>>,
}

impl<'fd> BorrowedAsyncFd<'fd> {
    pub(crate) fn new(fd: BorrowedFd<'fd>) -> io::Result<Self> {
        // SAFETY: BorrowedFd keeps the descriptor's owner borrowed for 'fd,
        // so the descriptor stays valid and open until registration is dropped.
        // BorrowedFd always returns the same descriptor. `inner` is private and
        // we expose no mutable access, replacement, or ownership-transfer API.
        // AsyncFd deregisters before dropping its inner borrow; neither closes
        // the owner's descriptor. Callers must already satisfy BorrowedFd's
        // contract (including any unsafe code used to construct that borrow).
        let inner = unsafe { AsyncFd::register(fd) }.map_err(io::Error::from)?;
        Ok(Self { inner })
    }

    pub(crate) async fn readable(&self) -> io::Result<AsyncFdReadyGuard<'_, BorrowedFd<'fd>>> {
        self.inner.readable().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::os::fd::AsFd;
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    #[tokio::test]
    async fn readiness_can_be_cleared_and_rearmed() -> io::Result<()> {
        let (mut socket, mut peer) = UnixStream::pair()?;
        socket.set_nonblocking(true)?;
        let registered = BorrowedAsyncFd::new(socket.as_fd())?;
        for byte in [b'a', b'b'] {
            peer.write_all(&[byte])?;
            let mut guard = tokio::time::timeout(Duration::from_secs(2), registered.readable())
                .await
                .expect("socket must become readable")?;
            let mut received = [0];
            // Reading through a shared reference preserves the owner's borrow.
            (&socket).read_exact(&mut received)?;
            assert_eq!(received, [byte]);
            guard.clear_ready();
        }
        drop(registered);
        peer.write_all(b"c")?;
        let mut received = [0];
        socket.read_exact(&mut received)?;
        assert_eq!(received, [b'c']);
        Ok(())
    }

    #[tokio::test]
    async fn dropping_registration_preserves_socket_and_allows_reregistration() -> io::Result<()> {
        let (socket, mut peer) = UnixStream::pair()?;
        socket.set_nonblocking(true)?;
        for byte in [b'x', b'y'] {
            let registered = BorrowedAsyncFd::new(socket.as_fd())?;
            peer.write_all(&[byte])?;
            let _guard = tokio::time::timeout(Duration::from_secs(2), registered.readable())
                .await
                .expect("reregistered socket must become readable")?;
            let mut received = [0];
            (&socket).read_exact(&mut received)?;
            assert_eq!(received, [byte]);
        }
        Ok(())
    }

    #[tokio::test]
    async fn peer_disconnect_remains_readable_eof() -> io::Result<()> {
        let (socket, peer) = UnixStream::pair()?;
        socket.set_nonblocking(true)?;
        let registered = BorrowedAsyncFd::new(socket.as_fd())?;
        drop(peer);
        let _guard = tokio::time::timeout(Duration::from_secs(2), registered.readable())
            .await
            .expect("peer shutdown must wake the registration")?;
        assert_eq!((&socket).read(&mut [0])?, 0);
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn registration_error_does_not_close_owner() -> io::Result<()> {
        use std::os::unix::fs::FileExt;
        let file = std::fs::File::open("/dev/null")?;
        assert!(BorrowedAsyncFd::new(file.as_fd()).is_err());
        assert_eq!(file.read_at(&mut [0], 0)?, 0);
        Ok(())
    }
}
