# AGENTS.md

This file applies to canonical authored analogy nodes under `ToS/canon/analogy/`.

## What lives here

`ToS/canon/analogy/` is the home of canonical authored analogy nodes.

It holds:

- route-local image-bearing analogy surfaces promoted from reviewed intake
- bounded symbolic comparisons that have crossed the review boundary
- one `node.json` per stabilized analogy surface

## Editing posture

Treat these files as canonical authored tree law; raw candidate tables route to
`ToS/candidate-intake/`.
Keep each `node.json` bounded to one analogy surface.
Keep the source route explicit in `source_anchor`.
Keep analogy nodes route-local and image-bearing; doctrine-wide claims route to
`ToS/doctrine/`.
Keep `ToS/public-compatibility/analogy_node.example.json` aligned with the worked canonical
mirror as a compatibility surface.

## Validation

Run:

```bash
python scripts/validate_tree_node_contracts.py
python mechanics/boundary-bridge/parts/public-mirror-sync/scripts/validate_tree_example_sync.py
python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/generate_kag_export.py
python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/validate_kag_export.py
```
