# Access contracts

This directory owns the standalone consumer-facing shape of Access requests,
responses, runtime profiles and selected publication bindings. Prefer the exact
versioned schema or contract for implementation work; this page is a route map.
Runtime interpretation and engine-specific limits are in
[`../shared/NATIVE_SEMANTICS.md`](../shared/NATIVE_SEMANTICS.md).

These contracts describe delivery mechanics. ToS remains the owner of source
identity, authored meaning, assessment, rights, review, registries and canon.
A packet can preserve those recorded postures but cannot strengthen them.

## Find the contract

| Concern | Owner contract |
| --- | --- |
| Runtime profile and admitted data members | [`runtime-manifest.v1.json`](runtime-manifest.v1.json) · [`runtime-data.v1.json`](runtime-data.v1.json) |
| Transport-neutral read operations | [`query-operations.v1.json`](query-operations.v1.json) · [`knowledge-api.v1.json`](knowledge-api.v1.json) |
| Normalized knowledge, lenses and search | [`knowledge-graph.v1.schema.json`](knowledge-graph.v1.schema.json) · [`lens-spec.v1.schema.json`](lens-spec.v1.schema.json) · [`lens-result.v1.schema.json`](lens-result.v1.schema.json) · [`knowledge-search-indexed.v2.md`](knowledge-search-indexed.v2.md) |
| Exploration and temporal comparison | [`exploration-request.v1.schema.json`](exploration-request.v1.schema.json) · [`exploration-result.v1.schema.json`](exploration-result.v1.schema.json) · [`exploration-request.v2.schema.json`](exploration-request.v2.schema.json) · [`exploration-result.v2.schema.json`](exploration-result.v2.schema.json) · [`temporal-comparison-request.v1.schema.json`](temporal-comparison-request.v1.schema.json) · [`temporal-comparison-result.v1.schema.json`](temporal-comparison-result.v1.schema.json) |
| Human-readable forms and context | [`human-form-delivery.md`](human-form-delivery.md) · [`readable-context.v1.schema.json`](readable-context.v1.schema.json) |
| Epistemic and joined evidence packets | [`epistemic-packet.v1.schema.json`](epistemic-packet.v1.schema.json) · [`evidence-lens-packet.v1.schema.json`](evidence-lens-packet.v1.schema.json) |
| Browser commands and local workspace | [`page-commands.v1.json`](page-commands.v1.json) · [`research-workspace.v1.schema.json`](research-workspace.v1.schema.json) · [`web-actions.v1.json`](web-actions.v1.json) |
| Native managed and prepared readers | [`native-managed-release.v1.md`](native-managed-release.v1.md) · [`native-selected-snapshot-profile.v1.md`](native-selected-snapshot-profile.v1.md) |
| Prepared source publication inputs | [`prepared-source-binding.v1.md`](prepared-source-binding.v1.md) · [`prepared-source-dependencies.v1.md`](prepared-source-dependencies.v1.md) · [`source-agent-publication.v1.md`](source-agent-publication.v1.md) · [`source-assembly-normalization.v1.md`](source-assembly-normalization.v1.md) · [`source-metadata-publication.v1.md`](source-metadata-publication.v1.md) · [`source-claim-publication.v1.md`](source-claim-publication.v1.md) |
| Source read and optional text gateway | [`source-read.v1.schema.json`](source-read.v1.schema.json) · [`source-text-gateway.v1.md`](source-text-gateway.v1.md) · [`source-text-gateway-capability.v1.schema.json`](source-text-gateway-capability.v1.schema.json) |
| Source command descriptor | [`source-commands.v1.json`](source-commands.v1.json) |

The ToS-owned stable semantic vocabulary lives under
[`ToS/doctrine/semantic-interchange/`](../../ToS/doctrine/semantic-interchange/)
and is linked from the relevant source/profile contracts. An Access contract
may carry those IDs but does not own or revise their meaning.

## Compatibility boundaries

- Personal workspace hypotheses may carry an optional `kind`, independently
  of their local, unreviewed posture. Earlier v1 records without a kind remain
  unspecified; the neutral `thought` kind does not infer that they are notes.
  Constructor export retains an explicitly recorded draft kind and its ID.
- Software and data are selected separately. `--root`/`TOS_DATA_ROOT`, a
  prepared publication and `--release-root` are distinct selection routes.
  Prepared and managed-release selectors cannot be silently combined.
- Readers fail closed when the selected profile lacks required bindings,
  metadata or companions. They do not discover a different dataset, create a
  missing publication, or treat capability discovery as source admission.
- Each search engine, cursor and exploration runtime retains its own declared
  grammar. A consumer does not reinterpret an offset as a cursor or transfer a
  Worker cursor into a local native reader. See the
  [compressed-search contract](../COMPRESSED_SEARCH_V3.md) for its explicit
  prepared profile.
- Schemas constrain packets and fields; the owner implementation defines
  runtime bounds and currentness checks. Use the native semantics and Rust
  access guides for those details. Green schema checks do not prove complete
  data, semantic correctness, rights or deployment.
- `web-actions.v1.json` is a migration marker. Current browser action shape is
  owned by `page-commands.v1.json` and WebMCP's active page implementation.

Run the repository's `standalone_access` validation route for contract changes.
Its command ordering is maintained in the root validation lane manifest; the
[Access guide](../README.md) remains the product entry point.
