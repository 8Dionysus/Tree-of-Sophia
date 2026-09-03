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
| `../../VALIDATION.md` | on-demand human selector for named internal lanes and district validation routes |
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

Root and district `VALIDATION.md` files are human maps, not inherited prompt
cards or second machine manifests. They may select a lane or preserve an
external-owner procedure; exact internal sequence membership and order remain
here in `validation_lanes.json`.

## Boundary Routes

- Source-home checks route to `ToS/source_home.manifest.json`, the nearest
  `ToS/**/AGENTS.md`, and `ToS/VALIDATION.md` after the source branch is known.
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
  `tests/VALIDATION.md`, and `tests/test_inventory.json`.
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
real loader or CI/manifest consumer for it, while README, AGENTS, VALIDATION,
`.agents`, public-entry, and KAG manifests already own their active entry seams.
The minimal added route is the internal on-demand currentness carrier. Select
and run its exact current sequence through root `VALIDATION.md` and the
`cross_corpus_documentation` lane.

The coordinator reuses the named AGENTS-route, mechanics topology, decision,
agent-surface, KAG, and public-entry validators and adds only cross-family
coverage, authority, public-safety, and context probes.

## Validation

Use root `VALIDATION.md` to inspect or run a named lane. The
`validation_authority` sequence owns manifest self-check; the `release_check`
sequence owns broad command composition and `scripts/release_check.py` remains
its entrypoint.
