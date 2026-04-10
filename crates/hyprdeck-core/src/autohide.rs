use std::time::Duration;

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
        let (hide_delay, show_delay, phase) = match &mode {
            AutoHideMode::Disabled => (
                Duration::ZERO,
                Duration::ZERO,
                AnimPhase::Visible,
            ),
            AutoHideMode::AutoHide { .. } => (
                Duration::from_millis(500),
                Duration::ZERO,
                AnimPhase::Visible,
            ),
            AutoHideMode::DodgeActive => (
                Duration::ZERO,
                Duration::ZERO,
                AnimPhase::Visible,
            ),
            AutoHideMode::DockHover {
                hide_delay_ms, ..
            } => (
                Duration::from_millis(*hide_delay_ms),
                Duration::ZERO,
                AnimPhase::Hidden,
            ),
        };

        Self {
            mode,
            phase,
            progress: if phase == AnimPhase::Hidden { 1.0 } else { 0.0 },
            hide_delay,
            show_delay,
            hover_timeout: None,
        }
    }

    /// Advance the animation by `dt` seconds; returns `true` if a redraw is needed.
    pub fn tick(&mut self, dt: f32) -> bool {
        if matches!(self.mode, AutoHideMode::Disabled) {
            return false;
        }

        // Check hide delay timer
        if let Some(timeout) = self.hover_timeout {
            if std::time::Instant::now() >= timeout {
                self.hover_timeout = None;
                if self.phase == AnimPhase::Visible {
                    self.phase = AnimPhase::Hiding;
                }
            }
        }

        let speed = 4.0; // 0→1 in 0.25s
        match self.phase {
            AnimPhase::Visible | AnimPhase::Hidden => false,
            AnimPhase::Showing => {
                self.progress -= speed * dt;
                if self.progress <= 0.0 {
                    self.progress = 0.0;
                    self.phase = AnimPhase::Visible;
                }
                true
            }
            AnimPhase::Hiding => {
                self.progress += speed * dt;
                if self.progress >= 1.0 {
                    self.progress = 1.0;
                    self.phase = AnimPhase::Hidden;
                }
                true
            }
        }
    }

    /// Notify the state machine that the cursor entered the panel area.
    pub fn on_cursor_enter(&mut self) {
        if matches!(self.mode, AutoHideMode::Disabled) {
            return;
        }
        if self.phase == AnimPhase::Hidden || self.phase == AnimPhase::Hiding {
            self.phase = AnimPhase::Showing;
        }
        self.hover_timeout = None;
    }

    /// Notify the state machine that the cursor left the panel area.
    pub fn on_cursor_leave(&mut self) {
        if matches!(self.mode, AutoHideMode::Disabled) {
            return;
        }
        if self.hide_delay.is_zero() {
            if self.phase == AnimPhase::Visible {
                self.phase = AnimPhase::Hiding;
            }
        } else {
            self.hover_timeout = Some(std::time::Instant::now() + self.hide_delay);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_mode_starts_visible() {
        let state = AutoHideState::from_mode(AutoHideMode::Disabled);
        assert_eq!(state.phase, AnimPhase::Visible);
        assert_eq!(state.progress, 0.0);
    }

    #[test]
    fn auto_hide_starts_visible() {
        let state = AutoHideState::from_mode(AutoHideMode::AutoHide {
            edge_trigger: false,
        });
        assert_eq!(state.phase, AnimPhase::Visible);
        assert_eq!(state.progress, 0.0);
    }

    #[test]
    fn dock_hover_starts_hidden() {
        let state = AutoHideState::from_mode(AutoHideMode::DockHover {
            hover_margin: 4,
            hide_delay_ms: 300,
        });
        assert_eq!(state.phase, AnimPhase::Hidden);
        assert_eq!(state.progress, 1.0);
    }

    #[test]
    fn disabled_tick_returns_false() {
        let mut state = AutoHideState::from_mode(AutoHideMode::Disabled);
        assert!(!state.tick(0.1));
    }

    #[test]
    fn disabled_cursor_events_are_noop() {
        let mut state = AutoHideState::from_mode(AutoHideMode::Disabled);
        state.on_cursor_enter();
        assert_eq!(state.phase, AnimPhase::Visible);
        state.on_cursor_leave();
        assert_eq!(state.phase, AnimPhase::Visible);
    }

    #[test]
    fn cursor_enter_transitions_hidden_to_showing() {
        let mut state = AutoHideState::from_mode(AutoHideMode::DockHover {
            hover_margin: 4,
            hide_delay_ms: 500,
        });
        assert_eq!(state.phase, AnimPhase::Hidden);

        state.on_cursor_enter();
        assert_eq!(state.phase, AnimPhase::Showing);
    }

    #[test]
    fn showing_tick_reduces_progress() {
        let mut state = AutoHideState::from_mode(AutoHideMode::DockHover {
            hover_margin: 4,
            hide_delay_ms: 500,
        });
        state.on_cursor_enter();
        assert_eq!(state.phase, AnimPhase::Showing);

        let needs_redraw = state.tick(0.1);
        assert!(needs_redraw);
        assert!(state.progress < 1.0);
    }

    #[test]
    fn showing_completes_to_visible() {
        let mut state = AutoHideState::from_mode(AutoHideMode::DockHover {
            hover_margin: 4,
            hide_delay_ms: 500,
        });
        state.on_cursor_enter();

        // Tick enough to fully show
        for _ in 0..100 {
            state.tick(0.05);
        }
        assert_eq!(state.phase, AnimPhase::Visible);
        assert_eq!(state.progress, 0.0);
    }

    #[test]
    fn cursor_leave_with_zero_delay_starts_hiding_immediately() {
        let mut state = AutoHideState::from_mode(AutoHideMode::AutoHide {
            edge_trigger: false,
        });
        assert_eq!(state.phase, AnimPhase::Visible);

        // Auto-hide has 500ms delay, so override for this test
        state.hide_delay = Duration::ZERO;
        state.on_cursor_leave();
        assert_eq!(state.phase, AnimPhase::Hiding);
    }

    #[test]
    fn hiding_tick_increases_progress() {
        let mut state = AutoHideState::from_mode(AutoHideMode::AutoHide {
            edge_trigger: false,
        });
        state.hide_delay = Duration::ZERO;
        state.on_cursor_leave();
        assert_eq!(state.phase, AnimPhase::Hiding);

        let needs_redraw = state.tick(0.1);
        assert!(needs_redraw);
        assert!(state.progress > 0.0);
    }

    #[test]
    fn hiding_completes_to_hidden() {
        let mut state = AutoHideState::from_mode(AutoHideMode::AutoHide {
            edge_trigger: false,
        });
        state.hide_delay = Duration::ZERO;
        state.on_cursor_leave();

        for _ in 0..100 {
            state.tick(0.05);
        }
        assert_eq!(state.phase, AnimPhase::Hidden);
        assert_eq!(state.progress, 1.0);
    }

    #[test]
    fn cursor_leave_with_delay_arms_timeout() {
        let mut state = AutoHideState::from_mode(AutoHideMode::AutoHide {
            edge_trigger: false,
        });
        // Default hide_delay is 500ms for AutoHide
        state.on_cursor_leave();
        assert!(state.hover_timeout.is_some());
        // Phase stays Visible until timeout fires
        assert_eq!(state.phase, AnimPhase::Visible);
    }

    #[test]
    fn cursor_enter_clears_timeout() {
        let mut state = AutoHideState::from_mode(AutoHideMode::AutoHide {
            edge_trigger: false,
        });
        state.on_cursor_leave();
        assert!(state.hover_timeout.is_some());

        state.on_cursor_enter();
        assert!(state.hover_timeout.is_none());
    }

    #[test]
    fn visible_and_hidden_tick_returns_false() {
        let mut visible = AutoHideState::from_mode(AutoHideMode::AutoHide {
            edge_trigger: false,
        });
        assert!(!visible.tick(0.1));

        let mut hidden = AutoHideState::from_mode(AutoHideMode::DockHover {
            hover_margin: 4,
            hide_delay_ms: 500,
        });
        assert!(!hidden.tick(0.1));
    }
}
