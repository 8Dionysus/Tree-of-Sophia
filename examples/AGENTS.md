# AGENTS.md

This file applies to compatibility and scaffold example surfaces under `examples/`.

## Read first

Before editing examples, read:
1. the repository root `AGENTS.md`
2. `tree/AGENTS.md`
3. `schemas/AGENTS.md`
4. the exact canonical tree surface the example mirrors
5. `docs/KAG_EXPORT.md` when the example participates in the current export seam

## Local role

`examples/` holds the current public compatibility and entry surfaces for Tree of Sophia.

These files illustrate:
- the authored node contract
- the current tiny-entry route
- bounded quest compatibility artifacts used for public-safe review or transport

Older superseded pilots may live under `examples/review/`. Treat those as archive or review surfaces, not as current compatibility truth.

## Editing posture

Keep examples aligned with both:
- `schemas/`
- their canonical mirrors in `tree/`

Keep `node_id` values stable, readable, and scoped to one authored object. Use one shared `node_id` across multilingual witnesses rather than language-split copies.

Examples are not:
- a second canon
- generated runtime state
- a substitute for doctrine docs
- permission to broaden the bounded route into a full corpus program

`quest_catalog.min.example.json` and `quest_dispatch.min.example.json` are reviewable compatibility artifacts. They are not runtime authority, not live quest state, and not a license to collapse authored meaning into backlog language.

## Hard no

Do not:
- edit an example first and retrofit the tree later
- treat `examples/review/` as an active compatibility surface
- widen example payloads just because downstream consumers might want more
- blur authored nodes and operational compatibility artifacts into one envelope

## Validation

For scaffold or doctrine changes, use `docs/REVIEW_CHECKLIST.md`.

If you change the current mirror set or the current tiny-entry route examples, run:

```bash
python scripts/validate_tree_example_sync.py
python scripts/generate_kag_export.py
python scripts/validate_kag_export.py
```
