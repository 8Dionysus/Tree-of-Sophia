# AGENTS.md

This card applies to `mechanics/source-witnessing/`.

## Role

Source Witnessing is a ToS-local operation around witness route discipline.

## Boundary

This package does not store witness material. `ToS/source-witnesses/` owns
the witness branch.

## Validation

```bash
python scripts/validate_mechanics_topology.py
python scripts/validate_tos_source_home.py
```
