# Review Checklist

## Purpose

Use this checklist when reviewing work that claims to bring up a bounded local multi-service stack through render review, readiness checks, and one explicit lifecycle path.

## When it applies

- the task starts a local stack with more than one meaningful service or runtime layer
- the selected runtime shape can change what will actually start
- readiness should be checked before launch rather than after startup failure
- the review needs to confirm that startup stayed explicit, bounded, and locally reviewable

## Review checklist

- [ ] Rendered runtime truth was reviewed before any startup step.
- [ ] The readiness verdict is selector-aware and names item-level `ok`, `warn`, or `fail` results.
- [ ] Blockers or unresolved ambiguity stopped the launch path or were explicitly deferred.
- [ ] Operator intent or confirmation is visible before the mutating launch step.
- [ ] Startup status names what started and how to stop it.
- [ ] Any full rendered config stayed local and controlled rather than becoming a share artifact.

## Not a fit

- remote deployment, continuous monitoring, or fleet orchestration work
- generic bootstrap or install flows that do not stay inside a bounded local runtime bring-up
