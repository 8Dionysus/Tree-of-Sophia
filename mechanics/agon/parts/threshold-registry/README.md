# Threshold Registry

## Operating Card

| Field | Route |
| --- | --- |
| role | keep threshold registry entries candidate-only and checkable |
| input | registry config, registry schema, public-safe example |
| output | generated candidate-only registry companion |
| owner | `mechanics/agon/parts/threshold-registry/` |
| next route | threshold review, not canon write |
| tools | config, schemas, example, generated companion, part-local registry builder, part-local validator |
| check | `python scripts/run_mechanics_local_tests.py` |

## Payload

- `config/tos_agon_threshold_intakes.config.json`
- `scripts/build_tos_agon_threshold_intake_registry.py`
- `scripts/validate_tos_agon_threshold_intake_registry.py`
- `tests/test_tos_agon_threshold_intake_registry.py`
- `schemas/tos-agon-threshold-intake-registry.schema.json`
- `examples/tos_agon_threshold_intake_registry.example.json`
- `generated/tos_agon_threshold_intake_registry.min.json`
