# AGENTS.md

This file applies to canonical authored state nodes under `tree/state/`.

## What lives here

`tree/state/` is the home of canonical authored state nodes.

It holds:

- route-local state surfaces that make bounded conditions legible
- review-gated dynamic nodes promoted from candidate intake
- one `node.json` per stabilized state surface

## Editing posture

Treat these files as canonical authored tree law, not as raw candidate tables.
Keep each `node.json` bounded to one state surface.
Keep the source route explicit in `source_anchor`.
Keep state nodes source-linked, route-local, and condition-bearing rather than
inflating them into commentary doctrine.
Keep `examples/state_node.example.json` aligned with the worked canonical
mirror rather than creating a second canon.

## Validation

Run:

```bash
python scripts/validate_tree_node_contracts.py
python scripts/validate_tree_example_sync.py
python scripts/generate_kag_export.py
python scripts/validate_kag_export.py
```
