# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
