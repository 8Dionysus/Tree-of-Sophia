# Source Witnessing Mechanic

<a id="active-route"></a>

## Mechanic Card

| Field | Route |
| --- | --- |
| status | `planted` |
| class | `local` |
| trigger | source witness material needs route discipline before branch or canon movement |
| input | primary witness, published source metadata, translation layer, evidence status |
| output | witness route or return-to-source |
| owner | `mechanics/source-witnessing/` |
| stronger route | `ToS/source-witnesses/` owns witness material |
| next route | [Witness Route](#registry-to-source-operation) |
| validation | `tos-ops-mechanics-plan --repo-root ABS --source-home` |

## Operations
| Part | Function |
| --- | --- |
| [Witness Route](#registry-to-source-operation) | keep source witnesses distinct from branch, candidate, and canon material |

The [local card](AGENTS.md) owns editing; [validation](../VALIDATION.md) owns checks.
Source or semantic questions return to [the source owner](../../ToS/AGENTS.md).

The part uses source-home and witness manifests; its next routes are
`ToS/source-witnesses/`, `ToS/philosophy/` and `ToS/candidate-intake/`.

## Registry-to-source operation

Research originals and reported normalized values belong to
`ToS/research-packets/source-registries/`. The field adapter in
`config/registry-normalization.v1.json` and `tos-source-registry normalize`
preserve complete raw/source trace.
Select the adapter owner with `--source-root`, the packet with `--packet-root`
and new originals with `--input-root`. Without an input root, normalization
replays the packet's retained originals. New snapshots identify the actual
native producer; `check` reproduces an existing snapshot while preserving its
recorded producer provenance. `validate` additionally checks the document
schema, exact field accounting and record counts.

`tos-source-registry inspect --packet-root PACKET --corpus ID --document ID`
opens a report; `--record ID` selects a source or normalized record ID.
`reconcile --source-root SOURCE_ROOT --output-root OUTPUT_ROOT` exposes possible
owner matches. `coverage` uses the same explicit roots to project reviewed
preparation, acquisition and exact-version planting evidence. Its
`--verify-local` mode reads current file hashes; `--remaining` and `--document`
print selected registry rows. These observation modes write no projection.
Output roots for these views remain separate from the selected source root.

Before acquisition, review exact versions, live access, intended-use rights,
local presence and branch anchors independently. Use the existing discovery
queue's readiness mode to freeze a source-bound plan, preserving chronological
selection and historical receipts. `tos-ops-mechanics-plan --repo-root ROOT --prepare-source-anchor`
resolves actual branch/backlog anchors without creating planted records. Pass
`--atlas-row`, `--source-table-index`, `--source-row-index` and the exact
`--source-label`. The JSON plan retains the source row number, row digest,
branch manifest digest and complete backlog digest. Duplicate, missing,
escaping or mismatched selections fail before producing a plan.

Actual acquisition retains upstream bytes, checks their pinned identity and
content structure, records per-file fixity and rights, and links the exact
source to its branch. Items and Composite representations use the same
local ignored payload route with tracked metadata. A file must be opened and
verified locally before a batch result claims it is present.

The ordered checks remain in `docs/validation/validation_lanes.json` under
`source_witness_foundation` and the affected philosophy route.

The native acquisition owner is `tos-native-owner-command acquisition` with
`family` set to `registry`. `registry.verify_preparation` checks the frozen
manifest and retained evidence; `registry.acquire` requires the preparation
checkpoint receipt; `registry.verify_local` verifies installed files. Select a
durable `payload_source_root` that survives the worktree; relative Item paths
contain `payload/`. Metadata and review records stay in the checkout. Pass the
same root to `registry.verify_local` and the foundation validator with
`--require-local-payloads`. The
[custody route](#standalone-retained-payload-custody)
copies or verifies retained files.

For a growing provider queue, use the [versioned batch route](#versioned-acquisition-batch-route)
with `tos-native-owner-command acquisition`, `family` set to `batch`, and
`prepare`, `acquire`, or `verify_local`. The route uses the
`ToS/contracts/acquisition-batch.schema.json` manifest and an explicit local
custody root. Its `handoff` family prepares a selected handoff for
`corpus-admit`; the legacy seven-package converter remains separate. The batch
route creates no per-target `topology-before` preimage.
The separate frozen-plan import route in
`ToS/source-witnesses/server-import/SERVER_IMPORT_PROTOCOL.md` binds those bytes
to the exact Item, File and rights revision before private R2 transfer. Adding
an object replica does not change the philosophical graph or require its
compilation. Website delivery remains a separate consumer integration.
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
The native registry owner permits only the new Expression claim refs and the
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

## Versioned acquisition batch route

The native `tos-command` acquisition owner implements the repeatable
preparation and local transfer boundary for a new provider batch. Invoke
`tos-native-owner-command acquisition` with a bounded JSONL request whose
`family` is `batch` and whose operation is `prepare`, `acquire`, `verify_local`,
or `measure_storage`. Select the installed owner binary through `PATH` or the
explicit `TOS_NATIVE_OWNER_COMMAND_BIN`; the native route has no Python
fallback. The native HTTP transport uses `/usr/bin/curl` with curlrc disabled,
restricted HTTP/HTTPS protocols, a finite provider deadline and byte cap.
Tests may supply fixture bytes through the native callback frame only when
requested by the native attempt loop. The route is deliberately
selection-driven: the caller supplies one immutable manifest matching
`ToS/contracts/acquisition-batch.schema.json`, an explicit metadata root, and
an empty output root. The route never discovers records by walking the
checkout and never mutates the shared source topology.

The manifest is the frozen input selection. Every selected Item carries its
exact metadata record references and SHA-256 values, a rights record and
posture, and one provider revision/source identity. Every payload carries its
stable ToS File ID, Item root, credential-free provider URL (no userinfo,
query string, or fragment), provider revision/source ID, byte size,
SHA-256, and optional Git blob SHA-1. `provenance_delta` closes over the same
record and File sets and binds them to the exact accepted base revision. A
digest supplied to the command is the operator's immutable-selection check.
Because ToS File IDs are content-addressed, one File may be selected by more
than one Item in a batch, but each Item binds a File ID to only one payload
destination. The provenance delta lists the shared File ID once, while
acquisition, fixity, and handoff custody retain one binding per
`(Item, File, destination)` so every Item keeps its own path and rights closure.

`prepare` copies only the enumerated records into `source/`, emits one
batch-level `provenance-delta.json`, and writes an immutable preparation
receipt. It does not acquire bytes. The output root and its `source/`,
`payload/`, and `receipts/` custody roots are created and rechecked with
owner-only `0700` permissions. On every resume and immediately before a
handoff, the route rereads every selected metadata record, including the
rights record, and compares its bytes with the manifest and preparation
receipt. It also checks the exact deterministic provenance-delta path and
digest. A changed selected record or custody-root permission fails closed
before a provider fetch or handoff receipt can be produced. The adapter keeps
historical sealed handoff bytes readable without rewriting their legacy modes;
all newly prepared roots use and retain the private mode.

The producer preflight requires each selection to name its exact Item record,
Item manifest, rights record, and Item provenance record, and every metadata
reference must pass the existing corpus-source membership predicate. The
manifest also rejects a metadata reference reused with a conflicting kind or
digest. Provider URL parsing and HTTP protocol errors are recorded as isolated
per-file source failures rather than aborting the batch.
prepared Item record identity is checked against the selected Item ID before
payload transfer. If preparation is interrupted, recovery removes only a
directory whose manifest bytes match the requested selection, whose source
tree contains only selected record paths and the deterministic delta, and
whose payload and receipt trees are empty. Existing payload, receipt, or
foreign source evidence is retained and causes a fail-closed error.

`acquire` uses provider fields to fetch each payload independently and publishes
it through descriptor-anchored native no-clobber custody. It verifies
destination readback and appends a durable per-Item/File/destination journal
row. A failed provider is recorded and does not prevent other files from
completing. A later invocation
rechecks successful destinations and retries the failed or missing files.
Preparation is rebuilt when an interrupted run left only the narrow
route-owned `source/`, `payload/`, and `receipts/` shape without
`receipts/preparation.json`; an output with unrelated files or an existing
receipt is never removed. A sibling lock serializes standalone `prepare` and
`acquire` calls for the same output root, including first preparation and
handoff run identity.

Payload custody files are published read-only with mode `0444`. On resume and
during the separate fixity pass, the verifier checks that mode on the same
no-follow descriptor used to hash each file; byte-identical writable files
remain conflicts and cannot be sealed as acquired. The fixity pass writes
`receipts/fixity-*.jsonl` plus its summary. The immutable
`receipts/handoff-*.json` carries selected record refs/digests, provider and
payload custody rows, the fixity refs and JSONL/summary SHA-256 values, the
provenance-delta ref and SHA-256, and these
boundaries:

```text
acquisition_status: acquired-not-admitted | partially-acquired-not-admitted | prepared-not-acquired
admission_status: not-admitted
publication_status: not-published
```

The handoff is an input to the corpus-intake owner. It is not a corpus batch,
accepted revision, R2 transfer, publication decision, semantic review, canon
change, or deployment receipt. Intake should select one or more explicit
handoff paths and bind each selected handoff's manifest and fixity digests;
the acquisition route intentionally provides no hardcoded historical batch
list. The native `handoff` family is the consumer for one selected handoff: it
verifies those bindings and emits a private `tos_corpus_batch_v1` input root
with explicit `source/` and `payload/` roots. It checks the selected revision
against an explicit accepted store pointer and loads that pointer's cryptographically bound immutable
`revisions/<base_revision>/snapshot.json`. For every selected source path, the
accepted-source view must contain the exact snapshot member and bytes; a path
absent from the snapshot must also be absent from the view. An arbitrary empty
or mismatched view therefore fails closed before candidate output. The adapter
then calls the existing native `AdmissionBatch::read` contract and does not call
admission. The caller must provide the exact validation context that produced
the selected `validator_sha256`: grammar root plus every paired historical
capture/restored root. The adapter verifies retained capture and restore
receipts, writes that context and its admission flags into the candidate
receipt, and reports `explicit-grammar-update-required` when the selected
identity differs from the accepted snapshot. It never silently rewrites the
batch identity or uses the accepted validator as a substitute. A downstream
admission run must pass the receipt's exact context to `corpus_admit`; the
adapter's read-batch check is transport evidence, not SourceValidator
acceptance. The receipt marks this as
`validation_context_posture: transport-bound; downstream-source-validator-required`;
the adapter does not recompute validator identity or claim the grammar/history
preflight has run. It also binds every custody row to all selected provider
fields and the handoff run, checks the selected Item rights scope and
acquisition event, and compares the payload Git-blob digest and filesystem
modes before emitting a candidate. Accepted files and selected metadata
records must use mode `0644`, matching the candidate update contract; other
modes fail closed rather than being silently rewritten. It also copies the
exact selection manifest, handoff, provenance delta, and independent fixity
files under
`receipts/acquisition-evidence/`. The selected Item provenance event must be
an `acquisition` event with `status` `completed` or
`completed_with_warnings`; a failed, stopped, or superseded event cannot be
transferred. The delta remains operational evidence and
is not added to `tos_corpus_batch_v1.updates`; its source ref and SHA remain
bound in the adapter receipt. Top-level receipt refs point to these candidate
copies, while explicit `*_source_ref` fields retain the original acquisition
paths.
The queued-corpus-intake owner can use this adapter's selector and receipt
shape while its seven-batch converter remains a separate owner surface.

Both `verify_handoff_for_intake` and the adapter API require the caller to
pass `expected_manifest_sha256` from the immutable selection or preparation
receipt; the adapter CLI requires the same value as
`--expected-manifest-sha256`. The verifier checks that caller-held digest
against the handoff's declared selection and the actual `manifest.json` bytes.
The hash inside the handoff cannot authenticate the handoff by itself. This
also permits an older sealed handoff to be consumed when its exact manifest
digest is independently retained.

The registry and handoff families share the native owner-command entrypoint;
no Python command or import facade is maintained for these routes.
Historical Python implementations and assertions remain as nonexecutable
`.py.snapshot` files under `tests/historical/`. Native fixtures use real
filesystem descriptors and an isolated local HTTP provider.

### Standalone retained-payload custody

The native custody entry is `tos-native-owner-command acquisition` with the
`custody` family, `cli` operation and `verify` or `copy` command. Callers select Item manifests, frozen
inventories or registry manifests and provide explicit metadata, source and
destination roots. The owner checks source size and available SHA-256/Git blob
identity, plans one destination per Item/File, and publishes copies with
no-clobber semantics. Payloads must retain mode `0444`; matching writable
files and different bytes are conflicts. New receipts are immutable mode
`0600` JSON containing rows, fixity, counts and refs, never copied payload
bytes or absolute host paths. The operation does not alter metadata, rights,
Item admission, semantic review or publication state. The prior Python
implementation is retained only as
`tests/historical/oracles/acquisition/source_payload_custody.py.snapshot`.
Maintained custody regressions live with the native owner.

Historical registry preparation remains in
`tests/historical/oracles/acquisition/prepare_registry_sources.py.snapshot`;
maintained preparation uses the versioned batch route above.

### Direct contracts

- `witness-route`: [ACQUISITION_BATCH_ROUTE](#versioned-acquisition-batch-route), [MANUAL_CORPUS_ENTRY_GATE](#manual-corpus-entry-gate)

### Manual Corpus Entry Gate

New branch-facing corpus movement begins with maintainer-led manual source
entry. The current gate is the bounded trilingual Zarathustra route within
`tos.source.thus-spoke-zarathustra.prologue`: one shared `node_id` with German
as canonical source, Russian as working translation and English as bridge
translation.

Until this route proves itself, ToS may prepare context-node law, review law
for pre-expansion preparation, and the compact relation vocabulary for already
published node families. A new family, a second bounded branch and wider
world-thought expansion require their own opening. Templates, review routes
and schema hardening prepare the architecture; source entry makes tree motion.

## History

Immutable Git history preserves moved-path accounting from the former
`ToS/doctrine/MANUAL_CORPUS_ENTRY_GATE.md` placement. Source texts remain
active source-home witnesses, not mechanics legacy.

[Exact prior provenance](https://github.com/8Dionysus/Tree-of-Sophia/blob/b54cbf8e30ff46aeddeec5c47a5e7aa1f370977d/mechanics/source-witnessing/PROVENANCE.md) preserves the pre-compaction route.

## Growth conditions

Add witness-route payload or split source entry, translation posture, and witness-to-branch handoff when reviewed receipts or cross-branch corpus movements recur. Anchor atlas rows and research dossiers to editions, translations, corpora, or published evidence before treating them as witnesses.
