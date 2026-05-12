# Runtime Example

## Scenario

A reviewed `aoa-sdk` route already has a parent task anchor, a clear list of
expected outputs, and a bounded need for a local coding-agent reviewer to inspect one
package-layer fix before the parent route proceeds to validation.

## Why this skill fits

The parent route is already real and anchored. What is needed is not a whole
new orchestration layer but one bounded child route that keeps the same
passport, returns explicit outputs, and maps cleanly back into reviewed
closeout.

## Expected inputs

- a quest passport with the parent difficulty, risk, control mode, and
  delegate tier
- a summon request naming the child role, transport preference, anchor, and
  expected outputs
- the reviewed parent artifact or route note
- optional stress, checkpoint, or progression refs when the route needs them

## Expected outputs

- one summon decision that selects the local reviewed execution lane
- one local child target with named expected outputs
- reason codes explaining why local reviewed execution is sufficient
- a return plan that says how the child result rejoins the parent route
- a checkpoint bridge plan for reviewed closeout if the child narrows but does
  not fully finish the route
- an owner publication plan that keeps terminal meaning in the parent owner
  repo rather than in derived traces

The SDK-owned fixture
`repo:aoa-sdk/examples/a2a/summon_return_checkpoint_e2e.fixture.json` is the
current full-chain check for this scenario. It should validate this skill's v3
`summon_request` and `summon_result` schemas while leaving the SDK helper,
checkpoint bridge, memo candidate, eval packet, runtime dry-run receipt, and
routing re-entry in their owning repositories.

## Boundary notes

- Do not use this skill until the parent route has a real anchor and named outputs.
- Do not use this skill while several next routes still compete; use
  `aoa-session-route-forks` first.
- Do not let `d3+` work skip the split step.
- Do not let remote transport bypass the same gates that local execution must satisfy.
- Treat historical fixture suffixes such as wave labels as lineage only. Skill
  output should use stable v3 request/result names and explicit route refs.

## Verification notes

- Confirm the lane is justified by the passport and requested role.
- Confirm local coding-agent execution stayed the default.
- Confirm return and closeout planning are explicit.
- Confirm receipt or acceptance expectations are clear when the child crosses
  actor, session, or owner boundaries.
- Confirm any traces remain subordinate to reviewed closeout.
- Confirm owner publication stays in the canonical owner family.
