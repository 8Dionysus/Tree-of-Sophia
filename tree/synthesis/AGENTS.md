# AGENTS.md

This file applies to canonical authored synthesis nodes under `tree/synthesis/`.

## What lives here

`tree/synthesis/` is the home of canonical authored synthesis nodes.

It holds:

- route-local interpretive synthesis surfaces promoted from reviewed intake
- bounded commentary-like readings that now have a canonical authored home
- one `node.json` per stabilized synthesis surface

## Editing posture

Treat these files as canonical authored tree law, not as raw candidate tables.
Keep each `node.json` bounded to one synthesis surface.
Keep the source route explicit in `source_anchor`.
Keep synthesis nodes route-local, source-linked, and explicitly interpretive
rather than silently replacing principle or source canon.
Keep `examples/synthesis_node.example.json` aligned with the worked canonical
mirror rather than creating a second canon.

## Validation

Run:

```bash
python scripts/validate_tree_node_contracts.py
python scripts/validate_tree_example_sync.py
python scripts/generate_kag_export.py
python scripts/validate_kag_export.py
```
