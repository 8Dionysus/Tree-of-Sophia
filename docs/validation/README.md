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
| `agents_route_inventory.json` | source map for tracked route cards, inheritance, task routes, context budgets, and influencing surfaces |
| `../../.agents/agents-route.current.json` | generated route-card currentness read model; never stronger than the cards or owner docs |
| `documentation_family_map.json` | authored cross-corpus family law, consumer/load posture, owner roles, guard families, and projection decision |
| `documentation-family-map.schema.json` / `documentation-family-currentness.schema.json` | structural contracts for the authored map and its generated carrier |
| `documentation-family.current.json` | generated exact tracked-surface/hash/family currentness carrier; records are on demand and never source authority |
| `../testing/TEST_TOPOLOGY.md` | descriptive map of test homes, families, and failure routes |

Inventories describe coverage. They are not command authority.

## Boundary Routes

- Source-home checks route to `ToS/source_home.manifest.json` and the nearest
  `ToS/**/AGENTS.md`.
- Mechanics checks route to `mechanics/topology.json`, the package-local
  `PARTS.md` maps, and the active part route. The mechanics lane also checks
  current route links, executable script references, and the bounded
  always-on mechanics entrypoint; `PROVENANCE.md` and `ROADMAP.md` remain
  on-demand historical and growth surfaces.
- Mechanics-local contract tests route to the owning mechanic lane before
  broader repo tests.
- Generated parity checks route from source surface to builder to generated
  artifact to validator.
- Script topology routes to `SCRIPT_TOPOLOGY.md` and `script_inventory.json`.
  Validator scripts are covered there as script organs; a separate validator
  registry only becomes useful after ToS grows a distinct validator-module
  surface.
- AGENTS route topology routes to `agents_route_inventory.json`, its generated
  currentness companion, and `scripts/agents_route_harness.py`; the harness is
  a deterministic route-shape check, not a behavioral or semantic eval.
- Test topology routes to `docs/testing/TEST_TOPOLOGY.md`, `tests/AGENTS.md`,
  and `tests/test_inventory.json`.
- Local eval pressure routes to `evals/`, while proof authority stays with
  `aoa-evals`.

## Cross-corpus documentation

`documentation_family_map.json` is the compact source-owned map for the atlas
families. It preserves the initial tracked atlas method and declares, for each
family, the real consumer, load moment, owner, carrier role, currentness inputs,
next organ, context posture, public-safety boundary, and validation lane.

The generated `documentation-family.current.json` is a carrier for the
cross-corpus validator and on-demand machine readers. It enumerates tracked
documentation, structured, and executable surfaces with family membership and
content hashes. It is rebuilt from `git ls-files` and the authored map; it does
not author ToS meaning, runtime status, KAG authority, or acceptance receipts.

The map records why no `llms.txt` projection was added: the repository has no
real loader or CI/manifest consumer for it, while README, AGENTS, `.agents`,
public-entry, and KAG manifests already own their active entry seams. The
minimal added route is the internal on-demand currentness carrier, checked by:
`python scripts/build_documentation_family_currentness.py --check` followed by
`python scripts/validate_documentation_cross_corpus.py`.

The coordinator reuses the named AGENTS-route, mechanics topology, decision,
agent-surface, KAG, and public-entry validators and adds only cross-family
coverage, authority, public-safety, and context probes.

## Validation

Run the lane manifest self-check before changing release command composition:

The `validation_authority` sequence in `validation_lanes.json` owns the
manifest self-check.

For release-facing changes use:

The `release_check` sequence in `validation_lanes.json` owns broad command
composition; `scripts/release_check.py` is its entrypoint.
