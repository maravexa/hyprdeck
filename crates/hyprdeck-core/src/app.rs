use std::collections::HashMap;
use std::path::Path;

use crate::action::{self, Action, ActionError};
use crate::config::Config;
use crate::geometry::{DisplayGeometry, Edge};
use crate::ipc::event::HyprEvent;
use crate::layout::{LayoutEngine, ModuleGroups};
use crate::module::{PanelModule, UpdateContext};
use crate::output::OutputState;
use crate::panel::{Panel, ResolvedStyle};
use crate::theme::{PanelDefinition, ThemeDefinition};

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
    /// Returns true if any panel needs redraw.
    pub fn tick_modules(&mut self, ctx: &UpdateContext<'_>) -> bool {
        let mut any_dirty = false;
        for output in self.outputs.values_mut() {
            for panel in &mut output.panels {
                if panel.update_modules(ctx) {
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
        let style = resolve_style_from_theme(&self.theme, &self.config);

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

/// Resolve a [`ResolvedStyle`] from a theme definition and user config overrides.
pub fn resolve_style_from_theme(theme: &ThemeDefinition, config: &Config) -> ResolvedStyle {
    use crate::panel::{ColorPalette, FontConfig, Padding, ResolvedSeparator};

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

    let font_size = config
        .theme_overrides
        .font_size
        .or_else(|| style_def.and_then(|s| s.font_size))
        .unwrap_or(13.0);

    let border_radius = style_def.and_then(|s| s.border_radius).unwrap_or(0.0);

    let opacity = config
        .theme_overrides
        .bar_opacity
        .or_else(|| style_def.and_then(|s| s.opacity))
        .unwrap_or(0.9);

    ResolvedStyle {
        colors: ColorPalette {
            background,
            foreground,
            accent,
            urgent,
            separator: separator_color,
        },
        fonts: FontConfig {
            family: font_family,
            size: font_size,
            bold_family: None,
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
        separator: ResolvedSeparator {
            color: separator_color,
            ..ResolvedSeparator::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autohide::AutoHideMode;
    use crate::config::{ModuleConfigs, ThemeOverrides};
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
    fn handle_hypr_event_propagates_to_panels() {
        let mut app = App::new(test_config(), test_theme(), null_factory);
        app.add_output("DP-1".into(), 1920, 1080);

        // Clear dirty flags
        for output in app.outputs.values_mut() {
            for panel in &mut output.panels {
                panel.dirty = false;
            }
        }

        let event = crate::ipc::event::HyprEvent::WorkspaceChanged { id: 2 };
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
        };
        let style = resolve_style_from_theme(&theme, &config);

        // User override should win
        assert_eq!(style.fonts.family, "serif");
        assert_eq!(style.colors.accent, [0, 0, 255, 255]);
    }
}
