# AGENTS.md

This file applies to canonical relation packs under `tree/relations/`.

## What lives here

`tree/relations/` holds canonical relation packs for reviewed route-local
relation layers.

These surfaces are:

- canonical relation packs
- route-local
- carried as `edges.csv`
- rewritten to canonical `tos.*` ids
- rewritten to canonical tos.* ids
- kept registry-first through registered predicates

They are not raw intake ledgers and not node payloads.

## Editing posture

Keep relation packs deterministic, route-local, and explicitly review-gated.
Do not copy unresolved intake residue into this layer.
Keep canonical `tos.*` ids in `from_id` and `to_id`.
Keep predicates aligned to the current registries instead of inventing local
aliases.

## Validation

Run:

```bash
python scripts/validate_tree_relation_pack.py
python scripts/validate_intake_pack.py
python scripts/validate_kag_export.py
```
