# AGENTS.md

This card applies to `manifests/`.

## Role

`manifests/` holds small machine-readable posture manifests for named ToS
surfaces. The current lane is observe-only recurrence around Agon threshold
intake surfaces.

These manifests describe watch posture and component identity. Source meaning
stays in `ToS/`; repeatable operation law stays in `mechanics/`; runtime hook
execution stays with the owning stack or OS layer.

## Operating Card

| Field | Route |
| --- | --- |
| input | reviewed ToS surface that needs an observe-only component posture |
| output | component manifest or hook binding manifest |
| owner | `manifests/AGENTS.md` and the owning source or mechanic surface named by the manifest |
| next route | source/mechanic owner -> manifest update -> owning runtime or stack layer when execution is requested |
| validation | run the source or mechanic validator that owns the named surface |

## Boundary Routes

- Keep observe-only manifests tied to explicit source or mechanic surfaces.
- Route live hooks, scheduling, runtime mutation, memory authority, and proof
  authority to their owning AoA layer.
- Keep recurrence manifests small enough to read as posture and separate from
  generated runtime state.

## Validation

Run the validator for the named owner surface. For the current Agon threshold
intake component:

```bash
python mechanics/agon/parts/threshold-registry/scripts/validate_tos_agon_threshold_intake_registry.py
python scripts/validate_mechanics_topology.py
```
