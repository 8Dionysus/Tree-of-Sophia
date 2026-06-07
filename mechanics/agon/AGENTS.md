# AGENTS.md

This card applies to `mechanics/agon/` and every nested Agon path.

## Role

Agon receives head-fed threshold pressure and keeps it candidate-only,
reviewable, and unable to mutate ToS canon directly.

## Read Before Editing

1. root `AGENTS.md`
2. `mechanics/AGENTS.md`
3. `mechanics/agon/README.md`
4. `mechanics/agon/PARTS.md`
5. `mechanics/agon/PROVENANCE.md`
6. the owning active part README

## Boundaries

- ToS owns authored philosophical meaning and canon decisions.
- Agon here owns threshold intake, canon restraint, registry, and handoff
  operation only.
- Former `ToS/` paths are provenance, not active routes.
- Do not claim live protocol, proof verdict, scar, retention, rank, memory,
  runtime, SDK, stats, KAG-substrate authority, or automatic canon writes.

## Validation

```bash
python scripts/validate_mechanics_topology.py
python scripts/build_tos_agon_threshold_intake_registry.py --check
python scripts/validate_tos_agon_threshold_intake_registry.py
python scripts/validate_nested_agents.py
```
