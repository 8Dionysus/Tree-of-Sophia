# Runtime Example

## Scenario

A reviewed diagnosis packet found repeated owner-layer collapse and missing
trigger-boundary fixtures in the session-harvest family.

## Why this skill fits

The next honest move is a bounded repair packet with checkpoint posture,
rollback thinking, and explicit owner targets rather than immediate silent
mutation.

## Expected inputs

- reviewed diagnosis packet
- target owner-layer candidates
- risk and approval posture
- known validation surfaces
- rollback anchors if available
- whether the repair is proposed, prepared, executing, verified, blocked, or handed off

## Expected outputs

- one `REPAIR_PACKET`
- a smallest diff shape or repair quest
- execution posture that distinguishes prepared repair from executed or verified repair
- approval need, rollback marker, health check, and improvement-log stub
- explicit stop conditions and escalation points
- one `REPAIR_CYCLE_RECEIPT` with diagnosis refs, checkpoint posture, and
  bounded verification refs
- one `CORE_SKILL_APPLICATION_RECEIPT` that records the finished
  `aoa-session-self-repair` run and points back to the detail receipt

## Boundary notes

- Do not use this skill without a reviewed diagnosis.
- Do not bypass approval or rollback posture.
- Do not turn a playbook-scale rollout into one fake "repair packet".

## Verification notes

- Confirm checkpoint fields are present.
- Confirm execution posture is present and does not overclaim validation.
- Confirm the repair stays bounded.
- Confirm escalation is named if the route widens.
- Confirm the finish receipt cites rollback and health-check posture without
  pretending the repair is already fully verified.
- Confirm the generic core receipt points back to the repair receipt and does
  not replace checkpoint posture or repair meaning.
