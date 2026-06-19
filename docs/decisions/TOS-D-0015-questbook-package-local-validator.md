# Questbook Package Local Validator

## Index Metadata

- Decision ID: TOS-D-0015
- Original date: 2026-06-17
- Surface classes: mechanics/questbook, scripts/topology, tests/topology, docs/validation
- ToS layers: mechanics, scripts, tests, validation
- Tree classes: mechanics package, public obligation record, script inventory, test inventory, validation lanes
- Guard families: mechanics symmetry, script topology, test topology, command authority, owner boundary
- Posture: accepted

## Context

`TOS-D-0009` split Questbook checks out of the KAG export validator and gave
them a dedicated `questbook_surface` lane. After the mechanics-local runner and
test topology landed, the remaining mismatch was physical: the Questbook
validator and its focused tests still lived in root `scripts/` and root
`tests/`, while their owner surface was already `mechanics/questbook/AGENTS.md`.

Questbook validates root `QUESTBOOK.md` and root `quests/*.yaml`, but those
files are public obligation records, not proof that the validator belongs to
root machinery.

## Decision

Move the Questbook surface validator to
`mechanics/questbook/scripts/validate_questbook_surface.py`.

Move its focused behavior tests to
`mechanics/questbook/tests/test_validate_questbook_surface.py`.

Keep the blocking command lane named `questbook_surface`, and keep command
authority in `docs/validation/validation_lanes.json`. Mechanics-local discovery
may also run the local test and validator, but it does not replace the explicit
Questbook lane in release.

## Options Considered

- Leave the validator and tests in root homes. This preserves the old short
  command, but keeps a mechanics-owned operation as root noise.
- Move the validator into one Questbook part. This overfits the check: the
  validator spans obligation boundary, dispatch contracts, root quest records,
  and the public index.
- Move the validator and tests to the Questbook package home. This names the
  owner honestly while preserving root `QUESTBOOK.md` and `quests/` as input
  source records.

## Rationale

Questbook is a package-level operation. Its parts own obligation boundary and
dispatch contracts, while the package owns the compatibility check that spans
both parts and the public obligation records.

The move follows the mechanics pattern without forcing a part split where the
operation is package-wide. It also keeps validators from becoming root doctrine:
the validator checks public obligation compatibility, not philosophical
authority, quest execution, or source meaning.

## Consequences

Root `scripts/` no longer carries the Questbook validator.

Root `tests/` no longer carries the Questbook validator behavior test.

Future Questbook validation that spans several Questbook parts should stay at
`mechanics/questbook/scripts/` and `mechanics/questbook/tests/`. A future
single-part Questbook builder, validator, or test can move under that part only
when the operation becomes truly part-local.

## Source Surfaces

- `mechanics/questbook/AGENTS.md`
- `mechanics/questbook/README.md`
- `mechanics/questbook/PARTS.md`
- `mechanics/questbook/scripts/validate_questbook_surface.py`
- `mechanics/questbook/tests/test_validate_questbook_surface.py`
- `QUESTBOOK.md`
- `quests/`
- `docs/validation/validation_lanes.json`
- `docs/validation/script_inventory.json`
- `docs/testing/TEST_TOPOLOGY.md`
- `tests/test_inventory.json`

## Validation

Run:

```bash
python mechanics/questbook/scripts/validate_questbook_surface.py
python -m unittest discover -s mechanics/questbook/tests -p 'test*.py'
python scripts/run_mechanics_local_tests.py
python -m unittest tests.test_script_topology tests.test_test_topology tests.test_validation_lanes
python scripts/generate_decision_indexes.py --check
python scripts/validate_decision_records.py
python scripts/release_check.py
```
