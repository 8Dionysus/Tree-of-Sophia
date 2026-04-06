---
name: aoa-quest-harvest
description: Give the final promotion verdict on one repeated reviewed quest-shaped unit without collapsing skills, playbooks, orchestrator classes, proof, or memory into one layer. Use when a bounded quest-shaped work pattern has repeated, reviewed evidence exists, and you need the final honest verdict on whether it should stay a quest or move into the next owner surface. Do not use when the route is still active, only one anecdotal occurrence exists, the task is to invent net-new doctrine, or a broader reviewed session artifact still needs donor harvest, route forks, diagnosis, repair, or progression reflection first.
license: Apache-2.0
compatibility: Designed for Codex or similar coding agents with repository file access and an interactive shell. Network access is optional and only needed when repository validation or referenced workflows require it.
metadata:
  aoa_scope: core
  aoa_status: scaffold
  aoa_invocation_mode: explicit-only
  aoa_source_skill_path: skills/aoa-quest-harvest/SKILL.md
  aoa_source_repo: 8Dionysus/aoa-skills
  aoa_technique_dependencies: AOA-T-PENDING-QUEST-HARVEST,AOA-T-PENDING-PROMOTION-TRIAGE
  aoa_portable_profile: codex-facing-wave-3
---

# aoa-quest-harvest

## Intent
Use this skill to decide whether a repeated, reviewed quest-shaped pattern should remain a quest or be promoted into the next honest owner surface.

## Trigger boundary
Use this skill when:
- a bounded work pattern has repeated and now needs an explicit promotion decision
- reviewed evidence exists, but the correct destination is still unclear
- the decision must distinguish between skill, playbook, orchestrator surface, proof surface, memo surface, or staying a quest
- the route needs a compact final promotion triage rather than another free-form discussion

Do not use this skill when:
- the route is still active and the evidence has not been reviewed yet
- there is only one anecdotal occurrence with no honest repeat signal
- the task is to invent net-new doctrine rather than classify a repeated pattern
- the route is already clearly scenario-shaped and only needs playbook authoring
- the source is a reviewed session artifact with multiple candidate donor units that still need owner-layer routing first; use `aoa-session-donor-harvest`
- the source still needs explicit route forks, diagnosis, repair, or progression reflection from the wider session-harvest family before final promotion triage

## Inputs
- source quest or quest family
- reviewed run summary or harvest pack
- repeat count and repeat shape
- owner layer and touched surfaces
- difficulty, risk, and control posture
- residual ambiguity or reasons to defer promotion

## Outputs
- promotion verdict
- owner repo and follow-up surface
- explicit reason for promotion or non-promotion
- named next artifact or next quest action
- concise note on what boundary must remain intact

## Procedure
1. collect a bounded reviewed harvest pack rather than raw runtime state
2. name what actually repeated: leaf workflow, route, proof pattern, recall pattern, or boundary law
3. reject theme-only repetition; only repeatable work or repeatable law counts
4. if the repeated unit is a bounded leaf workflow, consider promotion to a skill
5. if the repeated unit is a multi-step scenario route, consider promotion to a playbook
6. if the repeated unit is orchestrator boundary law, read order, or allowed outputs, promote toward orchestrator class surfaces in `aoa-agents`
7. if the repeated unit is proof posture, promote toward `aoa-evals`
8. if the repeated unit is recall, writeback, or recurrence posture, promote toward `aoa-memo`
9. if repetition is still weak, owner is unclear, or boundary risk is high, keep or open a quest instead of forcing promotion
10. record the verdict with one clear reason for promotion and one clear reason against the nearest wrong target

## Contracts
- invocation must remain explicit and post-session
- orchestrator class identity must not be promoted into a skill
- a skill promotion must stay leaf-workflow-shaped
- a playbook promotion must stay route-shaped
- proof and memory promotions must stay in their owner layers
- one good run is not enough to justify promotion by itself
- active quest state must not be copied into memo canon as if it were settled truth

## Risks and anti-patterns
- promoting a theme instead of a repeatable workflow
- collapsing orchestrator class law into the skill layer
- collapsing recurring route method into a single skill
- treating proof debt as skill meaning
- treating memo writeback as active quest ownership
- forcing promotion just because the repeated work feels important

## Verification
- confirm the harvest pack is reviewed and bounded
- confirm the repeated unit is named explicitly
- confirm the chosen promotion target matches the owner layer
- confirm the nearest wrong target is explicitly rejected
- confirm class identity is not being defined by quest metadata
- confirm the result is one of the allowed outcomes: keep or open quest, skill, playbook, orchestrator surface, proof surface, or memo surface

## Technique traceability
Pending manifest-backed techniques:
- AOA-T-PENDING-QUEST-HARVEST from `8Dionysus/aoa-techniques` with `path: TBD` and `source_ref: TBD`; intended sections: Intent, When to use, Inputs, Outputs, Core procedure, Contracts, Validation
- AOA-T-PENDING-PROMOTION-TRIAGE from `8Dionysus/aoa-techniques` with `path: TBD` and `source_ref: TBD`; intended sections: Intent, When to use, Outputs, Core procedure, Risks, Validation

## Adaptation points
Project overlays may add:
- local quest IDs and harvest entrypoints
- local owner-repo route maps
- local review packet or acceptance surfaces
- local memo and eval references
- local stop conditions for when promotion must remain deferred
