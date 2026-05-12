# Example

## Scenario

A generated catalog builder turns authored source bundles into compact indexes, installed copies, and route hints. The team has a few example fixtures, but regressions still appear because stable IDs, source references, eligibility filters, and rebuild repeatability need to hold across many source sets and profile options.

## Why this skill fits

- the system has stable truths that should hold across many artifacts, transformations, and profile states
- example-only testing is too narrow for the risk surface
- the important value comes from expressing source-to-generated invariants, not from enumerating a few happy paths

## Expected inputs

- the invariant candidates, such as "eligible sources appear once", "source refs are preserved", and "same source plus same options rebuilds equivalently"
- the current example tests, fixtures, and known edge cases
- the generator or input strategy for source sets, eligibility states, and profile options
- limits needed to keep the property checks reviewable

## Expected outputs

- explicit invariants for the source-to-generated rules
- property-oriented tests or repeated checks over bounded source sets and profile options
- notes on generator assumptions, excluded profile behavior, and what the properties do not cover

## Boundary notes

- this example is not about UI rendering or snapshot-style presentation behavior
- the point is to encode stable truths, not to replace every concrete example test
- if the main question is only whether current checks already cover this rule, use `aoa-invariant-coverage-audit` first

## Verification notes

- verify that each property expresses a real invariant instead of a weak wish
- verify that the generated cases broaden coverage beyond the original handpicked examples
- verify that the report explains the invariant and the bounds of the generator strategy
- verify that generated surfaces remain derived evidence rather than source authority
