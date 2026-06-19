# AGENTS.md

## Applies To

This card applies to `tests/`.

## Role

`tests/` owns repo-local regression checks for ToS route surfaces, generated
parity, canon contracts, mechanics payload contracts, validator behavior, test
topology, and release contour. Tests prove bounded behavior; they do not
promote candidate material, create doctrine, store command authority, or
replace eval/proof authority.

## Operating Card

| Field | Route |
| --- | --- |
| input | source-home route, generated payload, schema contract, mechanic part, validator behavior, or release contour |
| output | focused regression signal tied to an owner surface |
| owner | `docs/testing/TEST_TOPOLOGY.md`, `tests/AGENTS.md`, and `tests/test_inventory.json` |
| next route | failing test -> owner surface -> validator or builder -> focused test -> release lane when needed |
| tools | `unittest`, local fixtures, schema validators, temporary repos |
| check | focused test module first, then `python -m unittest discover -s tests` for broad changes |

## Boundaries

- Keep test meaning tied to the owner surface named in `test_inventory.json`.
- Keep home scopes aligned with `docs/testing/TEST_TOPOLOGY.md`.
- Keep eval verdicts, scoring doctrine, and proof authority with `aoa-evals`.
- Keep generated drift checks routed through builders and validators before
  broad test runs.
- Keep mechanics-local schema tests in mechanics-local validation lanes rather
  than routing them through canon by convenience.
- Treat migration-era names as inventory facts to improve later, not as future
  topology.
- Keep release command order in `docs/validation/validation_lanes.json`, not in
  the test inventory.

## Validation

Run the focused module for the touched surface first. For release-facing
changes run:

```bash
python -m unittest discover -s tests
```
