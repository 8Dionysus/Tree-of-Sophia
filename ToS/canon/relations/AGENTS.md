# AGENTS.md

This file applies to canonical relation packs under `ToS/canon/relations/`.

## What lives here

`ToS/canon/relations/` holds canonical relation packs for reviewed route-local
relation layers.

These surfaces are:

- canonical relation packs
- route-local
- carried as `edges.csv`
- rewritten to canonical `tos.*` ids
- kept registry-first through registered predicates

Raw intake ledgers stay in `ToS/candidate-intake/`; node payloads stay in the
owning canon node branch.

## Editing posture

Keep relation packs deterministic, route-local, and explicitly review-gated.
Unresolved intake residue stays in `ToS/candidate-intake/` until reviewed.
Keep canonical `tos.*` ids in `from_id` and `to_id`.
Keep predicates aligned to the current registries instead of inventing local
aliases.

## Validation

Run:

```bash
python mechanics/relation-weaving/parts/graph-promotion/scripts/validate_tree_relation_pack.py
python scripts/validate_intake_pack.py
python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/validate_kag_export.py
```
