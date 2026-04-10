use crate::geometry::{DisplayGeometry, Rect, Size};
use crate::panel::ResolvedStyle;

use super::{linear_layout, Axis, LayoutResult, ModuleGroups, ModuleSizeProvider};

/// Three-group vertical layout: `start | center | end` (top → bottom).
///
/// Mirrors [`HorizontalLayout`](super::HorizontalLayout) but along the Y axis;
/// used for left/right edge panels.
///
/// - Panel occupies full screen height, fixed width from style.
/// - Modules flow top-to-bottom within groups.
/// - Start group = top, End group = bottom, Center group = centered vertically.
/// - Module width: bar_width - padding.left - padding.right (stretch to fill).
/// - Module height: desired_size().height per module.
#[derive(Debug, Clone)]
pub struct VerticalLayout {
    // No mutable state needed — vertical layout is purely functional.
}

impl VerticalLayout {
    pub fn new() -> Self {
        VerticalLayout {}
    }

    pub fn layout(
        &self,
        groups: &ModuleGroups,
        sizes: &dyn ModuleSizeProvider,
        style: &ResolvedStyle,
        display: &DisplayGeometry,
    ) -> LayoutResult {
        // For rectangular displays, the panel height equals display height.
        // NOTE: For non-rectangular displays (usable_region is Some), this would
        // need to clip the layout height to the tallest vertical span of the
        // region at the panel's horizontal position.
        let panel_height = display.bounds.height;
        let panel_width = style.bar_height as f32; // bar_height is the panel thickness

        let module_bounds = linear_layout(
            Axis::Vertical,
            groups,
            sizes,
            panel_height,
            panel_width,
            &style.padding,
            &style.separator,
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
