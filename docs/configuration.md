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

## Module tables

| Module | Fields and defaults |
| --- | --- |
| `calendar` | `system = "gregorian"` (`gregorian`, `discordian`, `custom`); `show_week_numbers = false`; `first_day = "monday"`; optional `date_format` for Gregorian/custom output |
| `clock` | `format = "%H:%M"`; optional `secondary_timezone` |
| `favorites` | `entries = []`; optional `icon_size`; `show_running_indicator = true` |
| `lunar` | `show_label = false`; `body = "luna"`; `locale = "en"`; `render_mode = "canvas"` (`canvas`, `icons`, `emoji`, `ascii`); `display = "icon"` (`icon`, `verbose`) |
| `menu_button` | `label = ""`; `icon = "start-here"`; `action` (see below) |
| `network` | `display = "icon"`; optional `interface`; `poll_secs = 5` |
| `power` | `[modules.power.commands]` with `shutdown`, `reboot`, `logout`, `lock`, `suspend` command strings |
| `shell` | required `command`; `interval_secs = 5`; `max_chars = 64`; `timeout_secs = 5` |
| `sound` | `display = "icon"`; `backend = "auto"` (`auto`, `pipewire`, `pulseaudio`, `alsa`); `poll_interval_ms = 500`; `volume_step = 5` |
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
action = { type = "exec", command = "wofi", args = ["--show", "drun"] }

[modules.power.commands]
lock = "hyprlock"
logout = "hyprctl dispatch exit"
```

Actions are tagged TOML tables: `{ type = "exec", command = "program", args = [] }`, `{ type = "hypr_dispatch", dispatch = "workspace 2" }`, or `{ type = "chain", actions = [...] }`. `module_action` is parsed but currently reserved; see [known limitations](known-limitations.md).

For panel placement, layouts, and style settings, see [Themes](themes.md).
