# AGENTS.md

This file applies to public schema contracts under `schemas/`.

## Read first

Before changing a schema, read:
1. the repository root `AGENTS.md`
2. `docs/KNOWLEDGE_MODEL.md`
3. `docs/NODE_CONTRACT.md`
4. the relevant example and canonical tree mirror
5. `docs/KAG_EXPORT.md` if the schema touches the current export seam

## Local role

`schemas/` defines the public structure of current Tree of Sophia scaffold surfaces.

The current contract files include:
- `tos-node-contract.schema.json`
- `tos-tiny-entry-route.schema.json`
- `quest.schema.json`
- `quest_dispatch.schema.json`

Any schema edit here is a public contract change.

## Editing posture

Keep:
- `node_id` grammar stable, readable, and bounded to ToS-owned object types
- multilingual witness support inside one shared authored identity
- relation enums explicit and bounded
- repo-relative surfaces local to ToS

Do not fold:
- AoA routing semantics
- derived KAG export envelopes
- hidden runtime assumptions
into source-owned schema contracts.

Quest contracts here are operational compatibility surfaces only. They must not flatten authored meaning into backlog or game-state language.

## Hard no

Do not:
- smuggle a semantic model rewrite through a "small schema tweak"
- split one authored node identity into language-specific variants
- let downstream convenience become schema sovereignty

## Validation

Schema changes rely on structured review through `docs/REVIEW_CHECKLIST.md`.

If the same change affects current examples or the bounded export seam, also run:

```bash
python scripts/validate_tree_example_sync.py
python scripts/validate_kag_export.py
```
