# Runtime Example

## Scenario

Bring up a local application stack with an API, worker, and database after choosing a profile that may change which services actually start.

## Why this skill fits

This is a bounded local bring-up task, not just a generic configuration edit. The operator needs to see the rendered runtime shape, confirm host readiness for that exact selection, and then start the stack through one explicit lifecycle path.

## Expected inputs

- selected local profile, preset, or equivalent runtime choice
- render command or surface for the chosen runtime
- readiness or doctor command for the chosen runtime
- explicit lifecycle entrypoint and known stop path
- any blocker or warning policy that affects go versus hold

## Expected outputs

- rendered summary of what would actually start
- readiness verdict with named blockers or warnings
- explicit go, hold, or confirm-before-launch recommendation
- startup status or deferred-start report with stop-path notes

## Boundary notes

- Do not use this skill for remote deployment, ongoing monitoring, or fleet orchestration.
- If the task is a broader infrastructure change without a bounded local stack bring-up, use `aoa-safe-infra-change`.
- If the main question is whether the action is authorized at all, use `aoa-approval-gate-check` first.

## Verification notes

- Confirm rendered truth was reviewed before startup.
- Confirm readiness signals were tied to the selected runtime.
- Confirm launch required explicit operator intent before mutating runtime state.
- Confirm the result states what started, how to stop it, and what remains risky or deferred.
