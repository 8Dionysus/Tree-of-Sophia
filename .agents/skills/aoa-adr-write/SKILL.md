---
name: aoa-adr-write
description: Capture a meaningful architectural or workflow decision as an ADR or decision note, place it in the canonical decision surface, and verify the rationale stays discoverable. Use when structure, boundaries, tooling, or workflow expectations change and future contributors need to know why. Do not use for tiny self-evident edits or when the real problem is source-of-truth ambiguity rather than decision rationale.
license: Apache-2.0
compatibility: Designed for Codex or similar coding agents with repository file access and an interactive shell. Network access is optional and only needed when repository validation or referenced workflows require it.
metadata:
  aoa_scope: core
  aoa_status: canonical
  aoa_invocation_mode: explicit-preferred
  aoa_source_skill_path: skills/core/engineering/aoa-adr-write/SKILL.md
  aoa_source_repo: 8Dionysus/aoa-skills
  aoa_technique_dependencies: AOA-T-0033,AOA-T-0002
  aoa_portable_profile: codex-facing-wave-3
---

# aoa-adr-write

## Intent
Use this skill to capture an architectural, structural, or workflow decision in a concise reviewable note, then place and verify that note in the canonical repo surface so the rationale stays findable.

## Trigger boundary
Use this skill when:
- a decision changes structure, boundaries, tooling, or workflow expectations
- future contributors will need to know why a path was chosen
- several plausible options existed and the reasoning matters
- the team or project risks repeating the same debate later
- the note needs a clear canonical home, not just a one-off comment
- the decision crosses ownership, layer, evidence, lifecycle, portability, runtime-facing, handoff, risk, or scale boundaries and needs a durable rationale
- reviewed evidence, planning work, generated output, audit results, or operational receipts have revealed a decision that must stay separate from the evidence that revealed it

Do not use this skill when:
- the change is tiny and self-evident
- the note would only restate an obvious diff with no real decision content
- the main problem is unclear authoritative documentation rather than decision rationale; use `aoa-source-of-truth-check` first
- the main problem is deciding whether logic belongs in the core or at the edge; use `aoa-core-logic-boundary` first
- no decision exists yet and the next step is still discovery, option framing, or owner-route clarification
- the boundary itself is still ambiguous; use `aoa-bounded-context-map` before recording the decision
- the request is to record every possible lens, provisional hint, planning idea, or generated observation without one reviewed decision to preserve
- the decision is an ordinary implementation choice whose rationale is already clear from the diff, tests, commit message, or review summary
- the next honest move is a runbook, incident note, risk approval, or operational follow-up rather than durable decision rationale

## Inputs
- decision to record
- context and problem statement
- relevant options or alternatives
- chosen path and rationale
- known consequences or tradeoffs
- owner, source, evidence, placement, or lifecycle boundaries that shape the decision

## Outputs
- concise decision note or ADR draft
- statement of rationale
- consequence notes
- canonical placement or reference for the note
- verification that the note landed in the expected decision surface
- verification that the note matches the actual change
- decision-boundary notes that say what the ADR records and what stays with stronger owners, evidence surfaces, generated or derived outputs, or follow-up routes

## Procedure
1. confirm a real decision exists and is meaningful enough to record; if not, route to discovery rather than writing an ADR
2. when placement, owner, evidence, or lifecycle could blur the record, choose the smallest useful decision-lens set from `references/decision-boundary-lenses.md`
3. state the context and the problem the decision addresses
4. list the main options if they meaningfully shaped the choice
5. record the chosen decision in clear language
6. note why it was chosen and what tradeoffs it introduces
7. place the note in the canonical decision surface or repo-local home that future reviewers should use
8. connect the note to the actual change surface when relevant
9. verify that the note explains the decision rather than narrating the diff only
10. verify that the note is reachable from the intended canonical location

## Contracts
- the note should explain why, not just what changed
- the decision should be bounded and understandable
- tradeoffs should not be hidden behind certainty theater
- the note should help future reviewers, not merely satisfy process
- the note should have an explicit canonical placement, not a hidden or implied home
- the note should distinguish the decision from the evidence, planning surface, generated output, audit result, workflow event, or runtime-facing event that motivated it
- decision lenses should be selected only when they clarify the decision's authority, placement, or future effect
- the skill should prefer no ADR when a lighter artifact preserves enough rationale for future work
- verification should check both rationale quality and note placement

## Risks and anti-patterns
- writing an ADR for a trivial edit with no real decision
- writing a decision record before a decision actually exists
- using inflated language to mask weak reasoning
- recording the chosen path without naming consequences
- letting the ADR drift away from the actual change
- treating placement as optional after the note is written
- using the skill when the main work is still source-of-truth clarification
- treating a provisional note, generated observation, planning wish, or workflow candidate as a reviewed decision without owner evidence
- copying stronger owner law into a local ADR instead of naming the decision's handoff and authority limit
- creating ADR clutter for ordinary implementation choices that need verification or review summary instead
- applying decision lenses exhaustively until the ADR becomes a governance essay

## Verification
- confirm the decision was meaningful enough to record
- confirm the rationale is explicit
- confirm consequences or tradeoffs are named
- confirm at least one rejected option or accepted tradeoff is visible when alternatives shaped the choice
- confirm the note aligns with the real change surface
- confirm the note is placed where future reviewers will look for it
- confirm the canonical location or reference is part of the result, not an afterthought
- confirm a lighter artifact would not preserve enough rationale before creating a durable ADR
- confirm evidence, generated, planning, workflow, or runtime-facing surfaces are cited as context rather than promoted into decision authority
- confirm any selected decision lenses are necessary and narrow enough for this one decision

## Technique traceability
Manifest-backed techniques:
- AOA-T-0033 from `8Dionysus/aoa-techniques` at `cd276f040d55d490bd015b8698c7a5d594b9f875` using path `techniques/instruction/docs-boundary/decision-rationale-recording/TECHNIQUE.md` and sections: Intent, When to use, When not to use, Inputs, Outputs, Core procedure, Contracts, Risks, Validation
- AOA-T-0002 from `8Dionysus/aoa-techniques` at `cd276f040d55d490bd015b8698c7a5d594b9f875` using path `techniques/instruction/docs-boundary/source-of-truth-layout/TECHNIQUE.md` and sections: Intent, When to use, When not to use, Inputs, Outputs, Core procedure, Contracts, Risks, Validation

## Adaptation points
Future project overlays may add:
- local ADR templates
- alternative decision-note homes outside formal `docs/adr/` layouts
- a short decision-lens pass from `references/decision-boundary-lenses.md` when durable rationale crosses owner, layer, evidence, workflow, runtime-facing, or scale boundaries
- a compact note skeleton from `references/decision-note.template.md` when no repo-local template exists
- local placement rules
- local template variants that still preserve context, options, decision, and consequences
- architecture review expectations
- cross-linking rules back to canonical authority docs without replacing them
- repository-specific examples
