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

The current reader includes the full declared identity ladder as reified
claims: 20 `has_expression`, 20 `embodied_by`, and 15 `exemplified_by`
packets. They enter only after exact record/manifest, claim-file, provenance,
and review closure. No direct Work→Expression, Expression→Edition, or
Edition→Item fact edge is emitted, and `embodied_by` remains bibliographic
routing rather than textual equivalence.

The same graph currently reifies one `authored_by` claim for each of the seven
Works. These are seven separately source-returnable claims to the Nietzsche
Agent, not a generic creator shortcut and not a path-derived assertion.

## Read-only query route

`scripts/query_source_witness_bibliographic_graph.py` is the repository-local
stdout query route. Before returning a result it validates the tracked graph,
checks its projection fingerprint, rebuilds it from the exact source-owned
catalog and claim packets, and requires byte-equivalent canonical parity.

The current exact selectors are `claim-ref`, `subject-ref`, identity-valued
`object-ref`, `predicate`, `review-status`, and `visibility`. Combined
selectors use AND semantics. Every match returns:

- the exact source claim object, repository-relative JSONL file, line, and
  canonical digest;
- the reified claim, subject, and identity-or-literal object nodes;
- evidence, counterevidence, maker, provenance event, and actual review nodes;
- every claim-centered edge and the unchanged trace packet.

At least one selector is mandatory. The route returns an explicit `no_match`
result for an empty result set and fails instead of silently truncating when
the result count exceeds the declared limit. It writes no query result,
database, cache, review, or accepted relation.

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
