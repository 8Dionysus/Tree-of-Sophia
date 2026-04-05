---
name: aoa-session-donor-harvest
description: Harvest reusable pattern, mechanic, utility, law, proof, recall, or route candidates from a reviewed session artifact, route each one to the right AoA owner layer, and draft the smallest honest next artifact. Use when the source is a reviewed session transcript, compaction note, or recap and the real question is what reusable object emerged here rather than merely what happened. Do not use for active or unreviewed sessions, raw session capture or indexing work, or when there is already one repeated quest unit that only needs `aoa-quest-harvest` for the final promotion verdict.
license: Apache-2.0
compatibility: Designed for Codex or similar coding agents with repository file access and an interactive shell. Network access is optional and only needed when repository validation or referenced workflows require it.
metadata:
  aoa_scope: core
  aoa_status: scaffold
  aoa_invocation_mode: explicit-only
  aoa_source_skill_path: skills/aoa-session-donor-harvest/SKILL.md
  aoa_source_repo: 8Dionysus/aoa-skills
  aoa_technique_dependencies: AOA-T-PENDING-SESSION-DONOR-HARVEST,AOA-T-PENDING-OWNER-LAYER-TRIAGE
  aoa_portable_profile: codex-facing-wave-3
---

# aoa-session-donor-harvest

## Intent
Use this skill to turn a reviewed session artifact into a bounded donor harvest pack that names reusable units, routes each one to the right AoA owner layer, and drafts the next honest artifact without forcing promotion.

## Trigger boundary
Use this skill when:
- a session transcript, compaction note, review packet, or bounded recap exists and must be distilled into reusable donor units
- the work is post-session and reviewable rather than live execution
- candidate outputs may belong in technique canon, `aoa-skills`, `aoa-playbooks`, `aoa-evals`, `aoa-memo`, `aoa-agents`, or a hold/quest lane
- the question is not merely "what happened?" but "what reusable object, if any, emerged here?"

Do not use this skill when:
- the session is still active or unreviewed
- the task is raw session capture, transcript export, or local indexing; those are history techniques, not this skill
- the result is clearly one bounded repeated quest unit that only needs the narrower final promotion verdict from `aoa-quest-harvest`
- the material is only a progress log, emotional recap, or theme cloud with no bounded reusable unit
- the intended first destination is `aoa-routing` or `aoa-kag`; those are derivative layers and should not receive source-owned meaning first

## Inputs
- reviewed session artifact
- session goal and closure state
- candidate repeat signals or reuse signals
- touched repos or AoA layers
- explicit uncertainty and boundary risks
- desired output posture: classify-only, draft-stub, or patch-ready proposal

## Outputs
- one bounded donor harvest pack
- named candidates, each with:
  - reusable unit name
  - unit kind: pattern, mechanic, utility, law, proof, recall, or route
  - abstraction shape: technique, skill, playbook, eval, memo, agent, or hold
  - owner repo recommendation
  - one chosen next artifact
  - one rejected nearest-wrong target
  - evidence anchors from the session artifact
- one short list of items to defer, drop, or keep as quest residue

## Procedure
1. start from a reviewed session artifact rather than transient chat memory
2. extract candidate reusable units, not topics; prefer explicit moves, laws, checklists, structures, routes, or proof patterns
3. split merged candidates until each unit has one honest owner shape
4. classify each kept candidate twice:
   - by reuse kind: pattern, mechanic, utility, law, proof, recall, or route
   - by owner shape: technique, skill, playbook, eval, memo, agent, or hold
5. reject theme-only repetition, aesthetic resonance, and broad "good idea" residue unless a bounded reusable unit exists
6. route reusable practice meaning to technique canon first
7. route bounded executable leaf workflows to `aoa-skills`
8. route multi-step recurring scenario methods to `aoa-playbooks`
9. route rubrics, verdict postures, and proof surfaces to `aoa-evals`
10. route recall, writeback, recurrence, and memory-support patterns to `aoa-memo`
11. route role law, orchestrator class law, handoff law, and actor-boundary rules to `aoa-agents`
12. keep `aoa-routing` and `aoa-kag` out of first-authoring unless the source-owned object already exists elsewhere and the session only discovered a derivative bridge update
13. when the candidate is a repeated reviewed quest unit and the remaining ambiguity is specifically the final promotion target among quest, skill, playbook, agent, eval, or memo, hand off to `aoa-quest-harvest`
14. draft the smallest next artifact for each accepted candidate, such as `TECHNIQUE.md`, `SKILL.md`, `PLAYBOOK.md`, `EVAL.md`, memory object seed, or agent/orchestrator surface note
15. record one clear reason for the chosen owner and one clear reason against the nearest wrong owner

## Contracts
- invocation must remain explicit and post-session
- the skill harvests donor units; it does not treat session history as memory canon or instruction authority
- one candidate must map to one primary owner layer
- `usefulness` is a reuse signal, not an owner layer by itself
- derivative layers do not become first authoring targets for source-owned meaning
- weak evidence may end in `hold` or `keep_or_open_quest`
- drafting a next artifact is allowed; forcing promotion is not

## Risks and anti-patterns
- collapsing technique, skill, and playbook into one generic "good reuse" bucket
- mistaking session themes for reusable units
- routing authored meaning into `aoa-routing` or `aoa-kag` first
- turning agent class law into a skill just because an agent repeatedly used a workflow
- promoting memory residue as if it were proof or source meaning
- over-harvesting one session into too many thin objects
- treating a transcript package or session index as the same thing as donor harvest

## Verification
- confirm the source artifact is reviewed and bounded
- confirm each kept candidate names one reusable unit rather than a topic
- confirm each accepted candidate has one primary owner layer
- confirm the nearest wrong target is rejected explicitly
- confirm no candidate routes source-owned meaning first into derivative layers
- confirm hold and defer outcomes remain available
- confirm the output names the next artifact rather than only abstract categories

## Technique traceability
Pending manifest-backed techniques:
- AOA-T-PENDING-SESSION-DONOR-HARVEST from `8Dionysus/aoa-techniques` with `path: TBD` and `source_ref: TBD`; intended sections: Intent, When to use, Inputs, Outputs, Core procedure, Contracts, Validation
- AOA-T-PENDING-OWNER-LAYER-TRIAGE from `8Dionysus/aoa-techniques` with `path: TBD` and `source_ref: TBD`; intended sections: Intent, When to use, Outputs, Core procedure, Risks, Validation

## Adaptation points
Project overlays may add:
- local session artifact entrypoints
- local review packet templates
- repo-relative destination paths for drafted artifacts
- local stop conditions for when the result must stay `hold`
- local naming rules for donor packs and quest IDs

This skill assumes the session artifact already exists. Adjacent history techniques such as session capture, transcript packaging, and local indexing remain separate neighbors rather than being reopened here.
