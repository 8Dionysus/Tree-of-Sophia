---
name: aoa-change-protocol
description: Plan and execute a bounded, non-trivial repository change with explicit verification and reporting. Use when code, config, docs, or operational surfaces change in a meaningful way and the task benefits from an explicit plan. Do not use for trivial wording fixes or when a more specific risk skill fits better.
license: Apache-2.0
compatibility: Designed for Codex or similar coding agents with repository file access and an interactive shell. Network access is optional and only needed when repository validation or referenced workflows require it.
metadata:
  aoa_scope: core
  aoa_status: canonical
  aoa_invocation_mode: explicit-preferred
  aoa_source_skill_path: skills/core/engineering/aoa-change-protocol/SKILL.md
  aoa_source_repo: 8Dionysus/aoa-skills
  aoa_technique_dependencies: AOA-T-0001,AOA-T-0002
  aoa_portable_profile: codex-facing-wave-3
---

# aoa-change-protocol

## Intent
Ground non-trivial changes in the owner route before editing, then keep the diff reviewable and explicitly verified without hidden scope expansion.

## Trigger boundary
Use this skill when:
- the change affects code, config, docs, or operational surfaces in a meaningful way
- the task benefits from an explicit plan and verification path
- the task touches more than a trivial wording fix
- the request references previous work, sibling repositories, root surfaces, mechanics, generated outputs, or owner boundaries that must be inspected before planning

Do not use this skill when:
- the edit is tiny and has no meaningful review or operational consequence
- a more specific risk skill should be used instead

## Inputs
- target goal
- owner repository or route surface
- touched files or surfaces
- source-of-truth or route-law surfaces that constrain the change
- main risk
- intended validation path
- rollback idea

## Outputs
- explicit plan
- scoped change
- named verification result
- post-change route review when source-of-truth, generated, decision, roadmap, changelog, quest, mechanic, or owner surfaces may have moved
- concise final report

## Procedure
1. choose the owner repository and read the nearest route card, `AGENTS.md`, source-of-truth surfaces, and local validation path before planning
2. state the goal, touched surfaces, and evidence already inspected; if the request depends on prior work, inspect the current repo state rather than relying on memory
3. identify the main risk, owner boundary, and rollback or recovery shape before editing
4. prepare the smallest reviewable change that follows the current source route
5. avoid unrelated cleanup, opportunistic refactors, or importing a sibling owner's authority into the local surface
6. apply the change inside the declared scope
7. run or name explicit verification, including generated/export rebuilds when source surfaces feed derived outputs
8. perform the narrow post-change route review only for surfaces whose meaning actually moved
9. report what changed, what was verified, what was skipped, what remains risky, and where the next owner route is

## Contracts
- the change must remain reviewable
- verification must be explicit, not implied
- the change should stay inside the declared scope
- rollback thinking should exist before apply
- the plan should be grounded in inspected source surfaces, not only conversation memory or generic pattern recall
- generated, exported, compact, or derived surfaces should be refreshed from source rather than hand-authored as truth
- cross-repo or mechanics changes should name owner boundaries and stop-lines before moving content

## Risks and anti-patterns
- over-formalizing trivial edits
- symbolic verification that creates false safety
- using the report as a substitute for a readable diff
- silently expanding the task during implementation
- planning from stale memory when the repository already has route cards, mechanics, or source-of-truth docs that answer the question
- treating legacy, provenance, generated, or sibling-owner surfaces as if they were the active local contract
- inserting broad law blocks into local surfaces instead of shaping the change through the existing route

## Verification
- confirm the owner route and relevant source surfaces were inspected before apply
- confirm the change stayed scoped
- confirm at least one explicit verification step was run or intentionally skipped with explanation
- confirm generated or export surfaces were rebuilt when canonical inputs changed
- confirm the post-change route review touched only surfaces whose meaning moved
- confirm the report includes outcome, rollback thinking, skipped checks, and remaining owner-route risk

## Technique traceability
Manifest-backed techniques:
- AOA-T-0001 from `8Dionysus/aoa-techniques` at `cd276f040d55d490bd015b8698c7a5d594b9f875` using path `techniques/execution/agent-workflows-core/plan-diff-apply-verify-report/TECHNIQUE.md` and sections: Intent, When to use, Inputs, Outputs, Core procedure, Contracts, Risks, Validation
- AOA-T-0002 from `8Dionysus/aoa-techniques` at `cd276f040d55d490bd015b8698c7a5d594b9f875` using path `techniques/instruction/docs-boundary/source-of-truth-layout/TECHNIQUE.md` and sections: Intent, When to use, Inputs, Outputs, Core procedure, Contracts, Risks, Validation

## Adaptation points
Project overlays should add:
- local source-of-truth files
- local validation commands
- local risk tiers
- approval or review rules
- local post-change route review surfaces such as changelog, roadmap, decision records, generated indexes, questbooks, or mechanics ledgers
