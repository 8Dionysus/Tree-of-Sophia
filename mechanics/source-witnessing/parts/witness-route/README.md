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

The implemented bounded acquisition entrypoint is
`scripts/acquire_registry_sources.py`: `verify-preparation` checks the frozen
manifest and retained evidence; `acquire` requires the actual preparation
checkpoint receipt; `verify-local` opens and verifies the exact installed files.
The first operation is retained in
`ToS/source-witnesses/discovery/registry-first-planting-2026-09-08/`, including
its exact plan, per-transfer history, source-scope corrections, canary result
and batch execution record. A supplied provider description can be corrected
against actual payload fields while retaining the earlier observation and bytes.

Larger preparations may bind `metadata_evidence_refs` per target, so each
discovery record carries only its own reviewed source observations. The exact
refs must resolve before installation. Perseus editions with nested books,
chapters or sections use the explicit `hierarchical_divisions` citation scope:
local numbers are qualified by their source-supplied ancestor type/number
pairs. These structural addresses do not assert remote CTS resolution or
critical completeness. The original flat-section checks remain in place for
earlier preparations.

Post-acquisition readiness records use explicit `execution` status.
`completed` requires digest-bound local files and the exact Work/branch planting
record. Completed, deferred and blocked entries remain in history and are not
silently proposed for acquisition again. Absence of execution status preserves
the existing pending/chronological behavior.
