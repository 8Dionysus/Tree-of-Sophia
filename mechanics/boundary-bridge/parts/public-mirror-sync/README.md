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
| tools | `tos-ops-mechanics-plan --repo-root ROOT --public-mirror-sync`, `tos-ops-mechanics-plan --repo-root ROOT --public-mirror-sync`, `tos-ops-mechanics-plan --repo-root ROOT --public-mirror-validate` |
| check | `tos-ops-mechanics-plan --repo-root . --public-mirror-validate` |

## Implementation

The native owner is `rust/crates/tos-ops-mechanics-plan/src/public_mirror.rs`.
Use `--public-mirror-sync` to write the declared mirrors and
`--public-mirror-validate` for a read-only check, both with `--repo-root ROOT`.
The mirrored JSON payloads stay in `ToS/public-compatibility/`; their authored
source authority remains with the canonical nodes.
