# AGENTS.md

This file applies to canonical authored tree surfaces under `tree/`.

## What lives here

`tree/` is the canonical authored tree layer for Tree of Sophia.

It holds:

- canonical authored source nodes
- canonical authored concept nodes
- canonical authored principle nodes
- canonical authored lineage nodes
- canonical vocabulary governance registries for the tabular graph layer
- future canonical authored context or synthesis nodes

The current wave uses `node.json` files as the canonical authored tree payloads.

## Editing posture

Treat `tree/` as the canonical authored tree, not as raw witness storage and not
as a derived export layer.
Keep one authored object per directory-scoped `node.json`.
Keep `examples/source_node.example.json` and `examples/concept_node.example.json`
and `examples/principle_node.example.json` and
`examples/lineage_node.example.json` as compatibility mirrors of the canonical
tree rather than a second canon.
Keep relation names explicit and keep node identity stable.

## Validation

Run:

```bash
python scripts/validate_tree_node_contracts.py
python scripts/validate_intake_pack.py
python scripts/validate_tree_example_sync.py
python scripts/generate_kag_export.py
python scripts/validate_kag_export.py
```
