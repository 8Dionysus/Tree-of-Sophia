# AGENTS.md

This file applies to authored doctrine and review surfaces under `docs/`.

## What lives here

`docs/` is where Tree of Sophia states its public knowledge law.
These files define the knowledge model, node contract, practice-lineage boundary, counterpart and compost posture, calibration and growth law, identifier discipline, node templates, bounded lineage pilot, tiny-entry route, current Zarathustra capsule, KAG export posture, and manual review route.

Key files here include:

- `KNOWLEDGE_MODEL.md`
- `NODE_CONTRACT.md`
- `PRACTICE_BRANCH.md`
- `COUNTERPART_POLICY.md`
- `CONTEXT_COMPOST.md`
- `CALIBRATION_AXIS.md`
- `HUMAN_CURATED_EXPANSION.md`
- `GROWTH_STRUCTURE.md`
- `IDENTIFIER_DISCIPLINE.md`
- `SOURCE_NODE_TEMPLATE.md`
- `CONCEPT_NODE_TEMPLATE.md`
- `LINEAGE_NODE_TEMPLATE.md`
- `CONTEXT_NODE_TEMPLATE.md`
- `MANUAL_CORPUS_ENTRY_GATE.md`
- `TINY_ENTRY_ROUTE.md`
- `ZARATHUSTRA_TRILINGUAL_ENTRY.md`
- `KAG_EXPORT.md`
- `TABULAR_BASE_CONTRACT.md`
- `REVIEW_CHECKLIST.md`
- review notes under `docs/reviews/`

## Editing posture

Treat these docs as authored ToS meaning, not as operational notes or derived KAG restatements.
Keep `README.md` as the current public `tos-root`.
Preserve the difference between raw source in `sources/`, candidate extraction in `intake/`, canonical authored tree surfaces in `tree/`, compatibility surfaces in `examples/`, interpretation, synthesis, and derived export posture.

When a note touches multilingual witnesses or the Zarathustra route:

- keep one shared `node_id`
- keep the canonical source visibly authoritative
- do not split the tree into parallel language branches
- keep fallback inside ToS rather than jumping straight to downstream repos

When a note touches counterpart mapping, compost, calibration, or growth:

- keep those surfaces optional, bounded, and explicitly non-identity
- do not let AoA operational language silently replace ToS doctrine

## Validation

For broader doctrine changes, use `docs/REVIEW_CHECKLIST.md`.
If you touch the current tiny export seam or the docs it depends on, run:

```bash
python scripts/validate_tree_example_sync.py
python scripts/validate_intake_pack.py
python scripts/generate_kag_export.py
python scripts/validate_kag_export.py
```
