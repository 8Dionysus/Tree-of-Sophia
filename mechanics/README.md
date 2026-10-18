# Tree of Sophia Mechanics

`mechanics/` is the operation atlas for repeatable Tree of Sophia work.

Use it when the change is about a process around the philosophy home: intake,
review, distillation, boundary handoff, obligation routing, release support,
or generated companion checks. Use `ToS/` when authored philosophical meaning,
source witnesses, canon, Zarathustra, or the philosophy domain tree changes.

## Route

1. Choose the package from the map below.
2. Before editing, read the nearest `AGENTS.md`; use the package README operation map to select the exact contract.
3. Follow a direct contract link; use a part entry only when its operation detail is needed.
4. Read payload docs, schemas, examples, or manifests only for the operation at
   hand. Use `README.md#history` only for former placement or source lineage, and
   `README.md#growth-conditions` only for future pressure.
5. Run the named owner check, then the repository-level mechanics gate.

## Shared Package Map

These parent names match the AoA-aligned mechanics vocabulary. Their parts are
ToS-local and may differ from sibling repositories.

| Package | Class | Use for |
| --- | --- | --- |
| [`agon`](agon/README.md) | head-fed/local | threshold intake, canon restraint, candidate-only registries, and Agon-to-ToS handoff |
| [`audit`](audit/README.md) | head-fed/local | review-ledger route checks and source-home inspection evidence |
| [`boundary-bridge`](boundary-bridge/README.md) | head-fed/local | derived KAG/public seams without authority transfer |
| [`distillation`](distillation/README.md) | head-fed/local | source compost and extraction into reviewable ToS form |
| [`experience`](experience/README.md) | head-fed/local | adoption, governance, installation, service, pattern, candidate, and write-guard boundaries |
| [`growth-cycle`](growth-cycle/README.md) | head-fed/local | branch growth loops from witness pressure to reviewed structure |
| [`questbook`](questbook/README.md) | head-fed/local | public obligation posture and quest dispatch contracts |
| [`release-support`](release-support/README.md) | head-fed/local | source-home release gates and public support checks |

## ToS-Local Package Map

These packages are local to Tree of Sophia. They name repeatable operations
around the philosophy home without moving the authored home into mechanics.

| Package | Class | Use for |
| --- | --- | --- |
| [`source-witnessing`](source-witnessing/README.md) | local | witness route discipline before branch or canon movement |
| [`canon-formation`](canon-formation/README.md) | local | reviewed promotion into canonical nodes, relations, and registries |
| [`relation-weaving`](relation-weaving/README.md) | local | graph-workbench fragments and relation-pack promotion |

## Task-to-owner map

This entrypoint routes; the strongest carrier below owns the current detail.

| Task | Strongest carrier | Exact next route |
| --- | --- | --- |
| Understand a repeated operation | Authored part contract and its payload docs | Package README operation map -> exact docs/schema/config |
| Choose a package or part | Human map here plus machine map in [`topology.json`](topology.json) | Package `README.md#operations`; do not infer ownership from a directory tour |
| Run a builder or validator | Executable script/CLI; blocking order in [`validation_lanes.json`](../docs/validation/validation_lanes.json) | Part `README.md` `tools`/`check` -> owning lane; mechanics-local discovery remains [`tos-ops-mechanics-plan`](../rust/crates/tos-ops-mechanics-plan/README.md) |
| Change a schema, config, or manifest | Part-local schema/config/manifest | Source contract -> builder -> generated companion -> validator; generated output is never authoring truth |
| Check release or validation state | Validation lane manifest and release entrypoint | [`mechanics/release-support`](release-support/README.md) -> [`docs/RELEASING.md`](../docs/RELEASING.md) -> named lane |
| Find durable rationale | Authored decision record; generated indexes are lookup only | [`docs/decisions/README.md`](../docs/decisions/README.md) -> `TOS-D-*`; current source or mechanic contract remains stronger |
| Inspect live state or execution evidence | Owner telemetry, status surface, receipt, or external AoA owner | Follow the receipt/status handle named by the active part; mechanics docs do not snapshot runtime truth |

## Progressive disclosure

Always-on context stays here: what mechanics own, where package selection starts,
and which owner carries each task. Everything else is on demand: the nearest
`AGENTS.md` for edit law, `README.md#operations` for selection, the active part `README.md`
for operation, and its payload or executable for detail. Historical placement
and future growth remain behind `README.md#history` and `README.md#growth-conditions` respectively.

## Status Semantics

`active` marks a mechanic that already protects a repeated current route:
payload, owner split, part-local check, release-visible seam, or live source
home contract.

`planted` marks a named mechanic contour whose first part gives future pressure
a route before it has enough repeated payload to become an active operation.

Every package still names `active_parts` in `topology.json` because `parts/`
is the operation vocabulary. The package status says how mature that operation
is today.

## Root Contract

Root `mechanics/` owns only:

- `README.md` for human route selection.
- `AGENTS.md` for mechanics-tree edit law.
- `topology.json` for the active package, part, and historical-recovery policy.

Do not add root rosters, migration ledgers, backlogs, notes, `_meta/`,
scratch, or root `legacy/` holding areas. Active operation detail belongs in
the owning package or part. Durable rationale belongs in `docs/decisions/`.
Former-path accounting belongs in package `README.md#history` and its exact Git
history route, consulted only when current work needs historical context.

The package README growth section owns future pressure contours: what the mechanic is
currently holding, what condition would make it grow, and what boundary keeps
that growth honest. They are not backlogs, validation inventories, script
placement plans, or migration ledgers.

## Placement

- Source-authored philosophical material stays under `ToS/`.
- Zarathustra stays a golden source/canon axis inside `ToS/`, not a mechanics
  package.
- Mechanic-owned docs, schemas, examples, config, generated companions,
  manifests, builders, and focused tests live under the nearest active part.
- Root `QUESTBOOK.md` owns obligation membership, owners and return conditions. Root `quests/` retains exact record evidence for the named native compatibility consumer. `mechanics/questbook/`
  owns obligation compatibility and dispatch posture.
- Root scripts stay in `scripts/` when they are repo-wide validators,
  builders, or public compatibility entrypoints.
- Generated read models stay root-published or `ToS/derived-exports/` only
  when consumed outside one mechanic.

## Legacy Rule

Historical archives remain recoverable in Git, outside the current checkout.
The owning package links an immutable commit and original path through
`README.md#history` when old paths, receipts, or names need historical accounting.

The route is:

`active package -> README.md#history -> immutable Git source`

Do not recreate `legacy/` directories or empty accounting scaffolds. Current
operational receipts and candidate evidence stay with the active owning part.
The topology retains `legacy_required: false` as an explicit absence guard;
setting it to true does not authorize restoring a retired archive.

## Validation

The `mechanics_topology` and `route_docs` sequences in
`docs/validation/validation_lanes.json` own executable verification. Use
`mechanics/AGENTS.md` for the focused operator route.

For release-facing changes, repository-wide verification routes through the
`release_check` sequence in `docs/validation/validation_lanes.json` and its
script entrypoint.

Compact packages declare `compact_routes` in the existing topology. Owner,
selection, source return, exact history and validation may share the package
README; no five-document family is required. Old navigation files are narrow
compatibility entries for existing and unknown external readers.
