use crate::geometry::Size;

use super::{LayoutResult, ModuleGroups};

/// Three-group vertical layout: `start | center | end` (top → bottom).
///
/// Mirrors [`HorizontalLayout`](super::HorizontalLayout) but along the Y axis;
/// used for left/right edge panels.
#[derive(Debug)]
pub struct VerticalLayout {
    /// Module groupings declared by the theme.
    pub groups: ModuleGroups,
    /// Gap between adjacent module bounding boxes, in logical pixels.
    pub spacing: f32,
}

impl VerticalLayout {
    pub fn new(groups: ModuleGroups, spacing: f32) -> Self {
        VerticalLayout { groups, spacing }
    }

    /// Compute per-module [`Rect`](crate::geometry::Rect) bounds.
    pub fn layout(&self, available: Size, module_sizes: &[(String, Size)]) -> LayoutResult {
        todo!()
    }
}
