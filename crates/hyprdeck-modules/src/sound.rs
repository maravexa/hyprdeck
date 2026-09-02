//! Sound / volume module.
//!
//! Displays the current volume level and mute state on the panel.
//! Supports PulseAudio and PipeWire's PulseAudio compatibility service (via
//! `pactl`), PipeWire-only setups (via `wpctl`), and ALSA (via `amixer`),
//! auto-detecting that order at startup.
//!
//! - **Left click**: toggle popup slider
//! - **Middle click**: toggle mute
//! - **Scroll up/down**: adjust volume by `volume_step`
//!
//! Queries and mutations run in background tokio tasks.  The module and an
//! open popup share one small state store, so a dropdown never becomes a stale
//! copy of the volume shown on the panel.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};

use hyprdeck_core::{
    ConfigField, ConfigFieldType, DisplayMode, EventResult, InputEvent, ModuleConfigSchema,
    MouseButton, PanelModule, Pixmap, Point, PopupContent, PopupEventResult, Rect, Size,
    ThemeContext, UpdateContext,
};

use crate::render_utils;

// ── Config ────────────────────────────────────────────────────────────────────

/// Which audio backend to use.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AudioBackend {
    /// Auto-detect: try pactl → wpctl → amixer.
    #[default]
    Auto,
    Pipewire,
    Pulseaudio,
    Alsa,
}

/// Configuration for the sound module.
#[derive(Debug, Deserialize)]
pub struct SoundConfig {
    /// `icon` — square icon only; `verbose` — icon left half + volume % right half.
    #[serde(default)]
    pub display: DisplayMode,
    /// Preferred audio backend.
    #[serde(default)]
    pub backend: AudioBackend,
    /// How often to poll the audio backend (milliseconds).
    #[serde(default = "default_poll_ms")]
    pub poll_interval_ms: u64,
    /// Volume change per scroll step (percent).
    #[serde(default = "default_volume_step")]
    pub volume_step: u32,
    /// Show the optional launcher for the system's advanced mixer below the
    /// built-in output slider.
    #[serde(default = "default_show_pavucontrol")]
    pub show_pavucontrol: bool,
    /// Show the expanded input and per-application mixer below the main
    /// output slider. Requires a PulseAudio-compatible `pactl` server.
    #[serde(default)]
    pub show_mixer: bool,
    /// Show default-source controls in the expanded mixer.
    #[serde(default = "default_show_input")]
    pub show_input: bool,
    /// Show per-application playback stream controls in the expanded mixer.
    #[serde(default = "default_show_applications")]
    pub show_applications: bool,
    /// Bound the expanded popup when many applications are producing audio.
    #[serde(default = "default_max_applications")]
    pub max_applications: usize,
}

fn default_poll_ms() -> u64 {
    500
}
fn default_volume_step() -> u32 {
    5
}
fn default_show_pavucontrol() -> bool {
    true
}
fn default_show_input() -> bool {
    true
}
fn default_show_applications() -> bool {
    true
}
fn default_max_applications() -> usize {
    6
}

impl Default for SoundConfig {
    fn default() -> Self {
        Self {
            display: DisplayMode::Icon,
            backend: AudioBackend::Auto,
            poll_interval_ms: default_poll_ms(),
            volume_step: default_volume_step(),
            show_pavucontrol: default_show_pavucontrol(),
            show_mixer: false,
            show_input: default_show_input(),
            show_applications: default_show_applications(),
            max_applications: default_max_applications(),
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
    pub outputs: Vec<AudioDevice>,
    pub inputs: Vec<AudioDevice>,
    pub default_input: Option<AudioDevice>,
    pub applications: Vec<AudioApplication>,
}

/// A PulseAudio/PipeWire endpoint exposed to the expanded mixer.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct AudioDevice {
    pub index: u32,
    pub name: String,
    pub description: String,
    pub volume_percent: u32,
    pub muted: bool,
}

/// A playback stream belonging to an application.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct AudioApplication {
    pub index: u32,
    pub name: String,
    pub volume_percent: u32,
    pub muted: bool,
}

/// A requested default-output mutation.  Requests are coalesced while a
/// slider is dragged; only the newest target is sent to the audio backend.
#[derive(Debug, Clone, PartialEq, Eq)]
enum AudioMutation {
    SetOutputVolume(u32),
    ToggleOutputMute,
    SetInputVolume(u32),
    ToggleInputMute,
    SetApplicationVolume(u32, u32),
    ToggleApplicationMute(u32),
    SetDefaultOutput(String),
    SetDefaultInput(String),
}

/// Data shared by [`SoundModule`] and any [`SoundPopup`] it opens.
#[derive(Debug, Default)]
struct SharedSoundState {
    state: SoundState,
    revision: u64,
    pending_mutation: Option<(AudioMutation, Instant)>,
    mutation_running: bool,
    last_error: Option<String>,
}

impl SharedSoundState {
    fn replace_state(&mut self, state: SoundState) -> bool {
        if self.state == state {
            return false;
        }
        self.state = state;
        self.revision = self.revision.wrapping_add(1);
        true
    }

    fn enqueue(&mut self, mutation: AudioMutation) {
        match &mutation {
            AudioMutation::SetOutputVolume(volume) => {
                self.state.volume_percent = *volume;
            }
            AudioMutation::ToggleOutputMute => {
                self.state.muted = !self.state.muted;
            }
            AudioMutation::SetInputVolume(volume) => {
                if let Some(input) = &mut self.state.default_input {
                    input.volume_percent = *volume;
                }
            }
            AudioMutation::ToggleInputMute => {
                if let Some(input) = &mut self.state.default_input {
                    input.muted = !input.muted;
                }
            }
            AudioMutation::SetApplicationVolume(index, volume) => {
                if let Some(application) = self
                    .state
                    .applications
                    .iter_mut()
                    .find(|application| application.index == *index)
                {
                    application.volume_percent = *volume;
                }
            }
            AudioMutation::ToggleApplicationMute(index) => {
                if let Some(application) = self
                    .state
                    .applications
                    .iter_mut()
                    .find(|application| application.index == *index)
                {
                    application.muted = !application.muted;
                }
            }
            AudioMutation::SetDefaultOutput(name) => self.state.sink_name = name.clone(),
            AudioMutation::SetDefaultInput(name) => {
                if let Some(input) = &mut self.state.default_input {
                    input.name = name.clone();
                }
            }
        }
        self.revision = self.revision.wrapping_add(1);
        self.pending_mutation = Some((mutation, Instant::now()));
    }
}

const MUTATION_DEBOUNCE: Duration = Duration::from_millis(75);

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
    /// Shared model observed by both the panel module and popup content.
    shared: Arc<Mutex<SharedSoundState>>,
    /// Shared slot written by background query tasks.
    query_result: Arc<Mutex<Option<Result<SoundState, String>>>>,
    query_running: bool,
    /// Shared slot written by a single, coalesced mutation worker.
    mutation_result: Arc<Mutex<Option<Result<(), String>>>>,
    /// Whether backend detection has been attempted.
    backend_detected: bool,
    /// Whether a detection task is currently running.
    detection_running: bool,
    /// Shared slot for backend detection result.
    detected_shared: Arc<Mutex<Option<DetectedBackend>>>,
    /// A long-lived `pactl subscribe` task increments this whenever a sink,
    /// source, or playback stream changes. The next UI tick immediately reloads
    /// JSON state instead of waiting for the fallback polling interval.
    pactl_events: Arc<AtomicU64>,
    last_pactl_event: u64,
    subscription_task: Option<tokio::task::JoinHandle<()>>,
    subscription_result: Arc<Mutex<Option<Result<(), String>>>>,
    subscription_retry_at: Option<Instant>,
}

impl SoundModule {
    pub fn new(config: SoundConfig) -> Self {
        Self {
            config,
            state: SoundState::default(),
            detected_backend: DetectedBackend::None,
            last_poll: None,
            shared: Arc::new(Mutex::new(SharedSoundState::default())),
            query_result: Arc::new(Mutex::new(None)),
            query_running: false,
            mutation_result: Arc::new(Mutex::new(None)),
            backend_detected: false,
            detection_running: false,
            detected_shared: Arc::new(Mutex::new(None)),
            pactl_events: Arc::new(AtomicU64::new(0)),
            last_pactl_event: 0,
            subscription_task: None,
            subscription_result: Arc::new(Mutex::new(None)),
            subscription_retry_at: None,
        }
    }

    fn should_poll(&self) -> bool {
        self.detected_backend != DetectedBackend::None
            && match self.last_poll {
                None => true,
                Some(t) => t.elapsed() >= Duration::from_millis(self.config.poll_interval_ms),
            }
    }

    fn pactl_changed(&self) -> bool {
        self.pactl_events.load(Ordering::Relaxed) != self.last_pactl_event
    }

    fn start_pactl_subscription(&mut self) {
        if self.subscription_task.is_some()
            || self.detected_backend != DetectedBackend::Pulseaudio
            || self
                .subscription_retry_at
                .is_some_and(|retry_at| Instant::now() < retry_at)
        {
            return;
        }
        let events = Arc::clone(&self.pactl_events);
        let result = Arc::clone(&self.subscription_result);
        self.subscription_task = Some(tokio::spawn(async move {
            let outcome = subscribe_pactl(events).await;
            if let Ok(mut slot) = result.lock() {
                *slot = Some(outcome);
            }
        }));
    }
}

impl Drop for SoundModule {
    fn drop(&mut self) {
        if let Some(task) = self.subscription_task.take() {
            task.abort();
        }
    }
}

impl PanelModule for SoundModule {
    fn id(&self) -> &str {
        "sound"
    }

    fn desired_size(&self, theme: &ThemeContext) -> Size {
        let slot = theme.icon_slot_size;
        match self.config.display {
            DisplayMode::Icon => Size::new(slot, slot),
            DisplayMode::Verbose => Size::new(slot * 2.0, slot),
        }
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

        if let Some(outcome) = self
            .subscription_result
            .try_lock()
            .ok()
            .and_then(|mut slot| slot.take())
        {
            self.subscription_task.take();
            self.subscription_retry_at = Some(Instant::now() + Duration::from_secs(2));
            match outcome {
                Ok(()) => tracing::debug!("Sound pactl subscription ended"),
                Err(error) => tracing::debug!("Sound pactl subscription ended: {error}"),
            }
        }
        if self.backend_detected {
            self.start_pactl_subscription();
        }

        let mut changed = false;

        // Collect the completed poll without blocking the panel thread.
        if let Some(result) = self
            .query_result
            .try_lock()
            .ok()
            .and_then(|mut slot| slot.take())
        {
            self.query_running = false;
            self.last_poll = Some(Instant::now());
            match result {
                Ok(new_state) => {
                    self.state = new_state.clone();
                    if let Ok(mut shared) = self.shared.lock() {
                        shared.last_error = None;
                        changed |= shared.replace_state(new_state);
                    }
                }
                Err(error) => {
                    tracing::warn!("Sound query failed: {error}");
                    if let Ok(mut shared) = self.shared.lock() {
                        shared.last_error = Some(error);
                        shared.revision = shared.revision.wrapping_add(1);
                        changed = true;
                    }
                }
            }
        }

        // A mutation is always executed by exactly one worker.  Slider motion
        // only replaces `pending_mutation`, avoiding one process per pixel.
        if let Some(result) = self
            .mutation_result
            .try_lock()
            .ok()
            .and_then(|mut slot| slot.take())
        {
            if let Ok(mut shared) = self.shared.lock() {
                shared.mutation_running = false;
                match result {
                    Ok(()) => shared.last_error = None,
                    Err(error) => {
                        tracing::warn!("Sound mutation failed: {error}");
                        shared.last_error = Some(error);
                    }
                }
                shared.revision = shared.revision.wrapping_add(1);
                changed = true;
            }
            // Poll immediately after a write so optimistic values converge to
            // the compositor's authoritative audio state.
            self.last_poll = None;
        }

        self.start_pending_mutation();

        // Spawn a new query if polling interval has elapsed.
        if (self.should_poll() || self.pactl_changed()) && !self.query_running {
            self.last_poll = Some(Instant::now());
            self.last_pactl_event = self.pactl_events.load(Ordering::Relaxed);
            self.query_running = true;
            let query_result = Arc::clone(&self.query_result);
            let backend = self.detected_backend.clone();
            tokio::spawn(async move {
                let result = match backend {
                    DetectedBackend::Pipewire => query_pipewire().await,
                    DetectedBackend::Pulseaudio => query_pulseaudio().await,
                    DetectedBackend::Alsa => query_alsa().await,
                    DetectedBackend::None => return,
                };
                if let Ok(mut slot) = query_result.lock() {
                    *slot = Some(result);
                }
            });
        }

        changed
    }

    fn render(&self, canvas: &mut Pixmap, theme: &ThemeContext, bounds: Rect) {
        match self.config.display {
            DisplayMode::Icon => {
                render_utils::draw_speaker_icon(
                    canvas,
                    render_utils::icon_content_rect(bounds, theme.icon_padding),
                    theme.colors.foreground,
                    self.state.volume_percent,
                    self.state.muted,
                );
            }
            DisplayMode::Verbose => {
                let vol_text = if self.state.backend_name.is_empty() {
                    "--".to_owned()
                } else {
                    format!("{}%", self.state.volume_percent.clamp(0, 100))
                };
                render_utils::draw_verbose(
                    canvas,
                    bounds,
                    theme,
                    &vol_text,
                    theme.colors.foreground,
                    |canvas, icon_rect| {
                        render_utils::draw_speaker_icon(
                            canvas,
                            icon_rect,
                            theme.colors.foreground,
                            self.state.volume_percent,
                            self.state.muted,
                        );
                    },
                );
            }
        }
    }

    fn handle_event(&mut self, event: &InputEvent, _bounds: Rect) -> EventResult {
        match event {
            InputEvent::MousePress {
                button: MouseButton::Middle,
                ..
            } => {
                self.enqueue_mutation(AudioMutation::ToggleOutputMute);
                EventResult::Handled
            }
            InputEvent::Scroll { dy, .. } => {
                if *dy == 0.0 {
                    return EventResult::Ignored;
                }
                let step = self.config.volume_step as i32;
                // dy < 0 = scroll down = decrease; dy > 0 = scroll up = increase
                let delta = if *dy > 0.0 { step } else { -step };
                let new_vol = (self.state.volume_percent as i32 + delta).clamp(0, 150) as u32;
                self.enqueue_mutation(AudioMutation::SetOutputVolume(new_vol));
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
                    key: "display".to_owned(),
                    label: "Display mode".to_owned(),
                    description:
                        "Icon-only square or double-wide icon + volume percentage readout."
                            .to_owned(),
                    field_type: ConfigFieldType::LabeledChoice {
                        options: vec!["icon".to_owned(), "verbose".to_owned()],
                        labels: vec!["Icon only".to_owned(), "Icon + value".to_owned()],
                        default: "icon".to_owned(),
                    },
                },
                ConfigField {
                    key: "show_pavucontrol".to_owned(),
                    label: "Show advanced mixer launcher".to_owned(),
                    description: "Show a themed Open pavucontrol button below the output slider."
                        .to_owned(),
                    field_type: ConfigFieldType::Boolean { default: true },
                },
                ConfigField {
                    key: "show_mixer".to_owned(),
                    label: "Show expanded mixer".to_owned(),
                    description: "Show input and per-application controls below the output slider when pactl is available.".to_owned(),
                    field_type: ConfigFieldType::Boolean { default: false },
                },
                ConfigField {
                    key: "show_input".to_owned(),
                    label: "Show input controls".to_owned(),
                    description: "Show default microphone/source volume in the expanded mixer.".to_owned(),
                    field_type: ConfigFieldType::Boolean { default: true },
                },
                ConfigField {
                    key: "show_applications".to_owned(),
                    label: "Show application controls".to_owned(),
                    description: "Show per-application playback stream volume controls in the expanded mixer.".to_owned(),
                    field_type: ConfigFieldType::Boolean { default: true },
                },
                ConfigField {
                    key: "max_applications".to_owned(),
                    label: "Maximum applications".to_owned(),
                    description: "Maximum playback streams shown in the expanded popup.".to_owned(),
                    field_type: ConfigFieldType::Integer { default: 6, min: Some(1), max: Some(20) },
                },
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
        true
    }

    fn popup_content(&self) -> Option<Box<dyn PopupContent>> {
        let (state, revision) = self
            .shared
            .lock()
            .map(|shared| (shared.state.clone(), shared.revision))
            .unwrap_or_else(|_| (self.state.clone(), 0));
        Some(Box::new(SoundPopup {
            state: state.clone(),
            last_revision: revision,
            slider_value: state.volume_percent as f32 / 100.0,
            slider_dragging: false,
            shared: Arc::clone(&self.shared),
            show_pavucontrol: self.config.show_pavucontrol,
            show_mixer: self.config.show_mixer,
            show_input: self.config.show_input,
            show_applications: self.config.show_applications,
            max_applications: self.config.max_applications.max(1),
        }))
    }
}

impl SoundModule {
    fn enqueue_mutation(&mut self, mutation: AudioMutation) {
        if let Ok(mut shared) = self.shared.lock() {
            shared.enqueue(mutation);
            self.state = shared.state.clone();
        }
    }

    fn start_pending_mutation(&mut self) {
        let mutation = self.shared.lock().ok().and_then(|mut shared| {
            let (mutation, queued_at) = shared.pending_mutation.clone()?;
            if shared.mutation_running || queued_at.elapsed() < MUTATION_DEBOUNCE {
                return None;
            }
            shared.pending_mutation = None;
            shared.mutation_running = true;
            Some(mutation)
        });
        let Some(mutation) = mutation else {
            return;
        };

        let backend = self.detected_backend.clone();
        let result_slot = Arc::clone(&self.mutation_result);
        tokio::spawn(async move {
            let result = run_mutation(backend, mutation).await;
            if let Ok(mut slot) = result_slot.lock() {
                *slot = Some(result);
            }
        });
    }
}

// ── Sound popup ───────────────────────────────────────────────────────────────

/// Popup content for the sound module — shows a volume slider.
pub struct SoundPopup {
    state: SoundState,
    last_revision: u64,
    /// Slider position in [0.0, 1.5] representing 0–150%.
    slider_value: f32,
    slider_dragging: bool,
    shared: Arc<Mutex<SharedSoundState>>,
    show_pavucontrol: bool,
    show_mixer: bool,
    show_input: bool,
    show_applications: bool,
    max_applications: usize,
}

impl PopupContent for SoundPopup {
    fn desired_size(&self, _theme: &ThemeContext) -> Size {
        let app_count = if self.show_mixer && self.show_applications {
            self.state.applications.len().min(self.max_applications)
        } else {
            0
        };
        let input_height = if self.show_mixer && self.show_input {
            72.0
        } else {
            0.0
        };
        let apps_height = if app_count > 0 {
            24.0 + app_count as f32 * 44.0
        } else {
            0.0
        };
        let launcher_height = if self.show_pavucontrol { 38.0 } else { 0.0 };
        Size::new(340.0, 118.0 + input_height + apps_height + launcher_height)
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
        render_utils::draw_text_centered(
            canvas,
            &title,
            title_rect,
            bold,
            font_size,
            theme.colors.foreground,
        );

        if let Ok(shared) = self.shared.try_lock() {
            if let Some(error) = &shared.last_error {
                let error_rect =
                    Rect::new(bounds.x + 8.0, bounds.y + 92.0, bounds.width - 16.0, 16.0);
                render_utils::draw_text_centered(
                    canvas,
                    error,
                    error_rect,
                    &theme.fonts.family,
                    font_size * 0.72,
                    render_utils::dim_color(theme.colors.foreground, 0.65),
                );
            }
        }

        // ── Sink name (smaller, dimmed) ──
        if !self.state.sink_name.is_empty() {
            let sink_color = render_utils::dim_color(theme.colors.foreground, 0.6);
            let sink_rect = Rect::new(bounds.x, bounds.y + 24.0, bounds.width, 18.0);
            let sink_label = if self.show_mixer {
                format!("Output: {}  (click to change)", self.state.sink_name)
            } else {
                self.state.sink_name.clone()
            };
            render_utils::draw_text_centered(
                canvas,
                &sink_label,
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
        let track_color = render_utils::dim_color(theme.colors.foreground, 0.2);
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
            Point::new(
                handle_x.clamp(bounds.x + 8.0, bounds.x + bounds.width - 8.0),
                slider_y + 12.0,
            ),
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

        let mut y = bounds.y + 108.0;
        if self.show_mixer && self.show_input {
            if let Some(input) = &self.state.default_input {
                render_utils::draw_text_ellipsis(
                    canvas,
                    &format!("Input: {}  (click to change)", input.description),
                    Rect::new(bounds.x + 8.0, y, bounds.width - 16.0, 18.0),
                    &theme.fonts.family,
                    font_size * 0.82,
                    render_utils::dim_color(theme.colors.foreground, 0.75),
                );
                draw_mixer_slider(
                    canvas,
                    bounds.x,
                    y + 20.0,
                    bounds.width,
                    input.volume_percent,
                    theme,
                );
                let input_label = if input.muted {
                    "Muted".to_owned()
                } else {
                    format!("{}%", input.volume_percent)
                };
                render_utils::draw_text_centered(
                    canvas,
                    &input_label,
                    Rect::new(bounds.x, y + 35.0, bounds.width, 18.0),
                    &theme.fonts.family,
                    font_size * 0.78,
                    theme.colors.foreground,
                );
                y += 66.0;
            }
        }
        if self.show_mixer && self.show_applications {
            let applications = self.state.applications.iter().take(self.max_applications);
            if !self.state.applications.is_empty() {
                render_utils::draw_text_ellipsis(
                    canvas,
                    "Applications  (middle-click a slider to mute)",
                    Rect::new(bounds.x + 8.0, y, bounds.width - 16.0, 18.0),
                    bold,
                    font_size * 0.88,
                    theme.colors.foreground,
                );
                y += 20.0;
            }
            for application in applications {
                render_utils::draw_text_ellipsis(
                    canvas,
                    &application.name,
                    Rect::new(bounds.x + 8.0, y, bounds.width - 16.0, 16.0),
                    &theme.fonts.family,
                    font_size * 0.8,
                    render_utils::dim_color(theme.colors.foreground, 0.8),
                );
                draw_mixer_slider(
                    canvas,
                    bounds.x,
                    y + 17.0,
                    bounds.width,
                    application.volume_percent,
                    theme,
                );
                y += 44.0;
            }
        }
        if self.show_pavucontrol {
            let button = Rect::new(bounds.x + 16.0, y, bounds.width - 32.0, 26.0);
            render_utils::fill_rounded_rect(
                canvas,
                button,
                render_utils::dim_color(theme.colors.accent, 0.8),
                5.0,
            );
            render_utils::draw_text_centered(
                canvas,
                "Open pavucontrol",
                button,
                &theme.fonts.family,
                font_size * 0.9,
                theme.colors.foreground,
            );
        }
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
                    self.enqueue_volume();
                    PopupEventResult::Handled
                } else if self.show_mixer
                    && Rect::new(bounds.x, bounds.y + 22.0, bounds.width, 22.0)
                        .contains(Point::new(*x, *y))
                {
                    self.cycle_output();
                    PopupEventResult::Handled
                } else if self.show_mixer
                    && self.show_input
                    && Rect::new(bounds.x, bounds.y + 108.0, bounds.width, 20.0)
                        .contains(Point::new(*x, *y))
                {
                    self.cycle_input();
                    PopupEventResult::Handled
                } else if self.show_mixer
                    && self.show_input
                    && self
                        .input_slider_area(bounds)
                        .is_some_and(|area| area.contains(Point::new(*x, *y)))
                {
                    let area = self.input_slider_area(bounds).expect("checked above");
                    self.enqueue_input_volume(value_from_x(*x, area));
                    PopupEventResult::Handled
                } else if let Some((index, area)) = self.application_slider_area_at(bounds, *x, *y)
                {
                    self.enqueue_application_volume(index, value_from_x(*x, area));
                    PopupEventResult::Handled
                } else if self.show_pavucontrol
                    && self.launcher_rect(bounds).contains(Point::new(*x, *y))
                {
                    PopupEventResult::Action(hyprdeck_core::Action::Exec {
                        command: "pavucontrol".to_owned(),
                        args: Vec::new(),
                    })
                } else {
                    PopupEventResult::Ignored
                }
            }
            InputEvent::MousePress {
                x,
                y,
                button: MouseButton::Middle,
            } => {
                if self.show_mixer
                    && self.show_input
                    && self
                        .input_slider_area(bounds)
                        .is_some_and(|area| area.contains(Point::new(*x, *y)))
                {
                    if let Ok(mut shared) = self.shared.lock() {
                        shared.enqueue(AudioMutation::ToggleInputMute);
                    }
                    PopupEventResult::Handled
                } else if let Some((index, _)) = self.application_slider_area_at(bounds, *x, *y) {
                    if let Ok(mut shared) = self.shared.lock() {
                        shared.enqueue(AudioMutation::ToggleApplicationMute(index));
                    }
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
                    self.enqueue_volume();
                    PopupEventResult::Handled
                } else {
                    PopupEventResult::Ignored
                }
            }
            _ => PopupEventResult::Ignored,
        }
    }

    fn update(&mut self) -> bool {
        let Ok(shared) = self.shared.try_lock() else {
            return false;
        };
        if shared.revision == self.last_revision {
            return false;
        }
        self.last_revision = shared.revision;
        if !self.slider_dragging {
            self.state = shared.state.clone();
            self.slider_value = self.state.volume_percent as f32 / 100.0;
        }
        true
    }

    fn is_dragging(&self) -> bool {
        self.slider_dragging
    }
}

impl SoundPopup {
    fn update_slider_from_x(&mut self, x: f32, bounds_x: f32, bounds_width: f32) {
        let relative = ((x - bounds_x) / bounds_width).clamp(0.0, 1.0);
        self.slider_value = relative * 1.5; // 0–150%
        self.state.volume_percent = (self.slider_value * 100.0).round() as u32;
    }

    fn enqueue_volume(&self) {
        if let Ok(mut shared) = self.shared.lock() {
            shared.enqueue(AudioMutation::SetOutputVolume(self.state.volume_percent));
        }
    }

    fn enqueue_input_volume(&self, volume: u32) {
        if let Ok(mut shared) = self.shared.lock() {
            shared.enqueue(AudioMutation::SetInputVolume(volume));
        }
    }

    fn enqueue_application_volume(&self, index: u32, volume: u32) {
        if let Ok(mut shared) = self.shared.lock() {
            shared.enqueue(AudioMutation::SetApplicationVolume(index, volume));
        }
    }

    fn cycle_output(&self) {
        let Some(position) = self
            .state
            .outputs
            .iter()
            .position(|output| output.description == self.state.sink_name)
        else {
            return;
        };
        let Some(next) = self
            .state
            .outputs
            .get((position + 1) % self.state.outputs.len())
        else {
            return;
        };
        if let Ok(mut shared) = self.shared.lock() {
            shared.enqueue(AudioMutation::SetDefaultOutput(next.name.clone()));
        }
    }

    fn cycle_input(&self) {
        let Some(input) = &self.state.default_input else {
            return;
        };
        let Some(position) = self
            .state
            .inputs
            .iter()
            .position(|candidate| candidate.name == input.name)
        else {
            return;
        };
        let Some(next) = self
            .state
            .inputs
            .get((position + 1) % self.state.inputs.len())
        else {
            return;
        };
        if let Ok(mut shared) = self.shared.lock() {
            shared.enqueue(AudioMutation::SetDefaultInput(next.name.clone()));
        }
    }

    fn input_slider_area(&self, bounds: Rect) -> Option<Rect> {
        (self.show_mixer && self.show_input && self.state.default_input.is_some())
            .then(|| Rect::new(bounds.x, bounds.y + 128.0, bounds.width, 26.0))
    }

    fn application_slider_area_at(&self, bounds: Rect, x: f32, y: f32) -> Option<(u32, Rect)> {
        if !self.show_mixer || !self.show_applications {
            return None;
        }
        let start = bounds.y
            + 108.0
            + if self.show_input && self.state.default_input.is_some() {
                66.0
            } else {
                0.0
            }
            + 20.0;
        self.state
            .applications
            .iter()
            .take(self.max_applications)
            .enumerate()
            .find_map(|(position, application)| {
                let area = Rect::new(
                    bounds.x,
                    start + position as f32 * 44.0 + 17.0,
                    bounds.width,
                    26.0,
                );
                area.contains(Point::new(x, y))
                    .then_some((application.index, area))
            })
    }

    fn launcher_rect(&self, bounds: Rect) -> Rect {
        let app_count = if self.show_mixer && self.show_applications {
            self.state.applications.len().min(self.max_applications)
        } else {
            0
        };
        let input_height =
            if self.show_mixer && self.show_input && self.state.default_input.is_some() {
                66.0
            } else {
                0.0
            };
        Rect::new(
            bounds.x + 16.0,
            bounds.y
                + 108.0
                + input_height
                + if app_count > 0 {
                    20.0 + app_count as f32 * 44.0
                } else {
                    0.0
                },
            bounds.width - 32.0,
            26.0,
        )
    }
}

fn value_from_x(x: f32, area: Rect) -> u32 {
    (((x - area.x) / area.width).clamp(0.0, 1.0) * 150.0).round() as u32
}

fn draw_mixer_slider(
    canvas: &mut Pixmap,
    x: f32,
    y: f32,
    width: f32,
    volume: u32,
    theme: &ThemeContext,
) {
    let track = Rect::new(x, y + 7.0, width, 6.0);
    render_utils::fill_rounded_rect(
        canvas,
        track,
        render_utils::dim_color(theme.colors.foreground, 0.2),
        3.0,
    );
    let fraction = (volume as f32 / 150.0).clamp(0.0, 1.0);
    render_utils::fill_rounded_rect(
        canvas,
        Rect::new(x, y + 7.0, width * fraction, 6.0),
        theme.colors.accent,
        3.0,
    );
    render_utils::fill_circle(
        canvas,
        Point::new(x + width * fraction, y + 10.0),
        6.0,
        theme.colors.accent,
    );
}

// ── Helper ────────────────────────────────────────────────────────────────────

// ── Backend detection ─────────────────────────────────────────────────────────

async fn detect_backend(preferred: &AudioBackend) -> DetectedBackend {
    match preferred {
        AudioBackend::Pipewire => DetectedBackend::Pipewire,
        AudioBackend::Pulseaudio => DetectedBackend::Pulseaudio,
        AudioBackend::Alsa => DetectedBackend::Alsa,
        AudioBackend::Auto => {
            // PipeWire normally exposes this PulseAudio-compatible control
            // plane too, which gives the mixer access to inputs and streams.
            if command_exists("pactl").await {
                tracing::debug!("Sound: detected pactl-compatible audio server");
                DetectedBackend::Pulseaudio
            } else if command_exists("wpctl").await {
                tracing::debug!("Sound: detected PipeWire (wpctl)");
                DetectedBackend::Pipewire
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
        .output()
        .await
        .map(|output| output.status.success())
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
    ensure_success("wpctl get-volume", &output)?;

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
        ..SoundState::default()
    })
}

async fn query_pulseaudio() -> Result<SoundState, String> {
    let (sinks, sources, streams, info) = tokio::try_join!(
        pactl_json("list", "sinks"),
        pactl_json("list", "sources"),
        pactl_json("list", "sink-inputs"),
        pactl_json("info", ""),
    )?;
    parse_pactl_state(&sinks, &sources, &streams, &info)
}

async fn pactl_json(command: &str, subject: &str) -> Result<Value, String> {
    let mut process = tokio::process::Command::new("pactl");
    process.args(["-f", "json", command]);
    if !subject.is_empty() {
        process.arg(subject);
    }
    let output = process.output().await.map_err(|error| error.to_string())?;
    ensure_success("pactl -f json", &output)?;
    serde_json::from_slice(&output.stdout).map_err(|error| format!("invalid pactl JSON: {error}"))
}

/// Parse `pactl -f json` output without assuming a specific PulseAudio or
/// PipeWire server revision. Fields unavailable on an older server simply
/// become empty controls rather than breaking the panel.
pub(crate) fn parse_pactl_state(
    sinks: &Value,
    sources: &Value,
    streams: &Value,
    info: &Value,
) -> Result<SoundState, String> {
    let outputs = parse_devices(sinks);
    // PulseAudio exposes monitor sources for each output. They are useful to
    // recording software but are not microphones, so do not present them as
    // selectable default input devices in the panel mixer.
    let inputs = parse_devices(sources)
        .into_iter()
        .filter(|device| !device.name.ends_with(".monitor"))
        .collect::<Vec<_>>();
    let applications = parse_applications(streams);
    let default_sink_name = info
        .get("default_sink_name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let default_source_name = info
        .get("default_source_name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let output = outputs
        .iter()
        .find(|device| device.name == default_sink_name)
        .or_else(|| outputs.first())
        .cloned()
        .ok_or_else(|| "pactl reported no output sinks".to_owned())?;
    let default_input = inputs
        .iter()
        .find(|device| device.name == default_source_name)
        .or_else(|| inputs.first())
        .cloned();
    Ok(SoundState {
        volume_percent: output.volume_percent,
        muted: output.muted,
        sink_name: output.description.clone(),
        backend_name: "PulseAudio".to_owned(),
        outputs,
        inputs,
        default_input,
        applications,
    })
}

fn parse_devices(value: &Value) -> Vec<AudioDevice> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            Some(AudioDevice {
                index: entry.get("index")?.as_u64()? as u32,
                name: entry.get("name")?.as_str()?.to_owned(),
                description: entry
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or_else(|| {
                        entry
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                    })
                    .to_owned(),
                volume_percent: pactl_volume_percent(entry.get("volume")),
                muted: entry.get("mute").and_then(Value::as_bool).unwrap_or(false),
            })
        })
        .collect()
}

fn parse_applications(value: &Value) -> Vec<AudioApplication> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let index = entry.get("index")?.as_u64()? as u32;
            let properties = entry.get("properties");
            let name = properties
                .and_then(|properties| properties.get("application.name"))
                .and_then(Value::as_str)
                .or_else(|| {
                    properties
                        .and_then(|properties| properties.get("media.name"))
                        .and_then(Value::as_str)
                })
                .unwrap_or("Application")
                .to_owned();
            Some(AudioApplication {
                index,
                name,
                volume_percent: pactl_volume_percent(entry.get("volume")),
                muted: entry.get("mute").and_then(Value::as_bool).unwrap_or(false),
            })
        })
        .collect()
}

fn pactl_volume_percent(volume: Option<&Value>) -> u32 {
    let Some(volume) = volume else { return 0 };
    let Some(channels) = volume.as_object() else {
        return 0;
    };
    channels
        .values()
        .find_map(|channel| {
            channel
                .get("value_percent")
                .and_then(Value::as_str)
                .and_then(|value| {
                    value
                        .trim()
                        .trim_end_matches('%')
                        .trim()
                        .parse::<f32>()
                        .ok()
                })
                .map(|value| value.round().max(0.0) as u32)
                .or_else(|| {
                    channel
                        .get("value")
                        .and_then(Value::as_u64)
                        .map(|value| ((value as f64 / 65536.0) * 100.0).round() as u32)
                })
        })
        .unwrap_or(0)
}

async fn subscribe_pactl(events: Arc<AtomicU64>) -> Result<(), String> {
    let mut command = tokio::process::Command::new("pactl");
    command
        .arg("subscribe")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    let mut child = command
        .spawn()
        .map_err(|error| format!("pactl subscribe: {error}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "pactl subscribe did not provide stdout".to_owned())?;
    let mut lines = BufReader::new(stdout).lines();
    while let Some(line) = lines.next_line().await.map_err(|error| error.to_string())? {
        let line = line.to_ascii_lowercase();
        if line.contains("sink") || line.contains("source") {
            events.fetch_add(1, Ordering::Relaxed);
        }
    }
    let output = child
        .wait_with_output()
        .await
        .map_err(|error| error.to_string())?;
    ensure_success("pactl subscribe", &output)
}

async fn query_alsa() -> Result<SoundState, String> {
    let output = tokio::process::Command::new("amixer")
        .args(["sget", "Master"])
        .output()
        .await
        .map_err(|e| e.to_string())?;
    ensure_success("amixer sget Master", &output)?;

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
        ..SoundState::default()
    })
}

// ── Volume setting ────────────────────────────────────────────────────────────

async fn set_volume_pipewire(percent: u32) -> Result<(), String> {
    let vol = format!("{}%", percent.min(150));
    let output = tokio::process::Command::new("wpctl")
        .args(["set-volume", "-l", "1.5", "@DEFAULT_AUDIO_SINK@", &vol])
        .output()
        .await
        .map_err(|e| e.to_string())?;
    ensure_success("wpctl set-volume", &output)
}

async fn toggle_mute_pipewire() -> Result<(), String> {
    let output = tokio::process::Command::new("wpctl")
        .args(["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"])
        .output()
        .await
        .map_err(|e| e.to_string())?;
    ensure_success("wpctl set-mute", &output)
}

async fn set_volume_pulseaudio(percent: u32) -> Result<(), String> {
    let vol = format!("{}%", percent.min(150));
    let output = tokio::process::Command::new("pactl")
        .args(["set-sink-volume", "@DEFAULT_SINK@", &vol])
        .output()
        .await
        .map_err(|e| e.to_string())?;
    ensure_success("pactl set-sink-volume", &output)
}

async fn toggle_mute_pulseaudio() -> Result<(), String> {
    let output = tokio::process::Command::new("pactl")
        .args(["set-sink-mute", "@DEFAULT_SINK@", "toggle"])
        .output()
        .await
        .map_err(|e| e.to_string())?;
    ensure_success("pactl set-sink-mute", &output)
}

async fn set_input_volume_pulseaudio(percent: u32) -> Result<(), String> {
    pactl_mutate([
        "set-source-volume",
        "@DEFAULT_SOURCE@",
        &format!("{}%", percent.min(150)),
    ])
    .await
}

async fn toggle_input_mute_pulseaudio() -> Result<(), String> {
    pactl_mutate(["set-source-mute", "@DEFAULT_SOURCE@", "toggle"]).await
}

async fn set_application_volume_pulseaudio(index: u32, percent: u32) -> Result<(), String> {
    let index = index.to_string();
    let volume = format!("{}%", percent.min(150));
    pactl_mutate(["set-sink-input-volume", &index, &volume]).await
}

async fn toggle_application_mute_pulseaudio(index: u32) -> Result<(), String> {
    let index = index.to_string();
    pactl_mutate(["set-sink-input-mute", &index, "toggle"]).await
}

async fn set_default_output_pulseaudio(name: String) -> Result<(), String> {
    pactl_mutate(["set-default-sink", &name]).await
}

async fn set_default_input_pulseaudio(name: String) -> Result<(), String> {
    pactl_mutate(["set-default-source", &name]).await
}

async fn pactl_mutate<const N: usize>(args: [&str; N]) -> Result<(), String> {
    let output = tokio::process::Command::new("pactl")
        .args(args)
        .output()
        .await
        .map_err(|error| error.to_string())?;
    ensure_success("pactl", &output)
}

async fn set_volume_alsa(percent: u32) -> Result<(), String> {
    let vol = format!("{}%", percent.min(100));
    let output = tokio::process::Command::new("amixer")
        .args(["sset", "Master", &vol])
        .output()
        .await
        .map_err(|e| e.to_string())?;
    ensure_success("amixer sset Master", &output)
}

async fn toggle_mute_alsa() -> Result<(), String> {
    let output = tokio::process::Command::new("amixer")
        .args(["sset", "Master", "toggle"])
        .output()
        .await
        .map_err(|e| e.to_string())?;
    ensure_success("amixer sset Master toggle", &output)
}

async fn run_mutation(backend: DetectedBackend, mutation: AudioMutation) -> Result<(), String> {
    match (backend, mutation) {
        (DetectedBackend::Pipewire, AudioMutation::SetOutputVolume(volume)) => {
            set_volume_pipewire(volume).await
        }
        (DetectedBackend::Pipewire, AudioMutation::ToggleOutputMute) => {
            toggle_mute_pipewire().await
        }
        (DetectedBackend::Pulseaudio, AudioMutation::SetOutputVolume(volume)) => {
            set_volume_pulseaudio(volume).await
        }
        (DetectedBackend::Pulseaudio, AudioMutation::ToggleOutputMute) => {
            toggle_mute_pulseaudio().await
        }
        (DetectedBackend::Pulseaudio, AudioMutation::SetInputVolume(volume)) => {
            set_input_volume_pulseaudio(volume).await
        }
        (DetectedBackend::Pulseaudio, AudioMutation::ToggleInputMute) => {
            toggle_input_mute_pulseaudio().await
        }
        (DetectedBackend::Pulseaudio, AudioMutation::SetApplicationVolume(index, volume)) => {
            set_application_volume_pulseaudio(index, volume).await
        }
        (DetectedBackend::Pulseaudio, AudioMutation::ToggleApplicationMute(index)) => {
            toggle_application_mute_pulseaudio(index).await
        }
        (DetectedBackend::Pulseaudio, AudioMutation::SetDefaultOutput(name)) => {
            set_default_output_pulseaudio(name).await
        }
        (DetectedBackend::Pulseaudio, AudioMutation::SetDefaultInput(name)) => {
            set_default_input_pulseaudio(name).await
        }
        (DetectedBackend::Alsa, AudioMutation::SetOutputVolume(volume)) => {
            set_volume_alsa(volume).await
        }
        (DetectedBackend::Alsa, AudioMutation::ToggleOutputMute) => toggle_mute_alsa().await,
        (DetectedBackend::None, _) => Err("no supported audio backend is available".to_owned()),
        (_, _) => Err("expanded mixer controls require a pactl-compatible audio server".to_owned()),
    }
}

fn ensure_success(command: &str, output: &std::process::Output) -> Result<(), String> {
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let detail = if stderr.is_empty() {
        format!("exited with {}", output.status)
    } else {
        stderr
    };
    Err(format!("{command}: {detail}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutation_updates_shared_state_optimistically() {
        let mut shared = SharedSoundState::default();
        shared.enqueue(AudioMutation::SetOutputVolume(85));
        assert_eq!(shared.state.volume_percent, 85);
        shared.enqueue(AudioMutation::ToggleOutputMute);
        assert!(shared.state.muted);
    }

    #[test]
    fn popup_observes_shared_state_when_not_dragging() {
        let shared = Arc::new(Mutex::new(SharedSoundState::default()));
        let mut popup = SoundPopup {
            state: SoundState::default(),
            last_revision: 0,
            slider_value: 0.0,
            slider_dragging: false,
            shared: Arc::clone(&shared),
            show_pavucontrol: false,
            show_mixer: false,
            show_input: true,
            show_applications: true,
            max_applications: 6,
        };
        shared.lock().unwrap().replace_state(SoundState {
            volume_percent: 42,
            muted: false,
            sink_name: String::new(),
            backend_name: "Test".to_owned(),
            ..SoundState::default()
        });

        assert!(popup.update());
        assert_eq!(popup.state.volume_percent, 42);
        assert_eq!(popup.slider_value, 0.42);
        assert!(!popup.update());
    }

    #[test]
    fn default_config_values() {
        let cfg = SoundConfig::default();
        assert_eq!(cfg.poll_interval_ms, 500);
        assert_eq!(cfg.volume_step, 5);
        assert_eq!(cfg.backend, AudioBackend::Auto);
        assert!(!cfg.show_mixer);
        assert!(cfg.show_input);
        assert!(cfg.show_applications);
        assert_eq!(cfg.max_applications, 6);
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

    #[test]
    fn pactl_json_fixture_includes_devices_input_and_applications() {
        let sinks: Value = serde_json::from_str(
            r#"[{"index":1,"name":"alsa_output","description":"Desk speakers","mute":false,"volume":{"front-left":{"value_percent":"75%"}}}]"#,
        )
        .unwrap();
        let sources: Value = serde_json::from_str(
            r#"[{"index":2,"name":"alsa_input","description":"USB microphone","mute":true,"volume":{"mono":{"value":32768}}},{"index":3,"name":"alsa_output.monitor","description":"Monitor","mute":false,"volume":{"mono":{"value_percent":"100%"}}}]"#,
        )
        .unwrap();
        let streams: Value = serde_json::from_str(
            r#"[{"index":7,"mute":false,"volume":{"front-left":{"value_percent":"42%"}},"properties":{"application.name":"Firefox"}}]"#,
        )
        .unwrap();
        let info: Value = serde_json::from_str(
            r#"{"default_sink_name":"alsa_output","default_source_name":"alsa_input"}"#,
        )
        .unwrap();

        let state = parse_pactl_state(&sinks, &sources, &streams, &info).unwrap();
        assert_eq!(state.volume_percent, 75);
        assert_eq!(state.default_input.as_ref().unwrap().volume_percent, 50);
        assert!(state.default_input.as_ref().unwrap().muted);
        assert_eq!(state.inputs.len(), 1);
        assert_eq!(state.applications[0].name, "Firefox");
        assert_eq!(state.applications[0].volume_percent, 42);
    }
}
