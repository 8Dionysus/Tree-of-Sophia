# ToS Validation Topology

This directory owns the repository-level validation topology for
`Tree-of-Sophia`.

## Role

Validation lanes name the checks that protect ToS routes. They do not create
philosophical authority. Source witnesses, doctrine, canon, mechanics packages,
decision records, generated read models, tests, and local eval ports keep their
own owner surfaces.

## Surfaces

| Surface | Role |
| --- | --- |
| `validation_lanes.json` | executable command authority for named validation lanes |
| `validator_inventory.json` | descriptive map of validators to owner surfaces and failure routes |
| `script_inventory.json` | descriptive map of root scripts to script families and side effects |

Inventories describe coverage. They are not command authority.

## Boundary Routes

- Source-home checks route to `ToS/source_home.manifest.json` and the nearest
  `ToS/**/AGENTS.md`.
- Mechanics checks route to `mechanics/topology.json` and package-local
  `PARTS.md`, `PROVENANCE.md`, and `ROADMAP.md`.
- Mechanics-local contract tests route to the owning mechanic lane before
  broader repo tests.
- Generated parity checks route from source surface to builder to generated
  artifact to validator.
- Test topology routes to `tests/AGENTS.md` and `tests/test_inventory.json`.
- Local eval pressure routes to `evals/`, while proof authority stays with
  `aoa-evals`.

## Validation

Run the lane manifest self-check before changing release command composition:

```bash
python scripts/validation_lanes.py --check
```

For release-facing changes use:

```bash
python scripts/release_check.py
```
