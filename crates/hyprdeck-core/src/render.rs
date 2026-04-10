use tiny_skia::Pixmap;

use crate::geometry::Rect;

/// Rendering context wrapping a `tiny_skia` pixmap with convenience helpers.
///
/// One `RenderContext` is created per panel surface per frame.  Modules receive
/// a `&mut Pixmap` directly (see [`crate::module::PanelModule::render`]); this
/// context is used by the panel compositor layer that draws backgrounds,
/// separators, and manages final buffer submission to Wayland.
pub struct RenderContext {
    /// The backing pixel buffer for this frame.
    pub pixmap: Pixmap,
    /// HiDPI scale factor (physical px per logical px).
    pub scale_factor: f32,
}

impl RenderContext {
    /// Allocate a new pixmap of the given *physical* dimensions.
    pub fn new(width: u32, height: u32, scale_factor: f32) -> Self {
        todo!()
    }

    /// Fill the entire pixmap with `color` ([r, g, b, a]).
    pub fn clear(&mut self, color: [u8; 4]) {
        todo!()
    }

    /// Fill a rounded rectangle with `color`.
    pub fn fill_rounded_rect(&mut self, bounds: Rect, radius: f32, color: [u8; 4]) {
        todo!()
    }

    /// Draw a horizontal or vertical separator line.
    pub fn draw_separator(&mut self, x: f32, y: f32, length: f32, vertical: bool, color: [u8; 4]) {
        todo!()
    }

    /// Blit `src` into this pixmap at the top-left corner of `dst_rect`.
    pub fn blit(&mut self, src: &Pixmap, dst_rect: Rect) {
        todo!()
    }

    /// Return the raw ARGB8 byte slice ready for submission to a Wayland shm buffer.
    pub fn as_bytes(&self) -> &[u8] {
        todo!()
    }

    /// Convert a logical-pixel rect to physical pixels using the scale factor.
    pub fn logical_to_physical(&self, rect: Rect) -> Rect {
        Rect {
            x: rect.x * self.scale_factor,
            y: rect.y * self.scale_factor,
            width: rect.width * self.scale_factor,
            height: rect.height * self.scale_factor,
        }
    }
}
