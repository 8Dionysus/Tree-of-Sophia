# Derived KAG Seam

## Operating Card

| Field | Route |
| --- | --- |
| role | keep ToS-to-KAG handoff derived and bounded |
| input | source node, public mirror, derived export |
| output | checked downstream read model |
| owner | `mechanics/boundary-bridge/parts/derived-kag-seam/` |
| next route | `ToS/derived-exports/` or sibling KAG owner |
| tools | `ToS/doctrine/KAG_EXPORT.md`, `scripts/validate_kag_export.py` |
| check | `python scripts/validate_kag_export.py` |
