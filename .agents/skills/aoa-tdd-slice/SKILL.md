---
name: aoa-tdd-slice
description: Implement a bounded feature slice through test-first discipline, minimal implementation, and explicit refactor boundaries. Use when a behavior change can be specified in tests before code and confidence or regression resistance matters. Do not use for undefined exploratory work, one-off glue, or broader architectural restructuring.
license: Apache-2.0
compatibility: Designed for Codex or similar coding agents with repository file access and an interactive shell. Network access is optional and only needed when repository validation or referenced workflows require it.
metadata:
  aoa_scope: core
  aoa_status: canonical
  aoa_invocation_mode: explicit-preferred
  aoa_source_skill_path: skills/core/engineering/aoa-tdd-slice/SKILL.md
  aoa_source_repo: 8Dionysus/aoa-skills
  aoa_technique_dependencies: AOA-T-0014,AOA-T-0001
  aoa_portable_profile: codex-facing-wave-3
---

# aoa-tdd-slice

## Intent
Use test-first discipline to implement a small behavior slice with a clear observable target, minimal implementation, and explicit verification.

## Trigger boundary
Use this skill when:
- a behavior change can be expressed as tests before implementation
- a module, CLI, builder, parser, validator, schema check, generated/export path, adapter, router, or workflow step has observable behavior that can be specified before implementation
- the task fits a bounded slice rather than a broad rewrite
- confidence and regression resistance matter enough to justify a red-green-refactor loop

Do not use this skill when:
- the task is purely exploratory and behavior is still undefined
- the task is mostly one-off glue with no reusable logic
- the main problem is a stable interface or boundary contract rather than a feature slice; use `aoa-contract-test`
- the main problem is invariant coverage rather than slice implementation; use `aoa-property-invariants`
- the main problem is source-of-truth, ownership, or guidance authority rather than observable behavior; use `aoa-source-of-truth-check`
- the behavior cannot be observed without broad architecture work, long-running discovery, or a new proof strategy
- broader architectural restructuring is the real need

## Inputs
- desired behavior
- target module, command, builder, validator, adapter, generated/export path, or workflow step
- constraints and non-goals
- available test surface
- fixture, golden, schema, fake adapter, CLI, or focused smoke surface when relevant

## Outputs
- new or updated tests
- minimal implementation
- small refactor if needed
- verification summary
- explicit slice boundary and untouched behavior

## Procedure
1. define the desired behavior in a bounded way
2. when the behavior is wider than a simple module change, choose the smallest useful shape from `references/tdd-slice-shapes.md`
3. add or update tests before implementation
4. make the smallest change that satisfies the tests
5. refactor only inside the declared slice after the behavior is green
6. run the tests and record the result
7. report what behavior is now specified, what surface was intentionally untouched, and what remains out of scope

## Contracts
- behavior should be made explicit before implementation when reasonably possible
- the implementation should stay bounded to the slice
- unrelated refactors should be avoided
- tests should express behavior, not incidental implementation detail
- generated/export behavior should be tested through the source-owned builder or fixture path, not by hand-editing derived output
- workflow or CLI behavior should test the stable machine-readable contract, exit state, or observable route rather than incidental log prose
- refactor work should start only after the focused behavior check is green

## Risks and anti-patterns
- writing tests that merely mirror the implementation
- turning a small slice into a hidden architectural rewrite
- using TDD ritualistically where the problem is not well-shaped yet
- overfitting tests to brittle internal details
- treating snapshot churn, generated file bytes, or current sort order as the behavior unless consumers rely on it
- adding a broad fixture matrix before the slice has one clear target behavior

## Verification
- confirm tests were added or updated first when the task was suitable for TDD
- confirm the implementation stayed bounded
- confirm the relevant test suite passed
- confirm the report names the covered behavior and the untouched behavior
- confirm any generated/export or workflow assertion still points back to the source-owned behavior under test

## Technique traceability
Manifest-backed techniques:
- AOA-T-0014 from `8Dionysus/aoa-techniques` at `cd276f040d55d490bd015b8698c7a5d594b9f875` using path `techniques/execution/agent-workflows-core/tdd-slice/TECHNIQUE.md` and sections: Intent, When to use, Inputs, Outputs, Core procedure, Contracts, Risks, Validation
- AOA-T-0001 from `8Dionysus/aoa-techniques` at `cd276f040d55d490bd015b8698c7a5d594b9f875` using path `techniques/execution/agent-workflows-core/plan-diff-apply-verify-report/TECHNIQUE.md` and sections: Contracts, Validation

## Adaptation points
Project overlays should add:
- local test commands
- local module boundaries
- guidance for when TDD is mandatory versus optional
- local CLI, builder, validator, adapter, generated/export, or workflow examples from `references/tdd-slice-shapes.md`
- local rules for when fixtures, snapshots, or golden files are stable enough to test
