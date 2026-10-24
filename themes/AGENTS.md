# Shipped theme instructions

Apply the workspace instructions in `../AGENTS.md`.

- Treat each `theme.toml` as shipped source data. Keep its copy under
  `../crates/hyprdeck-themes/themes/` identical; the crate embeds that packaged copy.
- Use only fields defined by `../crates/hyprdeck-core/src/theme.rs` and valid module IDs registered by `hyprdeck-modules`.
- Keep panel edge, dimensions, layout, auto-hide, module groups, dock settings, and style values internally coherent with the schema.
- Keep a theme directory name aligned with its embedded theme lookup name; do not assume runtime assets beyond files supported by the loader.
- Update `../docs/themes.md` for user-visible changes to shipped themes.
