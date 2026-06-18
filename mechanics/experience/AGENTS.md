# AGENTS.md

This card applies to `mechanics/experience/` and every nested Experience path.

## Role

Experience keeps boundary packets reviewable and owner-routed. It does not
activate runtime, office, service, governance, proof, memory, SDK, or ToS canon
authority.

## Read Before Editing

1. root `AGENTS.md`
2. `mechanics/AGENTS.md`
3. `mechanics/experience/README.md`
4. `mechanics/experience/PARTS.md`
5. `mechanics/experience/PROVENANCE.md`
6. the owning active part README

## Boundaries

- ToS owns authored philosophical meaning and canon.
- Experience here owns boundary operation payload only.
- Former `ToS/` paths are provenance, not active routes.
- Activation claims route to the stronger owner before ToS widens.

## Validation

Focused lane: `experience_contracts` in `docs/validation/validation_lanes.json`.

```bash
python scripts/validate_mechanics_topology.py
python -m unittest discover -s tests -p 'test_experience_*_contracts.py'
python scripts/validate_nested_agents.py
```
