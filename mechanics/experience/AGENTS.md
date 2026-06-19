# AGENTS.md

This card applies to `mechanics/experience/` and every nested Experience path.

## Role

Experience keeps adoption, governance, installation, service, pattern,
candidate, and write-guard boundary packets reviewable and owner-routed.

## Operating Card

| Field | Route |
| --- | --- |
| input | boundary dossier, adoption pressure, governance precedent, installation note, service posture, pattern review, or write-guard packet |
| output | reviewed boundary contract, public example, schema, or owner handoff |
| owner | `mechanics/experience/AGENTS.md`, `PARTS.md`, `PROVENANCE.md`, and active part routes |
| next route | active part README, package-local tests, then stronger runtime/governance/service owner when activation pressure appears |
| validation | `experience_contracts` lane, mechanics topology, nested route-card check |

## Boundary Routes

- ToS owns authored philosophical meaning and canon.
- Experience here owns boundary operation payload only.
- Former `ToS/` paths route through `PROVENANCE.md` and package-local
  `legacy/`.
- Runtime, office, service, governance, proof, memory, SDK, and canon authority
  route to stronger owners before ToS widens.

## Validation

Focused lane: `experience_contracts` in `docs/validation/validation_lanes.json`.

```bash
python scripts/validate_mechanics_topology.py
python -m unittest discover -s mechanics/experience/tests -p 'test*.py'
python scripts/validate_nested_agents.py
```
