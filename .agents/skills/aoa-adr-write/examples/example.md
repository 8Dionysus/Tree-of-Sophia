# Example

## Scenario

A team is splitting a monolithic service into a reusable domain package and a thin delivery layer. Several contributors agree on the direction, but future maintainers will need to know why the split was chosen, what alternatives were considered, and what tradeoffs were accepted.

## Why this skill fits

- the change introduces a meaningful architectural decision rather than a tiny local edit
- several plausible paths exist, and the rationale matters for future work
- the value comes from preserving why the choice was made, not only what files changed

## Expected inputs

- the decision to record and the problem it solves
- the main alternatives that were considered
- the chosen path and the rationale behind it
- known tradeoffs, consequences, or follow-up constraints

## Expected outputs

- a concise ADR or decision note
- explicit rationale for the chosen path
- consequence notes that future contributors can review
- a short verification note confirming the ADR matches the real change

## Boundary notes

- this example is about recording a decision after the decision is real enough to matter
- it is not a substitute for clarifying authoritative documentation or resolving document conflicts first

## Verification notes

- verify that the ADR explains why the split was chosen, not only that it happened
- verify that alternatives and tradeoffs are named in plain language
- verify that the final note still matches the actual implementation direction
