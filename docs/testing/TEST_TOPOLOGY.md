# ToS Test Topology

This map keeps Tree of Sophia tests readable as route-bound regression organs.
Tests prove bounded behavior for source-home routes, generated companions,
mechanics contracts, validator behavior, and release contour. They do not create
philosophical authority, command authority, eval verdicts, or runtime policy.

Machine-readable coverage lives in
[`tests/test_inventory.json`](../../tests/test_inventory.json). Update it when a
test file is added, moved, renamed, split, folded, or changes owner surface.

## Route Shape

Use this compact route shape:

```text
family -> protects -> owner surface -> home scope -> coverage authority -> focused target -> failure route
```

Test files are not command authority. Blocking command sequences live in
[`docs/validation/validation_lanes.json`](../validation/validation_lanes.json).
Current active test homes are root `tests/` and mechanics-local test homes.
Root tests are covered by `tests/AGENTS.md`; mechanics-local tests are covered
by their nearest mechanics route cards and `mechanics_local` lane discovery.

## Home Scopes

| Home Scope | Current Homes | Protects | Coverage Authority | Failure Route |
| --- | --- | --- | --- | --- |
| `root` | `tests/` | Repo-wide route docs, source-home schema, generated parity, validator behavior, validation authority, and release contour. | `tests/AGENTS.md` root unittest discovery | Fix the named owner surface before editing test expectations. |
| `mechanic-level` | `mechanics/experience/tests/`; `mechanics/questbook/tests/`; future `mechanics/<slug>/tests/` | One mechanic package's active topology or package-wide contracts. | package lane plus `mechanics_local` discovery | Fix the owning mechanic package, `PARTS.md`, schemas, examples, or local validator first. |
| `part-local` | `mechanics/agon/parts/threshold-registry/tests/`; future `mechanics/<slug>/parts/<part>/tests/` | One mechanic part, its generated companion, registry, or handoff packet. | `mechanics_local` lane discovery | Fix the part-local source, builder, validator, and schema before widening to release. |
| `agent-lane` | future `.agents/*/tests/` | Agent-surface operating guidance and local scenario contracts. | release or advisory lane, depending on owner decision | Fix the owning agent surface before treating the repo gate as clean. |

Do not create mechanic-level or part-local homes only for symmetry. Promote a
test out of `tests/` when the owner surface has become local enough that a
mechanic package or part should carry the regression with its own source.

## Families

| Family | Protects | Owner Surface |
| --- | --- | --- |
| `root_front_door` | Root entrypoint routing and command-light public docs. | `README.md`, `ROADMAP.md`. |
| `release_contour` | Current release-facing route and root-entry expectations. | `ROADMAP.md` and release lane manifest. |
| `generated_parity` | Derived read models and generated indexes remain rebuildable from source. | Builder scripts, schemas, generated companions. |
| `mechanics_generated_parity` | Mechanics-owned generated companions remain tied to their part source. | Owning mechanic part. |
| `schema_canon_contract` | Canon-facing schema contracts and public examples. | `ToS/contracts/` and related ToS surfaces. |
| `validator_behavior` | Focused validator behavior without making validators doctrine. | The exact validator script and its source owner. |
| `mechanics_validator_behavior` | Mechanics-local validator behavior. | Owning mechanic package or part. |
| `mechanics_schema_contract` | Mechanics-local schema and example contracts. | Owning mechanic package or part. |
| `source_home_contract` | ToS source-home manifest and branch schema. | `ToS/source_home.manifest.json` and source-home schema. |
| `command_authority` | Lane manifest shape, release coverage, and command delegation. | `docs/validation/validation_lanes.json`. |
| `test_topology` | Test inventory completeness and boundary posture. | This document and `tests/test_inventory.json`. |

## Inventory Rules

- Each inventory entry names a file path, family, validation lane, owner surface,
  protected boundary, home scope, coverage authority, focused target, failure
  route, runtime cost, and disposition.
- `focused_target` and `coverage_authority` name surfaces, not shell commands.
- All active `test*.py` files under root `tests/`, mechanics test homes, and
  future agent test homes must have exactly one inventory entry.
- Root tests may protect mechanics-owned contracts only while the local mechanic
  home is not ready; the owner surface and validation lane must make that route
  explicit.
- Release command order stays in `docs/validation/validation_lanes.json`; tests
  may assert lane coverage but must not store the release command sequence.
