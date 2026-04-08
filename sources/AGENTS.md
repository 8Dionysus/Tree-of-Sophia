# AGENTS.md

This file applies to primary witness and source files under `sources/`.

## Read first

Before editing witness material, read:
1. the repository root `AGENTS.md`
2. the exact source file you plan to touch
3. `docs/MANUAL_CORPUS_ENTRY_GATE.md`
4. the current route docs if the file participates in the bounded Zarathustra path

## Local role

`sources/` holds raw source-facing witness material that grounds authored ToS routes.

These files may include:
- primary-language witness text
- bounded bridge or working translations
- source-linked donor markdown used to ground authored tree nodes

This directory is where authority is witnessed, not where it is already interpreted into node law.

## Editing posture

Keep files here source-facing, reviewable, and provenance-aware.

Keep these differences explicit:
- witness material in `sources/`
- candidate structure in `intake/`
- canonical authored nodes in `tree/`
- compatibility mirrors in `examples/`

When multilingual witness files are present:
- keep one shared source route rather than parallel language trees
- keep canonical-source versus translation posture explicit
- preserve translator, editor, and donor attribution where it matters
- do not hide witness uncertainty behind smooth merged prose

If you are touching the current Zarathustra route, preserve the witness chain that supports the bounded tiny-entry path.

## Hard no

Do not:
- turn witness files into commentary notes
- encode relation law or node law directly in source text
- let a bridge translation quietly replace the canonical source posture
- add quest, runtime, or orchestration metadata that obscures provenance

## Validation

Use `docs/REVIEW_CHECKLIST.md` for broader review.

If the touched witness participates in the current bounded route or export seam, also run:

```bash
python scripts/validate_tiny_entry_route.py
python scripts/generate_kag_export.py
python scripts/validate_kag_export.py
```
