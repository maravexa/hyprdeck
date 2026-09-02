//! Application-menu / start-button module.
//!
//! Renders a single button that executes a configured action on click.
//! Optionally shows a freedesktop icon and/or a text label.

use serde::Deserialize;

use hyprdeck_core::{
    Action, ConfigField, ConfigFieldType, EventResult, InputEvent, ModuleConfigSchema, MouseButton,
    PanelModule, Pixmap, Rect, Size, ThemeContext, UpdateContext,
};

use crate::{icon_utils, render_utils};

// ── Config ────────────────────────────────────────────────────────────────────

/// Configuration for the application menu / start button module.
#[derive(Debug, Deserialize)]
pub struct MenuConfig {
    /// Label text displayed on the button; empty string shows icon only.
    #[serde(default = "default_label")]
    pub label: String,
    /// XDG icon name for the button icon.
    #[serde(default = "default_icon")]
    pub icon: String,
    /// Action triggered when the button is clicked.
    #[serde(default = "default_action")]
    pub action: Action,
}

fn default_label() -> String {
    ">>".to_owned()
}

fn default_icon() -> String {
    String::new()
}

fn default_action() -> Action {
    Action::HyprlandExec {
        variable: "menu".to_owned(),
        fallback: Some("wofi --show drun".to_owned()),
    }
}

impl Default for MenuConfig {
    fn default() -> Self {
        MenuConfig {
            label: default_label(),
            icon: default_icon(),
            action: default_action(),
        }
    }
}

// ── Module ────────────────────────────────────────────────────────────────────

/// Runtime state for the menu button module.
pub struct MenuModule {
    config: MenuConfig,
    /// Loaded icon image, if the icon name resolved successfully.
    icon_image: Option<image::RgbaImage>,
    /// Whether the pointer is currently hovering over this button.
    hovered: bool,
    /// Whether the button is in a pressed-down visual state.
    pressed: bool,
}

impl MenuModule {
    pub fn new(config: MenuConfig) -> Self {
        let icon_image = if config.icon.is_empty() {
            None
        } else {
            let img = icon_utils::load_freedesktop_icon(&config.icon, 24);
            if img.is_none() {
                tracing::warn!(
                    "Menu module: icon '{}' not found in icon theme",
                    config.icon
                );
            }
            img
        };

        MenuModule {
            config,
            icon_image,
            hovered: false,
            pressed: false,
        }
    }

    fn shows_icon(&self) -> bool {
        !self.config.icon.is_empty()
    }
}

impl PanelModule for MenuModule {
    fn id(&self) -> &str {
        "menu_button"
    }

    fn desired_size(&self, theme: &ThemeContext) -> Size {
        let h = theme.fonts.size + theme.padding.top + theme.padding.bottom;
        let font_size = render_utils::effective_font_size(h, theme.fonts.size);
        let icon_w = if self.shows_icon() {
            font_size + 4.0
        } else {
            0.0
        };
        let label_w = if self.config.label.is_empty() {
            0.0
        } else {
            render_utils::estimate_text_width(&self.config.label, font_size) + 4.0
        };
        let content_w = (icon_w + label_w).max(font_size);
        let w = content_w + theme.padding.left + theme.padding.right;
        Size::new(w, h)
    }

    fn update(&mut self, _ctx: &UpdateContext<'_>) -> bool {
        false // Stateless apart from hover/pressed state.
    }

    fn render(&self, canvas: &mut Pixmap, theme: &ThemeContext, bounds: Rect) {
        // Draw background highlight when hovered or pressed.
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

        let mut x = bounds.x + theme.padding.left;
        let content_h = render_utils::effective_font_size(bounds.height, theme.fonts.size);
        let y = bounds.y + (bounds.height - content_h) / 2.0;

        // Draw icon.
        if self.shows_icon() {
            let icon_rect = Rect::new(x, y, content_h, content_h);
            if let Some(icon) = &self.icon_image {
                render_utils::draw_image(canvas, icon, icon_rect, 1.0);
            } else {
                render_utils::draw_menu_icon(canvas, icon_rect, theme.colors.foreground);
            }
            x += content_h + 4.0;
        }

        // Draw label.
        if !self.config.label.is_empty() {
            let label_rect = Rect::new(x, bounds.y, bounds.x + bounds.width - x, bounds.height);
            let font = theme
                .fonts
                .bold_family
                .as_deref()
                .unwrap_or(&theme.fonts.family);
            render_utils::draw_text(
                canvas,
                &self.config.label,
                label_rect,
                font,
                content_h,
                theme.colors.foreground,
            );
        }
    }

    fn handle_event(&mut self, event: &InputEvent, _bounds: Rect) -> EventResult {
        match event {
            InputEvent::MouseMove { .. } => {
                if !self.hovered {
                    self.hovered = true;
                    return EventResult::Handled;
                }
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
                // The panel only routes this release while the pointer is
                // inside our bounds, so action dispatch must not depend on a
                // preceding motion event having established hover state.
                EventResult::Action(self.config.action.clone())
            }
            _ => EventResult::Ignored,
        }
    }

    fn config_schema(&self) -> ModuleConfigSchema {
        ModuleConfigSchema {
            module_id: self.id().to_owned(),
            fields: vec![
                ConfigField {
                    key: "label".to_owned(),
                    label: "Button label".to_owned(),
                    description:
                        "Text shown on the menu button. Empty string for icon-only mode.".to_owned(),
                    field_type: ConfigFieldType::Text {
                        default: default_label(),
                    },
                },
                ConfigField {
                    key: "icon".to_owned(),
                    label: "Icon name".to_owned(),
                    description:
                        "XDG icon name for the button (e.g. \"start-here\", \"distributor-logo-arch\").".to_owned(),
                    field_type: ConfigFieldType::Text { default: String::new() },
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyprdeck_core::HyprState;

    fn default_module() -> MenuModule {
        MenuModule::new(MenuConfig::default())
    }

    #[test]
    fn update_always_returns_false() {
        let mut m = default_module();
        let state = HyprState::default();
        let ctx = UpdateContext {
            now: chrono::Local::now(),
            hypr_state: &state,
            output_name: "",
        };
        assert!(!m.update(&ctx));
    }

    #[test]
    fn default_action_uses_hyprland_menu_variable() {
        assert!(matches!(
            MenuConfig::default().action,
            Action::HyprlandExec {
                variable,
                fallback: Some(_)
            } if variable == "menu"
        ));
    }

    #[test]
    fn click_returns_configured_action() {
        let action = Action::Exec {
            command: "rofi".to_owned(),
            args: vec!["-show".to_owned(), "drun".to_owned()],
        };
        let mut m = MenuModule::new(MenuConfig {
            action: action.clone(),
            ..MenuConfig::default()
        });
        let bounds = Rect::new(0.0, 0.0, 48.0, 32.0);
        // Press then release.
        m.handle_event(
            &InputEvent::MousePress {
                x: 10.0,
                y: 10.0,
                button: MouseButton::Left,
            },
            bounds,
        );
        let result = m.handle_event(
            &InputEvent::MouseRelease {
                x: 10.0,
                y: 10.0,
                button: MouseButton::Left,
            },
            bounds,
        );
        assert!(
            matches!(result, EventResult::Action(Action::Exec { command, .. }) if command == "rofi")
        );
    }

    #[test]
    fn hover_state_toggles() {
        let mut m = default_module();
        let bounds = Rect::new(0.0, 0.0, 48.0, 32.0);
        assert!(!m.hovered);
        m.handle_event(&InputEvent::MouseMove { x: 5.0, y: 5.0 }, bounds);
        assert!(m.hovered);
    }

    #[test]
    fn missing_icon_does_not_panic() {
        let m = MenuModule::new(MenuConfig {
            icon: "__no_such_icon__".to_owned(),
            ..MenuConfig::default()
        });
        assert!(m.icon_image.is_none());
        assert!(m.shows_icon());
    }

    #[test]
    fn missing_theme_icon_renders_visible_fallback() {
        let m = MenuModule::new(MenuConfig {
            icon: "__no_such_icon__".to_owned(),
            ..MenuConfig::default()
        });
        let theme = ThemeContext {
            colors: hyprdeck_core::ColorPalette {
                background: [30, 30, 30, 255],
                foreground: [255, 255, 255, 255],
                accent: [80, 160, 255, 255],
                urgent: [255, 80, 80, 255],
                separator: [128, 128, 128, 128],
            },
            fonts: hyprdeck_core::FontConfig {
                family: "sans-serif".to_owned(),
                size: 13.0,
                bold_family: None,
                mono_family: None,
            },
            padding: hyprdeck_core::Padding::default(),
            border_radius: 0.0,
            opacity: 1.0,
            icon_slot_size: 24.0,
            icon_padding: 2.0,
            verbose_text_padding: 4.0,
            module_styles: hyprdeck_core::ResolvedModuleStyles::default(),
        };
        let mut canvas = Pixmap::new(48, 32).unwrap();
        m.render(&mut canvas, &theme, Rect::new(0.0, 0.0, 48.0, 32.0));
        assert!(canvas.data().chunks_exact(4).any(|pixel| pixel[3] > 0));
    }
}
