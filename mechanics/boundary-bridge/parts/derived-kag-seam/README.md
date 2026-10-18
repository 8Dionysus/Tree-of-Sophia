# Derived KAG Seam

## Operating Card

| Field | Route |
| --- | --- |
| role | keep ToS-to-KAG handoff derived and bounded |
| input | source node, public mirror, derived export |
| output | checked downstream read model |
| owner | `mechanics/boundary-bridge/parts/derived-kag-seam/` |
| next route | `ToS/derived-exports/` or sibling KAG owner |
| tools | `mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md`, `rust/crates/tos-ops-mechanics-plan/src/kag_corpus_export.rs` |
| check | `tos-kag-release export-verify --release EXPORT` |

The bounded ToS export is selected explicitly. It requires no downstream
repository or runtime and does not gate public-mirror parity or source review.
Consumers own their imports.
