# AGENTS.md

This file applies to canonical authored event nodes under `tree/event/`.

## What lives here

`tree/event/` is the home of canonical authored event nodes.

It holds:

- route-local event surfaces that make bounded movement legible
- review-gated dynamic nodes promoted from candidate intake
- one `node.json` per stabilized event surface

## Editing posture

Treat these files as canonical authored tree law, not as raw candidate tables.
Keep each `node.json` bounded to one event surface.
Keep the source route explicit in `source_anchor`.
Keep event nodes source-linked, route-local, and dynamic rather than inflating
them into free-floating principles.
Keep `examples/event_node.example.json` aligned with the worked canonical
mirror rather than creating a second canon.

## Validation

Run:

```bash
python scripts/validate_tree_node_contracts.py
python scripts/validate_tree_example_sync.py
python scripts/generate_kag_export.py
python scripts/validate_kag_export.py
```
