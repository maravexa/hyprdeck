use include_dir::{Dir, include_dir};

/// The `themes/` directory is embedded at compile time via `include_dir!`.
///
/// At runtime this provides access to all shipped theme TOML files and assets
/// without requiring the user to have the source tree present.
static THEMES_DIR: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/themes");

/// Return the raw TOML bytes for an embedded theme by directory name
/// (e.g. `"win7"`, `"macos_dock"`), or `None` if not found.
pub fn embedded_theme_toml(name: &str) -> Option<&'static str> {
    THEMES_DIR
        .get_file(format!("{}/theme.toml", name))
        .and_then(|f| f.contents_utf8())
}

/// List the names of all embedded themes.
pub fn embedded_theme_names() -> Vec<&'static str> {
    THEMES_DIR
        .dirs()
        .filter_map(|d| d.path().file_name()?.to_str())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packaged_theme_copies_match_workspace_sources() {
        for name in embedded_theme_names() {
            let source = std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../themes")
                    .join(name)
                    .join("theme.toml"),
            )
            .unwrap_or_else(|error| panic!("could not read source theme {name}: {error}"));
            assert_eq!(
                embedded_theme_toml(name),
                Some(source.as_str()),
                "packaged copy of {name} drifted from themes/{name}/theme.toml"
            );
        }
    }
}
