# Runtime Example

## Scenario
You are asked to rotate credentials for a production service, but the request does not say who approved the change or whether the work is allowed in the current window.

## Why this skill fits
This is an authority question first. The task may be sensitive, so the right step is to classify whether execution can proceed or needs explicit approval before anything changes.

## Expected inputs
- the requested action
- the target surface
- any stated approval or authority context
- risk signals such as production impact or data access

## Expected outputs
- a clear classification: safe to proceed, explicit approval required, or do not execute
- the missing authority assumption, if any
- the safest bounded next step, such as waiting for approval or using an inspect-only path

## Boundary notes
- If the request is already clearly authorized, this skill should not be used to add extra process.
- If the main task is only to preview a safe execution path, a different workflow is a better fit.
- Do not treat uncertainty as permission.

## Verification notes
- Confirm the authority question was answered explicitly.
- Confirm the next step matches the stated approval level.
- Confirm unresolved authority gaps were named, not hidden.
