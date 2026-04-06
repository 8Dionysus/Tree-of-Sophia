---
name: aoa-session-self-repair
description: Turn a reviewed diagnosis packet into the smallest honest repair packet with checkpoint posture, rollback markers, health checks, and explicit owner-layer targets instead of silent self-mutation. Use when diagnosis already exists and the next honest move is a bounded repair plan or repair-ready packet. Do not use without a reviewed diagnosis, for playbook-scale rollout work, or for vague self-improvement rhetoric with no bounded target.
license: Apache-2.0
compatibility: Designed for Codex or similar coding agents with repository file access and an interactive shell. Network access is optional and only needed when repository validation or referenced workflows require it.
metadata:
  aoa_scope: core
  aoa_status: scaffold
  aoa_invocation_mode: explicit-only
  aoa_source_skill_path: skills/aoa-session-self-repair/SKILL.md
  aoa_source_repo: 8Dionysus/aoa-skills
  aoa_technique_dependencies: AOA-T-0082,AOA-T-0083
  aoa_portable_profile: codex-facing-wave-3
---

# aoa-session-self-repair

## Intent
Use this skill to author a bounded `REPAIR_PACKET`.

The repair packet should be small, explicit, reversible where needed, and
aligned with checkpoint posture before any important system surface is changed.

## Trigger boundary
Use this skill when:
- a reviewed diagnosis already exists
- the next honest move is a bounded repair plan or repair-ready packet
- the route may change skill, playbook, agent, eval, or memo surfaces and needs explicit checkpoint posture
- the route may need prerequisite repair before later automation becomes honest
- repair can still be kept inside one bounded execution unit

Do not use this skill when:
- there is no reviewed diagnosis yet
- the repair is actually a large scenario rollout better owned by a playbook
- the route is trying to bypass approval, rollback, or health-check posture
- the request is vague self-improvement rhetoric with no bounded target

## Inputs
- reviewed diagnosis packet
- owner-layer candidates
- risk and approval posture
- known validation surfaces
- current rollback anchors if any

## Outputs
- `REPAIR_PACKET` with target owner repo, smallest diff shape, approval need, rollback marker, health check, and improvement-log stub
- optional repair quest when execution should remain deferred
- optional automation-readiness prerequisite packet when the real need is to
  stabilize a route before later automation scanning or seeding
- explicit stop conditions and escalation points

## Procedure
1. start from the reviewed diagnosis rather than from general aspiration
2. choose the smallest honest repair shape
3. name the primary owner repo and target artifact class
4. record checkpoint posture: constitution or policy check, approval gate, rollback marker, post-change health check, bounded iteration limit, improvement log
5. if the target route was blocked automation, emit the smallest prerequisite repair that would make later automation classification more honest
6. define validation and stop conditions
7. emit a repair quest instead of mutating immediately when risk or approval posture requires it

## Contracts
- self-repair is not free self-modification
- important surface changes must pass checkpoint posture
- repair packets stay bounded and reviewable
- role law changes route to `aoa-agents`
- proof-law changes route to `aoa-evals`
- scenario-scale repair routes to `aoa-playbooks`
- repair does not smuggle live automation authority into the packet

## Risks and anti-patterns
- silent doctrine edits
- approval bypass
- retry loops disguised as repair
- using repair to hide broader governance debt
- changing too many surfaces at once

## Verification
- confirm diagnosis exists and is cited
- confirm the chosen repair is the smallest honest shape
- confirm checkpoint fields are present
- confirm validation and rollback posture are named
- confirm escalation route exists if the repair widens

## Technique traceability
Manifest-backed techniques:
- AOA-T-0082 from `8Dionysus/aoa-techniques` at `364da8f4e97d0c29f4b31c59d7bfd91585633f2a` using path `techniques/agent-workflows/repair-shape-from-diagnosis/TECHNIQUE.md` and sections: Intent, Inputs, Outputs, Core procedure, Validation
- AOA-T-0083 from `8Dionysus/aoa-techniques` at `364da8f4e97d0c29f4b31c59d7bfd91585633f2a` using path `techniques/agent-workflows/checkpoint-bound-self-repair/TECHNIQUE.md` and sections: Outputs, Contracts, Risks, Validation

## Adaptation points
Project overlays may add:
- local approval classes
- repo-specific rollback anchors
- bounded repair templates
