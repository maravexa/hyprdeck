//! Pinned application shortcuts (dock / favorites) module.
//!
//! Displays a row of icon buttons; each left-click fires the configured action.
//! Icons are loaded eagerly at construction time.  Missing icons render as a
//! coloured square with the first letter of the item label centred inside.

use serde::Deserialize;

use hyprdeck_core::{
    Action, ConfigField, ConfigFieldType, EventResult, InputEvent, ModuleConfigSchema, MouseButton,
    PanelModule, Pixmap, Rect, Size, ThemeContext, UpdateContext,
};

use crate::{icon_utils, render_utils};

// ── Config ────────────────────────────────────────────────────────────────────

/// A single pinned application entry.
#[derive(Debug, Deserialize, Clone)]
pub struct FavoriteEntry {
    /// Tooltip / label (shown on hover in a future release).
    pub label: String,
    /// XDG icon name used to look up the application icon.
    pub icon: String,
    /// Action executed on left-click.
    pub action: Action,
}

/// Configuration for the pinned-apps / favorites module.
#[derive(Debug, Default, Deserialize)]
pub struct FavoritesConfig {
    /// Ordered list of pinned entries.
    #[serde(default)]
    pub entries: Vec<FavoriteEntry>,
    /// Override icon size in logical pixels (defaults to bar height minus padding).
    pub icon_size: Option<f32>,
    /// Show a small dot under icons whose applications have open windows.
    #[serde(default = "default_true")]
    pub show_running_indicator: bool,
}

fn default_true() -> bool {
    true
}

// ── Module ────────────────────────────────────────────────────────────────────

struct LoadedEntry {
    entry: FavoriteEntry,
    /// Pre-loaded RGBA icon, or `None` when the icon was not found.
    icon: Option<image::RgbaImage>,
}

/// Runtime state for the favorites / dock shortcut module.
pub struct FavoritesModule {
    config: FavoritesConfig,
    loaded: Vec<LoadedEntry>,
    /// Index of the slot currently under the pointer.
    hovered_index: Option<usize>,
}

impl FavoritesModule {
    pub fn new(config: FavoritesConfig) -> Self {
        let icon_size: u16 = config.icon_size.unwrap_or(24.0) as u16;
        let loaded = config
            .entries
            .iter()
            .map(|e| LoadedEntry {
                icon: icon_utils::load_freedesktop_icon(&e.icon, icon_size),
                entry: e.clone(),
            })
            .collect();

        FavoritesModule {
            config,
            loaded,
            hovered_index: None,
        }
    }

    fn effective_icon_size(config: &FavoritesConfig, theme: &ThemeContext) -> f32 {
        config.icon_size.unwrap_or(theme.fonts.size)
    }

    fn gap() -> f32 {
        4.0
    }

    fn item_at_x(&self, x: f32, bounds: Rect, icon_size: f32) -> Option<usize> {
        let gap = Self::gap();
        for i in 0..self.loaded.len() {
            let item_x = bounds.x + i as f32 * (icon_size + gap);
            if x >= item_x && x < item_x + icon_size {
                return Some(i);
            }
        }
        None
    }
}

impl PanelModule for FavoritesModule {
    fn id(&self) -> &str {
        "favorites"
    }

    fn desired_size(&self, theme: &ThemeContext) -> Size {
        let n = self.loaded.len() as f32;
        if n == 0.0 {
            return Size::new(0.0, 0.0);
        }
        let icon_size = Self::effective_icon_size(&self.config, theme);
        let gap = Self::gap();
        let w = n * icon_size + (n - 1.0).max(0.0) * gap + theme.padding.left + theme.padding.right;
        let h = icon_size + theme.padding.top + theme.padding.bottom;
        Size::new(w, h)
    }

    fn update(&mut self, _ctx: &UpdateContext<'_>) -> bool {
        false // Favorites list is static at runtime.
    }

    fn render(&self, canvas: &mut Pixmap, theme: &ThemeContext, bounds: Rect) {
        let icon_size = Self::effective_icon_size(&self.config, theme);
        let gap = Self::gap();

        for (i, loaded) in self.loaded.iter().enumerate() {
            let ix = bounds.x + theme.padding.left + i as f32 * (icon_size + gap);
            let iy = bounds.y + (bounds.height - icon_size) / 2.0;
            let icon_rect = Rect::new(ix, iy, icon_size, icon_size);

            // Hover highlight.
            if self.hovered_index == Some(i) {
                render_utils::fill_rounded_rect_alpha(
                    canvas,
                    Rect::new(ix - 2.0, bounds.y, icon_size + 4.0, bounds.height),
                    theme.colors.foreground,
                    theme.border_radius,
                    0.12,
                );
            }

            if let Some(img) = &loaded.icon {
                render_utils::draw_image(canvas, img, icon_rect, 1.0);
            } else {
                // Placeholder: colored square + first letter.
                render_utils::fill_rounded_rect(
                    canvas,
                    icon_rect,
                    theme.colors.accent,
                    theme.border_radius * 0.5,
                );
                let letter = loaded
                    .entry
                    .label
                    .chars()
                    .next()
                    .map(|c| c.to_uppercase().to_string())
                    .unwrap_or_else(|| "?".to_owned());
                render_utils::draw_text_centered(
                    canvas,
                    &letter,
                    icon_rect,
                    &theme.fonts.family,
                    icon_size * 0.6,
                    theme.colors.background,
                );
            }
        }
    }

    fn handle_event(&mut self, event: &InputEvent, bounds: Rect) -> EventResult {
        // Use a default icon size when we don't have access to theme.
        let icon_size = self.config.icon_size.unwrap_or(24.0);

        match event {
            InputEvent::MouseMove { x, .. } => {
                let new_hover = self.item_at_x(*x, bounds, icon_size);
                if new_hover != self.hovered_index {
                    self.hovered_index = new_hover;
                    return EventResult::Handled;
                }
                EventResult::Handled
            }
            InputEvent::MousePress {
                x,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(idx) = self.item_at_x(*x, bounds, icon_size) {
                    return EventResult::Action(self.loaded[idx].entry.action.clone());
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
                    key: "icon_size".to_owned(),
                    label: "Icon size".to_owned(),
                    description: "Override icon size in logical pixels.".to_owned(),
                    field_type: ConfigFieldType::Float {
                        default: 24.0,
                        min: Some(16.0),
                        max: Some(64.0),
                    },
                },
                ConfigField {
                    key: "show_running_indicator".to_owned(),
                    label: "Show running indicator".to_owned(),
                    description: "Show a dot under icons with open windows.".to_owned(),
                    field_type: ConfigFieldType::Boolean { default: true },
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyprdeck_core::{ColorPalette, FontConfig, HyprState, Padding};

    fn action() -> Action {
        Action::Exec {
            command: "true".into(),
            args: vec![],
        }
    }

    fn theme() -> ThemeContext {
        use hyprdeck_core::ResolvedModuleStyles;
        ThemeContext {
            colors: ColorPalette {
                background: [30, 30, 30, 255],
                foreground: [255, 255, 255, 255],
                accent: [80, 160, 255, 255],
                urgent: [255, 80, 80, 255],
                separator: [128, 128, 128, 128],
            },
            fonts: FontConfig {
                family: "sans-serif".into(),
                size: 14.0,
                bold_family: None,
                mono_family: None,
            },
            padding: Padding {
                top: 4.0,
                right: 4.0,
                bottom: 4.0,
                left: 4.0,
            },
            border_radius: 4.0,
            opacity: 1.0,
            module_styles: ResolvedModuleStyles::default(),
        }
    }

    #[test]
    fn empty_config_returns_zero_size() {
        let m = FavoritesModule::new(FavoritesConfig::default());
        let sz = m.desired_size(&theme());
        assert_eq!(sz.width, 0.0);
        assert_eq!(sz.height, 0.0);
    }

    #[test]
    fn desired_size_scales_with_item_count() {
        let entries = vec![
            FavoriteEntry {
                label: "A".into(),
                icon: "a".into(),
                action: action(),
            },
            FavoriteEntry {
                label: "B".into(),
                icon: "b".into(),
                action: action(),
            },
        ];
        let m = FavoritesModule::new(FavoritesConfig {
            entries,
            ..FavoritesConfig::default()
        });
        let sz = m.desired_size(&theme());
        assert!(sz.width > 0.0);
    }

    #[test]
    fn item_at_x_returns_correct_index() {
        let entries = vec![
            FavoriteEntry {
                label: "A".into(),
                icon: "a".into(),
                action: action(),
            },
            FavoriteEntry {
                label: "B".into(),
                icon: "b".into(),
                action: action(),
            },
            FavoriteEntry {
                label: "C".into(),
                icon: "c".into(),
                action: action(),
            },
        ];
        let m = FavoritesModule::new(FavoritesConfig {
            entries,
            icon_size: Some(24.0),
            ..FavoritesConfig::default()
        });
        let bounds = Rect::new(0.0, 0.0, 200.0, 32.0);
        assert_eq!(m.item_at_x(12.0, bounds, 24.0), Some(0));
        assert_eq!(m.item_at_x(24.0 + 4.0 + 12.0, bounds, 24.0), Some(1));
    }

    #[test]
    fn click_returns_correct_action() {
        let entries = vec![
            FavoriteEntry {
                label: "A".into(),
                icon: "a".into(),
                action: Action::Exec {
                    command: "cmdA".into(),
                    args: vec![],
                },
            },
            FavoriteEntry {
                label: "B".into(),
                icon: "b".into(),
                action: Action::Exec {
                    command: "cmdB".into(),
                    args: vec![],
                },
            },
        ];
        let mut m = FavoritesModule::new(FavoritesConfig {
            entries,
            icon_size: Some(24.0),
            ..FavoritesConfig::default()
        });
        let bounds = Rect::new(0.0, 0.0, 200.0, 32.0);
        let result = m.handle_event(
            &InputEvent::MousePress {
                x: 12.0,
                y: 10.0,
                button: MouseButton::Left,
            },
            bounds,
        );
        assert!(
            matches!(result, EventResult::Action(Action::Exec { command, .. }) if command == "cmdA"),
            "expected cmdA action"
        );
    }

    #[test]
    fn missing_icon_does_not_panic() {
        let entries = vec![FavoriteEntry {
            label: "X".into(),
            icon: "__no_icon__".into(),
            action: action(),
        }];
        let m = FavoritesModule::new(FavoritesConfig {
            entries,
            ..FavoritesConfig::default()
        });
        assert!(m.loaded[0].icon.is_none());
    }

    #[test]
    fn update_always_returns_false() {
        let mut m = FavoritesModule::new(FavoritesConfig::default());
        let state = HyprState::default();
        let ctx = UpdateContext {
            now: chrono::Local::now(),
            hypr_state: &state,
            output_name: "",
        };
        assert!(!m.update(&ctx));
    }
}
