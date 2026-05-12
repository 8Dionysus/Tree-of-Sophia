# Runtime Example

## Scenario

A reviewed run shows that the same weekly doc drift check keeps repeating with
the same inputs, the same checklist, and the same small report output. The
team wants to know whether this should stay manual, become a bounded skill, or
become a playbook automation seed candidate.

## Why this skill fits

The route is no longer just donor residue. It is a recurring manual process
with repeat evidence, identifiable triggers, and a real question about whether
automation is honest yet.

## Expected inputs

- reviewed session or review packet
- current manual route
- repeat signal and friction notes
- input/output clarity and current proof posture
- approval or rollback posture if known

## Expected outputs

- one `AUTOMATION_OPPORTUNITY_PACKET`
- one `AUTOMATION_CANDIDATE` card for the doc drift check
- an explicit `seed_ready` or `not_now` verdict
- `automation_mode_posture: dry_run_preview` when the first honest automation mode is preview-only
- `checkpoint_required: false` if the route is read-only and previewable
- a next-artifact suggestion such as `skill` or `playbook_seed`
- one `AUTOMATION_CANDIDATE_RECEIPT` with repeat signal posture,
  `checkpoint_required`, and the next-artifact hint
- one `CORE_SKILL_APPLICATION_RECEIPT` that records the finished
  `aoa-automation-opportunity-scan` run without replacing the detector receipt

## Boundary notes

- Do not use this skill to grant live schedule authority.
- Do not use this skill when the route is still a one-off creative exploration.
- Do not smuggle self-repair or self-upgrade through an automation verdict.

## Verification notes

- Confirm the candidate names the current manual route.
- Confirm repeat evidence is explicit.
- Confirm the likely owner layer is named honestly.
- Confirm the automation mode posture is conservative.
- Confirm schedule hints stay hints.
- Confirm the finish receipt stays detector-shaped and does not pretend to
  grant schedule authority.
- Confirm the generic core receipt points back to the detector receipt and does
  not widen into automation authority.
