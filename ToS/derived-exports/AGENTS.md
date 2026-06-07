# AGENTS.md

This file applies to derived export artifacts under `ToS/derived-exports/`.

## Read first

Before touching anything here, read:
1. the repository root `AGENTS.md`
2. `../doctrine/KAG_EXPORT.md`
3. `../public-compatibility/AGENTS.md`
4. `../canon/AGENTS.md`
5. the generator or validator scripts that produce the file

## Local role

`ToS/derived-exports/` contains the current tiny KAG export surfaces for Tree of Sophia.

These files are:
- derived guide artifacts for downstream consumers
- bounded transport surfaces for the current route
- explicitly subordinate to authored ToS authority

Authored law stays in source-owned ToS surfaces; this branch publishes derived
read models.

## Operating Card

| Field | Route |
| --- | --- |
| role | generated downstream-facing read model surface |
| input | owned canon, compatibility examples, contracts, and generator logic |
| output | generated export payloads and compact read models |
| owner | `ToS/derived-exports/AGENTS.md` for route law; generator scripts for payload construction |
| next route | source-owned input or generator -> regenerate -> validate export |
| tools | `scripts/generate_kag_export.py`, `scripts/validate_kag_export.py`, source validators |
| check | generated parity and export validation |

## Editing posture

Change the source-owned inputs or generation logic, then regenerate the
derived payloads.

Keep the pilot narrow to the current Zarathustra route:
- preserve `tos.source.thus-spoke-zarathustra.prologue`
- keep `entry_surface` aligned with the current example and canonical tree mirror
- keep `section_handles` aligned with source `interpretation_layers`
- keep `non_identity_boundary` explicit so downstream consumers do not mistake export for authorship

Widen the export envelope only when the source-owned route, contract, and
review posture have widened first.

## Boundary Routes

- Hand-maintained data routes to source-owned inputs, contracts, or generator
  logic before the export changes.
- AoA routing and control-plane semantics route to their owning repositories.
- Runtime state routes to runtime owner surfaces, not derived ToS exports.
- Derived fields point back to the source-owned input that gives them meaning.

## Validation

Run:

```bash
python scripts/validate_tree_example_sync.py
python scripts/generate_kag_export.py
python scripts/validate_kag_export.py
```

If the touched export depends on the current tiny-entry route, also run:

```bash
python scripts/validate_tiny_entry_route.py
```
