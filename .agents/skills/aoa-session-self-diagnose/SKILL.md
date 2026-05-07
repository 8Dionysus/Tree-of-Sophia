---
name: aoa-session-self-diagnose
description: Classify drift, friction, proof gaps, ownership confusion, and repeated failure patterns from a reviewed session into a bounded diagnosis packet without mutating anything yet. Use when the next honest move is diagnosis before repair and the reviewed material points to repeated contradictions, blockers, or boundary blur. Do not use for live sessions, for issues that are already fully diagnosed, or when the real task is final quest-promotion triage.
license: Apache-2.0
compatibility: Designed for Codex or similar coding agents with repository file access and an interactive shell. Network access is optional and only needed when repository validation or referenced workflows require it.
metadata:
  aoa_scope: core
  aoa_status: evaluated
  aoa_invocation_mode: explicit-only
  aoa_source_skill_path: skills/core/session-growth/aoa-session-self-diagnose/SKILL.md
  aoa_source_repo: 8Dionysus/aoa-skills
  aoa_technique_dependencies: AOA-T-0080,AOA-T-0081
  aoa_portable_profile: codex-facing-wave-3
---

# aoa-session-self-diagnose

## Intent
Use this skill to turn a reviewed session into a `DIAGNOSIS_PACKET`.

It should answer: what drifted, what hurt, what boundary blurred, what proof is
missing, and what repair shape is most plausible.

## Trigger boundary
Use this skill when:
- a reviewed session contains repeated friction, contradiction, or drift
- the next honest move is diagnosis before repair
- boundary confusion or missing proof may be more important than immediate output production
- blocked automation readiness may need root-cause classification before any automation claim becomes honest
- the same class of problem may be appearing across sessions

Do not use this skill when:
- the session is still live
- the issue is already fully diagnosed and only needs repair execution
- the material is a celebration recap with no meaningful friction
- the route is actually a single quest-promotion decision

## Inputs
- reviewed session artifact or harvest packet
- observed frictions, failures, or contradictions
- relevant owner layers and touched repos
- previous related session evidence if available

## Outputs
- `DIAGNOSIS_PACKET` with drift types, symptoms, probable causes, repair shapes, and owner hints
- severity or urgency notes when evidence supports them
- explicit unknowns when diagnosis remains incomplete
- optional blocked-automation findings such as unstable inputs, hidden approval,
  rollback gaps, or secret coupling
- optional handoff to `aoa-session-self-repair`
- one `DIAGNOSIS_PACKET_RECEIPT` using
  `references/stats-event-envelope.md` and
  `references/diagnosis-packet-receipt-schema.yaml`
- one `CORE_SKILL_APPLICATION_RECEIPT` using
  `references/core-skill-application-receipt-schema.yaml`

## Procedure
1. gather reviewed symptoms and evidence refs
2. separate symptom from probable cause
3. classify drift types such as boundary drift, proof debt, role leakage, memory contamination, route collapse, compaction damage, or repeated blocker patterns
4. call out blocked automation causes when the route looks repetitive but still fails readiness because of unstable inputs, hidden authority, weak rollback posture, or secret coupling
5. map each diagnosis toward the likely owner layer
6. suggest a repair shape without silently performing it
7. preserve unknowns where evidence does not justify stronger claims
8. emit one bounded `DIAGNOSIS_PACKET_RECEIPT` when the diagnosis packet
   closes, keeping the payload smaller than the diagnosis itself
9. when the finish path closes, emit one `CORE_SKILL_APPLICATION_RECEIPT`
   that points back to the bounded detail receipt and records one finished
   kernel-skill application

## Contracts
- diagnosis is read-only
- one odd anecdote is not enough for structural certainty
- probable cause must remain probabilistic when evidence is thin
- owner hints must not override owner-layer law
- no hidden mutation or silent patching
- diagnosis does not grant automation readiness by itself
- the receipt remains descriptive and cannot become proof or repair authority
- the generic core receipt remains subordinate to the diagnosis receipt and
  cannot become diagnosis, proof, or repair authority
- receipt corrections use `supersedes` rather than silent overwrite

## Risks and anti-patterns
- mistaking symptom for cause
- using vague vibes as diagnosis
- smuggling repair work into diagnosis
- turning every inconvenience into a system flaw
- blaming one owner layer for a cross-layer issue
- treating automation frustration as if it automatically proved readiness
- letting a diagnosis receipt read like a final blame or repair verdict

## Verification
- confirm each diagnosis cites evidence refs
- confirm symptoms and causes are separated
- confirm a likely owner layer is named
- confirm unknowns are preserved where needed
- confirm no mutation happened
- confirm any emitted receipt stays evidence-linked and smaller than the diagnosis packet
- confirm any emitted `CORE_SKILL_APPLICATION_RECEIPT` points to the detail
  diagnosis receipt and stays finish-only

## Technique traceability
Manifest-backed techniques:
- AOA-T-0080 from `8Dionysus/aoa-techniques` at `cd276f040d55d490bd015b8698c7a5d594b9f875` using path `techniques/recovery/diagnosis-repair/session-drift-taxonomy/TECHNIQUE.md` and sections: Intent, Outputs, Risks, Validation
- AOA-T-0081 from `8Dionysus/aoa-techniques` at `cd276f040d55d490bd015b8698c7a5d594b9f875` using path `techniques/recovery/diagnosis-repair/diagnosis-from-reviewed-evidence/TECHNIQUE.md` and sections: Intent, Inputs, Outputs, Core procedure, Contracts, Validation

## Adaptation points
Project overlays may add:
- local drift taxonomies
- repo-specific failure classes
- severity bands
