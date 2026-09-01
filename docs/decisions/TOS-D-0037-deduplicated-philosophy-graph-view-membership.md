# Deduplicated Philosophy Graph View Membership

## Index Metadata

- Decision ID: TOS-D-0037
- Original date: 2026-09-01
- Surface classes: derived-export, contract, scripts/validation, docs/route-law
- ToS layers: derived-export, contract, validation, docs
- Tree classes: graph projection, view lens, generated companion
- Guard families: source-returning projection, deterministic generation, repository portability, runtime boundary
- Posture: accepted

## Context

The complete 190-dossier philosophy atlas produces one global materialized
graph plus eleven overlapping graph lenses. Projection v1 copied every full
node and edge record into each matching lens as well as the global arrays. The
Table III landing grew that deterministic JSON export to 125,123,899 bytes,
past GitHub's 100 MiB per-file admission boundary, while more than 100 MiB of
the payload was repeated lens material.

The graph projection is a generated downstream read model. Its source-owned
atlas, view filters, review packets, clusters, source references, and global
node and edge records remain the meaningful inputs and trace surfaces.

## Decision

Projection contract v2 materializes every projected node and edge exactly once
in the top-level `nodes` and `edges` arrays. Each entry in `views` publishes
ordered `node_ids` and `edge_ids` membership over those global arrays instead
of embedding duplicate full records.

The builder still computes each view from the same source-owned filters and
uses the full in-memory view material for layer counts, review packets, and
snapshot fingerprints before emitting ID membership. Validators require every
view reference to resolve inside the global materialized set. `abyss-stack`
remains the runtime owner and must dereference view membership without writing
runtime state back into ToS.

## Options Considered

- Store the 125 MiB JSON through Git LFS. Rejected because it would add a
  quota- and client-dependent transport requirement without correcting the
  repeated representation.
- Split every lens into a separate generated file. Deferred because it adds a
  shard manifest and multi-file atomicity contract that the current consumer
  does not need.
- Keep one global materialized graph and represent lenses by stable IDs.
  Chosen because it preserves all current graph content, source returns,
  review calculations, and lens membership while removing redundant bytes.

## Rationale

The chosen representation matches the existing `cluster-first` visibility
model: global records own the generated payload, and clusters and views select
them by stable identity. It removes transport pressure without suppressing
nodes, edges, source refs, multilingual labels, review posture, or graph
layers. The v2 marker makes the consumer-visible shape change explicit.

## Consequences

The generated export falls from 125,123,899 bytes to about 30 MiB for the
current atlas. Downstream consumers of v1 view-local `nodes` and `edges` must
switch to `node_ids` and `edge_ids`, then resolve those IDs against the global
arrays. Counts now expose total view node and edge references separately from
unique global records.

This decision does not change atlas meaning, dossier admission, review or
canon posture, source truth, or runtime ownership. A future move to sharded
transport requires a separate contract decision.

## Source Surfaces

- `ToS/contracts/philosophy-graph-projection.schema.json`
- `scripts/philosophy_graph_projection_common.py`
- `scripts/validate_philosophy_graph_projection.py`
- `tests/test_philosophy_graph_projection.py`
- `ToS/derived-exports/AGENTS.md`
- `ToS/derived-exports/README.md`
- `ToS/derived-exports/philosophy_graph_projection.min.json`

## Validation

Run the philosophy graph projection builder check, validator, and focused
tests. Regenerate and validate the decision indexes, then run the repository
release check and pinned repo-local KAG parity before landing.
