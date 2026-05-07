---
name: aoa-core-logic-boundary
description: Separate reusable core logic from glue, orchestration, and infrastructure detail. Use when stable rules are mixed with wiring, the same logic repeats in several places, or reviews are muddy because the responsibility center is unclear. Do not use for tiny isolated fixes or when the real task is introducing a port or adapter around a concrete dependency.
license: Apache-2.0
compatibility: Designed for Codex or similar coding agents with repository file access and an interactive shell. Network access is optional and only needed when repository validation or referenced workflows require it.
metadata:
  aoa_scope: core
  aoa_status: evaluated
  aoa_invocation_mode: explicit-preferred
  aoa_source_skill_path: skills/core/engineering/aoa-core-logic-boundary/SKILL.md
  aoa_source_repo: 8Dionysus/aoa-skills
  aoa_technique_dependencies: AOA-T-0016,AOA-T-0015
  aoa_portable_profile: codex-facing-wave-3
---

# aoa-core-logic-boundary

## Intent
Use this skill to separate a stable reusable center from surrounding glue, orchestration, projection, infrastructure, or presentation detail so future changes land in the right place.

## Trigger boundary
Use this skill when:
- a module mixes stable rules with wiring or execution detail
- an execution workflow, practice pattern, evaluation artifact, scenario, memory or recall object, role contract, router, SDK, metric, generated/export, or process surface mixes reusable meaning with projection, report, runtime, or local delivery detail
- the same logic is starting to appear in several places
- tests or reviews are muddy because the center of responsibility is unclear
- you need to decide whether something belongs in the core or at the edges
- generated, exported, adapter, report, or presentation surfaces are starting to look like they own the reusable rule they only carry

Do not use this skill when:
- the task is a tiny isolated fix with no meaningful boundary ambiguity
- the code is already clearly partitioned
- the result would only rename folders without improving understanding
- the owner, context, or source-of-truth boundary is still unclear; use `aoa-bounded-context-map` first
- the main problem is recording decision rationale rather than boundary placement; use `aoa-adr-write` first
- the main problem is validating a consumer-visible contract after the boundary is clear; use `aoa-contract-test`
- the boundary is already clear and the main task is introducing a port or adapter around a concrete dependency; use `aoa-port-adapter-refactor`

## Inputs
- target module, service, repository surface, layer surface, or slice
- current responsibilities
- repeated or reusable logic candidates
- surrounding glue, projection, rendering, runtime, infrastructure, or orchestration context
- source owner and edge consumers when the split crosses generated, exported, adapter, or downstream surfaces

## Outputs
- clarified boundary between core logic and surrounding glue
- notes on what should stay reusable versus edge-specific
- small refactor proposal or bounded implementation
- source-owner and derived-surface stop-lines when relevant
- verification summary

## Procedure
1. identify which rules or behaviors are stable enough to count as reusable core logic
2. when the target is not a simple code module, choose the smallest useful shape from `references/core-boundary-shapes.md`
3. identify which parts are mostly wiring, orchestration, I/O, projection, report rendering, local path handling, generated output, adapter behavior, or environment detail
4. separate the concerns conceptually before moving code or rewriting docs
5. move or propose moving only the bounded subset that clearly belongs in the reusable center
6. avoid renaming or restructuring for ceremony alone
7. verify that the new boundary improves changeability and review clarity

## Contracts
- reusable logic should not stay trapped in glue if that blocks testing or reuse
- glue should not be over-promoted into domain logic without reason
- the boundary should improve clarity, not just aesthetics
- the change should remain reviewable and bounded
- derived, generated, exported, adapter, and presentation surfaces should not become source authority just because they are convenient
- reusable center can mean stable rule, mapping, scoring logic, role contract, recall rule, receipt envelope, route choice, or scenario phase, but it must stay named and bounded

## Risks and anti-patterns
- treating all logic as core logic and over-abstracting the system
- moving orchestration detail into the core under the label of purity
- renaming layers without reducing actual confusion
- hiding a broad rewrite inside a boundary-cleanup task
- treating generated output, report prose, runtime projection, or local adapter code as the reusable center
- freezing one repository, layer, or surface's current implementation detail as universal project architecture
- using this skill when the actual problem is owner-route ambiguity, contract validation, or a concrete dependency seam

## Verification
- confirm the core candidate is genuinely reusable or stability-shaped
- confirm edge-specific code remains at the edge when appropriate
- confirm the refactor improved clarity for future changes
- confirm no unrelated structural churn was introduced
- confirm source-owned meaning stays stronger than generated, exported, adapter, or presentation surfaces
- confirm the split says what future changes should update together and what should remain independent

## Technique traceability
Manifest-backed techniques:
- AOA-T-0016 from `8Dionysus/aoa-techniques` at `fbead87e01b82df6c56e3d92a074cd7515131847` using path `techniques/proof/skill-support/bounded-context-map/TECHNIQUE.md` and sections: Intent, When to use, Outputs, Core procedure, Contracts, Validation
- AOA-T-0015 from `8Dionysus/aoa-techniques` at `fbead87e01b82df6c56e3d92a074cd7515131847` using path `techniques/proof/skill-support/contract-test-design/TECHNIQUE.md` and sections: Intent, When to use, Inputs, Outputs, Core procedure, Contracts, Risks, Validation

## Adaptation points
Future project overlays may add:
- local layering conventions
- preferred directories or module boundaries
- repository-specific examples of core versus glue
- local project-specific examples from `references/core-boundary-shapes.md`
- local verification commands
