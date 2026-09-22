# Relation Pack Contract

This document defines the first route-local canonical relation-pack contract
for Tree of Sophia.

## Role

This contract defines one reviewed carrier for canonical route-local
relations:

- `ToS/canon/relations/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/edges.csv`

The carrier holds reviewed relations; intake retains the complete candidate
set and each row’s review outcome.

## Carrier

The canonical carrier is a route-local CSV relation pack.

The current pack keeps these fields:

```text
edge_id,edge_kind,from_id,predicate_id,to_id,layer,anchor_mode,anchor_start_secondary,anchor_end_secondary,anchor_segment_ids,witness_scope,connectivity_role,confidence,note
```

`from_id` and `to_id` must use canonical `tos.*` ids only.

## Promotion boundary

The current pass promotes only those rows whose two endpoints are already
canonical in `ToS/canon/`.

That yields one bounded pack of exactly:

- 125 promoted edges
- 92 `source_edge`
- 11 `bridge_edge`
- 22 `principle_edge`

Everything else stays in `ToS/candidate-intake/edges.csv` with an explicit deferred status.

## Predicate posture

The canonical relation pack stays registry-first.

That means:

- `predicate_id` stays aligned to `ToS/canon/registries/predicates.csv`
- no aliasing into the narrow public node `relations` enum
- endpoint compatibility remains checked against `ToS/canon/registries/classes.csv`

The relation pack therefore makes reviewed graph structure canonical without
collapsing ToS node payloads into the full tabular graph.

## Boundary with intake

`ToS/candidate-intake/.../edges.csv` remains the review ledger for the wider bounded route.

Its current required split is:

- 125 `promoted`
- 3 `deferred_literal`

The canonical relation pack retains the reviewed rows and their source
relationship to intake.

## Validation

Run:

The `intake_contracts`, `canon_contracts`, and `public_entry` sequences in
`docs/validation/validation_lanes.json` own executable verification. Their
validators remain in the corresponding script and mechanic-part homes.
