# Audit Mechanic

<a id="active-route"></a>

## Mechanic Card

| Field | Route |
| --- | --- |
| status | `planted` |
| class | `head-fed/local` |
| trigger | source-home review, ledger evidence, or route inspection needs an operation owner |
| input | review note, checklist result, source-home drift |
| output | audit route and owner handoff |
| owner | `mechanics/audit/` |
| stronger route | `ToS/` owns source meaning; `aoa-evals` owns proof verdicts |
| next route | [Review Ledger Route](#review-ledger-route) |
| validation | manual review of the changed owner surface |

## Operations
| Part | Function |
| --- | --- |
| [Review Ledger Route](#review-ledger-route) | route review-ledger evidence back to source-home decisions and validators |

The [local card](AGENTS.md) owns editing; [validation](../VALIDATION.md) owns checks.
Source or semantic questions return to [the source owner](../../ToS/AGENTS.md).

### Direct contracts

- `review-ledger-route`: [REVIEW_CHECKLIST](parts/review-ledger-route/docs/REVIEW_CHECKLIST.md)

### review-ledger-route

## Operating Card

| Field | Route |
| --- | --- |
| role | route review evidence without making it canon |
| input | dated review note, checklist finding, route drift |
| output | owner surface, validator, or decision follow-up |
| owner | `mechanics/audit/parts/review-ledger-route/` |
| next route | `ToS/review-ledger/`, `docs/decisions/`, or owning validator |
| tools | `mechanics/audit/parts/review-ledger-route/docs/REVIEW_CHECKLIST.md`, `ToS/review-ledger/` |

## Review return

A partial review keeps its finding, exact source and next action with the owning
review ledger, branch or validator. Refusal and pruning follow the
[review checklist](parts/review-ledger-route/docs/REVIEW_CHECKLIST.md) and the existing
[canon-restraint route](../agon/README.md#threshold-intake-and-canon-restraint).
Route drift returns to [calibration](../../ToS/doctrine/CALIBRATION_AXIS.md)
and then to the affected source owner. These return points do not establish
canon, runtime state or durable memory truth.

## History

Former `ToS/doctrine/REVIEW_CHECKLIST.md` placement remains recoverable through
immutable Git history; no package-local legacy archive is maintained.
`ToS/review-ledger/` remains the active source-home evidence branch.

The former resilience, review-return and calibration routing cards are retained
at commit `0486a9a457e0291eb7716c330b7521f96912aa89`, paths
`mechanics/antifragility/`, `mechanics/checkpoint/` and `mechanics/recurrence/`.
Their current operation returns through the Review Ledger Route.

[Exact prior provenance](https://github.com/8Dionysus/Tree-of-Sophia/blob/b54cbf8e30ff46aeddeec5c47a5e7aa1f370977d/mechanics/audit/PROVENANCE.md) preserves the pre-compaction route.

## Growth conditions

Add an audit part or route note when inspection packets, return routes, or repair handoffs recur. Keep graph-review and post-planting audits linked to the branch or graph workbench they inspect.
