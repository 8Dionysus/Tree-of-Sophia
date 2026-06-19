# AGENTS.md

This file applies to canonical authored lineage nodes under `ToS/canon/lineage/`.

## What lives here

`ToS/canon/lineage/` is the home of canonical authored lineage nodes.

It holds:

- route-local lineage surfaces that make a bounded branch legible
- review-gated branch handles that tie source, concept, and principle nodes
  into one authored route
- one `node.json` per stabilized lineage surface

## Editing posture

Treat these files as canonical authored tree law; raw candidate tables route to
`ToS/candidate-intake/`.
Keep each `node.json` bounded to one lineage surface.
Keep the route explicit in `source_anchor`.
Keep lineage nodes branch-legible and reviewable; atlas-wide topology routes to
`ToS/philosophy/` until a later reviewed pass broadens canon.
Keep `ToS/public-compatibility/lineage_node.example.json` aligned with the worked canonical
mirror as a compatibility surface.

## Validation

Run:

```bash
python scripts/validate_tree_node_contracts.py
python mechanics/boundary-bridge/parts/public-mirror-sync/scripts/validate_tree_example_sync.py
python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/generate_kag_export.py
python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/validate_kag_export.py
```
