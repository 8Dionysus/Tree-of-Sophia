# AGENTS.md

This file applies to canonical authored synthesis nodes under `ToS/canon/synthesis/`.

## What lives here

`ToS/canon/synthesis/` is the home of canonical authored synthesis nodes.

It holds:

- route-local interpretive synthesis surfaces promoted from reviewed intake
- bounded commentary-like readings that now have a canonical authored home
- one `node.json` per stabilized synthesis surface

## Editing posture

Treat these files as canonical authored tree law; raw candidate tables route to
`ToS/candidate-intake/`.
Keep each `node.json` bounded to one synthesis surface.
Keep the source route explicit in `source_anchor`.
Keep synthesis nodes route-local, source-linked, and explicitly interpretive;
principle and source canon keep their own routes.
Keep `ToS/public-compatibility/synthesis_node.example.json` aligned with the worked canonical
mirror as a compatibility surface.

## Validation

Run:

```bash
python scripts/validate_tree_node_contracts.py
python mechanics/boundary-bridge/parts/public-mirror-sync/scripts/validate_tree_example_sync.py
python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/generate_kag_export.py
python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/validate_kag_export.py
```
