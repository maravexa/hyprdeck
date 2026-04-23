use std::path::PathBuf;

use serde::Deserialize;
use thiserror::Error;

/// Error returned by [`default_config_path`] when the XDG base directory cannot
/// be determined, or by [`Config::load`] when the config file is missing.
#[derive(Debug, Error)]
pub enum ConfigPathError {
    /// Neither `XDG_CONFIG_HOME` nor `HOME` is set in the environment.
    #[error("cannot determine config directory: neither XDG_CONFIG_HOME nor HOME is set")]
    NoBaseDir,
    /// Config file not found at the resolved path.
    #[error("config not found at {0}")]
    NotFound(PathBuf),
}

/// Return the default path for `hyprdeck.toml`.
///
/// Resolution order:
/// 1. `$XDG_CONFIG_HOME/hypr/hyprdeck.toml` — when `XDG_CONFIG_HOME` is set and non-empty.
/// 2. `$HOME/.config/hypr/hyprdeck.toml` — when only `HOME` is set.
/// 3. [`ConfigPathError::NoBaseDir`] — when neither variable is set.
///
/// This is the canonical function for HyprCube to locate HyprDeck's config; do not
/// duplicate this path-resolution logic on HyprCube's side.
pub fn default_config_path() -> Result<PathBuf, ConfigPathError> {
    resolve_config_path(std::env::var_os("XDG_CONFIG_HOME"), std::env::var_os("HOME"))
}

fn resolve_config_path(
    xdg_config_home: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
) -> Result<PathBuf, ConfigPathError> {
    let config_dir = if let Some(xdg) = xdg_config_home {
        PathBuf::from(xdg)
    } else if let Some(h) = home {
        PathBuf::from(h).join(".config")
    } else {
        return Err(ConfigPathError::NoBaseDir);
    };
    Ok(config_dir.join("hypr").join("hyprdeck.toml"))
}

/// Top-level user configuration, loaded from `$XDG_CONFIG_HOME/hypr/hyprdeck.toml`
/// (or `~/.config/hypr/hyprdeck.toml` when `XDG_CONFIG_HOME` is not set).
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

/// Partial style overrides that the user can set in `hyprdeck.toml` without forking a theme.
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
    /// Returns [`ConfigPathError::NotFound`] (wrapped in a `Box`) when the file
    /// does not exist; the error message includes the checked path so operators
    /// see exactly where HyprDeck looked.
    pub fn load(path: &std::path::Path) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(ConfigPathError::NotFound(path.to_owned()).into());
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

#[cfg(test)]
mod tests {
    use super::*;

    fn unwrap_path(r: Result<PathBuf, ConfigPathError>) -> String {
        r.unwrap().to_string_lossy().into_owned()
    }

    #[test]
    fn xdg_config_home_set() {
        let result = resolve_config_path(Some("/custom/config".into()), Some("/home/user".into()));
        assert_eq!(unwrap_path(result), "/custom/config/hypr/hyprdeck.toml");
    }

    #[test]
    fn xdg_config_home_unset_home_set() {
        let result = resolve_config_path(None, Some("/home/mara".into()));
        assert_eq!(unwrap_path(result), "/home/mara/.config/hypr/hyprdeck.toml");
    }

    #[test]
    fn both_unset_returns_no_base_dir_error() {
        let result = resolve_config_path(None, None);
        assert!(matches!(result, Err(ConfigPathError::NoBaseDir)));
    }

    #[test]
    fn missing_file_error_contains_path() {
        let path = std::path::Path::new("/nonexistent/hyprdeck.toml");
        let err = Config::load(path).unwrap_err();
        assert!(err.to_string().contains("/nonexistent/hyprdeck.toml"));
    }
}
