//! Workspace switcher module.
//!
//! Reads `HyprState` on every tick and renders a row of clickable workspace
//! indicators.  Left-click switches to the workspace; middle-click moves the
//! focused window there.

use serde::Deserialize;

use hyprdeck_core::{
    Action, ConfigField, ConfigFieldType, EventResult, InputEvent, ModuleConfigSchema, MouseButton,
    PanelModule, Pixmap, Rect, Size, ThemeContext, UpdateContext, Workspace,
};

use crate::render_utils;

// ── Config ────────────────────────────────────────────────────────────────────

/// Configuration for the workspace switcher module.
#[derive(Debug, Default, Deserialize)]
pub struct WorkspacesConfig {
    /// Display workspace names instead of numeric IDs.
    #[serde(default)]
    pub show_names: bool,
    /// Hide workspaces that contain no windows (only show occupied ones).
    #[serde(default)]
    pub hide_empty: bool,
    /// Highlight workspaces with urgent windows using the urgent colour.
    #[serde(default = "default_true")]
    pub highlight_urgent: bool,
}

fn default_true() -> bool {
    true
}

// ── Module ────────────────────────────────────────────────────────────────────

/// Runtime state for the workspace switcher.
pub struct WorkspacesModule {
    config: WorkspacesConfig,
    /// Snapshot of workspaces from the last `update()` call.
    workspaces: Vec<Workspace>,
    active_id: i32,
}

impl WorkspacesModule {
    pub fn new(config: WorkspacesConfig) -> Self {
        WorkspacesModule {
            config,
            workspaces: Vec::new(),
            active_id: 1,
        }
    }

    /// Gap between adjacent slots.
    fn gap() -> f32 {
        2.0
    }

    /// Return the workspace at horizontal position `x` within `bounds`, if any.
    ///
    /// Slot positions are right-aligned within `bounds` to match `render()`.
    fn slot_at_x(&self, x: f32, bounds: Rect, slot: f32) -> Option<&Workspace> {
        let gap = Self::gap();
        let n = self.workspaces.len();
        for (i, ws) in self.workspaces.iter().enumerate() {
            let remaining = (n - i) as f32;
            let sx = bounds.x + bounds.width - remaining * slot - (remaining - 1.0) * gap;
            if x >= sx && x < sx + slot {
                return Some(ws);
            }
        }
        None
    }
}

impl PanelModule for WorkspacesModule {
    fn id(&self) -> &str {
        "workspaces"
    }

    fn desired_size(&self, theme: &ThemeContext) -> Size {
        // Estimate slot size as font_size * 2.5; render() uses bounds.height for
        // the actual slot size, so this is a width-allocation hint only.
        let estimated_slot = theme.fonts.size * 2.5;
        let count = self.workspaces.len().max(1) as f32;
        let gap = Self::gap();
        let w = count * estimated_slot + (count - 1.0).max(0.0) * gap;
        Size::new(w, estimated_slot)
    }

    fn update(&mut self, ctx: &UpdateContext<'_>) -> bool {
        // Use the active workspace for THIS monitor so each bar highlights the
        // workspace that is active on its own output, regardless of focus.
        let new_active = ctx
            .hypr_state
            .monitors
            .iter()
            .find(|m| m.name == ctx.output_name)
            .map(|m| m.active_workspace)
            .unwrap_or(ctx.hypr_state.active_workspace);

        let new_ws: Vec<Workspace> = {
            let mut ws: Vec<Workspace> = ctx.hypr_state.workspaces.clone();
            // Filter negative IDs (special workspaces) and empty workspaces if requested.
            ws.retain(|w| {
                if w.id < 0 {
                    return false;
                }
                if self.config.hide_empty {
                    // A workspace is occupied if any window lives in it.
                    let occupied = ctx
                        .hypr_state
                        .windows
                        .iter()
                        .any(|win| win.workspace_id == w.id);
                    return occupied || w.id == new_active;
                }
                true
            });
            ws.sort_by_key(|w| w.id);
            ws
        };

        let changed = new_ws != self.workspaces || new_active != self.active_id;
        if changed {
            self.workspaces = new_ws;
            self.active_id = new_active;
        }
        changed
    }

    fn render(&self, canvas: &mut Pixmap, theme: &ThemeContext, bounds: Rect) {
        // Squares fill the full bar height — no top/bottom gap.
        let slot = bounds.height;
        let gap = Self::gap();
        let radius = theme.border_radius.min(slot / 2.0);
        let n = self.workspaces.len();

        for (i, ws) in self.workspaces.iter().enumerate() {
            // Right-align: slots are packed against the right edge of bounds.
            let remaining = (n - i) as f32;
            let sx = bounds.x + bounds.width - remaining * slot - (remaining - 1.0) * gap;
            let slot_rect = Rect::new(sx, bounds.y, slot, slot);

            let is_active = ws.id == self.active_id;
            let is_urgent = self.config.highlight_urgent && ws.has_urgent;

            // Use per-module theme colors; urgent stays as-is from the base palette.
            let ws_style = &theme.module_styles.workspaces;
            let bg = if is_active {
                ws_style.active_background
            } else if is_urgent {
                theme.colors.urgent
            } else {
                ws_style.inactive_background
            };

            render_utils::fill_rounded_rect(canvas, slot_rect, bg, radius);

            // Draw label — bold, large font that fills the square.
            let label = if self.config.show_names {
                ws.name.as_str().to_owned()
            } else {
                ws.id.to_string()
            };
            let text_color = if is_active {
                ws_style.active_foreground
            } else {
                ws_style.inactive_foreground
            };
            let font_family = theme
                .fonts
                .bold_family
                .as_deref()
                .unwrap_or(&theme.fonts.family);
            let font_size = (slot * 0.6).floor();
            render_utils::draw_text_centered(
                canvas,
                &label,
                slot_rect,
                font_family,
                font_size,
                text_color,
            );
        }
    }

    fn handle_event(&mut self, event: &InputEvent, bounds: Rect) -> EventResult {
        // Slot size matches render(): squares fill the full bar height.
        let slot = bounds.height;

        match event {
            InputEvent::MousePress { x, button, .. } => {
                if let Some(ws) = self.slot_at_x(*x, bounds, slot) {
                    let id = ws.id;
                    return match button {
                        MouseButton::Left => EventResult::Action(Action::HyprDispatch {
                            dispatch: format!("workspace {id}"),
                        }),
                        MouseButton::Middle => EventResult::Action(Action::HyprDispatch {
                            dispatch: format!("movetoworkspace {id}"),
                        }),
                        MouseButton::Right => EventResult::Ignored,
                    };
                }
                EventResult::Ignored
            }
            _ => EventResult::Ignored,
        }
    }

    fn config_schema(&self) -> ModuleConfigSchema {
        ModuleConfigSchema {
            module_id: self.id().to_owned(),
            fields: vec![
                ConfigField {
                    key: "show_names".to_owned(),
                    label: "Show workspace names".to_owned(),
                    description: "Display workspace names instead of numbers.".to_owned(),
                    field_type: ConfigFieldType::Boolean { default: false },
                },
                ConfigField {
                    key: "hide_empty".to_owned(),
                    label: "Hide empty workspaces".to_owned(),
                    description: "Only show workspaces that contain at least one window."
                        .to_owned(),
                    field_type: ConfigFieldType::Boolean { default: false },
                },
                ConfigField {
                    key: "highlight_urgent".to_owned(),
                    label: "Highlight urgent".to_owned(),
                    description: "Use the urgent colour for workspaces with urgent windows."
                        .to_owned(),
                    field_type: ConfigFieldType::Boolean { default: true },
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyprdeck_core::{HyprState, WindowInfo};

    fn state_with(workspaces: Vec<Workspace>, active: i32) -> HyprState {
        HyprState {
            workspaces,
            active_workspace: active,
            windows: Vec::new(),
            ..HyprState::default()
        }
    }

    #[test]
    fn update_detects_workspace_change() {
        let mut m = WorkspacesModule::new(WorkspacesConfig::default());
        let ws = vec![
            Workspace {
                id: 1,
                name: "1".into(),
                monitor: "HDMI-1".into(),
                has_urgent: false,
            },
            Workspace {
                id: 2,
                name: "2".into(),
                monitor: "HDMI-1".into(),
                has_urgent: false,
            },
        ];
        let state = state_with(ws.clone(), 1);
        let ctx = UpdateContext {
            now: chrono::Local::now(),
            hypr_state: &state,
            output_name: "",
        };
        assert!(m.update(&ctx), "first update should return true");
        assert!(!m.update(&ctx), "same state should return false");
    }

    #[test]
    fn update_hides_special_workspaces() {
        let mut m = WorkspacesModule::new(WorkspacesConfig::default());
        let ws = vec![
            Workspace {
                id: -1,
                name: "special".into(),
                monitor: "HDMI-1".into(),
                has_urgent: false,
            },
            Workspace {
                id: 1,
                name: "1".into(),
                monitor: "HDMI-1".into(),
                has_urgent: false,
            },
        ];
        let state = state_with(ws, 1);
        let ctx = UpdateContext {
            now: chrono::Local::now(),
            hypr_state: &state,
            output_name: "",
        };
        m.update(&ctx);
        assert_eq!(m.workspaces.len(), 1);
        assert_eq!(m.workspaces[0].id, 1);
    }

    #[test]
    fn hide_empty_keeps_active_workspace() {
        let mut m = WorkspacesModule::new(WorkspacesConfig {
            hide_empty: true,
            ..WorkspacesConfig::default()
        });
        let ws = vec![
            Workspace {
                id: 1,
                name: "1".into(),
                monitor: "HDMI-1".into(),
                has_urgent: false,
            },
            Workspace {
                id: 2,
                name: "2".into(),
                monitor: "HDMI-1".into(),
                has_urgent: false,
            },
        ];
        // Only workspace 2 has windows; 1 is active but empty.
        let mut state = state_with(ws, 1);
        state.windows = vec![WindowInfo {
            address: "0x1".into(),
            title: "app".into(),
            class: "App".into(),
            workspace_id: 2,
            is_focused: false,
            is_fullscreen: false,
        }];
        let ctx = UpdateContext {
            now: chrono::Local::now(),
            hypr_state: &state,
            output_name: "",
        };
        m.update(&ctx);
        // Workspace 1 should still be present because it is active.
        assert!(m.workspaces.iter().any(|w| w.id == 1));
    }

    #[test]
    fn slot_at_x_returns_correct_index() {
        let mut m = WorkspacesModule::new(WorkspacesConfig::default());
        m.workspaces = vec![
            Workspace {
                id: 1,
                name: "1".into(),
                monitor: "".into(),
                has_urgent: false,
            },
            Workspace {
                id: 2,
                name: "2".into(),
                monitor: "".into(),
                has_urgent: false,
            },
            Workspace {
                id: 3,
                name: "3".into(),
                monitor: "".into(),
                has_urgent: false,
            },
        ];
        let slot = 24.0_f32;
        let bounds = Rect::new(0.0, 0.0, 200.0, 32.0);
        // Slots are right-aligned within bounds (gap = 2.0):
        //   ws1 (i=0): sx = 200 - 3*24 - 2*2 = 124, centre = 136
        //   ws2 (i=1): sx = 200 - 2*24 - 1*2 = 150, centre = 162
        let ws = m.slot_at_x(136.0, bounds, slot);
        assert_eq!(ws.map(|w| w.id), Some(1));
        let ws2 = m.slot_at_x(162.0, bounds, slot);
        assert_eq!(ws2.map(|w| w.id), Some(2));
        // Below the first right-aligned slot — no workspace there.
        let none = m.slot_at_x(24.5, bounds, slot);
        let _ = none; // just assert no panic
    }

    #[test]
    fn left_click_returns_dispatch_action() {
        let mut m = WorkspacesModule::new(WorkspacesConfig::default());
        m.workspaces = vec![Workspace {
            id: 3,
            name: "3".into(),
            monitor: "".into(),
            has_urgent: false,
        }];
        // handle_event uses bounds.height as slot size.
        // With 1 workspace and bounds 32×32, the single slot fills [0, 32).
        let bounds = Rect::new(0.0, 0.0, 32.0, 32.0);
        let result = m.handle_event(
            &InputEvent::MousePress {
                x: 16.0, // centre of the single workspace slot
                y: 16.0,
                button: MouseButton::Left,
            },
            bounds,
        );
        assert!(
            matches!(result, EventResult::Action(Action::HyprDispatch { dispatch }) if dispatch == "workspace 3")
        );
    }
}
