# AGENTS.md

This file applies to the generator and validator tools under `scripts/`.

## Read first

Before editing tools here, read:
1. the repository root `AGENTS.md`
2. `ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE.md`
3. `mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md`
4. `mechanics/audit/parts/review-ledger-route/docs/REVIEW_CHECKLIST.md`
5. `docs/decisions/AGENTS.md` when decision-index tooling is touched
6. the schema, example, intake, decision, or tree surfaces the script actually touches

## Local role

`scripts/` owns the bounded validator and generation machinery for the current ToS route.

These tools should:
- stay local
- stay deterministic
- stay source-owned
- make review easier rather than more magical

`validate_philosophy_topology.py` protects the domain-shaped `ToS/philosophy/`
branch so it cannot collapse into a flat import folder or inherit source UI
labels as repository topology.

## Operating Card

| Field | Route |
| --- | --- |
| role | deterministic generator and validator lane for current ToS surfaces |
| input | schema, example, intake, canon, decision, topology, or export surface |
| output | generated payload, generated index, or pass/fail validation signal |
| owner | `scripts/AGENTS.md` and the exact script being changed |
| next route | source surface -> script update -> generated artifact when needed -> validator |
| tools | local Python scripts, unittest, schema files, generated parity checks |
| check | affected script plus `python scripts/release_check.py` for broad changes |

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

## Boundary Routes

- Canonical source for authored meaning routes to `ToS/canon/` and
  `ToS/source-witnesses/`; compatibility examples remain mirrors.
- Generation logic follows contracts and owning source surfaces.
- Scope widening routes through a visible source, contract, decision, and
  validator change.
- Adjunct quest or progression behavior stays compatibility-only unless the
  owning ToS doctrine and contracts change.

## Validation

Run:

```bash
python scripts/validate_intake_pack.py
python scripts/validate_tiny_entry_route.py
python scripts/validate_philosophy_topology.py
python scripts/validate_nested_agents.py
python scripts/validate_tree_node_contracts.py
python scripts/validate_tree_relation_pack.py
python scripts/validate_tree_example_sync.py
python scripts/generate_kag_export.py
python scripts/validate_kag_export.py
python scripts/generate_decision_indexes.py --check
python scripts/validate_decision_records.py
```
