# Review Checklist

## Purpose

Use this checklist when reviewing a non-trivial change that claims to follow `aoa-change-protocol`.

## When it applies

- the change touches code, config, docs, or operational guidance in a meaningful way
- the author claims the work stayed bounded and explicitly verified
- the review needs to confirm that the workflow was visible rather than implicit
- the change relies on route cards, source-of-truth surfaces, generated exports, mechanics, legacy/provenance bridges, or sibling-owner boundaries

## Review checklist

- [ ] The owner route and relevant source-of-truth surfaces are inspected before the plan is treated as stable.
- [ ] The goal, touched surfaces, and inspected evidence are named before the change is applied.
- [ ] The main risk, owner boundary, and rollback or recovery thinking exist before execution.
- [ ] The diff stays inside the declared scope and avoids unrelated cleanup.
- [ ] Generated, exported, compact, or derived surfaces are rebuilt from source when canonical inputs changed.
- [ ] At least one explicit verification step was run, or a clear reason is given for intentionally skipping it.
- [ ] The post-change route review touched only surfaces whose meaning moved.
- [ ] The final report names what changed, what was verified, what was skipped, and what remains risky or deferred.

## Not a fit

- tiny wording or formatting edits with no meaningful review or operational consequence
- tasks where a more specific risk skill should own the workflow
