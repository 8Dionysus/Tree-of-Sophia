# Boundary Bridge Mechanic

## Mechanic Card

| Field | Route |
| --- | --- |
| status | `planted` |
| class | `head-fed/local` |
| trigger | ToS material crosses into KAG, public compatibility, or sibling handoff |
| input | source-owned export, public mirror, derived read model |
| output | bounded bridge seam and owner split |
| owner | `mechanics/boundary-bridge/` |
| stronger route | `ToS/` remains authored truth; sibling repos own their layers |
| next route | [Derived KAG Seam](parts/derived-kag-seam/README.md) or [Public Mirror Sync](parts/public-mirror-sync/README.md) |
| validation | `python mechanics/boundary-bridge/parts/public-mirror-sync/scripts/validate_tree_example_sync.py`; `python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/validate_kag_export.py` |

## Active Route

- [PARTS](PARTS.md)
- [PROVENANCE](PROVENANCE.md)
- [ROADMAP](ROADMAP.md)
