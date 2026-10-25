# Manual planting operator route

This runbook organizes existing source-owner commands. It creates no new
writer, grant, corpus instance, use admission or release permission.

## Select and look up before creating

Select the actual software identity, admitted reader data/current manifest and
mutation source owner independently. `TOS_DATA_ROOT` or `--root` selects data,
not code or a grant. A Git checkout or discovery response is not a current
source catalog; missing files there do not prove missing subjects.

Search through the selected ordinary reader. Check `/api/source/capabilities`
and `/api/source/contracts`; discover a typed selector through
`POST /api/source/handles`, then send the complete returned handle with
`representation: "record"` to `POST /api/source/read`. The equivalent CLI is
`tos ... source capabilities|contracts|discover REQUEST.json|read REQUEST.json`.
Unavailable or stale lookup stays unperformed; catalog `missing` has only the
selected catalog's scope. Inspect exact source wording and retained versions.

Record the identity level, criterion, grounds, counterreading and source pins.
Same referent means reuse its stable ID. Different referents retain an explicit
non-join. Possible matches require a request to the subject's single creation
owner before another identity is created. A new metadata version keeps the
subject ID. A name, URL or matching file hash does not equate every level of
Work, Expression, Edition, Item and File. Identity proposals do not execute
merge, split, redirect or endpoint rewrites.

## Pinned descriptor and actual-handler drift

At 18ea8dca/tree7d3 the packaged descriptor is not exact evidence for every
handler's implemented request keys. The five owner-local TextUnit/TextLayer
families require only `{"operation":"describe"}` for description. Initial or
derived TextLayer preparation requires only `{"operation":"prepare-create"}`.
Their inspected handlers reject the descriptor's additional schema_version.
Their mutation requests still require schema_version; never remove it there.
Local TextUnit preparation reads operation/spans/excluded_gaps without the
declared exact top-level/schema gate; the PG preparation accepts exactly those
three keys. Local Alignment information commands do not enforce the descriptor's
exact keys/schema; PG Alignment information commands do. These are source-level
observations, not runtime tests or a grant. Keep affected contracts on HOLD
until their owner reconciles implementation, catalog and documentation.

The complete fourteen affected local operation contracts remain **CONTRACT
DRIFT / HOLD; NOT RUN**. The following source observations do not authorize
submission, bypass current owner selection, or claim production reconciliation:

| Handler | Operation | Pinned local observation; operator disposition |
| --- | --- | --- |
| owner-local-text-unit-create | describe | Operation-only exact keys; descriptor adds schema_version. HOLD / NOT RUN. |
| owner-local-text-layer-create | describe | Operation-only exact keys; descriptor adds schema_version. HOLD / NOT RUN. |
| owner-local-text-layer-create | prepare-create | Operation-only exact keys; descriptor adds schema_version. HOLD / NOT RUN. |
| owner-local-text-layer-derive | describe | Operation-only exact keys; descriptor adds schema_version. HOLD / NOT RUN. |
| owner-local-text-layer-derive | prepare-create | Operation-only exact keys; descriptor adds schema_version. HOLD / NOT RUN. |
| owner-local-text-layer-record-owner-ocr | describe | Operation-only exact keys; descriptor adds schema_version. HOLD / NOT RUN. |
| owner-local-text-layer-record-owner-ocr | prepare-create | Operation-only exact keys; descriptor adds schema_version. HOLD / NOT RUN. |
| owner-local-text-layer-record-owner-page-ocr | describe | Operation-only exact keys; descriptor adds schema_version. HOLD / NOT RUN. |
| owner-local-text-layer-record-owner-page-ocr | prepare-create | Operation-only exact keys; descriptor adds schema_version. HOLD / NOT RUN. |
| owner-local-text-unit-create | prepare-create | Local reads operation/spans/excluded_gaps without the declared exact top-level/schema gate; not the same local rejection. PG has an exact three-key gate. HOLD / NOT RUN. |
| owner-local-native-translation-alignment | describe | Local information path lacks the declared exact top-level/schema gate; PG checks both. HOLD / NOT RUN. |
| owner-local-native-translation-alignment | inspect | Local information path lacks the declared exact top-level/schema gate; PG checks both. HOLD / NOT RUN. |
| owner-local-native-translation-alignment | inspect-version | Local information path lacks the declared exact top-level/schema gate; PG checks both. HOLD / NOT RUN. |
| owner-local-native-translation-alignment | inspect-recovery | Local information path lacks the declared exact top-level/schema gate; PG checks both. HOLD / NOT RUN. |

Select a backend before choosing verbs. PG Claims does not expose prepare-create
or prepare-revise; PG Item exposes only prepare-create/item.adopt; the PG record
adapter does not expose inspect-version/record.recover and excludes profile
revision families. Initial PG creation lacks local describe/prepare, and PG
compound recovery is exact mutation replay. Do not carry local previews or
expectation tokens into another backend. Record an unavailable operation and
return it to its current owner, preserving all source/version evidence.

## Use the existing typed operation

Run `tos-native-owner-command source-commands --discover --handler HANDLER` for
the exact packaged request shape. Discovery is implementation-only. Execute
only through an independently issued protected invocation:

```sh
tos-native-owner-command source-commands --invocation ABS < request.json
```

Use `native-corpus-create` / `source.create` for initial Agent, Place,
Organization or Work; v2 also permits a bare Collection. Use
`native-work-expression` / `work.expression.create` for a new Expression,
`native-expression-edition` / `expression.edition.create` for a new Edition,
`native-item-adoption` / `item.adopt` for an already obtained local File and
acquired Item, `native-collection-membership` / `collection.work.attach` for
membership, and `native-object-link-create` / `object.link.create` for a Link
and qualified association. The Nietzsche Work home retains its stronger
authored_by and first_publication_chronology closure. Generic Claim writing
does not bypass compound topology owners.

For declared metadata use the existing profile creator; source_record_profile
is a reader/type contract, not a grant. Keep exact IDs, native shapes and
versions. Preserve the legacy relation streams and native source-claims.jsonl
homes as separate supported carriers with their original provenance.

Descriptive supplementation applies only where the actual existing metadata,
Claim or form family declares that revision route and the issuer independently
scopes it. A generic correction cross-reference does not create a writer.
There is **no standalone native File CorpusRecord creator, reviser or version
reader**: original bytes and fixity stay immutable; any changed file needs its
proper existing Item/payload-owner route and distinct File/event evidence, not
an overwrite. There is **no generic Environment CorpusRecord creator or
reviser**: execution context/provenance, explicit selections or an already
declared typed metadata profile remain with their actual owners. Neither this
runbook nor a correction cross-reference supplies a new File/Environment API,
identity, grant or permission.

Read the actual backend contract. The inspected PG initial creation adapters
support prepare-create and mutation rather than every local information verb.
PG Growth supports its declared describe/preparation/mutation, but local
transaction-ID resume/rollback requests are not PG recovery operations.

## Prepare, publish and read back

Save the exact proposal, readable preparation, target/write set, source refs,
configuration/dependency/publication guards and command ID before sending.
Copy only the expected fields accepted by that family's exact request shape.
Do not submit a superset envelope from another operation or guess a digest.
Reprepare after a current-version conflict. Creation receipts do not grant
current admission. Read current source and forms, exact predecessor versions,
compound origin and current assessment separately. A metadata version reader's
selected-record-chain verification is not a full package-byte audit.

Claims retain evidence, uncertainty and source-visible assessment. Manual
authoring use requires its independently selected policy/authority/competence,
exact source-owned procedure and basis, and the existing assessment journal.
Unread or unsupported work is not a source-backed historical unknown.
Record source/version/sheet/row/field locators even when a native locator
adapter is unavailable; the implemented basis json-pointer adapter cannot
attest reading an original spreadsheet, scan or text file.

## Interruption and two-author handoff

Checkpoint command ID, request digest, selected source/software, expected
guards, receipt/pending publication, current/history readback, unprocessed
mapping keys and exact return condition. After an uncertain response, inspect
preservation before retrying. Reuse the original command ID and request for
exact replay. Local selected recovery uses the retained transaction's explicit
resume/rollback under a current recovery grant. PG recovery replays the original
mutation. Never remove locks, archives, pending evidence or retained payloads
to make a retry succeed; third-state bytes require owner reconciliation.

Before two authors work, assign exactly one creator for each shared referent
and disjoint exact subject/Claim/Form/Event allocations. Other authors reuse
the returned current exact ID/receipt or request that creator; reservations
are not grants or source locks. CAS and current publication/source fences still
apply when authors touch the same parent. After one append, another stale
proposal must be reread and prepared against the current parent.

Every source field/Claim maps to target exact ref and command receipt/readback,
or an explicit unresolved/non-transfer disposition with grounds and owner.
Retain negative identity decisions and immutable raw and transformed source
versions separately. Send the whole changed scope to an independent reviewer.
Return defects to the same author with exact candidate/version/hash and mapping
keys; integration selects only the reviewed version. Keep at most one pending
package per author. Overall rehearsal, final review, mass planting and
publication require their independent coordinating acceptance decisions.
