use crate::geometry::Size;

use super::{LayoutResult, ModuleGroups};

/// Three-group horizontal layout: `start | center | end`.
///
/// Start and end groups are left/right-aligned; the center group is always
/// mid-panel regardless of how many items are in the other groups.
#[derive(Debug)]
pub struct HorizontalLayout {
    /// Module groupings declared by the theme.
    pub groups: ModuleGroups,
    /// Gap between adjacent module bounding boxes, in logical pixels.
    pub spacing: f32,
}

impl HorizontalLayout {
    pub fn new(groups: ModuleGroups, spacing: f32) -> Self {
        HorizontalLayout { groups, spacing }
    }

    /// Compute per-module [`Rect`](crate::geometry::Rect) bounds.
    ///
    /// `module_sizes` must enumerate every module referenced by `groups`; order
    /// within the slice does not matter — lookup is by module ID.
    pub fn layout(&self, available: Size, module_sizes: &[(String, Size)]) -> LayoutResult {
        todo!()
    }
}
