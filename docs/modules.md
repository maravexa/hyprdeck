# Modules

Built-in modules are registered in `hyprdeck-modules::create_module`. The recognized IDs are `calendar`, `clock`, `favorites`, `lunar`, `menu_button`, `network`, `power`, `shell`, `sound`, `weather`, `window_list`, and `workspaces`. A theme places IDs in its panel arrays; the application then passes the matching `[modules.<id>]` TOML table to the registry.

## Lifecycle

A module implements the `hyprdeck_core::PanelModule` trait. All trait calls occur on the panel main thread.

1. `id()` returns a stable lowercase ID.
2. `desired_size()` reports its preferred bounds for the resolved theme.
3. `update()` receives the current time, Hyprland state, and output name; returning `true` schedules a redraw.
4. `render()` draws only within its provided bounds in the shared `tiny_skia::Pixmap`.
5. `handle_event()` returns `Ignored`, `Handled`, or an `Action` for panel dispatch.
6. `config_schema()` describes configuration fields for consumers of that API. Popup modules additionally opt in with `has_popup()` and provide `popup_content()`.

Modules that need I/O should keep `update()` non-blocking: start background work and move completed values into module state on a later update.

## Adding a built-in module

Add an implementation under `crates/hyprdeck-modules/src/`, export it from `lib.rs`, and add its ID to both `create_module()` and `builtin_module_ids()`. The registry test requires every listed ID to construct successfully and to agree with `id()`. Add the ID to a theme's `modules_start`, `modules_center`, or `modules_end` array to display it, and document its TOML table in [Configuration](configuration.md).

There is no runtime plug-in discovery: third-party modules require a source-level integration into the modules crate and registry.

## Testing

Run focused crate tests while developing:

```sh
cargo test -p hyprdeck-modules
cargo test -p hyprdeck-core
```

For a new module, test configuration deserialization/defaults, its event behavior, and rendering or sizing where practical. Keep the registry coverage test passing with `cargo test -p hyprdeck-modules`.
