# Questbook Mechanic

Questbook is the ToS-local route for public obligations and dispatch
compatibility. It keeps tasks and obligations from replacing philosophy.

## Mechanic Card

| Field | Route |
| --- | --- |
| status | `active` |
| class | `head-fed/local` |
| trigger | ToS obligation posture, quest schemas, catalog examples, or dispatch examples change |
| input | root `QUESTBOOK.md`, root `quests/`, obligation boundary docs, schemas, examples |
| output | public obligation boundary and dispatch contracts |
| owner | `mechanics/questbook/AGENTS.md`, `PARTS.md`, and active part routes |
| stronger route | `ToS/` for authored meaning; root `quests/` for public quest records |
| next route | obligation boundary or dispatch contracts, then root `QUESTBOOK.md` when public index changes |
| validation | `python scripts/validate_questbook_surface.py`; `python scripts/validate_mechanics_topology.py`; questbook-focused tests |

## Active Route

- [PARTS](PARTS.md)
- [PROVENANCE](PROVENANCE.md)
- [ROADMAP](ROADMAP.md)
- [Obligation Boundary](parts/obligation-boundary/README.md)
- [Dispatch Contracts](parts/dispatch-contracts/README.md)

## Historical Provenance

Use [PROVENANCE](PROVENANCE.md) when auditing former quest compatibility
placement. Active work starts from [PARTS](PARTS.md).
