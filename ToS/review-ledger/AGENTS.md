# AGENTS.md

This file applies to dated review notes under `ToS/review-ledger/`.

## Role

`ToS/review-ledger/` preserves inspection notes, review outcomes, and migration
evidence for ToS surfaces.

## Operating Card

| Field | Route |
| --- | --- |
| role | dated review evidence surface |
| input | inspection note, review outcome, migration evidence, or branch-check record |
| output | dated note that explains what was inspected and where current authority lives |
| owner | `ToS/review-ledger/AGENTS.md` and the dated review note |
| next route | review note -> owning doctrine/source/canon/contract/decision surface when authority must move |
| tools | review checklist, source-home validator, route-card validator |
| check | source-home and route-card validators for moved review surfaces |

## Boundary Routes

- Keep entries dated and surface-specific.
- Route current doctrine to `ToS/doctrine/`.
- Route durable rationale to `docs/decisions/`.
- Route active compatibility examples to `ToS/public-compatibility/`.

## Validation

Use `scripts/validate_tos_source_home.py` and
`scripts/validate_nested_agents.py` for review-ledger moves or route changes.
