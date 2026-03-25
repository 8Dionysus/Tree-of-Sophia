# AGENTS.md

This file applies to derived export artifacts under `generated/`.

## What lives here

`generated/` contains the current tiny KAG export surfaces for Tree of Sophia.
These files are derived guide artifacts for downstream consumers.
They do not replace authored ToS authority.

The current generated set is:

- `kag_export.json`
- `kag_export.min.json`

## Editing posture

Do not hand-edit derived payloads here as the normal workflow.
Change the source-owned inputs or generation logic, then regenerate the outputs.

Keep the current pilot bounded to `tos.source.thus-spoke-zarathustra.prologue`.
Keep `entry_surface` aligned with `examples/source_node.example.json`.
Keep `section_handles` aligned with source `interpretation_layers`.
Keep `non_identity_boundary` explicit so downstream KAG consumers do not mistake this export for ToS-authored authority.

## Validation

Run:

```bash
python scripts/generate_kag_export.py
python scripts/validate_kag_export.py
```
