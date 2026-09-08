# Tree-of-Sophia Local KAG Provider

`kag/` exposes the current Tree-of-Sophia KAG provider packet as portable
source-linked records.

## Operating Card

| Field | Route |
| --- | --- |
| role | local KAG provider for ToS-derived export surfaces |
| records | `nodes/`, `edges/`, `indexes/`, `projections/`, `receipts/` |
| manifest | `manifest.json` |
| provider pin | `provider-pin.json` |
| source route | `ToS/derived-exports/` and the derived KAG seam mechanic |
| consumer route | `aoa-kag` registry/composition, `abyss-stack`, MCP resources |
| owner return | `ToS/derived-exports/README.md` and `mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md` |

## Record Classes

| Class | Current record |
| --- | --- |
| node | KAG export capsule and derived export route |
| edge | source export returns to the derived export route |
| index | repository source, entity, artifact, and event indexes |
| projection | MCP-readable source-return packet |
| receipt | validator receipt for the current export seam |

Runtime graph and vector stores consume these records downstream through their
own owner routes. Git holds the compact provider packet and source-return
handles. The generated family may use the pinned `aoa-repo-local-kag-segmented-family-v1`
carrier when the complete corpus exceeds the legacy v3/v4 materialisation
ceiling; the local adapter validates one bounded segment at a time and does
not silently assemble the full compatibility view.
