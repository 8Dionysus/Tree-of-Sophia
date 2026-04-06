# Contributing to Tree-of-Sophia

Thank you for contributing.

## What belongs here

Good contributions:
- source-first architecture notes for ToS
- node, lineage, context, and relation model guidance
- docs that clarify provenance, interpretation discipline, and knowledge-layer boundaries
- public scaffold surfaces that remain visibly reviewable and source-linked

Bad contributions:
- runtime or deployment implementation detail
- agent orchestration surfaces that belong to AoA
- routing-only datasets with no authored ToS meaning
- derived substrate material presented as if it were the source of truth
- flattened summaries that erase lineage, context, or interpretive uncertainty

## Before opening a PR

Please make sure:
- source, extraction, interpretation, and synthesis remain distinguishable
- lineage and context stay explicit where they matter
- authored meaning remains distinct from derived structure
- uncertainty is named honestly when the material is contested or interpretive
- public additions remain provenance-aware and reviewable

For the current bounded route, run `python scripts/validate_kag_export.py` and `python -m unittest discover -s tests`.
When the touched surface falls outside that validator coverage, use `docs/REVIEW_CHECKLIST.md` as the manual validation route and include a short review note in the PR when the change is boundary-sensitive.

## Preferred PR scope

Prefer:
- 1 focused architecture or doctrine change per PR
- or 1 focused scaffold or review-surface addition
- or 1 focused clarification of source-discipline rules

## Review criteria

PRs are reviewed for:
- provenance clarity
- layered meaning discipline
- lineage preservation
- boundary discipline between ToS and neighboring repositories
- restraint and public safety

## Security

Do not use public issues or pull requests for leaks, credentials, or sensitive unpublished material.
Use the process in `SECURITY.md`.
