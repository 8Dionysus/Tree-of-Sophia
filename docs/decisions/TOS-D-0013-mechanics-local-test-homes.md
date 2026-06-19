# Mechanics Local Test Homes

## Index Metadata

- Decision ID: TOS-D-0013
- Original date: 2026-06-17
- Surface classes: mechanics/experience, mechanics/agon, scripts/topology, tests/topology, docs/validation
- ToS layers: mechanics, scripts, tests, validation
- Tree classes: mechanics package, mechanics part, command lane, test inventory, script inventory
- Guard families: mechanics symmetry, test topology, script topology, owner boundary, command authority
- Posture: accepted

## Context

After the Agon threshold registry moved into a part-local home, the next
mechanics-owned tests still lived in root `tests/`. The Experience contract
tests were not all part-local: some cover several Experience parts that form one
package-level boundary family.

Keeping only a `mechanics_part_local` lane would make the topology lie as soon
as package-level mechanics tests moved out of root.

## Decision

Use `mechanics_local` as the validation lane for mechanics-owned local homes.
The lane covers both package-local homes such as `mechanics/experience/tests/`
and part-local homes such as
`mechanics/agon/parts/threshold-registry/tests/`.

Rename the runner to `scripts/run_mechanics_local_tests.py`. It discovers
mechanic-level tests, part-local tests, and local mechanics builders or
validators from the filesystem, while command authority remains in
`docs/validation/validation_lanes.json`.

Move Experience contract tests to `mechanics/experience/tests/` because their
owner is the Experience package route, not root `tests/` and not one single
Experience part.

## Options Considered

- Keep the lane named `mechanics_part_local`. This preserves the first pilot
  name but misdescribes package-local Experience tests.
- Split every Experience test into individual parts. This would force topology
  symmetry where the contracts currently span several boundary parts.
- Promote a broader `mechanics_local` lane. This keeps part-local Agon checks
  and package-local Experience checks under one honest mechanics-local route.

## Rationale

Mechanics are operation machines. Some machines are small enough to live in one
part; others are package-level because they coordinate several parts. The
topology should name that difference instead of flattening it into root tests or
forcing every contract into a part.

The runner is still descriptive discovery, not doctrine. It does not decide
what Experience means, what Agon means, or which checks are release blockers;
those decisions stay in mechanics route cards and the validation lane manifest.

## Consequences

Root `tests/` is no longer the only active test home.

Experience contract tests now travel with `mechanics/experience/`, and future
mechanics package tests can use the same package-local shape when a contract
spans several parts.

Part-local migrations still need a complete owner packet: local route card,
source payload, test, builder or validator when relevant, inventory entry, and
release-lane coverage.

## Source Surfaces

- `mechanics/experience/AGENTS.md`
- `mechanics/experience/tests/`
- `mechanics/agon/parts/threshold-registry/`
- `scripts/run_mechanics_local_tests.py`
- `docs/validation/validation_lanes.json`
- `docs/validation/SCRIPT_TOPOLOGY.md`
- `docs/testing/TEST_TOPOLOGY.md`
- `docs/validation/script_inventory.json`
- `tests/test_inventory.json`

## Validation

Run:

```bash
python scripts/run_mechanics_local_tests.py
python -m unittest discover -s mechanics/experience/tests
python -m unittest tests.test_script_topology tests.test_test_topology tests.test_validation_lanes
python scripts/validate_mechanics_topology.py
python scripts/validate_nested_agents.py
python scripts/generate_decision_indexes.py --check
python scripts/validate_decision_records.py
python scripts/release_check.py
```
