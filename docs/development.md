# Development

## Prerequisites

HyprDeck builds on Linux. Install Rust 1.89.0 or newer (the CI MSRV is 1.89.0)
with the `rustfmt` and `clippy` components. The native build dependencies used
by CI on Debian/Ubuntu are:

```sh
sudo apt-get install libwayland-dev libwayland-client0 libxkbcommon-dev pkg-config
```

Source builds and all workspace crates require Rust 1.89.0 or newer, including
`hyprdeck-config` consumers. If the distribution compiler is older, use a
compatible Rust toolchain before building; prebuilt binaries do not require
a Rust compiler. This does not change the native runtime dependencies.

Equivalent development packages are required on other distributions. The
application uses Wayland client, XKB, and layer-shell APIs; a normal build and
unit-test run do not require a live compositor.

## Cargo commands

Run these at the workspace root:

```sh
cargo check --workspace
cargo build --workspace
cargo build --workspace --release
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --workspace --doc
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
npx --yes markdownlint-cli2@0.23.2 "**/*.md" "#target"
```

To focus on a package, use `-p`, for example:

```sh
cargo test -p hyprdeck-core
cargo test -p hyprdeck-modules
cargo test -p hyprdeck --test version
```

The workspace tests cover core configuration, state, layout, rendering helpers,
IPC parsing and command handling, module behaviour, style resolution, and the
binary's version flags. They do not exercise a real Wayland compositor,
layer-shell negotiation, input delivery, or a live Hyprland socket.

CI also uses Lychee in offline mode to validate local Markdown links and
fragments. If `lychee` is installed locally, run:

```sh
lychee --offline --include-fragments --root-dir "$PWD" './**/*.md'
```

## Isolated notification D-Bus regression

```sh
cargo test -p hyprdeck --test notification_dbus --locked
```

This requires `dbus-run-session` and `gdbus` (`dbus-daemon` and `libglib2.0-bin`
on Debian/Ubuntu) and launches a private bus, a production
notification-service bridge, and a separate client connection. It verifies
capabilities, server information, notification creation and replacement,
actions, compound image hints, timeout values, invalid signatures, and close
requests over the real wire protocol. An independent GLib client verifies external
wire compatibility, and a second service must fail to claim the occupied name.
It does not connect to the desktop bus or start Wayland. Run the normal wrapper; the ignored child test is reserved
for that wrapper's isolated subprocess.

The workspace uses zbus 6 for the service, wire values, names and derives; do not
reintroduce the unmaintained standalone zvariant family or mixed derive pins.
Its Rust 1.89 requirement matches the workspace minimum. The explicit Tokio,
service, proxy and tracing features preserve the async runtime and diagnostic
behavior. The `unixexec` feature retains the transport accepted by zbus 5; the
separate default runtime and the new `ibus` transport remain disabled.
Run this regression on the minimum compiler as well as the development compiler
when changing that dependency. It does not establish notification rendering,
click handling, signal support, or live compositor acceptance.

## Renderer migration checks

The text tests use a bundled, licensed DejaVu Sans font so metrics and pixels
can be compared independently of host fonts. The fixtures cover Latin text,
ligatures, Arabic, combining marks, missing-glyph fallback, alignment, empty
text, wrapping and ellipsis. They do not validate every installed font or CJK
font fallback. PNG and palette-PNG fixtures plus PNG/BMP-backed ICO fixtures
check decoding and alpha handling.

The generic fallback families remain Fira Sans, Fira Mono and DejaVu Serif.
Named theme fonts, manual alignment, popup scale and configuration are unchanged.
The HarfRust/Swash migration can still change font matching and anti-aliasing;
fixed-font comparisons found unchanged measured widths and ink bounds, with
small rasterization differences. A live Hyprland comparison is still required.

## Logging

The binary uses `tracing`. By default it enables `hyprdeck=info` and
`hyprdeck_core=info`. Set `RUST_LOG` to make either target more verbose:

```sh
RUST_LOG=hyprdeck=debug,hyprdeck_core=trace cargo run -p hyprdeck
```

## Live Hyprland smoke test

A live run must be started from a Hyprland Wayland session. It needs a valid
`WAYLAND_DISPLAY`, Hyprland's `XDG_RUNTIME_DIR` and
`HYPRLAND_INSTANCE_SIGNATURE`, and a configuration file at
`$XDG_CONFIG_HOME/hypr/hyprdeck.toml` (or
`~/.config/hypr/hyprdeck.toml`). The configuration selects a loadable theme.

```sh
cargo run -p hyprdeck
```

Confirm that panels appear on the intended outputs, module updates react to
workspace/window changes, and panel/pop-up input works. Then test monitor
hot-plug if available. This is a manual smoke test: CI cannot validate it on
its headless Ubuntu runners, and no hot reload exists, so restart the process
after changing configuration or theme files.

For project workflow and mandatory pull-request checks, see
[Contributing](../CONTRIBUTING.md).
