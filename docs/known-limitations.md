# Known limitations

- Pointer scroll events are logged by the Wayland frontend but are not routed to modules, so sound's scroll-volume handler is not reachable from the current frontend.
- `dodge_active` is accepted by the auto-hide state machine but currently behaves as a visible panel; it does not inspect or avoid the focused window.
- The `auto_hide.edge_trigger` and `dock_hover.hover_margin` settings are parsed but do not create a separate pointer trigger region.
- `module_action` can be parsed as an action but the dispatcher logs and ignores it.
- Configuration and themes are loaded at startup; there is no hot reload.
- Modules can receive key presses, but key repeat and key release are not delivered.
- Lunar `render_mode = "icons"` is accepted but falls back to the canvas-drawn phase because theme icon sets are not implemented.
- Calendar `show_week_numbers` and `first_day`, clock `secondary_timezone`, favorites `show_running_indicator`, and lunar `locale` are exposed in configuration/schema data but are not consumed by current rendering; lunar labels remain English.
- Themes can parse `menu_button` color overrides, but the current style resolver applies only `window_list` and `workspaces` overrides.
- Theme `separator_color` is resolved, but separators remain disabled; dock `background_radius` and `background_opacity` are parsed but not used by the renderer.
- Wayland scale-factor and output-transform callbacks are present but do not update rendering, so HiDPI, fractional-scale, and transformed-output behavior is not complete.
