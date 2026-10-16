# Native prepared semantic index

The Python `tos_access.semantic_index` implementation and its caller-owned
transaction API are retired. The native compiler owns exact catalog and
semantic-index preparation in
[`prepared_maintenance.rs`](../rust/crates/tos-compiler/src/prepared_maintenance.rs)
and [`prepared_catalog_semantics.rs`](../rust/crates/tos-compiler/src/prepared_catalog_semantics.rs).
The installed command exposes the bounded optional attachment through
[`OFFLINE_PREPARED_BOOTSTRAP.md`](OFFLINE_PREPARED_BOOTSTRAP.md).

A computed report is mechanical evidence, not semantic acceptance. Source
admission, rights, canon and the choice to select a prepared publication remain
with their stronger owners.
