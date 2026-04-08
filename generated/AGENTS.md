# AGENTS.md

This file applies to derived export artifacts under `generated/`.

## Read first

Before touching anything here, read:
1. the repository root `AGENTS.md`
2. `docs/KAG_EXPORT.md`
3. `examples/AGENTS.md`
4. `tree/AGENTS.md`
5. the generator or validator scripts that produce the file

## Local role

`generated/` contains the current tiny KAG export surfaces for Tree of Sophia.

These files are:
- derived guide artifacts for downstream consumers
- bounded transport surfaces for the current route
- explicitly subordinate to authored ToS authority

They are not authored law.

## Editing posture

Do not hand-edit derived payloads as the normal workflow.

Change the source-owned inputs or generation logic, then regenerate.

Keep the pilot narrow to the current Zarathustra route:
- preserve `tos.source.thus-spoke-zarathustra.prologue`
- keep `entry_surface` aligned with the current example and canonical tree mirror
- keep `section_handles` aligned with source `interpretation_layers`
- keep `non_identity_boundary` explicit so downstream consumers do not mistake export for authorship

Do not widen the export envelope just because downstream tooling could ingest more. A larger export that weakens provenance is a paper crown.

## Hard no

Do not:
- treat `generated/` as a hand-maintained data layer
- slip AoA routing or control-plane semantics into ToS export by convenience
- let quest or progression fields become hidden runtime state here
- present a derived field as a stronger authority than its source-owned input

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
