use serde::Deserialize;

/// Top-level user configuration, loaded from `~/.config/hyprdeck/config.toml`.
#[derive(Debug, Deserialize)]
pub struct Config {
    /// Name of the active theme (matches a directory under `themes/` or an embedded default).
    pub theme: String,
    /// Optional per-field overrides applied on top of the chosen theme's style.
    #[serde(default)]
    pub theme_overrides: ThemeOverrides,
    /// Module-specific configuration sections.
    #[serde(default)]
    pub modules: ModuleConfigs,
}

/// Partial style overrides that the user can set in `config.toml` without forking a theme.
#[derive(Debug, Default, Deserialize)]
pub struct ThemeOverrides {
    pub bar_opacity: Option<f32>,
    pub accent_color: Option<String>,
    pub font_family: Option<String>,
    pub font_size: Option<f32>,
}

/// Per-module configuration sections.
///
/// Each key is a module identifier (e.g. `"clock"`, `"weather"`).
/// Values are raw TOML tables passed verbatim to the corresponding module for
/// self-parsing via its own `Deserialize` implementation.
#[derive(Debug, Default, Deserialize)]
pub struct ModuleConfigs {
    #[serde(flatten)]
    pub modules: std::collections::HashMap<String, toml::Value>,
}

impl Config {
    /// Load `Config` from the given file path.
    ///
    /// If the file does not exist, returns a default config using the
    /// `"gnome_classic"` theme.
    pub fn load(path: &std::path::Path) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::info!("No config file at {:?}, using defaults", path);
                return Ok(Self {
                    theme: "gnome_classic".to_owned(),
                    theme_overrides: ThemeOverrides::default(),
                    modules: ModuleConfigs::default(),
                });
            }
            Err(e) => return Err(e.into()),
        };
        let config: Self = toml::from_str(&src)?;
        Ok(config)
    }

    /// Return the module config for `id`, if present.
    pub fn module_config(&self, id: &str) -> Option<&toml::Value> {
        self.modules.modules.get(id)
    }
}
