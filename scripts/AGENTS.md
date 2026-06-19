# AGENTS.md

This file applies to the generator and validator tools under `scripts/`.

## Read first

Before editing tools here, read:
1. the repository root `AGENTS.md`
2. `docs/validation/validation_lanes.json` for command authority
3. `docs/validation/SCRIPT_TOPOLOGY.md` and `docs/validation/script_inventory.json`
4. `ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE.md`
5. `mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md`
6. `mechanics/audit/parts/review-ledger-route/docs/REVIEW_CHECKLIST.md`
7. `docs/decisions/AGENTS.md` when decision-index tooling is touched
8. the schema, example, intake, decision, or tree surfaces the script actually touches

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

`build_tos_corpus_index.py` and `validate_tos_corpus_index.py` publish the
checked whole-corpus index for graph review. They index the whole `ToS/` home
as a derived resource map; they do not move runtime projection or visualization
authority into Tree of Sophia.

## Operating Card

| Field | Route |
| --- | --- |
| role | deterministic generator and validator lane for current ToS surfaces |
| input | schema, example, intake, canon, decision, topology, or export surface |
| output | generated payload, generated index, or pass/fail validation signal |
| owner | `scripts/AGENTS.md` and the exact script being changed |
| next route | source surface -> script update -> generated artifact when needed -> validator |
| tools | local Python scripts, unittest, schema files, generated parity checks |
| check | affected script plus the release lane in `docs/validation/validation_lanes.json` for broad changes |

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
- broad corpus automation without an explicit derived-export contract
- discovery magic that blurs ownership
- turning validators into a runtime or orchestration control plane

The scripts should serve the source-first route, not become a kingdom of their own.

`scripts/release_check.py` is a runner for the `release_check` command sequence
declared in `docs/validation/validation_lanes.json`. Keep command composition
there, not as a hidden list inside Python code.

Every active file under `*/scripts/*` must stay represented in
`docs/validation/script_inventory.json`. That inventory describes owner,
source truth, side effects, lane posture, CI inclusion, and focused test target;
it does not execute commands or promote advisory skill helpers into release
authority.

## Boundary Routes

- Canonical source for authored meaning routes to `ToS/canon/` and
  `ToS/source-witnesses/`; compatibility examples remain mirrors.
- Generation logic follows contracts and owning source surfaces.
- Scope widening routes through a visible source, contract, decision, and
  validator change.
- Adjunct quest or progression behavior stays compatibility-only unless the
  owning ToS doctrine and contracts change.

## Validation

Run the affected script directly. For broad or release-visible changes, run the
repo gate:

```bash
python scripts/release_check.py
```

Local owner routes:

| Pressure | Route |
| --- | --- |
| public tiny entry | `python scripts/validate_tiny_entry_route.py` |
| bounded KAG export | `python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/generate_kag_export.py` when inputs move, then `python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/validate_kag_export.py` |
| questbook surface | `python mechanics/questbook/scripts/validate_questbook_surface.py` |
| corpus index | `python scripts/build_tos_corpus_index.py --check` and `python scripts/validate_tos_corpus_index.py` |
| decision indexes | `python scripts/generate_decision_indexes.py --check` and `python scripts/validate_decision_records.py` |
| source-home or branch topology | `python scripts/validate_tos_source_home.py` and `python scripts/validate_philosophy_topology.py` |
| canon/example contracts | `python scripts/validate_tree_node_contracts.py`, `python mechanics/relation-weaving/parts/graph-promotion/scripts/validate_tree_relation_pack.py`, or `python mechanics/boundary-bridge/parts/public-mirror-sync/scripts/validate_tree_example_sync.py` |
