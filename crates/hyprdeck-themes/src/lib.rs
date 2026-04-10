pub mod defaults;

pub use defaults::{embedded_theme_names, embedded_theme_toml};

use hyprdeck_core::ThemeDefinition;

/// Errors that can occur during theme loading.
#[derive(Debug)]
pub enum ThemeLoadError {
    /// The theme name was not found in embedded defaults or the user themes directory.
    NotFound(String),
    /// The theme TOML failed to parse.
    ParseError(toml::de::Error),
    /// An I/O error occurred reading a user theme file.
    Io(std::io::Error),
}

impl std::fmt::Display for ThemeLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThemeLoadError::NotFound(name) => write!(f, "theme not found: {}", name),
            ThemeLoadError::ParseError(e) => write!(f, "theme parse error: {}", e),
            ThemeLoadError::Io(e) => write!(f, "theme I/O error: {}", e),
        }
    }
}

impl std::error::Error for ThemeLoadError {}

/// Load a [`ThemeDefinition`] by name.
///
/// Resolution order:
/// 1. `~/.config/hyprdeck/themes/<name>/theme.toml` (user override)
/// 2. Embedded shipped themes (compiled in via `include_dir!`)
pub fn load_theme(name: &str) -> Result<ThemeDefinition, ThemeLoadError> {
    // 1. Check user themes directory first.
    if let Some(user_path) = user_theme_path(name) {
        if user_path.exists() {
            let src = std::fs::read_to_string(&user_path).map_err(ThemeLoadError::Io)?;
            return ThemeDefinition::from_toml(&src).map_err(ThemeLoadError::ParseError);
        }
    }

    // 2. Fall back to embedded themes.
    let src = embedded_theme_toml(name).ok_or_else(|| ThemeLoadError::NotFound(name.to_owned()))?;
    ThemeDefinition::from_toml(src).map_err(ThemeLoadError::ParseError)
}

/// Returns the path to a user-overridden theme directory, if the XDG config
/// directory can be determined.
fn user_theme_path(name: &str) -> Option<std::path::PathBuf> {
    let config_dir = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| dirs_path())?;
    Some(config_dir.join("hyprdeck").join("themes").join(name).join("theme.toml"))
}

fn dirs_path() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(std::path::Path::new(&home).join(".config"))
}
