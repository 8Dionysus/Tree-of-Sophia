# Runtime Example

## Scenario

A reviewed session has already produced a donor packet, but three honest next
routes remain:
1. deepen the donor-harvest nucleus
2. add a new sibling skill for route choice
3. defer the family split and keep the work as one bounded owner-repo change

## Why this skill fits

The surviving problem is not donor extraction anymore.
It is explicit branch analysis across several plausible next routes with
different gains, costs, and risks.

## Expected inputs

- reviewed session artifact or harvest packet
- named next-route candidates
- known risks and dependencies
- desired control mode or operator preference if already known

## Expected outputs

- `FORK_CARDS` for each materially different route
- one likely first owner repo for each branch
- one suggested default route if the evidence is strong enough
- one hold or defer option if uncertainty remains meaningful
- one `DECISION_FORK_RECEIPT` with branch ids, risk posture, and stop-condition refs
- one `CORE_SKILL_APPLICATION_RECEIPT` that records the finished
  `aoa-session-route-forks` run and points back to the detail receipt

## Boundary notes

- Do not use this skill before donor harvest exists.
- Do not use this skill as hidden routing policy.
- Do not launch a child route from the cards; once one branch is selected,
  anchored, and output-named, use `aoa-summon`.
- Do not confuse branch analysis with final promotion triage.
- Treat historical fixture suffixes such as wave labels as lineage only. Skill
  output should use stable names like `FORK_CARDS`,
  `DECISION_FORK_RECEIPT`, and `CORE_SKILL_APPLICATION_RECEIPT`.

## Verification notes

- Confirm each branch is materially distinct.
- Confirm costs, risks, and stop conditions are visible.
- Confirm the cards preserve choice rather than erase alternatives.
- Confirm the finish receipt stays descriptive rather than pretending the route
  is already chosen by policy.
- Confirm the generic core receipt stays separate from route authority and
  references the detail receipt.
