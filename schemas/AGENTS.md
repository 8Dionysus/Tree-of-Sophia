# AGENTS.md

This file applies to public schema contracts under `schemas/`.

## What lives here

`schemas/` defines the public structure of current Tree of Sophia scaffold surfaces.
The current contract files are:

- `tos-node-contract.schema.json`
- `tos-tiny-entry-route.schema.json`

## Editing posture

Any schema change here is a public contract change.
Keep `node_id` grammar stable, human-readable, and bounded to ToS-owned object types.
Keep multilingual witness support inside one shared authored node rather than splitting identity by language.
Keep relation enums explicit and bounded.
Keep repo-relative surfaces bounded to ToS-local paths.
Do not fold AoA routing semantics or derived KAG output envelopes into these source contracts.

## Validation

Schema changes currently rely on structured manual review through `docs/REVIEW_CHECKLIST.md`.
If the same change also affects the current tiny export seam, run `python scripts/validate_kag_export.py` after reviewing the schema diff.
