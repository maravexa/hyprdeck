# `hyprdeck-modules` instructions

Apply the workspace instructions in `../../AGENTS.md`.

- Own built-in `PanelModule` implementations and their registry in `src/lib.rs`.
- Keep each registered module ID stable and keep `create_module`, `builtin_module_ids`, exports, and tests aligned when adding or removing a built-in module.
- Keep module-specific configuration parsing and schemas with their implementation; do not add module-specific behavior to `hyprdeck-core`.
- Do not draw outside the `Rect` supplied by `PanelModule::render`; do not block the panel thread for background work.
- Update module documentation for user-visible module IDs, options, defaults, or behavior changes.
