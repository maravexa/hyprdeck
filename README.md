# HyprDeck

HyprDeck is a Rust/Wayland panel, taskbar, and dock for Hyprland. It is an early `0.1.0` project: the current tree implements layer-shell panels, Hyprland IPC-backed modules, layouts, themes, and input, but its interfaces are still evolving.

## Requirements

- Hyprland on a Wayland session
- Rust toolchain with edition 2024 support
- `pkg-config`, Wayland and xkbcommon development headers, and installed system fonts
- Optional command-line tools used by enabled modules: `wofi` (the default menu action), `wpctl`, `pactl`, or `amixer` (sound), and `hyprlock`/`systemctl` (power defaults)

## Build and install from source

```sh
git clone https://github.com/maravexa/hyprdeck
cd hyprdeck
cargo build --release
sudo install -Dm755 target/release/hyprdeck /usr/local/sbin/hyprdeck
```

`install.sh` performs those same build and install steps. Create `$XDG_CONFIG_HOME/hypr/hyprdeck.toml`; if `XDG_CONFIG_HOME` is unset, HyprDeck uses `$HOME/.config/hypr/hyprdeck.toml`.

```toml
theme = "win7"

[modules.clock]
format = "%H:%M"
```

Start it with `hyprdeck`, or add `exec-once = hyprdeck` to `hyprland.conf`.

## Built-in components

Themes: `gnome_classic`, `gnome_left`, `macos_dock`, `win7`, and `winxp`.

Modules: `calendar`, `clock`, `favorites`, `lunar`, `menu_button`, `network`, `power`, `shell`, `sound`, `weather`, `window_list`, and `workspaces`.

See the [configuration guide](docs/configuration.md), [module guide](docs/modules.md), [theme guide](docs/themes.md), and [known limitations](docs/known-limitations.md). The project is MIT licensed; see [LICENSE](LICENSE).
