# Compressed knowledge search v3

Compressed search is a native Rust query mode for the explicitly selected
`tos_local_prepared_read_model_v1` profile. It returns complete graph carriers
under the `tos_knowledge_search_compressed_v3` packet contract. Rust owns
selection binding, normalization, filtering, continuation, work/response bounds
and body verification. The imported Python SDK only frames a native call; it
does not ship a `SearchStore` or `PublishedSearchService`.

The Python search-store design formerly recorded in this file is retired. Its
implementation details and synthetic storage measurements are not current
runtime contracts or full-corpus forecasts; the native `tos-query` owner and
the [Access runtime guide](../rust/crates/tos-access/README.md) describe the
maintained route.

## Local adapters

Compressed mode requires both an explicit prepared-reader selection and an
explicit mode. The default remains `legacy`; no reader creates a publication,
chooses another dataset, or silently falls back after a selected mode fails.
The local access guide documents the paired selectors under
[Explicit prepared local reader](README.md#explicit-prepared-local-reader).

Use any of these native adapters:

```sh
tos --prepared-read-model /absolute/selected.sqlite \
  --prepared-binding /absolute/owner-selected-binding.json \
  knowledge search QUERY --mode compressed
```

- HTTP: `GET /api/knowledge/search?mode=compressed`
- Native MCP: `tos_knowledge_search(..., mode="compressed")`
- Imported SDK: `ToSAccessCore.knowledge_search_compressed(...)`

Provide the same `query`, filters and returned `cursor` for each page. Offsets
are not accepted. An empty page is not necessarily exhaustion; continue until
`has_more` is false. Cursor validation binds the query, selected filters and
publication. A stale binding or cursor requires a fresh search.

Discover the selected engine through `knowledge_search_capabilities()`, CLI
`tos knowledge search-capabilities`, HTTP `GET
/api/knowledge/search/capabilities`, or native MCP
`tos_knowledge_search_capabilities`. Compatibility-mode availability describes
engine selection only. Compressed availability checks the selected publication's
search headers and required objects, not every corpus row.

HTTP maps malformed requests/cursors to 400, stale bindings to 409, authenticated
expiry to 410, hard budget refusal to 413, and unavailable/corrupt publication
to 503. CLI and MCP use their normal error envelopes. HTTP and CLI emit compact
UTF-8 JSON; MCP carries the same compact packet as text and structured content.
Logical result limits do not include the additional MCP envelope and duplicated
representation; GET requests remain subject to the HTTP request-line limit.
The installed prepared reader uses native defaults: 256 candidate work units,
65,536 verification bytes, and 4 MiB each for metadata and body work. A request
asks for 1–100 results; those result counts are separate from the work budgets.
[The native search owner](../rust/crates/tos-query/src/compressed_search_state.rs)
defines the exact request normalization and configurable bounds, and
[the installed reader](../rust/crates/tos-access/src/prepared_local.rs) selects
those defaults. The separate
[prepared-read limits](../rust/crates/tos-compiler/src/local_prepared_read.rs)
also bound SQL, rows and response bytes. Worker/D1 semantics and limits are
maintained separately in [NATIVE_SEMANTICS.md](shared/NATIVE_SEMANTICS.md).
