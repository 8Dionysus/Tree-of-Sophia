---
name: aoa-approval-gate-check
description: Classify whether a requested action is safe to proceed, requires explicit approval, or should not be executed. Use when a task may be destructive, security-sensitive, or operationally sensitive and the authority boundary is unclear. Do not use for clearly low-risk work or when the main need is previewing a mutation rather than deciding whether it may proceed.
license: Apache-2.0
compatibility: Designed for Codex or similar coding agents with repository file access and an interactive shell. Network access is optional and only needed when repository validation or referenced workflows require it.
metadata:
  aoa_scope: risk
  aoa_status: canonical
  aoa_invocation_mode: explicit-only
  aoa_source_skill_path: skills/risk/aoa-approval-gate-check/SKILL.md
  aoa_source_repo: 8Dionysus/aoa-skills
  aoa_technique_dependencies: AOA-T-0028
  aoa_portable_profile: codex-facing-wave-3
---

# aoa-approval-gate-check

## Intent
Use this skill to classify whether a requested action crosses an approval boundary and to return a bounded next step that keeps authority explicit before any execution proceeds.

## Trigger boundary
Use this skill when:
- a task may be destructive, operationally sensitive, or security-relevant
- the current authority level is unclear
- the agent needs to classify whether the next step is safe, explicit-only, or out of bounds
- the task needs an operational gate decision rather than a broader workflow plan

Do not use this skill when:
- the task is clearly low-risk and already bounded by an ordinary workflow
- no meaningful approval boundary exists in the current context
- the authority is already clear and the main need is choosing a preview path before execution; use `aoa-dry-run-first`
- the task is only about preparing a public-safe artifact for sharing; use `aoa-sanitized-share`

## Inputs
- requested action
- touched surfaces
- known approval state
- risk signals
- possible fallback or inspect-only path

## Outputs
- classification of the action: safe to proceed, explicit approval required, or do not execute
- note on whether explicit approval is needed
- bounded next-step recommendation
- report of unresolved authority assumptions
- a reviewable gate decision that can be passed to the next workflow step

## Procedure
1. identify the requested action and touched surfaces
2. assess whether the action could be destructive, sensitive, or authority-gated
3. classify the action as safe to proceed, explicit-approval required, or not appropriate to execute
4. prefer inspect-only or bounded alternatives when authority is insufficient
5. name the concrete next action or refusal in terms another agent can execute
6. report the classification and the reason for it

## Contracts
- unclear authority should not be silently interpreted as permission
- classification should be explicit and reviewable
- safer bounded alternatives should be preferred when possible
- the result should reduce accidental overreach
- the gate decision should be concrete enough to hand to the next step without ambiguity
- the skill remains a bounded operational package, not just a policy label

## Risks and anti-patterns
- assuming approval because a task sounds routine
- collapsing several risk levels into a single vague warning
- using approval logic to avoid useful bounded analysis
- hiding destructive steps behind innocent labels
- returning a generic caution instead of a real gate classification
- widening into a general workflow planner instead of a gate checker

## Verification
- confirm the touched surfaces were identified
- confirm the approval need was classified explicitly rather than as a vague warning
- confirm the next step fits the stated authority level
- confirm uncertainty was not masked as permission
- confirm the gate decision is actionable and bounded
- confirm the skill still reads as an approval-classification package

## Technique traceability
Manifest-backed techniques:
- AOA-T-0028 from `8Dionysus/aoa-techniques` at `cd276f040d55d490bd015b8698c7a5d594b9f875` using path `techniques/execution/agent-workflows-core/confirmation-gated-mutating-action/TECHNIQUE.md` and sections: Intent, When to use, Inputs, Outputs, Core procedure, Contracts, Risks, Validation

## Adaptation points
Future project overlays may add:
- local authority models
- local risk categories
- approval examples
- repository-specific explicit-only rules
