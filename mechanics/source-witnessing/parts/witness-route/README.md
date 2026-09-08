# Witness Route

## Operating Card

| Field | Route |
| --- | --- |
| role | keep source witnesses distinct before branch or canon movement |
| input | primary witness, published source metadata, translation layer, evidence status |
| output | witness route, candidate route, or branch route |
| owner | `mechanics/source-witnessing/parts/witness-route/` |
| next route | `ToS/source-witnesses/`, `ToS/philosophy/`, or `ToS/candidate-intake/` |
| tools | source-home manifest and witness manifests |
| check | `python scripts/validate_tos_source_home.py` |

## Registry-to-source operation

Research originals and reported normalized values belong to
`ToS/research-packets/source-registries/`. The field adapter in
`config/registry-normalization.v1.json` and the `normalize_source_registries.py`
entrypoint preserve complete raw/source trace; they never clear rights or admit
identity. `inspect_source_registry.py` opens a scoped record or report, and
`build_source_registry_reconciliation.py` exposes possible current owner matches.

Before acquisition, review exact versions, live access, intended-use rights,
local presence and branch anchors independently. Use the existing discovery
queue's readiness mode to freeze a source-bound plan, preserving chronological
selection and historical receipts. `prepare_philosophy_source_planting.py`
resolves actual branch/backlog anchors without creating planted records.

Actual acquisition retains upstream bytes, checks their pinned identity and
content structure, records per-file fixity and rights, and links the exact
source to its branch. The preparation checkpoint is evidence of the preceding
work; it does not create new permission. Operator scope and source rights remain
with their actual owners. Items and Composite representations use the same
local ignored payload route with tracked metadata. A file must be opened and
verified locally before a batch result claims it is present.

The ordered checks remain in `docs/validation/validation_lanes.json` under
`source_witness_foundation` and the affected philosophy route. A green import,
queue or resource inventory does not accept source text, translation, semantics,
canon, public transfer or deployment.
