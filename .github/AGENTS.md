# GitHub platform route

`.github/` owns workflows, templates, CODEOWNERS and repository metadata.
Keep automation public-safe and deterministic; preserve source-owned contracts
and the checks that detect drift. Align workflow names, CODEOWNERS and PR
expectations with the root route.

## Validation and landing

[Repo Validation](workflows/repo-validation.yml) selects jobs from changed
paths through `tos-software-ci`. The selector, AGENTS word-limit check and each
selected job must succeed. Unknown or shared paths select the full software
checks; manual full-release runs select all checks. ToS source Markdown and
Questbook inputs retain their Rust consumers.

Rust selection includes the conformance, workspace and cold-install matrix
groups plus PostgreSQL. Software selection includes preparation, fixture checks
and package/browser checks. The required gate aggregates selected jobs and
verifies candidate digests before publishing the qualified artifact.

[Releasing](../docs/RELEASING.md) owns the complete job matrix, source-CI reuse,
exact-SHA release qualification and separate corpus and integration routes.
Update that guide and PR expectations when a workflow or required check changes.
Inspect the affected workflow and run the selected repository validation.
Report the GitHub surface, local checks, required-check changes and remaining
platform risk.
