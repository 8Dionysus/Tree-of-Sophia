---
name: aoa-session-self-repair
description: Turn a reviewed diagnosis packet into the smallest honest repair packet with checkpoint posture, rollback markers, health checks, and explicit owner-layer targets instead of silent self-mutation. Use when diagnosis already exists and the next honest move is a bounded repair plan or repair-ready packet. Do not use without a reviewed diagnosis, for playbook-scale rollout work, or for vague self-improvement rhetoric with no bounded target.
license: Apache-2.0
compatibility: Designed for Codex or similar coding agents with repository file access and an interactive shell. Network access is optional and only needed when repository validation or referenced workflows require it.
metadata:
  aoa_scope: core
  aoa_status: evaluated
  aoa_invocation_mode: explicit-only
  aoa_source_skill_path: skills/core/session-growth/aoa-session-self-repair/SKILL.md
  aoa_source_repo: 8Dionysus/aoa-skills
  aoa_technique_dependencies: AOA-T-0082,AOA-T-0083
  aoa_portable_profile: codex-facing-wave-3
---

# aoa-session-self-repair

## Intent
Use this skill to author a bounded `REPAIR_PACKET`.

The repair packet should be small, explicit, reversible where needed, and
aligned with checkpoint posture before any important system surface is changed.
It should also say whether the repair is only proposed, prepared, executing,
verified, blocked, or handed off.

## Trigger boundary
Use this skill when:
- a reviewed diagnosis already exists
- the next honest move is a bounded repair plan or repair-ready packet
- the route may change skill, playbook, agent, eval, or memo surfaces and needs explicit checkpoint posture
- the route may need prerequisite repair before later automation becomes honest
- repair can still be kept inside one bounded execution unit
- a prepared repair packet or receipt must not be mistaken for executed or verified repair

Do not use this skill when:
- there is no reviewed diagnosis yet
- the repair is actually a large scenario rollout better owned by a playbook
- the route is trying to bypass approval, rollback, or health-check posture
- the request is vague self-improvement rhetoric with no bounded target
- the route only needs a diagnosis packet, route fork, automation-readiness scan, or final quest-promotion verdict

## Inputs
- reviewed diagnosis packet
- owner-layer candidates
- risk and approval posture
- known validation surfaces
- current rollback anchors if any
- current execution state, if a repair packet already exists
- evidence showing whether validation actually ran or is only planned

## Outputs
- `REPAIR_PACKET` with target owner repo, smallest diff shape, approval need, rollback marker, health check, and improvement-log stub
- explicit repair execution posture such as proposed, prepared, executing, verified, blocked, or handoff_required
- optional repair quest when execution should remain deferred
- optional automation-readiness prerequisite packet when the real need is to
  stabilize a route before later automation scanning or seeding
- explicit stop conditions and escalation points
- one `REPAIR_CYCLE_RECEIPT` using `references/stats-event-envelope.md` and
  `references/repair-cycle-receipt-schema.yaml`
- one `CORE_SKILL_APPLICATION_RECEIPT` using
  `references/core-skill-application-receipt-schema.yaml`

## Procedure
1. start from the reviewed diagnosis rather than from general aspiration
2. choose the smallest honest repair shape
3. name the primary owner repo and target artifact class
4. state the repair execution posture before writing any outcome language: proposed, prepared, executing, verified, blocked, or handoff_required
5. record checkpoint posture: constitution or policy check, approval gate, rollback marker, post-change health check, bounded iteration limit, improvement log
6. if validation has not actually run, keep health checks as planned checks and set execution posture below verified
7. if the target route was blocked automation, emit the smallest prerequisite repair that would make later automation classification more honest
8. define validation and stop conditions
9. emit a repair quest instead of mutating immediately when risk or approval posture requires it
10. emit one `REPAIR_CYCLE_RECEIPT` when the repair packet or repair-quest handoff closes, keeping the receipt smaller than the packet
11. when the finish path closes, emit one `CORE_SKILL_APPLICATION_RECEIPT`
   that points back to the bounded repair receipt and records one finished
   kernel-skill application

## Contracts
- self-repair is not free self-modification
- important surface changes must pass checkpoint posture
- repair packets stay bounded and reviewable
- prepared repair packets are not executed repairs, and executed repairs are not verified repairs unless validation evidence is present
- role law changes route to `aoa-agents`
- proof-law changes route to `aoa-evals`
- scenario-scale repair routes to `aoa-playbooks`
- repair does not smuggle live automation authority into the packet
- repair-cycle receipts stay descriptive and append-only
- generic core receipts stay subordinate to the repair receipt and do not
  replace checkpoint or repair meaning
- receipt corrections use `supersedes` rather than silent overwrite

## Risks and anti-patterns
- silent doctrine edits
- approval bypass
- retry loops disguised as repair
- using repair to hide broader governance debt
- changing too many surfaces at once
- letting a repair receipt pretend the repair is already verified when it is only planned
- using `resolved`, `prepared`, or `finished` without saying what was actually executed and verified

## Verification
- confirm diagnosis exists and is cited
- confirm the chosen repair is the smallest honest shape
- confirm checkpoint fields are present
- confirm repair execution posture is explicit and does not overclaim validation
- confirm validation and rollback posture are named
- confirm escalation route exists if the repair widens
- confirm any emitted receipt cites diagnosis and validation refs without duplicating the whole packet
- confirm any emitted `CORE_SKILL_APPLICATION_RECEIPT` points to the repair
  detail receipt and stays finish-only

## Technique traceability
Manifest-backed techniques:
- AOA-T-0082 from `8Dionysus/aoa-techniques` at `b0f7e094bf81abaf0895a729d504a97f3af91ae8` using path `techniques/recovery/diagnosis-repair/repair-shape-from-diagnosis/TECHNIQUE.md` and sections: Intent, Inputs, Outputs, Core procedure, Validation
- AOA-T-0083 from `8Dionysus/aoa-techniques` at `b0f7e094bf81abaf0895a729d504a97f3af91ae8` using path `techniques/recovery/diagnosis-repair/checkpoint-bound-self-repair/TECHNIQUE.md` and sections: Outputs, Contracts, Risks, Validation

## Adaptation points
Project overlays may add:
- local approval classes
- repo-specific rollback anchors
- bounded repair templates
