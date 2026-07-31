# Source-Returnable Graph Projections

`graph/` holds generated, deletable graph readers over stronger tracked Tree of
Sophia records. It does not own claims, reviews, runtime databases, graph UI,
MCP behavior, Neo4j namespaces, or RDF stores.

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

## Boundaries

- Source claim packets remain authoritative.
- The generated `source-witnesses/catalog/claims.jsonl` is the queryable input,
  not a second claim authority.
- Only catalog-admitted `public` or `public_metadata_only` claims enter this
  tracked graph.
- Local source payload bytes, transcriptions, quotations, and restricted
  material do not enter the projection.
- `abyss-stack` owns any runtime materialization, service, API, MCP, UI,
  Neo4j, or Oxigraph behavior.
- The separate root-level `philosophy_graph_projection.min.json` remains the
  atlas/view projection and is not widened into a bibliographic claim owner.

## Verify

Use the graph parity and validation entries owned by the
[scripts route card](../../../scripts/AGENTS.md) and the repository
`release_check` lane. The focused regression owner is
`tests/test_source_witness_bibliographic_graph.py`.
