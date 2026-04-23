# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- Sound module icon now scales to theme icon size and centers correctly in its
  slot (was rendering at intrinsic size in the top-left corner).
- Adjacent modules on the status bar now render with configurable horizontal
  spacing (was rendering with no gap). Set `module_gap` in a theme's `[style]`
  block to control the spacing; all five shipped themes now carry per-theme
  values (2–6 px) that match each theme's visual language.

### Added

- Icon-only / verbose display modes for `lunar`, `sound`, and `network` modules,
  configurable via `hyprdeck.toml`.
  - `display = "icon"` (default) — single icon square; preserves existing layout.
  - `display = "verbose"` — double-wide widget: icon in the left half, numeric
    readout in the right half.
    - **lunar** readout: integer illumination percentage (e.g. `87%`), derived from
      the same `fn0rd` synodic-period calculation used by the popup.
    - **sound** readout: master volume percentage clamped to 0–100 (e.g. `75%`);
      shows `--` until the audio backend is detected.
    - **network** readout: `-45 dBm` for Wi-Fi, compact link speed for wired
      (`10Mb`, `100Mb`, `1Gb`, `2.5Gb`, …), or `--` when no default interface
      is active.
  - All three modules expose a `display` field in `config_schema()` with
    human-readable labels (`"Icon only"` / `"Icon + value"`) for HyprCube
    auto-generated settings UI.
  - `hyprdeck_core::DisplayMode` enum is publicly exported for future use by
    third-party modules.
  - All five shipped themes extended with a `verbose_text_padding` style field
    that documents (and in future wires) the gap between icon and readout halves.
  - Note: hot reload is not yet implemented; a panel restart is required to apply
    `display` mode changes.

### Breaking

- Config file renamed to `hyprdeck.toml` and moved to `$XDG_CONFIG_HOME/hypr/hyprdeck.toml`
  (fallback `~/.config/hypr/hyprdeck.toml`). No automatic migration; users must move
  their existing `config.toml` manually.
- A missing config file is now a hard error instead of silently falling back to
  the `gnome_classic` theme defaults. The error message includes the exact path
  that was checked.
- `hyprdeck_core::default_config_path()` is now the canonical path-resolution
  function; downstream tools (HyprCube) should call it rather than re-implementing
  the XDG lookup.
- `network` module: the `display` field now accepts `"icon"` or `"verbose"` only.
  The previous values `"iconlabel"` and `"iconrate"` are no longer valid and will
  produce a config parse error. Users who set these values should migrate to
  `display = "verbose"` for the equivalent readout behaviour.

## [0.4.0] - 2026-04-10

### Added

- Shader crossfade transitions via dual-framebuffer compositing
- Cycle mode: `shader = "cycle"` and `palette = "cycle"` with configurable intervals
- CycleManager with random and sequential playlist ordering
- Named playlist definitions in config (`[playlists.name]`)
- True multi-monitor independence: each output can cycle independently
- `synced` config option and `--synced` / `--no-synced` CLI flags
- New shaders: bezier, planet, tesla (20 built-in shaders total)
- 12 Mandelbrot zoom targets with random selection per cycle (was 4)
- Mandelbrot full-fractal home view with smooth zoom-to-target cycles
- Mandelbrot cardioid/bulb early bailout for reduced GPU cost in set interior
- Preview mode: playlist editor tab with drag-and-drop reordering
- Preview mode: palette editor tab with live cosine param sliders
- Preview mode: save config button, transition preview button
- CLI flags: `--shader-interval`, `--palette-interval`, `--cycle-order`, `--playlist`
- GPU benchmark results in `docs/BENCHMARK-0.4.0.md`
- Pre-baked LUT palette pipeline: all palettes (including cosine) now render via texture lookup for consistent GPU performance across palette types

### Changed

- Default shader changed from `mandelbrot` to `cycle`
- Default palette changed from `electric` to `cycle`
- Per-monitor `[[monitor]]` blocks now support `shader_playlist` and `palette_playlist` overrides
- Renamed shaders: `aurora_sphere` → `planet`, `raymarcher` → `donut`, `flow_field` → `marble`
- Renamed palettes: `electric` → `rainbow`, `vapor` → `vaporwave`
- Snowfall: doubled particle density
- Bezier: 6 curves with hard-edged smoothstep lines, AABB early rejection
- Tesla: fixed center node (500% larger), orbiting triangle, 6 arc connections
- Hypercube: 50% slower palette transitions, smoothed intra-palette color mapping, double line thickness
- Geometry: triple line thickness
- Lissajous: 4× thicker curves, reduced sample count (512 → 192)
- Network: connection fade in/out transitions, triple line thickness, increased connection distance threshold
- Starfield: non-repeating star generation, 3× longer tails drawn toward center, center dead zone
- Fire: enforced black background regardless of palette
- Planet: 20% reduced background star density
- All glow effects replaced with smoothstep hard edges across bezier, starfield, lissajous, network, geometry, hypercube for GPU optimization

### Removed

- Wormhole shader (deferred to v0.5.0 — needs fundamental rewrite)
- `config.example.toml` from project root (redundant with `examples/hypersaver.toml`)
- Redundant paru AUR install instruction from README

### Fixed

- Palette transition duration stuck at 0s regardless of config
- Wormhole (and other new v0.4.0 shaders) missing `u_alpha` uniform declaration
- New v0.4.0 shaders missing from `--list-shaders` CLI and preview dropdown
- Mandelbrot zooming into black interior (cardioid/bulb bailout + verified boundary targets)
- Mandelbrot zoom cycle jumping between targets (snap center at home zoom, no panning)
