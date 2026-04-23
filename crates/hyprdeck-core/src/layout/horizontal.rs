use crate::geometry::{DisplayGeometry, Rect, Size};
use crate::panel::ResolvedStyle;

use super::{Axis, LayoutResult, ModuleGroups, ModuleSizeProvider, linear_layout};

/// Three-group horizontal layout: `start | center | end`.
///
/// Start and end groups are left/right-aligned; the center group is always
/// mid-panel regardless of how many items are in the other groups.
///
/// Layout algorithm:
/// 1. The panel occupies full screen width, fixed height from style.
/// 2. Inset by panel padding.
/// 3. Measure all modules via ModuleSizeProvider.
/// 4. Place start group flush-left, flowing right.
/// 5. Place end group flush-right, flowing left.
/// 6. Place center group centered in remaining space between start and end groups.
/// 7. If center group would overlap start or end, shift it to avoid collision.
/// 8. Insert separator gaps between adjacent modules within a group if separators are enabled.
#[derive(Debug, Clone)]
pub struct HorizontalLayout {
    // No mutable state needed — horizontal layout is purely functional.
}

impl Default for HorizontalLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl HorizontalLayout {
    pub fn new() -> Self {
        HorizontalLayout {}
    }

    pub fn layout(
        &self,
        groups: &ModuleGroups,
        sizes: &dyn ModuleSizeProvider,
        style: &ResolvedStyle,
        display: &DisplayGeometry,
    ) -> LayoutResult {
        // For rectangular displays, the panel width equals display width.
        // NOTE: For non-rectangular displays (usable_region is Some), this would
        // need to clip the layout width to the widest horizontal span of the
        // region at the panel's vertical position. Currently only the rectangular
        // path is implemented.
        let panel_width = display.bounds.width;
        let panel_height = style.bar_height as f32;

        let module_bounds = linear_layout(
            Axis::Horizontal,
            groups,
            sizes,
            panel_width,
            panel_height,
            &style.padding,
            &style.separator,
            style.module_gap,
        );

        let total_size = Size::new(panel_width, panel_height);
        let background_bounds = Rect::new(0.0, 0.0, panel_width, panel_height);

        LayoutResult {
            module_bounds,
            total_size,
            background_bounds,
        }
    }
}
