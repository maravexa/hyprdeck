//! Sound / volume module.
//!
//! Displays the current volume level and mute state on the panel.
//! Supports PipeWire (via `wpctl`), PulseAudio (via `pactl`), and ALSA
//! (via `amixer`), auto-detecting the available backend at startup.
//!
//! - **Left click**: toggle popup slider
//! - **Middle click**: toggle mute
//! - **Scroll up/down**: adjust volume by `volume_step`
//!
//! Volume queries run in background tokio tasks; results are shared back
//! to `update()` via `Arc<Mutex<Option<SoundState>>>`.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Deserialize;

use hyprdeck_core::{
    ConfigField, ConfigFieldType, EventResult, InputEvent, ModuleConfigSchema, MouseButton,
    PanelModule, Pixmap, Point, PopupContent, PopupEventResult, Rect, Size, ThemeContext,
    UpdateContext,
};

use crate::render_utils;

// ── Config ────────────────────────────────────────────────────────────────────

/// Which audio backend to use.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AudioBackend {
    /// Auto-detect: try wpctl → pactl → amixer.
    Auto,
    Pipewire,
    Pulseaudio,
    Alsa,
}

impl Default for AudioBackend {
    fn default() -> Self {
        Self::Auto
    }
}

/// Configuration for the sound module.
#[derive(Debug, Deserialize)]
pub struct SoundConfig {
    /// Preferred audio backend.
    #[serde(default)]
    pub backend: AudioBackend,
    /// How often to poll the audio backend (milliseconds).
    #[serde(default = "default_poll_ms")]
    pub poll_interval_ms: u64,
    /// Volume change per scroll step (percent).
    #[serde(default = "default_volume_step")]
    pub volume_step: u32,
}

fn default_poll_ms() -> u64 {
    500
}
fn default_volume_step() -> u32 {
    5
}

impl Default for SoundConfig {
    fn default() -> Self {
        Self {
            backend: AudioBackend::Auto,
            poll_interval_ms: default_poll_ms(),
            volume_step: default_volume_step(),
        }
    }
}

// ── Internal state ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct SoundState {
    pub volume_percent: u32,
    pub muted: bool,
    pub sink_name: String,
    pub backend_name: String,
}

/// Which audio backend was successfully detected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DetectedBackend {
    Pipewire,
    Pulseaudio,
    Alsa,
    None,
}

// ── Module ────────────────────────────────────────────────────────────────────

/// Runtime state for the sound / volume module.
pub struct SoundModule {
    config: SoundConfig,
    state: SoundState,
    detected_backend: DetectedBackend,
    last_poll: Option<Instant>,
    /// Shared slot written by background query tasks.
    shared: Arc<Mutex<Option<SoundState>>>,
    /// Whether backend detection has been attempted.
    backend_detected: bool,
    /// Whether a detection task is currently running.
    detection_running: bool,
    /// Shared slot for backend detection result.
    detected_shared: Arc<Mutex<Option<DetectedBackend>>>,
}

impl SoundModule {
    pub fn new(config: SoundConfig) -> Self {
        Self {
            config,
            state: SoundState::default(),
            detected_backend: DetectedBackend::None,
            last_poll: None,
            shared: Arc::new(Mutex::new(None)),
            backend_detected: false,
            detection_running: false,
            detected_shared: Arc::new(Mutex::new(None)),
        }
    }

    fn should_poll(&self) -> bool {
        self.detected_backend != DetectedBackend::None
            && match self.last_poll {
                None => true,
                Some(t) => t.elapsed() >= Duration::from_millis(self.config.poll_interval_ms),
            }
    }
}

impl PanelModule for SoundModule {
    fn id(&self) -> &str {
        "sound"
    }

    fn desired_size(&self, theme: &ThemeContext) -> Size {
        let h = theme.fonts.size + theme.padding.top + theme.padding.bottom;
        let font_size = render_utils::effective_font_size(h, theme.fonts.size);
        // Icon + space + "100%"
        let text = speaker_icon(self.state.volume_percent, self.state.muted).to_owned()
            + " "
            + &format!("{}%", self.state.volume_percent);
        let w = render_utils::estimate_text_width(&text, font_size)
            + theme.padding.left
            + theme.padding.right;
        Size::new(w.max(48.0), h)
    }

    fn update(&mut self, _ctx: &UpdateContext<'_>) -> bool {
        // Kick off backend detection on the first update() call.
        if !self.backend_detected && !self.detection_running {
            self.detection_running = true;
            let pref = self.config.backend.clone();
            let det_shared = Arc::clone(&self.detected_shared);
            tokio::spawn(async move {
                let backend = detect_backend(&pref).await;
                if let Ok(mut g) = det_shared.lock() {
                    *g = Some(backend);
                }
            });
        }

        // Check if backend detection completed.
        if !self.backend_detected {
            if let Ok(mut g) = self.detected_shared.lock() {
                if let Some(backend) = g.take() {
                    self.detected_backend = backend;
                    self.backend_detected = true;
                }
            }
        }

        // Collect result from background query task.
        let result = self.shared.try_lock().ok().and_then(|mut g| g.take());

        if let Some(new_state) = result {
            self.last_poll = Some(Instant::now());
            if new_state != self.state {
                self.state = new_state;
                return true;
            }
        }

        // Spawn a new query if polling interval has elapsed.
        if self.should_poll() {
            self.last_poll = Some(Instant::now());
            let shared = Arc::clone(&self.shared);
            let backend = self.detected_backend.clone();
            tokio::spawn(async move {
                let result = match backend {
                    DetectedBackend::Pipewire => query_pipewire().await,
                    DetectedBackend::Pulseaudio => query_pulseaudio().await,
                    DetectedBackend::Alsa => query_alsa().await,
                    DetectedBackend::None => return,
                };
                if let Ok(state) = result {
                    if let Ok(mut g) = shared.lock() {
                        *g = Some(state);
                    }
                }
            });
        }

        false
    }

    fn render(&self, canvas: &mut Pixmap, theme: &ThemeContext, bounds: Rect) {
        let font_size = render_utils::effective_font_size(bounds.height, theme.fonts.size);
        let icon = speaker_icon(self.state.volume_percent, self.state.muted);

        let display = if self.state.backend_name.is_empty() {
            // No backend detected yet — show a placeholder
            format!("{icon} --")
        } else {
            format!("{icon} {}%", self.state.volume_percent)
        };

        render_utils::draw_text_centered(
            canvas,
            &display,
            bounds,
            &theme.fonts.family,
            font_size,
            theme.colors.foreground,
        );
    }

    fn handle_event(&mut self, event: &InputEvent, _bounds: Rect) -> EventResult {
        match event {
            InputEvent::MousePress {
                button: MouseButton::Middle,
                ..
            } => {
                self.state.muted = !self.state.muted; // optimistic update
                let backend = self.detected_backend.clone();
                tokio::spawn(async move {
                    match backend {
                        DetectedBackend::Pipewire => toggle_mute_pipewire().await,
                        DetectedBackend::Pulseaudio => toggle_mute_pulseaudio().await,
                        DetectedBackend::Alsa => toggle_mute_alsa().await,
                        DetectedBackend::None => {}
                    }
                });
                EventResult::Handled
            }
            InputEvent::Scroll { dy, .. } => {
                let step = self.config.volume_step as i32;
                // dy < 0 = scroll down = decrease; dy > 0 = scroll up = increase
                let delta = if *dy > 0.0 { step } else { -step };
                let new_vol =
                    (self.state.volume_percent as i32 + delta).clamp(0, 150) as u32;
                self.state.volume_percent = new_vol; // optimistic update
                let backend = self.detected_backend.clone();
                tokio::spawn(async move {
                    match backend {
                        DetectedBackend::Pipewire => set_volume_pipewire(new_vol).await,
                        DetectedBackend::Pulseaudio => set_volume_pulseaudio(new_vol).await,
                        DetectedBackend::Alsa => set_volume_alsa(new_vol).await,
                        DetectedBackend::None => {}
                    }
                });
                EventResult::Handled
            }
            _ => EventResult::Ignored,
        }
    }

    fn config_schema(&self) -> ModuleConfigSchema {
        ModuleConfigSchema {
            module_id: self.id().to_owned(),
            fields: vec![
                ConfigField {
                    key: "backend".to_owned(),
                    label: "Audio backend".to_owned(),
                    description: "Audio system to use. \"auto\" detects wpctl → pactl → amixer."
                        .to_owned(),
                    field_type: ConfigFieldType::Choice {
                        options: vec![
                            "auto".to_owned(),
                            "pipewire".to_owned(),
                            "pulseaudio".to_owned(),
                            "alsa".to_owned(),
                        ],
                        default: "auto".to_owned(),
                    },
                },
                ConfigField {
                    key: "poll_interval_ms".to_owned(),
                    label: "Poll interval (ms)".to_owned(),
                    description: "How often to query the audio backend for volume changes."
                        .to_owned(),
                    field_type: ConfigFieldType::Integer {
                        default: 500,
                        min: Some(100),
                        max: Some(5000),
                    },
                },
                ConfigField {
                    key: "volume_step".to_owned(),
                    label: "Volume step (%)".to_owned(),
                    description: "How much to change volume per scroll event.".to_owned(),
                    field_type: ConfigFieldType::Integer {
                        default: 5,
                        min: Some(1),
                        max: Some(20),
                    },
                },
            ],
        }
    }

    fn has_popup(&self) -> bool {
        tracing::debug!("{} has_popup called → true", self.id());
        true
    }

    fn popup_content(&self) -> Option<Box<dyn PopupContent>> {
        tracing::debug!("{} popup_content called", self.id());
        Some(Box::new(SoundPopup {
            state: self.state.clone(),
            slider_value: self.state.volume_percent as f32 / 100.0,
            slider_dragging: false,
            backend: self.detected_backend.clone(),
        }))
    }
}

// ── Speaker icon ──────────────────────────────────────────────────────────────

fn speaker_icon(volume: u32, muted: bool) -> &'static str {
    if muted || volume == 0 {
        "🔇"
    } else if volume < 33 {
        "🔈"
    } else if volume < 66 {
        "🔉"
    } else {
        "🔊"
    }
}

// ── Sound popup ───────────────────────────────────────────────────────────────

/// Popup content for the sound module — shows a volume slider.
pub struct SoundPopup {
    state: SoundState,
    /// Slider position in [0.0, 1.5] representing 0–150%.
    slider_value: f32,
    slider_dragging: bool,
    backend: DetectedBackend,
}

impl PopupContent for SoundPopup {
    fn desired_size(&self, _theme: &ThemeContext) -> Size {
        Size::new(280.0, 120.0)
    }

    fn render(&self, canvas: &mut Pixmap, theme: &ThemeContext, bounds: Rect) {
        let font_size = 13.0;
        let bold = theme
            .fonts
            .bold_family
            .as_deref()
            .unwrap_or(&theme.fonts.family);

        // ── Backend / sink title ──
        let title = if self.state.backend_name.is_empty() {
            "Audio".to_owned()
        } else {
            format!("Audio: {}", self.state.backend_name)
        };
        let title_rect = Rect::new(bounds.x, bounds.y, bounds.width, 22.0);
        render_utils::draw_text_centered(canvas, &title, title_rect, bold, font_size, theme.colors.foreground);

        // ── Sink name (smaller, dimmed) ──
        if !self.state.sink_name.is_empty() {
            let sink_color = dim_color(theme.colors.foreground, 0.6);
            let sink_rect = Rect::new(bounds.x, bounds.y + 24.0, bounds.width, 18.0);
            render_utils::draw_text_centered(
                canvas,
                &self.state.sink_name,
                sink_rect,
                &theme.fonts.family,
                font_size * 0.85,
                sink_color,
            );
        }

        // ── Slider ──
        let slider_y = bounds.y + 50.0;
        let track_rect = Rect::new(bounds.x, slider_y + 8.0, bounds.width, 8.0);
        // Track background
        let track_color = dim_color(theme.colors.foreground, 0.2);
        render_utils::fill_rounded_rect(canvas, track_rect, track_color, 4.0);

        // Fill (clamped to track width)
        let fill_frac = (self.slider_value / 1.5).clamp(0.0, 1.0);
        let fill_width = bounds.width * fill_frac;
        if fill_width > 0.0 {
            let fill_rect = Rect::new(bounds.x, slider_y + 8.0, fill_width, 8.0);
            render_utils::fill_rounded_rect(canvas, fill_rect, theme.colors.accent, 4.0);
        }

        // Handle circle
        let handle_x = bounds.x + fill_width;
        render_utils::fill_circle(
            canvas,
            Point::new(handle_x.clamp(bounds.x + 8.0, bounds.x + bounds.width - 8.0), slider_y + 12.0),
            8.0,
            theme.colors.accent,
        );

        // ── Volume label ──
        let vol_text = if self.state.muted {
            "Muted".to_owned()
        } else {
            format!("{}%", self.state.volume_percent)
        };
        let vol_rect = Rect::new(bounds.x, slider_y + 30.0, bounds.width, 22.0);
        render_utils::draw_text_centered(
            canvas,
            &vol_text,
            vol_rect,
            &theme.fonts.family,
            font_size,
            theme.colors.foreground,
        );
    }

    fn handle_event(&mut self, event: &InputEvent, bounds: Rect) -> PopupEventResult {
        let slider_y = bounds.y + 50.0;
        let slider_area = Rect::new(bounds.x, slider_y, bounds.width, 30.0);

        match event {
            InputEvent::MousePress {
                x,
                y,
                button: MouseButton::Left,
            } => {
                if slider_area.contains(Point::new(*x, *y)) {
                    self.slider_dragging = true;
                    self.update_slider_from_x(*x, bounds.x, bounds.width);
                    self.send_volume();
                    PopupEventResult::Handled
                } else {
                    PopupEventResult::Ignored
                }
            }
            InputEvent::MouseRelease {
                button: MouseButton::Left,
                ..
            } => {
                if self.slider_dragging {
                    self.slider_dragging = false;
                    PopupEventResult::Handled
                } else {
                    PopupEventResult::Ignored
                }
            }
            InputEvent::MouseMove { x, .. } => {
                if self.slider_dragging {
                    self.update_slider_from_x(*x, bounds.x, bounds.width);
                    self.send_volume();
                    PopupEventResult::Handled
                } else {
                    PopupEventResult::Ignored
                }
            }
            _ => PopupEventResult::Ignored,
        }
    }

    fn update(&mut self) -> bool {
        false
    }
}

impl SoundPopup {
    fn update_slider_from_x(&mut self, x: f32, bounds_x: f32, bounds_width: f32) {
        let relative = ((x - bounds_x) / bounds_width).clamp(0.0, 1.0);
        self.slider_value = relative * 1.5; // 0–150%
        self.state.volume_percent = (self.slider_value * 100.0).round() as u32;
    }

    fn send_volume(&self) {
        let vol = self.state.volume_percent;
        let backend = self.backend.clone();
        tokio::spawn(async move {
            match backend {
                DetectedBackend::Pipewire => set_volume_pipewire(vol).await,
                DetectedBackend::Pulseaudio => set_volume_pulseaudio(vol).await,
                DetectedBackend::Alsa => set_volume_alsa(vol).await,
                DetectedBackend::None => {}
            }
        });
    }
}

// ── Helper ────────────────────────────────────────────────────────────────────

fn dim_color(color: [u8; 4], opacity: f32) -> [u8; 4] {
    let a = (color[3] as f32 * opacity.clamp(0.0, 1.0)) as u8;
    [color[0], color[1], color[2], a]
}

// ── Backend detection ─────────────────────────────────────────────────────────

async fn detect_backend(preferred: &AudioBackend) -> DetectedBackend {
    match preferred {
        AudioBackend::Pipewire => DetectedBackend::Pipewire,
        AudioBackend::Pulseaudio => DetectedBackend::Pulseaudio,
        AudioBackend::Alsa => DetectedBackend::Alsa,
        AudioBackend::Auto => {
            if command_exists("wpctl").await {
                tracing::debug!("Sound: detected PipeWire (wpctl)");
                DetectedBackend::Pipewire
            } else if command_exists("pactl").await {
                tracing::debug!("Sound: detected PulseAudio (pactl)");
                DetectedBackend::Pulseaudio
            } else if command_exists("amixer").await {
                tracing::debug!("Sound: detected ALSA (amixer)");
                DetectedBackend::Alsa
            } else {
                tracing::warn!("Sound: no audio backend detected (wpctl/pactl/amixer not found)");
                DetectedBackend::None
            }
        }
    }
}

async fn command_exists(cmd: &str) -> bool {
    tokio::process::Command::new("which")
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

// ── Volume querying ───────────────────────────────────────────────────────────

async fn query_pipewire() -> Result<SoundState, String> {
    // wpctl get-volume @DEFAULT_AUDIO_SINK@
    // Output: "Volume: 0.75" or "Volume: 0.75 [MUTED]"
    let output = tokio::process::Command::new("wpctl")
        .args(["get-volume", "@DEFAULT_AUDIO_SINK@"])
        .output()
        .await
        .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let muted = stdout.contains("[MUTED]");
    let volume = stdout
        .split_whitespace()
        .nth(1)
        .and_then(|v| v.trim_end_matches(']').parse::<f64>().ok())
        .unwrap_or(0.0);

    Ok(SoundState {
        volume_percent: (volume * 100.0).round() as u32,
        muted,
        sink_name: String::new(),
        backend_name: "PipeWire".into(),
    })
}

async fn query_pulseaudio() -> Result<SoundState, String> {
    let vol_out = tokio::process::Command::new("pactl")
        .args(["get-sink-volume", "@DEFAULT_SINK@"])
        .output()
        .await
        .map_err(|e| e.to_string())?;
    let mute_out = tokio::process::Command::new("pactl")
        .args(["get-sink-mute", "@DEFAULT_SINK@"])
        .output()
        .await
        .map_err(|e| e.to_string())?;

    let vol_str = String::from_utf8_lossy(&vol_out.stdout);
    // "Volume: front-left: 49152 /  75% / -7.50 dB ..."
    let volume = vol_str
        .split('/')
        .find(|s| s.trim().ends_with('%'))
        .and_then(|s| s.trim().trim_end_matches('%').trim().parse::<u32>().ok())
        .unwrap_or(0);

    let mute_str = String::from_utf8_lossy(&mute_out.stdout);
    let muted = mute_str.to_lowercase().contains("yes");

    Ok(SoundState {
        volume_percent: volume,
        muted,
        sink_name: String::new(),
        backend_name: "PulseAudio".into(),
    })
}

async fn query_alsa() -> Result<SoundState, String> {
    let output = tokio::process::Command::new("amixer")
        .args(["sget", "Master"])
        .output()
        .await
        .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Parse "[75%] [on]" or "[75%] [off]"
    let volume = stdout
        .split('[')
        .find(|s| s.contains('%'))
        .and_then(|s| s.split('%').next())
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(0);

    let muted = stdout.contains("[off]");

    Ok(SoundState {
        volume_percent: volume,
        muted,
        sink_name: String::new(),
        backend_name: "ALSA".into(),
    })
}

// ── Volume setting ────────────────────────────────────────────────────────────

async fn set_volume_pipewire(percent: u32) {
    let vol = format!("{}%", percent.min(150));
    let _ = tokio::process::Command::new("wpctl")
        .args(["set-volume", "-l", "1.5", "@DEFAULT_AUDIO_SINK@", &vol])
        .status()
        .await;
}

async fn toggle_mute_pipewire() {
    let _ = tokio::process::Command::new("wpctl")
        .args(["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"])
        .status()
        .await;
}

async fn set_volume_pulseaudio(percent: u32) {
    let vol = format!("{}%", percent.min(150));
    let _ = tokio::process::Command::new("pactl")
        .args(["set-sink-volume", "@DEFAULT_SINK@", &vol])
        .status()
        .await;
}

async fn toggle_mute_pulseaudio() {
    let _ = tokio::process::Command::new("pactl")
        .args(["set-sink-mute", "@DEFAULT_SINK@", "toggle"])
        .status()
        .await;
}

async fn set_volume_alsa(percent: u32) {
    let vol = format!("{}%", percent.min(100));
    let _ = tokio::process::Command::new("amixer")
        .args(["sset", "Master", &vol])
        .status()
        .await;
}

async fn toggle_mute_alsa() {
    let _ = tokio::process::Command::new("amixer")
        .args(["sset", "Master", "toggle"])
        .status()
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speaker_icon_muted() {
        assert_eq!(speaker_icon(50, true), "🔇");
    }

    #[test]
    fn speaker_icon_zero() {
        assert_eq!(speaker_icon(0, false), "🔇");
    }

    #[test]
    fn speaker_icon_low() {
        assert_eq!(speaker_icon(20, false), "🔈");
    }

    #[test]
    fn speaker_icon_mid() {
        assert_eq!(speaker_icon(50, false), "🔉");
    }

    #[test]
    fn speaker_icon_high() {
        assert_eq!(speaker_icon(80, false), "🔊");
    }

    #[test]
    fn default_config_values() {
        let cfg = SoundConfig::default();
        assert_eq!(cfg.poll_interval_ms, 500);
        assert_eq!(cfg.volume_step, 5);
        assert_eq!(cfg.backend, AudioBackend::Auto);
    }

    #[test]
    fn module_id_is_sound() {
        let m = SoundModule::new(SoundConfig::default());
        assert_eq!(m.id(), "sound");
    }

    #[test]
    fn has_popup_is_true() {
        let m = SoundModule::new(SoundConfig::default());
        assert!(m.has_popup());
    }

    #[test]
    fn popup_content_returns_some() {
        let m = SoundModule::new(SoundConfig::default());
        assert!(m.popup_content().is_some());
    }
}
