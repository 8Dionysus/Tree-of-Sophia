# Tree-of-Sophia model-facing route

This is the human entrypoint for the repository's model-facing companion
surface. It maps selection and handoff; it does not replace a skill body, an
owner port, an AGENTS.md contract, or an AoA owner.

## Read by load moment

| Moment | Carrier | What it owns | Next route |
| --- | --- | --- | --- |
| always discovered | skill frontmatter description and agents/openai.yaml | short selection signal and display/activation metadata | trigger the selected package only when its boundary fits |
| triggered | skills/*/SKILL.md | procedure, authority boundary, refusal/return semantics, provenance, uncertainty, and negative controls | nearest repository or stronger AoA owner named by the skill |
| on demand | references/ and examples/ | schema, rationale, or a worked shape needed by the current task | return to the triggered skill and its owner route |
| executable or review | scripts/ and checks/ | local helper or review checklist; neither is philosophical authority | use the named owner-local or validation lane |
| portable package support | assets/ | package UI assets | keep portable copies aligned with the aoa-skills source owner |

The machine map is agent-surface.manifest.json. The generated structural read
model is agent-surface.current.json. The map is authored here; the currentness
file is rebuilt, never hand-edited.

## Skill families

The repository carries 25 portable skill entrypoints. Their source owner is
8Dionysus/aoa-skills; this directory is the local model-facing export.

| Family | Use | Next organ |
| --- | --- | --- |
| engineering core | authority, boundaries, bounded repository change, contracts, logic seams, invariants, and tests | the nearest source, mechanic, docs, or test owner and its narrow validator |
| risk controls | approval, preview, local stack, infrastructure change, and public-safe sharing | approval/dry-run carrier, runtime owner, or sanitized sharing owner; status and receipts stay outside this map |
| session growth | reviewed-session diagnosis, repair, closeout, route forks, progression, quest, automation, commit, and bounded delegation | a reviewed session packet and the named AoA skill/owner; never a live-session snapshot |

The SKILL.md body is loaded only after a package is selected. References,
examples, checks, scripts, and assets are task-local reads. A green helper or
validator proves only its declared mechanics; it does not admit a source,
claim, canon object, runtime, graph, eval verdict, memory, or release.

## Owner-port routes

These ports expose local pressure while preserving stronger owners.

| Local port | Consumer and load moment | Canonical owner | Currentness and next organ |
| --- | --- | --- | --- |
| evals/PORT.yaml | eval pressure intake before suite or verdict work | aoa-evals | local packet/route shape; hand to aoa-evals for selection, proof, scoring, or verdict |
| stats/port.manifest.json | an owner-local measurement question or reference packet | aoa-stats for shared grammar; ToS authors the question | manifest, source rows, route map, and reference packet; return to the atlas owner, then central stats composition |
| kag/manifest.json | a downstream KAG/MCP consumer requests the portable provider family | aoa-kag for registry/composition; ToS derived exports remain source | source export, derived seam, local manifest/index family, and provider records; return to ToS/derived-exports/ and the KAG seam |
| memo/PORT.yaml | a reviewed candidate needs a tree-local memory intake | aoa-memo | candidate-only local route and generated index; reviewed handoff goes to aoa-memo, never directly to durable memory |

Local ports can be reference, candidate, generated, or receipt carriers. They
do not become live status, runtime truth, central eval authority, KAG substrate
authority, or durable memory authority by being present in Git.

## Task probes

Use the task probe matrix in the machine map for the smallest chain below.

| Task pressure | First route | Required chain | Negative control |
| --- | --- | --- | --- |
| source authority | aoa-source-of-truth-check | canonical file -> owner -> downstream projection only after source | do not promote a generated view, KAG record, or receipt to source |
| bounded context | aoa-bounded-context-map | contexts -> interfaces -> nearest owner contract | do not invent a new taxonomy when the boundary is already clear |
| repository change | aoa-change-protocol | root/nearest card -> bounded diff -> narrow check -> report | do not widen scope or hide an owner handoff in a copied procedure |
| session diagnosis/repair | aoa-session-self-diagnose -> aoa-session-self-repair | reviewed evidence -> diagnosis -> repair packet -> owner health check | do not diagnose live/unreviewed evidence or repair without a packet |
| approval or dry-run | aoa-approval-gate-check or aoa-dry-run-first | authority classification -> preview/confirmation -> owner carrier | dry-run does not grant authorization; approval does not replace preview |
| eval intake | evals/PORT.yaml -> aoa-evals | local pressure -> central selection/proof/verdict owner | local eval notes are not a verdict |
| owner-local stats | stats/port.manifest.json -> aoa-stats | question -> declared measurement -> evidence-linked packet -> central grammar | reference snapshot is not live state |
| KAG currentness | kag/AGENTS.md -> kag/manifest.json | source export -> derived seam -> local provider validation -> downstream registry/MCP | local provider is not runtime graph authority |
| memo candidate | memo/PORT.yaml -> aoa-memo | candidate-only write -> validation -> reviewed owner intake | candidate, index, or receipt is not durable memory |

## Context and safety

The map records a soft context budget: discovery descriptions stay under 100
words, triggered bodies under 2,000 words, and mandatory task-probe reading at
most five local route hops. Those are drift signals, not a claim that a task is
semantically complete. The generated companion records measured package bytes,
digests, word counts, companion counts, activation metadata, and the declared
owner-port input set.

Public authored model-facing docs do not carry host-local paths, secrets,
session snapshots, provider internals, transient runtime claims, or operator
evidence. Put those in task-local files, tool/MCP resources, status surfaces,
or typed receipts owned by the relevant organ.

Identical reference schemas and logo assets are intentionally retained inside
portable skill packages. Their repetition is an upstream packaging concern;
deduplication belongs in aoa-skills, followed by a local export refresh. This
map does not make a copied packet a second source owner.

## Validation

The focused command is the agent_surface lane in
docs/validation/validation_lanes.json: python scripts/validate_agent_surface.py.
It checks manifest/package parity, metadata activation parity, on-demand route
resolution, owner-port inputs, task-probe negative controls, public-safe
authored surfaces, and generated currentness. Release checks call the same
lane; skill-local helper scripts remain advisory unless their own owner
explicitly promotes them.
