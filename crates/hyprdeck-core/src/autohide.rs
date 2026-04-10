use serde::{Deserialize, Serialize};

/// Controls when and how a panel hides itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum AutoHideMode {
    /// Panel is always visible; never auto-hides.
    Disabled,
    /// Panel slides off-screen when not hovered; optional edge trigger strip.
    AutoHide {
        #[serde(default)]
        edge_trigger: bool,
    },
    /// Panel dodges the active (focused) window by moving out of its way.
    DodgeActive,
    /// Dock-style reveal on hover with configurable margin and hide delay.
    DockHover {
        /// Pixels from the screen edge that trigger reveal.
        hover_margin: u32,
        /// Milliseconds to wait before hiding after the cursor leaves.
        hide_delay_ms: u64,
    },
}

/// Current phase of the auto-hide animation state machine.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnimPhase {
    /// Panel is fully visible.
    Visible,
    /// Panel is in the process of sliding off-screen.
    Hiding,
    /// Panel is fully hidden (only edge trigger strip visible, if enabled).
    Hidden,
    /// Panel is in the process of sliding back on-screen.
    Showing,
}

/// Runtime state for auto-hide animation.
pub struct AutoHideState {
    /// Configured hide behaviour.
    pub mode: AutoHideMode,
    /// Current animation phase.
    pub phase: AnimPhase,
    /// Animation progress in [0.0, 1.0]; 0 = fully hidden, 1 = fully visible.
    pub progress: f32,
    /// Duration before the panel begins to hide after the cursor leaves.
    pub hide_delay: std::time::Duration,
    /// Duration before the panel begins to show after the cursor enters.
    pub show_delay: std::time::Duration,
    /// Deadline at which the hide timer fires, if armed.
    pub hover_timeout: Option<std::time::Instant>,
}

impl AutoHideState {
    /// Construct state from an [`AutoHideMode`] with sensible defaults.
    pub fn from_mode(mode: AutoHideMode) -> Self {
        todo!()
    }

    /// Advance the animation by `dt` seconds; returns `true` if a redraw is needed.
    pub fn tick(&mut self, dt: f32) -> bool {
        todo!()
    }

    /// Notify the state machine that the cursor entered the panel area.
    pub fn on_cursor_enter(&mut self) {
        todo!()
    }

    /// Notify the state machine that the cursor left the panel area.
    pub fn on_cursor_leave(&mut self) {
        todo!()
    }
}
