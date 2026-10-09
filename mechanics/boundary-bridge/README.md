# Boundary Bridge Mechanic

## Mechanic Card

| Field | Route |
| --- | --- |
| status | `active` |
| class | `head-fed/local` |
| trigger | ToS material crosses into KAG, public compatibility, or sibling handoff |
| input | source-owned export, public mirror, derived read model |
| output | bounded bridge seam and owner split |
| owner | `mechanics/boundary-bridge/` |
| stronger route | `ToS/` remains authored truth; sibling repos own their layers |
| next route | [Derived KAG Seam](parts/derived-kag-seam/README.md) or [Public Mirror Sync](parts/public-mirror-sync/README.md) |
| validation | `tos-ops-mechanics-plan --repo-root ROOT --public-mirror-validate`; `tos-ops-mechanics-plan --kag-source-export-verify --kag-export EXPORT` |

## Active Route

- [PARTS](PARTS.md)
- [PROVENANCE](PROVENANCE.md)
- [ROADMAP](ROADMAP.md)
