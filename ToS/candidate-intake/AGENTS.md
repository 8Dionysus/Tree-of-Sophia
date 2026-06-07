# AGENTS.md

This file applies to candidate intake material under `ToS/candidate-intake/`.

## Read first

Before editing intake material, read:
1. the repository root `AGENTS.md`
2. `mechanics/source-witnessing/parts/witness-route/docs/MANUAL_CORPUS_ENTRY_GATE.md`
3. `ToS/doctrine/TABULAR_BASE_CONTRACT.md` and `ToS/doctrine/RELATION_PACK_CONTRACT.md` when applicable
4. the exact source witness or source route the intake pass depends on
5. `mechanics/audit/parts/review-ledger-route/docs/REVIEW_CHECKLIST.md` if the change broadens scope

## Local role

`ToS/candidate-intake/` holds candidate structure that may later inform authored ToS work.

Typical contents include:
- candidate node tables
- candidate relation tables
- bounded intake packs
- pass-specific notes about extraction or normalization work

This directory is a staging ledge for material that still needs review before
it can become authored tree law.

## Operating Card

| Field | Route |
| --- | --- |
| role | provisional extraction and candidate structure surface |
| input | source-linked extraction pass, candidate node table, relation table, normalization note, or promotion residue |
| output | reviewable candidate pack with source pointer, pass frame, blocker state, and uncertainty |
| owner | `ToS/candidate-intake/AGENTS.md` and the pass-local README or manifest |
| next route | source witness -> candidate pass -> branch workbench or canon review -> compatibility/export only after owned surfaces change |
| tools | tabular base contract, relation pack contract, review checklist, route validators |
| check | `python scripts/validate_intake_pack.py` when a bounded intake pack changes |

## Editing posture

Keep `ToS/candidate-intake/` visibly provisional.

Material here routes toward:
- primary witness authority in `ToS/source-witnesses/`
- canonical tree law in `ToS/canon/`
- public route examples in `ToS/public-compatibility/`
- deterministic tooling in `scripts/` when automation is needed

Every intake change should preserve a clear trail back to:
- the source witness
- the pass or extraction frame
- the uncertainty that still remains
- the authored surfaces that do **not** yet exist

Keep early intake light and route-oriented. Do not inflate one pass into a pseudo-platform before the source-facing use case demands it.

Quest or progression language may appear here only as bounded work-tracking or compatibility support. It must not redefine source-linked meaning.

## Boundary Routes

- Authored nodes route to `ToS/canon/`; candidate tables keep pass-local
  extraction posture until reviewed.
- Broad backlog or planning material routes to the owning roadmap, review, or
  decision surface.
- Interpretive uncertainty stays visible in the candidate pack.
- Canonical relation law routes through reviewed canon relation packs and
  registry checks.

## Validation

Use `mechanics/audit/parts/review-ledger-route/docs/REVIEW_CHECKLIST.md` for broader review.

If you change a bounded intake pack or a surface consumed by the current export seam, also run:

```bash
python scripts/validate_intake_pack.py
python scripts/validate_tiny_entry_route.py
python scripts/validate_kag_export.py
```
