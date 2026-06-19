# AGENTS.md

This file applies to candidate intake material under `ToS/candidate-intake/`.

## Role

`ToS/candidate-intake/` holds provisional extraction and candidate structure
that may later inform authored ToS work. It is a staging ledge, not canon,
doctrine, or public authority.

## Operating Card

| Field | Route |
| --- | --- |
| role | provisional extraction and candidate structure surface |
| input | source-linked extraction pass, candidate node table, relation table, normalization note, or promotion residue |
| output | reviewable candidate pack with source pointer, pass frame, blocker state, and uncertainty |
| owner | `ToS/candidate-intake/AGENTS.md` and the pass-local README or manifest |
| next route | source witness -> candidate pass -> philosophy branch or canon review -> compatibility/export only after owned surfaces change |
| tools | tabular base contract, relation pack contract, review checklist, route validators |
| check | `scripts/validate_intake_pack.py` when a bounded intake pack changes |

## Boundary Routes

- Keep intake visibly provisional and source-linked.
- Route primary witness authority to `ToS/source-witnesses/`.
- Route reviewed authored nodes and relation packs to `ToS/canon/`.
- Route public examples to `ToS/public-compatibility/` only after an owned
  source or canon surface changes.
- Keep uncertainty in the pack instead of smoothing it into authority.

## Validation

Use `mechanics/audit/parts/review-ledger-route/docs/REVIEW_CHECKLIST.md` for
scope-broadening review. For the current bounded intake pack, use
`scripts/validate_intake_pack.py` and any route/export validator named by the
touched surface.
