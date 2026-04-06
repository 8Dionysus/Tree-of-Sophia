# Checkpoint Boundary For Automation Opportunity Scanning

Automation opportunity detection should stay cheap.
Checkpoint posture should activate only when the candidate route would reshape
important system surfaces, cross approval boundaries, or act in self-changing
ways.

## Strong checkpoint triggers

Mark `checkpoint_required: true` when the candidate would:

- mutate important system surfaces
- operate with hidden or changing authority
- require rollback discipline that is not already explicit
- act as a self-repair or self-upgrade route
- blur thinker and operator roles

## Typical safe lower-risk candidates

Usually lower-risk candidates look like:

- report generation
- repeated drift checks
- repeatable session closeout packaging
- bounded read-only audits
- preview or dry-run orchestration

## Resulting action

When checkpoint posture is needed, the next artifact is usually not a ready
automation seed.
It is a checkpoint-aware repair route, playbook review packet, or bounded skill
proposal with explicit approval seams.
