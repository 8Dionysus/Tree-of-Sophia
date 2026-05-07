---
name: aoa-quest-harvest
description: Give the final promotion verdict on one repeated reviewed quest-shaped unit without collapsing skills, playbooks, orchestrator classes, proof, or memory into one layer. Use when a bounded quest-shaped work pattern has repeated, reviewed evidence exists, and you need the final honest verdict on whether it should stay a quest or move into the next owner surface. Do not use when the route is still active, only one anecdotal occurrence exists, the task is to invent net-new doctrine, or a broader reviewed session artifact still needs donor harvest, route forks, diagnosis, repair, or progression reflection first.
license: Apache-2.0
compatibility: Designed for Codex or similar coding agents with repository file access and an interactive shell. Network access is optional and only needed when repository validation or referenced workflows require it.
metadata:
  aoa_scope: core
  aoa_status: evaluated
  aoa_invocation_mode: explicit-only
  aoa_source_skill_path: skills/core/session-growth/aoa-quest-harvest/SKILL.md
  aoa_source_repo: 8Dionysus/aoa-skills
  aoa_technique_dependencies: AOA-T-0089,AOA-T-0090
  aoa_portable_profile: codex-facing-wave-3
---

# aoa-quest-harvest

## Intent
Use this skill to decide whether one repeated, reviewed quest-shaped unit should remain a quest or be promoted into the next honest owner surface.

The output is a promotion verdict, not authorship completion and not acceptance
by the destination owner repository. It should also say whether repetition is
reviewed and isolated, reviewed but weak, contested, owner-pending, or still
too mixed for promotion.

## Trigger boundary
Use this skill when:
- a bounded work pattern has repeated and now needs an explicit promotion decision
- reviewed evidence exists, but the correct destination is still unclear
- donor harvest or equivalent review has already isolated one repeated unit
  from broader session residue
- the repeat signal needs to be distinguished from topic recurrence,
  conversation recurrence, owner-pending follow-through, or mixed donor residue
- the decision must distinguish between skill, playbook, orchestrator surface, proof surface, memo surface, or staying a quest
- the route needs a compact final promotion triage rather than another free-form discussion

Do not use this skill when:
- the route is still active and the evidence has not been reviewed yet
- there is only one anecdotal occurrence with no honest repeat signal
- the task is to invent net-new doctrine rather than classify a repeated pattern
- the repeated unit is still mixed with several reusable donor units, unresolved
  route forks, diagnosis, repair, automation-readiness questions, or
  progression reflection
- the repeat evidence posture cannot be stated without guessing
- the route is already clearly scenario-shaped and only needs playbook authoring
- the source is a reviewed session artifact with multiple candidate donor units that still need owner-layer routing first; use `aoa-session-donor-harvest`
- the source still needs explicit automation-readiness classification for a
  repeated manual route; use `aoa-automation-opportunity-scan`
- the source still needs explicit route forks, diagnosis, repair, or progression reflection from the wider session-harvest family before final promotion triage

## Inputs
- source quest or quest family
- reviewed run summary or harvest pack
- repeat count and repeat shape
- isolated repeated unit ref, or an explicit statement that the unit was
  isolated during reviewed harvest
- repeat evidence posture
- owner layer and touched surfaces
- difficulty, risk, and control posture
- residual ambiguity or reasons to defer promotion

## Outputs
- promotion verdict
- owner repo and follow-up surface
- explicit reason for promotion or non-promotion
- repeat evidence posture such as `reviewed_repeat_isolated`,
  `reviewed_repeat_weak`, `contested_repeat`, `topic_only_repeat`,
  `mixed_residue`, or `owner_pending`
- explicit statement that the verdict is smaller than authoring or owner
  acceptance in the destination repo
- named next artifact or next quest action
- concise note on what boundary must remain intact
- one `QUEST_PROMOTION_RECEIPT` using `references/stats-event-envelope.md`
  and `references/quest-promotion-receipt-schema.yaml`
- one `CORE_SKILL_APPLICATION_RECEIPT` using
  `references/core-skill-application-receipt-schema.yaml`

## Procedure
1. collect a bounded reviewed harvest pack rather than raw runtime state
2. confirm there is exactly one isolated repeated unit; if several units remain
   mixed, return to donor harvest, route forks, diagnosis, repair, automation
   scan, or progression lift instead of forcing triage
3. name the repeat evidence posture before choosing a destination
4. name what actually repeated: leaf workflow, route, proof pattern, recall pattern, or boundary law
5. reject theme-only repetition, repeated discussion, and aesthetic similarity;
   only repeatable work or repeatable law counts
6. if the repeated unit is a bounded leaf workflow, consider promotion to a skill
7. if the repeated unit is a multi-step scenario route, consider promotion to a playbook
8. if the repeated unit is orchestrator boundary law, read order, or allowed outputs, promote toward orchestrator class surfaces in `aoa-agents`
9. if the repeated unit is proof posture, promote toward `aoa-evals`
10. if the repeated unit is recall, writeback, or recurrence posture, promote toward `aoa-memo`
11. if repetition is still weak, owner is unclear, boundary risk is high, repeat
    posture is contested or mixed, or
    the destination owner has not accepted the work shape, keep or open a quest
    instead of forcing promotion
12. record the verdict with one clear reason for promotion and one clear reason against the nearest wrong target
13. name the next artifact as a proposal or follow-through target, not as proof
    that the owner repo has accepted or authored it
14. emit one `QUEST_PROMOTION_RECEIPT` when the triage closes, keeping the
    receipt smaller than the promotion packet itself
15. when the finish path closes, emit one `CORE_SKILL_APPLICATION_RECEIPT`
    that points back to the bounded promotion receipt and records one finished
    kernel-skill application

## Contracts
- invocation must remain explicit and post-session
- orchestrator class identity must not be promoted into a skill
- a skill promotion must stay leaf-workflow-shaped
- a playbook promotion must stay route-shaped
- proof and memory promotions must stay in their owner layers
- one good run is not enough to justify promotion by itself
- repeated discussion is not the same thing as repeated work
- repeat evidence posture must stay visible when repetition is weak, mixed,
  contested, or owner-pending
- a promotion verdict is not downstream authoring, downstream acceptance, or
  owner truth
- active quest state must not be copied into memo canon as if it were settled truth
- `QUEST_PROMOTION_RECEIPT` is verdict telemetry, not promotion authority
- `CORE_SKILL_APPLICATION_RECEIPT` is generic kernel telemetry, not promotion
  authority and not a replacement for the verdict receipt
- receipt corrections use `supersedes` rather than silent overwrite

## Risks and anti-patterns
- promoting a theme instead of a repeatable workflow
- treating repeated discussion, urgency, or excitement as repeat evidence
- hiding weak or contested repeat evidence behind a confident promotion target
- collapsing orchestrator class law into the skill layer
- collapsing recurring route method into a single skill
- treating proof debt as skill meaning
- treating memo writeback as active quest ownership
- forcing promotion just because the repeated work feels important
- presenting the next surface as already accepted by `aoa-skills`,
  `aoa-playbooks`, `aoa-agents`, `aoa-evals`, or `aoa-memo`

## Verification
- confirm the harvest pack is reviewed and bounded
- confirm exactly one repeated reviewed unit is isolated before final triage
- confirm repeat evidence posture is named and supports the verdict
- confirm the repeated unit is named explicitly
- confirm the chosen promotion target matches the owner layer
- confirm the nearest wrong target is explicitly rejected
- confirm class identity is not being defined by quest metadata
- confirm the result is one of the allowed outcomes: keep or open quest, skill, playbook, orchestrator surface, proof surface, or memo surface
- confirm the next artifact is framed as a proposal or follow-through target,
  not as downstream owner acceptance
- confirm any emitted receipt stays evidence-linked and subordinate to the closed triage
- confirm any emitted `CORE_SKILL_APPLICATION_RECEIPT` points to the
  promotion detail receipt and stays finish-only

## Technique traceability
Manifest-backed techniques:
- AOA-T-0089 from `8Dionysus/aoa-techniques` at `b4e5c1446469f142c60a85d0a7a4d9de7835ea65` using path `techniques/governance/promotion-boundary/quest-unit-promotion-review/TECHNIQUE.md` and sections: Intent, When to use, Inputs, Outputs, Core procedure, Contracts, Validation
- AOA-T-0090 from `8Dionysus/aoa-techniques` at `b4e5c1446469f142c60a85d0a7a4d9de7835ea65` using path `techniques/governance/promotion-boundary/nearest-wrong-target-rejection/TECHNIQUE.md` and sections: Intent, When to use, Outputs, Core procedure, Risks, Validation

## Adaptation points
Project overlays may add:
- local quest IDs and harvest entrypoints
- local owner-repo route maps
- local review packet or acceptance surfaces
- local memo and eval references
- local stop conditions for when promotion must remain deferred
- local destination-owner acceptance checks that happen after this verdict and
  outside the skill itself
- local repeat evidence posture vocabulary, provided it still distinguishes
  reviewed isolated repetition from weak, contested, topic-only, mixed, or
  owner-pending repetition
