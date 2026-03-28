# AGENTS.md

This file applies to scaffold and source-owned example surfaces under `examples/`.

## What lives here

`examples/` holds the current public scaffold and entry surfaces for Tree of Sophia.
These files illustrate the authored node contract and the current tiny-entry route without pretending to be a full corpus.
They now also act as compatibility mirrors of the canonical authored tree in `tree/`.

The current example set includes:

- `source_node.example.json`
- `concept_node.example.json`
- `principle_node.example.json`
- `lineage_node.example.json`
- `event_node.example.json`
- `state_node.example.json`
- `support_node.example.json`
- `tos_tiny_entry_route.example.json`

Older superseded pilot scaffolds may live under `examples/review/`.
Treat those files as review/archive material rather than as active canon.

## Editing posture

Keep examples aligned with `schemas/`.
Keep `node_id` values stable, readable, and scoped to one authored object.
Use one shared `node_id` across multilingual witnesses rather than language-split copies.
Keep source-node, concept-node, principle-node, lineage-node, event-node, state-node, support-node, and tiny-entry-route examples distinct.
Do not let bounded examples quietly become an uncontrolled corpus program.
Do not use example payloads as a substitute for the owning doctrine docs.
Keep `examples/source_node.example.json`, `examples/concept_node.example.json`, and `examples/principle_node.example.json` aligned with their canonical authored tree mirrors.
Keep `examples/lineage_node.example.json` aligned with its canonical authored tree mirror.
Keep `examples/event_node.example.json` and `examples/state_node.example.json` aligned with their canonical authored tree mirrors.
Keep `examples/support_node.example.json` aligned with its canonical authored tree mirror.
Do not treat `examples/review/` as an active compatibility surface.

## Validation

For scaffold or doctrine changes, use `docs/REVIEW_CHECKLIST.md`.
If you change `source_node.example.json`, `concept_node.example.json`, `principle_node.example.json`, `lineage_node.example.json`, `event_node.example.json`, `state_node.example.json`, or `support_node.example.json` as part of the current canonical mirror set, run:

```bash
python scripts/validate_tree_example_sync.py
python scripts/generate_kag_export.py
python scripts/validate_kag_export.py
```
