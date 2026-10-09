# Native prepared maintenance

The Python `tos_access.prepared_semantics` and its joined caller-transaction
maintenance API are retired. Native maintenance is owned by
[`tos-compiler::prepared_maintenance`](../rust/crates/tos-compiler/src/prepared_maintenance.rs)
and is reached through the optional `tos prepare --attach-maintenance` route
in [`OFFLINE_PREPARED_BOOTSTRAP.md`](OFFLINE_PREPARED_BOOTSTRAP.md).

The native command keeps catalog and semantic index creation inside the
selected private publication flow. A successful build does not admit source
meaning or select a consumer; publication and source-owner checks remain
separate.
