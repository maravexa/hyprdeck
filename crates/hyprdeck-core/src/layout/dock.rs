use crate::geometry::{DisplayGeometry, Point, Rect, Size};
use crate::panel::ResolvedStyle;
use crate::theme::DockConfig;

use super::{LayoutResult, ModuleGroups, ModuleSizeProvider};

/// macOS-style dock with icon magnification on hover.
///
/// Unlike bar layouts, the dock:
/// - Floats centered on its edge (not full-width)
/// - Has dynamically sized icons based on cursor proximity
/// - Animates scale changes smoothly
/// - Expands upward (or toward screen center) when icons magnify
#[derive(Debug, Clone)]
pub struct DockLayout {
    config: DockLayoutConfig,
    /// Mutable animation state, public for read access from `LayoutEngine`.
    pub state: DockAnimState,
}

/// Tuning parameters for dock layout and magnification behaviour.
#[derive(Debug, Clone)]
pub struct DockLayoutConfig {
    pub icon_base_size: f32,
    pub icon_max_scale: f32,
    pub magnification_radius: f32,
    pub animation_speed: f32,
    pub padding: f32,
    pub background_radius: f32,
    pub background_opacity: f32,
    /// Gap between icons, default 4.0.
    pub icon_gap: f32,
}

impl Default for DockLayoutConfig {
    fn default() -> Self {
        Self {
            icon_base_size: 48.0,
            icon_max_scale: 1.8,
            magnification_radius: 150.0,
            animation_speed: 12.0,
            padding: 8.0,
            background_radius: 12.0,
            background_opacity: 0.6,
            icon_gap: 4.0,
        }
    }
}

impl DockLayoutConfig {
    pub fn from_dock_config(config: &DockConfig) -> Self {
        Self {
            icon_base_size: config.icon_base_size,
            icon_max_scale: config.icon_max_scale,
            magnification_radius: config.magnification_radius,
            animation_speed: config.animation_speed,
            padding: config.padding,
            background_radius: config.background_radius,
            background_opacity: config.background_opacity,
            icon_gap: 4.0,
        }
    }
}

/// Mutable animation state for dock magnification.
#[derive(Debug, Clone, Default)]
pub struct DockAnimState {
    /// Current cursor position relative to dock, or None if cursor is outside.
    pub cursor: Option<Point>,
    /// Per-icon current scale (interpolating toward target).
    pub current_scales: Vec<f32>,
    /// Per-icon target scale (computed from cursor distance).
    pub target_scales: Vec<f32>,
    /// Whether we're currently animating (scales haven't settled).
    pub animating: bool,
}

impl DockLayout {
    pub fn new(config: DockLayoutConfig) -> Self {
        Self {
            config,
            state: DockAnimState::default(),
        }
    }

    /// Run a layout pass, computing bounds for all modules.
    ///
    /// The dock flattens all groups into a single ordered list (start ++ center ++ end).
    /// Icons are bottom-aligned within the dock surface.
    pub fn layout(
        &self,
        groups: &ModuleGroups,
        sizes: &dyn ModuleSizeProvider,
        style: &ResolvedStyle,
        display: &DisplayGeometry,
    ) -> LayoutResult {
        let _ = sizes; // Dock ignores per-module desired sizes; all icons are icon_base_size.
        let _ = style; // Dock uses its own config for sizing.

        // Flatten all groups into a single icon list.
        let all_ids: Vec<&String> = groups
            .start
            .iter()
            .chain(groups.center.iter())
            .chain(groups.end.iter())
            .collect();

        let icon_count = all_ids.len();
        if icon_count == 0 {
            return LayoutResult {
                module_bounds: Vec::new(),
                total_size: Size::new(0.0, 0.0),
                background_bounds: Rect::default(),
            };
        }

        let base = self.config.icon_base_size;
        let gap = self.config.icon_gap;
        let pad = self.config.padding;

        // Use current_scales if available and correctly sized, otherwise 1.0.
        let scales: Vec<f32> = if self.state.current_scales.len() == icon_count {
            self.state.current_scales.clone()
        } else {
            vec![1.0; icon_count]
        };

        // Compute total dock width and max icon height.
        let mut total_icons_width = 0.0f32;
        let mut max_icon_height = 0.0f32;
        for (i, scale) in scales.iter().enumerate() {
            let scaled = base * scale;
            if i > 0 {
                total_icons_width += gap;
            }
            total_icons_width += scaled;
            max_icon_height = max_icon_height.max(scaled);
        }

        let total_dock_width = total_icons_width + 2.0 * pad;
        let dock_height = max_icon_height + 2.0 * pad;
        let screen_width = display.bounds.width;

        // Center the dock horizontally on screen.
        let dock_x = (screen_width - total_dock_width) / 2.0;

        // The surface height accommodates the maximum magnified size.
        let max_possible_height = base * self.config.icon_max_scale + 2.0 * pad;
        let surface_height = max_possible_height;
        let surface_width = screen_width;

        // Background bounds: the rounded pill behind the icons.
        let background_bounds = Rect::new(
            dock_x,
            surface_height - dock_height,
            total_dock_width,
            dock_height,
        );

        // Place each icon, bottom-aligned within the dock surface.
        let mut module_bounds = Vec::with_capacity(icon_count);
        let mut cursor_x = dock_x + pad;
        for (i, id) in all_ids.iter().enumerate() {
            if i > 0 {
                cursor_x += gap;
            }
            let scaled = base * scales[i];
            // Bottom-aligned: y pushes upward as scale increases.
            let icon_y = surface_height - pad - scaled;
            module_bounds.push(((*id).clone(), Rect::new(cursor_x, icon_y, scaled, scaled)));
            cursor_x += scaled;
        }

        LayoutResult {
            module_bounds,
            total_size: Size::new(surface_width, surface_height),
            background_bounds,
        }
    }

    /// Update cursor position. Returns true if target scales changed.
    pub fn update_cursor(&mut self, cursor: Option<Point>) -> bool {
        let all_count = self.state.current_scales.len();

        // Ensure scale vectors are initialised.
        if self.state.target_scales.len() != all_count {
            self.state.target_scales.resize(all_count, 1.0);
        }

        let old_cursor = self.state.cursor;
        self.state.cursor = cursor;

        match cursor {
            Some(pos) => {
                // Iterative position solving: because magnifying an icon shifts its
                // neighbours, which changes their distance to the cursor, which changes
                // their scale. Three iterations converge well since magnification
                // influence is local.
                let base = self.config.icon_base_size;
                let gap = self.config.icon_gap;
                let pad = self.config.padding;

                let mut scales = self.state.current_scales.clone();
                for _iteration in 0..3 {
                    let mut cursor_x = 0.0f32; // relative to dock left edge (after padding)
                    for i in 0..all_count {
                        if i > 0 {
                            cursor_x += gap;
                        }
                        let scaled = base * scales[i];
                        let icon_center = cursor_x + scaled / 2.0;
                        // pos.x is relative to the dock; we approximate by using the
                        // icon center in dock-local coordinates.
                        let cursor_local = pos.x - pad;
                        scales[i] = self.compute_icon_scale(icon_center, cursor_local);
                        cursor_x += scaled;
                    }
                }
                let changed = scales != self.state.target_scales;
                self.state.target_scales = scales;
                if changed {
                    self.state.animating = true;
                }
                changed
            }
            None => {
                // Cursor left — all targets back to 1.0.
                let was_magnified = self.state.target_scales.iter().any(|&s| s != 1.0);
                for t in self.state.target_scales.iter_mut() {
                    *t = 1.0;
                }
                if was_magnified {
                    self.state.animating = true;
                }
                was_magnified || old_cursor.is_some()
            }
        }
    }

    /// Advance animation by dt seconds. Returns true if still animating.
    pub fn tick_animation(&mut self, dt: f32) -> bool {
        let speed = self.config.animation_speed;
        let mut still_moving = false;

        for i in 0..self.state.current_scales.len() {
            let target = self.state.target_scales[i];
            let current = self.state.current_scales[i];
            let diff = target - current;

            if diff.abs() < 0.001 {
                self.state.current_scales[i] = target;
            } else {
                // Exponential ease toward target.
                self.state.current_scales[i] += diff * (speed * dt).min(1.0);
                still_moving = true;
            }
        }

        self.state.animating = still_moving;
        still_moving
    }

    /// Ensure the animation state vectors match the given icon count.
    pub fn ensure_icon_count(&mut self, count: usize) {
        if self.state.current_scales.len() != count {
            self.state.current_scales = vec![1.0; count];
            self.state.target_scales = vec![1.0; count];
            self.state.animating = false;
        }
    }

    /// Compute the target scale for an icon at `icon_center_main` given `cursor_main`.
    fn compute_icon_scale(&self, icon_center_main: f32, cursor_main: f32) -> f32 {
        let dist = (icon_center_main - cursor_main).abs();
        if dist > self.config.magnification_radius {
            return 1.0;
        }
        let t = 1.0 - (dist / self.config.magnification_radius);
        // Cosine interpolation for smooth falloff.
        let factor = (1.0 - (std::f32::consts::PI * t).cos()) / 2.0;
        1.0 + (self.config.icon_max_scale - 1.0) * factor
    }
}
