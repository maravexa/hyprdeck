# Documentation

This index is the navigation point for maintained project documentation. Source code, Cargo manifests, and CI workflows remain authoritative for implementation, dependency, and automation details.

## Sources of truth

| Subject | Authoritative source |
| --- | --- |
| Workspace members, versions, edition, dependencies | Root and crate `Cargo.toml` files plus `Cargo.lock` |
| Runtime behavior and public Rust contracts | `crates/*/src/` and generated rustdoc |
| Built-in module IDs | `crates/hyprdeck-modules/src/lib.rs` |
| Configuration schema and defaults | `crates/hyprdeck-core/src/config.rs` and module config types |
| Theme schema, loading, and shipped data | `crates/hyprdeck-core/src/theme.rs`, `crates/hyprdeck-themes/src/`, and `themes/` |
| Required automation | `.github/workflows/` |
| User-visible change history | `CHANGELOG.md` |

## Project guides

- [Project overview](../README.md)
- [Contributing](../CONTRIBUTING.md)
- [Architecture](architecture.md)
- [Configuration](configuration.md)
- [Development](development.md)
- [Modules](modules.md)
- [Themes](themes.md)
- [Known limitations](known-limitations.md)
- [HyprCube integration](integrations/hyprcube.md)

## Decisions

- [Architecture decision records](decisions/README.md)

## Generated API documentation

Generate local Rust API documentation with:

```sh
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

The generated output is not checked into this repository.
