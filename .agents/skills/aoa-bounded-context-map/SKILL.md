---
name: aoa-bounded-context-map
description: Clarify system or domain boundaries, name contexts, and surface interfaces so changes stay semantically scoped. Use when naming is overloaded, responsibilities are mixed, or a task needs a cleaner boundary before coding. Do not use for tiny local edits or when the boundary is already clear and the real task is contract validation.
license: Apache-2.0
compatibility: Designed for Codex or similar coding agents with repository file access and an interactive shell. Network access is optional and only needed when repository validation or referenced workflows require it.
metadata:
  aoa_scope: core
  aoa_status: canonical
  aoa_invocation_mode: explicit-preferred
  aoa_source_skill_path: skills/core/engineering/aoa-bounded-context-map/SKILL.md
  aoa_source_repo: 8Dionysus/aoa-skills
  aoa_technique_dependencies: AOA-T-0016,AOA-T-0002
  aoa_portable_profile: codex-facing-wave-3
---

# aoa-bounded-context-map

## Intent
Use bounded-context thinking to reduce semantic drift, mixed responsibilities, and unclear interfaces.

## Trigger boundary
Use this skill when:
- a project mixes several domains or subsystems
- naming is drifting or overloaded
- the task needs a clearer boundary before coding safely
- an agent is likely to confuse nearby concepts without sharper separation
- a repository has a dual posture, such as standalone public package and ecosystem component
- practice patterns, execution workflows, evaluation artifacts, scenarios, generated/export surfaces, or owner-local implementations are easy to blur together
- the same work changes shape across task, session, repository, layer, surface, or process context
- several boundary types may matter at once, such as owner, layer, lifecycle, authority, surface-state, portability, proof, runtime, or handoff

Do not use this skill when:
- the change is tiny and fully local
- the boundary is already clear and agreed on, and the real task is validating the interface contract; use `aoa-contract-test`
- the main problem is deciding whether logic belongs in the core or at the edge; use `aoa-core-logic-boundary` first
- the request is a broad architecture redesign, taxonomy program, or durable governance map rather than one bounded ambiguity-reduction pass
- the request asks to apply every boundary lens exhaustively without one concrete ambiguity or next change to constrain

## Inputs
- target area or subsystem
- current naming and responsibilities
- known neighboring contexts
- ambiguous or overloaded terms
- owner layers, stronger source surfaces, or portability constraints that shape the boundary
- active boundary lenses when the ambiguity spans several kinds of difference

## Outputs
- named contexts or subsystems
- rough boundary map
- interface notes between contexts
- ambiguity notes and recommended vocabulary
- owner split, stop-line, and portable-versus-integration vocabulary when those boundaries matter
- selected lens notes that explain which kinds of boundary matter for this task and which ones are intentionally out of scope

## Procedure
1. confirm the task has one concrete ambiguity or scoping problem; if it asks for a broad architecture program, narrow or route away first
2. when several kinds of boundary are mixed, choose the smallest useful lens set from `references/boundary-lenses.md`
3. identify the target area and the terms people use for it
4. separate responsibilities into bounded contexts, subsystems, layers, owner surfaces, or repositories
5. name what belongs inside each context and what stays outside
6. distinguish portable core meaning from ecosystem integration, local implementation, generated projection, or historical provenance when those surfaces coexist
7. describe the interfaces, handoffs, stop-lines, or translations between contexts
8. note ambiguous terms and propose clearer language
9. report how the boundary should constrain the next change, including what should route away

## Contracts
- boundaries should reduce semantic confusion, not create ceremony for its own sake
- neighboring contexts should be named explicitly when relevant
- interface or translation points should be visible
- boundary lenses should be selected because they reduce the current ambiguity, not because a checklist exists
- a context map should not transfer authority from a stronger owner into the local repository
- portable core wording should remain usable without hidden ecosystem dependencies when the target surface is public

## Risks and anti-patterns
- inventing too many contexts for a small problem
- renaming concepts without reducing confusion
- treating context maps as proof of good architecture when interfaces remain muddy
- copying center or sibling-repo law into a local surface instead of naming a light handoff
- turning a dual-posture repo into two unrelated identities rather than one bounded interface
- using context labels as decoration while the next diff still crosses owner boundaries
- letting one context map become an open-ended architecture taxonomy or governance program
- treating `references/boundary-lenses.md` as an exhaustive checklist rather than a way to choose the right few distinctions
- freezing one session, repository, layer, or surface's current shape as universal project law

## Verification
- confirm the main ambiguity was reduced
- confirm interfaces or handoff points are named
- confirm the output helps future scoped changes
- confirm the map says what routes away when a stronger owner owns the truth
- confirm portable and integration-only wording remain distinct when both are present
- confirm a future change can tell what stays out of scope, not only what was renamed
- confirm the selected lenses are necessary and enough for the next change, with unused lenses left out rather than padded in

## Technique traceability
Manifest-backed techniques:
- AOA-T-0016 from `8Dionysus/aoa-techniques` at `fbead87e01b82df6c56e3d92a074cd7515131847` using path `techniques/proof/skill-support/bounded-context-map/TECHNIQUE.md` and sections: Intent, When to use, Inputs, Outputs, Core procedure, Contracts, Risks, Validation
- AOA-T-0002 from `8Dionysus/aoa-techniques` at `fbead87e01b82df6c56e3d92a074cd7515131847` using path `techniques/instruction/docs-boundary/source-of-truth-layout/TECHNIQUE.md` and sections: Intent, When to use, Inputs, Outputs, Core procedure, Contracts, Risks, Validation

## Adaptation points
Project overlays should add:
- local domain vocabulary
- canonical docs that define terminology
- local examples of context boundaries
- a short lens pass from `references/boundary-lenses.md` when the same task spans session, repository, layer, lifecycle, authority, or runtime-facing differences
- a compact map skeleton from `references/context-map.template.md` when no repo-local format exists
- local owner-route maps, portability rules, and generated-surface handoffs
