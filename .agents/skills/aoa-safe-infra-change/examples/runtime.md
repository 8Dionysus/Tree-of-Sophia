# Runtime Example

## Scenario
You need to update a deployment setting that controls a service timeout and restart policy in a non-production environment before rolling the same change forward.

## Why this skill fits
This is a bounded infrastructure change with operational impact. The skill helps keep the change small, explicit, and verifiable while preserving rollback thinking.

## Expected inputs
- the target operational surface
- the exact config or infra change
- the rollout or validation plan
- any rollback or recovery idea

## Expected outputs
- a bounded change plan
- the updated infra or config action
- verification results that match the risk level
- any remaining operational risk or rollback note

## Boundary notes
- If the task is only about deciding whether the change is allowed, use approval-gate logic instead.
- If the task is only about preparing a preview, keep it inspect-only.
- Keep unrelated cleanup out of the change.

## Verification notes
- Confirm the operational surface was named clearly.
- Confirm the change stayed bounded to the requested setting.
- Confirm verification and rollback thinking were explicit.
- Confirm no wider operational churn was introduced.
