# Configuration

HyprDeck reads one TOML file at `$XDG_CONFIG_HOME/hypr/hyprdeck.toml`. If `XDG_CONFIG_HOME` is unset, it reads `$HOME/.config/hypr/hyprdeck.toml`. The file must set `theme`; missing or invalid module tables fall back to that module's defaults (with a warning for invalid tables).

```toml
theme = "win7"

[theme_overrides]
bar_opacity = 0.9
accent_color = "#ff6a00"
font_family = "Inter"
font_size = 12.0

[modules.clock]
format = "%H:%M"
```

`theme_overrides` accepts only `bar_opacity`, `accent_color`, `font_family`, and `font_size`. Colors accepted by the renderer use `#rrggbb` or `#rrggbbaa`.

Each `[modules.<id>]` table is passed to the module named by `<id>`. A table for a module not placed by the selected theme has no effect. Unknown keys in a valid module table are ignored by serde; use the field names below.

## Desktop notifications

HyprDeck can optionally provide the standard `org.freedesktop.Notifications`
D-Bus service. It is disabled by default so it never competes with an existing
notification daemon. Enable it explicitly after disabling any other daemon:

```toml
[notifications]
enabled = true
anchor = "top_right" # top_left, top_center, top_right, bottom_left, bottom_center, bottom_right
monitor = "focused"  # focused, primary, or a connector name such as "DP-2"
width = 360
margin_x = 16
margin_y = 16
gap = 10
max_visible = 4
default_timeout_ms = 5000

# Optional anchor-relative manual adjustment. Positive values move right/down.
offset_x = 0
offset_y = 0
```

Notifications appear as themed overlay surfaces on the selected output. The
newest notification is closest to the chosen anchor; extra notifications remain
queued until space is available. `primary` currently means the first monitor
reported by Hyprland, because Hyprland does not expose a separate primary flag.
`expire_timeout = 0` from a client stays visible until replaced or closed;
negative timeouts use `default_timeout_ms`.

## Module tables

| Module | Fields and defaults |
| --- | --- |
| `calendar` | `system = "gregorian"` (`gregorian`, `discordian`, `custom`); `show_week_numbers = false`; `first_day = "monday"`; optional `date_format` for Gregorian/custom output |
| `clock` | `format = "%H:%M"`; optional `secondary_timezone` |
| `favorites` | `entries = []`; optional `icon_size`; `show_running_indicator = true` |
| `hyprcube` | `command = "hyprcube"`; `icon = "start-here"`; launches HyprCube directly without a popup |
| `lunar` | `show_label = false`; `body = "luna"`; `locale = "en"`; `render_mode = "canvas"` (`canvas`, `icons`, `emoji`, `ascii`); `display = "icon"` (`icon`, `verbose`) |
| `menu_button` | `label = ">>"`; `icon = ""`; `action` defaults to the command in Hyprland's `$menu` variable, with `wofi --show drun` as a fallback |
| `network` | `display = "icon"`; optional `interface`; `poll_secs = 5` |
| `power` | `[modules.power.commands]` with `shutdown`, `reboot`, `logout`, `lock`, `suspend` command strings |
| `shell` | required `command`; `interval_secs = 5`; `max_chars = 64`; `timeout_secs = 5` |
| `sound` | `display = "icon"`; `backend = "auto"` (`auto`, `pipewire`, `pulseaudio`, `alsa`); `poll_interval_ms = 500`; `volume_step = 5`; `show_pavucontrol = true`; `show_mixer = false`; `show_input = true`; `show_applications = true`; `max_applications = 6` |
| `weather` | optional `location = "lat,lon"`; `unit = "celsius"` (`celsius`, `fahrenheit`); `refresh_minutes = 30` |
| `window_list` | `style = "buttons"` (`buttons`, `icons`, `iconlabel`); `current_workspace_only = true`; `max_button_width = 200.0`; `min_button_width = 60.0` |
| `workspaces` | `show_names = false`; `hide_empty = false`; `highlight_urgent = true` |

Some accepted fields are forward scaffolding and currently have no visible
effect: calendar `show_week_numbers` and `first_day`, clock
`secondary_timezone`, favorites `show_running_indicator`, and lunar `locale`.
See [Known limitations](known-limitations.md).

Examples:

```toml
[modules.weather]
location = "40.7128,-74.0060"
unit = "fahrenheit"

[modules.favorites]
show_running_indicator = true
entries = [
  { label = "Firefox", icon = "firefox", action = { type = "exec", command = "firefox" } },
]

[modules.menu_button]
action = { type = "hyprland_exec", variable = "menu", fallback = "wofi --show drun" }

[modules.power.commands]
lock = "hyprlock"
logout = "hyprctl dispatch exit"
```

The sound popup follows external volume and mute changes while it is open.
Dragging its slider coalesces writes, rather than starting an audio command for
every pointer-motion event. `show_pavucontrol = true` adds a themed launcher
for the system's advanced mixer below the built-in output control.

Set `show_mixer = true` to add themed default-input and per-application
playback controls below the output slider. The mixer uses `pactl -f json` and a
long-lived `pactl subscribe` listener, so it works with PulseAudio and
PipeWire's PulseAudio compatibility server without polling every popup frame.
With `backend = "auto"`, HyprDeck prefers this `pactl` control plane before
falling back to `wpctl` or ALSA.
Click an output or input label to cycle its default device. The expanded
controls gracefully remain absent when only `wpctl` or ALSA is available.
Middle-click an input or application slider to toggle its mute state. Monitor
sources are omitted from the input picker so it lists physical/capture inputs.

Actions are tagged TOML tables: `{ type = "exec", command = "program", args = [] }`, `{ type = "hypr_dispatch", dispatch = "workspace 2" }`, `{ type = "hyprland_exec", variable = "menu", fallback = "wofi --show drun" }`, or `{ type = "chain", actions = [...] }`. `hyprland_exec` reads the last active definition of the named variable from `hyprland.conf` and runs its command through the shell. `module_action` is parsed but currently reserved; see [known limitations](known-limitations.md).

For panel placement, layouts, and style settings, see [Themes](themes.md).
