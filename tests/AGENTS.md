# AGENTS.md

## Applies To

This card applies to `tests/`.

## Role

`tests/` owns repo-local regression checks for ToS route surfaces, generated
parity, canon contracts, mechanics payload contracts, validator behavior, and
release contour. Tests prove bounded behavior; they do not promote candidate
material, create doctrine, or replace eval/proof authority.

## Operating Card

| Field | Route |
| --- | --- |
| input | source-home route, generated payload, schema contract, mechanic part, validator behavior, or release contour |
| output | focused regression signal tied to an owner surface |
| owner | `tests/AGENTS.md` and `tests/test_inventory.json` |
| next route | failing test -> owner surface -> validator or builder -> focused test -> release lane when needed |
| tools | `unittest`, local fixtures, schema validators, temporary repos |
| check | focused test module first, then `python -m unittest discover -s tests` for broad changes |

## Boundaries

- Keep test meaning tied to the owner surface named in `test_inventory.json`.
- Keep eval verdicts, scoring doctrine, and proof authority with `aoa-evals`.
- Keep generated drift checks routed through builders and validators before
  broad test runs.
- Treat migration-era names such as batch labels as inventory facts to improve
  later, not as future topology.

## Validation

Run the focused module for the touched surface first. For release-facing
changes run:

```bash
python -m unittest discover -s tests
```
