# AGENTS.md

This file applies to canonical authored principle nodes under `tree/principle/`.

## What lives here

`tree/principle/` is the home of canonical authored principle nodes.

It holds:

- route-local principle promotions that remain source-linked
- review-gated distilled claims that have moved beyond raw intake rows
- one `node.json` per stabilized principle surface

## Editing posture

Treat these files as canonical authored tree law, not as raw candidate tables.
Keep each `node.json` bounded to one principle claim.
Keep the source route explicit in `source_anchor`.
Keep principle nodes reversible, review-gated, and route-local unless a later
pass explicitly broadens them.
Keep `examples/principle_node.example.json` aligned with the worked canonical
mirror rather than creating a second canon.

## Validation

Run:

```bash
python scripts/validate_tree_node_contracts.py
python scripts/validate_tree_example_sync.py
python scripts/generate_kag_export.py
python scripts/validate_kag_export.py
```
