# Runtime Example

## Scenario

A reviewed session artifact contains several potentially reusable outcomes:
1. a repeatable practice for turning raw post-session notes into a bounded donor packet
2. one bounded leaf workflow for explicit post-session harvesting by a local coding agent
3. one broader recurring rollout route that spans skill drafting, review, and later proof updates
4. one unresolved post-session branch where the next route should stay explicit
5. one repeated manual review-closeout ritual that may be ripe for automation but still needs explicit readiness classification
6. one checkpoint hint that looks relevant but must be accepted or rejected
   against the reviewed artifact before it can influence candidate minting

## Why this skill fits

The task is no longer about executing the original work. It is about extracting reusable units from a finished session and routing each one to the correct owner layer without collapsing practice, workflow, and scenario composition into one object.

## Expected inputs

- reviewed session transcript or compaction note
- candidate repeat or reuse signals
- touched AoA layers
- uncertainty notes and residual boundary risk

## Expected outputs

- one `HARVEST_PACKET`
- one reviewed intake note that accepts, rejects, or carries checkpoint and
  closeout-handoff hints before any `candidate_ref` is minted
- one reviewed accepted candidate with a minted `candidate_ref`
- one carried `cluster_ref` when the reviewed source already named it
- one candidate routed to `aoa-techniques` as reusable practice meaning
- one candidate routed to `aoa-skills` as a bounded executable leaf workflow
- one candidate routed to `aoa-playbooks` or deferred because it is route-shaped rather than skill-shaped
- one optional `automation_candidate` extract plus an explicit handoff hint to
  `aoa-automation-opportunity-scan`
- one explicit rejected-nearest-wrong-target note for each accepted candidate
- one explicit handoff hint to `aoa-session-route-forks` because the session
  still has more than one honest next route
- one `HARVEST_PACKET_RECEIPT` with bounded counts, owner-layer distribution,
  and evidence-linked candidate refs
- one `CORE_SKILL_APPLICATION_RECEIPT` that records a finished
  `aoa-session-donor-harvest` run and points back to the detail receipt

## Boundary notes

- Do not use this skill while the session is still active.
- Do not use this skill to turn session history directly into memory canon.
- Do not author `aoa-routing` or `aoa-kag` first when the real source-owned object has not been named yet.

## Verification notes

- Confirm the kept units are reusable objects rather than themes.
- Confirm checkpoint and handoff hints were filtered through reviewed evidence
  instead of becoming candidates by presence alone.
- Confirm `candidate_ref` was minted only after reviewed owner-shaping.
- Confirm the chosen owner layer matches the unit shape.
- Confirm each accepted candidate names the next artifact, not only the repo.
- Confirm any post-session family handoff stays explicit.
- Confirm the finish receipt stays smaller than the packet and links evidence
  instead of duplicating raw recap.
- Confirm the generic core receipt stays finish-only and references the detail
  receipt instead of becoming a second donor packet.
