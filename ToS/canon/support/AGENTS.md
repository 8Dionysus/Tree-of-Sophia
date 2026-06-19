# AGENTS.md

This file applies to canonical authored support nodes under `ToS/canon/support/`.

## What lives here

`ToS/canon/support/` is the home of canonical authored support nodes.

It holds:

- route-local support surfaces promoted from the reviewed `n.*` layer
- bounded carriers, symbols, places, hinges, and recipient-facing nodes that
  keep the route legible
- one `node.json` per stabilized support surface

## Editing posture

Treat these files as canonical authored tree law; raw candidate tables route to
`ToS/candidate-intake/`.
Keep each `node.json` bounded to one support surface.
Keep the source route explicit in `source_anchor`.
Keep support nodes route-local and bounded; wider semantic families route to a
reviewed philosophy or canon pass.
Keep `ToS/public-compatibility/support_node.example.json` aligned with the worked canonical
mirror as a compatibility surface.

## Validation

Run:

```bash
python scripts/validate_tree_node_contracts.py
python mechanics/boundary-bridge/parts/public-mirror-sync/scripts/validate_tree_example_sync.py
python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/generate_kag_export.py
python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/validate_kag_export.py
```
