# Tree of Sophia Mechanics

`mechanics/` is the operation atlas for repeatable Tree of Sophia work.

Use it when the change is about a process around the philosophy home: intake,
review, distillation, boundary handoff, obligation routing, release support,
or generated companion checks. Use `ToS/` when authored philosophical meaning,
source witnesses, canon, Zarathustra, or the philosophy domain tree changes.

## Route

1. Choose the package from the map below.
2. Read package `AGENTS.md`, `README.md`, `PARTS.md`, `PROVENANCE.md`, and
   `ROADMAP.md`.
3. Follow the active part route named by `PARTS.md`.
4. Use `PROVENANCE.md` only when former placement or source lineage matters.
5. Enter package-local `legacy/` only through `PROVENANCE.md`, and only in
   packages that have real moved-path or raw-receipt accounting.
6. Run the package or part validator, then the repo-level mechanics gate.

## Shared Package Map

These parent names match the AoA-aligned mechanics vocabulary. Their parts are
ToS-local and may differ from sibling repositories.

| Package | Class | Use for |
| --- | --- | --- |
| [`agon`](agon/README.md) | head-fed/local | threshold intake, canon restraint, candidate-only registries, and Agon-to-ToS handoff |
| [`antifragility`](antifragility/README.md) | head-fed/local | source-first refusal, bad-growth pruning, and via negativa review posture |
| [`audit`](audit/README.md) | head-fed/local | review-ledger route checks and source-home inspection evidence |
| [`boundary-bridge`](boundary-bridge/README.md) | head-fed/local | derived KAG/public seams without authority transfer |
| [`checkpoint`](checkpoint/README.md) | head-fed/local | review return points that preserve state without becoming memory truth |
| [`distillation`](distillation/README.md) | head-fed/local | source compost and extraction into reviewable ToS form |
| [`experience`](experience/README.md) | head-fed/local | adoption, governance, installation, service, pattern, candidate, and write-guard boundaries |
| [`growth-cycle`](growth-cycle/README.md) | head-fed/local | branch growth loops from witness pressure to reviewed structure |
| [`method-growth`](method-growth/README.md) | head-fed/local | node, relation, template, and validator method maturation |
| [`questbook`](questbook/README.md) | head-fed/local | public obligation posture and quest dispatch contracts |
| [`recurrence`](recurrence/README.md) | head-fed/local | calibration return and re-entry after route drift |
| [`release-support`](release-support/README.md) | head-fed/local | source-home release gates and public support checks |
| [`rpg`](rpg/README.md) | head-fed/local | bounded reading-progression reflection without gameplay or canon authority |

## ToS-Local Package Map

These packages are local to Tree of Sophia. They name repeatable operations
around the philosophy home without moving the authored home into mechanics.

| Package | Class | Use for |
| --- | --- | --- |
| [`source-witnessing`](source-witnessing/README.md) | local | witness route discipline before branch or canon movement |
| [`canon-formation`](canon-formation/README.md) | local | reviewed promotion into canonical nodes, relations, and registries |
| [`relation-weaving`](relation-weaving/README.md) | local | graph-workbench fragments and relation-pack promotion |

## Root Contract

Root `mechanics/` owns only:

- `README.md` for human route selection.
- `AGENTS.md` for mechanics-tree edit law.
- `topology.json` for the active package, part, and legacy-eligibility map.

Do not add root rosters, migration ledgers, backlogs, notes, `_meta/`,
scratch, or root `legacy/` holding areas. Active operation detail belongs in
the owning package or part. Durable rationale belongs in `docs/decisions/`.
Former-path accounting belongs in package `PROVENANCE.md` and package-local
`legacy/` only when an active route needs it.

Package `ROADMAP.md` files own future pressure contours: what the mechanic is
currently holding, what condition would make it grow, and what boundary keeps
that growth honest. They are not backlogs, validation inventories, script
placement plans, or migration ledgers.

## Placement

- Source-authored philosophical material stays under `ToS/`.
- Zarathustra stays a golden source/canon axis inside `ToS/`, not a mechanics
  package.
- Mechanic-owned docs, schemas, examples, config, generated companions,
  manifests, builders, and focused tests live under the nearest active part.
- Root `quests/` stays the public quest item store. `mechanics/questbook/`
  owns obligation compatibility and dispatch posture.
- Root scripts stay in `scripts/` when they are repo-wide validators,
  builders, or public compatibility entrypoints.
- Generated read models stay root-published or `ToS/derived-exports/` only
  when consumed outside one mechanic.

## Legacy Rule

`legacy/` is not a warehouse and not an active route. It forms after an active
package and part map exist, when old paths, raw receipts, or previous names
must remain accountable to the current route.

The route is:

`active package -> PARTS.md -> PROVENANCE.md -> package-local legacy/`

Packages without moved-path or raw-receipt accounting do not get empty
`legacy/` directories for symmetry.

## Validation

```bash
python scripts/validate_mechanics_topology.py
python scripts/validate_nested_agents.py
```

For release-facing changes, run:

```bash
python scripts/release_check.py
```
