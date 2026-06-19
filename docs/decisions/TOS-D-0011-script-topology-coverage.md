# Script Topology Coverage

## Index Metadata

- Decision ID: TOS-D-0011
- Original date: 2026-06-17
- Surface classes: docs/validation, scripts/topology, tests/topology
- ToS layers: docs, scripts, validation, tests
- Tree classes: script inventory, validation lanes, skill helper boundary
- Guard families: script topology, command authority, owner boundary, validator restraint
- Posture: accepted

## Context

`TOS-D-0009` moved release command authority into
`docs/validation/validation_lanes.json`, but `script_inventory.json` still
described only root scripts. The live repository already had additional active
script surfaces under `.agents/skills/*/scripts/` and `scripts/AGENTS.md`.

That left three risks:

- active script files could remain outside inventory coverage;
- skill-local helper scripts could be mistaken for ToS release gates or runtime
  policy;
- mechanics-owned root scripts could stay visible only as generic root tools,
  making a later part-local move harder to review.

The deeper sibling pattern in `aoa-techniques` treats scripts as command-plane
organs with owner surface, source truth, reads, writes, side effects, lane
posture, CI inclusion, and focused test target. ToS needs the same shape, but
with ToS boundaries: scripts serve the philosophical source home and mechanics;
they do not become doctrine, graph runtime, proof verdict, MCP service, or
skill execution authority.

## Decision

Add `docs/validation/SCRIPT_TOPOLOGY.md` as the descriptive route map for ToS
script surfaces.

Promote `docs/validation/script_inventory.json` to `tos_script_inventory_v2`
and require it to cover every active non-pyc file under `*/scripts/*`, including
`scripts/AGENTS.md` and `.agents/skills/*/scripts/*.py`.

Represent validator commands in the same script inventory when they live under
`*/scripts/*`. A separate validator inventory becomes useful only after ToS
grows a distinct validator-module surface that carries information beyond
script topology and lane authority.

Add `tests/test_script_topology.py` and `tests/support/script_inventory.py` to
guard inventory completeness, required fields, lane-command references,
side-effect visibility, and advisory skill helper boundaries.

Keep `.agents/skills/*/scripts/*.py` as `skill_local_contract_tool` entries with
`advisory-only` CI posture. They are allowed as deterministic local helper
contracts, but they are not ToS release commands, runtime policy engines, or
hidden hard gates.

Keep mechanics-owned root scripts in root `scripts/` for this slice. Their
`owner_surface` and `organ_lane` now make the part or package ownership visible.
A later part-local move should happen only when the owning mechanic has the
builder, validator, test, and runner route ready.

## Options Considered

- Keep the root-only script inventory. This avoids churn, but leaves active
  script surfaces and skill helpers outside the topology guard.
- Move mechanics-owned scripts immediately into part-local homes. This copies
  the sibling shape too early and risks moving files before ToS has a local
  part-runner contract.
- Add full script topology coverage first. This keeps behavior stable while
  making future moves reviewable and testable.

## Rationale

Scripts are machinery, not source meaning. ToS needs to know which machine owns
which movement before moving the machines.

The inventory v2 shape keeps command execution with
`docs/validation/validation_lanes.json`, while making each script's owner,
source truth, side effects, and CI posture explicit. This prevents both hidden
hard gates and unowned helper sprawl.

Advisory skill helper scripts stay visible without being promoted. Mechanics
part-owned scripts stay visible without forcing a premature filesystem move.

## Consequences

New files under `*/scripts/*` must receive inventory entries before they are
treated as active.

Changing a script's owner surface, side effects, validation lane, or CI posture
requires updating `script_inventory.json`.

Skill-local helper scripts remain advisory unless a future decision promotes one
concrete ToS-owned check into a blocking lane.

Mechanics-owned root scripts are now explicit migration candidates. The next
slice can decide which of them deserve part-local homes and runner discovery.

Validator coverage now has one descriptive surface: `script_inventory.json`.
This reduces inventory duplication while keeping command authority in
`validation_lanes.json`.

## Source Surfaces

- `docs/validation/SCRIPT_TOPOLOGY.md`
- `docs/validation/script_inventory.json`
- `docs/validation/validation_lanes.json`
- `docs/validation/README.md`
- `scripts/AGENTS.md`
- `tests/test_script_topology.py`
- `tests/support/script_inventory.py`
- `.agents/skills/*/scripts/*.py`
- `docs/decisions/TOS-D-0009-validation-lane-command-authority.md`
- `docs/decisions/TOS-D-0010-test-topology-coverage.md`

## Validation

Run:

```bash
python -m unittest tests.test_script_topology
python -m unittest tests.test_test_topology
python -m unittest discover -s tests
python scripts/generate_decision_indexes.py --check
python scripts/validate_decision_records.py
python scripts/release_check.py
```
