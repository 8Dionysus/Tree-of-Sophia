# AGENTS.md

This file applies to canonical authored analogy nodes under `tree/analogy/`.

## What lives here

`tree/analogy/` is the home of canonical authored analogy nodes.

It holds:

- route-local image-bearing analogy surfaces promoted from reviewed intake
- bounded symbolic comparisons that have crossed the review boundary
- one `node.json` per stabilized analogy surface

## Editing posture

Treat these files as canonical authored tree law, not as raw candidate tables.
Keep each `node.json` bounded to one analogy surface.
Keep the source route explicit in `source_anchor`.
Keep analogy nodes route-local and image-bearing rather than turning them into
free-floating doctrine.
Keep `examples/analogy_node.example.json` aligned with the worked canonical
mirror rather than creating a second canon.

## Validation

Run:

```bash
python scripts/validate_tree_node_contracts.py
python scripts/validate_tree_example_sync.py
python scripts/generate_kag_export.py
python scripts/validate_kag_export.py
```
