# Dispatch Contracts

## Operating Card

| Field | Route |
| --- | --- |
| role | keep quest catalog and dispatch contracts public and checkable |
| input | quest schema, dispatch schema, catalog example, dispatch example |
| output | public-safe obligation contract surfaces |
| owner | `mechanics/questbook/parts/dispatch-contracts/` |
| next route | root `quests/` and `QUESTBOOK.md` |
| tools | schemas and examples |
| check | `tos-ops-mechanics-plan --repo-root . --questbook-validate`; `tos-ops-mechanics-plan --repo-root . --mechanics-topology-validate` |

## Payload

- `schemas/`
- `examples/`

The native Questbook compatibility assertions are the named consumer of these
retained schemas/examples. Membership and return conditions belong to the
existing [register](../../../../QUESTBOOK.md). Core validation accepts a register
without this demonstration set; if retained projections exist they must match
the registered legacy records exactly. These examples do not activate dispatch.
