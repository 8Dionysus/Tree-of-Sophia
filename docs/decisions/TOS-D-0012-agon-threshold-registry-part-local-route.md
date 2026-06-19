# Agon Threshold Registry Part-Local Route

## Index Metadata

- Decision ID: TOS-D-0012
- Original date: 2026-06-17
- Surface classes: mechanics/agon, scripts/topology, tests/topology, docs/validation
- ToS layers: mechanics, scripts, tests, validation
- Tree classes: mechanics part, script inventory, test inventory, validation lanes
- Guard families: mechanics symmetry, script topology, test topology, owner boundary, command authority
- Posture: accepted

## Context

The Agon threshold registry already owned its config, schema, example, and
generated companion under `mechanics/agon/parts/threshold-registry/`. Its
builder, validator, and regression test still lived in root `scripts/` and
root `tests/`, even though their `owner_surface` was the part itself.

After `TOS-D-0010` and `TOS-D-0011`, ToS had enough topology coverage to make a
small part-local move without turning it into a blind mass migration. This part
is the first suitable pilot because the owner is clear and the builder,
validator, generated companion, and test form one bounded machine.

## Decision

Move the Agon threshold registry builder, validator, and regression test into
`mechanics/agon/parts/threshold-registry/`.

Add `scripts/run_mechanics_local_tests.py` as the ToS mechanics-local runner.
For this pilot it discovers `mechanics/*/parts/*/tests/test*.py`, then runs
related `build_*.py --check` and `validate_*.py` scripts from the same part
homes.

Add a blocking `mechanics_local` validation lane and include it in the
release lane. Remove the Agon threshold registry builder and validator from the
root `generated_parity` sequence so this part is checked through the part-local
route rather than a root-generated bucket.

Keep the move limited to Agon threshold registry. Other mechanics-owned root
scripts remain visible migration candidates in `script_inventory.json`, but do
not move until their package or part has an equally clear owner route.

## Options Considered

- Leave the builder, validator, and test in root surfaces. This keeps the
  current release route stable, but leaves a mechanics-owned part implemented as
  root machinery.
- Move all mechanics-owned root scripts at once. This would be noisy and would
  risk moving scripts before their part-local runner/test route is ready.
- Move only Agon threshold registry and add part-local discovery. This makes one
  complete part-owned machine local while proving the route for later moves.

## Rationale

Mechanics are organs of operation. When a part owns config, schema, generated
companion, builder, validator, and regression test, the machine should live with
the part instead of staying in root by inertia.

The runner keeps command authority in `docs/validation/validation_lanes.json`
while deriving part-local coverage from current source homes. This prevents a
frozen historical file list and keeps future mechanics migrations reviewable.

The move also keeps ToS philosophical source home clean: no ToS authored meaning
or canon authority moved into mechanics, and no runtime/proof authority is
created by the new lane.

## Consequences

The Agon threshold registry is now the first active ToS part-local mechanics
machine.

Future part-local migrations should include the same complete packet: owning
part route, builder or validator scripts, focused test, inventory updates,
validation lane coverage, and release verification.

Root generated parity no longer owns the Agon threshold registry check. The
`mechanics_local` lane owns that route.

## Source Surfaces

- `mechanics/agon/parts/threshold-registry/README.md`
- `mechanics/agon/parts/threshold-registry/scripts/build_tos_agon_threshold_intake_registry.py`
- `mechanics/agon/parts/threshold-registry/scripts/validate_tos_agon_threshold_intake_registry.py`
- `mechanics/agon/parts/threshold-registry/tests/test_tos_agon_threshold_intake_registry.py`
- `scripts/run_mechanics_local_tests.py`
- `docs/validation/validation_lanes.json`
- `docs/validation/script_inventory.json`
- `tests/test_inventory.json`
- `docs/testing/TEST_TOPOLOGY.md`
- `docs/validation/SCRIPT_TOPOLOGY.md`

## Validation

Run:

```bash
python scripts/run_mechanics_local_tests.py
python -m unittest tests.test_script_topology tests.test_test_topology tests.test_validation_lanes
python scripts/validate_nested_agents.py
python scripts/generate_decision_indexes.py --check
python scripts/validate_decision_records.py
python scripts/release_check.py
```
