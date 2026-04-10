//! Freedesktop icon loading shared across module implementations.
//!
//! Only PNG and ICO formats are supported by this build's `image` feature set.
//! SVG paths are silently skipped with a `tracing::debug!` log entry.

use std::path::Path;

/// Look up a freedesktop icon by name at `size` pixels and load it as RGBA.
///
/// Returns `None` if the icon is not found in the current theme hierarchy or
/// if the located file cannot be decoded (e.g. it is an SVG).
pub fn load_freedesktop_icon(name: &str, size: u16) -> Option<image::RgbaImage> {
    let path = freedesktop_icons::lookup(name).with_size(size).find()?;
    load_icon_from_path(&path)
}

/// Load and decode an icon from an arbitrary file path.
///
/// Only PNG and ICO are supported.  SVG files are skipped without error.
pub fn load_icon_from_path(path: &Path) -> Option<image::RgbaImage> {
    if path.extension().and_then(|e| e.to_str()) == Some("svg") {
        tracing::debug!("Skipping SVG icon at {:?} (not supported in this build)", path);
        return None;
    }
    match image::open(path) {
        Ok(img) => Some(img.to_rgba8()),
        Err(e) => {
            tracing::debug!("Failed to decode icon at {:?}: {}", path, e);
            None
        }
    }
}

/// Try multiple name variants to find an icon for a window class string.
///
/// Probe order:
/// 1. Exact `class` string.
/// 2. Lowercase version.
/// 3. Lowercase with dots replaced by hyphens.
pub fn load_window_icon(class: &str, size: u16) -> Option<image::RgbaImage> {
    if let Some(img) = load_freedesktop_icon(class, size) {
        return Some(img);
    }
    let lower = class.to_lowercase();
    if lower != class {
        if let Some(img) = load_freedesktop_icon(&lower, size) {
            return Some(img);
        }
    }
    let alt = lower.replace('.', "-");
    if alt != lower {
        if let Some(img) = load_freedesktop_icon(&alt, size) {
            return Some(img);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_nonexistent_icon_returns_none() {
        let result = load_freedesktop_icon("__hyprdeck_no_such_icon__", 24);
        assert!(result.is_none());
    }

    #[test]
    fn load_window_icon_nonexistent_returns_none() {
        let result = load_window_icon("__no_such_app__", 24);
        assert!(result.is_none());
    }

    #[test]
    fn load_icon_from_path_svg_returns_none() {
        // We don't need a real file — the SVG guard fires on extension alone.
        let result = load_icon_from_path(Path::new("/tmp/fake.svg"));
        assert!(result.is_none());
    }
}
