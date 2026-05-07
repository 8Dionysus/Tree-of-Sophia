---
name: aoa-source-of-truth-check
description: Clarify which files are authoritative for repository guidance, operational instructions, architecture, and status, and keep entrypoint docs short and link-driven once canonical homes exist. Use when docs overlap, conflict, or confuse contributors about which file to trust. Do not use for purely code-local tasks or when authoritative files are already clear and the main need is decision rationale.
license: Apache-2.0
compatibility: Designed for Codex or similar coding agents with repository file access and an interactive shell. Network access is optional and only needed when repository validation or referenced workflows require it.
metadata:
  aoa_scope: core
  aoa_status: canonical
  aoa_invocation_mode: explicit-preferred
  aoa_source_skill_path: skills/core/engineering/aoa-source-of-truth-check/SKILL.md
  aoa_source_repo: 8Dionysus/aoa-skills
  aoa_technique_dependencies: AOA-T-0013,AOA-T-0002,AOA-T-0009
  aoa_portable_profile: codex-facing-wave-3
---

# aoa-source-of-truth-check

## Intent
Use this skill to clarify which repository surfaces are authoritative for status, architecture, source behavior, configuration, run instructions, policy, and change guidance.

## Trigger boundary
Use this skill when:
- a repository has several docs that may overlap or conflict
- contributors may not know which file, source surface, config, schema, manifest, report, or export to trust first
- a change touches docs, source, config, process, generated output, or operational guidance and the question is which surface is authoritative
- confusion exists between overview docs and authoritative docs
- one authoritative source must stay aligned across multiple downstream consumer surfaces
- top-level status docs such as `README` or `MANIFEST` are accumulating status/history that should live in canonical homes instead
- the repository already has canonical detail surfaces and the summary docs should stay short, navigable, and link-driven
- root docs, mechanics packages, source/config/schema surfaces, legacy receipts, provenance bridges, decisions, generated surfaces, runtime receipts, or owner-local handoff docs are being confused with one another
- a public repository must stay portable while still pointing to project-local, ecosystem, or downstream owner context

Do not use this skill when:
- the repository is tiny and has no meaningful source-of-truth ambiguity
- the task is purely code-local with no guidance, configuration, generated/export, or policy authority impact
- the authoritative files are already clear and the main need is recording rationale for a decision; use `aoa-adr-write`
- the main problem is deciding whether logic belongs in the core or at the edge; use `aoa-core-logic-boundary` first
- the main problem is broader policy design rather than document authority or ownership
- the task is only about building or maintaining a derived docs surface; that belongs in a separate review-surface workflow
- the generated or exported surface has a known source owner and only needs a deterministic rebuild or no-drift check

## Inputs
- repository guidance, source, configuration, schema, manifest, generated/export, operational, and status surfaces
- target area of ambiguity or overlap
- known canonical files or source-owned surfaces if any
- active, legacy, provenance, generated, or decision surfaces involved
- owner repositories or stronger owner layers involved
- current contributor confusion points

## Outputs
- clearer source-of-truth map
- active/current versus historical/provenance/generated placement map
- fan-out map when one source feeds multiple downstream consumers
- note of overlaps or conflicts
- proposed or implemented document role clarification
- lightweight snapshot guidance for entrypoint docs when canonical homes already exist
- verification summary

## Procedure
1. identify the main docs, source/config/schema surfaces, route cards, generated outputs, mechanics surfaces, runtime receipts, legacy/provenance bridges, and owner-local handoff files involved in the target area
2. when the authority question spans more than ordinary docs, choose the smallest useful shape from `references/authority-surface-shapes.md`
3. determine which file or surface should be authoritative for each concern and which surfaces are entrypoints, active contracts, provenance bridges, historical receipts, generated companions, operational receipts, or decision rationale
4. note any overlap, contradiction, role ambiguity, or stale route caused by old growth rather than current source truth
5. if one source feeds multiple consumers, name each consumer and refresh them from the same source
6. if top-level status docs are bloating, trim them into short snapshots and route detail to canonical homes
7. when historical or raw material matters, route it through legacy/provenance rather than letting it burden the active contract
8. clarify or propose clarifying ownership and purpose without inserting broad law blocks into unrelated sections
9. keep the change bounded to the authority surface under review
10. verify that the result reduces ambiguity for future changes and preserves standalone readability where the repo is public

## Contracts
- authoritative sources should be visible and named explicitly
- overview documents should not silently replace canonical ones
- lightweight entrypoint docs should link outward instead of duplicating chronology or changing counters
- active behavior should live in active surfaces; legacy and provenance should preserve lineage without becoming the first route
- generated, exported, compact, and derived surfaces should remain subordinate to source-authored files
- public portable surfaces should not require a full local ecosystem deployment to make sense, even when project-owner routes are linked
- role separation should reduce confusion, not create extra ceremony
- the resulting guidance should be understandable to another human or agent

## Risks and anti-patterns
- over-formalizing a tiny docs surface
- creating many labels without reducing ambiguity
- moving truth across files without clearly signaling the change
- letting summaries masquerade as canonical instructions
- trimming top-level docs too aggressively before canonical homes are actually available
- widening the skill into generic docs hygiene or derived surface maintenance
- treating provenance as trash archive, or treating raw legacy as the active contract
- inserting direct law/local-form blocks everywhere instead of making the existing route surface clearer
- hiding sibling-owner authority inside a local docs surface because the local repo is nearby
- letting a generated, compact, installed, or runtime convenience surface override the authored source it describes

## Verification
- confirm the main source-of-truth ambiguity was reduced
- confirm authoritative files are named explicitly
- confirm overlaps or conflicts were surfaced rather than hidden
- confirm summary docs stay short and route detail to canonical homes where those already exist
- confirm active, legacy/provenance, generated, and decision surfaces keep distinct roles when they are involved
- confirm generated, exported, compact, installed, and runtime receipt surfaces remain weaker than their source owners
- confirm public portable surfaces remain understandable without hidden local system knowledge
- confirm the result helps future contributors orient faster

## Technique traceability
Manifest-backed techniques:
- AOA-T-0013 from `8Dionysus/aoa-techniques` at `cd276f040d55d490bd015b8698c7a5d594b9f875` using path `techniques/instruction/instruction-surface/single-source-rule-distribution/TECHNIQUE.md` and sections: Intent, When to use, Inputs, Outputs, Core procedure, Contracts, Risks, Validation
- AOA-T-0002 from `8Dionysus/aoa-techniques` at `cd276f040d55d490bd015b8698c7a5d594b9f875` using path `techniques/instruction/docs-boundary/source-of-truth-layout/TECHNIQUE.md` and sections: Intent, When to use, Inputs, Outputs, Core procedure, Contracts, Risks, Validation
- AOA-T-0009 from `8Dionysus/aoa-techniques` at `cd276f040d55d490bd015b8698c7a5d594b9f875` using path `techniques/instruction/docs-boundary/lightweight-status-snapshot/TECHNIQUE.md` and sections: Intent, When to use, Inputs, Outputs, Core procedure, Contracts, Risks, Validation

## Adaptation points
Future project overlays may add:
- local doc hierarchies
- mixed ADR, architecture, and operations doc maps that split authority by concern
- preferred canonical-file patterns
- local review rules for doc changes
- repository-specific examples of authoritative surfaces
- lightweight snapshot rules for README or MANIFEST surfaces
- rules for keeping entrypoint docs short once deeper canonical homes already exist
- legacy/provenance placement rules and generated-surface rebuild checks
- local authority examples from `references/authority-surface-shapes.md`
