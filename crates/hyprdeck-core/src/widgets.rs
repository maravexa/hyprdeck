//! Reusable UI widgets for popup content and future HyprCube integration.

use std::cell::RefCell;

use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache, SwashContent};
use tiny_skia::{FillRule, LineCap, Paint, PathBuilder, Pixmap, Stroke, Transform};

use crate::geometry::Rect;
use crate::panel::Color;

thread_local! {
    static FONT_SYSTEM: RefCell<FontSystem> = RefCell::new(FontSystem::new());
    static SWASH_CACHE: RefCell<SwashCache> = RefCell::new(SwashCache::new());
}

/// Configuration for a table grid widget.
#[derive(Debug, Clone)]
pub struct TableConfig {
    /// Number of columns.
    pub columns: usize,
    /// Header labels for each column.
    pub column_headers: Vec<String>,
    /// Optional title displayed above the table.
    pub title: Option<String>,
    /// Width of each cell in pixels.
    pub cell_width: f32,
    /// Height of each cell in pixels.
    pub cell_height: f32,
    /// Height of the title row (if title is set).
    pub title_height: f32,
    /// Height of the column header row.
    pub header_height: f32,
    /// Font size for cell content.
    pub cell_font_size: f32,
    /// Font size for column headers.
    pub header_font_size: f32,
    /// Font size for the title.
    pub title_font_size: f32,
    /// Corner radius for highlight backgrounds.
    pub highlight_radius: f32,
}

impl Default for TableConfig {
    fn default() -> Self {
        Self {
            columns: 7,
            column_headers: Vec::new(),
            title: None,
            cell_width: 32.0,
            cell_height: 26.0,
            title_height: 30.0,
            header_height: 26.0,
            cell_font_size: 13.0,
            header_font_size: 12.0,
            title_font_size: 14.0,
            highlight_radius: 4.0,
        }
    }
}

/// A single cell in the table.
#[derive(Debug, Clone, Default)]
pub struct TableCell {
    /// Text to display in the cell.
    pub text: String,
    /// Cell style.
    pub style: CellStyle,
}

/// Visual style for a table cell.
#[derive(Debug, Clone, Copy, Default)]
pub enum CellStyle {
    /// Normal cell — standard foreground color.
    #[default]
    Normal,
    /// Highlighted cell (e.g., today's date) — accent background, contrasting text.
    Highlighted,
    /// Dimmed cell (e.g., adjacent month days) — reduced opacity.
    Dimmed,
    /// Accent text without background (e.g., holidays, special days).
    Accent,
}

/// Computed dimensions of a table.
#[derive(Debug, Clone)]
pub struct TableSize {
    pub width: f32,
    pub height: f32,
}

/// Compute the total pixel dimensions of a table given its config and row count.
///
/// Useful for `PopupContent::desired_size()` implementations.
pub fn compute_table_size(config: &TableConfig, row_count: usize) -> TableSize {
    let width = config.columns as f32 * config.cell_width;
    let mut height = 0.0;
    if config.title.is_some() {
        height += config.title_height;
    }
    height += config.header_height;
    height += row_count as f32 * config.cell_height;
    TableSize { width, height }
}

/// Draw a table grid into `pixmap` within the given `bounds`.
///
/// The table is centered within `bounds`. `rows` is a slice of rows; each row
/// is a `Vec<TableCell>` with `config.columns` cells (extra cells are ignored,
/// missing cells are blank).
#[allow(clippy::too_many_arguments)]
pub fn draw_table(
    pixmap: &mut Pixmap,
    bounds: Rect,
    config: &TableConfig,
    rows: &[Vec<TableCell>],
    fg: Color,
    bg: Color,
    accent: Color,
    header_color: Color,
    title_color: Color,
    _font_family: &str,
    bold_font_family: &str,
    mono_font_family: &str,
) {
    let table_size = compute_table_size(config, rows.len());
    let start_x = bounds.x + (bounds.width - table_size.width) / 2.0;
    let mut y = bounds.y + (bounds.height - table_size.height) / 2.0;

    // ── Title ──────────────────────────────────────────────────────────────────
    if let Some(title) = &config.title {
        let title_rect = Rect::new(start_x, y, table_size.width, config.title_height);
        draw_text_centered(
            pixmap,
            title,
            title_rect,
            bold_font_family,
            config.title_font_size,
            title_color,
        );
        y += config.title_height;
    }

    // ── Column Headers ─────────────────────────────────────────────────────────
    for (col, header) in config.column_headers.iter().enumerate() {
        if col >= config.columns {
            break;
        }
        let cell_rect = Rect::new(
            start_x + col as f32 * config.cell_width,
            y,
            config.cell_width,
            config.header_height,
        );
        draw_text_centered(
            pixmap,
            header,
            cell_rect,
            bold_font_family,
            config.header_font_size,
            header_color,
        );
    }
    y += config.header_height;

    // ── Separator line under headers ───────────────────────────────────────────
    let line_color = with_opacity(fg, 0.15);
    draw_line(
        pixmap,
        start_x,
        y - 1.0,
        start_x + table_size.width,
        y - 1.0,
        line_color,
        1.0,
    );

    // ── Data Rows ──────────────────────────────────────────────────────────────
    for row in rows {
        for (col, cell) in row.iter().enumerate() {
            if col >= config.columns {
                break;
            }
            if cell.text.is_empty() {
                continue;
            }

            let cell_rect = Rect::new(
                start_x + col as f32 * config.cell_width,
                y,
                config.cell_width,
                config.cell_height,
            );

            match cell.style {
                CellStyle::Highlighted => {
                    let inset = 3.0;
                    let highlight_rect = Rect::new(
                        cell_rect.x + inset,
                        cell_rect.y + inset,
                        cell_rect.width - inset * 2.0,
                        cell_rect.height - inset * 2.0,
                    );
                    fill_rounded_rect(pixmap, highlight_rect, accent, config.highlight_radius);
                    draw_text_centered(
                        pixmap,
                        &cell.text,
                        cell_rect,
                        bold_font_family,
                        config.cell_font_size,
                        bg,
                    );
                }
                CellStyle::Dimmed => {
                    let dimmed = with_opacity(fg, 0.3);
                    draw_text_centered(
                        pixmap,
                        &cell.text,
                        cell_rect,
                        mono_font_family,
                        config.cell_font_size,
                        dimmed,
                    );
                }
                CellStyle::Accent => {
                    draw_text_centered(
                        pixmap,
                        &cell.text,
                        cell_rect,
                        mono_font_family,
                        config.cell_font_size,
                        accent,
                    );
                }
                CellStyle::Normal => {
                    draw_text_centered(
                        pixmap,
                        &cell.text,
                        cell_rect,
                        mono_font_family,
                        config.cell_font_size,
                        fg,
                    );
                }
            }
        }
        y += config.cell_height;
    }
}

// ── Private drawing helpers ────────────────────────────────────────────────────

fn with_opacity(color: Color, opacity: f32) -> Color {
    let a = (color[3] as f32 * opacity.clamp(0.0, 1.0)) as u8;
    [color[0], color[1], color[2], a]
}

fn fill_rounded_rect(pixmap: &mut Pixmap, rect: Rect, color: Color, radius: f32) {
    let mut paint = Paint::default();
    paint.set_color_rgba8(color[0], color[1], color[2], color[3]);

    if radius <= 0.0 {
        let Some(r) = rect.to_skia() else {
            return;
        };
        paint.anti_alias = false;
        pixmap.fill_rect(r, &paint, Transform::identity(), None);
        return;
    }

    let Some(path) = rounded_rect_path(&rect, radius) else {
        return;
    };
    paint.anti_alias = true;
    pixmap.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
}

fn draw_line(pixmap: &mut Pixmap, x1: f32, y1: f32, x2: f32, y2: f32, color: Color, width: f32) {
    let mut pb = PathBuilder::new();
    pb.move_to(x1, y1);
    pb.line_to(x2, y2);
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

fn draw_text_centered(
    pixmap: &mut Pixmap,
    text: &str,
    rect: Rect,
    font_family: &str,
    font_size: f32,
    color: Color,
) {
    FONT_SYSTEM.with(|fs_cell| {
        SWASH_CACHE.with(|sc_cell| {
            let mut fs = fs_cell.borrow_mut();
            let mut sc = sc_cell.borrow_mut();

            let line_height = (font_size * 1.2).ceil();
            let metrics = Metrics::new(font_size, line_height);
            let mut buf = Buffer::new(&mut fs, metrics);
            buf.set_size(&mut fs, Some(rect.width), Some(rect.height));
            let attrs = Attrs::new().family(Family::Name(font_family));
            buf.set_text(&mut fs, text, attrs, Shaping::Advanced);
            buf.shape_until_scroll(&mut fs, false);

            let text_width: f32 = buf.layout_runs().map(|r| r.line_w).fold(0.0f32, f32::max);
            let x_off = rect.x + (rect.width - text_width) / 2.0;
            let y_off_base = rect.y + (rect.height - line_height) / 2.0;

            let pw = pixmap.width() as i32;
            let ph = pixmap.height() as i32;

            for run in buf.layout_runs() {
                let y_off = y_off_base + run.line_y;
                for glyph in run.glyphs.iter() {
                    let phys = glyph.physical((x_off, y_off), 1.0);
                    let Some(img) = sc.get_image(&mut fs, phys.cache_key) else {
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
                                    alpha_blend(
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
                                    alpha_blend(
                                        data,
                                        stride,
                                        px as usize,
                                        py as usize,
                                        src,
                                        src[3],
                                    );
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
                                    alpha_blend(
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
        });
    });
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_table_size_correct() {
        let config = TableConfig {
            columns: 7,
            title: Some("Test".into()),
            cell_width: 32.0,
            cell_height: 26.0,
            title_height: 30.0,
            header_height: 24.0,
            ..Default::default()
        };
        let size = compute_table_size(&config, 5);
        assert_eq!(size.width, 224.0); // 7 * 32
        assert_eq!(size.height, 30.0 + 24.0 + 5.0 * 26.0); // title + header + 5 rows
    }

    #[test]
    fn compute_table_size_no_title() {
        let config = TableConfig {
            columns: 5,
            title: None,
            cell_width: 38.0,
            cell_height: 26.0,
            header_height: 24.0,
            ..Default::default()
        };
        let size = compute_table_size(&config, 15);
        assert_eq!(size.width, 190.0); // 5 * 38
        assert_eq!(size.height, 24.0 + 15.0 * 26.0); // header + 15 rows (no title)
    }
}
