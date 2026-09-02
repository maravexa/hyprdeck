# `hyprdeck-config` instructions

Apply the workspace instructions in `../../AGENTS.md`.

- Own the lightweight, renderer-independent configuration contract shared with
  external editors such as HyprCube.
- Keep this crate free of Wayland, rendering, theme discovery, and module
  implementation dependencies.
- Preserve unknown configuration data when loading and saving, validate before
  replacing an existing file, and keep writes atomic within the destination
  directory.
- Treat serialized schema types and `CONFIG_CONTRACT_VERSION` as a public,
  versioned integration surface.
