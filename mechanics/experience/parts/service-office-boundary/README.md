# Service Office Boundary

## Operating Card

| Field | Route |
| --- | --- |
| role | keep service and office packets out of direct ToS writes |
| input | service dossier, office runtime guard |
| output | service-office boundary packet |
| owner | `mechanics/experience/parts/service-office-boundary/` |
| next route | service, office, or runtime owner |
| tools | docs, schemas, examples |
| check | `python rust/crates/tos-ops-mechanics-plan/src/mechanics_topology.rs` |

## Payload

- `docs/`
- `schemas/`
- `examples/`
