//! Rendering primitives for module implementations.
//!
//! Provides shape and text drawing that works on a borrowed `&mut Pixmap`, matching
//! the `PanelModule::render` signature.  Text rendering uses a thread-local
//! `FontSystem` and `SwashCache` so each call doesn't reallocate font state.

use std::cell::RefCell;

use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache, SwashContent};
use hyprdeck_core::{Color, Point, Rect, ThemeContext};
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
    fill_rounded_rect(pixmap, rect, dim_color(color, opacity), radius);
}

/// Multiply a color's alpha channel by `opacity` (clamped to 0.0–1.0).
pub fn dim_color(color: Color, opacity: f32) -> Color {
    let a = (color[3] as f32 * opacity.clamp(0.0, 1.0)) as u8;
    [color[0], color[1], color[2], a]
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

/// Draw a moon phase circle with a terminator curve onto a `Pixmap`.
///
/// `fraction`: 0.0 = new moon (fully dark), 0.5 = full moon (fully lit), 1.0 = new moon again.
/// The moon is centered within `bounds` using the smaller dimension as diameter.
pub fn draw_moon_phase(
    pixmap: &mut Pixmap,
    bounds: hyprdeck_core::Rect,
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

    fill_circle(pixmap, Point::new(cx, cy), radius, dark_color);

    let phase = fraction.rem_euclid(1.0) as f32;
    let terminator_x = if phase <= 0.5 {
        radius * (1.0 - phase * 4.0).max(-1.0)
    } else {
        radius * ((phase - 0.5) * 4.0 - 1.0).min(1.0)
    };

    if let Some(path) = build_moon_lit_path(cx, cy, radius, terminator_x, phase) {
        let mut paint = Paint::default();
        paint.set_color_rgba8(lit_color[0], lit_color[1], lit_color[2], lit_color[3]);
        paint.anti_alias = true;
        pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
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

/// Compute the square content rect shared by all status-icon renderers.
///
/// `padding` is measured from the module slot edge. The returned rect is a
/// centered square, so it also works for an icon half in verbose mode and for
/// vertical panels. Keeping this calculation in one place means a theme's
/// icon padding affects lunar, network, power, and sound identically.
pub fn icon_content_rect(slot: Rect, padding: f32) -> Rect {
    let available = (slot.width.min(slot.height) - padding.max(0.0) * 2.0).max(1.0);
    let x = slot.x + (slot.width - available) / 2.0;
    let y = slot.y + (slot.height - available) / 2.0;
    Rect::new(x, y, available, available)
}

/// Draw a font-independent application-menu grid centered in `bounds`.
///
/// This is used as a dependable fallback when the configured freedesktop icon
/// is absent from the current icon theme.
pub fn draw_menu_icon(pixmap: &mut Pixmap, bounds: Rect, color: Color) {
    let side = bounds.width.min(bounds.height);
    if side <= 0.0 {
        return;
    }

    let cell = (side / 6.0).max(1.0);
    let gap = cell * 0.75;
    let grid = cell * 3.0 + gap * 2.0;
    let start_x = bounds.x + (bounds.width - grid) / 2.0;
    let start_y = bounds.y + (bounds.height - grid) / 2.0;

    for row in 0..3 {
        for column in 0..3 {
            fill_rounded_rect(
                pixmap,
                Rect::new(
                    start_x + column as f32 * (cell + gap),
                    start_y + row as f32 * (cell + gap),
                    cell,
                    cell,
                ),
                color,
                cell * 0.25,
            );
        }
    }
}

/// Draw a font-independent power icon centered in `bounds`.
pub fn draw_power_icon(pixmap: &mut Pixmap, bounds: Rect, color: Color) {
    let side = bounds.width.min(bounds.height);
    if side <= 0.0 {
        return;
    }
    let scale = side / 24.0;
    let center = bounds.center();
    let stroke_width = (2.25 * scale).max(1.0);
    let radius = 7.6 * scale;

    let mut ring = PathBuilder::new();
    ring.move_to(center.x - radius * 0.58, center.y - radius * 0.78);
    ring.cubic_to(
        center.x - radius * 1.05,
        center.y - radius * 0.28,
        center.x - radius * 1.05,
        center.y + radius * 0.65,
        center.x,
        center.y + radius,
    );
    ring.cubic_to(
        center.x + radius * 1.05,
        center.y + radius * 0.65,
        center.x + radius * 1.05,
        center.y - radius * 0.28,
        center.x + radius * 0.58,
        center.y - radius * 0.78,
    );
    if let Some(path) = ring.finish() {
        let mut paint = Paint::default();
        paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
        paint.anti_alias = true;
        pixmap.stroke_path(
            &path,
            &paint,
            &Stroke {
                width: stroke_width,
                line_cap: LineCap::Round,
                ..Stroke::default()
            },
            Transform::identity(),
            None,
        );
    }

    draw_line(
        pixmap,
        Point::new(center.x, center.y - radius * 1.24),
        Point::new(center.x, center.y - radius * 0.08),
        color,
        stroke_width,
    );
}

/// Draw a font-independent speaker icon centered in `bounds`.
///
/// The number of waves communicates low, medium, or high volume. Muted and
/// zero-volume states use a diagonal slash instead of waves.
pub fn draw_speaker_icon(
    pixmap: &mut Pixmap,
    bounds: Rect,
    color: Color,
    volume_percent: u32,
    muted: bool,
) {
    let side = bounds.width.min(bounds.height);
    if side <= 0.0 {
        return;
    }
    let scale = side / 24.0;
    let cx = bounds.x + (bounds.width - 24.0 * scale) / 2.0;
    let cy = bounds.y + (bounds.height - 24.0 * scale) / 2.0;
    let p = |x: f32, y: f32| Point::new(cx + x * scale, cy + y * scale);

    let mut body = PathBuilder::new();
    body.move_to(p(3.0, 9.0).x, p(3.0, 9.0).y);
    body.line_to(p(7.5, 9.0).x, p(7.5, 9.0).y);
    body.line_to(p(13.0, 4.5).x, p(13.0, 4.5).y);
    body.line_to(p(13.0, 19.5).x, p(13.0, 19.5).y);
    body.line_to(p(7.5, 15.0).x, p(7.5, 15.0).y);
    body.line_to(p(3.0, 15.0).x, p(3.0, 15.0).y);
    body.close();
    if let Some(path) = body.finish() {
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

    let stroke_width = (2.0 * scale).max(1.0);
    if muted || volume_percent == 0 {
        draw_line(pixmap, p(4.0, 4.0), p(20.0, 20.0), color, stroke_width);
        return;
    }

    let wave_count = speaker_wave_count(volume_percent);
    draw_speaker_wave(
        pixmap,
        p(15.0, 9.0),
        p(18.5, 12.0),
        p(15.0, 15.0),
        color,
        stroke_width,
    );
    if wave_count >= 2 {
        draw_speaker_wave(
            pixmap,
            p(17.0, 6.0),
            p(22.0, 12.0),
            p(17.0, 18.0),
            color,
            stroke_width,
        );
    }
}

/// Number of speaker waves for a non-muted volume level.
pub fn speaker_wave_count(volume_percent: u32) -> u8 {
    match volume_percent {
        0..=32 => 1,
        _ => 2,
    }
}

fn draw_speaker_wave(
    pixmap: &mut Pixmap,
    start: Point,
    control: Point,
    end: Point,
    color: Color,
    width: f32,
) {
    let mut wave = PathBuilder::new();
    wave.move_to(start.x, start.y);
    wave.quad_to(control.x, control.y, end.x, end.y);
    let Some(path) = wave.finish() else {
        return;
    };
    let mut paint = Paint::default();
    paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
    paint.anti_alias = true;
    pixmap.stroke_path(
        &path,
        &paint,
        &Stroke {
            width,
            line_cap: LineCap::Round,
            ..Stroke::default()
        },
        Transform::identity(),
        None,
    );
}

// ── Verbose display mode ──────────────────────────────────────────────────────

/// Split a verbose-mode slot into `(icon_half, text_half)` with `gap` logical
/// pixels of blank space at the midline (`gap / 2` taken from each half).
///
/// `gap` is clamped to `bounds.width / 2` so tiny slots can never produce
/// negative-width halves.
pub fn verbose_split(bounds: Rect, gap: f32) -> (Rect, Rect) {
    let gap = gap.clamp(0.0, bounds.width / 2.0);
    let (icon_half, text_half) = bounds.split_h(bounds.width / 2.0);
    (
        Rect::new(
            icon_half.x,
            icon_half.y,
            icon_half.width - gap / 2.0,
            icon_half.height,
        ),
        Rect::new(
            text_half.x + gap / 2.0,
            text_half.y,
            text_half.width - gap / 2.0,
            text_half.height,
        ),
    )
}

/// Draw a verbose-mode module: a square icon in the left half and a centered
/// text readout in the right half, separated by the theme's
/// `verbose_text_padding`.
///
/// The icon half is computed via [`verbose_split`] /
/// [`icon_content_rect`] and handed to `draw_icon`;
/// the readout is drawn with the bold family (falling back to the regular
/// family) at [`effective_font_size`].
pub fn draw_verbose(
    canvas: &mut Pixmap,
    bounds: Rect,
    theme: &ThemeContext,
    readout: &str,
    text_color: Color,
    draw_icon: impl FnOnce(&mut Pixmap, Rect),
) {
    let (icon_half, text_half) = verbose_split(bounds, theme.verbose_text_padding);
    let icon_rect = icon_content_rect(icon_half, theme.icon_padding);
    draw_icon(canvas, icon_rect);

    let font = theme
        .fonts
        .bold_family
        .as_deref()
        .unwrap_or(&theme.fonts.family);
    let text_size = effective_font_size(bounds.height, theme.fonts.size);
    draw_text_centered(canvas, readout, text_half, font, text_size, text_color);
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

fn build_moon_lit_path(
    cx: f32,
    cy: f32,
    radius: f32,
    terminator_x: f32,
    phase: f32,
) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    let waxing = phase <= 0.5;
    let segments = 32u32;

    pb.move_to(cx, cy - radius);

    if waxing {
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
    fn icon_content_rect_centers_the_inset_square() {
        let slot = Rect::new(0.0, 0.0, 80.0, 60.0);
        let r = icon_content_rect(slot, 4.0);
        assert_eq!(r.x, 14.0);
        assert_eq!(r.y, 4.0);
        assert_eq!(r.width, 52.0);
        assert_eq!(r.height, 52.0);
    }

    #[test]
    fn icon_content_rect_handles_nonzero_slot_origin() {
        let slot = Rect::new(10.0, 20.0, 80.0, 60.0);
        let r = icon_content_rect(slot, 2.0);
        assert_eq!(r.x, 22.0);
        assert_eq!(r.y, 22.0);
        assert_eq!(r.width, 56.0);
        assert_eq!(r.height, 56.0);
    }

    #[test]
    fn icon_content_rect_uses_shorter_axis_and_theme_padding() {
        let slot = Rect::new(0.0, 0.0, 26.0, 24.0);
        let r = icon_content_rect(slot, 2.0);
        assert_eq!(r.width, 20.0);
        assert_eq!(r.height, 20.0);
        assert_eq!(r.x, 3.0);
        assert_eq!(r.y, 2.0);
    }

    #[test]
    fn icon_content_rect_preserves_a_minimum_size() {
        let slot = Rect::new(0.0, 0.0, 1.0, 1.0);
        let r = icon_content_rect(slot, 8.0);
        assert_eq!(r.width, 1.0);
        assert_eq!(r.height, 1.0);
    }

    #[test]
    fn vector_status_icons_paint_pixels() {
        let mut power = Pixmap::new(32, 32).unwrap();
        draw_power_icon(
            &mut power,
            Rect::new(4.0, 4.0, 24.0, 24.0),
            [255, 255, 255, 255],
        );
        assert!(power.data().chunks_exact(4).any(|px| px[3] > 0));

        let mut speaker = Pixmap::new(32, 32).unwrap();
        draw_speaker_icon(
            &mut speaker,
            Rect::new(4.0, 4.0, 24.0, 24.0),
            [255, 255, 255, 255],
            75,
            false,
        );
        assert!(speaker.data().chunks_exact(4).any(|px| px[3] > 0));

        let mut menu = Pixmap::new(32, 32).unwrap();
        draw_menu_icon(
            &mut menu,
            Rect::new(4.0, 4.0, 24.0, 24.0),
            [255, 255, 255, 255],
        );
        assert!(menu.data().chunks_exact(4).any(|px| px[3] > 0));
    }

    #[test]
    fn speaker_wave_count_is_stable_at_volume_boundaries() {
        assert_eq!(speaker_wave_count(1), 1);
        assert_eq!(speaker_wave_count(32), 1);
        assert_eq!(speaker_wave_count(33), 2);
        assert_eq!(speaker_wave_count(150), 2);
    }

    #[test]
    fn dim_color_scales_alpha_only() {
        assert_eq!(dim_color([10, 20, 30, 200], 0.5), [10, 20, 30, 100]);
    }

    #[test]
    fn dim_color_clamps_opacity() {
        assert_eq!(dim_color([10, 20, 30, 200], 2.0), [10, 20, 30, 200]);
        assert_eq!(dim_color([10, 20, 30, 200], -1.0), [10, 20, 30, 0]);
    }

    #[test]
    fn verbose_split_inserts_gap_at_midline() {
        let bounds = Rect::new(0.0, 0.0, 64.0, 32.0);
        let (icon, text) = verbose_split(bounds, 4.0);
        // Gap between the icon half's right edge and the text half's left edge.
        assert_eq!(text.x - (icon.x + icon.width), 4.0);
        // Halves shrink symmetrically.
        assert_eq!(icon.width, 30.0);
        assert_eq!(text.width, 30.0);
        assert_eq!(icon.height, 32.0);
        assert_eq!(text.height, 32.0);
    }

    #[test]
    fn verbose_split_zero_gap_matches_plain_split() {
        let bounds = Rect::new(10.0, 5.0, 64.0, 32.0);
        let (icon, text) = verbose_split(bounds, 0.0);
        let (pi, pt) = bounds.split_h(bounds.width / 2.0);
        assert_eq!((icon.x, icon.width), (pi.x, pi.width));
        assert_eq!((text.x, text.width), (pt.x, pt.width));
    }

    #[test]
    fn verbose_split_clamps_oversized_gap() {
        let bounds = Rect::new(0.0, 0.0, 8.0, 8.0);
        let (icon, text) = verbose_split(bounds, 100.0);
        assert!(icon.width >= 0.0);
        assert!(text.width >= 0.0);
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
