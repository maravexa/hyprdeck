# HyprDeck

**A modular, themeable panel / taskbar / dock for Hyprland** — ships opinionated, ready-to-use themes so you can be up and running in minutes.

[![CI](https://github.com/maravexa/hyprdeck/actions/workflows/ci.yml/badge.svg)](https://github.com/maravexa/hyprdeck/actions/workflows/ci.yml)
![Status](https://img.shields.io/badge/status-WIP-orange)
![License](https://img.shields.io/badge/license-MIT-blue)

---

<!-- Screenshot placeholder -->
> **Screenshot coming once rendering is implemented.**

---

## Philosophy

Most status bars hand you a blank canvas and a DSL to configure every pixel.
HyprDeck takes the opposite approach: **ship great themes, let users fork them.**

Pick a shipped theme, set your accent colour, done.  Power users can copy a theme directory and tweak TOML until it looks exactly right.  There is no Lua, no JavaScript, no proprietary scripting language.

---

## Shipped Themes

| Theme | Edge | Description |
|-------|------|-------------|
| `win7` | Bottom | Windows 7 Aero glass taskbar (height 40, translucent) |
| `winxp` | Bottom | Windows XP chunky taskbar (height 30, solid teal) |
| `macos_dock` | Bottom | Floating dock with icon magnification |
| `gnome_classic` | Top + Bottom | Classic GNOME 2 dual-panel layout |
| `gnome_left` | Left | Vertical left sidebar, icon-only |

---

## Available Modules

| ID | Description |
|----|-------------|
| `calendar` | Month calendar pop-up, supports Gregorian and Discordian |
| `clock` | Digital clock with configurable `strftime` format |
| `favorites` | Pinned application launchers with running indicators |
| `lunar` | Lunar phase icon (powered by `fn0rd`, shared with HyprSaver) |
| `menu_button` | Application menu / start button |
| `network` | Wi-Fi / ethernet indicator with optional rate display |
| `shell` | Display the stdout of any shell command on an interval |
| `weather` | Current temperature and condition via Open-Meteo |
| `window_list` | Taskbar-style list of open windows |
| `workspaces` | Hyprland workspace switcher with urgent highlighting |

---

## Quick Start

### Install

```sh
# Build from source (Rust 1.85+ required for edition 2024)
git clone https://github.com/maravexa/hyprdeck
cd hyprdeck
cargo build --release
# Copy binary to PATH
install -Dm755 target/release/hyprdeck ~/.local/bin/hyprdeck
```

### Installation

```sh
# AUR (Arch / Arch-based)
yay -S hyprdeck
```

For other distributions, build from source as shown above.

### Configure

Create `~/.config/hyprdeck/config.toml`:

```toml
# Select a shipped theme
theme = "win7"

# Optional: override individual style properties without forking the theme
[theme_overrides]
accent_color = "#ff6a00"
font_family  = "Inter"

# Optional: per-module configuration
[modules.clock]
format = "%H:%M:%S"

[modules.weather]
unit             = "celsius"
refresh_minutes  = 30

[modules.workspaces]
hide_empty = true
```

### Run

```sh
# Start manually
hyprdeck

# Or add to hyprland.conf for autostart
exec-once = hyprdeck
```

---

## Built-in Shaders

20 shaders ship out of the box:

bezier, caustics, donut, fire, geometry, hypercube, julia, kaleidoscope, lissajous, mandelbrot, marble, matrix, network, planet, plasma, snowfall, starfield, tesla, tunnel, voronoi

Use `shader = "cycle"` to rotate through all shaders automatically.

### Palettes

Built-in palettes: rainbow, vaporwave (and more). Use `palette = "cycle"` to rotate.

### Performance

See [`docs/BENCHMARK-0.4.0.md`](docs/BENCHMARK-0.4.0.md) for per-shader GPU utilization benchmarks on a GMKtec Nucbox K12 (AMD ATI HawkPoint1), dual 1920×1200 monitors.

---

## Architecture

HyprDeck is a Cargo workspace with four crates:

```
hyprdeck-core       ← traits, type system, layout engines, Hyprland IPC, theme engine
      ↑
hyprdeck-modules    ← built-in module implementations (PanelModule trait)
      ↑
hyprdeck-themes     ← theme loading, validation, embedded defaults (include_dir)
      ↑
hyprdeck (bin)      ← main event loop, Wayland surface management
```

Key libraries:
- **Smithay Client Toolkit** — Wayland `wlr-layer-shell` surface management
- **tiny-skia** — software 2-D rendering (paths, fills, blending)
- **cosmic-text** — text shaping with CJK, ligature, and font-fallback support
- **tokio** — async runtime driving the event loop, IPC, and network requests

---

## HyprCube and HyprSaver Integration

HyprDeck is part of a trio of Hyprland-native tools:

- **HyprCube** *(planned)* — GUI system settings panel.  HyprDeck modules implement `config_schema()` so HyprCube can auto-generate a settings UI for each module without any extra glue code.
- **HyprSaver** *(in development)* — screensaver for Hyprland.  Shares theme colour palettes and the `fn0rd` lunar-phase library with HyprDeck.

---

## Roadmap

### 1.0

- [ ] Full Wayland surface management (layer shell, exclusive zones, multi-output)
- [ ] All built-in module rendering implemented
- [ ] Auto-hide with smooth animation (disabled, auto-hide, dodge-active, dock-hover)
- [ ] Dock layout with icon magnification
- [ ] Theme hot-reload on file change
- [ ] Hyprland IPC (event socket + command socket)
- [ ] HiDPI / fractional scale support

### v0.5.0

- [ ] Wormhole shader (fundamental rewrite from v0.4.0 deferral)
- [ ] Further heavy shader optimizations (target: bring 70%-tier shaders below 60%)
- [x] ~~Screencopy pipeline~~ (moved to v0.5.0)
- [x] ~~Rain-on-Glass shader~~ (moved to v0.5.0)

### Post-1.0

- [ ] DBus system tray (StatusNotifierItem)
- [ ] Audio volume module (PipeWire)
- [ ] Bluetooth status module
- [ ] SVG icon rendering (resvg)
- [ ] GPU-accelerated rendering (wgpu back-end)
- [ ] Non-rectangular display support (curved/circular panels)
- [ ] HyprCube integration (live config editing)

---

## License

MIT — see [LICENSE](LICENSE).
