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
| check | `tos-ops-mechanics-plan --repo-root ABS --local-contracts mechanics/agon/parts/threshold-registry` |

## Payload

- `config/tos_agon_threshold_intakes.config.json`
- `scripts/build_tos_agon_threshold_intake_registry.py`
- `scripts/validate_tos_agon_threshold_intake_registry.py`
- Native retained assertions: `rust/crates/tos-ops-mechanics-plan/tests/mechanics_contracts.rs`
- `schemas/tos-agon-threshold-intake-registry.schema.json`
- `examples/tos_agon_threshold_intake_registry.example.json`
- `generated/tos_agon_threshold_intake_registry.min.json`

The installed native builder is `tos-ops-mechanics-plan --repo-root . --threshold-registry-build` (`--check` checks current output); validation uses `--threshold-registry-validate`. Python modules retain explicit comparison APIs only; executable compatibility selects the installed native command without fallback.
