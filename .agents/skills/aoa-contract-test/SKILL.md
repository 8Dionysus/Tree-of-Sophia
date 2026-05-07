---
name: aoa-contract-test
description: Design or extend contract-oriented validation at module, service, or workflow boundaries. Use when two components interact across a meaningful interface, downstream assumptions matter, or a smoke path needs an explicit contract. Do not use for purely local changes or when the real need is invariant or property testing instead of boundary validation.
license: Apache-2.0
compatibility: Designed for Codex or similar coding agents with repository file access and an interactive shell. Network access is optional and only needed when repository validation or referenced workflows require it.
metadata:
  aoa_scope: core
  aoa_status: canonical
  aoa_invocation_mode: explicit-preferred
  aoa_source_skill_path: skills/core/engineering/aoa-contract-test/SKILL.md
  aoa_source_repo: 8Dionysus/aoa-skills
  aoa_technique_dependencies: AOA-T-0003,AOA-T-0015
  aoa_portable_profile: codex-facing-wave-3
---

# aoa-contract-test

## Intent
Strengthen boundary reliability by making stable producer-consumer expectations, validation surfaces, and claim limits explicit.

## Trigger boundary
Use this skill when:
- two modules or services interact across a meaningful boundary
- a smoke path or interface needs a stable validation contract
- a module, service, CLI, schema, manifest, report, receipt, generated/export surface, workflow handoff, or repo-to-repo seam has consumers that rely on stable shape or behavior
- a source-owned surface feeds generated, exported, adapter, or downstream consumer surfaces
- a reusable practice, execution workflow, evaluation artifact, scenario, memory or recall object, role contract, router, SDK, metric, or generated/export surface exposes a stable object shape that other surfaces consume
- a change risks breaking downstream assumptions

Do not use this skill when:
- the change is entirely local and does not affect a meaningful boundary
- the boundary itself is still semantically unclear and naming is drifting; use `aoa-bounded-context-map`
- the main problem is expressing a broad invariant rather than a boundary contract; use `aoa-property-invariants`
- the main problem is auditing whether existing checks really cover a stable rule; use `aoa-invariant-coverage-audit`
- the output is incidental logs, debug prose, or a one-off snapshot with no named consumer
- the request would freeze internal implementation details or current incidental output as a public contract
- a broad system rewrite is needed before the boundary itself is stable

## Inputs
- boundary under review
- producer and named consumer
- expected inputs and outputs
- current verification surface
- known downstream dependencies
- contract limits and out-of-contract behavior

## Outputs
- explicit contract assumptions
- tests, fixture checks, schema checks, smoke summaries, or structured validation notes
- verification notes
- downstream impact notes
- contract limits that say what the check does not prove

## Procedure
1. identify the boundary and its consumers
2. when the boundary is not a simple module or service interface, choose the smallest useful shape from `references/contract-shapes.md`
3. state the expected input, output, behavior, report, receipt, schema, or handoff shape
4. state what remains out of contract so the check does not become a whole-system proof
5. express the contract in tests, fixture checks, schema checks, smoke summaries, or structured checks
6. verify both the boundary behavior and the reporting shape
7. report what became explicit, which consumers were protected, and what remains weak

## Contracts
- the contract should be visible to another human or agent
- verification should be tied to the boundary, not only to internals
- downstream assumptions should be named when relevant
- out-of-contract behavior should stay explicit when it affects consumers
- a contract test should protect the named seam, not claim the whole system is correct
- generated, exported, adapter, and derived surfaces should remain subordinate to their source owners
- consumer convenience should not become new source authority

## Risks and anti-patterns
- vague contracts that do not actually constrain behavior
- treating a smoke summary as proof when it does not cover the real boundary
- changing interface behavior without downstream impact notes
- asserting internal implementation details as if they were public contract
- freezing incidental output, debug logs, or transient runtime text as stable contract
- turning a local boundary check into federation-wide law
- widening the skill into generic test strategy instead of one named producer-consumer contract

## Verification
- confirm the contract is visible and reviewable
- confirm validation is tied to the interface or boundary
- confirm downstream impact was considered when relevant
- confirm the report names any known contract limits or exclusions
- confirm the named consumer and producer are both clear
- confirm generated or exported surfaces are checked as derived consumers, not promoted into source truth

## Technique traceability
Manifest-backed techniques:
- AOA-T-0003 from `8Dionysus/aoa-techniques` at `cd276f040d55d490bd015b8698c7a5d594b9f875` using path `techniques/proof/evaluation-chain/contract-first-smoke-summary/TECHNIQUE.md` and sections: Intent, When to use, Outputs, Contracts, Validation
- AOA-T-0015 from `8Dionysus/aoa-techniques` at `cd276f040d55d490bd015b8698c7a5d594b9f875` using path `techniques/proof/skill-support/contract-test-design/TECHNIQUE.md` and sections: Intent, Inputs, Core procedure, Risks

## Adaptation points
Project overlays should add:
- local endpoints or module boundaries
- local smoke or test commands
- boundary-specific invariants
- local schema, receipt, registry, report, export, or handoff contract examples
- local source-owner and downstream-consumer names
