# Contributing to HyprDeck

Thanks for contributing. For changes larger than a small bug fix, open an
issue first so the intended behaviour and scope can be discussed.

## Development setup

HyprDeck is a Rust workspace targeting Linux and Wayland. Install a Rust
toolchain with the `rustfmt` and `clippy` components, plus the Wayland, XKB, and
`pkg-config` development dependencies described in the
[development guide](docs/development.md). The CI MSRV check uses Rust 1.85.0;
use that version or newer when making a change.

```sh
git clone https://github.com/maravexa/hyprdeck
cd hyprdeck
cargo build --workspace
```

For a live run, use a Hyprland session and create the required
`hyprdeck.toml` configuration first. See [development](docs/development.md)
for the environment and smoke-test limitations.

## Workflow

1. Create a focused branch from the current main branch.
2. Keep a change scoped to the relevant crate(s), tests, and documentation.
3. Add or update tests when behaviour that can be tested without a compositor
   changes.
4. Run the quality gates below before opening a pull request.
5. Explain the user-visible change, testing performed, and any Hyprland or
   Wayland conditions reviewers need to reproduce it.

The workspace consists of the `hyprdeck` binary and three libraries:
`hyprdeck-core`, `hyprdeck-modules`, and `hyprdeck-themes`. Keep the core crate
independent of the built-in module and theme crates; the binary supplies the
module factory and depends on all three.

## Required checks

Run these commands from the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace
cargo test --workspace --all-targets
cargo test --workspace --doc
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
npx --yes markdownlint-cli2@0.23.2 "**/*.md" "#target"
```

Use `cargo fmt --all` to apply formatting after the check reports differences.
The CI pipeline also builds the complete workspace in release mode and checks
local documentation links with Lychee.

## License

By contributing, you agree that your contributions are licensed under the MIT
license; see [LICENSE](LICENSE).
