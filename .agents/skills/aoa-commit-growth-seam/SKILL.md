---
name: aoa-commit-growth-seam
description: Turn a validated bounded diff into one intentional local commit with explicit scope review, named verification carry-forward, and a visible stop line before push or publish. Use when a bounded code, config, or docs diff is already prepared locally and the next honest move is one local commit boundary rather than more coding. Do not use when the task still needs more repair or verification, when no bounded diff exists, or when the main task is push, PR, release, or public-share.
license: Apache-2.0
compatibility: Designed for Codex or similar coding agents with repository file access and an interactive shell. Network access is optional and only needed when repository validation or referenced workflows require it.
metadata:
  aoa_scope: core
  aoa_status: evaluated
  aoa_invocation_mode: explicit-only
  aoa_source_skill_path: skills/core/session-growth/aoa-commit-growth-seam/SKILL.md
  aoa_source_repo: 8Dionysus/aoa-skills
  aoa_technique_dependencies: AOA-T-0001,AOA-T-0028
  aoa_portable_profile: codex-facing-wave-3
---

# aoa-commit-growth-seam

## Intent
Use this skill to cross one honest local commit boundary after a bounded change
is already prepared and reviewed enough to stop widening the working session.

The goal is not "commit because work happened."
The goal is to preserve one bounded unit with an honest message, explicit
validation carry-forward, explicit commit authorization posture, and a visible
stop line before any push, PR, release, or public-share route begins.

## Trigger boundary
Use this skill when:
- a bounded code, config, or docs diff is already prepared locally
- the next honest move is one local commit boundary rather than more coding
- the commit needs to preserve what was verified and what remains unresolved
- the working tree may contain unrelated changes that must stay outside the
  commit
- visible operator intent authorizes this local commit boundary now
- the workflow needs a visible stop line before push, PR, release, or broader
  closeout follow-through

Do not use this skill when:
- the task still needs more coding, repair, or verification before any commit
  is honest
- no bounded diff exists yet
- the operator only asked for review, preparation, or a report and did not
  authorize a local commit now
- the main question is whether a mutating step is authorized at all; use
  `aoa-approval-gate-check`
- the main task is push, PR, release, or public-share rather than the local
  commit boundary itself
- the route should trigger or classify a post-commit review artifact rather
  than author the commit boundary itself

## Inputs
- one bounded local diff
- working tree status, including any unrelated local changes
- validation results or explicit validation debt
- explicit commit authorization or a defer marker
- intended commit scope and message shape
- explicit next stop line after the commit

## Outputs
- one bounded commit-or-defer decision
- `commit_authorization_posture`, such as `authorized_now`, `defer_commit`,
  `needs_split`, or `needs_verification`
- one intentional local commit, or one explicit refusal to commit yet
- one honest commit message that matches the bounded unit
- one carry-forward note for what was verified and what remains unresolved
- one explicit stop line before any push, PR, or publish path

## Procedure
1. reread `git status` and the target diff to isolate the exact bounded unit
2. stop if unrelated changes are mixed in and cannot be excluded cleanly
3. name the `commit_authorization_posture`; only `authorized_now` may cross
   the local commit boundary
4. restate what was verified, what was intentionally not verified, and what the
   commit boundary is meant to preserve
5. confirm the proposed commit is still one bounded mutation rather than the
   start of a hidden push or publish loop
6. shape one commit message that matches the bounded unit and any remaining
   debt honestly
7. create one local commit only for the intended diff
8. record or report the commit ref together with the validation state and next
   stop line
9. stop after the local commit and hand off push, PR, review, release, or
   closeout publication explicitly if still needed

## Contracts
- the commit boundary matches the actual bounded diff
- unrelated local state is not silently swept into the commit
- verification status stays attached to the commit story
- `authorized_now` is required before the skill mutates the local repository
- the local commit does not imply push, publish, merge, or release
- if the route needs a new approval seam or broader workflow, it hands off
  instead of stretching this skill

## Risks and anti-patterns
- using one commit as a catch-all bucket for unrelated changes
- writing a clean commit message that hides missing or weak verification
- treating the local commit boundary as silent permission to push or publish
- treating a ready diff as commit authority when the operator only asked for
  analysis, review, or preparation
- committing while the task still clearly needs more repair or diff cleanup
- widening the skill into a release, PR, or post-commit automation workflow

## Verification
- confirm the committed diff matches the named bounded unit
- confirm `commit_authorization_posture` was `authorized_now` before the commit
- confirm unrelated local changes were excluded or explicitly deferred
- confirm the validation state was named honestly before the commit happened
- confirm the commit message reflects the real change rather than generic
  success language
- confirm the workflow stopped at the local commit boundary and kept push,
  publish, or review as separate next moves

## Technique traceability
Manifest-backed techniques:
- AOA-T-0001 from `8Dionysus/aoa-techniques` at `b0f7e094bf81abaf0895a729d504a97f3af91ae8` using path `techniques/execution/agent-workflows-core/plan-diff-apply-verify-report/TECHNIQUE.md` and sections: Intent, When to use, Inputs, Outputs, Core procedure, Contracts, Risks, Validation
- AOA-T-0028 from `8Dionysus/aoa-techniques` at `b0f7e094bf81abaf0895a729d504a97f3af91ae8` using path `techniques/execution/agent-workflows-core/confirmation-gated-mutating-action/TECHNIQUE.md` and sections: Intent, When to use, Inputs, Outputs, Core procedure, Contracts, Risks, Validation

## Adaptation points
Project overlays may add:
- local commit-message conventions or trailers
- local validation commands that should be named before commit
- local dirty-worktree split rules when unrelated edits are present
- local stop-line conventions for push, PR, release, or public-share handoff
