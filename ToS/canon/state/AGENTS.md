# AGENTS.md

This file applies to canonical authored state nodes under `ToS/canon/state/`.

## What lives here

`ToS/canon/state/` is the home of canonical authored state nodes.

It holds:

- route-local state surfaces that make bounded conditions legible
- review-gated dynamic nodes promoted from candidate intake
- one `node.json` per stabilized state surface

## Editing posture

Treat these files as canonical authored tree law; raw candidate tables route to
`ToS/candidate-intake/`.
Keep each `node.json` bounded to one state surface.
Keep the source route explicit in `source_anchor`.
Keep state nodes source-linked, route-local, and condition-bearing; commentary
doctrine routes to `ToS/doctrine/`.
Keep `ToS/public-compatibility/state_node.example.json` aligned with the worked canonical
mirror as a compatibility surface.

## Validation

Run:

```bash
python scripts/validate_tree_node_contracts.py
python mechanics/boundary-bridge/parts/public-mirror-sync/scripts/validate_tree_example_sync.py
python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/generate_kag_export.py
python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/validate_kag_export.py
```
