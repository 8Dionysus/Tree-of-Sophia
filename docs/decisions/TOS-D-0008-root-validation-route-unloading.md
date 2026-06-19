# Root Validation Route Unloading

## Index Metadata

- Decision ID: TOS-D-0008
- Original date: 2026-06-10
- Surface classes: docs/route-law, scripts/validation, tests/topology
- ToS layers: docs, scripts, tests, root
- Tree classes: none
- Guard families: validator restraint, owner boundary, route-law, source-first authority
- Posture: accepted

## Context

The root README and roadmap had become constrained by tests and validators that
searched for exact command strings, exact prose fragments, and script-path
inventories. That made green validation push command noise back into public
orientation surfaces.

The same pressure made `validate_active_naming.py` police ordinary prose and
historical release notes instead of only active path or identifier references.

## Decision

Keep root README and roadmap as route surfaces. They may point to owners, but
they should not carry executable command inventories.

Move executable validation authority to `AGENTS.md`, `scripts/AGENTS.md`, local
route cards, and `scripts/release_check.py`. Adjust tests and validators so
they protect owner routing:

- README must link to current public route surfaces and validation owners;
- README must not carry `python scripts/...` command text;
- ROADMAP must preserve current release contour meaning without listing script
  paths as release philosophy;
- active naming validation checks active paths and path-like identifiers, not
  ordinary historical prose.

## Options Considered

- Keep old tests and leave a transitional command block in README. This keeps
  validation green but makes the public entry surface noisier every time root
  docs are cleaned.
- Remove route checks entirely. This loses useful protection for the current
  public route.
- Retain route checks while making command authority explicit in owner
  surfaces.

## Rationale

Validators in ToS should protect owner boundaries, source-returning routes,
generated parity, and regressions. They should not make public orientation docs
carry command inventories or present exact prose matches as philosophical
truth.

This keeps the README close to the mature AoA-family pattern: public front door
first, executable authority in route cards and validation surfaces.

## Consequences

`README.md` no longer carries the current bounded validation battery.

Root entrypoint docs now keep distinct jobs:

- `README.md` is the public route map;
- `AGENTS.md` is the agent execution route;
- `DESIGN.md` is the system-shape note;
- `DESIGN.AGENTS.md` is the route-card shape note;
- `ROADMAP.md` is the current contour and next pressure.

Detailed branch, mechanics, validation, test, release, and decision surfaces
own their local maps instead of being duplicated in root docs.

`scripts/validate_tiny_entry_route.py` now checks README route links and bans
command text in README.

`tests/test_docs_verify_routes.py` now asserts route-to-owner behavior instead
of command presence.

`scripts/validate_active_naming.py` ignores `CHANGELOG.md` and checks active
path/id references rather than every occurrence of retired words.

`scripts/validate_nested_agents.py` remains phrase-based overall. This decision
only unloads the harmful `scripts/AGENTS.md` command-dump pressure; a deeper
nested-agent validator topology refactor remains future work.

## Source Surfaces

- `README.md`
- `AGENTS.md`
- `ROADMAP.md`
- `DESIGN.md`
- `DESIGN.AGENTS.md`
- `CONTRIBUTING.md`
- `scripts/AGENTS.md`
- `scripts/validate_tiny_entry_route.py`
- `scripts/validate_active_naming.py`
- `scripts/validate_nested_agents.py`
- `tests/test_docs_verify_routes.py`
- `tests/test_current_direction_routes.py`
- `tests/test_roadmap_parity.py`
- `tests/test_validate_tiny_entry_route.py`

## Validation

Run:

```bash
python scripts/generate_decision_indexes.py
python scripts/generate_decision_indexes.py --check
python scripts/validate_decision_records.py
python scripts/release_check.py
```
