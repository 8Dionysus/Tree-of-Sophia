# AGENTS.md

This file applies to canonical authored principle nodes under `ToS/canon/principle/`.

## What lives here

`ToS/canon/principle/` is the home of canonical authored principle nodes.

It holds:

- route-local principle promotions that remain source-linked
- review-gated distilled claims that have moved beyond raw intake rows
- one `node.json` per stabilized principle surface

## Editing posture

Treat these files as canonical authored tree law; raw candidate tables route to
`ToS/candidate-intake/`.
Keep each `node.json` bounded to one principle claim.
Keep the source route explicit in `source_anchor`.
Keep principle nodes reversible, review-gated, and route-local unless a later
pass explicitly broadens them.
Keep `ToS/public-compatibility/principle_node.example.json` aligned with the worked canonical
mirror as a compatibility surface.

## Validation

Run:

```bash
python scripts/validate_tree_node_contracts.py
python mechanics/boundary-bridge/parts/public-mirror-sync/scripts/validate_tree_example_sync.py
python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/generate_kag_export.py
python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/validate_kag_export.py
```
