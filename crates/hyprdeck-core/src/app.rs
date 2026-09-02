use std::collections::HashMap;
use std::path::Path;

use crate::action::{self, Action, ActionError};
use crate::config::Config;
use crate::geometry::{DisplayGeometry, Edge};
use crate::ipc::event::{HyprEvent, HyprState};
use crate::layout::{LayoutEngine, ModuleGroups};
use crate::module::{PanelModule, UpdateContext};
use crate::output::OutputState;
use crate::panel::{
    ColorPalette, Panel, ResolvedModuleStyles, ResolvedStyle, ResolvedWindowListStyle,
    ResolvedWorkspacesStyle,
};
use crate::theme::{ModuleStyleMap, PanelDefinition, ThemeDefinition};

/// Function type for creating a module by its string ID and optional config.
///
/// The binary crate provides this by wrapping `hyprdeck_modules::create_module`,
/// keeping `hyprdeck-core` free of a dependency on `hyprdeck-modules`.
pub type ModuleFactory = fn(&str, toml::Value) -> Option<Box<dyn PanelModule>>;

/// Top-level application state.
///
/// Coordinates outputs, panels, IPC, and the theme/config pipeline.
#[derive(Debug)]
pub struct App {
    /// Resolved configuration.
    pub config: Config,
    /// Theme definition (immutable after load).
    pub theme: ThemeDefinition,
    /// Per-output state, keyed by Wayland output name.
    pub outputs: HashMap<String, OutputState>,
    /// Module factory function (injected from the binary crate).
    module_factory: ModuleFactory,
    /// Whether a shutdown has been requested.
    pub shutdown: bool,
}

impl App {
    /// Create a new App from an already-loaded config, theme, and module factory.
    pub fn new(config: Config, theme: ThemeDefinition, module_factory: ModuleFactory) -> Self {
        Self {
            config,
            theme,
            outputs: HashMap::new(),
            module_factory,
            shutdown: false,
        }
    }

    /// Replace the runtime configuration and theme, dropping every current
    /// output so the binary can recreate its Wayland surfaces in place.
    pub fn reload(&mut self, config: Config, theme: ThemeDefinition) {
        self.outputs.clear();
        self.config = config;
        self.theme = theme;
    }

    /// Called when a new Wayland output (monitor) is discovered.
    ///
    /// Creates panels for this output based on the theme definition, instantiates
    /// modules via the factory, and marks them all dirty for first render.
    pub fn add_output(&mut self, name: String, width: u32, height: u32) {
        tracing::info!("Adding output '{}' ({}x{})", name, width, height);

        let mut output = OutputState::new(name.clone(), width, height);

        for panel_def in &self.theme.panels {
            let panel = self.create_panel(panel_def, &output.display_geometry);
            output.panels.push(panel);
        }

        self.outputs.insert(name, output);
    }

    /// Called when a Wayland output is removed (monitor unplugged).
    pub fn remove_output(&mut self, name: &str) {
        tracing::info!("Removing output '{}'", name);
        self.outputs.remove(name);
    }

    /// Route a Hyprland IPC event to all panels across all outputs.
    /// Returns true if any panel flagged a redraw.
    pub fn handle_hypr_event(&mut self, event: &HyprEvent) -> bool {
        let mut any_dirty = false;
        for output in self.outputs.values_mut() {
            for panel in &mut output.panels {
                if panel.handle_hypr_event(event) {
                    any_dirty = true;
                }
            }
        }
        any_dirty
    }

    /// Run periodic module updates (clock tick, weather poll, etc).
    ///
    /// Creates a per-output [`UpdateContext`] for each output so that
    /// per-monitor modules (workspaces, window list) can identify which
    /// monitor they belong to and read the correct per-monitor state.
    ///
    /// Returns true if any panel needs redraw.
    pub fn tick_modules(
        &mut self,
        now: chrono::DateTime<chrono::Local>,
        hypr_state: &HyprState,
    ) -> bool {
        let mut any_dirty = false;
        for (output_name, output) in self.outputs.iter_mut() {
            let ctx = UpdateContext {
                now,
                hypr_state,
                output_name: output_name.as_str(),
            };
            for panel in &mut output.panels {
                if panel.update_modules(&ctx) {
                    any_dirty = true;
                }
            }
        }
        any_dirty
    }

    /// Tick auto-hide and dock animations on all panels.
    /// Returns true if any panel is still animating.
    pub fn tick_animations(&mut self, dt: f32) -> bool {
        let mut any_animating = false;
        for output in self.outputs.values_mut() {
            for panel in &mut output.panels {
                if panel.tick_auto_hide(dt) {
                    any_animating = true;
                }
                if panel.layout.tick_animation(dt) {
                    panel.dirty = true;
                    any_animating = true;
                }
            }
        }
        any_animating
    }

    /// Returns true if any panel across all outputs is currently animating.
    pub fn is_animating(&self) -> bool {
        self.outputs.values().any(|o| {
            o.panels
                .iter()
                .any(|p| p.is_auto_hide_animating() || p.layout.is_animating())
        })
    }

    /// Render all dirty panels across all outputs.
    pub fn render_dirty(&mut self) {
        for output in self.outputs.values_mut() {
            output.render_dirty_panels();
        }
    }

    /// Dispatch an action returned by a module.
    pub async fn dispatch_action(
        &self,
        act: &Action,
        hypr_socket: &Path,
    ) -> Result<(), ActionError> {
        action::dispatch_action(act, hypr_socket).await
    }

    /// Create a single panel from a theme panel definition.
    fn create_panel(&self, panel_def: &PanelDefinition, display: &DisplayGeometry) -> Panel {
        let mut style = resolve_style_from_theme(&self.theme, &self.config);

        // Override bar_height from the panel's explicit size so the layout engine
        // and background drawing use the actual surface dimensions (e.g. 40 for win7,
        // not the hardcoded default of 32).
        if let Some(h) = panel_def.height {
            style.bar_height = h;
        }
        if let Some(w) = panel_def.width {
            style.bar_height = w; // for vertical panels, bar_height doubles as bar_width
        }

        // Resolve per-panel module styles, overriding palette defaults with any
        // colors explicitly defined in the theme's [panels.module_styles.*] section.
        style.module_styles =
            resolve_module_styles_for_panel(&panel_def.module_styles, &style.colors);

        let (surface_width, surface_height) = match panel_def.edge {
            Edge::Top | Edge::Bottom => {
                let w = display.bounds.width as u32;
                let h = panel_def.height.unwrap_or(style.bar_height);
                (w, h)
            }
            Edge::Left | Edge::Right => {
                let w = panel_def.width.unwrap_or(48);
                let h = display.bounds.height as u32;
                (w, h)
            }
        };

        let layout = LayoutEngine::from_panel_def(&panel_def.layout, panel_def.dock.as_ref());

        let groups = ModuleGroups {
            start: panel_def.modules_start.clone(),
            center: panel_def.modules_center.clone(),
            end: panel_def.modules_end.clone(),
        };

        let mut panel = Panel::new(
            panel_def.edge,
            style,
            panel_def.auto_hide.clone(),
            layout,
            groups,
            surface_width,
            surface_height,
        );

        // Instantiate modules via the factory
        let all_ids: Vec<&String> = panel_def
            .modules_start
            .iter()
            .chain(panel_def.modules_center.iter())
            .chain(panel_def.modules_end.iter())
            .collect();

        let factory = self.module_factory;
        let modules: Vec<Box<dyn PanelModule>> = all_ids
            .iter()
            .filter_map(|id| {
                let config = self
                    .config
                    .module_config(id)
                    .cloned()
                    .unwrap_or(toml::Value::Table(toml::map::Map::new()));
                factory(id, config)
            })
            .collect();

        panel.set_modules(modules);
        panel
    }
}

/// Resolve per-panel module styles from raw TOML definitions, falling back to
/// palette-derived defaults for any missing color values.
pub fn resolve_module_styles_for_panel(
    module_styles: &ModuleStyleMap,
    base_colors: &ColorPalette,
) -> ResolvedModuleStyles {
    let parse = |hex: Option<&str>, fallback: [u8; 4]| -> [u8; 4] {
        hex.and_then(ColorPalette::parse_hex).unwrap_or(fallback)
    };

    let wl = module_styles.window_list.as_ref();
    let ws = module_styles.workspaces.as_ref();

    // Derive sensible defaults from the panel's base colors.
    let mut default_wl_active_bg = base_colors.accent;
    default_wl_active_bg[3] = 200;
    let mut default_wl_inactive_bg = base_colors.foreground;
    default_wl_inactive_bg[3] = 10;
    let mut default_ws_inactive_bg = base_colors.foreground;
    default_ws_inactive_bg[3] = 80;
    let muted = |color: [u8; 4], neutral: u8, alpha: u8| -> [u8; 4] {
        let mix = |channel: u8| ((u16::from(channel) + u16::from(neutral)) / 2) as u8;
        [mix(color[0]), mix(color[1]), mix(color[2]), alpha]
    };
    let default_ws_remote_bg = muted(base_colors.foreground, 96, 80);
    let default_ws_remote_fg = muted(base_colors.foreground, 160, 190);
    let default_ws_remote_urgent_bg = muted(base_colors.urgent, 112, 190);

    ResolvedModuleStyles {
        window_list: ResolvedWindowListStyle {
            active_background: parse(
                wl.and_then(|s| s.active_background.as_deref()),
                default_wl_active_bg,
            ),
            active_foreground: parse(
                wl.and_then(|s| s.active_foreground.as_deref()),
                base_colors.background,
            ),
            inactive_background: parse(
                wl.and_then(|s| s.inactive_background.as_deref()),
                default_wl_inactive_bg,
            ),
            inactive_foreground: parse(
                wl.and_then(|s| s.inactive_foreground.as_deref()),
                base_colors.foreground,
            ),
        },
        workspaces: ResolvedWorkspacesStyle {
            active_background: parse(
                ws.and_then(|s| s.active_background.as_deref()),
                base_colors.accent,
            ),
            active_foreground: parse(
                ws.and_then(|s| s.active_foreground.as_deref()),
                base_colors.background,
            ),
            inactive_background: parse(
                ws.and_then(|s| s.inactive_background.as_deref()),
                default_ws_inactive_bg,
            ),
            inactive_foreground: parse(
                ws.and_then(|s| s.inactive_foreground.as_deref()),
                base_colors.foreground,
            ),
            remote_background: parse(
                ws.and_then(|s| s.remote_background.as_deref()),
                default_ws_remote_bg,
            ),
            remote_foreground: parse(
                ws.and_then(|s| s.remote_foreground.as_deref()),
                default_ws_remote_fg,
            ),
            remote_urgent_background: parse(
                ws.and_then(|s| s.remote_urgent_background.as_deref()),
                default_ws_remote_urgent_bg,
            ),
            remote_urgent_foreground: parse(
                ws.and_then(|s| s.remote_urgent_foreground.as_deref()),
                default_ws_remote_fg,
            ),
        },
    }
}

/// Resolve a [`ResolvedStyle`] from a theme definition and user config overrides.
pub fn resolve_style_from_theme(theme: &ThemeDefinition, config: &Config) -> ResolvedStyle {
    use crate::panel::{FontConfig, Padding, ResolvedSeparator};

    let style_def = theme.style.as_ref();

    let parse_color = |hex: &Option<String>, fallback: [u8; 4]| -> [u8; 4] {
        hex.as_deref()
            .and_then(ColorPalette::parse_hex)
            .unwrap_or(fallback)
    };

    let background = parse_color(
        &style_def.and_then(|s| s.background_color.clone()),
        [30, 30, 30, 230],
    );
    let foreground = parse_color(
        &style_def.and_then(|s| s.foreground_color.clone()),
        [255, 255, 255, 255],
    );
    let accent = parse_color(
        &config
            .theme_overrides
            .accent_color
            .clone()
            .or_else(|| style_def.and_then(|s| s.accent_color.clone())),
        [80, 160, 255, 255],
    );
    let urgent = parse_color(
        &style_def.and_then(|s| s.urgent_color.clone()),
        [255, 80, 80, 255],
    );
    let separator_color = parse_color(
        &style_def.and_then(|s| s.separator_color.clone()),
        [128, 128, 128, 128],
    );

    let font_family = config
        .theme_overrides
        .font_family
        .clone()
        .or_else(|| style_def.and_then(|s| s.font_family.clone()))
        .unwrap_or_else(|| "sans-serif".to_owned());

    let mono_font_family = style_def.and_then(|s| s.mono_font_family.clone());

    let font_size = config
        .theme_overrides
        .font_size
        .or_else(|| style_def.and_then(|s| s.font_size))
        .unwrap_or(13.0);

    let border_radius = style_def.and_then(|s| s.border_radius).unwrap_or(0.0);

    let module_gap = style_def.and_then(|s| s.module_gap).unwrap_or(0.0);

    let verbose_text_padding = style_def.and_then(|s| s.verbose_text_padding);

    let icon_padding = style_def
        .and_then(|s| s.icon_padding)
        .unwrap_or(2.0)
        .max(0.0);

    let opacity = config
        .theme_overrides
        .bar_opacity
        .or_else(|| style_def.and_then(|s| s.opacity))
        .unwrap_or(0.9);

    let colors = ColorPalette {
        background,
        foreground,
        accent,
        urgent,
        separator: separator_color,
    };

    // Build default module styles from the resolved palette; will be
    // overridden per-panel in create_panel() once panel_def is known.
    let module_styles = ResolvedModuleStyles::from_palette(&colors);

    ResolvedStyle {
        colors,
        fonts: FontConfig {
            family: font_family,
            size: font_size,
            bold_family: None,
            mono_family: mono_font_family,
        },
        bar_height: 32,
        padding: Padding {
            top: 4.0,
            right: 8.0,
            bottom: 4.0,
            left: 8.0,
        },
        border_radius,
        background_opacity: opacity,
        icon_padding,
        separator: ResolvedSeparator {
            color: separator_color,
            ..ResolvedSeparator::default()
        },
        module_gap,
        verbose_text_padding,
        module_styles,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autohide::AutoHideMode;
    use crate::config::{ModuleConfigs, NotificationConfig, ThemeOverrides};
    use crate::theme::StyleDefinition;

    /// Stub module factory that creates nothing (for testing App without modules).
    fn null_factory(_id: &str, _config: toml::Value) -> Option<Box<dyn PanelModule>> {
        None
    }

    fn test_config() -> Config {
        Config {
            theme: "test".into(),
            theme_overrides: ThemeOverrides::default(),
            modules: ModuleConfigs::default(),
            notifications: NotificationConfig::default(),
            extra: Default::default(),
        }
    }

    fn test_theme() -> ThemeDefinition {
        ThemeDefinition {
            name: "test".into(),
            description: "Test theme".into(),
            panels: vec![PanelDefinition {
                edge: Edge::Top,
                height: Some(32),
                width: None,
                auto_hide: AutoHideMode::Disabled,
                layout: crate::theme::LayoutType::Horizontal,
                modules_start: vec!["clock".into()],
                modules_center: vec![],
                modules_end: vec![],
                dock: None,
                module_styles: crate::theme::ModuleStyleMap::default(),
            }],
            style: None,
        }
    }

    #[test]
    fn app_new_has_no_outputs() {
        let app = App::new(test_config(), test_theme(), null_factory);
        assert!(app.outputs.is_empty());
        assert!(!app.shutdown);
    }

    #[test]
    fn add_output_creates_panels() {
        let mut app = App::new(test_config(), test_theme(), null_factory);
        app.add_output("DP-1".into(), 1920, 1080);
        assert!(app.outputs.contains_key("DP-1"));
        let output = &app.outputs["DP-1"];
        assert_eq!(output.panels.len(), 1);
        assert_eq!(output.panels[0].edge, Edge::Top);
    }

    #[test]
    fn remove_output_cleans_up() {
        let mut app = App::new(test_config(), test_theme(), null_factory);
        app.add_output("DP-1".into(), 1920, 1080);
        assert!(app.outputs.contains_key("DP-1"));

        app.remove_output("DP-1");
        assert!(!app.outputs.contains_key("DP-1"));
    }

    #[test]
    fn reload_replaces_configuration_and_drops_existing_outputs() {
        let mut app = App::new(test_config(), test_theme(), null_factory);
        app.add_output("DP-1".into(), 1920, 1080);
        let mut config = test_config();
        config.theme = "replacement".into();
        let theme = ThemeDefinition {
            name: "Replacement".into(),
            description: String::new(),
            panels: vec![],
            style: None,
        };

        app.reload(config, theme);

        assert!(app.outputs.is_empty());
        assert_eq!(app.config.theme, "replacement");
        assert_eq!(app.theme.name, "Replacement");
    }

    #[test]
    fn handle_hypr_event_propagates_to_panels() {
        let mut app = App::new(test_config(), test_theme(), null_factory);
        app.add_output("DP-1".into(), 1920, 1080);

        // Clear dirty flags
        for output in app.outputs.values_mut() {
            for panel in &mut output.panels {
                panel.dirty = false;
            }
        }

        let event = crate::ipc::event::HyprEvent::WorkspaceChanged {
            workspace: crate::ipc::event::WorkspaceRef::Id(2),
        };
        let dirty = app.handle_hypr_event(&event);
        assert!(dirty);
    }

    #[test]
    fn is_animating_false_for_disabled_auto_hide() {
        let mut app = App::new(test_config(), test_theme(), null_factory);
        app.add_output("DP-1".into(), 1920, 1080);
        assert!(!app.is_animating());
    }

    #[test]
    fn resolve_style_uses_defaults_when_no_style_defined() {
        let theme = ThemeDefinition {
            name: "minimal".into(),
            description: "".into(),
            panels: vec![],
            style: None,
        };
        let config = test_config();
        let style = resolve_style_from_theme(&theme, &config);

        assert_eq!(style.colors.background, [30, 30, 30, 230]);
        assert_eq!(style.fonts.family, "sans-serif");
        assert_eq!(style.fonts.size, 13.0);
        assert_eq!(style.icon_padding, 2.0);
    }

    #[test]
    fn resolve_style_applies_theme_overrides() {
        let theme = ThemeDefinition {
            name: "styled".into(),
            description: "".into(),
            panels: vec![],
            style: Some(StyleDefinition {
                background_color: Some("#ff0000".into()),
                font_family: Some("monospace".into()),
                font_size: Some(16.0),
                ..StyleDefinition::default()
            }),
        };
        let config = test_config();
        let style = resolve_style_from_theme(&theme, &config);

        assert_eq!(style.colors.background, [255, 0, 0, 255]);
        assert_eq!(style.fonts.family, "monospace");
        assert_eq!(style.fonts.size, 16.0);
    }

    #[test]
    fn resolve_style_applies_icon_padding() {
        let theme = ThemeDefinition {
            name: "icon-padding".into(),
            description: "".into(),
            panels: vec![],
            style: Some(StyleDefinition {
                icon_padding: Some(5.0),
                ..StyleDefinition::default()
            }),
        };

        assert_eq!(
            resolve_style_from_theme(&theme, &test_config()).icon_padding,
            5.0
        );
    }

    #[test]
    fn resolve_style_user_overrides_take_precedence() {
        let theme = ThemeDefinition {
            name: "styled".into(),
            description: "".into(),
            panels: vec![],
            style: Some(StyleDefinition {
                font_family: Some("monospace".into()),
                accent_color: Some("#00ff00".into()),
                ..StyleDefinition::default()
            }),
        };
        let config = Config {
            theme: "styled".into(),
            theme_overrides: ThemeOverrides {
                font_family: Some("serif".into()),
                accent_color: Some("#0000ff".into()),
                ..ThemeOverrides::default()
            },
            modules: ModuleConfigs::default(),
            notifications: NotificationConfig::default(),
            extra: Default::default(),
        };
        let style = resolve_style_from_theme(&theme, &config);

        // User override should win
        assert_eq!(style.fonts.family, "serif");
        assert_eq!(style.colors.accent, [0, 0, 255, 255]);
    }
}
