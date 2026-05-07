# AGENTS.md

## Guidance for `.github/`

`.github/` is this repository's GitHub platform surface: workflows, PR templates, issue templates, CODEOWNERS, and repository metadata.

Read the root `AGENTS.md` first. Root `AGENTS.md` owns repository identity, owner boundaries, the branch/PR/CI/merge route, and the shortest local validation path. This file owns only the GitHub-native files under `.github/`.

Do not encode sibling-repo doctrine, private workspace assumptions, or hidden release behavior here. Do not add secrets, private environment assumptions, or workflow steps that mutate sibling repositories without explicit owner routing. Keep GitHub automation public-safe, deterministic, and weaker than source-owned repository docs. Do not make CI green by weakening the guardrail that should catch drift.

## Platform sync

Keep `.github/CODEOWNERS`, PR templates, and workflow names aligned with the root route card.
`Repo Validation` is the landing check expected by the root GitHub landing workflow. If that check is added, renamed, or its meaning changes, update the root route, PR expectations, and this file in the same change.

When workflow or repository-policy files change, report:

- GitHub surface touched
- local validation run
- whether `Repo Validation` was added, renamed, skipped, or changed
- remaining platform risk

## Verify

Use the root `AGENTS.md` verification path for the changed surface. For GitHub-only edits, inspect the workflow YAML and run the nearest repo-local static, release, or validation check when available.
