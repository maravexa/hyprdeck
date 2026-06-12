# HyprCube ↔ HyprDeck Integration Guide

Design bridge document for the HyprCube ↔ HyprDeck integration.
Covers config location, schema discovery, and theme enumeration.

---

## Config file location

HyprCube must **not** re-implement path resolution. Call the exported helper:

```rust
// In HyprCube (add hyprdeck-core as a dependency, or call the binary and parse output)
use hyprdeck_core::default_config_path;

let path = default_config_path()?;
// → $XDG_CONFIG_HOME/hypr/hyprdeck.toml, or $HOME/.config/hypr/hyprdeck.toml
```

### Resolution order (for documentation purposes only — use the helper)

| Condition | Resolved path |
|-----------|--------------|
| `XDG_CONFIG_HOME` is set and non-empty | `$XDG_CONFIG_HOME/hypr/hyprdeck.toml` |
| `XDG_CONFIG_HOME` unset, `HOME` set | `$HOME/.config/hypr/hyprdeck.toml` |
| Both unset | `ConfigPathError::NoBaseDir` — surface an error to the user |

### Error handling

`default_config_path` returns `Result<PathBuf, hyprdeck_core::ConfigPathError>`.

```rust
pub enum ConfigPathError {
    /// Neither XDG_CONFIG_HOME nor HOME is set.
    NoBaseDir,
    /// Config file not found at the resolved path.
    NotFound(PathBuf),
}
```

When HyprCube opens the config for editing it should propagate `NotFound` as a
prompt to create a new file rather than silently creating an empty one.

---

## Module config schema discovery

Every built-in module implements `PanelModule::config_schema()`, which returns a
`ModuleConfigSchema` describing all user-facing fields. HyprCube calls this to
auto-generate the settings UI without any compile-time knowledge of the modules.

```rust
use hyprdeck_core::PanelModule;
use hyprdeck_modules::{builtin_module_ids, create_module};

// Iterate every registered module and collect its schema:
for id in builtin_module_ids() {
    let module = create_module(id, toml::Value::Table(Default::default()))
        .expect("builtin id is always constructible");
    let schema = module.config_schema();
    // schema.fields: Vec<ConfigField>
    // ConfigField { key, label, description, field_type }
}
```

Field types (`ConfigFieldType`) cover: `Text`, `Integer { min, max }`,
`Float { min, max }`, `Boolean`, `Choice { options }`,
`LabeledChoice { options, labels }`, and `Color`.

---

## Theme enumeration

```rust
use hyprdeck_themes::{embedded_theme_names, load_theme};

// List all shipped themes (for a theme picker):
let names: Vec<&str> = embedded_theme_names();

// Load a theme to inspect its metadata:
let def = load_theme("win7")?;
println!("{}", def.name);  // "Windows 7 Aero"
```

User theme overrides live at `$XDG_CONFIG_HOME/hyprdeck/themes/<name>/theme.toml`
and take precedence over embedded themes automatically inside `load_theme`.

---

## Reading and writing the user config

HyprDeck's config is plain TOML. The root struct is `hyprdeck_core::Config`:

```toml
# $HOME/.config/hypr/hyprdeck.toml
theme = "win7"

[theme_overrides]
accent_color = "#ff6a00"
font_family  = "Inter"

[modules.clock]
format = "%H:%M:%S"

[modules.weather]
unit            = "celsius"
refresh_minutes = 30
```

HyprCube should read this file via `hyprdeck_core::Config::load(&path)` and write
edits back as TOML using the same structure. Hot-reload is triggered by HyprDeck
watching the file for changes (post-1.0 feature).

---

## IPC (post-1.0)

Live config editing without restart is planned for post-1.0 via a HyprDeck IPC
command. This document will be updated when the IPC surface is stabilised.
