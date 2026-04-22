//! Power / session module.
//!
//! Displays a power symbol (⏻) on the panel. Clicking opens a popup menu
//! with five session actions: lock, log out, suspend, reboot, and shut down.
//!
//! Commands are configurable via TOML; defaults use `systemctl` and `hyprctl`.

use serde::Deserialize;

use hyprdeck_core::{
    Action, ConfigField, ConfigFieldType, EventResult, InputEvent, ModuleConfigSchema, MouseButton,
    PanelModule, Pixmap, PopupContent, PopupEventResult, Rect, Size, ThemeContext, UpdateContext,
};

use crate::render_utils;

// ── Config ────────────────────────────────────────────────────────────────────

/// Shell commands executed by each power action.
#[derive(Debug, Clone, Deserialize)]
pub struct PowerCommands {
    #[serde(default = "default_shutdown")]
    pub shutdown: String,
    #[serde(default = "default_reboot")]
    pub reboot: String,
    #[serde(default = "default_logout")]
    pub logout: String,
    #[serde(default = "default_lock")]
    pub lock: String,
    #[serde(default = "default_suspend")]
    pub suspend: String,
}

fn default_shutdown() -> String {
    "systemctl poweroff".into()
}
fn default_reboot() -> String {
    "systemctl reboot".into()
}
fn default_logout() -> String {
    "hyprctl dispatch exit".into()
}
fn default_lock() -> String {
    "hyprlock".into()
}
fn default_suspend() -> String {
    "systemctl suspend".into()
}

impl Default for PowerCommands {
    fn default() -> Self {
        Self {
            shutdown: default_shutdown(),
            reboot: default_reboot(),
            logout: default_logout(),
            lock: default_lock(),
            suspend: default_suspend(),
        }
    }
}

/// Configuration for the power module.
#[derive(Debug, Default, Deserialize)]
pub struct PowerConfig {
    #[serde(default)]
    pub commands: PowerCommands,
}

// ── Module ────────────────────────────────────────────────────────────────────

/// Runtime state for the power / session module.
pub struct PowerModule {
    config: PowerConfig,
    /// Whether the cursor is hovering over the icon (for visual feedback).
    hovered: bool,
}

impl PowerModule {
    pub fn new(config: PowerConfig) -> Self {
        Self {
            config,
            hovered: false,
        }
    }
}

impl PanelModule for PowerModule {
    fn id(&self) -> &str {
        "power"
    }

    fn desired_size(&self, theme: &ThemeContext) -> Size {
        // Square tile: height drives both dimensions.
        let h = theme.fonts.size + theme.padding.top + theme.padding.bottom;
        let font_size = render_utils::effective_font_size(h, theme.fonts.size);
        let w = font_size + theme.padding.left + theme.padding.right;
        Size::new(w, h)
    }

    fn update(&mut self, _ctx: &UpdateContext<'_>) -> bool {
        false
    }

    fn render(&self, canvas: &mut Pixmap, theme: &ThemeContext, bounds: Rect) {
        let icon_size = (bounds.height * 0.8).floor();

        if self.hovered {
            let bg = dim_color(theme.colors.foreground, 0.12);
            render_utils::fill_rounded_rect(canvas, bounds, bg, theme.border_radius);
        }

        let color = if self.hovered {
            theme.colors.accent
        } else {
            theme.colors.foreground
        };
        render_utils::draw_text_centered(
            canvas,
            "\u{23FB}",
            bounds,
            &theme.fonts.family,
            icon_size,
            color,
        );
    }

    fn handle_event(&mut self, event: &InputEvent, _bounds: Rect) -> EventResult {
        match event {
            InputEvent::MouseMove { .. } => {
                if !self.hovered {
                    self.hovered = true;
                    EventResult::Handled
                } else {
                    EventResult::Ignored
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
                    key: "commands.shutdown".to_owned(),
                    label: "Shutdown command".to_owned(),
                    description: "Command to power off the system.".to_owned(),
                    field_type: ConfigFieldType::Text {
                        default: default_shutdown(),
                    },
                },
                ConfigField {
                    key: "commands.reboot".to_owned(),
                    label: "Reboot command".to_owned(),
                    description: "Command to restart the system.".to_owned(),
                    field_type: ConfigFieldType::Text {
                        default: default_reboot(),
                    },
                },
                ConfigField {
                    key: "commands.logout".to_owned(),
                    label: "Log out command".to_owned(),
                    description: "Command to end the Hyprland session.".to_owned(),
                    field_type: ConfigFieldType::Text {
                        default: default_logout(),
                    },
                },
                ConfigField {
                    key: "commands.lock".to_owned(),
                    label: "Lock command".to_owned(),
                    description: "Command to lock the screen.".to_owned(),
                    field_type: ConfigFieldType::Text {
                        default: default_lock(),
                    },
                },
                ConfigField {
                    key: "commands.suspend".to_owned(),
                    label: "Suspend command".to_owned(),
                    description: "Command to suspend the system.".to_owned(),
                    field_type: ConfigFieldType::Text {
                        default: default_suspend(),
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
        Some(Box::new(PowerPopup::new(&self.config.commands)))
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn dim_color(color: [u8; 4], opacity: f32) -> [u8; 4] {
    let a = (color[3] as f32 * opacity.clamp(0.0, 1.0)) as u8;
    [color[0], color[1], color[2], a]
}

// ── Power popup ───────────────────────────────────────────────────────────────

struct PowerPopupItem {
    label: &'static str,
    icon: &'static str,
    command: String,
}

/// Popup content for the power module — a vertical list of session actions.
pub struct PowerPopup {
    items: Vec<PowerPopupItem>,
    hovered_index: Option<usize>,
}

impl PowerPopup {
    pub fn new(commands: &PowerCommands) -> Self {
        Self {
            items: vec![
                PowerPopupItem {
                    label: "Lock Screen",
                    icon: "🔒",
                    command: commands.lock.clone(),
                },
                PowerPopupItem {
                    label: "Log Out",
                    icon: "🚪",
                    command: commands.logout.clone(),
                },
                PowerPopupItem {
                    label: "Suspend",
                    icon: "💤",
                    command: commands.suspend.clone(),
                },
                PowerPopupItem {
                    label: "Reboot",
                    icon: "🔄",
                    command: commands.reboot.clone(),
                },
                PowerPopupItem {
                    label: "Shut Down",
                    icon: "\u{23FB}",
                    command: commands.shutdown.clone(),
                },
            ],
            hovered_index: None,
        }
    }
}

const ITEM_HEIGHT: f32 = 36.0;

impl PopupContent for PowerPopup {
    fn desired_size(&self, _theme: &ThemeContext) -> Size {
        let height = self.items.len() as f32 * ITEM_HEIGHT + 12.0; // 6px top/bottom padding
        Size::new(200.0, height)
    }

    fn render(&self, canvas: &mut Pixmap, theme: &ThemeContext, bounds: Rect) {
        let font_size = 13.0;

        for (i, item) in self.items.iter().enumerate() {
            let item_rect = Rect::new(
                bounds.x,
                bounds.y + 6.0 + i as f32 * ITEM_HEIGHT,
                bounds.width,
                ITEM_HEIGHT,
            );

            // Hover highlight
            if Some(i) == self.hovered_index {
                let hover_color = [
                    theme.colors.accent[0],
                    theme.colors.accent[1],
                    theme.colors.accent[2],
                    76, // ~30% opacity
                ];
                render_utils::fill_rounded_rect(canvas, item_rect, hover_color, 4.0);
            }

            // Icon (left-aligned in a 32px column)
            let icon_rect = Rect::new(item_rect.x, item_rect.y, 32.0, ITEM_HEIGHT);
            render_utils::draw_text_centered(
                canvas,
                item.icon,
                icon_rect,
                &theme.fonts.family,
                font_size + 2.0,
                theme.colors.foreground,
            );

            // Label
            let label_rect = Rect::new(
                item_rect.x + 32.0,
                item_rect.y,
                item_rect.width - 32.0,
                ITEM_HEIGHT,
            );
            render_utils::draw_text(
                canvas,
                item.label,
                label_rect,
                &theme.fonts.family,
                font_size,
                theme.colors.foreground,
            );
        }
    }

    fn handle_event(&mut self, event: &InputEvent, bounds: Rect) -> PopupEventResult {
        match event {
            InputEvent::MouseMove { x, y } => {
                let relative_y = y - (bounds.y + 6.0);
                let index = (relative_y / ITEM_HEIGHT) as usize;
                let in_bounds = *x >= bounds.x
                    && *x < bounds.x + bounds.width
                    && relative_y >= 0.0;
                let new_hover = if in_bounds && index < self.items.len() {
                    Some(index)
                } else {
                    None
                };
                if new_hover != self.hovered_index {
                    self.hovered_index = new_hover;
                    return PopupEventResult::Handled;
                }
                PopupEventResult::Ignored
            }
            InputEvent::MousePress {
                x,
                y,
                button: MouseButton::Left,
            } => {
                let relative_y = y - (bounds.y + 6.0);
                let index = (relative_y / ITEM_HEIGHT) as usize;
                let in_bounds = *x >= bounds.x
                    && *x < bounds.x + bounds.width
                    && relative_y >= 0.0;
                if in_bounds && index < self.items.len() {
                    let cmd = &self.items[index].command;
                    let parts: Vec<&str> = cmd.split_whitespace().collect();
                    if let Some((program, args)) = parts.split_first() {
                        return PopupEventResult::Action(Action::Exec {
                            command: program.to_string(),
                            args: args.iter().map(|s| s.to_string()).collect(),
                        });
                    }
                }
                PopupEventResult::Ignored
            }
            _ => PopupEventResult::Ignored,
        }
    }

    fn update(&mut self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_id_is_power() {
        let m = PowerModule::new(PowerConfig::default());
        assert_eq!(m.id(), "power");
    }

    #[test]
    fn has_popup_is_true() {
        let m = PowerModule::new(PowerConfig::default());
        assert!(m.has_popup());
    }

    #[test]
    fn popup_content_returns_some() {
        let m = PowerModule::new(PowerConfig::default());
        assert!(m.popup_content().is_some());
    }

    #[test]
    fn power_popup_has_five_items() {
        let popup = PowerPopup::new(&PowerCommands::default());
        assert_eq!(popup.items.len(), 5);
    }

    #[test]
    fn default_commands_are_reasonable() {
        let cmds = PowerCommands::default();
        assert!(cmds.shutdown.contains("poweroff") || cmds.shutdown.contains("shutdown"));
        assert!(cmds.reboot.contains("reboot"));
        assert!(cmds.logout.contains("exit") || cmds.logout.contains("logout"));
        assert!(!cmds.lock.is_empty());
        assert!(!cmds.suspend.is_empty());
    }

    #[test]
    fn popup_desired_size_is_nonzero() {
        use hyprdeck_core::{ColorPalette, FontConfig, Padding, ResolvedModuleStyles};
        let theme = ThemeContext {
            colors: ColorPalette {
                background: [30, 30, 30, 230],
                foreground: [255, 255, 255, 255],
                accent: [80, 160, 255, 255],
                urgent: [255, 80, 80, 255],
                separator: [128, 128, 128, 128],
            },
            fonts: FontConfig {
                family: "sans-serif".into(),
                size: 13.0,
                bold_family: None,
            },
            padding: Padding::default(),
            border_radius: 4.0,
            opacity: 1.0,
            module_styles: ResolvedModuleStyles::default(),
        };
        let popup = PowerPopup::new(&PowerCommands::default());
        let size = popup.desired_size(&theme);
        assert!(size.width > 0.0);
        assert!(size.height > 0.0);
    }
}
