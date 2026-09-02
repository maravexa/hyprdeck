# `hyprdeck-core` instructions

Apply the workspace instructions in `../../AGENTS.md`.

- Own shared application state, configuration, geometry, layout, panel/render contracts, theme schema, actions, and Hyprland IPC types.
- Keep this crate independent of `hyprdeck-modules`, `hyprdeck-themes`, and the binary crate.
- Preserve `PanelModule` as the module boundary: implementations are `Send`, render only inside supplied bounds, and report state changes through the existing update/event contracts.
- Preserve theme parsing in this crate; theme discovery, embedded assets, and filesystem resolution belong in `hyprdeck-themes`.
- Update the relevant documentation and downstream callers when changing exported types, configuration schema, or behavior shared across crates.
