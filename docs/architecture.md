# Architecture

HyprDeck is one Wayland client process that manages every output known to its
Hyprland IPC state. It is not one process per monitor. An `App` owns a map of
output names to `OutputState`; each output can host the panel definitions from
the active theme.

## Workspace dependencies

```text
hyprdeck (binary)
├──> hyprdeck-core
├──> hyprdeck-modules ───> hyprdeck-core
└──> hyprdeck-themes  ───> hyprdeck-core
```

`hyprdeck-core` is the base crate. `hyprdeck-modules` and `hyprdeck-themes` are
sibling dependents of it; neither depends on the other. The `hyprdeck` binary
depends on all three, injects `hyprdeck_modules::create_module` into `App`, and
owns the Smithay Client Toolkit/Wayland callbacks.

## Startup flow

1. The binary initializes tracing, resolves the canonical configuration path,
   loads `Config`, and loads the selected `ThemeDefinition`.
2. It connects to Hyprland IPC, hydrates a shared `HyprState`, and starts the
   event listener.
3. It connects to the Wayland display, binds compositor, shared-memory,
   layer-shell, output, and seat globals, then discovers advertised outputs.
4. For every monitor in the hydrated Hyprland state, `App::add_output` creates
   output state and panels from the theme; the binary creates a layer surface
   for each panel on that output.
5. The compositor's configure callbacks allocate/update panel buffers and
   render the first frame. Module state is then updated and dirty panels are
   rendered.

The configuration is deserialized only at startup. There is no config watcher
or reload path.

## Event, update, and render flow

The Tokio event loop combines four sources: Wayland socket activity, Hyprland
IPC events, a 250 ms module-update fallback tick, and a 16 ms animation tick
while an animation is active. IPC events update the shared Hyprland state
before subscribers receive them. The binary handles monitor add/remove events,
performs an immediate module update for other state changes, and finally
renders every dirty panel before flushing Wayland requests.

`App::tick_modules` constructs an `UpdateContext` per output, including the
output name, so per-monitor modules can use the correct monitor state.
`Panel::frame` calculates layout, resizes its software canvas when necessary,
renders through `tiny-skia`, records module bounds for input, and submits an
SHM buffer to the layer surface. Panels and popup surfaces have independent
dirty state; a popup can redraw without redrawing its parent panel.

Pointer and keyboard callbacks locate the owning panel surface, route input to
the appropriate module or popup, and dispatch returned Hyprland actions via
the command socket. Popups are separate overlay layer surfaces positioned next
to their triggering module.

## Invariants

- Output state is keyed by output name and is added/removed in one `App`.
- Each themed panel definition is instantiated for each discovered output.
- `hyprdeck-core` does not select or construct built-in modules; the injected
  factory preserves that dependency boundary.
- Only dirty panels are rendered. A popup buffer is never attached before its
  first compositor configure event.
- Layout/render code uses each output's geometry, while module updates receive
  the output name needed for monitor-specific Hyprland state.

See [development](development.md) for local build and runtime verification.
