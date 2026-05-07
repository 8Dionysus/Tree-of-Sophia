---
name: aoa-dry-run-first
description: Prefer simulation, preview, or inspect-only paths before any real mutation, then require one explicit confirmation for the concrete mutating step. Use when the action can be previewed and mistakes would be costly, such as delete, restore, migrate, or reconfigure operations. Do not use for purely analytical tasks or when the primary question is whether execution is authorized at all.
license: Apache-2.0
compatibility: Designed for Codex or similar coding agents with repository file access and an interactive shell. Network access is optional and only needed when repository validation or referenced workflows require it.
metadata:
  aoa_scope: risk
  aoa_status: canonical
  aoa_invocation_mode: explicit-only
  aoa_source_skill_path: skills/risk/aoa-dry-run-first/SKILL.md
  aoa_source_repo: 8Dionysus/aoa-skills
  aoa_technique_dependencies: AOA-T-0004,AOA-T-0028
  aoa_portable_profile: codex-facing-wave-3
---

# aoa-dry-run-first

## Intent
Use this skill to run a preview-first workflow that stops at inspection until one concrete mutation is ready, then asks for a visible confirmation before the real action proceeds.

## Trigger boundary
Use this skill when:
- the task can be simulated, previewed, or inspected before real execution
- the action may delete, restore, migrate, reconfigure, or otherwise alter a live surface
- the cost of a mistaken execution is meaningfully higher than the cost of a preview step
- the task needs a clear seam between dry-run evidence and the one confirmed mutating action

Do not use this skill when:
- no meaningful dry-run or preview path exists and the task is already clearly bounded and harmless
- the task is purely analytical and does not approach execution at all
- the central question is whether execution is authorized at all rather than how to preview it; use `aoa-approval-gate-check`
- the task is only about preparing a sanitized artifact for sharing; use `aoa-sanitized-share`

## Inputs
- requested action
- available preview or dry-run mechanisms
- touched surfaces
- known limits of the preview path

## Outputs
- dry-run or preview recommendation
- bounded preview result or plan
- note on what the preview does and does not prove
- explicit confirmation request for the mutating step when the preview is complete
- recommendation for next step

## Procedure
1. identify whether a preview, simulation, inspect-only, or dry-run path exists
2. prefer the safest bounded preview path before real execution
3. make explicit what the preview covers and what it cannot guarantee
4. stop at inspection until the mutating step is concrete enough to name
5. require one explicit confirmation before any state-changing action runs
6. avoid treating dry-run output as proof of total safety
7. recommend the next step only after the preview and confirmation seam have both been interpreted

## Contracts
- dry-run should reduce uncertainty before execution
- preview results should not be overstated
- lack of a dry-run path should be named as a risk, not hidden
- real execution should not be smuggled into a preview step
- confirmation must name the concrete mutation, not just generic permission
- the mutating step stays singular and bounded after the gate

## Risks and anti-patterns
- treating a preview as complete validation
- skipping preview because the real change looks small
- using a fake or weak preview path that does not touch the real risk surface
- blurring inspect-only and execute behavior
- turning the confirmation seam into a vague caution prompt
- chaining extra mutations after the confirmed step without a fresh gate

## Verification
- confirm a preview or dry-run path was considered first
- confirm the preview scope was named honestly
- confirm unresolved risk after preview was not hidden
- confirm the preview step did not silently perform the real action
- confirm the explicit confirmation names the exact mutating step
- confirm the recommended next step matches the preview confidence
- confirm the workflow stops after the confirmed mutation instead of widening into a loop

## Technique traceability
Manifest-backed techniques:
- AOA-T-0004 from `8Dionysus/aoa-techniques` at `cd276f040d55d490bd015b8698c7a5d594b9f875` using path `techniques/execution/intent-chain/intent-plan-dry-run-contract-chain/TECHNIQUE.md` and sections: Intent, When to use, Outputs, Core procedure, Validation
- AOA-T-0028 from `8Dionysus/aoa-techniques` at `cd276f040d55d490bd015b8698c7a5d594b9f875` using path `techniques/execution/agent-workflows-core/confirmation-gated-mutating-action/TECHNIQUE.md` and sections: Intent, When to use, Outputs, Core procedure, Validation

## Adaptation points
Future project overlays may add:
- local preview commands
- dry-run limitations
- restore or recovery expectations
- project-specific inspect-only patterns
