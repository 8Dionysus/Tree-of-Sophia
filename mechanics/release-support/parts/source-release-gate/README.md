# Source Release Gate

## Operating Card

| Field | Route |
| --- | --- |
| role | run release-facing checks through ToS source-home validators |
| input | release change, generated drift, decision drift |
| output | passing release gate or failing owner surface |
| owner | `mechanics/release-support/parts/source-release-gate/` |
| next route | `docs/RELEASING.md`, `tos-release-check`, or failing owner |
| tools | `tos-release-check`, decision index generator |
| check | `tos-release-check --repo-root "$PWD"` |
