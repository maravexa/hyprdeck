use crate::geometry::Size;

/// Dock-style layout with macOS-inspired cursor-proximity icon magnification.
///
/// Icons are arranged in a single row (for bottom/top edge) or column (for
/// left/right edge).  Scale factors are computed per-icon based on cursor
/// distance, then animated toward their target values each tick.
#[derive(Debug, Clone)]
pub struct DockLayout {
    /// Baseline icon size in logical pixels (no magnification).
    pub icon_base_size: f32,
    /// Maximum scale multiplier at peak magnification (e.g. 1.8 → 72 px for 40 px base).
    pub icon_max_scale: f32,
    /// Cursor distance (logical px) within which magnification is applied.
    pub magnification_radius: f32,
    /// Animation lerp speed (higher = snappier).
    pub animation_speed: f32,
    /// Padding around the dock background pill, in logical pixels.
    pub padding: f32,
    /// Corner radius of the dock background pill.
    pub background_radius: f32,
}

/// Per-frame magnification state — held by the panel, passed to [`DockLayout::update`].
#[derive(Debug, Default)]
pub struct DockMagnification {
    /// Most recent cursor X (or Y for vertical panels) relative to dock origin.
    pub cursor_pos: f32,
    /// Current interpolated scale per icon slot.
    pub icon_scales: Vec<f32>,
    /// Target scale per icon slot derived from cursor proximity.
    pub target_scales: Vec<f32>,
}

impl DockLayout {
    pub fn new(
        icon_base_size: f32,
        icon_max_scale: f32,
        magnification_radius: f32,
        animation_speed: f32,
        padding: f32,
        background_radius: f32,
    ) -> Self {
        DockLayout {
            icon_base_size,
            icon_max_scale,
            magnification_radius,
            animation_speed,
            padding,
            background_radius,
        }
    }

    /// Recalculate target scales based on cursor position, then lerp current
    /// scales toward targets.  `dt` is elapsed time in seconds.
    pub fn update(&self, state: &mut DockMagnification, cursor_pos: f32, dt: f32) {
        todo!()
    }

    /// Return the total size of the dock pill given `icon_count` and the
    /// current `scales` array.
    pub fn measure(&self, icon_count: usize, scales: &[f32]) -> Size {
        todo!()
    }

    /// Compute per-icon [`Rect`](crate::geometry::Rect)s for the current `scales`.
    pub fn layout(
        &self,
        icon_count: usize,
        scales: &[f32],
        available: Size,
    ) -> Vec<crate::geometry::Rect> {
        todo!()
    }
}
