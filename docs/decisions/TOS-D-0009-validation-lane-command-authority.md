# Validation Lane Command Authority

## Index Metadata

- Decision ID: TOS-D-0009
- Original date: 2026-06-13
- Surface classes: docs/validation, scripts/validation, tests/topology, mechanics/topology, ToS/source-home
- ToS layers: docs, scripts, tests, mechanics, source-home, generated
- Tree classes: source-home manifest, mechanics topology, validation lanes, test inventory
- Guard families: validator restraint, command authority, owner boundary, generated parity, route-card structure
- Posture: accepted

## Context

`TOS-D-0008` unloaded command pressure from the root README but left the deeper
validator topology refactor for later. The next pressure appeared in three
places:

- `scripts/release_check.py` owned a hidden hardcoded release command list;
- `ToS/source_home.manifest.json` stored shell commands inside branch records;
- `scripts/validate_nested_agents.py` froze ordinary route-card prose as exact
  required phrases.

The mechanics topology validator also carried moved-path old-to-new accounting
inside Python code, which made the validator act partly as the manifest it was
supposed to check.

The same command-pressure pattern also appeared in the public-entry lane:
`scripts/validate_kag_export.py` checked questbook obligation and dispatch
surfaces even though those surfaces belong to `mechanics/questbook/`, not to
the KAG export seam.

## Decision

Create `docs/validation/validation_lanes.json` as the executable command
authority for named ToS validation lanes.

Keep `docs/validation/validator_inventory.json`,
`docs/validation/script_inventory.json`, and `tests/test_inventory.json` as
descriptive coverage maps only. They do not own command execution.

Make `scripts/release_check.py` load the `release_check` command sequence from
the lane manifest. Move source-home branch validation from shell commands to
lane ids. Keep mechanics moved-path target data in `mechanics/topology.json`
and make the validator check that manifest data.

Refactor `scripts/validate_nested_agents.py` into a structural route-card
checker: required files, headings, operating-card fields where present, and
stable path or id references. Exact ordinary prose is not authority.

Split questbook obligation and dispatch checks into
`scripts/validate_questbook_surface.py` and route them through a
`questbook_surface` lane. Keep `scripts/validate_kag_export.py` focused on the
bounded KAG export seam.

Reduce the remaining tiny-entry prose checks to stable route tokens: headings,
repo-relative surfaces, downstream boundary names, and validator entrypoints.

## Rationale

ToS validators should be organs of route verification, not hidden doctrine.
Executable command order belongs in one explicit lane authority. Inventories
help review coverage but must remain weaker than the lane manifest and weaker
than the source surfaces they describe.

This keeps `ToS/` a philosophical source home, leaves mechanics in
`mechanics/`, keeps local eval pressure in `evals/`, and gives future graph and
export work a named lane without moving runtime visualization authority into
Tree of Sophia.

## Consequences

`docs/validation/validation_lanes.json` is now the local command-authority
surface for validation lanes.

`scripts/validation_lanes.py` is only the loader and checker for that manifest.

`scripts/release_check.py` no longer owns a hidden command list.

`ToS/source_home.manifest.json` records `validation_lanes` per branch instead
of shell commands.

`tests/AGENTS.md` and `tests/test_inventory.json` identify what tests protect
without making the inventory command authority.

`mechanics/topology.json` now stores moved-path targets, and
`scripts/validate_mechanics_topology.py` checks those targets instead of
carrying the old-to-new map in code.

`scripts/validate_questbook_surface.py` now owns the questbook surface check.
The release lane runs it explicitly instead of receiving questbook coverage
through the KAG export validator.

`scripts/validate_tiny_entry_route.py` still protects the bounded public entry
route, but it no longer treats ordinary explanatory sentences as required
authority.

`evals/` remains a local port. Central proof verdicts and scoring remain with
`aoa-evals`.

## Source Surfaces

- `docs/validation/README.md`
- `docs/validation/validation_lanes.json`
- `docs/validation/validator_inventory.json`
- `docs/validation/script_inventory.json`
- `scripts/validation_lanes.py`
- `scripts/release_check.py`
- `scripts/validate_tos_source_home.py`
- `scripts/validate_nested_agents.py`
- `scripts/validate_mechanics_topology.py`
- `scripts/validate_tiny_entry_route.py`
- `scripts/validate_kag_export.py`
- `scripts/validate_questbook_surface.py`
- `ToS/source_home.manifest.json`
- `ToS/contracts/tos-source-home.schema.json`
- `mechanics/topology.json`
- `mechanics/questbook/AGENTS.md`
- `mechanics/questbook/README.md`
- `tests/AGENTS.md`
- `tests/test_inventory.json`
- `tests/test_validation_lanes.py`
- `tests/test_validate_questbook_surface.py`
- `tests/test_tos_source_home_schema.py`

## Validation

Run:

```bash
python scripts/validation_lanes.py --check
python scripts/validate_tos_source_home.py
python -m unittest tests.test_tos_source_home_schema
python scripts/validate_mechanics_topology.py
python scripts/validate_nested_agents.py
python scripts/validate_questbook_surface.py
python scripts/generate_decision_indexes.py --check
python scripts/validate_decision_records.py
python scripts/release_check.py
python -m unittest discover -s tests
```
