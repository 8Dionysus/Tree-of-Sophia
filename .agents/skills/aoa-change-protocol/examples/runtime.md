# Runtime Example

## Scenario

Update a public README route and a validator message after a root source-of-truth contract changes in the repository.

## Why this skill fits

This is a non-trivial change because it touches documentation, validation-facing text, and a route contract. It benefits from reading the owner route first, then making scoped edits and running a clear verification step.

## Expected inputs

- target goal
- owner route and relevant `AGENTS.md` or source-of-truth surfaces
- touched files or surfaces
- the main risk of the change
- the validation path to run after editing

## Expected outputs

- a small, reviewable diff
- a short verification note
- a post-change route review note for any roadmap, changelog, generated, decision, quest, or mechanics surface whose meaning moved
- a concise final report that names what changed and what was checked

## Boundary notes

- Do not use this skill for a tiny typo fix with no meaningful review consequence.
- If the main question is whether the task is allowed at all, use the approval-gate skill first.
- If the main risk is operational rollback or preview behavior, use the dry-run-first skill instead.
- If the change depends on legacy or provenance material, start from the active route and open legacy only when lineage must be audited.

## Verification notes

- Confirm the owner route and relevant source surfaces were read before editing.
- Confirm the diff stayed inside the declared scope.
- Confirm at least one explicit check was run or intentionally skipped with a reason.
- Confirm derived or export surfaces were rebuilt when canonical inputs changed.
- Confirm the final report names the outcome, skipped checks, and any remaining owner-route risk.
