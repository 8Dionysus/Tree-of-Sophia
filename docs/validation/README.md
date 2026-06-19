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
| `script_inventory.json` | descriptive map of active `*/scripts/*` surfaces to owners, lanes, and side effects |
| `SCRIPT_TOPOLOGY.md` | descriptive map of script homes, families, side effects, and lane posture |
| `../testing/TEST_TOPOLOGY.md` | descriptive map of test homes, families, and failure routes |

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
- Script topology routes to `SCRIPT_TOPOLOGY.md` and `script_inventory.json`.
  Validator scripts are covered there as script organs; a separate validator
  registry only becomes useful after ToS grows a distinct validator-module
  surface.
- Test topology routes to `docs/testing/TEST_TOPOLOGY.md`, `tests/AGENTS.md`,
  and `tests/test_inventory.json`.
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
