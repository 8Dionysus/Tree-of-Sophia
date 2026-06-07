# Source Release Gate

## Operating Card

| Field | Route |
| --- | --- |
| role | run release-facing checks through ToS source-home validators |
| input | release change, generated drift, decision drift |
| output | passing release gate or failing owner surface |
| owner | `mechanics/release-support/parts/source-release-gate/` |
| next route | `docs/RELEASING.md`, `scripts/release_check.py`, or failing owner |
| tools | `scripts/release_check.py`, decision index generator |
| check | `python scripts/release_check.py` |
