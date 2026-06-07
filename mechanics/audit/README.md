# Audit Mechanic

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
| validation | `python scripts/validate_mechanics_topology.py` |

## Active Route

- [PARTS](PARTS.md)
- [PROVENANCE](PROVENANCE.md)
- [ROADMAP](ROADMAP.md)
