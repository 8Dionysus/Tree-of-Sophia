# Runtime Example

## Scenario

A bounded `aoa-sdk` patch is already applied, the targeted tests and lint checks
are green, and the next honest move is one local commit that preserves the
repair as a distinct review boundary before any later push or merge step.

## Why this skill fits

The coding work is done for now, but the commit boundary still matters. The
working tree needs one intentional commit with an honest message and a visible
stop line so the session does not blur commit, push, and publication into one
hidden loop.

## Expected inputs

- one bounded local diff
- `git status` showing whether unrelated changes are present
- the explicit validation results that justify the commit boundary
- explicit operator authorization for one local commit now
- the intended stop line after commit

## Expected outputs

- one bounded commit-or-defer decision
- `commit_authorization_posture: authorized_now`
- one local commit that matches the intended diff
- one short note on what was verified and what remains outside the commit
- one explicit stop line before push, PR, or publish

## Boundary notes

- Do not use this skill when the task still clearly needs more repair or verification.
- Do not cross the commit boundary when the operator only asked for review,
  preparation, or analysis.
- Do not use this skill as a hidden push or publish workflow.
- If the main question is whether the mutation is authorized at all, use the approval-gate skill first.
- If the main need is the broader coding workflow rather than the commit boundary itself, use `aoa-change-protocol`.

## Verification notes

- Confirm the commit boundary matched the actual diff.
- Confirm unrelated local changes were not swept in silently.
- Confirm commit authorization was explicit before mutation.
- Confirm the validation story was named honestly.
- Confirm the route stopped after the local commit boundary.
