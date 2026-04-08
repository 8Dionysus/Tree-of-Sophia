# AGENTS.md

This file applies to authored doctrine and review surfaces under `docs/`.

## Read first

Before changing anything here, read:
1. the repository root `AGENTS.md`
2. `README.md`
3. `CHARTER.md` and `BOUNDARIES.md`
4. `docs/KNOWLEDGE_MODEL.md` and `docs/NODE_CONTRACT.md`
5. the exact template, route, checklist, or review note you are touching

## Local role

`docs/` is where Tree of Sophia states its public knowledge law.

These files govern:
- the knowledge model and interpretation law
- node and relation contracts
- growth, calibration, compost, and counterpart posture
- identifier and template discipline
- review and manual corpus-entry gates
- the current bounded public route and export posture

Use this directory to clarify how source-linked meaning becomes authored structure. Do not use it as a dumping ground for generic notes, runtime policy, or downstream restatements.

## Editing posture

Treat `docs/` as authored ToS meaning.

Keep these distinctions legible:
- raw witness in `sources/`
- candidate structure in `intake/`
- canonical authored tree surfaces in `tree/`
- compatibility mirrors in `examples/`
- derived exports in `generated/`

When a doc touches multilingual witness routes:
- keep one shared `node_id`
- keep the canonical source visibly authoritative
- do not split identity into parallel language trees
- keep fallback and route repair inside ToS before reaching for downstream repos

When a doc touches growth, calibration, compost, or counterpart posture:
- keep those surfaces bounded and explicitly non-identity
- do not let AoA operational language silently replace ToS doctrine
- do not let quest or RPG reflection vocabulary replace node, source, or authority semantics

Review notes in `docs/reviews/` are dated inspection surfaces. They can justify, archive, or challenge a route, but they do not overrule the current doctrine files they discuss.

## Hard no

Do not:
- restate derived KAG projections as if they were ToS law
- turn route docs into backlog, quest, or runtime-control surfaces
- flatten source, interpretation, and synthesis into one tone
- use a review note to bypass a stronger current contract

## Validation

For broader doctrine changes, use `docs/REVIEW_CHECKLIST.md`.

If you touch the current tiny route, the current export seam, or the docs those surfaces depend on, run:

```bash
python scripts/validate_tiny_entry_route.py
python scripts/validate_tree_example_sync.py
python scripts/validate_tree_node_contracts.py
python scripts/validate_tree_relation_pack.py
python scripts/validate_intake_pack.py
python scripts/generate_kag_export.py
python scripts/validate_kag_export.py
```
