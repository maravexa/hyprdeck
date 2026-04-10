use crate::geometry::{DisplayGeometry, Rect};
use crate::panel::Panel;

/// State for a single Wayland output (monitor).
///
/// One output corresponds to one physical monitor. Each output may host
/// one or more [`Panel`] instances as declared by the active theme.
#[derive(Debug)]
pub struct OutputState {
    /// Output name (from Wayland and Hyprland, e.g. `"DP-1"`).
    pub name: String,
    /// Output width in pixels.
    pub width: u32,
    /// Output height in pixels.
    pub height: u32,
    /// Display geometry passed to layout engines.
    pub display_geometry: DisplayGeometry,
    /// Panels on this output (typically 1–2, but theme can define more).
    pub panels: Vec<Panel>,
}

impl OutputState {
    /// Create a new output state with no panels.
    pub fn new(name: String, width: u32, height: u32) -> Self {
        let display_geometry = DisplayGeometry {
            bounds: Rect::new(0.0, 0.0, width as f32, height as f32),
            usable_region: None,
            edge_path: None,
        };
        Self {
            name,
            width,
            height,
            display_geometry,
            panels: Vec::new(),
        }
    }

    /// Redraw all panels that have been marked dirty.
    pub fn render_dirty_panels(&mut self) {
        for panel in &mut self.panels {
            if panel.dirty {
                panel.frame(&self.display_geometry);
            }
        }
    }

    /// Check if any panel needs a frame.
    pub fn has_dirty_panels(&self) -> bool {
        self.panels.iter().any(|p| p.dirty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_output_has_no_panels() {
        let output = OutputState::new("DP-1".to_owned(), 1920, 1080);
        assert!(output.panels.is_empty());
        assert!(!output.has_dirty_panels());
    }

    #[test]
    fn display_geometry_matches_dimensions() {
        let output = OutputState::new("HDMI-A-1".to_owned(), 3840, 2160);
        assert_eq!(output.display_geometry.bounds.width, 3840.0);
        assert_eq!(output.display_geometry.bounds.height, 2160.0);
    }
}
