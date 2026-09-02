# `hyprdeck` binary instructions

Apply the workspace instructions in `../../AGENTS.md`.

- Keep this crate as the executable composition and Wayland/SCTK integration layer.
- Preserve its dependency direction: it may use `hyprdeck-core`, `hyprdeck-modules`, and `hyprdeck-themes`; do not move reusable application contracts or module implementations here.
- Keep layer-surface lifecycle, Wayland input routing, output handling, and redraw scheduling coherent with the `hyprdeck-core` `App` and `Panel` contracts.
- Validate changes to Wayland surfaces, input, outputs, or Hyprland IPC in a live Hyprland Wayland session.
- Update user-facing documentation when CLI behavior, runtime setup, or observable integration behavior changes.
