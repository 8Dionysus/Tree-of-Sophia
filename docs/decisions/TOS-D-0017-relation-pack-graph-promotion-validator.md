# Relation Pack Graph Promotion Validator

## Index Metadata

- Decision ID: TOS-D-0017
- Original date: 2026-06-18
- Surface classes: mechanics/relation-weaving, scripts/topology, ToS/canon/relations, docs/validation
- ToS layers: mechanics, scripts, canon, candidate-intake, validation
- Tree classes: mechanics part, relation pack, script inventory, validation lanes
- Guard families: mechanics symmetry, owner boundary, script topology, command authority, canon promotion
- Posture: accepted

## Context

`validate_tree_relation_pack.py` checks the route-local canonical relation pack
against promoted rows from the current candidate-intake ledger and canon
registries. The checked material lives in `ToS/canon/relations/` and
`ToS/candidate-intake/`, but the repeated operation is graph promotion: deciding
whether a relation carrier stays aligned with the reviewed promoted subset.

`mechanics/relation-weaving/parts/graph-promotion/` already existed for that
operation and named the relation-pack validator as its tool. Keeping the
validator in root `scripts/` hid the active mechanic owner after mechanics-local
script discovery had become available.

## Decision

Move the relation-pack validator to
`mechanics/relation-weaving/parts/graph-promotion/scripts/validate_tree_relation_pack.py`.

Keep `scripts/validate_intake_pack.py` in root `scripts/` as the
candidate-intake source-home gate. The graph-promotion validator may import its
helpers, but this does not move candidate-intake ownership into Relation
Weaving.

Keep the blocking lane as `canon_contracts`, with command authority in
`docs/validation/validation_lanes.json`.

## Options Considered

- Leave the validator in root `scripts/`. This keeps the old command short, but
  hides the graph-promotion operation that already has a mechanics part.
- Move the validator under `ToS/canon/relations/`. This would place machinery
  inside the canonical material home and blur source payload with operation.
- Move the validator under Graph Promotion. This names the operation while
  keeping canonical relation payloads and candidate ledgers in their stronger
  ToS homes.

## Rationale

Relation Weaving owns the operation around graph fragments and relation-pack
promotion. It does not own graph objects, canon relation payloads, or
candidate-intake ledgers.

The new path says that distinction directly: mechanics owns the movement, ToS
owns the material. `canon_contracts` still protects the release-facing canon
contract lane.

## Consequences

Root `scripts/` no longer carries the relation-pack promotion validator.

Future changes to relation-pack promotion must update the Graph Promotion part,
the relevant ToS canon/candidate surfaces, inventories, and validation lanes
together.

The candidate-intake validator remains a root source-home validator until a
future intake operation becomes a real mechanics owner rather than a source-home
gate.

## Source Surfaces

- `mechanics/relation-weaving/parts/graph-promotion/README.md`
- `mechanics/relation-weaving/parts/graph-promotion/scripts/validate_tree_relation_pack.py`
- `ToS/doctrine/RELATION_PACK_CONTRACT.md`
- `ToS/canon/relations/`
- `ToS/candidate-intake/`
- `scripts/validate_intake_pack.py`
- `docs/validation/validation_lanes.json`
- `docs/validation/script_inventory.json`
- `docs/validation/SCRIPT_TOPOLOGY.md`

## Validation

Run:

```bash
python mechanics/relation-weaving/parts/graph-promotion/scripts/validate_tree_relation_pack.py
python scripts/validate_intake_pack.py
python scripts/run_mechanics_local_tests.py
python -m unittest tests.test_script_topology tests.test_docs_verify_routes tests.test_validation_lanes
python scripts/generate_decision_indexes.py --check
python scripts/validate_decision_records.py
python scripts/release_check.py
```
