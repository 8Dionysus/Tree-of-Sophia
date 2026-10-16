# Review Ledger Route

## Operating Card

| Field | Route |
| --- | --- |
| role | route review evidence without making it canon |
| input | dated review note, checklist finding, route drift |
| output | owner surface, validator, or decision follow-up |
| owner | `mechanics/audit/parts/review-ledger-route/` |
| next route | `ToS/review-ledger/`, `docs/decisions/`, or owning validator |
| tools | `mechanics/audit/parts/review-ledger-route/docs/REVIEW_CHECKLIST.md`, `ToS/review-ledger/` |
| check | `tos-route-cards --repo-root ABS validate` |

## Review return

A partial review keeps its finding, exact source and next action with the owning
review ledger, branch or validator. Refusal and pruning follow the
[review checklist](docs/REVIEW_CHECKLIST.md) and the existing
[canon-restraint route](../../../agon/parts/canon-restraint/README.md).
Route drift returns to [calibration](../../../../ToS/doctrine/CALIBRATION_AXIS.md)
and then to the affected source owner. These return points do not establish
canon, runtime state or durable memory truth.
