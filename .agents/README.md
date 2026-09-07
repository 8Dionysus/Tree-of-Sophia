# Tree-of-Sophia model-facing route

This is the human entrypoint for the repository's model-facing companion
surface. It maps selection and handoff; it does not replace an owner skill,
an owner port, an `AGENTS.md` contract, or an AoA source.

## Profile-owned discovery

Tree-of-Sophia has no repository-local projection of shared or sibling-owned
skills. The selected Codex user profile `os-user-default` is the one discovery
carrier for the current bundles. Its authored binding is recorded in
`agent-surface.manifest.json`; the profile source and installer remain owned by
`aoa-skills`.

The binding selects seven shared `aoa-skills` bundles, nine admitted owner-port
bundles from `aoa-evals`, `aoa-memo`, `aoa-stats`, `aoa-kag`, `aoa-agents`,
`abyss-machine`, and `abyss-stack`, plus two `.aoa` session-memory owner links.
The profile uses Codex user scope and managed copies. The binding records
selection and ownership only; it does not prove installation, current live
visibility, invocation, runtime health, behavior, or acceptance.

The old 25 Tree-of-Sophia names remain in the authored migration crosswalk.
Each row is either a current owner skill, a merged capability mode, or a
workflow/guard owner object. A crosswalk row is not a package. Deferred
`engineering-shape` and `verification` modes remain deferred, and runtime
approval, preview, stack, infrastructure, and commit effects remain with their
named owners.

## Read by load moment

| Moment | Carrier | What it owns | Next route |
| --- | --- | --- | --- |
| always discovered | the selected profile package description and activation metadata | short selection signal and display policy | trigger the selected profile bundle only when its boundary fits |
| triggered | the selected profile package body | procedure, authority boundary, refusal/return semantics, and negative controls | nearest repository owner or stronger AoA owner named by the bundle |
| on demand | references and examples in the selected profile package | schema, rationale, or a worked shape | return to the triggered body and its owner route |
| executable or review | owner-provided helpers and checks | local helper or review aid; neither is philosophical authority | use the named owner validation lane |
| owner port | `evals/`, `stats/`, `kag/`, and `memo/` | local intake and handoff pressure | the named sibling owner |

The machine map is `agent-surface.manifest.json`. The generated structural
read model is `agent-surface.current.json`. The map is authored here; the
currentness file is rebuilt, never hand-edited. The generated file records zero
repository-local package entries, the 18 selected profile bindings, the 25-row
legacy crosswalk, task-probe depth, and owner-port inputs.
The manifest is a machine validation and migration input; discovery starts from
this entrypoint and reads the manifest on demand.

## Owner-port routes

| Local port | Consumer and load moment | Canonical owner | Currentness and next organ |
| --- | --- | --- | --- |
| `evals/PORT.yaml` | eval pressure before suite or verdict work | `aoa-evals` | local packet/route shape; hand to central selection/proof/verdict owner |
| `stats/port.manifest.json` | an owner-local measurement question or reference packet | `aoa-stats` for shared grammar; ToS for the question | source rows, route map, and reference packet; return to atlas and stats owners |
| `kag/manifest.json` | a downstream KAG/MCP consumer requests the provider family | `aoa-kag` for registry/composition; ToS derived exports remain source | derived export, local provider family, and downstream registry/MCP consumer |
| `memo/PORT.yaml` | reviewed evidence suggests a tree-local memory candidate | `aoa-memo` | candidate-only local route; reviewed handoff goes to the stronger memory owner |

Local ports can carry reference, candidate, generated, or receipt data. They do
not become live status, runtime truth, central eval authority, KAG substrate
authority, or durable memory authority by being present in Git.

## Task probes

Use the task-probe matrix in the machine map for the smallest chain below.

| Task pressure | First route | Required chain | Negative control |
| --- | --- | --- | --- |
| source authority | `aoa-knowledge-stewardship` | canonical source -> named owner -> projection after source | generated view, receipt, or ADR is not source authority |
| bounded context | deferred `aoa-skills` engineering-shape mode | contexts -> interfaces -> nearest owner contract -> narrow check | do not make a deferred mode look installed |
| repository change | host-agent repository-change workflow | root/nearest card -> bounded diff -> focused validation -> report | route prose does not own the repository effect |
| session diagnosis/repair | `aoa-session-recovery` | reviewed evidence -> diagnosis -> repair packet -> owner health check | do not diagnose live/unreviewed evidence or repair without a packet |
| approval or dry-run | host runtime approval or target runtime preview | authority classification -> preview -> explicit confirmation -> owner carrier | dry-run does not grant authorization |
| eval intake | `evals/PORT.yaml` -> `aoa-evals` | local pressure -> local validation -> central selection/proof/verdict | local packet is not a verdict |
| owner-local stats | `stats/port.manifest.json` -> `aoa-stats` | question -> measurement -> evidence-linked packet -> shared grammar | reference snapshot is not live state |
| KAG currentness | `kag/manifest.json` -> `aoa-kag` | source export -> derived seam -> provider validation -> registry/MCP | local provider is not runtime graph authority |
| memo candidate | `memo/PORT.yaml` -> `aoa-memo` | candidate-only route -> local validation -> reviewed owner intake | candidate, index, or receipt is not durable memory |

## Context and safety

The map records a soft context budget: profile descriptions stay under 100
words, triggered bodies stay under 2,000 words, and mandatory task-probe
reading reaches at most five local route hops. These are drift signals, not a
claim that a task is semantically complete.

Public authored model-facing docs do not carry host-local paths, secrets,
session snapshots, provider internals, transient runtime claims, or operator
evidence. Put those in task-local files, tool resources, owner status surfaces,
or typed receipts owned by the relevant organ.

## Validation

The focused command is the `agent_surface` lane in
`docs/validation/validation_lanes.json`:

```bash
python scripts/build_agent_surface_currentness.py
python scripts/validate_agent_surface.py --check --fetch-budget-bases
```

The builder proves parity for the authored map and generated read model. The
validator proves profile-binding shape, zero local projections, crosswalk
coverage, owner-port inputs, task-probe negative controls, and public safety.
Neither command proves profile installation, fresh-session discovery, model
behavior, owner acceptance, CI, merge, deployment, runtime health, or release.
