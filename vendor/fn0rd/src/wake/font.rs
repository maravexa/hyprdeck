//! Minimal block font used by `fnord wake`.
//!
//! Each glyph is a 5-column × 5-row bitmap authored as a `[&'static str; 5]`
//! where `'#'` marks a filled cell and everything else is empty. The
//! `render` function turns a string into a vector of rendered rows, one
//! per row of the chosen font. Standard renders as 5 rows of `█`; banner
//! stretches to 7 rows of `▓` by duplicating the middle rows.

/// A 5-column × 5-row glyph bitmap authored as strings for readability.
pub type Glyph = [&'static str; 5];

/// Font style selectable by the `--font` flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontStyle {
    /// 5-row block characters rendered with `█` (or `#` in ASCII mode).
    Standard,
    /// 7-row taller characters rendered with `▓` (or `#` in ASCII mode).
    Banner,
}

impl FontStyle {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "standard" | "doom" | "block" => FontStyle::Standard,
            "banner" | "smush" | "tall" => FontStyle::Banner,
            _ => FontStyle::Standard,
        }
    }

    pub fn height(&self) -> usize {
        match self {
            FontStyle::Standard => 5,
            FontStyle::Banner => 7,
        }
    }
}

const SPACE: Glyph = ["     ", "     ", "     ", "     ", "     "];

const A: Glyph = [" ### ", "#   #", "#####", "#   #", "#   #"];
const B: Glyph = ["#### ", "#   #", "#### ", "#   #", "#### "];
const C: Glyph = [" ####", "#    ", "#    ", "#    ", " ####"];
const D: Glyph = ["#### ", "#   #", "#   #", "#   #", "#### "];
const E: Glyph = ["#####", "#    ", "#####", "#    ", "#####"];
const F: Glyph = ["#####", "#    ", "#####", "#    ", "#    "];
const G: Glyph = [" ####", "#    ", "#  ##", "#   #", " ### "];
const H: Glyph = ["#   #", "#   #", "#####", "#   #", "#   #"];
const I: Glyph = ["#####", "  #  ", "  #  ", "  #  ", "#####"];
const J: Glyph = ["#####", "    #", "    #", "#   #", " ### "];
const K: Glyph = ["#   #", "#  # ", "###  ", "#  # ", "#   #"];
const L: Glyph = ["#    ", "#    ", "#    ", "#    ", "#####"];
const M: Glyph = ["#   #", "## ##", "# # #", "#   #", "#   #"];
const N: Glyph = ["#   #", "##  #", "# # #", "#  ##", "#   #"];
const O: Glyph = [" ### ", "#   #", "#   #", "#   #", " ### "];
const P: Glyph = ["#### ", "#   #", "#### ", "#    ", "#    "];
const Q: Glyph = [" ### ", "#   #", "#   #", "#  # ", " ## #"];
const R: Glyph = ["#### ", "#   #", "#### ", "#  # ", "#   #"];
const S: Glyph = [" ####", "#    ", " ### ", "    #", "#### "];
const T: Glyph = ["#####", "  #  ", "  #  ", "  #  ", "  #  "];
const U: Glyph = ["#   #", "#   #", "#   #", "#   #", " ### "];
const V: Glyph = ["#   #", "#   #", "#   #", " # # ", "  #  "];
const W: Glyph = ["#   #", "#   #", "# # #", "# # #", " # # "];
const X: Glyph = ["#   #", " # # ", "  #  ", " # # ", "#   #"];
const Y: Glyph = ["#   #", " # # ", "  #  ", "  #  ", "  #  "];
const Z: Glyph = ["#####", "   # ", "  #  ", " #   ", "#####"];

const D0: Glyph = [" ### ", "#  ##", "# # #", "##  #", " ### "];
const D1: Glyph = ["  #  ", " ##  ", "  #  ", "  #  ", " ### "];
const D2: Glyph = [" ### ", "#   #", "   # ", "  #  ", "#####"];
const D3: Glyph = ["#### ", "    #", " ### ", "    #", "#### "];
const D4: Glyph = ["#  # ", "#  # ", "#####", "   # ", "   # "];
const D5: Glyph = ["#####", "#    ", "#### ", "    #", "#### "];
const D6: Glyph = [" ### ", "#    ", "#### ", "#   #", " ### "];
const D7: Glyph = ["#####", "    #", "   # ", "  #  ", " #   "];
const D8: Glyph = [" ### ", "#   #", " ### ", "#   #", " ### "];
const D9: Glyph = [" ### ", "#   #", " ####", "    #", " ### "];

const COMMA: Glyph = ["     ", "     ", "     ", "  #  ", " #   "];
const APOS: Glyph = ["  #  ", "  #  ", "     ", "     ", "     "];
const PERIOD: Glyph = ["     ", "     ", "     ", "     ", "  #  "];
const HYPHEN: Glyph = ["     ", "     ", " ### ", "     ", "     "];
const COLON: Glyph = ["     ", "  #  ", "     ", "  #  ", "     "];

/// Look up the 5x5 glyph for a character, converting to uppercase first.
/// Unknown characters fall back to a blank space.
pub fn glyph_for(c: char) -> Glyph {
    let c = c.to_ascii_uppercase();
    match c {
        ' ' => SPACE,
        'A' => A,
        'B' => B,
        'C' => C,
        'D' => D,
        'E' => E,
        'F' => F,
        'G' => G,
        'H' => H,
        'I' => I,
        'J' => J,
        'K' => K,
        'L' => L,
        'M' => M,
        'N' => N,
        'O' => O,
        'P' => P,
        'Q' => Q,
        'R' => R,
        'S' => S,
        'T' => T,
        'U' => U,
        'V' => V,
        'W' => W,
        'X' => X,
        'Y' => Y,
        'Z' => Z,
        '0' => D0,
        '1' => D1,
        '2' => D2,
        '3' => D3,
        '4' => D4,
        '5' => D5,
        '6' => D6,
        '7' => D7,
        '8' => D8,
        '9' => D9,
        ',' => COMMA,
        '\'' => APOS,
        '.' => PERIOD,
        '-' => HYPHEN,
        ':' => COLON,
        _ => SPACE,
    }
}

/// Render a string of text into a vector of rows, one row per font line.
pub fn render(text: &str, style: FontStyle, no_unicode: bool) -> Vec<String> {
    let height = style.height();
    let fill_char: &str = if no_unicode {
        "#"
    } else {
        match style {
            FontStyle::Standard => "█",
            FontStyle::Banner => "▓",
        }
    };
    let empty_char = " ";

    // Pre-expand each character into the rows of its (possibly stretched)
    // glyph, with a one-column space separator between characters.
    let chars: Vec<char> = text.chars().collect();
    let mut rows: Vec<String> = vec![String::new(); height];

    for (i, c) in chars.iter().enumerate() {
        let glyph = glyph_for(*c);
        let glyph_rows = stretch_glyph(&glyph, height);
        for (r, grow) in rows.iter_mut().zip(glyph_rows.iter()) {
            for ch in grow.chars() {
                if ch == '#' {
                    r.push_str(fill_char);
                } else {
                    r.push_str(empty_char);
                }
            }
            if i + 1 < chars.len() {
                r.push_str(empty_char);
            }
        }
    }

    rows
}

/// Expand a 5-row glyph to `height` rows by duplicating the middle rows.
/// For `height == 5` this is a no-op (returns the original rows).
/// For `height == 7` the mapping is [0, 1, 1, 2, 3, 3, 4].
fn stretch_glyph(glyph: &Glyph, height: usize) -> Vec<&str> {
    match height {
        5 => glyph.to_vec(),
        7 => vec![
            glyph[0], glyph[1], glyph[1], glyph[2], glyph[3], glyph[3], glyph[4],
        ],
        _ => {
            // Nearest-neighbor scale for any other target height.
            let mut out = Vec::with_capacity(height);
            for i in 0..height {
                let src = (i * 5) / height;
                out.push(glyph[src.min(4)]);
            }
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_all_digits_without_panic() {
        for c in "0123456789".chars() {
            let rows = render(&c.to_string(), FontStyle::Standard, false);
            assert_eq!(rows.len(), 5);
        }
    }

    #[test]
    fn standard_height_is_five() {
        let rows = render("HELLO", FontStyle::Standard, false);
        assert_eq!(rows.len(), 5);
    }

    #[test]
    fn banner_height_is_seven() {
        let rows = render("HELLO", FontStyle::Banner, false);
        assert_eq!(rows.len(), 7);
    }

    #[test]
    fn ascii_fallback_uses_hash_not_block() {
        let rows = render("A", FontStyle::Standard, true);
        let joined = rows.join("\n");
        assert!(joined.contains('#'));
        assert!(!joined.contains('█'));
    }

    #[test]
    fn unicode_mode_uses_block_char() {
        let rows = render("A", FontStyle::Standard, false);
        let joined = rows.join("\n");
        assert!(joined.contains('█'));
        assert!(!joined.contains('#'));
    }

    #[test]
    fn single_char_has_five_columns() {
        let rows = render("A", FontStyle::Standard, true);
        for r in rows {
            assert_eq!(r.chars().count(), 5, "row should be exactly 5 cols: {r:?}");
        }
    }

    #[test]
    fn two_chars_have_correct_width() {
        // 5 + 1 separator + 5 = 11 columns
        let rows = render("AB", FontStyle::Standard, true);
        for r in rows {
            assert_eq!(r.chars().count(), 11, "row should be 11 cols: {r:?}");
        }
    }

    #[test]
    fn unknown_char_renders_as_space() {
        let rows = render("@", FontStyle::Standard, true);
        for r in rows {
            assert!(r.chars().all(|c| c == ' '), "expected blank row: {r:?}");
        }
    }

    #[test]
    fn font_style_parse() {
        assert_eq!(FontStyle::parse("standard"), FontStyle::Standard);
        assert_eq!(FontStyle::parse("doom"), FontStyle::Standard);
        assert_eq!(FontStyle::parse("banner"), FontStyle::Banner);
        assert_eq!(FontStyle::parse("smush"), FontStyle::Banner);
        assert_eq!(FontStyle::parse("unknown"), FontStyle::Standard);
    }
}
