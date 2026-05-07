# Self-Repair Checkpoint Bridge

`aoa-session-self-repair` should stay skill-shaped, but it must respect the
self-agent checkpoint stack.

## Required checkpoints

- constitution or policy check
- approval gate
- rollback marker
- post-change health check
- bounded iteration limit
- explicit improvement log

## Repair rule

Self-repair may propose or author the smallest bounded repair packet, but it
must not silently mutate important system surfaces without naming checkpoint
posture.

## Good repair outputs

- repair quest
- bounded skill delta
- playbook correction note
- owner-layer issue packet
- explicit validation plan
- improvement-log stub

## Anti-patterns

- "the system fixed itself"
- hidden doctrine changes
- role-law changes without `aoa-agents`
- proof-law changes without `aoa-evals`
- silent retries disguised as reanchor
