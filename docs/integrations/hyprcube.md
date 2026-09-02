# HyprCube integration

This page describes Rust APIs that HyprDeck currently exposes to an in-process
consumer such as HyprCube. It does not describe a stable process-to-process
integration protocol.

## Implemented APIs

### Configuration location and loading

Use `hyprdeck_core::default_config_path()` rather than reimplementing XDG
resolution. It returns `$XDG_CONFIG_HOME/hypr/hyprdeck.toml`, or
`$HOME/.config/hypr/hyprdeck.toml` when `XDG_CONFIG_HOME` is unavailable. It
returns `ConfigPathError::NoBaseDir` if neither base directory is available.

```rust
use hyprdeck_core::{Config, default_config_path};

let path = default_config_path()?;
let config = Config::load(&path)?;
```

`Config::load` returns a `ConfigPathError::NotFound(path)` error when the file
does not exist. The config type is deserialization-only in the current API.

### Built-in module schemas

Every built-in module implements `PanelModule::config_schema()`, returning a
`ModuleConfigSchema` with a stable module ID and `ConfigField` values. Create
each registered module with an empty TOML table and inspect its schema:

```rust
use hyprdeck_core::PanelModule;
use hyprdeck_modules::{builtin_module_ids, create_module};

for id in builtin_module_ids() {
    let config = toml::Value::Table(toml::map::Map::new());
    let module = create_module(id, config).expect("registered module ID");
    let schema = module.config_schema();
    // schema.module_id and schema.fields drive the settings UI.
}
```

`ConfigFieldType` represents text, integer, float, boolean, choice,
labeled-choice, and color controls, including defaults and applicable ranges
or options.

Schema presence does not guarantee that every field affects current rendering;
consult [known limitations](../known-limitations.md) before exposing a control.

### Theme enumeration and loading

`hyprdeck_themes::embedded_theme_names()` returns the names of themes embedded
in the binary. `load_theme(name)` parses a `ThemeDefinition`, preferring a user
theme at `$XDG_CONFIG_HOME/hyprdeck/themes/<name>/theme.toml` (or
`~/.config/hyprdeck/themes/<name>/theme.toml`) when present.

```rust
use hyprdeck_themes::{embedded_theme_names, load_theme};

for name in embedded_theme_names() {
    let theme = load_theme(name)?;
    println!("{}", theme.name);
}
```

## Not implemented

HyprDeck currently exposes no API to serialize or write `Config`, no config or
theme hot reload, and no HyprCube-facing IPC. A consumer can read the TOML and
use the Rust APIs above, but writing changes and applying them safely is the
consumer's responsibility; HyprDeck must be restarted to load them.

HyprDeck does have internal Rust support for communicating with Hyprland's
command and event sockets. That is application infrastructure, not an IPC
contract for HyprCube.
