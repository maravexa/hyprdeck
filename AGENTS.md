# HyprDeck workspace instructions

## Scope

These instructions apply to the whole workspace. More-specific `AGENTS.md` files add to them.

## Sources of truth

- `Cargo.toml` defines workspace membership, shared dependencies, Rust edition, and package metadata.
- Each crate's `Cargo.toml` defines its dependency boundary.
- `crates/*/src/` defines runtime behavior and public Rust contracts.
- `themes/*/theme.toml` defines shipped theme data; `crates/hyprdeck-themes/src/defaults.rs` embeds it.
- `.github/workflows/ci.yml` defines required CI checks; `.github/workflows/security.yml` defines the dependency audit.
- The documentation index at `docs/README.md` identifies the maintained project documents.

## Change rules

- Keep dependencies flowing from `hyprdeck` to the library crates, from `hyprdeck-modules` and `hyprdeck-themes` to `hyprdeck-core`, and never in reverse.
- Keep public behavior, configuration, module IDs, and theme data consistent with their defining source and tests.
- Update the relevant maintained documentation when changing a public interface, user configuration, shipped theme/module behavior, supported workflow, or known limitation.
- Test Wayland or Hyprland behavior in a live Hyprland Wayland session when runtime validation is needed; compilation and unit tests do not validate compositor integration.

## Quality commands

Run the narrowest relevant commands first, then use the CI-equivalent commands appropriate to the change:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace
cargo test --workspace --all-targets
cargo test --workspace --doc
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo build --workspace --release
cargo +1.85.0 check --workspace
cargo audit
npx --yes markdownlint-cli2@0.23.2 "**/*.md" "#target"
lychee --offline --include-fragments --root-dir "$PWD" './**/*.md'
```

Run the MSRV command only when Rust 1.85.0 is installed through rustup; CI is
the authoritative MSRV gate. Run `cargo audit` and `lychee` where those tools
are available. CI runs the Markdown linter and local-link checker for
documentation changes.
