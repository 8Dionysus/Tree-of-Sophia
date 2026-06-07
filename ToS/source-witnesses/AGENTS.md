# AGENTS.md

This file applies to primary witness and source files under `ToS/source-witnesses/`.

## Read first

Before editing witness material, read:
1. the repository root `AGENTS.md`
2. the exact source file you plan to touch
3. `mechanics/source-witnessing/parts/witness-route/docs/MANUAL_CORPUS_ENTRY_GATE.md`
4. the current route docs if the file participates in the bounded Zarathustra path

## Local role

`ToS/source-witnesses/` holds raw source-facing witness material that grounds authored ToS routes.

These files may include:
- primary-language witness text
- bounded bridge or working translations
- source-linked donor markdown used to ground authored tree nodes

This directory witnesses authority and routes later interpretation toward the
owning doctrine, candidate, philosophy, or canon surface.

## Operating Card

| Field | Route |
| --- | --- |
| role | primary witness and source-facing evidence surface |
| input | primary-language text, bridge translation, donor markdown, source-page metadata, and provenance notes |
| output | reviewable witness surface with explicit source posture |
| owner | `ToS/source-witnesses/AGENTS.md` and the nearest source-route file |
| next route | source witness -> `ToS/philosophy/` branch or `ToS/candidate-intake/` pass -> `ToS/canon/` review |
| tools | manual corpus gate, source route docs, witness manifests |
| check | route validator when the witness feeds a current public or export surface |

## Editing posture

Keep files here source-facing, reviewable, and provenance-aware.

Keep these differences explicit:
- witness material in `ToS/source-witnesses/`
- domain philosophy branch material in `ToS/philosophy/`
- candidate structure in `ToS/candidate-intake/`
- canonical authored nodes in `ToS/canon/`
- compatibility mirrors in `ToS/public-compatibility/`

When multilingual witness files are present:
- keep one shared source route rather than parallel language trees
- keep canonical-source versus translation posture explicit
- preserve translator, editor, and donor attribution where it matters
- keep witness uncertainty visible instead of smoothing it into merged prose

If you are touching the current Zarathustra route, preserve the witness chain that supports the bounded tiny-entry path.

## Boundary Routes

- Commentary routes to doctrine, review, or canon surfaces according to its
  owner; witness files keep source posture and provenance.
- Relation law and node law route to `ToS/canon/` and `ToS/doctrine/`.
- Bridge translations stay visibly marked as bridge material rather than
  replacing canonical source posture.
- Operational metadata routes to its owning compatibility, review, or sibling
  repo surface when it is needed.

## Validation

Use `mechanics/audit/parts/review-ledger-route/docs/REVIEW_CHECKLIST.md` for broader review.

If the touched witness participates in the current bounded route or export seam, also run:

```bash
python scripts/validate_tiny_entry_route.py
python scripts/generate_kag_export.py
python scripts/validate_kag_export.py
```
