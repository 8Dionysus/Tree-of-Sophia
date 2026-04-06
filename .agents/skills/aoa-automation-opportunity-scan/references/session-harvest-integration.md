# Session-Harvest Integration Notes

This add-on should plug into the existing session-harvest family without
swallowing it.

## Donor-harvest

Allow `automation_candidate` as one extract kind inside `HARVEST_PACKET`.

Use donor-harvest to say "there may be automation value here."
Use `aoa-automation-opportunity-scan` to say "here is the automation candidate
packet and why it is or is not ready."

## Route-forks

Route-forks may compare:

- keep manual for now
- convert into a bounded skill
- package as a playbook automation seed
- defer until prerequisite repair lands

## Self-diagnose

Self-diagnose should be allowed to report blocked automation due to:

- unstable inputs
- unclear source of truth
- missing approval gate
- missing rollback marker
- weak post-change health checks
- secret or environment coupling

## Self-repair

Self-repair should be allowed to emit the smallest prerequisite packet needed
before automation can become honest.

## Progression-lift

Progression-lift may include an optional automation-readiness hint.
Do not turn that hint into an all-authority score.
