# Themes

The selected `theme` is resolved first from `$XDG_CONFIG_HOME/hyprdeck/themes/<name>/theme.toml`; if `XDG_CONFIG_HOME` is unset, that base is `$HOME/.config`. If no user file exists, HyprDeck uses the embedded shipped theme of that name. Shipped names are `gnome_classic`, `gnome_left`, `macos_dock`, `win7`, and `winxp`.

A theme needs `name`, `description`, and one or more `[[panels]]`:

```toml
name = "Example"
description = "A simple top panel"

[[panels]]
edge = "top"
height = 32
layout = "horizontal"
auto_hide = { mode = "disabled" }
modules_start = ["menu_button", "workspaces"]
modules_center = ["clock"]
modules_end = ["sound", "network"]

[style]
background_color = "#202020ee"
foreground_color = "#f0f0f0"
accent_color = "#4fa3ff"
font_family = "sans-serif"
font_size = 13.0
opacity = 0.9
# Shared inset inside icon-only status slots. Defaults to 2.0 logical px.
icon_padding = 2.0
```

## `ThemeDefinition`

Top-level keys are:

- `name` and `description` (required strings).
- `panels` (required array).
- optional `[style]` (all fields optional): `background_color`, `foreground_color`, `accent_color`, `urgent_color`, `separator_color`, `font_family`, `mono_font_family`, `font_size`, `border_radius`, `opacity`, `verbose_text_padding`, `icon_padding`, and `module_gap`.

Theme colors use `#rrggbb` or `#rrggbbaa`. Missing style values use application defaults. Configuration overrides can replace only the accent color, font family, font size, and opacity.

`icon_padding` is the shared content inset for icon-only status modules such as
`lunar`, `sound`, `network`, and `power`. Their square slot size is derived
from the padded thickness of each panel, so `module_gap` remains solely the
space between adjacent module slots.

Some accepted style fields are not fully wired into rendering; see
[Known limitations](known-limitations.md).

## `PanelDefinition`

Each panel requires `edge` (`top`, `bottom`, `left`, or `right`) and `auto_hide`. `height` applies to top/bottom panels; `width` applies to left/right panels. `layout` defaults to `horizontal` and may be `horizontal`, `vertical`, or `dock`. Module ID arrays `modules_start`, `modules_center`, and `modules_end` default to empty.

Auto-hide values are tagged tables:

```toml
auto_hide = { mode = "disabled" }
# or: { mode = "auto_hide", edge_trigger = true }
# or: { mode = "dodge_active" }
# or: { mode = "dock_hover", hover_margin = 20, hide_delay_ms = 800 }
```

`edge_trigger` and `hover_margin` are accepted by the current schema but do not
yet create a separate pointer trigger region; see [Known limitations](known-limitations.md).

Dock panels require `[panels.dock]` with all of `icon_base_size`, `icon_max_scale`, `magnification_radius`, `animation_speed`, `padding`, `background_radius`, and `background_opacity`.

Per-panel color definitions belong below `panels.module_styles`. The schema accepts `window_list`, `workspaces`, and `menu_button`. `window_list` resolves `active_background`, `active_foreground`, `inactive_background`, and `inactive_foreground`. `workspaces` also accepts `remote_background`, `remote_foreground`, `remote_urgent_background`, and `remote_urgent_foreground`. Remote styles are used for workspaces assigned to another output; the urgent remote style is deliberately muted so multi-monitor panels indicate where attention is needed without duplicating a bright alert. Missing values derive from the panel palette. `menu_button` accepts `background` and `foreground` in TOML, but the current style resolver does not apply that table.

Copy a shipped `themes/<name>/theme.toml` into the user-theme path to customize it. Restart HyprDeck after changes; themes are read at startup only.
