---
name: aoa-automation-opportunity-scan
description: Detect reviewed or repeated project processes that are candidates for automation, classify whether they are seed-ready, and route them to the right owner layer without granting schedule or mutation authority. Use when a reviewed session or repeated project slice reveals a recurring manual route and the honest question is whether it should stay manual, become a bounded skill, or become a playbook automation seed candidate. Do not use for one-off exploratory work, creative synthesis with no stable trigger or output, live scheduling requests, or hidden self-change.
license: Apache-2.0
compatibility: Designed for Codex or similar coding agents with repository file access and an interactive shell. Network access is optional and only needed when repository validation or referenced workflows require it.
metadata:
  aoa_scope: core
  aoa_status: scaffold
  aoa_invocation_mode: explicit-only
  aoa_source_skill_path: skills/aoa-automation-opportunity-scan/SKILL.md
  aoa_source_repo: 8Dionysus/aoa-skills
  aoa_technique_dependencies: AOA-T-0086,AOA-T-0087,AOA-T-0088
  aoa_portable_profile: codex-facing-wave-3
---

# aoa-automation-opportunity-scan

## Intent
Use this skill to detect where repeated project work is ripe for automation and
to package that finding into a bounded `AUTOMATION_OPPORTUNITY_PACKET`.

The goal is not to automate by reflex.
The goal is to notice when a process has become repetitive, stable, legible,
and evidence-backed enough that automation or semi-automation becomes an honest
next move.

## Trigger boundary
Use this skill when:
- a reviewed session or repeated project slice reveals a recurring manual route
- the same audit, report, hygiene pass, or triage loop keeps coming back
- a session-harvest packet includes an `automation_candidate`
- an operator wants to know whether a recurring route should stay manual, become a bounded skill, or become a playbook automation seed
- repeated friction suggests a good automation opportunity but authority and risk still need classification

Do not use this skill when:
- the work happened only once and still looks exploratory
- the route is primarily creative synthesis with no stable trigger or output
- the task is asking for live scheduling or hidden background execution authority
- the route would mutate important system surfaces without checkpoint posture
- the work still needs basic donor harvest or source-of-truth clarification first

## Inputs
- reviewed session artifact, harvest packet, or recurring project slice
- evidence of repeated manual work
- known current manual route
- known triggers, inputs, outputs, or their current ambiguity
- current risk, approval, rollback, and proof posture if known

## Outputs
- `AUTOMATION_OPPORTUNITY_PACKET`
- one or more `AUTOMATION_CANDIDATE` cards
- `seed_ready` or `not_now` verdict for each candidate
- `checkpoint_required` flag when the route crosses self-change or approval-sensitive boundaries
- next-artifact suggestion such as `skill`, `playbook_seed`, `technique_candidate`, `repair_quest`, `quest`, or `defer`

## Procedure
1. start from reviewed evidence, not from vague enthusiasm
2. isolate the current manual route as actually practiced
3. classify repeat signal, friction, determinism, input clarity, output clarity, proof surface, reversibility, secret coupling, and approval sensitivity
4. decide whether the first honest landing is a bounded skill, a playbook automation seed candidate, a technique candidate, a repair quest, or a defer verdict
5. mark `checkpoint_required` when the route would cross into self-change, hidden authority, or important mutation
6. emit `seed_ready` only when the process is stable enough to name inputs, outputs, bounded prompts or activation hints, and a likely owner surface
7. state the nearest wrong target so promotion pressure stays honest
8. when classification pressure is high, use `references/automation-fit-matrix.md`, `references/session-harvest-integration.md`, `references/playbook-seed-bridge.md`, `references/checkpoint-boundary.md`, and `references/automation-opportunity-packet-schema.yaml`

## Contracts
- this skill detects and packages automation opportunities; it does not create live automation authority
- schedule hints remain hints, not runtime truth
- recurring scenario candidates belong in `aoa-playbooks` as seed candidates rather than hidden playbooks inside a skill
- self-changing or approval-heavy candidates must surface checkpoint posture explicitly
- unresolved candidates may become repair quests instead of fake-ready automations
- handoffs to `aoa-session-self-diagnose`, `aoa-session-self-repair`, or
  `aoa-quest-harvest` must stay explicit rather than being smuggled in as
  silent policy

## Risks and anti-patterns
- treating one exciting session as proof of repeatability
- automating unstable, under-specified, or secret-heavy work too early
- confusing a bounded skill with a recurring scenario seed
- using automation desire to bypass approval, rollback, or post-change health checks
- writing authored meaning first into derived routing or KAG surfaces
- inventing fake confidence about gains while hiding costs or risks

## Verification
- confirm each candidate names the current manual route
- confirm evidence refs justify the repeat signal
- confirm stable input and output posture is assessed explicitly
- confirm `seed_ready` or `not_now` is explicit for each candidate
- confirm a first owner layer and next artifact are named
- confirm the nearest wrong target is rejected when classification pressure exists
- confirm checkpoint posture appears for self-changing or approval-heavy routes

## Technique traceability
Manifest-backed techniques:
- AOA-T-0086 from `8Dionysus/aoa-techniques` at `1f1ec1b4f76d6e4726d3ea4343fbde834ddc6b1c` using path `techniques/agent-workflows/automation-fit-matrix/TECHNIQUE.md` and sections: Intent, Inputs, Outputs, Core procedure, Risks, Validation
- AOA-T-0087 from `8Dionysus/aoa-techniques` at `1f1ec1b4f76d6e4726d3ea4343fbde834ddc6b1c` using path `techniques/agent-workflows/human-loop-to-seed-lift/TECHNIQUE.md` and sections: Intent, When to use, Outputs, Core procedure, Contracts, Validation
- AOA-T-0088 from `8Dionysus/aoa-techniques` at `1f1ec1b4f76d6e4726d3ea4343fbde834ddc6b1c` using path `techniques/agent-workflows/approval-sensitivity-check/TECHNIQUE.md` and sections: Inputs, Outputs, Contracts, Risks, Validation

## Adaptation points
- project overlays may add local execution modes, trigger classes, or schedule vocabularies
- project overlays may add local approval classes or environment-risk labels
- project overlays may add local candidate examples tied to recurring repo rituals
