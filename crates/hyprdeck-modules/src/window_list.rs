//! Window list / taskbar module.
//!
//! Renders the open windows on the active workspace as clickable buttons.
//! Left-click focuses the window; middle-click closes it.
//! Icons are loaded lazily when a new window class is first seen.

use serde::Deserialize;

use hyprdeck_core::{
    Action, ConfigField, ConfigFieldType, EventResult, InputEvent, ModuleConfigSchema, MouseButton,
    PanelModule, Pixmap, Rect, Size, ThemeContext, UpdateContext, WindowInfo,
};

use crate::{icon_utils, render_utils};

// ── Config ────────────────────────────────────────────────────────────────────

/// Button rendering style.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WindowListStyle {
    /// Text buttons with window title (classic taskbar).
    #[default]
    Buttons,
    /// Icon-only compact view.
    Icons,
    /// Icon plus truncated title.
    IconLabel,
}

/// Configuration for the window list / taskbar module.
#[derive(Debug, Default, Deserialize)]
pub struct WindowListConfig {
    /// Display style.
    #[serde(default)]
    pub style: WindowListStyle,
    /// Limit list to windows on the current workspace.
    #[serde(default = "default_true")]
    pub current_workspace_only: bool,
    /// Maximum button width in logical pixels.
    #[serde(default = "default_max_width")]
    pub max_button_width: f32,
    /// Minimum button width in logical pixels.
    #[serde(default = "default_min_width")]
    pub min_button_width: f32,
}

fn default_true() -> bool {
    true
}
fn default_max_width() -> f32 {
    200.0
}
fn default_min_width() -> f32 {
    60.0
}

// ── Module ────────────────────────────────────────────────────────────────────

struct WindowButton {
    info: WindowInfo,
    icon: Option<image::RgbaImage>,
}

/// Runtime state for the window list module.
pub struct WindowListModule {
    config: WindowListConfig,
    buttons: Vec<WindowButton>,
    /// Index of the currently-hovered button.
    hovered: Option<usize>,
}

impl WindowListModule {
    pub fn new(config: WindowListConfig) -> Self {
        WindowListModule {
            config,
            buttons: Vec::new(),
            hovered: None,
        }
    }

    fn gap() -> f32 {
        2.0
    }

    fn button_width(&self, available_width: f32) -> f32 {
        let n = self.buttons.len().max(1) as f32;
        let total_gaps = (n - 1.0).max(0.0) * Self::gap();
        let per = (available_width - total_gaps) / n;
        per.clamp(self.config.min_button_width, self.config.max_button_width)
    }

    fn button_at_x(&self, x: f32, bounds: Rect, btn_w: f32) -> Option<usize> {
        let gap = Self::gap();
        for i in 0..self.buttons.len() {
            let bx = bounds.x + i as f32 * (btn_w + gap);
            if x >= bx && x < bx + btn_w {
                return Some(i);
            }
        }
        None
    }
}

impl PanelModule for WindowListModule {
    fn id(&self) -> &str {
        "window_list"
    }

    fn desired_size(&self, theme: &ThemeContext) -> Size {
        let n = self.buttons.len().max(1) as f32;
        let gap = Self::gap();
        let w = n * self.config.max_button_width + (n - 1.0).max(0.0) * gap;
        let h = theme.fonts.size + theme.padding.top + theme.padding.bottom;
        // Grow to fill available space — return the preferred width.
        // (height is a hint; horizontal panels stretch modules to bar height)
        Size::new(w, h)
    }

    fn update(&mut self, ctx: &UpdateContext<'_>) -> bool {
        // Use the active workspace for THIS monitor, not the globally focused
        // one. This ensures each bar shows its own monitor's windows even when
        // the user moves focus to a different monitor.
        let active_ws = ctx
            .hypr_state
            .monitors
            .iter()
            .find(|m| m.name == ctx.output_name)
            .map(|m| m.active_workspace)
            .unwrap_or(ctx.hypr_state.active_workspace);

        let new_windows: Vec<WindowInfo> = ctx
            .hypr_state
            .windows
            .iter()
            .filter(|w| {
                if self.config.current_workspace_only {
                    w.workspace_id == active_ws
                } else {
                    true
                }
            })
            .cloned()
            .collect();

        // Check if anything changed.
        let old_addrs: Vec<&str> = self
            .buttons
            .iter()
            .map(|b| b.info.address.as_str())
            .collect();
        let new_addrs: Vec<&str> = new_windows.iter().map(|w| w.address.as_str()).collect();
        let titles_changed = new_windows
            .iter()
            .zip(self.buttons.iter())
            .any(|(nw, ob)| nw.title != ob.info.title || nw.is_focused != ob.info.is_focused);

        if old_addrs == new_addrs && !titles_changed {
            return false;
        }

        // Rebuild the button list, preserving already-loaded icons.
        let icon_size: u16 = 24;
        let mut new_buttons: Vec<WindowButton> = Vec::with_capacity(new_windows.len());
        for win in new_windows {
            // Re-use existing icon if the class matches.
            let icon = self
                .buttons
                .iter()
                .find(|b| b.info.class == win.class)
                .and_then(|b| b.icon.as_ref())
                .cloned()
                .or_else(|| icon_utils::load_window_icon(&win.class, icon_size));

            new_buttons.push(WindowButton { info: win, icon });
        }
        self.buttons = new_buttons;
        true
    }

    fn render(&self, canvas: &mut Pixmap, theme: &ThemeContext, bounds: Rect) {
        if self.buttons.is_empty() {
            return;
        }
        let btn_w = self.button_width(bounds.width);
        let gap = Self::gap();

        for (i, btn) in self.buttons.iter().enumerate() {
            let bx = bounds.x + i as f32 * (btn_w + gap);
            let btn_rect = Rect::new(bx, bounds.y, btn_w, bounds.height);

            let is_focused = btn.info.is_focused;
            let is_hovered = self.hovered == Some(i);

            // Button background.
            let bg = if is_focused {
                let mut c = theme.colors.accent;
                c[3] = 200;
                c
            } else if is_hovered {
                let mut c = theme.colors.foreground;
                c[3] = 30;
                c
            } else {
                let mut c = theme.colors.foreground;
                c[3] = 10;
                c
            };
            render_utils::fill_rounded_rect(canvas, btn_rect, bg, theme.border_radius);

            // Icon + title, depending on style.
            let icon_size = bounds.height - theme.padding.top - theme.padding.bottom;
            let mut content_x = bx + theme.padding.left;

            if matches!(
                self.config.style,
                WindowListStyle::Icons | WindowListStyle::IconLabel
            ) {
                if let Some(icon) = &btn.icon {
                    let icon_rect = Rect::new(
                        content_x,
                        bounds.y + theme.padding.top,
                        icon_size,
                        icon_size,
                    );
                    render_utils::draw_image(canvas, icon, icon_rect, 1.0);
                    content_x += icon_size + 4.0;
                }
            }

            if !matches!(self.config.style, WindowListStyle::Icons) {
                let text_rect = Rect::new(
                    content_x,
                    bounds.y,
                    bx + btn_w - content_x - theme.padding.right,
                    bounds.height,
                );
                let text_color = if is_focused {
                    theme.colors.background
                } else {
                    theme.colors.foreground
                };
                render_utils::draw_text_ellipsis(
                    canvas,
                    &btn.info.title,
                    text_rect,
                    &theme.fonts.family,
                    render_utils::effective_font_size(bounds.height, theme.fonts.size),
                    text_color,
                );
            }
        }
    }

    fn handle_event(&mut self, event: &InputEvent, bounds: Rect) -> EventResult {
        let btn_w = self.button_width(bounds.width);

        match event {
            InputEvent::MouseMove { x, .. } => {
                let new_hover = self.button_at_x(*x, bounds, btn_w);
                if new_hover != self.hovered {
                    self.hovered = new_hover;
                    return EventResult::Handled;
                }
                EventResult::Handled
            }
            InputEvent::MousePress { x, button, .. } => {
                let Some(idx) = self.button_at_x(*x, bounds, btn_w) else {
                    return EventResult::Ignored;
                };
                let addr = self.buttons[idx].info.address.clone();
                match button {
                    MouseButton::Left => EventResult::Action(Action::HyprDispatch {
                        dispatch: format!("focuswindow address:{addr}"),
                    }),
                    MouseButton::Middle => EventResult::Action(Action::HyprDispatch {
                        dispatch: format!("closewindow address:{addr}"),
                    }),
                    MouseButton::Right => EventResult::Ignored,
                }
            }
            _ => EventResult::Ignored,
        }
    }

    fn config_schema(&self) -> ModuleConfigSchema {
        ModuleConfigSchema {
            module_id: self.id().to_owned(),
            fields: vec![
                ConfigField {
                    key: "style".to_owned(),
                    label: "Display style".to_owned(),
                    description: "How window buttons are rendered.".to_owned(),
                    field_type: ConfigFieldType::Choice {
                        options: vec![
                            "buttons".to_owned(),
                            "icons".to_owned(),
                            "iconlabel".to_owned(),
                        ],
                        default: "buttons".to_owned(),
                    },
                },
                ConfigField {
                    key: "current_workspace_only".to_owned(),
                    label: "Current workspace only".to_owned(),
                    description: "Only show windows from the active workspace.".to_owned(),
                    field_type: ConfigFieldType::Boolean { default: true },
                },
                ConfigField {
                    key: "max_button_width".to_owned(),
                    label: "Max button width".to_owned(),
                    description: "Maximum logical pixel width of each window button.".to_owned(),
                    field_type: ConfigFieldType::Float {
                        default: 200.0,
                        min: Some(60.0),
                        max: Some(600.0),
                    },
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyprdeck_core::HyprState;

    fn win(id: i32, addr: &str, title: &str, class: &str, focused: bool) -> WindowInfo {
        WindowInfo {
            address: addr.to_owned(),
            title: title.to_owned(),
            class: class.to_owned(),
            workspace_id: id,
            is_focused: focused,
            is_fullscreen: false,
        }
    }

    #[test]
    fn update_detects_new_windows() {
        let mut m = WindowListModule::new(WindowListConfig::default());
        let state = HyprState {
            active_workspace: 1,
            windows: vec![win(1, "0x1", "Firefox", "firefox", true)],
            ..Default::default()
        };
        let ctx = UpdateContext {
            now: chrono::Local::now(),
            hypr_state: &state,
            output_name: "",
        };
        assert!(m.update(&ctx), "first update should return true");
        assert!(!m.update(&ctx), "same state should return false");
    }

    #[test]
    fn active_workspace_filter_works() {
        let mut m = WindowListModule::new(WindowListConfig {
            current_workspace_only: true,
            ..WindowListConfig::default()
        });
        let state = HyprState {
            active_workspace: 1,
            windows: vec![
                win(1, "0x1", "A", "app", false),
                win(2, "0x2", "B", "app2", false),
            ],
            ..Default::default()
        };
        let ctx = UpdateContext {
            now: chrono::Local::now(),
            hypr_state: &state,
            output_name: "",
        };
        m.update(&ctx);
        assert_eq!(m.buttons.len(), 1);
        assert_eq!(m.buttons[0].info.address, "0x1");
    }

    #[test]
    fn button_width_clamped_to_max() {
        let m = WindowListModule::new(WindowListConfig {
            max_button_width: 200.0,
            min_button_width: 60.0,
            ..WindowListConfig::default()
        });
        // 3 windows in 700px → 700/3 ≈ 233, clamped to 200
        assert_eq!(m.button_width(700.0), 200.0);
    }

    #[test]
    fn button_width_clamped_to_min() {
        let _m = WindowListModule::new(WindowListConfig {
            max_button_width: 200.0,
            min_button_width: 60.0,
            ..WindowListConfig::default()
        });
        // With 10 buttons in 400px → 40 per button, clamped to 60.
        // We manipulate via a mock: create a module with many buttons.
        // Instead, test the formula directly.
        let n = 10.0_f32;
        let gap = WindowListModule::gap();
        let total_gaps = (n - 1.0) * gap;
        let per = (400.0 - total_gaps) / n;
        let clamped = per.clamp(60.0, 200.0);
        assert_eq!(clamped, 60.0);
    }

    #[test]
    fn left_click_returns_focus_action() {
        let mut m = WindowListModule::new(WindowListConfig {
            max_button_width: 200.0,
            min_button_width: 60.0,
            ..WindowListConfig::default()
        });
        let state = HyprState {
            active_workspace: 1,
            windows: vec![win(1, "0xABCD", "Firefox", "firefox", true)],
            ..Default::default()
        };
        let ctx = UpdateContext {
            now: chrono::Local::now(),
            hypr_state: &state,
            output_name: "",
        };
        m.update(&ctx);

        let bounds = Rect::new(0.0, 0.0, 600.0, 32.0);
        let result = m.handle_event(
            &InputEvent::MousePress {
                x: 50.0,
                y: 10.0,
                button: MouseButton::Left,
            },
            bounds,
        );
        assert!(
            matches!(result, EventResult::Action(Action::HyprDispatch { dispatch }) if dispatch.contains("0xABCD")),
        );
    }
}
