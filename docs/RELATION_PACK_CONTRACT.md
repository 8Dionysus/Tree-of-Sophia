# Relation Pack Contract

This document defines the first route-local canonical relation-pack contract
for Tree of Sophia.

## Role

The current relation pass does not introduce a new node family.
It introduces one reviewed carrier for canonical route-local relations:

- `tree/relations/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/edges.csv`

This surface exists so ToS can carry reviewed graph-bearing relations in canon
without pretending that every candidate edge in `intake/` has already crossed
the review boundary.

## Carrier

The canonical carrier remains tabular and route-local.
It is not a `node.json` payload.

The current pack keeps these fields:

```text
edge_id,edge_kind,from_id,predicate_id,to_id,layer,anchor_mode,anchor_start_secondary,anchor_end_secondary,anchor_segment_ids,witness_scope,connectivity_role,confidence,note
```

`from_id` and `to_id` must use canonical `tos.*` ids only.

## Promotion boundary

The current pass promotes only those rows whose two endpoints are already
canonical in `tree/`.

That yields one bounded pack of exactly:

- 122 promoted edges
- 90 `source_edge`
- 11 `bridge_edge`
- 21 `principle_edge`

Everything else stays in `intake/edges.csv` with an explicit deferred status.

## Predicate posture

The canonical relation pack stays registry-first.

That means:

- `predicate_id` stays aligned to `tree/registries/predicates.csv`
- no aliasing into the narrow public node `relations` enum
- endpoint compatibility remains checked against `tree/registries/classes.csv`

The relation pack therefore makes reviewed graph structure canonical without
collapsing ToS node payloads into the full tabular graph.

## Boundary with intake

`intake/.../edges.csv` remains the review ledger for the wider bounded route.

Its current required split is:

- 122 `promoted`
- 3 `deferred_literal`
- 2 `deferred_analogy`
- 1 `deferred_commentary`

The canonical relation pack is therefore a reviewed projection from intake, not
a second unrelated table.

## Validation

Run:

```bash
python scripts/validate_intake_pack.py
python scripts/validate_tree_relation_pack.py
python scripts/validate_kag_export.py
```
