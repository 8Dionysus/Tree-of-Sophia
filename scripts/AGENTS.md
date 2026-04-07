# AGENTS.md

This file applies to the small generator and validator tools under `scripts/`.

## What lives here

`scripts/` owns the bounded tiny-export seam and its local documentation guardrails.
The current scripts are:

- `validate_intake_pack.py`
- `validate_tiny_entry_route.py`
- `validate_tree_node_contracts.py`
- `validate_tree_relation_pack.py`
- `generate_kag_export.py`
- `tree_example_sync.py`
- `sync_tree_examples.py`
- `validate_tree_example_sync.py`
- `validate_kag_export.py`
- `validate_nested_agents.py`

## Editing posture

Keep these tools deterministic, local, and source-owned.
Do not add network calls, hidden state, or broad corpus automation here.
Keep the pilot narrow to the current Zarathustra route.
Prefer explicit file dependencies over discovery magic.
Keep tree-to-example compatibility explicit rather than silently treating `examples/` as the canonical source.

## Validation

Run:

```bash
python scripts/validate_intake_pack.py
python scripts/validate_tiny_entry_route.py
python scripts/validate_nested_agents.py
python scripts/validate_tree_node_contracts.py
python scripts/validate_tree_relation_pack.py
python scripts/validate_tree_example_sync.py
python scripts/generate_kag_export.py
python scripts/validate_kag_export.py
```
