# AGENTS.md

This file applies to the generator and validator tools under `scripts/`.

## Read first

Before editing tools here, read:
1. the repository root `AGENTS.md`
2. `docs/TINY_ENTRY_ROUTE.md`
3. `docs/KAG_EXPORT.md`
4. `docs/REVIEW_CHECKLIST.md`
5. the schema, example, intake, or tree surfaces the script actually touches

## Local role

`scripts/` owns the bounded validator and generation machinery for the current ToS route.

These tools should:
- stay local
- stay deterministic
- stay source-owned
- make review easier rather than more magical

## Editing posture

Keep the pilot narrow to the current Zarathustra route.

Prefer:
- explicit file dependencies
- clear exit conditions
- reproducible transforms
- visible validation boundaries

Avoid:
- network calls
- hidden state
- broad corpus automation
- discovery magic that blurs ownership
- turning validators into a runtime or orchestration control plane

The scripts should serve the source-first route, not become a kingdom of their own.

## Hard no

Do not:
- silently treat `examples/` as canonical source
- let generation logic outrun the contracts it is supposed to protect
- hide widening scope behind convenience flags
- add adjunct quest or progression behavior that changes semantic authority

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
