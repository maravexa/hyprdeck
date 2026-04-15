# CLAUDE.md — HyprDeck

Developer reference for AI-assisted work on this codebase.

---

## Project Overview

HyprDeck is a modular, themeable panel / taskbar / dock for the Hyprland Wayland
compositor.  It is a Cargo workspace with four crates, uses Smithay client toolkit
for Wayland integration, `tiny-skia` for software 2-D rendering, `cosmic-text` for
text shaping, and `tokio` for async.

The project is built for the author's own Hyprland setup; community adoption is a
welcome bonus, not a design constraint.

---

## Architecture

```
crates/
  hyprdeck-core/      traits, type system, layout engines, theme engine,
                      Hyprland IPC, action dispatcher, auto-hide state machine,
                      rendering abstraction
  hyprdeck-modules/   built-in modules implementing PanelModule trait
                      (calendar, weather, lunar, network, workspaces,
                       menu, favorites, shell, clock, window_list)
  hyprdeck-themes/    theme loading, validation, embedded defaults via include_dir
  hyprdeck/           binary crate — main event loop, wires everything together
```

**Dependency graph** (no cycles):

```
hyprdeck-core
    ↑
hyprdeck-modules
    ↑
hyprdeck-themes ← also depends on hyprdeck-core
    ↑
hyprdeck (bin)  ← depends on all three library crates
```

---

## Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| One process per output | Simplifies surface lifecycle; avoids cross-output locking |
| Themes are TOML data, not code | Users configure by forking a shipped theme; no scripting language |
| Hybrid Hyprland IPC | Persistent event socket (`socket2`) for reactivity; command socket (`socket`) for initial state hydration and dispatching |
| Unified `Action` enum | All user interactions (launch, dispatch, module-internal) share one serialisable type |
| `config_schema()` on every module | Future HyprCube integration can auto-generate settings UI without compile-time knowledge of modules |
| `DisplayGeometry` carries optional polygon data | Forward-compatible with circular/curved displays; `None` = standard rectangle |
| Layout engines are separate structs | `HorizontalLayout`, `VerticalLayout`, `DockLayout` are not unified behind an abstraction — simpler, less indirection |
| Software rendering (tiny-skia) | Avoids GPU context complexity at 1.0; wgpu is a viable future upgrade path |
| `cosmic-text` for all text | Handles CJK, ligatures, and font fallback correctly without bespoke shaping code |

---

## Build & Run

```sh
# Debug build
cargo build

# Release build
cargo build --release

# Run (requires a live Hyprland session)
cargo run -p hyprdeck

# Check only (no linking) — fast iteration
cargo check

# Run tests
cargo test
```

Minimum Rust version: **1.85** (Cargo edition 2024 support).

---

## Config Location

| Path | Purpose |
|------|---------|
| `~/.config/hypr/hyprdeck.toml` | User config (theme selection, overrides, module config) |
| `~/.config/hypr/hyprdeck/themes/<name>/theme.toml` | User theme override (takes precedence over embedded themes) |
| `themes/<name>/theme.toml` | Shipped/embedded themes (compiled in via `include_dir!`) |

---

## Coding Conventions

- All public types derive `Debug`.
- All config types derive `Deserialize` with `serde`; use `#[serde(default)]` on
  optional collection fields so missing TOML keys produce empty vecs/maps rather
  than parse errors.
- Use `tracing::{info, warn, error, debug, trace}` — never `println!` or `eprintln!`.
- Async via `tokio`; Hyprland IPC uses `tokio::net::UnixStream`.
- Module state mutations **only** in `update()` and `handle_event()` — `render()` is
  immutable (`&self`).
- Theme data is immutable after loading.  Style resolution (`ThemeDefinition` →
  `ResolvedStyle`) happens once at startup (and again on theme hot-reload).
- `tiny_skia::Pixmap` is re-exported from `hyprdeck_core` as `hyprdeck_core::Pixmap`
  so downstream crates don't need a direct `tiny-skia` dependency.

---

## Module Development Pattern

1. Create `crates/hyprdeck-modules/src/<name>.rs`.
2. Define a `<Name>Config` struct with `#[derive(Debug, Default, Deserialize)]`.
3. Define a `<Name>Module` state struct.
4. Implement `PanelModule` for `<Name>Module` — all six trait methods are required.
   Function bodies may be `todo!()` during scaffolding.
5. Declare `pub mod <name>;` and `pub use <name>::<Name>Module;` in
   `hyprdeck-modules/src/lib.rs`.
6. Add the module ID to `builtin_module_ids()` in `lib.rs`.
7. Add a `create_module` arm in `lib.rs`.
8. Add a default config entry in relevant shipped theme TOML files.

### Trait method responsibilities

| Method | Responsibility |
|--------|---------------|
| `id()` | Stable lowercase string ID used in TOML config |
| `desired_size()` | Preferred bounding box given current theme (called before layout) |
| `update()` | Advance state; returns `true` if redraw needed |
| `render()` | Paint into the shared pixmap within `bounds` only |
| `handle_event()` | Handle pointer/keyboard events; return `Handled`, `Ignored`, or `Action(...)` |
| `config_schema()` | Describe all configurable fields for HyprCube GUI auto-generation |

---

## Dependency Notes

| Crate | Role |
|-------|------|
| `smithay-client-toolkit` | Wayland `wlr-layer-shell`, seat/pointer events |
| `tiny-skia` | Software 2-D renderer (paths, fills, blending) |
| `cosmic-text` | Text shaping — CJK, ligatures, font fallback |
| `tokio` | Async runtime; all I/O and timers are async |
| `reqwest` | HTTP client with `rustls` (no OpenSSL) |
| `nix` | Low-level OS calls for network interface polling |
| `freedesktop-icons` | XDG icon theme lookups for window/app icons |
| `include_dir` | Embeds `themes/` directory at compile time |
| `fn0rd` | Lunar phase calculations (sibling project, not yet on crates.io) |

**No DBus at 1.0.**  System tray (StatusNotifierItem), audio (PipeWire), and
Bluetooth are all post-1.0.  This avoids a heavyweight runtime dependency and
keeps the build simple.

---

## Hyprland IPC

HyprDeck uses two Hyprland UNIX sockets:

| Socket | Path | Usage |
|--------|------|-------|
| Event socket | `$XDG_RUNTIME_DIR/hypr/$SIG/.socket2.sock` | Persistent stream; drives reactive updates |
| Command socket | `$XDG_RUNTIME_DIR/hypr/$SIG/.socket.sock` | One-shot queries and dispatches |

`HyprState` is built at startup by querying workspaces/windows/monitors via the
command socket, then kept live by applying `HyprEvent`s from the event socket.

---

## CI

GitHub Actions runs on every push to `main` and all PRs:

- **check**: fmt, clippy (-D warnings), cargo check
- **test**: workspace tests + doc tests
- **build**: release build verification
- **docs**: rustdoc with -D warnings
- **msrv**: verify against Rust 1.85.0

Security audit runs on push to main and weekly via cron.

Run locally before pushing:
```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## Related Projects

| Project | Status | Integration Point |
|---------|--------|-------------------|
| **HyprSaver** | In development | Shares theme colour palettes; shares `fn0rd` lunar lib |
| **HyprCube** | Planned | Will call `config_schema()` on every module to auto-generate settings UI; will provide a theme picker reading `embedded_theme_names()` |
