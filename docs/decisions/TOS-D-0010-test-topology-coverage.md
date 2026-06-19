# Test Topology Coverage

## Index Metadata

- Decision ID: TOS-D-0010
- Original date: 2026-06-17
- Surface classes: docs/testing, tests/topology, docs/validation
- ToS layers: docs, tests, validation
- Tree classes: test inventory, validation lanes, mechanics contracts
- Guard families: test topology, command authority, owner boundary, validator restraint
- Posture: accepted

## Context

`TOS-D-0009` created the validation lane command authority and kept
`tests/test_inventory.json` as descriptive coverage rather than command
authority. That first lane split removed hidden command pressure, but the test
inventory remained too flat for the next mechanics refactor:

- active test files could exist without inventory coverage;
- mechanics-owned tests still lived in root `tests/` without a visible home
  scope;
- failure routes were implicit;
- no focused test protected the test inventory itself from drifting back into a
  historical list.

The deeper sibling pattern in `aoa-techniques` shows that test topology should
answer what boundary is protected, which owner surface is authoritative, where
the test lives, which coverage authority reaches it, and where failure routes
next. ToS needs that shape, but without forcing premature movement of tests into
mechanics packages or part-local homes before the owning surfaces are ready.

## Decision

Add `docs/testing/TEST_TOPOLOGY.md` as the descriptive route map for ToS tests.

Promote `tests/test_inventory.json` to a richer descriptive inventory that
records each active test file's home, home scope, owner surface, validation lane,
coverage authority, focused target, runtime cost, failure route, and
disposition.

Add `tests/test_test_topology.py` and `tests/support/topology_inventory.py` so
the inventory must cover every active `tests/test*.py` file and must not store
release command sequences.

Keep all current tests in the root `tests/` home for this slice. Mechanics-owned
root tests remain allowed when their `owner_surface` and `validation_lane` make
the route explicit. Future `mechanic-level` or `part-local` test homes should be
introduced only when the mechanic package or part has become the real owner of
that regression surface.

## Options Considered

- Keep the flat test inventory. This avoids churn, but leaves ToS without a
  guard against missing test coverage records or hidden command authority in the
  test map.
- Move mechanics-owned tests immediately under `mechanics/`. This would mimic
  sibling shape too early and risk moving tests before the package-local script
  and runner topology is ready.
- Add test topology and inventory coverage first. This keeps the current test
  home stable while making the next mechanics/script split reviewable.

## Rationale

Tests should be route-bound proof organs, not a second command authority and not
a warehouse of files. A separate topology document gives future agents the map
without bloating root docs.

The richer inventory makes root-owned and mechanics-owned tests visible by owner
surface and failure route. That is enough to support the next slice: deciding
which mechanics-owned root tests and scripts are mature enough to move into
package or part-local homes.

This keeps command execution with `docs/validation/validation_lanes.json`, keeps
current tests runnable through the existing root test route, and avoids treating
symmetry with sibling repos as a reason to force premature movement.

## Consequences

New test files must receive inventory entries before they are treated as active.

Changing a test's owner surface, home scope, validation lane, or failure route
requires updating `tests/test_inventory.json`.

Mechanics-owned root tests are now explicit debt rather than hidden drift. They
can remain in root `tests/` until a package or part-local route is ready to own
them.

The inventory adds maintenance overhead, but `tests/test_test_topology.py` turns
that overhead into a clear failure route instead of quiet topology decay.

## Source Surfaces

- `docs/testing/TEST_TOPOLOGY.md`
- `tests/AGENTS.md`
- `tests/test_inventory.json`
- `tests/test_test_topology.py`
- `tests/support/topology_inventory.py`
- `docs/validation/README.md`
- `docs/validation/validation_lanes.json`
- `docs/decisions/TOS-D-0009-validation-lane-command-authority.md`

## Validation

Run:

```bash
python -m unittest tests.test_test_topology
python -m unittest tests.test_validation_lanes
python -m unittest discover -s tests
python scripts/generate_decision_indexes.py --check
python scripts/validate_decision_records.py
python scripts/release_check.py
```
