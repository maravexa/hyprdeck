# HyprDeck

[![CI](https://github.com/maravexa/hyprdeck/actions/workflows/ci.yml/badge.svg)](https://github.com/maravexa/hyprdeck/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/hyprdeck)](https://crates.io/crates/hyprdeck)
[![AUR](https://img.shields.io/aur/version/hyprdeck)](https://aur.archlinux.org/packages/hyprdeck)

HyprDeck is a Rust/Wayland panel, taskbar, and dock for Hyprland. It is an early `0.1.0` project: the current tree implements layer-shell panels, Hyprland IPC-backed modules, layouts, themes, and input, but its interfaces are still evolving.

## Requirements

- Hyprland on a Wayland session
- Rust toolchain with edition 2024 support
- `pkg-config`, Wayland and xkbcommon development headers, and installed system fonts
- Optional command-line tools used by enabled modules: the command referenced by Hyprland's `$menu` variable (`wofi` is the fallback), `wpctl`, `pactl`, or `amixer` (sound), and `hyprlock`/`systemctl` (power defaults)

## Build and install from source

```sh
git clone https://github.com/maravexa/hyprdeck
cd hyprdeck
cargo build --release
sudo install -Dm755 target/release/hyprdeck /usr/local/bin/hyprdeck
```

`install.sh` performs those same build and install steps. Create `$XDG_CONFIG_HOME/hypr/hyprdeck.toml`; if `XDG_CONFIG_HOME` is unset, HyprDeck uses `$HOME/.config/hypr/hyprdeck.toml`.

```toml
theme = "win7"

[modules.clock]
format = "%H:%M"
```

Start it with `hyprdeck`, or add `exec-once = hyprdeck` to `hyprland.conf`.

After the initial release, the same binary will also be installable with
`cargo install hyprdeck`, from the AUR as `hyprdeck`, or from the `.deb`,
`.rpm`, and `.tar.zst` files attached to each GitHub release.

## Built-in components

Themes: `gnome_classic`, `gnome_left`, `macos_dock`, `win7`, and `winxp`.

Modules: `calendar`, `clock`, `favorites`, `hyprcube`, `lunar`, `menu_button`,
`network`, `power`, `shell`, `sound`, `weather`, `window_list`, and
`workspaces`. Application buttons resolve PNG and SVG desktop icons and fall
back to a readable application initial when no icon can be found. Module popup
panels render at twice their original logical size by default.

See the [configuration guide](docs/configuration.md), [module guide](docs/modules.md), [theme guide](docs/themes.md), and [known limitations](docs/known-limitations.md). The project is MIT licensed; see [LICENSE](LICENSE).
