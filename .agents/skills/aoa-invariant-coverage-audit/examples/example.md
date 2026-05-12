# Example

## Scenario

A validation suite and generated report already cover a few happy-path and error-path cases for a bounded rule, but reviewers still are not sure whether the checks really constrain the stable truth or only repeat examples and complete-looking status.

## Why this skill fits

- the task is to audit existing invariant coverage, not to discover a brand-new invariant from scratch
- the current question is whether the suite and report prove the stable rule across meaningful cases
- the output should stay bounded to coverage mapping and the smallest next gaps, not widen into full test-strategy redesign

## Expected inputs

- the stable rule that should hold, such as "accepted input preserves required fields and rejected input fails before side effects"
- the current tests, checks, report fields, generated parity checks, or example cases
- known edge cases that already matter to reviewers
- any bounds needed to keep the audit reviewable

## Expected outputs

- one plain-language statement of the invariant under review
- a map from the invariant to the checks or report fields that currently constrain it
- a short gap list for weak or missing coverage
- the smallest bounded follow-up checks or revisions worth adding next
- a claim-limit note that prevents the report from being read as broader proof

## Boundary notes

- do not use this skill when the invariant itself is still unknown and discovery work must happen first
- do not widen the audit into generic boundary-contract design when `aoa-contract-test` is the better fit
- do not treat generated freshness or report completeness as proof unless it constrains the invariant itself

## Verification notes

- verify that each claimed invariant is mapped to a real existing check
- verify that the reported gaps are specific and actionable rather than abstract testing advice
- verify that the result states what the current suite or report proves and what still remains outside the bounded audit
