# ToS Script Topology

Scripts in Tree of Sophia are command-plane organs for source-first
philosophical growth. They build generated companions, validate route
boundaries, run release lanes, and provide small deterministic helper contracts.
They do not create philosophical authority, runtime policy, proof verdicts, or
graph service truth.

Machine-readable script coverage lives in
[`script_inventory.json`](script_inventory.json). It includes every tracked
non-pyc file under `*/scripts/*`, including local script route cards and
skill-local helper scripts.

## Command Authority

Blocking command sequences live in
[`validation_lanes.json`](validation_lanes.json). The script inventory is
descriptive and testable: it proves each active script surface has an owner
route, source truth, read/write boundary, lane posture, CI inclusion, and test
target.

Inventories describe script surfaces. They do not store release command order
and do not promote advisory helpers into hard gates.

## Inventory Fields

Each entry records:

- `path`
- `family`
- `organ_lane`
- `owner_surface`
- `source_truth`
- `reads`
- `writes`
- `side_effects`
- `validation_lane`
- `ci_inclusion`
- `test_target`
- `disposition`

## Script Families

| Family | Owns | Boundary |
| --- | --- | --- |
| `script_route_card` | Local route guidance for `scripts/`. | Covered by route-card validation and script topology; not a command sequence. |
| `projection_builder` | Generated/read-model writes from source surfaces. | May write tracked generated companions; must not define source meaning. |
| `projection_helper` | Shared builder library code. | Library only; command posture comes from callers. |
| `projection_validator` | Generated/read-model parity checks. | Compares projections against source and builder expectations; does not own source meaning. |
| `source_validator` | Source-home, domain, route-card, intake, canon, or mechanics checks. | Validates source-owned boundaries without becoming doctrine. |
| `compatibility_builder` | Public compatibility mirrors and public-safe examples. | Writes mirrors only from canonical source routes. |
| `compatibility_helper` | Shared compatibility mirror code. | Library only. |
| `lane_loader` | Validation lane manifest loading and checking. | Loads command authority; does not own lane meaning by itself. |
| `release_entrypoint` | Release lane execution. | Runs command sequences from the lane manifest. |
| `mechanics_local_runner` | Discovery of mechanic package-local and part-local tests, builders, and validators. | Runs only source-discovered mechanics homes; does not own mechanic meaning. |
| `skill_local_contract_tool` | Deterministic helper contracts shipped with local agent skills. | Advisory/local-only; not ToS release authority, runtime policy, or hidden hard gates. |

## Root Scripts

Root `scripts/*.py` currently own repo-wide builders, validators, release
execution, lane loading, mechanics-local discovery, and shared helpers. Root scripts
may be mechanics-owned by `owner_surface`, but a root location does not make the
script repository-wide truth.

The current mechanics-local homes are:

- `mechanics/agon/parts/threshold-registry/`, where the part owns its builder,
  validator, generated registry, and test;
- `mechanics/boundary-bridge/parts/derived-kag-seam/`, where the part owns the
  bounded KAG export generator and validator while generated payloads stay in
  `ToS/derived-exports/`;
- `mechanics/boundary-bridge/parts/public-mirror-sync/`, where the part owns
  public mirror sync scripts while mirror payloads stay in
  `ToS/public-compatibility/`;
- `mechanics/relation-weaving/parts/graph-promotion/`, where the part owns the
  relation-pack promotion validator while canonical relation payloads stay in
  `ToS/canon/relations/`;
- `mechanics/experience/tests/`, where the Experience package owns boundary
  contract tests that span several Experience parts.
- `mechanics/questbook/scripts/` and `mechanics/questbook/tests/`, where the
  Questbook package owns obligation and dispatch compatibility validation while
  root `QUESTBOOK.md` and `quests/` remain public source records.

`scripts/run_mechanics_local_tests.py` discovers these local homes and runs the
related checks.

## Skill Helper Scripts

`.agents/skills/*/scripts/*.py` helpers are deterministic contract tools for
local skill material. They can model dry-run, readiness, and risk contracts, but
they do not become ToS runtime policy enforcement or release blockers unless a
future owner decision explicitly promotes one concrete check.

## Promotion Rule

A script may move from advisory/local-only into a blocking lane only when a
current owner surface and decision record prove that Tree of Sophia owns the
checked behavior. Until then, runtime policy, MCP service authority, graph UI
caches, eval verdicts, federation, and skill execution remain route-only or
sibling-owned.
