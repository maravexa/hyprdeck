use tiny_skia::{FillRule, LineCap, Paint, PathBuilder, Pixmap, PixmapPaint, Stroke, Transform};

use crate::geometry::{Point, Rect};
use crate::layout::{Axis, LayoutResult};
use crate::module::{PanelModule, ThemeContext};
use crate::panel::{Color, ResolvedSeparator, ResolvedStyle};

/// Canvas wraps a `tiny_skia::Pixmap` and provides high-level draw operations.
///
/// One Canvas is created per panel surface. Modules receive `&mut Pixmap` directly
/// for low-level drawing; this canvas is used by the panel compositor layer that
/// draws backgrounds, separators, and manages the final buffer submission to Wayland.
pub struct Canvas {
    pixmap: Pixmap,
    font_system: cosmic_text::FontSystem,
    swash_cache: cosmic_text::SwashCache,
}

impl std::fmt::Debug for Canvas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Canvas")
            .field("width", &self.pixmap.width())
            .field("height", &self.pixmap.height())
            .finish_non_exhaustive()
    }
}

impl Canvas {
    /// Create a new canvas with the given pixel dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        let pixmap = Pixmap::new(width.max(1), height.max(1)).expect("failed to allocate pixmap");
        let font_system = cosmic_text::FontSystem::new();
        let swash_cache = cosmic_text::SwashCache::new();
        Self {
            pixmap,
            font_system,
            swash_cache,
        }
    }

    /// Resize the canvas (reallocates the pixmap).
    pub fn resize(&mut self, width: u32, height: u32) {
        if let Some(pm) = Pixmap::new(width.max(1), height.max(1)) {
            self.pixmap = pm;
        }
    }

    /// Clear the entire canvas to transparent.
    pub fn clear(&mut self) {
        self.pixmap.fill(tiny_skia::Color::TRANSPARENT);
    }

    /// Get the underlying pixmap for passing to Wayland or module renderers.
    pub fn pixmap(&self) -> &Pixmap {
        &self.pixmap
    }

    /// Get a mutable reference to the underlying pixmap (for module rendering).
    pub fn pixmap_mut(&mut self) -> &mut Pixmap {
        &mut self.pixmap
    }

    /// Get the pixel data as a byte slice (for Wayland buffer submission).
    pub fn data(&self) -> &[u8] {
        self.pixmap.data()
    }

    /// Canvas width in pixels.
    pub fn width(&self) -> u32 {
        self.pixmap.width()
    }

    /// Canvas height in pixels.
    pub fn height(&self) -> u32 {
        self.pixmap.height()
    }

    // ── Primitive Drawing ──────────────────────────────────

    /// Fill a rectangle with a solid color.
    pub fn fill_rect(&mut self, rect: Rect, color: Color) {
        let Some(skia_rect) = rect.to_skia() else {
            return;
        };
        let mut paint = Paint::default();
        paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
        paint.anti_alias = false;
        self.pixmap
            .fill_rect(skia_rect, &paint, Transform::identity(), None);
    }

    /// Fill a rounded rectangle with a solid color.
    pub fn fill_rounded_rect(&mut self, rect: Rect, color: Color, radius: f32) {
        if radius <= 0.0 {
            self.fill_rect(rect, color);
            return;
        }
        let Some(path) = rounded_rect_path(&rect, radius) else {
            return;
        };
        let mut paint = Paint::default();
        paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
        paint.anti_alias = true;
        self.pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    /// Fill a rounded rectangle with an opacity multiplier.
    pub fn fill_rounded_rect_alpha(&mut self, rect: Rect, color: Color, radius: f32, opacity: f32) {
        let a = (color[3] as f32 * opacity.clamp(0.0, 1.0)) as u8;
        self.fill_rounded_rect(rect, [color[0], color[1], color[2], a], radius);
    }

    /// Draw a vertical gradient rectangle (top_color at top, bottom_color at bottom).
    pub fn fill_gradient_rect(&mut self, rect: Rect, top_color: Color, bottom_color: Color) {
        let Some(skia_rect) = rect.to_skia() else {
            return;
        };
        let mut pb = PathBuilder::new();
        pb.push_rect(skia_rect);
        let Some(path) = pb.finish() else {
            return;
        };

        let stops = vec![
            tiny_skia::GradientStop::new(
                0.0,
                tiny_skia::Color::from_rgba8(
                    top_color[0],
                    top_color[1],
                    top_color[2],
                    top_color[3],
                ),
            ),
            tiny_skia::GradientStop::new(
                1.0,
                tiny_skia::Color::from_rgba8(
                    bottom_color[0],
                    bottom_color[1],
                    bottom_color[2],
                    bottom_color[3],
                ),
            ),
        ];

        let Some(gradient) = tiny_skia::LinearGradient::new(
            tiny_skia::Point::from_xy(rect.x, rect.y),
            tiny_skia::Point::from_xy(rect.x, rect.y + rect.height),
            stops,
            tiny_skia::SpreadMode::Pad,
            Transform::identity(),
        ) else {
            return;
        };

        let paint = Paint {
            shader: gradient,
            anti_alias: false,
            ..Paint::default()
        };
        self.pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    /// Draw a line between two points.
    pub fn draw_line(&mut self, from: Point, to: Point, color: Color, width: f32) {
        let mut pb = PathBuilder::new();
        pb.move_to(from.x, from.y);
        pb.line_to(to.x, to.y);
        let Some(path) = pb.finish() else {
            return;
        };

        let mut paint = Paint::default();
        paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
        paint.anti_alias = true;

        let stroke = Stroke {
            width,
            line_cap: LineCap::Round,
            ..Stroke::default()
        };
        self.pixmap
            .stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }

    /// Draw a filled circle.
    pub fn fill_circle(&mut self, center: Point, radius: f32, color: Color) {
        let Some(path) = circle_path(center, radius) else {
            return;
        };
        let mut paint = Paint::default();
        paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
        paint.anti_alias = true;
        self.pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    /// Draw a moon phase circle with a terminator curve.
    ///
    /// `fraction`: 0.0 = new moon (fully dark), 0.5 = full moon (fully lit), 1.0 = new moon again.
    /// The moon is centered within `bounds` using the smaller dimension as diameter.
    pub fn draw_moon_phase(
        &mut self,
        bounds: Rect,
        fraction: f64,
        lit_color: Color,
        dark_color: Color,
    ) {
        let cx = bounds.x + bounds.width / 2.0;
        let cy = bounds.y + bounds.height / 2.0;
        let radius = (bounds.width.min(bounds.height) / 2.0) - 1.0;

        if radius <= 0.0 {
            return;
        }

        self.fill_circle(crate::geometry::Point::new(cx, cy), radius, dark_color);

        let phase = fraction.rem_euclid(1.0) as f32;
        let terminator_x = if phase <= 0.5 {
            radius * (1.0 - phase * 4.0).max(-1.0)
        } else {
            radius * ((phase - 0.5) * 4.0 - 1.0).min(1.0)
        };

        if let Some(path) = build_lit_path(cx, cy, radius, terminator_x, phase) {
            let mut paint = Paint::default();
            paint.set_color_rgba8(lit_color[0], lit_color[1], lit_color[2], lit_color[3]);
            paint.anti_alias = true;
            self.pixmap.fill_path(
                &path,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }

    // ── Image Drawing ──────────────────────────────────────

    /// Draw an RGBA image scaled to fit within the given rect.
    pub fn draw_image(&mut self, image: &image::RgbaImage, dest: Rect) {
        self.draw_image_alpha(image, dest, 1.0);
    }

    /// Draw an RGBA image scaled to fit, with an opacity multiplier.
    pub fn draw_image_alpha(&mut self, image: &image::RgbaImage, dest: Rect, opacity: f32) {
        let (w, h) = image.dimensions();
        if w == 0 || h == 0 {
            return;
        }
        let Some(src_pixmap) = rgba_image_to_pixmap(image) else {
            return;
        };

        let scale_x = dest.width / w as f32;
        let scale_y = dest.height / h as f32;
        let transform = Transform::from_scale(scale_x, scale_y);

        let paint = PixmapPaint {
            opacity: opacity.clamp(0.0, 1.0),
            ..PixmapPaint::default()
        };

        self.pixmap.draw_pixmap(
            dest.x as i32,
            dest.y as i32,
            src_pixmap.as_ref(),
            &paint,
            transform,
            None,
        );
    }

    /// Draw a pre-rendered tiny_skia Pixmap at a position.
    pub fn draw_pixmap(&mut self, pixmap: &Pixmap, x: i32, y: i32) {
        self.pixmap.draw_pixmap(
            x,
            y,
            pixmap.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
    }

    // ── Text Drawing ───────────────────────────────────────

    /// Draw a single line of text, left-aligned within the given rect.
    /// Text is vertically centered within the rect.
    /// Returns the width of the rendered text in pixels.
    pub fn draw_text(
        &mut self,
        text: &str,
        rect: Rect,
        font_family: &str,
        font_size: f32,
        color: Color,
    ) -> f32 {
        self.draw_text_aligned(text, rect, font_family, font_size, color, TextAlign::Left)
    }

    /// Draw text centered horizontally and vertically within the rect.
    pub fn draw_text_centered(
        &mut self,
        text: &str,
        rect: Rect,
        font_family: &str,
        font_size: f32,
        color: Color,
    ) -> f32 {
        self.draw_text_aligned(text, rect, font_family, font_size, color, TextAlign::Center)
    }

    /// Draw text right-aligned within the rect.
    pub fn draw_text_right(
        &mut self,
        text: &str,
        rect: Rect,
        font_family: &str,
        font_size: f32,
        color: Color,
    ) -> f32 {
        self.draw_text_aligned(text, rect, font_family, font_size, color, TextAlign::Right)
    }

    /// Measure the width of a text string without drawing it.
    /// Used by modules in `desired_size()` calculations.
    pub fn measure_text(&mut self, text: &str, font_family: &str, font_size: f32) -> f32 {
        let line_height = (font_size * 1.2).ceil();
        let metrics = cosmic_text::Metrics::new(font_size, line_height);
        let mut buffer = cosmic_text::Buffer::new(&mut self.font_system, metrics);

        buffer.set_size(&mut self.font_system, Some(f32::MAX), Some(line_height));

        let attrs = cosmic_text::Attrs::new().family(cosmic_text::Family::Name(font_family));
        buffer.set_text(
            &mut self.font_system,
            text,
            attrs,
            cosmic_text::Shaping::Advanced,
        );
        buffer.shape_until_scroll(&mut self.font_system, false);

        buffer
            .layout_runs()
            .map(|run| run.line_w)
            .fold(0.0f32, f32::max)
    }

    /// Draw text that truncates with ellipsis if it exceeds the rect width.
    pub fn draw_text_ellipsis(
        &mut self,
        text: &str,
        rect: Rect,
        font_family: &str,
        font_size: f32,
        color: Color,
    ) -> f32 {
        let text_width = self.measure_text(text, font_family, font_size);
        if text_width <= rect.width {
            return self.draw_text(text, rect, font_family, font_size, color);
        }

        // Binary search for the longest prefix that fits with ellipsis.
        let ellipsis = "\u{2026}";
        let ellipsis_width = self.measure_text(ellipsis, font_family, font_size);
        let available = rect.width - ellipsis_width;

        if available <= 0.0 {
            return self.draw_text(ellipsis, rect, font_family, font_size, color);
        }

        let chars: Vec<char> = text.chars().collect();
        let mut lo = 0usize;
        let mut hi = chars.len();
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            let prefix: String = chars[..mid].iter().collect();
            let w = self.measure_text(&prefix, font_family, font_size);
            if w <= available {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }

        let truncated: String = chars[..lo].iter().collect::<String>() + ellipsis;
        self.draw_text(&truncated, rect, font_family, font_size, color)
    }

    // ── Panel-Level Drawing ────────────────────────────────

    /// Draw the panel background (solid color with optional rounded corners).
    pub fn draw_panel_background(&mut self, bounds: Rect, style: &ResolvedStyle) {
        let bg = style.colors.background;
        let opacity = style.background_opacity;
        let radius = style.border_radius;

        if radius > 0.0 {
            self.fill_rounded_rect_alpha(bounds, bg, radius, opacity);
        } else {
            let a = (bg[3] as f32 * opacity.clamp(0.0, 1.0)) as u8;
            self.fill_rect(bounds, [bg[0], bg[1], bg[2], a]);
        }
    }

    /// Draw a separator line at the given main-axis position.
    ///
    /// For horizontal panels, draws a vertical line at `position` spanning the
    /// cross-axis extent within `bounds`. For vertical panels, draws a horizontal line.
    pub fn draw_separator(
        &mut self,
        position: f32,
        axis: Axis,
        bounds: Rect,
        separator: &ResolvedSeparator,
    ) {
        if !separator.visible {
            return;
        }
        let (from, to) = match axis {
            Axis::Horizontal => (
                Point::new(position, bounds.y + separator.margin),
                Point::new(position, bounds.y + bounds.height - separator.margin),
            ),
            Axis::Vertical => (
                Point::new(bounds.x + separator.margin, position),
                Point::new(bounds.x + bounds.width - separator.margin, position),
            ),
        };
        self.draw_line(from, to, separator.color, separator.width);
    }

    /// Draw all separators implied by a layout result.
    pub fn draw_layout_separators(
        &mut self,
        layout: &LayoutResult,
        axis: Axis,
        separator: &ResolvedSeparator,
    ) {
        if !separator.visible || layout.module_bounds.len() < 2 {
            return;
        }
        let bounds = layout.background_bounds;
        let gap = separator.margin + separator.width + separator.margin;

        for pair in layout.module_bounds.windows(2) {
            let (_, r1) = &pair[0];
            let (_, r2) = &pair[1];
            // Check if there is a gap between these modules (separator space).
            let (end1, start2) = match axis {
                Axis::Horizontal => (r1.x + r1.width, r2.x),
                Axis::Vertical => (r1.y + r1.height, r2.y),
            };
            let space = start2 - end1;
            if space >= gap * 0.5 {
                let mid = end1 + space / 2.0;
                self.draw_separator(mid, axis, bounds, separator);
            }
        }
    }

    // ── Internal helpers ───────────────────────────────────

    fn draw_text_aligned(
        &mut self,
        text: &str,
        rect: Rect,
        font_family: &str,
        font_size: f32,
        color: Color,
        align: TextAlign,
    ) -> f32 {
        let line_height = (font_size * 1.2).ceil();
        let metrics = cosmic_text::Metrics::new(font_size, line_height);
        let mut buffer = cosmic_text::Buffer::new(&mut self.font_system, metrics);

        buffer.set_size(&mut self.font_system, Some(rect.width), Some(rect.height));

        let attrs = cosmic_text::Attrs::new().family(cosmic_text::Family::Name(font_family));
        buffer.set_text(
            &mut self.font_system,
            text,
            attrs,
            cosmic_text::Shaping::Advanced,
        );
        buffer.shape_until_scroll(&mut self.font_system, false);

        // Measure text width for horizontal alignment.
        let text_width: f32 = buffer
            .layout_runs()
            .map(|run| run.line_w)
            .fold(0.0f32, f32::max);

        // Compute horizontal offset based on alignment.
        let x_offset = match align {
            TextAlign::Left => rect.x,
            TextAlign::Center => rect.x + (rect.width - text_width) / 2.0,
            TextAlign::Right => rect.x + rect.width - text_width,
        };

        // Vertical centering base: where the top of the line_height box sits in the rect.
        // Each run's actual y-origin is y_offset_base + run.line_y, which is the baseline
        // for that line in pixmap coordinates. cosmic-text's run.line_y already accounts
        // for the centering within line_height (centering_offset + max_ascent), so adding
        // it here gives glyph_y = (y_offset_base + run.line_y) - placement.top which
        // correctly places glyphs centred in the rect.
        let y_offset_base = rect.y + (rect.height - line_height) / 2.0;

        // Render glyphs.
        let pixmap_width = self.pixmap.width() as i32;
        let pixmap_height = self.pixmap.height() as i32;

        for run in buffer.layout_runs() {
            let y_offset = y_offset_base + run.line_y;
            for glyph in run.glyphs.iter() {
                let physical = glyph.physical((x_offset, y_offset), 1.0);

                let Some(image) = self
                    .swash_cache
                    .get_image(&mut self.font_system, physical.cache_key)
                else {
                    continue;
                };

                let glyph_x = physical.x + image.placement.left;
                let glyph_y = physical.y - image.placement.top;
                let img_w = image.placement.width as i32;
                let img_h = image.placement.height as i32;

                match image.content {
                    cosmic_text::SwashContent::Mask => {
                        let data = self.pixmap.data_mut();
                        let stride = pixmap_width as usize * 4;

                        for row in 0..img_h {
                            for col in 0..img_w {
                                let px = glyph_x + col;
                                let py = glyph_y + row;
                                if px < 0 || py < 0 || px >= pixmap_width || py >= pixmap_height {
                                    continue;
                                }
                                let alpha = image.data[(row * img_w + col) as usize];
                                if alpha == 0 {
                                    continue;
                                }
                                alpha_blend_pixel(
                                    data,
                                    stride,
                                    px as usize,
                                    py as usize,
                                    color,
                                    alpha,
                                );
                            }
                        }
                    }
                    cosmic_text::SwashContent::Color => {
                        let data = self.pixmap.data_mut();
                        let stride = pixmap_width as usize * 4;

                        for row in 0..img_h {
                            for col in 0..img_w {
                                let px = glyph_x + col;
                                let py = glyph_y + row;
                                if px < 0 || py < 0 || px >= pixmap_width || py >= pixmap_height {
                                    continue;
                                }
                                let src_idx = (row * img_w + col) as usize * 4;
                                let src_color = [
                                    image.data[src_idx],
                                    image.data[src_idx + 1],
                                    image.data[src_idx + 2],
                                    image.data[src_idx + 3],
                                ];
                                alpha_blend_pixel(
                                    data,
                                    stride,
                                    px as usize,
                                    py as usize,
                                    src_color,
                                    src_color[3],
                                );
                            }
                        }
                    }
                    cosmic_text::SwashContent::SubpixelMask => {
                        // Subpixel rendering not supported; fall back to luminance mask.
                        let data = self.pixmap.data_mut();
                        let stride = pixmap_width as usize * 4;

                        for row in 0..img_h {
                            for col in 0..img_w {
                                let px = glyph_x + col;
                                let py = glyph_y + row;
                                if px < 0 || py < 0 || px >= pixmap_width || py >= pixmap_height {
                                    continue;
                                }
                                let src_idx = (row * img_w + col) as usize * 3;
                                let r = image.data[src_idx] as u16;
                                let g = image.data[src_idx + 1] as u16;
                                let b = image.data[src_idx + 2] as u16;
                                let alpha = ((r + g + b) / 3) as u8;
                                if alpha == 0 {
                                    continue;
                                }
                                alpha_blend_pixel(
                                    data,
                                    stride,
                                    px as usize,
                                    py as usize,
                                    color,
                                    alpha,
                                );
                            }
                        }
                    }
                }
            }
        }

        text_width
    }
}

// ── Font sizing helpers ────────────────────────────────────────────────────────

/// Compute an effective font size that fills the available height.
///
/// Scales the height to ~65% to leave breathing room above and below the text.
/// Returns the larger of the height-derived value and `configured_size`, so the
/// configured size acts as a floor (text is never shrunk by this function).
pub fn effective_font_size(available_height: f32, configured_size: f32) -> f32 {
    let height_derived = (available_height * 0.75).floor();
    height_derived.max(configured_size)
}

// ── Text alignment ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
enum TextAlign {
    Left,
    Center,
    Right,
}

// ── Path helpers ───────────────────────────────────────────────────────────────

/// Build a rounded rectangle path using quadratic Bézier curves at the corners.
fn rounded_rect_path(rect: &Rect, radius: f32) -> Option<tiny_skia::Path> {
    let r = radius.min(rect.width / 2.0).min(rect.height / 2.0).max(0.0);
    let x = rect.x;
    let y = rect.y;
    let w = rect.width;
    let h = rect.height;

    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
    pb.finish()
}

/// Build a circle path using cubic Bézier approximation.
fn circle_path(center: Point, radius: f32) -> Option<tiny_skia::Path> {
    // Kappa: control point distance for cubic Bézier quarter-circle approximation.
    const KAPPA: f32 = 0.552_284_8;
    let r = radius;
    let k = r * KAPPA;
    let cx = center.x;
    let cy = center.y;

    let mut pb = PathBuilder::new();
    pb.move_to(cx + r, cy);
    pb.cubic_to(cx + r, cy + k, cx + k, cy + r, cx, cy + r);
    pb.cubic_to(cx - k, cy + r, cx - r, cy + k, cx - r, cy);
    pb.cubic_to(cx - r, cy - k, cx - k, cy - r, cx, cy - r);
    pb.cubic_to(cx + k, cy - r, cx + r, cy - k, cx + r, cy);
    pb.close();
    pb.finish()
}

// ── Image conversion ───────────────────────────────────────────────────────────

/// Convert an `image::RgbaImage` (straight alpha) to a `tiny_skia::Pixmap` (premultiplied alpha).
fn rgba_image_to_pixmap(img: &image::RgbaImage) -> Option<Pixmap> {
    let (w, h) = img.dimensions();
    let mut pixmap = Pixmap::new(w, h)?;
    let data = pixmap.data_mut();
    for (i, pixel) in img.pixels().enumerate() {
        let r = pixel[0];
        let g = pixel[1];
        let b = pixel[2];
        let a = pixel[3];
        let af = a as f32 / 255.0;
        let idx = i * 4;
        data[idx] = (r as f32 * af + 0.5) as u8;
        data[idx + 1] = (g as f32 * af + 0.5) as u8;
        data[idx + 2] = (b as f32 * af + 0.5) as u8;
        data[idx + 3] = a;
    }
    Some(pixmap)
}

// ── Alpha blending ─────────────────────────────────────────────────────────────

/// Alpha-blend a single pixel of `color` with `glyph_alpha` onto the pixmap data.
/// The pixmap stores premultiplied-alpha RGBA.
fn alpha_blend_pixel(
    data: &mut [u8],
    stride: usize,
    px: usize,
    py: usize,
    color: Color,
    glyph_alpha: u8,
) {
    let idx = py * stride + px * 4;
    if idx + 3 >= data.len() {
        return;
    }

    let src_a = (color[3] as f32 / 255.0) * (glyph_alpha as f32 / 255.0);
    if src_a < 1.0 / 512.0 {
        return;
    }

    // Source premultiplied.
    let src_r = color[0] as f32 / 255.0 * src_a;
    let src_g = color[1] as f32 / 255.0 * src_a;
    let src_b = color[2] as f32 / 255.0 * src_a;

    // Destination (already premultiplied).
    let dst_r = data[idx] as f32 / 255.0;
    let dst_g = data[idx + 1] as f32 / 255.0;
    let dst_b = data[idx + 2] as f32 / 255.0;
    let dst_a = data[idx + 3] as f32 / 255.0;

    // Porter-Duff "over" compositing.
    let inv_sa = 1.0 - src_a;
    let out_r = src_r + dst_r * inv_sa;
    let out_g = src_g + dst_g * inv_sa;
    let out_b = src_b + dst_b * inv_sa;
    let out_a = src_a + dst_a * inv_sa;

    data[idx] = (out_r * 255.0 + 0.5) as u8;
    data[idx + 1] = (out_g * 255.0 + 0.5) as u8;
    data[idx + 2] = (out_b * 255.0 + 0.5) as u8;
    data[idx + 3] = (out_a * 255.0 + 0.5) as u8;
}

// ── Moon phase path builder ───────────────────────────────────────────────────

/// Build the lit portion of a moon phase as a closed path.
///
/// The lit area is bounded by a semicircular limb arc on the lit side and an
/// elliptical terminator arc. `terminator_x` is the x-radius of the terminator
/// ellipse: +radius aligns it with the limb (nothing lit), -radius places it at
/// the opposite limb (fully lit), 0 is a straight vertical line (quarter phase).
fn build_lit_path(
    cx: f32,
    cy: f32,
    radius: f32,
    terminator_x: f32,
    phase: f32,
) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    let waxing = phase <= 0.5;
    let segments = 32u32;

    // Start at the top of the moon (shared by both arcs).
    pb.move_to(cx, cy - radius);

    if waxing {
        // Lit portion on the RIGHT: trace right limb top→bottom, then terminator bottom→top.
        for i in 1..=segments {
            let angle =
                -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * (i as f32 / segments as f32);
            pb.line_to(cx + radius * angle.cos(), cy + radius * angle.sin());
        }
        for i in (0..=segments).rev() {
            let angle =
                -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * (i as f32 / segments as f32);
            pb.line_to(cx + terminator_x * angle.cos(), cy + radius * angle.sin());
        }
    } else {
        // Lit portion on the LEFT: trace left limb top→bottom, then terminator bottom→top.
        for i in 1..=segments {
            let angle =
                -std::f32::consts::FRAC_PI_2 - std::f32::consts::PI * (i as f32 / segments as f32);
            pb.line_to(cx + radius * angle.cos(), cy + radius * angle.sin());
        }
        for i in (0..=segments).rev() {
            let angle =
                -std::f32::consts::FRAC_PI_2 - std::f32::consts::PI * (i as f32 / segments as f32);
            pb.line_to(cx + terminator_x * angle.cos(), cy + radius * angle.sin());
        }
    }

    pb.close();
    pb.finish()
}

// ── Panel-level rendering ─────────────────────────────────────────────────────

/// Render a complete panel frame: clear canvas, draw background, render each
/// module into its layout-assigned bounds, then draw separators.
pub fn render_panel(
    canvas: &mut Canvas,
    layout: &LayoutResult,
    modules: &[Box<dyn PanelModule>],
    style: &ResolvedStyle,
    theme_ctx: &ThemeContext,
) {
    // Clear to transparent
    canvas.clear();

    // Draw panel background
    canvas.draw_panel_background(layout.background_bounds, style);

    // Render each module into its assigned bounds
    for (module_id, bounds) in &layout.module_bounds {
        if let Some(module) = modules.iter().find(|m| m.id() == module_id) {
            module.render(canvas.pixmap_mut(), theme_ctx, *bounds);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Rect;

    fn term(radius: f32, phase: f32) -> f32 {
        if phase <= 0.5 {
            radius * (1.0 - phase * 4.0).max(-1.0)
        } else {
            radius * ((phase - 0.5) * 4.0 - 1.0).min(1.0)
        }
    }

    #[test]
    fn build_lit_path_returns_some_for_all_phases() {
        for i in 0..=100 {
            let phase = i as f32 / 100.0;
            let t = term(40.0, phase);
            assert!(
                build_lit_path(50.0, 50.0, 40.0, t, phase).is_some(),
                "phase {phase} returned None"
            );
        }
    }

    #[test]
    fn draw_moon_phase_no_panic_zero_radius() {
        let mut canvas = Canvas::new(4, 4);
        // bounds of 2x2 → radius = 0.0 after subtracting 1.0 → early return
        canvas.draw_moon_phase(
            Rect::new(1.0, 1.0, 2.0, 2.0),
            0.25,
            [255, 255, 255, 255],
            [30, 30, 30, 100],
        );
    }

    #[test]
    fn draw_moon_phase_no_panic_tiny_bounds() {
        let mut canvas = Canvas::new(10, 10);
        canvas.draw_moon_phase(
            Rect::new(0.0, 0.0, 3.0, 3.0),
            0.125,
            [255, 255, 255, 255],
            [30, 30, 30, 100],
        );
    }

    #[test]
    fn render_moon_phases_to_png() {
        let lit: Color = [205, 214, 244, 255];
        let dark: Color = [30, 30, 46, 100];
        for i in 0..8u32 {
            let fraction = i as f64 / 8.0;
            let mut canvas = Canvas::new(100, 100);
            canvas.fill_rect(Rect::new(0.0, 0.0, 100.0, 100.0), [30, 30, 46, 255]);
            canvas.draw_moon_phase(Rect::new(10.0, 10.0, 80.0, 80.0), fraction, lit, dark);
            canvas
                .pixmap()
                .save_png(format!("/tmp/moon_phase_{i}.png"))
                .unwrap();
        }
    }
}
