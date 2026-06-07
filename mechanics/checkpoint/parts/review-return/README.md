# Review Return

## Operating Card

| Field | Route |
| --- | --- |
| role | preserve where review or branch work should resume |
| input | partial review, paused branch, validator finding |
| output | return route and owner surface |
| owner | `mechanics/checkpoint/parts/review-return/` |
| next route | `ToS/review-ledger/`, `ToS/philosophy/`, or owning validator |
| tools | review ledger and source-home manifest |
| check | `python scripts/validate_tos_source_home.py` |
