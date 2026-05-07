---
name: aoa-checkpoint-closeout-bridge
description: Bridge provisional checkpoint evidence into one explicit reviewed closeout execution chain without turning notes into final harvest, progression, or quest authority. Use when a reviewed session artifact exists, checkpoint notes or closeout handoffs already carry focus hints, and the next honest move is to reread the reviewed artifact while driving donor harvest, progression lift, and quest harvest in that fixed order. Do not use for mid-session collection only, for hidden execution inside `aoa closeout run`, or when the request tries to mint final verdicts from checkpoint notes without reviewed evidence.
license: Apache-2.0
compatibility: Designed for Codex or similar coding agents with repository file access and an interactive shell. Network access is optional and only needed when repository validation or referenced workflows require it.
metadata:
  aoa_scope: core
  aoa_status: evaluated
  aoa_invocation_mode: explicit-preferred
  aoa_source_skill_path: skills/core/session-growth/aoa-checkpoint-closeout-bridge/SKILL.md
  aoa_source_repo: 8Dionysus/aoa-skills
  aoa_technique_dependencies: AOA-T-0075,AOA-T-0084,AOA-T-0089
  aoa_portable_profile: codex-facing-wave-3
---

# aoa-checkpoint-closeout-bridge

## Intent
Use this skill to connect checkpoint-led session growth with explicit reviewed
closeout execution.

It does not replace the core session-growth skills.
It keeps checkpoint capture provisional during the session and then drives the
explicit reviewed-closeout chain in the fixed order:

`aoa-session-donor-harvest -> aoa-session-progression-lift -> aoa-quest-harvest`

The fixed order is an execution discipline, not a promise that every stage will
produce a positive verdict. A stage may be skipped, deferred, or stopped with an
evidence-linked reason when its prerequisites are absent.

## Trigger boundary
Use this skill when:
- checkpoint notes already carry harvest, progression, or upgrade candidates
- reviewed closeout must use those candidates as focus hints without treating
  them as final authority
- the route needs one explicit bridge from local checkpoint evidence into the
  reviewed session-harvest family
- the agent must distinguish accepted focus hints from stale, cross-session, or
  contaminated checkpoint residue before downstream skills run
- the honest next move is not raw recap, but a bounded session-end chain

Do not use this skill when:
- the session is still active and only mid-session collection is needed
- there is no reviewed artifact yet
- the task is to implement or patch the bridge itself rather than execute a
  reviewed closeout chain
- the task wants final harvest, progression, or quest verdicts from checkpoint
  notes alone
- the request tries to make `aoa closeout run` a hidden skill runner
- the request demands that all three downstream stages produce verdicts even
  when reviewed evidence only supports a skip, defer, or stop

## Inputs
- reviewed session artifact
- optional local checkpoint note
- optional reviewed surface closeout handoff
- optional receipt refs from already published or staged session evidence
- repo scope and touched owner hints
- full session evidence available to the agent, such as the reviewed artifact,
  Codex rollout trace, transcript excerpts, and relevant receipts

## Outputs
- in `checkpoint-collect` mode:
  - updated local checkpoint note
  - updated provisional progression-axis hints
  - ordered session-end target list
- in `reviewed-closeout-execute` mode:
  - one explicit execution context bundle
  - accepted, rejected, and unresolved focus hints before the first downstream
    skill runs
  - one explicit run of:
    - `aoa-session-donor-harvest`
    - `aoa-session-progression-lift`
    - `aoa-quest-harvest`
  - explicit stage statuses: `executed`, `skipped`, `deferred`, or `stopped`
  - one execution report that records what ran, what was skipped, and which
    artifacts or receipts were emitted
  - one agent-authored closeout summary that says which conclusions came from
    reread session evidence, which checkpoint hints were accepted, and which
    hints were rejected as stale, cross-session, or contaminated

## Procedure
1. during checkpoint collection, keep the note append-only and provisional
2. treat checkpoint candidates and progression-axis hints as shortlist inputs,
   not verdict authority
3. publish the reviewed-closeout target order explicitly:
   - donor harvest first
   - progression lift second
   - quest harvest third
4. before executing reviewed closeout, the Codex agent must reread the skill
   instructions and the primary session evidence; checkpoint JSON, generated
   packets, and closeout reports are only navigation aids
5. build one closeout context bundle with reviewed artifact refs, checkpoint
   note refs, receipt refs, accepted focus hints, rejected focus hints,
   unresolved focus hints, and current-session boundaries
6. when executing reviewed closeout, reread the full reviewed artifact before
   every core skill stage
7. let checkpoint note and reviewed handoff narrow attention, but never replace
   the reviewed artifact or receipt evidence
8. explicitly separate current-session evidence from stale, neighboring, or
   diagnostic checkpoint contamination before naming final candidates
9. run `aoa-session-donor-harvest` first so reusable units and owner routing
   are bounded before any progression verdict
10. run `aoa-session-progression-lift` second so final multi-axis movement is
   gathered from reviewed evidence, donor outputs, and provisional checkpoint
   axes together
11. run `aoa-quest-harvest` third so final promotion triage happens only after
   donor harvest and progression lift have finished
12. if a downstream stage lacks prerequisites, record `skipped`, `deferred`, or
   `stopped` with a reason and evidence refs instead of forcing a verdict to
   satisfy the chain shape
13. keep stats refresh out of this bridge skill; that remains downstream after
   explicit closeout receipts exist

## Contracts
- checkpoint note stays provisional and lower-authority than reviewed closeout
- `checkpoint-collect` never emits final harvest, progression, or quest verdicts
- `reviewed-closeout-execute` must reread the reviewed artifact and any receipt
  evidence; it may not trust checkpoint notes alone
- generated bridge artifacts are mechanical outputs, not proof that the agent
  applied the skill protocol; the agent must still produce the final analytical
  judgment from reread evidence
- core skill order is fixed:
  - `aoa-session-donor-harvest`
  - `aoa-session-progression-lift`
  - `aoa-quest-harvest`
- fixed order means the dependency order for evaluation and reporting; it does
  not force final verdicts when a stage's prerequisites are absent
- this bridge skill coordinates core skills; it does not replace their meaning
- `aoa closeout run` remains a separate receipt-first publisher path
- stats refresh is not part of this skill

## Risks and anti-patterns
- treating checkpoint hints as if they were final donor harvest truth
- treating `execute-closeout-chain` JSON as if it were the actual skill
  application instead of an artifact bundle that still needs agent judgment
- skipping donor harvest and jumping straight to progression or quest verdicts
- using progression verdicts that were not reread against the reviewed artifact
- letting quest harvest run before progression lift
- forcing quest harvest or progression lift to emit a positive verdict when
  reviewed evidence only supports skip, defer, hold, or stop
- turning the bridge into a hidden auto-runner inside `aoa closeout run`
- refreshing stats before the explicit reviewed-closeout chain finishes

## Verification
- confirm `checkpoint-collect` only updates local ledger surfaces
- confirm the bridge never emits final verdicts during mid-session capture
- confirm reviewed closeout refuses to run without a reviewed artifact
- confirm the core skill order is fixed and visible
- confirm each downstream stage records `executed`, `skipped`, `deferred`, or
  `stopped` with an evidence-linked reason
- confirm checkpoint note and reviewed handoff are treated as hints rather than
  sole evidence
- confirm the final answer distinguishes current-session evidence from
  checkpoint hints and from stale or contaminated ledger entries
- confirm the execution report's mechanical bridge outputs were not presented
  as autonomous skill analysis
- confirm stats refresh remains downstream of the bridge execution

## Technique traceability
Manifest-backed techniques:
- AOA-T-0075 from `8Dionysus/aoa-techniques` at `b0f7e094bf81abaf0895a729d504a97f3af91ae8` using path `techniques/continuity/donor-harvest/session-donor-harvest/TECHNIQUE.md` and sections: Intent, Inputs, Outputs, Contracts, Validation
- AOA-T-0084 from `8Dionysus/aoa-techniques` at `b0f7e094bf81abaf0895a729d504a97f3af91ae8` using path `techniques/continuity/donor-harvest/progression-evidence-lift/TECHNIQUE.md` and sections: Intent, Inputs, Outputs, Contracts, Validation
- AOA-T-0089 from `8Dionysus/aoa-techniques` at `b0f7e094bf81abaf0895a729d504a97f3af91ae8` using path `techniques/governance/promotion-boundary/quest-unit-promotion-review/TECHNIQUE.md` and sections: Intent, When to use, Inputs, Outputs, Core procedure, Contracts, Validation

## Adaptation points
Project overlays may add:
- local reviewed artifact entrypoints
- local receipt bundle locations
- local stop conditions before quest triage
- local stage-status vocabulary and report locations, as long as the donor ->
  progression -> quest dependency order remains visible
- local owner-layer route maps for generated donor packets
- project-local reviewed follow-through notes under `docs/session-harvests/`
  when bridge upkeep becomes reusable but is still below promotion authority
- project-local reminders that downstream `aoa-session-self-diagnose`, owner
  follow-through, and stats refresh stay explicit next steps outside this skill
- local post-closeout stats or owner follow-through commands that stay outside
  this bridge skill
