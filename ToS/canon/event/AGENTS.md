# AGENTS.md

This file applies to canonical authored event nodes under `ToS/canon/event/`.

## What lives here

`ToS/canon/event/` is the home of canonical authored event nodes.

It holds:

- route-local event surfaces that make bounded movement legible
- review-gated dynamic nodes promoted from candidate intake
- one `node.json` per stabilized event surface

## Editing posture

Treat these files as canonical authored tree law; raw candidate tables route to
`ToS/candidate-intake/`.
Keep each `node.json` bounded to one event surface.
Keep the source route explicit in `source_anchor`.
Keep event nodes source-linked, route-local, and dynamic; distilled claims
route to principle nodes after review.
Keep `ToS/public-compatibility/event_node.example.json` aligned with the worked canonical
mirror as a compatibility surface.

## Validation

Run:

```bash
python scripts/validate_tree_node_contracts.py
python mechanics/boundary-bridge/parts/public-mirror-sync/scripts/validate_tree_example_sync.py
python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/generate_kag_export.py
python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/validate_kag_export.py
```
