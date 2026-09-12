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

An additional language version may extend an existing Work only through a
prepared `existing_work` binding to its retained exact preimage and SHA-256.
The acquisition helper permits only the new Expression claim refs and the
Work's next record version; it preserves all previous fields and claims and
fails on live preimage drift. Expression, Edition and Item records remain new
identities. A later `operation_date` names its discovery/acquisition events
without rewriting earlier operation identifiers.

The `perseus-tei-translation` profile requires an explicit English translation
role, the exact CTS translation identity, declared English XML language and
the source's qualified division addresses. It checks nonempty Latin-letter
text without claiming language identification or translation quality. Shared
Work identity does not establish which Greek Edition a translator used.
Where an exact source-language observation resolves a conflicting catalogue
label, preserve the contradiction and bind the inspected source opening by
digest; do not silently correct the provider's bytes. Registry coverage follows
the planting's discovery record to the exact Item and acquisition event, so a
branch route for one version does not mark another language version planted.

Post-acquisition readiness records use explicit `execution` status.
`completed` requires digest-bound local files and the exact Work/branch planting
record. Additional versions name an already known Work in `existing_record_refs`,
with the same digest and planting checks; they cannot also declare that Work
new in `create_record_refs`. Completed, deferred and blocked entries remain in history and are not
silently proposed for acquisition again. Absence of execution status preserves
the existing pending/chronological behavior.

Latin TEI intakes explicitly name the reviewed identity carrier: edition `n` or
body `xml:base`. A conflicting carrier or different language/version role fails
closed; legacy Greek and English profiles keep their original identity checks.
Missing print-exemplar metadata remains an explicit source uncertainty.
