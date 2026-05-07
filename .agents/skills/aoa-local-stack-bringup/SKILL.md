---
name: aoa-local-stack-bringup
description: Bring up a bounded local multi-service stack by rendering runtime truth, checking selector-aware host readiness, and launching through one explicit lifecycle entrypoint with a visible stop path. Use when profiles, presets, or overlays can change what starts and host readiness must be reviewed before launch. Do not use for remote deployment, pure diagnostics without launch intent, or generic infra-change work.
license: Apache-2.0
compatibility: Designed for Codex or similar coding agents with repository file access and an interactive shell. Network access is optional and only needed when repository validation or referenced workflows require it.
metadata:
  aoa_scope: risk
  aoa_status: evaluated
  aoa_invocation_mode: explicit-only
  aoa_source_skill_path: skills/risk/aoa-local-stack-bringup/SKILL.md
  aoa_source_repo: 8Dionysus/aoa-skills
  aoa_technique_dependencies: AOA-T-0036,AOA-T-0037,AOA-T-0028,AOA-T-0038
  aoa_portable_profile: codex-facing-wave-3
---

# aoa-local-stack-bringup

## Intent
Use this skill to turn local multi-service startup into a reviewable bring-up workflow with a visible pre-start seam and one explicit launch path.

## Trigger boundary
Use this skill when:
- a local stack has more than one meaningful service or runtime layer
- the selected profile, preset, or overlay can change what will actually start
- startup depends on host readiness signals that should be checked before launch
- the operator needs one explicit launch path and stop path after pre-start review
- a plain infrastructure change workflow would miss the local runtime selection, preflight, or lifecycle seam

Do not use this skill when:
- the main task is remote deployment, continuous monitoring, or fleet orchestration
- the task is only to inspect rendered runtime truth without any launch decision
- the task is only to diagnose readiness without planning a bounded local bring-up
- the main question is whether authority exists at all; use `aoa-approval-gate-check`
- the task is a broader operational or configuration change with no bounded local stack bring-up; use `aoa-safe-infra-change`

## Inputs
- selected local runtime, profile, or preset
- available render-truth surface for the chosen runtime
- available readiness or doctor surface for the chosen runtime
- one explicit local lifecycle entrypoint
- intended stop path or cleanup path

## Outputs
- rendered runtime truth summary for what would actually start
- selector-aware readiness verdict with named blockers or warnings
- explicit go, hold, or confirm-before-launch recommendation
- started bounded local stack with visible stop path, or a deferred-start report
- concise runtime report with unresolved warnings, blockers, or cleanup notes

## Procedure
1. name the selected runtime, the expected service set, and the explicit launch surface before starting anything
2. render the effective service list or composed runtime truth before startup, and keep any full rendered config local and controlled
3. run the selector-aware doctor or readiness surface for the same selected runtime and note `ok`, `warn`, or `fail` items
4. stop and report if the render or readiness step exposes a blocker or unresolved ambiguity that should be fixed before launch
5. require explicit operator confirmation before launch when startup would materially mutate the local runtime or when warnings remain
6. launch the bounded stack through one operator-facing lifecycle entrypoint and keep the stop path visible
7. report what was rendered, what the readiness verdict said, what started, and what remains risky, deferred, or cleanup-sensitive

## Contracts
- render review happens before startup
- readiness checks stay tied to the selected runtime rather than acting like one global score
- startup remains explicit and reviewable rather than hiding behind ambient background state
- the skill uses one bounded lifecycle entrypoint and one visible stop path
- rendered truth, readiness, confirmation, and lifecycle remain distinct steps even when one script wraps parts of them together
- the workflow stays local-stack oriented and does not widen into generic bootstrap, deployment, or monitoring doctrine

## Risks and anti-patterns
- treating rendered truth as proof that the host is ready or that startup will succeed
- treating a passing doctor result as proof of post-start health
- skipping the pre-start review seam because the launch command feels convenient
- leaking full rendered config or other sensitive local runtime material
- hiding startup in background helpers that obscure what started or how to stop it
- widening the bundle into generic install, bootstrap, or platform-management doctrine

## Verification
- confirm the rendered output answered what would actually start before launch
- confirm the readiness verdict was selector-aware and itemized with visible severity
- confirm blockers stopped the launch path or were explicitly deferred with explanation
- confirm explicit operator intent or confirmation existed before the mutating launch step
- confirm startup status named what started and how to stop it
- confirm unresolved warnings, sensitive rendered material handling, and cleanup notes were reported clearly

## Technique traceability
Manifest-backed techniques:
- AOA-T-0036 from `8Dionysus/aoa-techniques` at `cd276f040d55d490bd015b8698c7a5d594b9f875` using path `techniques/execution/runtime-truth-lifecycle/render-truth-before-startup/TECHNIQUE.md` and sections: Intent, When to use, Inputs, Outputs, Core procedure, Contracts, Risks, Validation
- AOA-T-0037 from `8Dionysus/aoa-techniques` at `cd276f040d55d490bd015b8698c7a5d594b9f875` using path `techniques/execution/runtime-truth-lifecycle/contextual-host-doctor/TECHNIQUE.md` and sections: Intent, When to use, Inputs, Outputs, Core procedure, Contracts, Risks, Validation
- AOA-T-0028 from `8Dionysus/aoa-techniques` at `cd276f040d55d490bd015b8698c7a5d594b9f875` using path `techniques/execution/agent-workflows-core/confirmation-gated-mutating-action/TECHNIQUE.md` and sections: Intent, When to use, Inputs, Outputs, Core procedure, Contracts, Risks, Validation
- AOA-T-0038 from `8Dionysus/aoa-techniques` at `cd276f040d55d490bd015b8698c7a5d594b9f875` using path `techniques/execution/runtime-truth-lifecycle/one-command-service-lifecycle/TECHNIQUE.md` and sections: Intent, When to use, Inputs, Outputs, Core procedure, Contracts, Risks, Validation

## Adaptation points
Project overlays should add:
- local render commands and safe storage rules for rendered config
- local runtime selectors, profiles, or presets
- local readiness checks and strictness policy
- local lifecycle entrypoints, stop commands, and cleanup expectations
