# AGENTS.md

This file applies to canonical authored support nodes under `tree/support/`.

## What lives here

`tree/support/` is the home of canonical authored support nodes.

It holds:

- route-local support surfaces promoted from the reviewed `n.*` layer
- bounded carriers, symbols, places, hinges, and recipient-facing nodes that
  keep the route legible
- one `node.json` per stabilized support surface

## Editing posture

Treat these files as canonical authored tree law, not as raw candidate tables.
Keep each `node.json` bounded to one support surface.
Keep the source route explicit in `source_anchor`.
Keep support nodes route-local and bounded rather than opening many semantic
families too early.
Keep `examples/support_node.example.json` aligned with the worked canonical
mirror rather than creating a second canon.

## Validation

Run:

```bash
python scripts/validate_tree_node_contracts.py
python scripts/validate_tree_example_sync.py
python scripts/generate_kag_export.py
python scripts/validate_kag_export.py
```
