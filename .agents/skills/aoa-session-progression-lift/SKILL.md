---
name: aoa-session-progression-lift
description: Lift reviewed session evidence into a bounded multi-axis progression delta with explicit unlock hints, quest reflection cues, and no fake single-score authority. Use when a reviewed session produced meaningful mastery evidence and you need progression legibility without mutating source role profiles or routing authority. Do not use when there is no reviewed evidence, when the request wants one universal power number, or when progression language is being used as hidden policy.
license: Apache-2.0
compatibility: Designed for Codex or similar coding agents with repository file access and an interactive shell. Network access is optional and only needed when repository validation or referenced workflows require it.
metadata:
  aoa_scope: core
  aoa_status: evaluated
  aoa_invocation_mode: explicit-only
  aoa_source_skill_path: skills/core/session-growth/aoa-session-progression-lift/SKILL.md
  aoa_source_repo: 8Dionysus/aoa-skills
  aoa_technique_dependencies: AOA-T-0084,AOA-T-0085
  aoa_portable_profile: codex-facing-wave-3
---

# aoa-session-progression-lift

## Intent
Use this skill to author a `PROGRESSION_DELTA` from reviewed evidence.

This is not score inflation.
It is an evidence-backed overlay that says what mastery axes moved, what should
hold or reanchor, and what small unlock hints are now safer.

## Trigger boundary
Use this skill when:
- a reviewed session generated meaningful mastery evidence
- the route needs progression legibility without mutating source role profiles
- quest or RPG reflection would help continuation
- the output needs to stay small, evidence-backed, and multi-axis

Do not use this skill when:
- there is no reviewed evidence
- the request wants one global power number
- progression is being used as hidden routing policy
- the route is trying to mint authority rights without evidence

## Inputs
- reviewed session artifact or harvest packet
- named evidence refs
- relevant role or cohort context if known
- existing progression baseline if available

## Outputs
- `PROGRESSION_DELTA` with axis movement, verdict, and optional unlock hints
- optional automation-readiness hint when reviewed evidence supports it
- optional rank reflection note if evidence is strong enough
- quest hooks or chronicle stub when useful
- negative or cautionary evidence when a hold, reanchor, or downgrade is more honest than advance
- one `PROGRESSION_DELTA_RECEIPT` using `references/stats-event-envelope.md`
  and `references/progression-delta-receipt-schema.yaml`
- one `CORE_SKILL_APPLICATION_RECEIPT` using
  `references/core-skill-application-receipt-schema.yaml`

## Procedure
1. collect reviewed evidence refs
2. assess movement qualitatively across `boundary_integrity`,
   `execution_reliability`, `change_legibility`, `review_sharpness`,
   `proof_discipline`, `provenance_hygiene`, and `deep_readiness`
3. emit a verdict: advance, hold, reanchor, or downgrade
4. name small unlock hints only when evidence supports them
5. keep any automation-readiness hint small, descriptive, and non-authoritative
6. allow negative, zero, and cautionary movement
7. map ability or feat hints only as reflection, not as ownership transfer
8. emit one `PROGRESSION_DELTA_RECEIPT` when the delta closes, keeping the
   receipt descriptive, evidence-linked, and smaller than the progression packet
9. when the finish path closes, emit one `CORE_SKILL_APPLICATION_RECEIPT`
   that points back to the bounded progression receipt and records one
   finished kernel-skill application

## Contracts
- progression remains evidence-backed
- multi-axis only; no authoritative universal score
- rank labels are descriptive, not sovereign
- unlock hints must stay reviewable and small
- progression does not replace owner-layer truth or routing authority
- progression does not greenlight automation by itself
- progression receipts stay descriptive and append-only
- generic core receipts stay subordinate to the progression receipt and do not
  become progression authority
- receipt corrections use `supersedes` rather than silent overwrite

## Risks and anti-patterns
- inventing progress from mood
- using progression as policy
- granting authority without cited evidence
- flattening multi-axis growth into one number
- confusing quest flavor with durable proof
- treating progression hints as if they were approval or schedule rights
- treating a progression receipt as if receipt emission itself proved growth

## Verification
- confirm all meaningful axis claims cite reviewed evidence
- confirm the verdict matches the evidence
- confirm zero or negative movement remains allowed
- confirm unlock hints are small and explicit
- confirm no universal score is introduced
- confirm any emitted receipt stays multi-axis, evidence-linked, and non-sovereign
- confirm any emitted `CORE_SKILL_APPLICATION_RECEIPT` points to the
  progression detail receipt and stays finish-only

## Technique traceability
Manifest-backed techniques:
- AOA-T-0084 from `8Dionysus/aoa-techniques` at `cd276f040d55d490bd015b8698c7a5d594b9f875` using path `techniques/continuity/donor-harvest/progression-evidence-lift/TECHNIQUE.md` and sections: Intent, Inputs, Outputs, Contracts, Validation
- AOA-T-0085 from `8Dionysus/aoa-techniques` at `cd276f040d55d490bd015b8698c7a5d594b9f875` using path `techniques/continuity/donor-harvest/multi-axis-quest-overlay/TECHNIQUE.md` and sections: Outputs, Risks, Validation

## Adaptation points
Project overlays may add:
- local axis notes
- role-affinity hints
- local unlock classes
