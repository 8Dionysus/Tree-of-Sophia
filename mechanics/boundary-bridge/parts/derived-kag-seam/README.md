# Derived KAG Seam

## Operating Card

| Field | Route |
| --- | --- |
| role | keep ToS-to-KAG handoff derived and bounded |
| input | source node, public mirror, derived export |
| output | checked downstream read model |
| owner | `mechanics/boundary-bridge/parts/derived-kag-seam/` |
| next route | `ToS/derived-exports/` or sibling KAG owner |
| tools | `mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md`, `mechanics/boundary-bridge/parts/derived-kag-seam/scripts/validate_kag_export.py`, `mechanics/boundary-bridge/parts/derived-kag-seam/scripts/prepare_tree_kag_owner_pair.py` |
| check | `python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/validate_kag_export.py` |
