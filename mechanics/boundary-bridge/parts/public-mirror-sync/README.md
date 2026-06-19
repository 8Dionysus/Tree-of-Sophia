# Public Mirror Sync

## Operating Card

| Field | Route |
| --- | --- |
| role | keep public compatibility mirrors aligned with canonical ToS nodes |
| input | canonical source, concept, principle, lineage, event, state, support, analogy, and synthesis nodes |
| output | checked public compatibility mirror payloads |
| owner | `mechanics/boundary-bridge/parts/public-mirror-sync/` |
| stronger route | `ToS/canon/` keeps authored node authority; `ToS/public-compatibility/` keeps public mirror payloads |
| next route | `ToS/public-compatibility/` and the bounded KAG seam when public exports consume the mirrors |
| tools | `mechanics/boundary-bridge/parts/public-mirror-sync/scripts/sync_tree_examples.py`, `mechanics/boundary-bridge/parts/public-mirror-sync/scripts/tree_example_sync.py`, `mechanics/boundary-bridge/parts/public-mirror-sync/scripts/validate_tree_example_sync.py` |
| check | `python mechanics/boundary-bridge/parts/public-mirror-sync/scripts/validate_tree_example_sync.py` |

## Payload

- `scripts/`

The scripts are local because mirror sync is a repeatable bridge operation.
The mirrored JSON payloads stay in `ToS/public-compatibility/`; they are public
compatibility surfaces, not mechanics-owned source truth.
