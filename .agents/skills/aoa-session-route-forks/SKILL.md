---
name: aoa-session-route-forks
description: Turn reviewed session evidence into explicit next-route forks with likely gains, costs, risks, owner targets, and stop conditions so continuation stays legible instead of buried in chat memory. Use when a reviewed session ended with several plausible next moves and you need visible branch choices rather than a hidden recommendation. Do not use when there is only one obvious next move, when donor harvest still has not happened, or when the real question is final promotion of one repeated quest unit.
license: Apache-2.0
compatibility: Designed for Codex or similar coding agents with repository file access and an interactive shell. Network access is optional and only needed when repository validation or referenced workflows require it.
metadata:
  aoa_scope: core
  aoa_status: scaffold
  aoa_invocation_mode: explicit-only
  aoa_source_skill_path: skills/aoa-session-route-forks/SKILL.md
  aoa_source_repo: 8Dionysus/aoa-skills
  aoa_technique_dependencies: AOA-T-0078,AOA-T-0079
  aoa_portable_profile: codex-facing-wave-3
---

# aoa-session-route-forks

## Intent
Use this skill to author `FORK_CARDS` from a reviewed session.

The goal is not prediction theater.
The goal is legible choice architecture: what routes exist, why each route
matters, what each one likely costs, and where each route probably lands first.

## Trigger boundary
Use this skill when:
- a reviewed session ended with multiple plausible next moves
- the operator or Codex needs explicit branch choices instead of a buried recommendation
- the next route may change owner repo, risk posture, or difficulty posture
- the session needs quest-board legibility without pretending to be runtime state

Do not use this skill when:
- there is only one obvious next bounded move
- the session still needs first-pass donor harvest
- the question is final promotion of a repeated quest unit
- the route needs scenario canon immediately rather than branch analysis

## Inputs
- reviewed session artifact or harvest packet
- candidate next routes
- known risks, dependencies, and blockers
- desired control mode or approval posture
- operator preference if already named

## Outputs
- `FORK_CARDS` with likely gain, cost, risk, owner repo, and stop conditions
- one suggested default route if evidence is strong enough
- one explicit hold or defer option when honest uncertainty remains
- optional quest hooks or campaign hints without runtime authority

## Procedure
1. start from reviewed evidence rather than free speculation
2. separate materially different branches instead of cosmetic variants
3. name the likely first owner repo for each branch
4. state likely gains, likely costs, likely risks, and stop conditions
5. attach a small route passport with difficulty, risk, control mode, and delegate tier
6. preserve a hold or reanchor path where uncertainty or risk remains meaningful
7. emit quest-board-readable language only as adjunct reflection

## Contracts
- branch cards do not become routing authority
- fork analysis must stay evidence-backed
- confidence should be named when weak
- stop conditions are first-class, not footnotes
- a fork card may recommend but must not hide alternatives

## Risks and anti-patterns
- fake certainty about future routes
- using fork cards as hidden routing policy
- collapsing all branches into one generic recommendation
- confusing playbook outline with branch analysis
- treating quest-board cards as runtime state

## Verification
- confirm each branch differs materially
- confirm likely owner repo is named
- confirm at least one cost or risk is explicit
- confirm stop conditions exist for risky branches
- confirm hold or defer remains possible when uncertainty is real

## Technique traceability
Manifest-backed techniques:
- AOA-T-0078 from `8Dionysus/aoa-techniques` at `364da8f4e97d0c29f4b31c59d7bfd91585633f2a` using path `techniques/agent-workflows/decision-fork-cards/TECHNIQUE.md` and sections: Intent, Inputs, Outputs, Core procedure, Validation
- AOA-T-0079 from `8Dionysus/aoa-techniques` at `364da8f4e97d0c29f4b31c59d7bfd91585633f2a` using path `techniques/agent-workflows/risk-passport-lift/TECHNIQUE.md` and sections: Outputs, Contracts, Risks, Validation

## Adaptation points
Project overlays may add:
- local control-mode labels
- local delegate tiers or approval classes
- repo-specific route passports
