# Native exact catalog index

The former Python `tos_access.catalog_index` and `catalog_semantics` APIs are
retired. Exact catalog materialization and index state now belong to the Rust
compiler: [`prepared_catalog_index.rs`](../rust/crates/tos-compiler/src/prepared_catalog_index.rs)
and [`prepared_catalog_semantics.rs`](../rust/crates/tos-compiler/src/prepared_catalog_semantics.rs).
The query-side selected catalog reader is
[`tos-query::knowledge_catalog`](../rust/crates/tos-query/src/knowledge_catalog.rs).

Use the installed native `tos prepare` flow documented in
[`OFFLINE_PREPARED_BOOTSTRAP.md`](OFFLINE_PREPARED_BOOTSTRAP.md) to create an
explicit local publication. Candidate catalog bytes and a green build do not
admit source meaning, rights, semantics, canon or a consumer selection.
