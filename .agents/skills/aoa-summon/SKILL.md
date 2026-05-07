---
name: aoa-summon
description: Delegate one bounded child route through quest-passport law, local-first Codex execution defaults, hard gates for progression, self-agent, and stress posture, and governed return into reviewed closeout. Use when a parent route already has a real anchor and named outputs and the honest next move is a narrower child reviewer, evaluator, verifier, or leaf helper rather than a larger orchestration layer. Do not use when the route lacks an anchor, the outputs are unnamed, the work is `d3+` and still needs a split, or remote delegation is being used to bypass proof, approval, or closeout posture.
license: Apache-2.0
compatibility: Designed for Codex or similar coding agents with repository file access and an interactive shell. Network access is optional and only needed when repository validation or referenced workflows require it.
metadata:
  aoa_scope: core
  aoa_status: evaluated
  aoa_invocation_mode: explicit-only
  aoa_source_skill_path: skills/core/session-growth/aoa-summon/SKILL.md
  aoa_source_repo: 8Dionysus/aoa-skills
  aoa_technique_dependencies: AOA-T-0078,AOA-T-0079,AOA-T-0062,AOA-T-0058
  aoa_portable_profile: codex-facing-wave-3
---

# aoa-summon

## Intent
Use this skill to decide whether and how to launch one bounded child route from
an already-anchored parent route.

The goal is not delegation theater.
The goal is to preserve quest-passport law, Codex-first local execution,
stress narrowing, progression and self-agent gates, return planning,
checkpoint-aware reviewed closeout, and owner-local publication mapping.

## Trigger boundary
Use this skill when:
- a parent route already has a real anchor and named expected outputs
- a narrower child actor would help through bounded review, evaluation, leaf
  implementation, or local verification
- the child result must map back into return, closeout, and owner-publication
  surfaces
- local Codex execution is the honest default unless a separate execution
  surface is actually required

Do not use this skill when:
- the parent route still lacks a real anchor or named outputs
- multiple plausible next routes still compete and need fork cards first
- the route is `d3+` and still needs a split before any child launch
- delegation would widen authority, hide approval posture, or bypass a human
  gate
- the task is asking for a whole orchestration mesh rather than one bounded
  child route
- remote transport is being requested for prestige rather than necessity

## Inputs
- a quest passport with difficulty, risk, control mode, and delegate tier
- a summon request with desired role or child target, transport preference, one
  parent anchor (`route_anchor`, `parent_task_id`, or `session_ref`), and
  named expected outputs
- named expected outputs for the child route
- optional reviewed artifact path, stress bundle ref, checkpoint note ref,
  Codex trace ref, progression overlay ref, self-agent checkpoint ref, and
  audit refs

## Outputs
- one summon decision with allowed or blocked posture
- one chosen lane such as `codex_local_leaf`, `codex_local_reviewed`,
  `remote_reviewed`, `split_required`, or `human_gate`
- `execution_surface`, `cohort_pattern`, `reason_codes`, and `blocked_actions`
- optional `codex_local_target`
- `return_plan`, `checkpoint_bridge_plan`, `memo_export_plan`, and
  `owner_publication_plan`
- return receipt or acceptance expectation when the child crosses an actor,
  session, or owner boundary
- `closeout_required`, `checkpoint_required`, and `progression_required`

## Procedure
1. start from the parent anchor and quest passport, not from raw pressure to delegate
2. default `transport_preference` to `codex_local` when the request leaves it open
3. verify that expected outputs are named before picking a lane
4. classify the lane with difficulty, risk, control mode, requested role, and
   `references/passport-lane-matrix.v3.md`
5. keep low-risk `d0_probe`, `d1_patch`, and bounded `d2_slice` leaf work in
   `codex_local_leaf` when anchor and outputs are clear
6. keep local reviewer, evaluator, and architect-like narrowing work in
   `codex_local_reviewed`
7. allow `remote_reviewed` only when a separate endpoint or execution surface
   is actually required
8. if difficulty is `d3+`, return `split_required` instead of launching child execution
9. if stress posture says `stop_before_mutation`, only allow a narrowing
   non-mutating child; otherwise gate to human review
10. if `require_progression` is true or the route depends on reviewed unlock
    posture, require a reviewed progression overlay before summon
11. if the route is self-agent or policy-sensitive, require reviewed
    self-agent checkpoint posture before summon
12. emit return, checkpoint bridge, memo export, and owner publication plans
    that stay subordinate to reviewed closeout and owner truth
13. name whether a return receipt or acceptance state is needed before the
    parent route may continue
14. apply `references/no-raw-traces-rule.md` so child traces remain aids rather
    than proof or memo canon
15. map failed, narrowed, or blocked child states back into return posture
    instead of letting them disappear

## Contracts
- this skill governs one bounded child route; it does not grant hidden
  orchestration authority
- local Codex child targeting is the default first choice, not an afterthought
- branch choice must already be settled; unresolved route choice belongs to
  `aoa-session-route-forks`
- the SDK-owned E2E fixture at
  `repo:aoa-sdk/examples/a2a/summon_return_checkpoint_e2e.fixture.json`
  may be used to check this skill's v3 request/result contract, but it does not
  move summon law into `aoa-sdk`
- stress may narrow or block, but it may never widen authority
- remote transport follows the same passport, return, checkpoint, and closeout
  law as local transport
- `d3+` routes split first
- progression and self-agent gates are real blockers when required evidence is
  missing
- child traces, memos, and receipts do not replace rereading the reviewed
  parent artifact
- terminal publication stays inside canonical owner families, and memo export
  stays reviewed and candidate-oriented
- failed, narrowed, or blocked child results still map into an explicit return
  surface
- cross-boundary child work needs an explicit receipt or acceptance expectation
  before the parent route treats the child result as returned

## Risks and anti-patterns
- treating delegation as progress without anchor or output clarity
- using summon to skip branch choice when several next routes still compete
- using a remote child route to bypass local proof, approval, or closeout
  posture
- allowing child scope to widen beyond the parent passport
- reading child traces as proof authority or memo canon
- skipping the required split for `d3+` work
- publishing terminal meaning into derived layers before owner-local landing

## Verification
- confirm the passport and parent anchor exist
- confirm expected outputs are named
- confirm branch choice is already settled; otherwise route to
  `aoa-session-route-forks`
- confirm the chosen lane matches difficulty, risk, control mode, and requested
  role
- confirm any E2E fixture check validates `summon_request` and `summon_result`
  against `references/summon-request-v3.schema.json` and
  `references/summon-result-v3.schema.json`
- confirm `d3+` routes split before summon
- confirm stress only narrows or blocks
- confirm progression and self-agent gates are not bypassed
- confirm return, checkpoint bridge, and publication plans stay explicit for
  nonterminal or failed child outcomes
- confirm cross-boundary child work has a return receipt or acceptance
  expectation before parent continuation
- confirm terminal publication stays owner-local and memo export stays reviewed
  and candidate-oriented
- confirm traces stay subordinate to reviewed closeout and parent reread

## Technique traceability
Manifest-backed techniques:
- AOA-T-0078 from `8Dionysus/aoa-techniques` at `b4e5c1446469f142c60a85d0a7a4d9de7835ea65` using path `techniques/governance/decision-routing/decision-fork-cards/TECHNIQUE.md` and sections: Intent, Inputs, Outputs, Core procedure, Contracts, Validation
- AOA-T-0079 from `8Dionysus/aoa-techniques` at `b4e5c1446469f142c60a85d0a7a4d9de7835ea65` using path `techniques/governance/decision-routing/risk-passport-lift/TECHNIQUE.md` and sections: Intent, Inputs, Outputs, Contracts, Risks, Validation
- AOA-T-0062 from `8Dionysus/aoa-techniques` at `b4e5c1446469f142c60a85d0a7a4d9de7835ea65` using path `techniques/continuity/handoff-continuation/episode-bounded-agent-loop/TECHNIQUE.md` and sections: Intent, Outputs, Core procedure, Contracts, Risks, Validation
- AOA-T-0058 from `8Dionysus/aoa-techniques` at `b4e5c1446469f142c60a85d0a7a4d9de7835ea65` using path `techniques/continuity/handoff-continuation/receipt-confirmed-handoff-packet/TECHNIQUE.md` and sections: Intent, Outputs, Core procedure, Contracts, Risks, Validation

## Adaptation points
- project overlays may add local child-role labels, control modes, or transport
  labels
- project overlays may map local reviewed-closeout or checkpoint families onto
  the core summon result without weakening the gates
- project overlays may specialize owner publication families or memo candidate
  classes while keeping the parent return route explicit
