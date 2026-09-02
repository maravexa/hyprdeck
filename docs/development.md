# Development

## Prerequisites

HyprDeck builds on Linux. Install Rust 1.85.0 or newer (the CI MSRV is 1.85.0)
with the `rustfmt` and `clippy` components. The native build dependencies used
by CI on Debian/Ubuntu are:

```sh
sudo apt-get install libwayland-dev libwayland-client0 libxkbcommon-dev pkg-config
```

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
