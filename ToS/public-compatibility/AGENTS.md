# AGENTS.md

This file applies to compatibility and scaffold example surfaces under `ToS/public-compatibility/`.

## Read first

Before editing examples, read:
1. the repository root `AGENTS.md`
2. `../canon/AGENTS.md`
3. `ToS/contracts/AGENTS.md`
4. the exact canonical tree surface the example mirrors
5. `../../mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md` when the example participates in the current export seam

## Local role

`ToS/public-compatibility/` holds the current public compatibility and entry surfaces for Tree of Sophia.

These files illustrate:
- the authored node contract
- the current tiny-entry route
- public-safe ToS compatibility mirrors that remain subordinate to canon

Older superseded pilots may live under `ToS/public-compatibility/review/`.
Treat those as archive or review surfaces; current compatibility truth stays
in the active files at this branch root.

## Operating Card

| Field | Route |
| --- | --- |
| role | public compatibility and tiny-entry example surface |
| input | reviewed canon object, contract change, public-safe route example, or bounded compatibility artifact |
| output | example payload aligned with canon, contract, and export validators |
| owner | `ToS/public-compatibility/AGENTS.md`; review archive owner for `review/` |
| next route | canon or contract -> public example -> generated export when included |
| tools | tree example sync, KAG export generator, schema validators |
| check | `python scripts/validate_tree_example_sync.py` and export validation when relevant |

## Editing posture

Keep examples aligned with both:
- `ToS/contracts/`
- their canonical mirrors in `../canon/`

Keep `node_id` values stable, readable, and scoped to one authored object. Use one shared `node_id` across multilingual witnesses rather than language-split copies.

Examples route back to:
- canon for authored object meaning
- contracts for public structure
- doctrine docs for route law
- generated exports only when the bounded route includes them

## Boundary Routes

- Change canon or contracts first when the example needs new source-owned
  meaning.
- Route superseded material to `ToS/public-compatibility/review/` with visible
  archive posture.
- Widen public payloads only after the owning source, canon, contract, and
  review surfaces support the wider route.
- Keep authored nodes distinct from operational artifacts and generated
  companions.

## Validation

For scaffold or doctrine changes, use `mechanics/audit/parts/review-ledger-route/docs/REVIEW_CHECKLIST.md`.

If you change the current mirror set or the current tiny-entry route examples, run:

```bash
python scripts/validate_tree_example_sync.py
python scripts/generate_kag_export.py
python scripts/validate_kag_export.py
```
