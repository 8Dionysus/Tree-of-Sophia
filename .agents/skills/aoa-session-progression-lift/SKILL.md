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
hold or reanchor, what baseline the movement is compared against, and what
small unlock hints are now safer.

## Trigger boundary
Use this skill when:
- a reviewed session generated meaningful mastery evidence
- the route needs progression legibility without mutating source role profiles
- donor harvest, checkpoint closeout, or a reviewed handoff produced provisional
  axis hints that must be accepted, rejected, or carried without becoming
  growth claims by themselves
- movement needs to be distinguished from first observation, stale baseline,
  missing baseline, or regression against a known prior state
- quest or RPG reflection would help continuation
- the output needs to stay small, evidence-backed, and multi-axis

Do not use this skill when:
- there is no reviewed evidence
- the only input is a checkpoint axis hint, mood note, or generated summary
  that has not been reread against reviewed session evidence
- the route wants comparative movement while refusing to name a baseline or
  admit that the baseline is missing, stale, or contested
- the request wants one global power number
- progression is being used as hidden routing policy
- the route is trying to mint authority rights without evidence

## Inputs
- reviewed session artifact or harvest packet
- named evidence refs
- optional donor harvest output, checkpoint-closeout stage result, or
  provisional axis hints
- current-session boundary for separating stale or neighboring evidence
- baseline ref, prior delta ref, or an explicit no-baseline marker
- relevant role or cohort context if known
- existing progression baseline if available

## Outputs
- `PROGRESSION_DELTA` with axis movement, verdict, and optional unlock hints
- baseline posture such as `baseline_ref`, `prior_delta_ref`,
  `first_observed`, `baseline_missing`, `baseline_stale`, or
  `baseline_contested`
- one axis table where each meaningful axis names movement, evidence refs,
  evidence posture, and any defer or no-movement reason
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
2. separate confirmed current-session evidence from checkpoint hints,
   closeout-handoff cues, generated summaries, stale residue, and neighboring
   session evidence
3. declare the movement basis before assigning any axis movement: baseline ref,
   prior delta ref, first-observed posture, missing baseline, stale baseline, or
   contested baseline
4. assess movement qualitatively across `boundary_integrity`,
   `execution_reliability`, `change_legibility`, `review_sharpness`,
   `proof_discipline`, `provenance_hygiene`, and `deep_readiness`
5. mark each touched axis with evidence posture: `confirmed`, `contested`,
   `provisional`, `stale`, `not_current_session`, or `no_movement`
6. emit a verdict: advance, hold, reanchor, or downgrade
7. when the baseline is missing, stale, or contested, avoid comparative advance
   language unless a specific current-session axis has confirmed evidence and
   the limitation remains visible
8. when evidence is only provisional or contested, prefer hold, reanchor, or an
   explicit defer reason over invented advance
9. name small unlock hints only when evidence supports them
10. keep any automation-readiness hint small, descriptive, and non-authoritative
11. allow negative, zero, and cautionary movement
12. map ability or feat hints only as reflection, not as ownership transfer
13. hand off to `aoa-quest-harvest` only when a repeated reviewed quest unit
   survives as a final promotion question; otherwise keep quest hooks as
   reflection or defer them
14. emit one `PROGRESSION_DELTA_RECEIPT` when the delta closes, keeping the
   receipt descriptive, evidence-linked, and smaller than the progression packet
15. when the finish path closes, emit one `CORE_SKILL_APPLICATION_RECEIPT`
   that points back to the bounded progression receipt and records one
   finished kernel-skill application

## Contracts
- progression remains evidence-backed
- multi-axis only; no authoritative universal score
- movement claims must name their baseline posture; missing baseline can support
  first-observed evidence notes, not fake comparative growth
- checkpoint, donor, and closeout hints may focus the review but do not become
  progression claims until reviewed evidence supports them
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
- promoting checkpoint axis hints or generated summaries into movement without
  reviewed evidence
- calling a first observation an improvement without naming the absent baseline
- treating absence of a problem as evidence of mastery movement
- using progression as policy
- granting authority without cited evidence
- flattening multi-axis growth into one number
- confusing quest flavor with durable proof
- treating progression hints as if they were approval or schedule rights
- treating a progression receipt as if receipt emission itself proved growth

## Verification
- confirm all meaningful axis claims cite reviewed evidence
- confirm the delta declares baseline posture before comparative movement claims
- confirm provisional, stale, contested, and no-movement axes are marked rather
  than smoothed into progress
- confirm the verdict matches the evidence
- confirm zero or negative movement remains allowed
- confirm unlock hints are small and explicit
- confirm no universal score is introduced
- confirm any emitted receipt stays multi-axis, evidence-linked, and non-sovereign
- confirm any emitted `CORE_SKILL_APPLICATION_RECEIPT` points to the
  progression detail receipt and stays finish-only

## Technique traceability
Manifest-backed techniques:
- AOA-T-0084 from `8Dionysus/aoa-techniques` at `b0f7e094bf81abaf0895a729d504a97f3af91ae8` using path `techniques/continuity/donor-harvest/progression-evidence-lift/TECHNIQUE.md` and sections: Intent, Inputs, Outputs, Contracts, Validation
- AOA-T-0085 from `8Dionysus/aoa-techniques` at `b0f7e094bf81abaf0895a729d504a97f3af91ae8` using path `techniques/continuity/donor-harvest/multi-axis-quest-overlay/TECHNIQUE.md` and sections: Outputs, Risks, Validation

## Adaptation points
Project overlays may add:
- local axis notes
- role-affinity hints
- local unlock classes
- local axis posture vocabulary, provided it still distinguishes confirmed
  reviewed evidence from provisional or stale hints
- local baseline vocabulary, provided it still distinguishes a reviewed prior
  baseline from first observation, missing baseline, stale baseline, or contested
  baseline
