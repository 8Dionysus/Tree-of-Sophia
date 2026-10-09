# Source-Returnable Graph Projections

`graph/` holds generated, deletable graph readers over stronger tracked Tree of
Sophia records. Claims and reviews retain their authored sources. Runtime databases, graph UI,
MCP behavior, Neo4j namespaces and RDF stores follow their consumer owner
routes.

## Current projection

`source-witness-bibliographic-claims.min.json` projects the public-safe
bibliographic object and claim catalog into a claim-reified graph:

```text
source identity <- has_subject - claim - has_object -> source identity or literal
                                  |
                                  +-> evidence
                                  +-> maker
                                  +-> provenance event with time and method
                                  +-> any actual human review
```

There is no unqualified subject-to-object edge. Every edge starts at the exact
claim node and carries the canonical claim digest, evidence nodes, maker node,
provenance-event node, review status, and source file/line return.

The graph preserves literal publication objects as literals instead of turning
dates, edition-state descriptions, or unresolved statuses into false
identities. All current claims remain `unreviewed`; projection cannot change
that state.

The current reader includes the full declared identity ladder as reified
claims: 20 `has_expression`, 20 `embodied_by`, and 15 `exemplified_by`
packets. They enter only after exact record/manifest, claim-file, provenance,
and review closure. No direct Work→Expression, Expression→Edition, or
Edition→Item fact edge is emitted, and `embodied_by` remains bibliographic
routing rather than textual equivalence.

The same graph currently reifies one `authored_by` claim for each of the seven
Works. Each of these seven Claims identifies its Work, the Nietzsche Agent and the
source evidence for that authorship assertion.

It also reifies seven `first_publication_chronology` claims. Their structured
intervals, staged events, availability, precision, and ordering warnings stay
literal objects under their own claim nodes. The graph does not select one
timeline facet or turn a date into an identity.

## Read-only query route

An explicit [local assessed build](../../../mechanics/growth-cycle/parts/branch-growth-cycle/README.md#local-assessed-graph-builds)
can carry current source-journal form results into the existing common reader.
It writes a separate private research candidate, never this tracked public
projection. Public-safety clearance, artifact admission and runtime publication
remain separate; the source-parity query route below still reads the ordinary
metadata-only projection.

The native `tos-native-owner-command corpus-projection-query --request ABS_JSON`
route returns exact claim bundles only after it composes the current source
projection and verifies both tracked products against the same source cut. It
preserves the exact selector flags `--claim-ref`, `--subject-ref`, `--object-ref`,
`--normalized-ref`, `--predicate`, `--review-status`, and `--visibility`, with
AND semantics and a bounded `--limit 1..100` (default 20); `--pretty` keeps the
optional indented output form. Each match carries the exact source claim, return path,
line, digest, claim trace, reified nodes, and claim-centered edges. The route
fails instead of truncating. Its native request contract is in
[`Data and corpus operations`](../../../docs/RELEASING.md#data-and-corpus-operations).

## Boundaries

- Source claim packets remain authoritative.
- The generated `source-witnesses/catalog/claims.jsonl` supplies queryable input
with exact return paths to the authored Claims.
- Only catalog-admitted `public` or `public_metadata_only` claims enter this
  tracked graph.
- Local source payload bytes, transcriptions, quotations, and restricted
  material do not enter the projection.
- `abyss-stack` owns any runtime materialization, service, API, MCP, UI,
  Neo4j, or Oxigraph behavior.
- The separate root-level `philosophy_graph_projection.min.json` projects the
atlas and its views; bibliographic Claims retain their own source owner.

## Verify

Use the native corpus-projection check and query commands documented in
[`Data and corpus operations`](../../../docs/RELEASING.md#data-and-corpus-operations).
The focused Rust regressions live with the native projection and query owners
in `rust/crates/tos-command/`.
