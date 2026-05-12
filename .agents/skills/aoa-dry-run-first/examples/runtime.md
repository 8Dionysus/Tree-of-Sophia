# Runtime Example

## Scenario

Preview a public configuration migration before applying it to a live service.

## Why this skill fits

This task can be inspected or simulated before execution, and the live surface may change if the migration is applied incorrectly.

## Expected inputs

- requested action
- available preview or dry-run mechanism
- the touched operational surface
- known limits of the preview path

## Expected outputs

- the preview result or dry-run summary
- the differences or risks observed
- a recommendation for the next step

## Boundary notes

- Do not use this skill when there is no meaningful preview path and the task is already clearly harmless.
- If the main question is whether the action is authorized, use the approval-gate skill first.
- If the task is only about preparing a shareable artifact, use the sanitized-share skill instead.

## Verification notes

- Confirm the preview was run before any real execution.
- Confirm the preview did not silently perform the real action.
- Confirm any remaining risk was named explicitly.
- Confirm the next step matches the confidence level of the preview.
