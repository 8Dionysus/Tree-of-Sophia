# ToS Generated

This directory holds derived export surfaces for bounded downstream consumption.

Generated files here do not replace ToS-authored authority.
Canonical authority remains in `../canon/`, while the public entry mirrors remain in `../public-compatibility/`.

## Current role

Use `ToS/derived-exports/` when you need:

- a compact downstream-safe export of the current bounded route
- a reviewable derived payload for KAG-oriented consumers
- a checked whole-corpus index for runtime graph, UI, and MCP access planes

Do not treat `ToS/derived-exports/` as:

- the primary authored home of the route
- a replacement for the canonical tree node
- an excuse to skip the source-owned capsule and tiny-entry docs
- a runtime projection store or graph UI authority

## Current bounded export

The current public export surfaces are:

- `kag_export.json`
- `kag_export.min.json`
- `root_entry_map.min.json`
- `tos_corpus_index.min.json`
- `philosophy_atlas_projection.min.json`

They summarize the current Zarathustra route for downstream consumers while pointing back to ToS-owned authority and compatibility surfaces.
The root entry map is the machine-facing entry capsule for consumers that need
schema-checked root-route orientation before touching downstream exports.
The corpus index covers the whole `ToS/` home as a derived resource map so
`abyss-stack` can project and visualize the corpus without owning ToS meaning.
The philosophy atlas projection turns `ToS/philosophy/atlas/` into a first
reviewable tree/graph read model for visualization and graph switching.
The current OS Abyss artifact bundle posture for these JSON readmodels is
ABI-only and verified through `mechanics/release-support/parts/artifact-bundles/`.

## How to verify

Use:

- `../../mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md`
- `../zarathustra/public-entry/TINY_ENTRY_ROUTE.md`
- `../public-compatibility/source_node.example.json`
- `python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/validate_kag_export.py`
- `python scripts/build_root_entry_map.py --check`
- `python scripts/validate_root_entry_map.py`
- `python scripts/build_tos_corpus_index.py --check`
- `python scripts/validate_tos_corpus_index.py`
- `python scripts/build_philosophy_atlas_projection.py --check`
- `python scripts/validate_philosophy_atlas_projection.py`
- `python mechanics/release-support/parts/artifact-bundles/scripts/validate_abyss_machine_generated_readmodel_bundle.py`
