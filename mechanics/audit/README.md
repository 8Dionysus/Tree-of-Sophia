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
| next route | [Review Ledger Route](parts/review-ledger-route/README.md) |
| validation | `tos-ops-mechanics-plan --repo-root ROOT --mechanics-topology-validate` |

## Operations
| Part | Function |
| --- | --- |
| [Review Ledger Route](parts/review-ledger-route/README.md) | route review-ledger evidence back to source-home decisions and validators |

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
| check | `tos-route-cards --repo-root ABS validate` |

## Review return

A partial review keeps its finding, exact source and next action with the owning
review ledger, branch or validator. Refusal and pruning follow the
[review checklist](parts/review-ledger-route/docs/REVIEW_CHECKLIST.md) and the existing
[canon-restraint route](../agon/parts/canon-restraint/README.md).
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
Former `PARTS.md`, `PROVENANCE.md` and `ROADMAP.md` remain narrow compatibility
entries for known local and unenumerated external readers. Their contents are
owned here; they are not required components of a new mechanic.

## Growth conditions
Audit is planted as ToS review visibility. It keeps inspection evidence
findable and routed without turning review notes into proof, source truth, or a
parallel canon.

### Current Contour

- Keep review-ledger route checks and source-home inspection posture in the
  `review-ledger-route` part.
- Keep audit output as evidence for a next route, not as final judgment.
- Keep dated review notes in `ToS/review-ledger/`; mechanics only owns the
  repeatable route around them.
- Keep graph-review packets and post-planting audits tied to the source branch
  or graph-workbench surface they inspect.

### Next Work

- Clarify audit handoff language when review evidence repeatedly routes work
  to the same owner surface.
- Keep audit findings close to the owner route they affect.

### When Time Comes

Time has come when: ToS review evidence repeatedly needs the same inspection
packet, return route, or repair handoff.

Move: add a narrower audit part or a route note that keeps the evidence close
to its owner surface.

Guardrail: audit makes the next route visible; truth remains with source,
canon, proof, or downstream owner surfaces.

### Out Of Scope

- Proof verdicts.
- Source truth.
- Runtime evidence authority.
- Hidden cleanup.
