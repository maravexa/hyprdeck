# `hyprdeck-themes` instructions

Apply the workspace instructions in `../../AGENTS.md`.

- Own theme loading, user-theme precedence, and compile-time embedding of the repository `themes/` directory.
- Keep theme data schema in `hyprdeck-core::ThemeDefinition`; do not duplicate parsing contracts here.
- Preserve load precedence: `$XDG_CONFIG_HOME/hyprdeck/themes/<name>/theme.toml`
  (with the documented `HOME` fallback) is considered before an embedded shipped theme.
- Keep `embedded_theme_names` and `embedded_theme_toml` consistent with the embedded directory structure.
- Update theme documentation when loading behavior, user override location, or shipped-theme availability changes.
