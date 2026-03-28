# AGENTS.md

This file applies to canonical authored lineage nodes under `tree/lineage/`.

## What lives here

`tree/lineage/` is the home of canonical authored lineage nodes.

It holds:

- route-local lineage surfaces that make a bounded branch legible
- review-gated branch handles that tie source, concept, and principle nodes
  into one authored route
- one `node.json` per stabilized lineage surface

## Editing posture

Treat these files as canonical authored tree law, not as raw candidate tables.
Keep each `node.json` bounded to one lineage surface.
Keep the route explicit in `source_anchor`.
Keep lineage nodes branch-legible and reviewable rather than atlas-like unless a
later pass explicitly broadens them.
Keep `examples/lineage_node.example.json` aligned with the worked canonical
mirror rather than creating a second canon.

## Validation

Run:

```bash
python scripts/validate_tree_node_contracts.py
python scripts/validate_tree_example_sync.py
python scripts/generate_kag_export.py
python scripts/validate_kag_export.py
```
