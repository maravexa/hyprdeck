//! Freedesktop application-icon discovery shared across modules.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Look up a freedesktop icon by name at `size` pixels and load it as RGBA.
pub fn load_freedesktop_icon(name: &str, size: u16) -> Option<image::RgbaImage> {
    let path = freedesktop_icons::lookup(name).with_size(size).find()?;
    load_icon_from_path_at_size(&path, size)
}

/// Load and decode an icon from an arbitrary file path.
pub fn load_icon_from_path(path: &Path) -> Option<image::RgbaImage> {
    load_icon_from_path_at_size(path, 64)
}

fn load_icon_from_path_at_size(path: &Path, size: u16) -> Option<image::RgbaImage> {
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"))
    {
        return load_svg(path, size);
    }

    match image::open(path) {
        Ok(image) => Some(image.to_rgba8()),
        Err(error) => {
            tracing::debug!(?path, %error, "could not decode application icon");
            None
        }
    }
}

fn load_svg(path: &Path, size: u16) -> Option<image::RgbaImage> {
    let data = std::fs::read(path).ok()?;
    let mut options = resvg::usvg::Options {
        resources_dir: path.parent().map(Path::to_owned),
        ..resvg::usvg::Options::default()
    };
    options.fontdb_mut().load_system_fonts();
    let tree = resvg::usvg::Tree::from_data(&data, &options).ok()?;
    let target = u32::from(size.max(1));
    let source = tree.size();
    let scale = (target as f32 / source.width()).min(target as f32 / source.height());
    let mut pixmap = resvg::tiny_skia::Pixmap::new(target, target)?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );

    let mut rgba = image::RgbaImage::new(target, target);
    for (source, destination) in pixmap.data().chunks_exact(4).zip(rgba.pixels_mut()) {
        let alpha = source[3];
        let unpremultiply = |channel: u8| {
            if alpha == 0 {
                0
            } else {
                ((u16::from(channel) * 255 + u16::from(alpha) / 2) / u16::from(alpha)).min(255)
                    as u8
            }
        };
        *destination = image::Rgba([
            unpremultiply(source[0]),
            unpremultiply(source[1]),
            unpremultiply(source[2]),
            alpha,
        ]);
    }
    Some(rgba)
}

/// Resolve a window class through icon-theme names and installed desktop files.
pub fn load_window_icon(class: &str, size: u16) -> Option<image::RgbaImage> {
    for candidate in class_icon_candidates(class) {
        if let Some(icon) = load_icon_name_or_path(&candidate, size) {
            return Some(icon);
        }
    }

    let wanted = normalize(class);
    desktop_icons()
        .iter()
        .filter(|entry| entry.matches(&wanted))
        .find_map(|entry| load_icon_name_or_path(&entry.icon, size))
}

fn load_icon_name_or_path(value: &str, size: u16) -> Option<image::RgbaImage> {
    let path = Path::new(value);
    if path.is_absolute() {
        load_icon_from_path_at_size(path, size)
    } else {
        load_freedesktop_icon(value, size)
    }
}

fn class_icon_candidates(class: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    let mut add = |candidate: String| {
        if !candidate.is_empty() && seen.insert(candidate.clone()) {
            candidates.push(candidate);
        }
    };

    add(class.trim().to_owned());
    let lower = class.trim().to_lowercase();
    add(lower.clone());
    add(lower.replace('.', "-"));
    add(lower.replace('_', "-"));
    if let Some(last) = lower.rsplit('.').next() {
        add(last.to_owned());
    }
    candidates
}

#[derive(Debug)]
struct DesktopIcon {
    desktop_id: String,
    name: String,
    startup_wm_class: String,
    executable: String,
    icon: String,
}

impl DesktopIcon {
    fn matches(&self, wanted: &str) -> bool {
        [
            &self.desktop_id,
            &self.name,
            &self.startup_wm_class,
            &self.executable,
            &self.icon,
        ]
        .iter()
        .map(|candidate| normalize(candidate))
        .any(|candidate| {
            candidate == wanted
                || (!wanted.is_empty()
                    && candidate.len() > wanted.len()
                    && candidate.ends_with(wanted))
        })
    }
}

fn desktop_icons() -> &'static [DesktopIcon] {
    static ICONS: OnceLock<Vec<DesktopIcon>> = OnceLock::new();
    ICONS.get_or_init(discover_desktop_icons)
}

fn discover_desktop_icons() -> Vec<DesktopIcon> {
    let mut roots = Vec::new();
    if let Some(data_home) = std::env::var_os("XDG_DATA_HOME").filter(|value| !value.is_empty()) {
        roots.push(PathBuf::from(data_home).join("applications"));
    } else if let Some(home) = std::env::var_os("HOME").filter(|value| !value.is_empty()) {
        roots.push(PathBuf::from(&home).join(".local/share/applications"));
        roots.push(PathBuf::from(home).join(".local/share/flatpak/exports/share/applications"));
    }
    let data_dirs = std::env::var_os("XDG_DATA_DIRS")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    roots.extend(std::env::split_paths(&data_dirs).map(|directory| directory.join("applications")));
    roots.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));

    let mut paths = Vec::new();
    for root in roots {
        collect_desktop_files(&root, 0, &mut paths);
    }
    paths.sort();
    paths.dedup();
    paths
        .into_iter()
        .filter_map(|path| {
            let source = std::fs::read_to_string(&path).ok()?;
            parse_desktop_icon(&source, path.file_stem()?.to_str()?)
        })
        .collect()
}

fn collect_desktop_files(directory: &Path, depth: usize, output: &mut Vec<PathBuf>) {
    if depth > 3 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_desktop_files(&path, depth + 1, output);
        } else if path.extension().and_then(|value| value.to_str()) == Some("desktop") {
            output.push(path);
        }
    }
}

fn parse_desktop_icon(source: &str, desktop_id: &str) -> Option<DesktopIcon> {
    let mut in_desktop_entry = false;
    let mut name = String::new();
    let mut startup_wm_class = String::new();
    let mut executable = String::new();
    let mut icon = String::new();

    for line in source.lines().map(str::trim) {
        if line.starts_with('[') {
            in_desktop_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_desktop_entry || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "Name" => name = value.trim().to_owned(),
            "StartupWMClass" => startup_wm_class = value.trim().to_owned(),
            "Icon" => icon = value.trim().to_owned(),
            "Exec" => executable = executable_from_exec(value),
            _ => {}
        }
    }

    (!icon.is_empty()).then(|| DesktopIcon {
        desktop_id: desktop_id.to_owned(),
        name,
        startup_wm_class,
        executable,
        icon,
    })
}

fn executable_from_exec(value: &str) -> String {
    value
        .split_whitespace()
        .find(|part| !part.contains('=') && !part.starts_with('%') && *part != "env")
        .and_then(|part| Path::new(part).file_name())
        .and_then(|part| part.to_str())
        .unwrap_or_default()
        .to_owned()
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_nonexistent_icon_returns_none() {
        assert!(load_freedesktop_icon("__hyprdeck_no_such_icon__", 24).is_none());
    }

    #[test]
    fn load_window_icon_nonexistent_returns_none() {
        assert!(load_window_icon("__no_such_app__", 24).is_none());
    }

    #[test]
    fn desktop_entry_maps_startup_class_to_icon() {
        let entry = parse_desktop_icon(
            "[Desktop Entry]\nName=Example Browser\nExec=/usr/bin/example %U\nIcon=example-icon\nStartupWMClass=ExampleBrowser\n",
            "org.example.Browser",
        )
        .unwrap();
        assert!(entry.matches(&normalize("examplebrowser")));
        assert!(entry.matches(&normalize("example")));
        assert_eq!(entry.icon, "example-icon");
    }

    #[test]
    fn icon_candidates_cover_reverse_dns_classes() {
        let candidates = class_icon_candidates("org.example.My_App");
        assert!(candidates.contains(&"org.example.my_app".to_owned()));
        assert!(candidates.contains(&"org-example-my_app".to_owned()));
        assert!(candidates.contains(&"my_app".to_owned()));
    }
}
