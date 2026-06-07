# Threshold Registry

## Operating Card

| Field | Route |
| --- | --- |
| role | keep threshold registry entries candidate-only and checkable |
| input | registry seed, registry schema, public-safe example |
| output | generated candidate-only registry companion |
| owner | `mechanics/agon/parts/threshold-registry/` |
| next route | threshold review, not canon write |
| tools | config, schemas, example, generated companion, registry builder |
| check | `python scripts/validate_tos_agon_threshold_intake_registry.py` |

## Payload

- `config/tos_agon_threshold_intakes.seed.json`
- `schemas/tos-agon-threshold-intake-registry.schema.json`
- `examples/tos_agon_threshold_intake_registry.example.json`
- `generated/tos_agon_threshold_intake_registry.min.json`
