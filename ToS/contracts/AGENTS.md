# AGENTS.md

This file applies to public schema contracts under `ToS/contracts/`.

## Read first

Before changing a schema, read:
1. the repository root `AGENTS.md`
2. `ToS/doctrine/KNOWLEDGE_MODEL.md`
3. `ToS/doctrine/NODE_CONTRACT.md`
4. the relevant example and canonical tree mirror
5. `mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md` if the schema touches the current export seam

## Local role

`ToS/contracts/` defines the public structure of current Tree of Sophia scaffold surfaces.

The current contract files include:
- `tos-node-contract.schema.json`
- `tos-tiny-entry-route.schema.json`

Any schema edit here is a public contract change.

## Operating Card

| Field | Route |
| --- | --- |
| role | public schema contract surface for ToS structures |
| input | reviewed structural change in canon, ToS compatibility, or export boundary |
| output | schema contract aligned with examples and validators |
| owner | `ToS/contracts/AGENTS.md` and the specific schema file |
| next route | source/canon/compatibility pressure -> schema update -> examples -> validators |
| tools | JSON schema, example sync, export validation, review checklist |
| check | schema-aware validators and affected route validators |

## Editing posture

Keep:
- `node_id` grammar stable, readable, and bounded to ToS-owned object types
- multilingual witness support inside one shared authored identity
- relation enums explicit and bounded
- repo-relative surfaces local to ToS

Route AoA routing semantics, generated KAG export envelopes, runtime
assumptions, and non-source operational contracts to their owning surfaces
before changing source-owned contracts.

## Boundary Routes

- Semantic model changes route through doctrine, canon, contracts, examples,
  and validators together.
- One authored node identity stays shared across multilingual witnesses unless
  the node contract itself changes.
- Downstream convenience becomes contract language only after source-owned ToS
  surfaces justify it.

## Validation

Schema changes rely on structured review through `mechanics/audit/parts/review-ledger-route/docs/REVIEW_CHECKLIST.md`.

If the same change affects current examples or the bounded export seam, also run:

```bash
python scripts/validate_tree_example_sync.py
python scripts/validate_kag_export.py
```
