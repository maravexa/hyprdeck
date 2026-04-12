//! Rendering primitives for module implementations.
//!
//! Provides shape and text drawing that works on a borrowed `&mut Pixmap`, matching
//! the `PanelModule::render` signature.  Text rendering uses a thread-local
//! `FontSystem` and `SwashCache` so each call doesn't reallocate font state.

use std::cell::RefCell;

use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache, SwashContent};
use hyprdeck_core::{Color, Point, Rect};
use tiny_skia::{FillRule, LineCap, Paint, PathBuilder, PixmapPaint, Stroke, Transform};

// Re-export Pixmap so callers only need one import.
pub use tiny_skia::Pixmap;

// ── Thread-local font state ───────────────────────────────────────────────────

thread_local! {
    static FONT_SYSTEM: RefCell<FontSystem> = RefCell::new(FontSystem::new());
    static SWASH_CACHE: RefCell<SwashCache> = RefCell::new(SwashCache::new());
}

// ── Text alignment ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
enum TextAlign {
    Left,
    Center,
}

// ── Shape primitives ──────────────────────────────────────────────────────────

/// Fill a solid rectangle.
pub fn fill_rect(pixmap: &mut Pixmap, rect: Rect, color: Color) {
    let Some(r) = rect.to_skia() else {
        return;
    };
    let mut paint = Paint::default();
    paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
    paint.anti_alias = false;
    pixmap.fill_rect(r, &paint, Transform::identity(), None);
}

/// Fill a rounded rectangle.  Falls back to `fill_rect` when `radius <= 0`.
pub fn fill_rounded_rect(pixmap: &mut Pixmap, rect: Rect, color: Color, radius: f32) {
    if radius <= 0.0 {
        fill_rect(pixmap, rect, color);
        return;
    }
    let Some(path) = rounded_rect_path(&rect, radius) else {
        return;
    };
    let mut paint = Paint::default();
    paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
    paint.anti_alias = true;
    pixmap.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
}

/// Fill a rounded rectangle with an additional opacity multiplier.
pub fn fill_rounded_rect_alpha(
    pixmap: &mut Pixmap,
    rect: Rect,
    color: Color,
    radius: f32,
    opacity: f32,
) {
    let a = (color[3] as f32 * opacity.clamp(0.0, 1.0)) as u8;
    fill_rounded_rect(pixmap, rect, [color[0], color[1], color[2], a], radius);
}

/// Draw a filled circle.
pub fn fill_circle(pixmap: &mut Pixmap, center: Point, radius: f32, color: Color) {
    let Some(path) = circle_path(center, radius) else {
        return;
    };
    let mut paint = Paint::default();
    paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
    paint.anti_alias = true;
    pixmap.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
}

/// Stroke a line between two points.
pub fn draw_line(pixmap: &mut Pixmap, from: Point, to: Point, color: Color, width: f32) {
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
    pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
}

// ── Image drawing ─────────────────────────────────────────────────────────────

/// Draw an RGBA image scaled to fill `dest`, with an optional opacity multiplier.
pub fn draw_image(pixmap: &mut Pixmap, image: &image::RgbaImage, dest: Rect, opacity: f32) {
    let (w, h) = image.dimensions();
    if w == 0 || h == 0 {
        return;
    }
    let Some(mut src) = tiny_skia::Pixmap::new(w, h) else {
        return;
    };
    // Convert straight-alpha RGBA → premultiplied alpha.
    let data = src.data_mut();
    for (i, pixel) in image.pixels().enumerate() {
        let [r, g, b, a] = pixel.0;
        let af = a as f32 / 255.0;
        let idx = i * 4;
        data[idx] = (r as f32 * af + 0.5) as u8;
        data[idx + 1] = (g as f32 * af + 0.5) as u8;
        data[idx + 2] = (b as f32 * af + 0.5) as u8;
        data[idx + 3] = a;
    }
    let sx = dest.width / w as f32;
    let sy = dest.height / h as f32;
    let paint = PixmapPaint {
        opacity: opacity.clamp(0.0, 1.0),
        ..Default::default()
    };
    pixmap.draw_pixmap(
        dest.x as i32,
        dest.y as i32,
        src.as_ref(),
        &paint,
        Transform::from_scale(sx, sy),
        None,
    );
}

// ── Text drawing ──────────────────────────────────────────────────────────────

/// Draw left-aligned text, vertically centred within `rect`.  Returns rendered width.
pub fn draw_text(
    pixmap: &mut Pixmap,
    text: &str,
    rect: Rect,
    font_family: &str,
    font_size: f32,
    color: Color,
) -> f32 {
    with_font(|fs, sc| {
        draw_text_impl(
            pixmap,
            text,
            rect,
            font_family,
            font_size,
            color,
            TextAlign::Left,
            fs,
            sc,
        )
    })
}

/// Draw horizontally and vertically centred text.  Returns rendered width.
pub fn draw_text_centered(
    pixmap: &mut Pixmap,
    text: &str,
    rect: Rect,
    font_family: &str,
    font_size: f32,
    color: Color,
) -> f32 {
    with_font(|fs, sc| {
        draw_text_impl(
            pixmap,
            text,
            rect,
            font_family,
            font_size,
            color,
            TextAlign::Center,
            fs,
            sc,
        )
    })
}

/// Draw text truncated with '…' if it overflows `rect.width`.  Returns rendered width.
pub fn draw_text_ellipsis(
    pixmap: &mut Pixmap,
    text: &str,
    rect: Rect,
    font_family: &str,
    font_size: f32,
    color: Color,
) -> f32 {
    let ew = estimate_text_width("\u{2026}", font_size);
    let est = estimate_text_width(text, font_size);
    if est <= rect.width {
        return draw_text(pixmap, text, rect, font_family, font_size, color);
    }
    let available = rect.width - ew;
    if available <= 0.0 {
        return draw_text(pixmap, "\u{2026}", rect, font_family, font_size, color);
    }
    let chars: Vec<char> = text.chars().collect();
    let mut take = chars.len();
    while take > 0 {
        let w = estimate_text_width(&chars[..take].iter().collect::<String>(), font_size);
        if w <= available {
            break;
        }
        take -= 1;
    }
    let s: String = chars[..take].iter().collect::<String>() + "\u{2026}";
    draw_text(pixmap, &s, rect, font_family, font_size, color)
}

/// Estimate rendered width using a character-count heuristic (no layout pass).
///
/// Used by `desired_size()` implementations that cannot call into a Canvas.
/// Formula: `font_size × 0.55 × max(char_count, 1)`.
pub fn estimate_text_width(text: &str, font_size: f32) -> f32 {
    font_size * 0.55 * text.chars().count().max(1) as f32
}

/// Compute an effective font size that fills the available height.
///
/// Scales the height to ~65% to leave breathing room above and below the text.
/// Returns the larger of the height-derived value and the configured size, so
/// the configured size acts as a floor (text is never shrunk by this function).
pub fn effective_font_size(available_height: f32, configured_size: f32) -> f32 {
    let height_derived = (available_height * 0.65).floor();
    height_derived.max(configured_size)
}

// ── Internals ─────────────────────────────────────────────────────────────────

fn with_font<F, R>(f: F) -> R
where
    F: FnOnce(&mut FontSystem, &mut SwashCache) -> R,
{
    FONT_SYSTEM.with(|fs| SWASH_CACHE.with(|sc| f(&mut fs.borrow_mut(), &mut sc.borrow_mut())))
}

#[allow(clippy::too_many_arguments)]
fn draw_text_impl(
    pixmap: &mut Pixmap,
    text: &str,
    rect: Rect,
    font_family: &str,
    font_size: f32,
    color: Color,
    align: TextAlign,
    fs: &mut FontSystem,
    sc: &mut SwashCache,
) -> f32 {
    let line_height = (font_size * 1.2).ceil();
    let metrics = Metrics::new(font_size, line_height);
    let mut buf = Buffer::new(fs, metrics);
    buf.set_size(fs, Some(rect.width), Some(rect.height));
    let attrs = Attrs::new().family(Family::Name(font_family));
    buf.set_text(fs, text, attrs, Shaping::Advanced);
    buf.shape_until_scroll(fs, false);

    // Measure text width for horizontal alignment.
    let text_width: f32 = buf.layout_runs().map(|r| r.line_w).fold(0.0f32, f32::max);

    let x_off = match align {
        TextAlign::Left => rect.x,
        TextAlign::Center => rect.x + (rect.width - text_width) / 2.0,
    };
    // Vertical centering base: the top of the line_height box centred in the rect.
    // The actual per-run baseline is y_off_base + run.line_y. cosmic-text sets
    // run.line_y = centering_offset + max_ascent so that
    // glyph_y = (y_off_base + run.line_y) - placement.top centres the text in the rect.
    let y_off_base = rect.y + (rect.height - line_height) / 2.0;

    let pw = pixmap.width() as i32;
    let ph = pixmap.height() as i32;

    for run in buf.layout_runs() {
        let y_off = y_off_base + run.line_y;
        for glyph in run.glyphs.iter() {
            let phys = glyph.physical((x_off, y_off), 1.0);
            let Some(img) = sc.get_image(fs, phys.cache_key) else {
                continue;
            };
            let gx = phys.x + img.placement.left;
            let gy = phys.y - img.placement.top;
            let iw = img.placement.width as i32;
            let ih = img.placement.height as i32;

            match img.content {
                SwashContent::Mask => {
                    let data = pixmap.data_mut();
                    let stride = pw as usize * 4;
                    for row in 0..ih {
                        for col in 0..iw {
                            let px = gx + col;
                            let py = gy + row;
                            if px < 0 || py < 0 || px >= pw || py >= ph {
                                continue;
                            }
                            let alpha = img.data[(row * iw + col) as usize];
                            if alpha == 0 {
                                continue;
                            }
                            alpha_blend(data, stride, px as usize, py as usize, color, alpha);
                        }
                    }
                }
                SwashContent::Color => {
                    let data = pixmap.data_mut();
                    let stride = pw as usize * 4;
                    for row in 0..ih {
                        for col in 0..iw {
                            let px = gx + col;
                            let py = gy + row;
                            if px < 0 || py < 0 || px >= pw || py >= ph {
                                continue;
                            }
                            let si = (row * iw + col) as usize * 4;
                            let src = [
                                img.data[si],
                                img.data[si + 1],
                                img.data[si + 2],
                                img.data[si + 3],
                            ];
                            alpha_blend(data, stride, px as usize, py as usize, src, src[3]);
                        }
                    }
                }
                SwashContent::SubpixelMask => {
                    let data = pixmap.data_mut();
                    let stride = pw as usize * 4;
                    for row in 0..ih {
                        for col in 0..iw {
                            let px = gx + col;
                            let py = gy + row;
                            if px < 0 || py < 0 || px >= pw || py >= ph {
                                continue;
                            }
                            let si = (row * iw + col) as usize * 3;
                            let r = img.data[si] as u16;
                            let g = img.data[si + 1] as u16;
                            let b = img.data[si + 2] as u16;
                            let alpha = ((r + g + b) / 3) as u8;
                            if alpha == 0 {
                                continue;
                            }
                            alpha_blend(data, stride, px as usize, py as usize, color, alpha);
                        }
                    }
                }
            }
        }
    }
    text_width
}

/// Porter-Duff "over" composite on a single pixel (premultiplied destination).
fn alpha_blend(
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
    let sr = color[0] as f32 / 255.0 * src_a;
    let sg = color[1] as f32 / 255.0 * src_a;
    let sb = color[2] as f32 / 255.0 * src_a;
    let dr = data[idx] as f32 / 255.0;
    let dg = data[idx + 1] as f32 / 255.0;
    let db = data[idx + 2] as f32 / 255.0;
    let da = data[idx + 3] as f32 / 255.0;
    let inv = 1.0 - src_a;
    data[idx] = ((sr + dr * inv) * 255.0 + 0.5) as u8;
    data[idx + 1] = ((sg + dg * inv) * 255.0 + 0.5) as u8;
    data[idx + 2] = ((sb + db * inv) * 255.0 + 0.5) as u8;
    data[idx + 3] = ((src_a + da * inv) * 255.0 + 0.5) as u8;
}

fn rounded_rect_path(rect: &Rect, radius: f32) -> Option<tiny_skia::Path> {
    let r = radius.min(rect.width / 2.0).min(rect.height / 2.0).max(0.0);
    let (x, y, w, h) = (rect.x, rect.y, rect.width, rect.height);
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

fn circle_path(center: Point, radius: f32) -> Option<tiny_skia::Path> {
    const K: f32 = 0.552_284_8; // Bézier approximation of a quarter-circle
    let (r, k, cx, cy) = (radius, radius * K, center.x, center.y);
    let mut pb = PathBuilder::new();
    pb.move_to(cx + r, cy);
    pb.cubic_to(cx + r, cy + k, cx + k, cy + r, cx, cy + r);
    pb.cubic_to(cx - k, cy + r, cx - r, cy + k, cx - r, cy);
    pb.cubic_to(cx - r, cy - k, cx - k, cy - r, cx, cy - r);
    pb.cubic_to(cx + k, cy - r, cx + r, cy - k, cx + r, cy);
    pb.close();
    pb.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_text_width_is_positive() {
        assert!(estimate_text_width("hello", 14.0) > 0.0);
    }

    #[test]
    fn estimate_text_width_scales_with_font_size() {
        let w1 = estimate_text_width("hi", 14.0);
        let w2 = estimate_text_width("hi", 28.0);
        assert!((w2 - w1 * 2.0).abs() < 0.01);
    }

    #[test]
    fn effective_font_size_scales_with_height() {
        // 32px bar → floor(32 * 0.65) = 20, which exceeds configured 14.
        assert_eq!(effective_font_size(32.0, 14.0), 20.0);
    }

    #[test]
    fn effective_font_size_respects_floor() {
        // Configured size acts as a floor: never shrink text.
        assert_eq!(effective_font_size(10.0, 14.0), 14.0);
    }

    #[test]
    fn effective_font_size_uses_height_when_larger() {
        // 40px bar → floor(40 * 0.65) = 26.
        assert_eq!(effective_font_size(40.0, 14.0), 26.0);
    }

    #[test]
    fn fill_rect_does_not_panic_on_zero_size() {
        let mut pm = tiny_skia::Pixmap::new(1, 1).unwrap();
        // Empty rect — should be a no-op.
        fill_rect(&mut pm, Rect::new(0.0, 0.0, 0.0, 0.0), [255, 0, 0, 255]);
    }

    #[test]
    fn fill_rect_paints_pixels() {
        let mut pm = tiny_skia::Pixmap::new(10, 10).unwrap();
        fill_rect(&mut pm, Rect::new(2.0, 2.0, 4.0, 4.0), [255, 0, 0, 255]);
        // Pixel at (3,3) should be non-zero after painting.
        let data = pm.data();
        let idx = (3 * 10 + 3) * 4;
        assert!(data[idx + 3] > 0, "expected painted pixel at (3,3)");
    }
}
