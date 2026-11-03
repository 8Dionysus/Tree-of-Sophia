# Tree of Sophia Mechanics

`mechanics/` owns repeatable operations around the ToS source home: intake,
review, distillation, handoff, obligations, release support and companion checks.

## Route

1. Choose a package below or in [topology.json](topology.json).
2. Read the nearest `AGENTS.md` and the package README's operation map; select
   the relevant contract, payload, executable or manifest.
3. Run its owner check and the repository mechanics gate. Use the package's
   `#history` for former paths and `#growth-conditions` for future pressure.

## Shared Package Map

These AoA-aligned names contain ToS-local operations.

| Package | Class | Use for |
| --- | --- | --- |
| [`agon`](agon/README.md) | head-fed/local | threshold intake, canon restraint, candidate-only registries and handoff |
| [`audit`](audit/README.md) | head-fed/local | review-ledger routes and source-home inspection |
| [`boundary-bridge`](boundary-bridge/README.md) | head-fed/local | derived KAG and public seams |
| [`distillation`](distillation/README.md) | head-fed/local | source compost and reviewable extraction |
| [`experience`](experience/README.md) | head-fed/local | adoption, governance, installation, service, pattern, candidate and write-guard boundaries |
| [`growth-cycle`](growth-cycle/README.md) | head-fed/local | growth from witness pressure to reviewed structure |
| [`questbook`](questbook/README.md) | head-fed/local | public obligations and dispatch contracts |
| [`release-support`](release-support/README.md) | head-fed/local | source-home release and public support checks |

## ToS-Local Package Map

| Package | Class | Use for |
| --- | --- | --- |
| [`source-witnessing`](source-witnessing/README.md) | local | witness routes before branch or canon movement |
| [`canon-formation`](canon-formation/README.md) | local | reviewed promotion into canonical nodes, relations and registries |
| [`relation-weaving`](relation-weaving/README.md) | local | graph-workbench fragments and relation-pack promotion |

## Task-to-owner map

Package maps select operation contracts. Schemas, configs and manifests lead to
their builder, generated companion and validator. The executable and
[validation lanes](../docs/validation/validation_lanes.json) own command order;
[tos-ops-mechanics-plan](../rust/crates/tos-ops-mechanics-plan/README.md) owns
mechanics-local discovery. Release work follows [release support](release-support/README.md)
and [RELEASING](../docs/RELEASING.md). Durable rationale is in
[decisions](../docs/decisions/README.md); live state follows the named owner's
telemetry, status or receipt.

## Progressive disclosure

Use the package map for selection, the local `AGENTS.md` for editing, and the
selected operation contract for details. Consult history and growth sections
when those questions arise.

## Status Semantics

`active` marks a protected, repeated current route. `planted` names an operation
whose first part gives future pressure a route. Each package lists `active_parts`
in `topology.json`; package status describes the operation's maturity.

## Root Contract

The root owns this map, `AGENTS.md` and `topology.json`. Active operation detail
and receipts belong to their package or part; durable rationale belongs in
`docs/decisions/`. Rosters, migration ledgers, backlogs, notes, `_meta/`, scratch
and `legacy/` holding areas belong outside the mechanics root. Package growth
sections own conditions for future change; backlogs, validation inventories,
script placement plans and migration ledgers follow their owning surfaces.

## Placement

Authored philosophy, source witnesses and canon remain in `ToS/`; Zarathustra
is a source/canon axis there. Mechanic docs, schemas, examples, config,
generated companions, manifests, builders and focused tests belong to the
nearest active part. Root `QUESTBOOK.md` owns obligation membership, owners and
return conditions; `quests/` retains exact record evidence and
`mechanics/questbook/` owns compatibility and dispatch posture. Repo-wide
validators, builders and public compatibility entrypoints live in `scripts/`.
Read models used outside one mechanic may be root-published or placed in
`ToS/derived-exports/`.

## Legacy Rule

Historical archives remain in Git. The owning package links an immutable commit
and original path through `README.md#history`:

`active package -> README.md#history -> immutable Git source`

The topology keeps `legacy_required: false` as an absence guard. Archive
restoration follows its owning decision. Current operational receipts and
candidate evidence stay with the active owning part.

## Validation

The `mechanics_local` sequence in `docs/validation/validation_lanes.json` owns
local contract verification; [mechanics/AGENTS.md](AGENTS.md) gives the focused
route. Release-facing changes use the `release_check` sequence. Compact packages
record owner, selection, source return, exact history and validation under
`compact_routes` in the topology; those routes may share one package README.
