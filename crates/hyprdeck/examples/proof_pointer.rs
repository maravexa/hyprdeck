//! One pointer click on the disposable compositor created by Proof B.

use std::io::{self, Read};
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{wl_pointer::ButtonState, wl_registry::WlRegistry};
use wayland_client::{Connection, Dispatch, QueueHandle, delegate_noop};
use wayland_protocols_wlr::virtual_pointer::v1::client::{
    zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1,
    zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1,
};

struct PointerFixture;

delegate_noop!(PointerFixture: ignore ZwlrVirtualPointerManagerV1);
delegate_noop!(PointerFixture: ignore ZwlrVirtualPointerV1);

impl Dispatch<WlRegistry, GlobalListContents> for PointerFixture {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: wayland_client::protocol::wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let socket = PathBuf::from(args.next().ok_or("missing fixture socket")?);
    let x = args.next().ok_or("missing pixel x")?.parse::<u32>()?;
    let runtime = socket.parent().ok_or("missing runtime directory")?;
    let scratch = runtime.parent().ok_or("missing fixture directory")?;
    if scratch.parent() != Some(std::path::Path::new("/tmp"))
        || !scratch
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with("hd."))
        || runtime.metadata()?.permissions().mode() & 0o077 != 0
        || !runtime.join("proof-pointer").is_file()
        || socket.canonicalize()? != socket
        || !socket.metadata()?.file_type().is_socket()
        || !(1..1024).contains(&x)
    {
        return Err("expected the private Proof B runtime and a fixture coordinate".into());
    }
    let connection = Connection::from_socket(UnixStream::connect(socket)?)?;
    let (globals, mut queue) = registry_queue_init::<PointerFixture>(&connection)?;
    let handle = queue.handle();
    let manager: ZwlrVirtualPointerManagerV1 = globals.bind(&handle, 1..=1, ())?;
    let pointer = manager.create_virtual_pointer(None, &handle, ());
    queue.roundtrip(&mut PointerFixture)?;
    // Keep the seat active while the panel registers its pointer capability.
    let mut control = [0];
    io::stdin().read_exact(&mut control)?;
    if control != *b"m" {
        return Err("expected fixture motion command".into());
    }
    pointer.motion_absolute(0, x, 16, 1024, 768);
    pointer.frame();
    queue.roundtrip(&mut PointerFixture)?;

    io::stdin().read_exact(&mut control)?;
    if control != *b"c" {
        return Err("invalid fixture control byte".into());
    }
    pointer.button(1, 0x110, ButtonState::Pressed);
    pointer.frame();
    pointer.button(2, 0x110, ButtonState::Released);
    pointer.frame();
    queue.roundtrip(&mut PointerFixture)?;
    // Keep the virtual seat until the controller verifies the compositor result.
    let _ = io::stdin().read(&mut control)?;
    Ok(())
}
