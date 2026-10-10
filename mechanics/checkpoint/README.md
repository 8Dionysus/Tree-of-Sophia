# Checkpoint Mechanic

## Mechanic Card

| Field | Route |
| --- | --- |
| status | `planted` |
| class | `head-fed/local` |
| trigger | review state must survive a return, pause, or handoff |
| input | review note, partial branch state, validator result |
| output | return route and owner surface |
| owner | `mechanics/checkpoint/` |
| stronger route | `ToS/` owns source state; memo repos own memory |
| next route | [Review Return](parts/review-return/README.md) |
| validation | `tos-ops-mechanics-plan --repo-root ROOT --mechanics-topology-validate` |

## Active Route

- [PARTS](PARTS.md)
- [PROVENANCE](PROVENANCE.md)
- [ROADMAP](ROADMAP.md)
