pub mod schema;

use std::path::PathBuf;

use figment::{
    providers::{Env, Format, Serialized, Toml},
    Figment,
};

use crate::error::FnordError;
pub use schema::Config;

pub fn load_config(config_override: Option<&PathBuf>) -> Result<Config, FnordError> {
    // Ensure ~/.config/eris/ exists
    if let Some(config_dir) = dirs::config_dir() {
        let eris_dir = config_dir.join("eris");
        if !eris_dir.exists() {
            std::fs::create_dir_all(&eris_dir)
                .map_err(|e| FnordError::Config(format!("failed to create config dir: {e}")))?;
        }
    }

    let user_config = dirs::config_dir()
        .map(|d| d.join("eris").join("fnord.toml"))
        .unwrap_or_default();

    let mut figment = Figment::from(Serialized::defaults(Config::default()))
        .merge(Toml::file("/etc/eris/fnord.toml"))
        .merge(Toml::file(&user_config))
        .merge(Env::prefixed("FNORD_CONFIG_").split("_"));

    // FNORD_CONFIG env var points to a file path override
    if let Ok(env_path) = std::env::var("FNORD_CONFIG") {
        figment = figment.merge(Toml::file(env_path));
    }

    if let Some(path) = config_override {
        figment = figment.merge(Toml::file(path));
    }

    figment
        .extract::<Config>()
        .map_err(|e| FnordError::Config(format!("failed to load config: {e}")))
}
