# AGENTS.md

## Local role

`.github/` is this repository's GitHub platform surface: workflows, PR templates, issue templates, CODEOWNERS, and repository metadata.

## Read before editing

Read the root `AGENTS.md` first. Root `AGENTS.md` owns repository identity, owner boundaries, the branch/PR/CI/merge route, and the shortest local validation path. This file owns only the GitHub-native files under `.github/`.

Do not encode sibling-repo doctrine, private workspace assumptions, or hidden release behavior here. Do not add secrets, private environment assumptions, or workflow steps that mutate sibling repositories without explicit owner routing. Keep GitHub automation public-safe, deterministic, and weaker than source-owned repository docs. Do not make CI green by weakening the guardrail that should catch drift.

## Platform sync

Keep `.github/CODEOWNERS`, PR templates, and workflow names aligned with the root route card.
`Repo Validation` is the landing check expected by the root GitHub landing workflow. If that check is added, renamed, or its meaning changes, update the root route, PR expectations, and this file in the same change.
Software and explicit data/integration release expectations route to
[`docs/RELEASING.md`](../docs/RELEASING.md). A data-operation registry transition
uses its explicit baseline; software CI does not accept source semantics.

TOS-D-0062 separates software checks from corpus and downstream releases.
Changed paths select software checks through `scripts/software_ci.py` and the
table in `docs/RELEASING.md`. The selector and documentation check must succeed.
Selected jobs must succeed; only explicitly unselected jobs may be skipped.
Unknown/shared paths and manual full-release runs select all software checks.
Absent, failed or cancelled selected jobs cannot be aggregated as success. Do not add a KAG,
stats, complete corpus or generated documentation currentness dependency to
the software gate. Data/integration workflows validate their own exact inputs.

When workflow or repository-policy files change, report:

- GitHub surface touched
- local validation run
- whether `Repo Validation` was added, renamed, skipped, or changed
- remaining platform risk

## Verify

Use the root `AGENTS.md` verification path for the changed surface. For
GitHub-only edits, inspect the workflow YAML and select the nearest route in
[`docs/VALIDATION.md`](../docs/VALIDATION.md) when a repo-local static, release,
or documentation check is needed.
