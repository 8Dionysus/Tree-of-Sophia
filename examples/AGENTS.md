# AGENTS.md

This file applies to scaffold and source-owned example surfaces under `examples/`.

## What lives here

`examples/` holds the current public scaffold and entry surfaces for Tree of Sophia.
These files illustrate the authored node contract and the current tiny-entry route without pretending to be a full corpus.
They now also act as compatibility mirrors of the canonical authored tree in `tree/`.

The current example set includes:

- `source_node.example.json`
- `source_node_gay_science.example.json`
- `concept_node.example.json`
- `concept_node_overcoming.example.json`
- `concept_node_stability.example.json`
- `lineage_node_calibration_family.example.json`
- `tos_tiny_entry_route.example.json`

## Editing posture

Keep examples aligned with `schemas/`.
Keep `node_id` values stable, readable, and scoped to one authored object.
Use one shared `node_id` across multilingual witnesses rather than language-split copies.
Keep source-node, concept-node, lineage-node, and tiny-entry-route examples distinct.
Do not let bounded examples quietly become an uncontrolled corpus program.
Do not use example payloads as a substitute for the owning doctrine docs.
Keep `examples/source_node.example.json` and `examples/concept_node.example.json` aligned with their canonical authored tree mirrors.

## Validation

For scaffold or doctrine changes, use `docs/REVIEW_CHECKLIST.md`.
If you change `source_node.example.json` or `concept_node.example.json` as part of the current export seam, run:

```bash
python scripts/validate_tree_example_sync.py
python scripts/generate_kag_export.py
python scripts/validate_kag_export.py
```
