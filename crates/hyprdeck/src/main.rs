use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialise logging from RUST_LOG env var; defaults to INFO.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("HyprDeck starting up");

    // Locate and load user config.
    let config_path = user_config_path();
    info!("Loading config from {:?}", config_path);
    let config = hyprdeck_core::Config::load(&config_path)?;

    // Load the selected theme (user override > embedded default).
    let theme_def = hyprdeck_themes::load_theme(&config.theme)?;
    info!("Loaded theme: {}", theme_def.name);

    // TODO: Connect to Hyprland event + command sockets.
    // TODO: Initialise Wayland connection via smithay-client-toolkit.
    // TODO: For each connected output, instantiate panels declared by the theme.
    // TODO: Enter main event loop (tokio::select! over Wayland events, Hyprland
    //       events, and a 1 Hz render/update ticker).

    todo!("Wayland event loop not yet implemented")
}

/// Return the path to `~/.config/hyprdeck/config.toml`.
fn user_config_path() -> std::path::PathBuf {
    let config_dir = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").unwrap_or_default();
            std::path::Path::new(&home).join(".config")
        });
    config_dir.join("hyprdeck").join("config.toml")
}
