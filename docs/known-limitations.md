# Known limitations

## Desktop notifications

HyprDeck's notification daemon currently renders a safe, compact subset of the
Desktop Notifications specification: summary, plain-text body, replacement IDs,
client close requests, and server/default timeouts. It retains action metadata
in its queue but does not yet expose action buttons or emit `ActionInvoked` /
`NotificationClosed` signals. Image hints, inline markup, urgency hints, and
per-application icons are also not rendered yet. Accordingly it advertises only
the `body` and `persistence` capabilities.

- The expanded sound mixer needs a working `pactl` JSON interface (PulseAudio
  or PipeWire's PulseAudio compatibility service). On wpctl-only and ALSA
  systems HyprDeck retains the compact default-output control. Device cycling
  is available from the labels; explicit ports/profiles and stream routing are
  not implemented yet.
- `dodge_active` is accepted by the auto-hide state machine but currently behaves as a visible panel; it does not inspect or avoid the focused window.
- The `auto_hide.edge_trigger` and `dock_hover.hover_margin` settings are parsed but do not create a separate pointer trigger region.
- `module_action` can be parsed as an action but the dispatcher logs and ignores it.
- Configuration and themes reload when `hyprdeck` is launched again; there is
  no automatic filesystem watcher.
- Modules can receive key presses, but key repeat and key release are not delivered.
- Lunar `render_mode = "icons"` is accepted but falls back to the canvas-drawn phase because theme icon sets are not implemented.
- Calendar `show_week_numbers` and `first_day`, clock `secondary_timezone`, favorites `show_running_indicator`, and lunar `locale` are exposed in configuration/schema data but are not consumed by current rendering; lunar labels remain English.
- Themes can parse `menu_button` color overrides, but the current style resolver applies only `window_list` and `workspaces` overrides.
- Theme `separator_color` is resolved, but separators remain disabled; dock `background_radius` and `background_opacity` are parsed but not used by the renderer.
- Wayland scale-factor and output-transform callbacks are present but do not update rendering, so HiDPI, fractional-scale, and transformed-output behavior is not complete.
