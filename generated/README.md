# ToS Generated

This directory holds derived export surfaces for bounded downstream consumption.

Generated files here do not replace ToS-authored authority.
Canonical authority remains in `tree/`, while the public entry mirrors remain in `examples/`.

## Current role

Use `generated/` when you need:

- a compact downstream-safe export of the current bounded route
- a reviewable derived payload for KAG-oriented consumers

Do not treat `generated/` as:

- the primary authored home of the route
- a replacement for the canonical tree node
- an excuse to skip the source-owned capsule and tiny-entry docs

## Current bounded export

The current public export surfaces are:

- `kag_export.json`
- `kag_export.min.json`

They summarize the current Zarathustra route for downstream consumers while pointing back to ToS-owned authority and compatibility surfaces.

## How to verify

Use:

- `docs/KAG_EXPORT.md`
- `docs/TINY_ENTRY_ROUTE.md`
- `examples/source_node.example.json`
- `python scripts/validate_kag_export.py`
