//! HyprCube settings launcher module.

use serde::Deserialize;

use hyprdeck_core::{
    Action, ConfigField, ConfigFieldType, EventResult, InputEvent, ModuleConfigSchema, MouseButton,
    PanelModule, Pixmap, Rect, Size, ThemeContext, UpdateContext,
};

use crate::{icon_utils, render_utils};

#[derive(Debug, Deserialize)]
pub struct HyprcubeConfig {
    /// Executable used to open the settings application.
    #[serde(default = "default_command")]
    pub command: String,
    /// Freedesktop icon name used by the launcher.
    #[serde(default = "default_icon")]
    pub icon: String,
}

fn default_command() -> String {
    "hyprcube".to_owned()
}

fn default_icon() -> String {
    "start-here".to_owned()
}

impl Default for HyprcubeConfig {
    fn default() -> Self {
        Self {
            command: default_command(),
            icon: default_icon(),
        }
    }
}

pub struct HyprcubeModule {
    config: HyprcubeConfig,
    icon: Option<image::RgbaImage>,
    hovered: bool,
    pressed: bool,
}

impl HyprcubeModule {
    pub fn new(config: HyprcubeConfig) -> Self {
        let icon = icon_utils::load_freedesktop_icon(&config.icon, 24);
        Self {
            config,
            icon,
            hovered: false,
            pressed: false,
        }
    }
}

impl PanelModule for HyprcubeModule {
    fn id(&self) -> &str {
        "hyprcube"
    }

    fn desired_size(&self, theme: &ThemeContext) -> Size {
        Size::new(theme.icon_slot_size, theme.icon_slot_size)
    }

    fn update(&mut self, _ctx: &UpdateContext<'_>) -> bool {
        false
    }

    fn render(&self, canvas: &mut Pixmap, theme: &ThemeContext, bounds: Rect) {
        if self.pressed {
            render_utils::fill_rounded_rect_alpha(
                canvas,
                bounds,
                theme.colors.accent,
                theme.border_radius,
                0.5,
            );
        } else if self.hovered {
            render_utils::fill_rounded_rect_alpha(
                canvas,
                bounds,
                theme.colors.foreground,
                theme.border_radius,
                0.12,
            );
        }

        let icon_bounds = render_utils::icon_content_rect(bounds, theme.icon_padding);
        if let Some(icon) = &self.icon {
            render_utils::draw_image(canvas, icon, icon_bounds, 1.0);
        } else {
            render_utils::draw_menu_icon(canvas, icon_bounds, theme.colors.foreground);
        }
    }

    fn handle_event(&mut self, event: &InputEvent, _bounds: Rect) -> EventResult {
        match event {
            InputEvent::MouseMove { .. } => {
                self.hovered = true;
                EventResult::Handled
            }
            InputEvent::MousePress {
                button: MouseButton::Left,
                ..
            } => {
                self.pressed = true;
                EventResult::Handled
            }
            InputEvent::MouseRelease {
                button: MouseButton::Left,
                ..
            } => {
                self.pressed = false;
                EventResult::Action(Action::Exec {
                    command: self.config.command.clone(),
                    args: Vec::new(),
                })
            }
            _ => EventResult::Ignored,
        }
    }

    fn config_schema(&self) -> ModuleConfigSchema {
        ModuleConfigSchema {
            module_id: self.id().to_owned(),
            fields: vec![
                ConfigField {
                    key: "command".to_owned(),
                    label: "HyprCube executable".to_owned(),
                    description: "Program launched when the button is clicked.".to_owned(),
                    field_type: ConfigFieldType::Text {
                        default: default_command(),
                    },
                },
                ConfigField {
                    key: "icon".to_owned(),
                    label: "Icon name".to_owned(),
                    description: "Freedesktop icon used for the HyprCube launcher.".to_owned(),
                    field_type: ConfigFieldType::Text {
                        default: default_icon(),
                    },
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launcher_has_no_popup_and_executes_hyprcube() {
        let mut module = HyprcubeModule::new(HyprcubeConfig::default());
        assert!(!module.has_popup());
        let result = module.handle_event(
            &InputEvent::MouseRelease {
                x: 4.0,
                y: 4.0,
                button: MouseButton::Left,
            },
            Rect::new(0.0, 0.0, 24.0, 24.0),
        );
        assert!(matches!(
            result,
            EventResult::Action(Action::Exec { command, args })
                if command == "hyprcube" && args.is_empty()
        ));
    }
}
