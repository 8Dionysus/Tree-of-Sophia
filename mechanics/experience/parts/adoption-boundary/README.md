# Adoption Boundary

## Operating Card

| Field | Route |
| --- | --- |
| role | keep adoption packets reviewable without runtime adoption |
| input | adoption dossier and no-runtime adoption guard |
| output | adoption boundary packet |
| owner | `mechanics/experience/parts/adoption-boundary/` |
| next route | runtime or owner repo only after explicit owner acceptance |
| tools | docs, schemas, examples |
| check | `python scripts/validate_mechanics_topology.py` |

## Payload

- `docs/`
- `schemas/`
- `examples/`
