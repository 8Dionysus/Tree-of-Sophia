# Releasing `Tree-of-Sophia`

`Tree-of-Sophia` is released as a source-first knowledge repository with a bounded public entry route.

See also:

- [README](../README.md)
- [CHANGELOG](../CHANGELOG.md)
- [REVIEW_CHECKLIST](../mechanics/audit/parts/review-ledger-route/docs/REVIEW_CHECKLIST.md)

## Recommended release flow

1. Keep the release bounded to source-owned ToS meaning.
2. Update `CHANGELOG.md` so the latest tagged section keeps `Summary`, `Validation`, and `Notes`.
3. Run the repo-level verifier:
   - `python scripts/release_check.py`
4. Run federation release audit when the change is part of a wider owner-repo pass:
   - `aoa release audit /srv --phase preflight --repo Tree-of-Sophia --strict --json`
5. Publish only through `aoa release publish`.

## Validation path

`scripts/release_check.py` runs the `release_check` command sequence from
`docs/validation/validation_lanes.json`. That manifest is the command-authority
surface; inventories only describe coverage.

The current bounded route battery covers validation authority, source-home and
mechanics topology, generated parity, canon contracts, intake contracts, public
entry, questbook surface, route-card structure, decision records, and repo-local
tests. Keep the exact command order in the lane manifest, then run it through
`python scripts/release_check.py`.
