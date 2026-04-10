pub mod dock;
pub mod horizontal;
pub mod vertical;

pub use dock::{DockAnimState, DockLayout, DockLayoutConfig};
pub use horizontal::HorizontalLayout;
pub use vertical::VerticalLayout;

use crate::geometry::{DisplayGeometry, Point, Rect, Size};
use crate::panel::{Padding, ResolvedSeparator, ResolvedStyle};
use crate::theme::{DockConfig, LayoutType};

/// Result of a layout pass — maps module IDs to their computed bounds.
#[derive(Debug, Clone)]
pub struct LayoutResult {
    /// Ordered list of (module_id, bounding_rect) for each module.
    pub module_bounds: Vec<(String, Rect)>,
    /// Total size the panel surface needs to be.
    pub total_size: Size,
    /// Background rect (may differ from total_size for dock floating effect).
    pub background_bounds: Rect,
}

/// Groups of module IDs for three-region layout.
#[derive(Debug, Clone, Default)]
pub struct ModuleGroups {
    pub start: Vec<String>,
    pub center: Vec<String>,
    pub end: Vec<String>,
}

/// Trait for requesting module sizes during layout.
/// Decouples layout engine from direct module references.
pub trait ModuleSizeProvider {
    fn desired_size(&self, module_id: &str) -> Size;
}

/// Discriminated union of the three supported layout strategies.
#[derive(Debug)]
pub enum LayoutEngine {
    Horizontal(HorizontalLayout),
    Vertical(VerticalLayout),
    Dock(DockLayout),
}

impl LayoutEngine {
    /// Create the appropriate layout engine from theme config.
    pub fn from_panel_def(layout_type: &LayoutType, dock_config: Option<&DockConfig>) -> Self {
        match layout_type {
            LayoutType::Horizontal => LayoutEngine::Horizontal(HorizontalLayout::new()),
            LayoutType::Vertical => LayoutEngine::Vertical(VerticalLayout::new()),
            LayoutType::Dock => {
                let config = dock_config
                    .map(DockLayoutConfig::from_dock_config)
                    .unwrap_or_default();
                LayoutEngine::Dock(DockLayout::new(config))
            }
        }
    }

    /// Run a layout pass, computing bounds for all modules.
    pub fn layout(
        &self,
        groups: &ModuleGroups,
        sizes: &dyn ModuleSizeProvider,
        style: &ResolvedStyle,
        display: &DisplayGeometry,
    ) -> LayoutResult {
        match self {
            LayoutEngine::Horizontal(h) => h.layout(groups, sizes, style, display),
            LayoutEngine::Vertical(v) => v.layout(groups, sizes, style, display),
            LayoutEngine::Dock(d) => d.layout(groups, sizes, style, display),
        }
    }

    /// Notify the layout engine of cursor position.
    /// Only meaningful for Dock layout (magnification).
    /// Returns true if layout changed and redraw is needed.
    pub fn update_cursor(&mut self, cursor: Option<Point>) -> bool {
        match self {
            LayoutEngine::Dock(d) => d.update_cursor(cursor),
            _ => false,
        }
    }

    /// Tick animation state forward by dt seconds.
    /// Returns true if still animating (needs another frame).
    pub fn tick_animation(&mut self, dt: f32) -> bool {
        match self {
            LayoutEngine::Dock(d) => d.tick_animation(dt),
            _ => false,
        }
    }

    /// Ensure the dock animation state vectors match the given icon count.
    /// No-op for non-dock layouts.
    pub fn ensure_icon_count(&mut self, count: usize) {
        if let LayoutEngine::Dock(d) = self {
            d.ensure_icon_count(count);
        }
    }

    /// Returns true if the dock layout is currently animating.
    pub fn is_animating(&self) -> bool {
        match self {
            LayoutEngine::Dock(d) => d.state.animating,
            _ => false,
        }
    }
}

// ── Shared linear layout ──────────────────────────────────────────────────────

/// Axis along which modules are arranged in a linear layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

/// Compute the total separator gap between adjacent modules.
fn separator_gap(separator: &ResolvedSeparator) -> f32 {
    if separator.visible {
        separator.margin + separator.width + separator.margin
    } else {
        0.0
    }
}

/// Measure the total main-axis extent of a group of modules, including separator gaps.
fn group_main_extent(
    group: &[String],
    sizes: &dyn ModuleSizeProvider,
    axis: Axis,
    sep_gap: f32,
) -> f32 {
    if group.is_empty() {
        return 0.0;
    }
    let mut total = 0.0;
    for (i, id) in group.iter().enumerate() {
        if i > 0 {
            total += sep_gap;
        }
        let s = sizes.desired_size(id);
        total += match axis {
            Axis::Horizontal => s.width,
            Axis::Vertical => s.height,
        };
    }
    total
}

/// Place a group of modules along the main axis, starting at `cursor`.
/// Returns the new cursor position after the last module.
fn place_group(
    group: &[String],
    sizes: &dyn ModuleSizeProvider,
    axis: Axis,
    cursor_start: f32,
    cross_start: f32,
    cross_extent: f32,
    sep_gap: f32,
    out: &mut Vec<(String, Rect)>,
) -> f32 {
    let mut cursor = cursor_start;
    for (i, id) in group.iter().enumerate() {
        if i > 0 {
            cursor += sep_gap;
        }
        let s = sizes.desired_size(id);
        let main_size = match axis {
            Axis::Horizontal => s.width,
            Axis::Vertical => s.height,
        };
        let rect = match axis {
            Axis::Horizontal => Rect::new(cursor, cross_start, main_size, cross_extent),
            Axis::Vertical => Rect::new(cross_start, cursor, cross_extent, main_size),
        };
        out.push((id.clone(), rect));
        cursor += main_size;
    }
    cursor
}

/// Generic linear layout along an axis. Used by both Horizontal and Vertical engines.
///
/// Lays out start/center/end groups along `panel_main_extent` (width for H, height for V),
/// with modules stretching to fill `panel_cross_extent` (height for H, width for V).
pub fn linear_layout(
    axis: Axis,
    groups: &ModuleGroups,
    sizes: &dyn ModuleSizeProvider,
    panel_main_extent: f32,
    panel_cross_extent: f32,
    padding: &Padding,
    separator: &ResolvedSeparator,
) -> Vec<(String, Rect)> {
    let (pad_main_start, pad_main_end, pad_cross_start, pad_cross_end) = match axis {
        Axis::Horizontal => (padding.left, padding.right, padding.top, padding.bottom),
        Axis::Vertical => (padding.top, padding.bottom, padding.left, padding.right),
    };

    let module_cross = panel_cross_extent - pad_cross_start - pad_cross_end;
    let sep_gap = separator_gap(separator);

    // Measure each group's total main-axis extent.
    let _start_extent = group_main_extent(&groups.start, sizes, axis, sep_gap);
    let center_extent = group_main_extent(&groups.center, sizes, axis, sep_gap);
    let end_extent = group_main_extent(&groups.end, sizes, axis, sep_gap);

    let mut result = Vec::new();

    // Place start group flush to the start edge.
    let start_cursor = pad_main_start;
    let start_end = place_group(
        &groups.start,
        sizes,
        axis,
        start_cursor,
        pad_cross_start,
        module_cross,
        sep_gap,
        &mut result,
    );

    // Place end group flush to the end edge.
    let end_cursor = panel_main_extent - pad_main_end - end_extent;
    place_group(
        &groups.end,
        sizes,
        axis,
        end_cursor,
        pad_cross_start,
        module_cross,
        sep_gap,
        &mut result,
    );

    // Place center group centered in available space.
    if !groups.center.is_empty() {
        let available_main = panel_main_extent - pad_main_start - pad_main_end;
        let midpoint = pad_main_start + available_main / 2.0;
        let mut center_start = midpoint - center_extent / 2.0;

        // Collision avoidance with start and end groups.
        let min_gap = 8.0;
        let start_boundary = if groups.start.is_empty() {
            pad_main_start
        } else {
            start_end + min_gap
        };
        let end_boundary = if groups.end.is_empty() {
            panel_main_extent - pad_main_end
        } else {
            end_cursor - min_gap
        };

        if center_start < start_boundary {
            center_start = start_boundary;
        }
        if center_start + center_extent > end_boundary {
            center_start = (end_boundary - center_extent).max(start_boundary);
        }

        place_group(
            &groups.center,
            sizes,
            axis,
            center_start,
            pad_cross_start,
            module_cross,
            sep_gap,
            &mut result,
        );
    }

    result
}
