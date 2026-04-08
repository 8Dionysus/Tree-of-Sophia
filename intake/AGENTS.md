# AGENTS.md

This file applies to candidate intake material under `intake/`.

## Read first

Before editing intake material, read:
1. the repository root `AGENTS.md`
2. `docs/MANUAL_CORPUS_ENTRY_GATE.md`
3. `docs/TABULAR_BASE_CONTRACT.md` and `docs/RELATION_PACK_CONTRACT.md` when applicable
4. the exact source witness or source route the intake pass depends on
5. `docs/REVIEW_CHECKLIST.md` if the change broadens scope

## Local role

`intake/` holds candidate structure that may later inform authored ToS work.

Typical contents include:
- candidate node tables
- candidate relation tables
- bounded intake packs
- pass-specific notes about extraction or normalization work

This directory is a staging ledge, not a throne room.

## Editing posture

Keep `intake/` visibly provisional.

Material here is:
- not primary witness authority
- not canonical tree law
- not the public route of truth
- not a hidden automation pipeline

Every intake change should preserve a clear trail back to:
- the source witness
- the pass or extraction frame
- the uncertainty that still remains
- the authored surfaces that do **not** yet exist

Keep early intake light and route-oriented. Do not inflate one pass into a pseudo-platform before the source-facing use case demands it.

Quest or progression language may appear here only as bounded work-tracking or compatibility support. It must not redefine source-linked meaning.

## Hard no

Do not:
- rewrite candidate tables as if they were authored nodes
- treat intake as a backlog warehouse
- hide interpretive uncertainty behind spreadsheet neatness
- let candidate relation packs silently become canonical law

## Validation

Use `docs/REVIEW_CHECKLIST.md` for broader review.

If you change a bounded intake pack or a surface consumed by the current export seam, also run:

```bash
python scripts/validate_intake_pack.py
python scripts/validate_tiny_entry_route.py
python scripts/validate_kag_export.py
```
