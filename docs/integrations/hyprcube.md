# HyprCube integration

HyprDeck exposes a lightweight, versioned configuration contract for settings
editors. It is intentionally independent of Wayland and rendering dependencies.

## Implemented APIs

### Configuration location and loading

Use `hyprdeck_config::default_config_path()` rather than reimplementing XDG
resolution. It returns `$XDG_CONFIG_HOME/hypr/hyprdeck.toml`, or
`$HOME/.config/hypr/hyprdeck.toml` when `XDG_CONFIG_HOME` is unavailable. It
returns `ConfigPathError::NoBaseDir` if neither base directory is available.

```rust
use hyprdeck_config::{Config, default_config_path};

let path = default_config_path()?;
let config = Config::load(&path)?;
```

`Config` implements serialization and preserves unknown TOML values. Its
`save_atomic` method validates the shared settings, writes and syncs a temporary
file in the destination directory, retains the previous config as
`hyprdeck.toml.bak`, and then atomically replaces the destination.

### Built-in module schemas

Schema structures live in `hyprdeck-config` and carry an explicit
`contract_version`. The runtime aggregates schemas from the actual built-in
module implementations, so an editor does not maintain a duplicate list.

An installed HyprDeck exposes the aggregate without connecting to Wayland:

```sh
hyprdeck --print-config-schema
hyprdeck --validate-config ~/.config/hypr/hyprdeck.toml
```

In-process HyprDeck consumers can request the same aggregate:

```rust
let schema = hyprdeck_modules::builtin_config_schema();
assert_eq!(schema.contract_version, hyprdeck_config::CONFIG_CONTRACT_VERSION);
```

`ConfigFieldType` represents text, integer, float, boolean, choice,
labeled-choice, and color controls, including defaults and applicable ranges
or options.

Editors must reject unsupported contract versions instead of guessing at field
semantics. Unknown config values are retained across load/save so older editors
do not discard settings introduced by newer HyprDeck versions.

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

## Runtime application

Configuration persistence and schema discovery are supported. HyprDeck does
not yet hot-reload its complete panel topology, so editors must clearly report
that a restart is required after saving. HyprDeck's internal Hyprland sockets
are application infrastructure and are not an editor protocol.
