# ADR: Use the unified zbus 6 notification API

- Status: accepted
- Date: 2026-11-16

## Context

The pinned zbus 5 family includes the unmaintained standalone `zvariant`,
`zbus_names` and `zvariant_derive` crates (RUSTSEC-2026-0370, RUSTSEC-2026-0371
and RUSTSEC-2026-0372). Updating the derive crate alone leaves the maintenance
notices and a mixed utility family.
The [upstream migration guide](https://z-galaxy.github.io/zbus/upgrading-to-6.html)
identifies zbus 6 as the unified service, wire-format, names and derive API.

## Decision

Use zbus 6.3.0, whose Rust 1.89 minimum matches this workspace. Remove the direct
standalone wire/name/derive dependencies and consume `OwnedVariant` from zbus.
Keep builder error propagation at `build().await`, where zbus 6 reports errors.
Explicitly disable replacement in both directions: startup must fail when another
notification daemon owns the name, even if that daemon permits replacement.
Retain Tokio execution, service/client APIs, tracing and the previously supported
`unixexec` transport explicitly; do not add the separate default runtime or the
new `ibus` transport.

Validate the production bridge on a private session bus with both a zbus client
and an independent GLib client, including compound hints, replacement, close,
malformed signatures and refusal to take an occupied service name. Run the
workspace tests on Rust 1.89 and the development compiler, and retain advisory
checks without suppression rules.

## Consequences

The obsolete standalone crates leave the locked dependency graph. A fresh audit
also required the minimal Quinn protocol/UDP and rustix patch updates in the
lockfile (RUSTSEC-2026-0523, RUSTSEC-2026-0524, RUSTSEC-2026-0538 and
RUSTSEC-2026-0486). The unrelated ttf-parser maintenance warning remains visible;
no advisory is suppressed. Source MSRV, the public notification
wire interface, advertised capabilities and rendering limitations remain the
same. Real-bus testing does not establish live compositor, notification visuals,
action-button or physical-device acceptance.

## Supersedes

None.
