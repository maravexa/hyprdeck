use std::time::Duration;

use tracing::{error, info, trace, warn};

use hyprdeck_core::ipc::HyprIpc;
use hyprdeck_core::module::UpdateContext;
use hyprdeck_core::App;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // ── 1. Initialize logging ─────────────────────────────
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| {
                    tracing_subscriber::EnvFilter::new("hyprdeck=info,hyprdeck_core=info")
                }),
        )
        .init();

    info!("HyprDeck starting");

    // ── 2. Load config ────────────────────────────────────
    let config_path = user_config_path();
    info!("Loading config from {:?}", config_path);
    let config = hyprdeck_core::Config::load(&config_path)?;
    info!("Theme: {}", config.theme);

    // ── 3. Resolve theme ──────────────────────────────────
    let theme_def = hyprdeck_themes::load_theme(&config.theme)?;
    info!(
        "Loaded theme '{}' with {} panel(s)",
        theme_def.name,
        theme_def.panels.len(),
    );

    // ── 4. Connect to Hyprland IPC ────────────────────────
    let ipc = HyprIpc::connect().await?;
    let hypr_state = ipc.state();
    let mut hypr_rx = ipc.subscribe();
    let _hypr_socket = ipc.command().socket_path().to_path_buf();
    info!("Connected to Hyprland IPC");

    // ── 5. Build App state ────────────────────────────────
    let mut app = App::new(config, theme_def, hyprdeck_modules::create_module);

    // Hydrate outputs from the initial Hyprland state.
    {
        let state = hypr_state.read().await;
        for monitor in &state.monitors {
            app.add_output(
                monitor.name.clone(),
                monitor.width,
                monitor.height,
            );
        }
    }

    // Do an initial module update so modules hydrate from HyprState.
    {
        let now = chrono::Local::now();
        let state = hypr_state.read().await;
        let ctx = UpdateContext {
            now,
            hypr_state: &state,
        };
        app.tick_modules(&ctx);
    }

    // ── 6. Connect to Wayland ─────────────────────────────
    //
    // SCTK 0.19 delegate-based setup will go here. For now we set up
    // the Wayland connection and fd for the event loop, but full SCTK
    // delegate wiring (LayerShellHandler, PointerHandler, OutputHandler,
    // ShmHandler) is deferred to the Wayland integration phase.
    //
    // The connection is established to validate the Wayland environment,
    // but surface creation requires the full delegate infrastructure.
    let _wayland_conn = match wayland_client::Connection::connect_to_env() {
        Ok(conn) => {
            info!("Connected to Wayland display");
            Some(conn)
        }
        Err(e) => {
            warn!("Could not connect to Wayland display: {}. Running in headless mode.", e);
            None
        }
    };

    // ── 7. Signal handling ────────────────────────────────
    let mut sigterm =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let mut sigint =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;

    // ── 8. Main event loop ────────────────────────────────
    //
    // Three concurrent concerns:
    //   a) Hyprland IPC events (workspace changes, window focus, etc.)
    //   b) Module update tick (1-second interval)
    //   c) Animation frame (16ms / 60fps, only active during animations)
    //   d) Wayland event dispatch (when Wayland is connected)
    //
    // Wayland surface events are handled by SCTK delegates once that
    // infrastructure is wired up.

    let mut tick_interval = tokio::time::interval(Duration::from_secs(1));
    tick_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    let mut frame_interval = tokio::time::interval(Duration::from_millis(16));
    frame_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    let mut last_frame_time = tokio::time::Instant::now();
    let mut animating = app.is_animating();

    info!("Entering main event loop");

    loop {
        tokio::select! {
            // Signal handling
            _ = sigterm.recv() => {
                info!("Received SIGTERM, shutting down");
                app.shutdown = true;
            }
            _ = sigint.recv() => {
                info!("Received SIGINT, shutting down");
                app.shutdown = true;
            }

            // Hyprland IPC events
            event = hypr_rx.recv() => {
                match event {
                    Ok(ev) => {
                        trace!("Hyprland event: {:?}", ev);

                        // Handle monitor hotplug via IPC
                        match &ev {
                            hyprdeck_core::HyprEvent::MonitorAdded { name, .. } => {
                                let state = hypr_state.read().await;
                                if let Some(mon) = state.monitors.iter().find(|m| &m.name == name) {
                                    app.add_output(mon.name.clone(), mon.width, mon.height);
                                }
                            }
                            hyprdeck_core::HyprEvent::MonitorRemoved { name } => {
                                app.remove_output(name);
                            }
                            _ => {}
                        }

                        app.handle_hypr_event(&ev);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!("Hyprland event receiver lagged by {} events", n);
                        // Mark all panels dirty since we missed events.
                        for output in app.outputs.values_mut() {
                            for panel in &mut output.panels {
                                panel.dirty = true;
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        error!("Hyprland IPC channel closed");
                        app.shutdown = true;
                    }
                }
            }

            // Module update tick (1 second)
            _ = tick_interval.tick() => {
                let now = chrono::Local::now();
                let state = hypr_state.read().await;
                let ctx = UpdateContext {
                    now,
                    hypr_state: &state,
                };
                app.tick_modules(&ctx);
            }

            // Animation frame (16ms, only when animating)
            _ = frame_interval.tick(), if animating => {
                let now = tokio::time::Instant::now();
                let dt = (now - last_frame_time).as_secs_f32();
                last_frame_time = now;
                trace!("Animation frame dt={:.4}s", dt);

                app.tick_animations(dt);
            }
        }

        // After any event source fires, render dirty panels
        app.render_dirty();

        // Update animating flag
        animating = app.is_animating();
        if !animating {
            // Reset frame time reference when we stop animating
            last_frame_time = tokio::time::Instant::now();
        }

        if app.shutdown {
            break;
        }
    }

    info!("HyprDeck shutting down");
    Ok(())
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
