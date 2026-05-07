---
name: aoa-safe-infra-change
description: Make bounded infrastructure, configuration, service, or operational changes with explicit risk framing, proportional verification, and rollback thinking. Use when a change has runtime or deployment implications and needs stronger discipline than a normal code edit. Do not use for purely local code changes, or when the main need is approval classification or preview-first execution.
license: Apache-2.0
compatibility: Designed for Codex or similar coding agents with repository file access and an interactive shell. Network access is optional and only needed when repository validation or referenced workflows require it.
metadata:
  aoa_scope: risk
  aoa_status: canonical
  aoa_invocation_mode: explicit-only
  aoa_source_skill_path: skills/risk/aoa-safe-infra-change/SKILL.md
  aoa_source_repo: 8Dionysus/aoa-skills
  aoa_technique_dependencies: AOA-T-0028,AOA-T-0001
  aoa_portable_profile: codex-facing-wave-3
---

# aoa-safe-infra-change

## Intent
Use this skill to shape infrastructure, service, configuration, or operational changes into a safer bounded workflow.

## Trigger boundary
Use this skill when:
- the task changes infrastructure, services, configuration, orchestration, or operational surfaces
- the change has runtime, safety, or deployment implications
- the task needs stronger verification and rollback thinking than a normal code edit

Do not use this skill when:
- the task is a purely local code change with no operational implications
- a more specific risk skill should be used instead
- the operator has not provided enough authority for the requested action
- the main question is whether authority exists at all; use `aoa-approval-gate-check`
- the main need is to prefer or interpret a preview path before execution; use `aoa-dry-run-first`

## Inputs
- target change
- touched operational surfaces
- stated authority or approval state
- validation path
- rollback idea

## Outputs
- explicit risk-aware plan
- bounded infrastructure or config change, or bounded execution recommendation
- verification result
- report with remaining risk notes

## Procedure
1. identify the operational surface and main risk
2. confirm whether the change belongs to a high-risk or explicit-only category
3. keep the change small and reviewable
4. avoid unrelated cleanup or hidden expansion of scope
5. verify the result using the strongest practical bounded checks available
6. report what changed, what was verified, and what remains risky or deferred

## Contracts
- infrastructure changes should stay explicit and reviewable
- risk should be named before apply, not after failure
- verification should be stronger than symbolic confidence
- rollback thinking should exist before execution

## Risks and anti-patterns
- treating infra edits like ordinary code edits
- making broad config or orchestration changes under a narrow task label
- relying on weak verification for changes with real runtime impact
- skipping explicit risk framing because the diff looks small

## Verification
- confirm the operational surface was named clearly
- confirm the change stayed bounded
- confirm verification was explicit and proportional to the risk
- confirm rollback or recovery thinking was present before execution or recommendation
- confirm the report includes unresolved risk or recovery notes

## Technique traceability
Manifest-backed techniques:
- AOA-T-0028 from `8Dionysus/aoa-techniques` at `cd276f040d55d490bd015b8698c7a5d594b9f875` using path `techniques/execution/agent-workflows-core/confirmation-gated-mutating-action/TECHNIQUE.md` and sections: Intent, When to use, Inputs, Outputs, Core procedure, Contracts, Risks, Validation
- AOA-T-0001 from `8Dionysus/aoa-techniques` at `cd276f040d55d490bd015b8698c7a5d594b9f875` using path `techniques/execution/agent-workflows-core/plan-diff-apply-verify-report/TECHNIQUE.md` and sections: Intent, Outputs, Contracts, Risks, Validation

## Adaptation points
Future project overlays may add:
- local risk classifications
- approval rules
- preferred validation commands
- rollback or recovery expectations
