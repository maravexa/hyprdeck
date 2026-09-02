# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- Panel module actions (workspace switch, favorites launch, menu button, and
  window-list focus) are dispatched instead of being dropped by the binary.
- The `verbose_text_padding` theme key controls the gap between icon and
  readout in verbose display mode, defaulting to `bar_height / 8` when absent.
- The MSRV CI job uses Rust 1.85.0, and Dependabot does not update its pinned
  `dtolnay/rust-toolchain` action.
- The sound-module icon scales to the theme icon size and is centred in its
  slot.
- Adjacent status modules render with configurable horizontal `module_gap`
  spacing; the shipped themes define their own values.

### Added

- Keyboard input support for panel and popup surfaces. Key presses go to the
  module owning an open popup, or the hovered module; Esc closes a popup.
- `lunar` `render_mode` values `emoji` and `ascii`, alongside the default
  canvas renderer. The reserved `icons` value falls back to canvas with a
  warning.
- `icon` and `verbose` display modes for the `lunar`, `sound`, and `network`
  modules. `verbose` uses an icon-and-readout widget, and the three modules
  expose the setting through their configuration schemas.
- The public `hyprdeck_core::DisplayMode` enum and the
  `verbose_text_padding` theme style field.

### Breaking

- The configuration file is `hyprdeck.toml` at
  `$XDG_CONFIG_HOME/hypr/hyprdeck.toml`, or
  `~/.config/hypr/hyprdeck.toml` when `XDG_CONFIG_HOME` is unset. There is no
  automatic migration from `config.toml`.
- A missing configuration file is an error that identifies the checked path.
- `hyprdeck_core::default_config_path()` is the canonical configuration-path
  helper for downstream integrations.
- The network module's `display` setting accepts only `icon` or `verbose`;
  migrate prior `iconlabel` and `iconrate` values to `verbose`.
