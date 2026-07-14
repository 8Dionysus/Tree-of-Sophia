# Releasing `Tree-of-Sophia`

`Tree-of-Sophia` is released as a source-first knowledge repository with a bounded public entry route.

See also:

- [README](../README.md)
- [CHANGELOG](../CHANGELOG.md)
- [REVIEW_CHECKLIST](../mechanics/audit/parts/review-ledger-route/docs/REVIEW_CHECKLIST.md)

## Recommended release flow

1. Confirm that no other session is changing the repository, canonical `main`
   is clean, and local/remote `main` agree before preparing a release branch.
2. Reconstruct the release from the complete previous-tag-to-`main`
   first-parent history, changed paths, source/mechanics owners, decisions,
   generated companions, and PR metadata. Do not treat the existing
   `[Unreleased]` prose as a complete inventory.
3. Update `CHANGELOG.md` with a dated section containing `Summary`,
   `Validation`, and `Notes`, and align the release marker in `README.md`,
   `ROADMAP.md`, and release-contour tests.
4. Regenerate owner-required companions and run the repo-level verifier:
   - `python scripts/release_check.py`
5. Land through a PR and GitHub Repo Validation, then rerun the same gate on
   the resulting `main` commit before creating a tag.
6. Run the strict official preflight from a clean workspace whose
   `Tree-of-Sophia` checkout is on `main` and whose dependency roots are the
   intended validation inputs:
   - `aoa release audit <workspace-root> --phase preflight --repo Tree-of-Sophia --strict --json`
7. Publish only through the official helper:
   - `aoa release publish <workspace-root> --repo Tree-of-Sophia --confirm --json`
8. Run strict postpublish audit and verify the remote tag, latest GitHub
   Release, changelog-derived body, and clean synchronized `main`.

## Validation path

`scripts/release_check.py` runs the `release_check` command sequence from
`docs/validation/validation_lanes.json`. That manifest is the command-authority
surface; inventories only describe coverage.

The current bounded route battery covers validation authority, source-home and
mechanics topology, Experience contracts, generated parity, graph exports,
canon contracts, intake contracts, public entry, questbook surface, route-card
structure, decision records, and repo-local tests. Keep the exact command order
in the lane manifest, then run it through `python scripts/release_check.py`.

Repo-local KAG index-family parity remains an explicit GitHub gate. When a
release changes tracked source surfaces, regenerate the seven canonical
indexes with the workflow-pinned `aoa-kag` action revision and verify full,
incremental, and family-contract parity before the final PR commit.
