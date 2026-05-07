# Bridge To Playbook Automation Seeds

Use this bridge when `aoa-automation-opportunity-scan` detects something that
should become a candidate automation seed.

## First honest landing

| shape | first landing |
|---|---|
| one bounded executable workflow | `aoa-skills` |
| recurring multi-skill or scheduled scenario | `aoa-playbooks` automation seed candidate |
| stable reusable practice behind multiple automations | `aoa-techniques` |
| approval-heavy self-change route | `aoa-agents` checkpoint posture plus proof hooks |
| not ready yet | repair quest, technique candidate, or defer |

## Minimum seed-ready posture

A candidate should usually be `seed_ready: true` only when it can name:

- the current manual route
- stable inputs and outputs
- a likely owning playbook or skill handle
- a bounded prompt or activation description
- the highest honest `automation_mode_posture`
- schedule hints as hints, not authority

## What not to do

Do not confuse:

- an automation seed with a live scheduler
- a playbook seed with a secret-bearing ops script
- a skill with a recurring scenario composition surface
- a human-approved execution route with unattended background authority
