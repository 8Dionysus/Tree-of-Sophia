# Derived KAG Seam

## Operating Card

| Field | Route |
| --- | --- |
| role | keep ToS-to-KAG handoff derived and bounded |
| input | source node, public mirror, derived export |
| output | checked downstream read model |
| owner | `mechanics/boundary-bridge/parts/derived-kag-seam/` |
| next route | `ToS/derived-exports/` or sibling KAG owner |
| tools | `mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md`, `scripts/build_kag_export.py`, `scripts/publish_kag_release.py` |
| check | `tos-kag-release export-verify --release EXPORT`; `tos-kag-release status --release-root RELEASE_ROOT --expected-revision REVISION` |
