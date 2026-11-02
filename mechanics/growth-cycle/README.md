# Growth Cycle Mechanic

<a id="active-route"></a>

## Mechanic Card

| Field | Route |
| --- | --- |
| status | `planted` |
| class | `head-fed/local` |
| trigger | source pressure must deepen a node, create a node, or form a branch |
| input | growth pressure, source anchor, review state |
| output | branch growth route or return-to-review |
| owner | `mechanics/growth-cycle/` |
| stronger route | `ToS/philosophy/` and `ToS/canon/` own content |
| next route | [Branch Growth Cycle](parts/branch-growth-cycle/README.md) |
| validation | `tos-ops-mechanics-plan --repo-root ROOT --philosophy-topology` |

## Operations
| Part | Function |
| --- | --- |
| [Branch Growth Cycle](parts/branch-growth-cycle/README.md) | route node deepening, node creation, and branch formation as distinct moves |

The [local card](AGENTS.md) owns editing; [validation](../VALIDATION.md) owns checks.
Source or semantic questions return to [the source owner](../../ToS/AGENTS.md).

### Direct contracts

- `branch-growth-cycle`: [GROWTH_STRUCTURE](#op-doc-growth-structure), [HUMAN_CURATED_EXPANSION](#op-doc-human-curated-expansion), [NATIVE_ARTIFACT_GROWTH](#native-artifact-metadata-creation), [NATIVE_BEHAVIOR_COVERAGE](README.md#native-growth-behavior-and-legacy-fate), [NATIVE_COLLECTION_GROWTH](#op-doc-native-collection-growth), [NATIVE_EXPRESSION_EDITION_GROWTH](#op-doc-native-expression-edition-growth), [NATIVE_EXPRESSION_RESPONSIBILITY](#op-doc-native-expression-responsibility), [NATIVE_IMAGE_OCR_ASSESSMENT](#exact-imageocr-comparison), [NATIVE_ITEM_ADOPTION](#op-doc-native-item-adoption), [NATIVE_OBJECT_LINK_GROWTH](#op-doc-native-object-link-growth), [NATIVE_TEXT_LAYER_ASSESSMENT](#exact-text-layer-comparison-and-current-quality-use), [NATIVE_TEXT_LAYER_CONSTRUCTION](#private-native-textlayer-and-first-textunit-construction), [NATIVE_TRANSLATION_ALIGNMENT](#confidential-native-translation-alignment), [NATIVE_WORK_EXPRESSION_GROWTH](#op-doc-native-work-expression-growth), [PRE_EXPANSION_SOIL](#op-doc-pre-expansion-soil), [PUBLIC_NATIVE_TEXT_CONSTRUCTION](#public-project-text-native-construction), [SELECTED_METADATA_TRANSACTIONS](README.md#selected-metadata-publication-and-recovery), [SOURCE_COMMAND_DISCOVERY](#op-doc-source-command-discovery), [SOURCE_COMMAND_HTTP](README.md#explicit-local-source-command-transport)

## Source-owner operations

## Find a source-owner command

Use `tos-native-owner-command source-commands --discover` before selecting a
grant. The [discovery contract](#op-doc-source-command-discovery) owns the command
grammar, result shape and limits; discovery reports implemented operations,
while a separately selected current grant authorizes execution. The
[loopback transport](README.md#explicit-local-source-command-transport) carries that same grammar for
authenticated browser or agent clients.

### Explicit Claim display fields

`public-source-forms` also exposes `tos_local_claim_form_owner_v2`, selected by
one `claim_id` as before. In addition to the v1 fields it requires a nonempty,
unique `allowed_field_ids` list drawn from `claim.statement`, `claim.name`,
`claim.caption` and `claim.hover`. Only exact source-copy changes are accepted
by this new delegation. The source Claim must explicitly declare the versioned
`qualifiers.display_fields` contract for the three compact fields. No field is
guessed, shortened, translated or admitted by the command. Every materialized
field retains its whole exact Claim as required context.

Public Claim-correction configurations may add `allowed_form_field_ids` with
the same finite selectors; omission preserves statement-only behavior. This
does not replace document-date, structured-value, reference-value or layer
permission. The operation must rebind all current forms with the successor
Claim, and both old and new selected fields remain in scope. Current scope
also checks exact retained predecessor forms on retry. Older public v1,
private and compound configurations acquire no new source-copy selectors.
Discovery separates existing `source_fields` from actually permitted fields.

See [Human Forms](../../ToS/doctrine/HUMAN_FORMS.md#explicit-compact-claim-fields)
and `ToS/contracts/claim-display-fields.schema.json` for source wording,
unknown-version and assessment boundaries. Consumer checks separately verify compact delivery budgets and presentation.

### Captured legacy historical Claims

`legacy-historical-claim-revision` and `legacy-historical-claim-forms` operate the `history/**/historical-claims.jsonl` family through its own legacy
schema and captured origin. Their owner schemas are respectively
`tos_local_historical_claim_revision_owner_v1` and
`tos_local_historical_claim_form_owner_v1`. Both require exact `claim_id`,
`historical_record_id`, `creation_receipt_sha256` (the prefixed raw SHA-256),
protected account/root/path/authority/expiry, selected form IDs and explicit
`allowed_form_field_ids`. Existing grants do not acquire either adapter.

Revision uses the existing `describe`, `prepare-revise`, `claim.revise` and
`inspect-version` shapes. `allowed_fields` can contain only `qualifiers`;
`allowed_qualifier_fields` selects `statement`, `statement_language`,
`statement_script` and/or `display_fields`. `allowed_evidence_refs` stays empty.
No object, subject, predicate, layer, evidence, maker, admission, version tag or
existing structural qualifier is writable. Revisions advance the Claim version
and rebind every current form, including the full statement, through the shared
flat-package lock/CAS/archive/exchange route. Other rows retain their exact bytes.

The separate form grant uses `describe`, `prepare` and `apply` with the ordinary
form command shape, and only exact source copies of selected fields. Every
compact field retains the whole Claim. Allocated form IDs are checked against
current and retained forms reached through public record/Claim source locators;
the adapter does not scan private/native payloads. No form write revises a Claim.

Both writing routes require complete captured `historical.create` v2 origin.
Original request, receipt, environment and provenance bytes remain immutable;
historical record revisions and shared Claim revisions keep independent chains
in the same package. Creation replay proves the initial Claim stream through
its retained archives rather than comparing corrected rows with initial hashes.
Uncaptured older packages remain read-only and need their own migration route.

The read-only exact Claim version reader follows this legacy family explicitly,
preserving its schema and IDs. It budgets its current package, contracts and
Claim archives, and checks captured Claim origin; it does not traverse or certify
the HistoricalEvent's separate record archives (`record_history_verified: false`).
The mutating adapter additionally verifies those record archives. Graph and assessed-form consumers use the adjacent forms of that exact Claim.
Historical assessment, public-use admission and current execution authority
retain their own source-bound checks.

### Catalogue-assigned Document dates

Discovery exposes `public-claim-create-document-catalogue-v1` and
`public-claim-revision-document-catalogue-v1`. Their protected owner schemas are
`tos_local_document_catalogue_date_create_owner_v1` and
`tos_local_document_catalogue_date_revision_owner_v1`. They use the existing
creation/revision request shapes and exact `allowed_object_values`; create
permits only `document_catalogue_date`. Revision requires the new grant even
for wording-only edits and replay, and both current and proposed values must
be explicitly allowed. These schemas cannot create or revise another family.

The source contract is `ToS/contracts/document-catalogue-claim.schema.json`.
Copy source wording without normalizing it into a citation. A full qualified
statement, language/script and exact catalogue attribution are mandatory;
the existing `claim.statement` form route retains the complete Claim context.
Unknown calendar or year numbering remain unknown. No grant follows from
discovery or a readable profile. Public v1–v4 and private owner-local grants
do not gain this date reader. `document_catalogue_origin` and
`document_catalogue_destination` remain separately allowlisted identity relations using existing public Claim
creation.
Composition, sending, receipt, historical events and Place creation each
require the grant for that separate operation.

[Native Expression/Edition growth](#op-doc-native-expression-edition-growth)
adds one separately delegated provisional Edition and exact `embodied_by`
Claim without creating an Item/File or changing global bibliographic cardinality.

[Native local Item adoption](#op-doc-native-item-adoption) separately retains
one already obtained File under an exact canonical payload grant, then publishes
an acquired Item and `exemplified_by` Claim with one Edition append. Private byte recovery and public metadata evidence retain their separate
scopes. Download, source reading, rights decisions and publication each follow
their owner route.

## Operating Card

The branch route card and operation map live in the
[Branch Growth Cycle README](parts/branch-growth-cycle/README.md); this journal retains the detailed
source-owner command contracts below.

## Assessment policy application

`rust/crates/tos-validation/src/assessment.rs` applies the source-owned
`ToS/doctrine/KNOWLEDGE_ASSESSMENT.md` law to authenticated owner inputs without
network, model calls or source writes. It distinguishes substantive assessment
from current admission. The caller supplies trusted policy, grants, competence,
current exact records and complete bounded subject history; submitted prose
cannot provide its own authority. The pure engine evaluates the supplied authenticated inputs; command adapters
bind them to actual source and authority records. Local invariant checks belong to
`tests/conformance/rust/command_owner_text_cases.rs` and the existing
`mechanics_local` discovery lane.

## Source-owner journal

### Owner-local translation Alignment through the PG source command

The protected `tos_local_native_alignment_owner_v1` grant remains the source
of Alignment scope and authority. An explicitly selected local-owner invocation
with schema `tos_local_pg_private_alignment_invocation_v1` supplies the source
revision, software capture and worker budgets; it cannot select another owner
path or widen the grant. It supports `describe`, `prepare-create`,
`prepare-revise`, `alignment.create`, `alignment.revise`, `inspect`,
`inspect-version` and `inspect-recovery`.

The protected invocation has exactly `owner_context`, `owner_config`,
`native_executable`, `native_executable_sha256`, `source_selection`,
`software_capture`, `software_restored_root`, `software_selection`,
`software_components`, `schema_worker` and `budgets`. The budgets object selects
member/byte caps, schema-worker receipt caps and worker CPU/address-space caps;
the adapter only tightens them against its fixed route limits.

Public ToS inputs, schemas, rights and identity owners are selected through the
held PG source revision. Owner-private Text and Alignment packages are read by
exact reference from the scoped private package broker. Identity absence uses
the separate public and owner-scoped private Text identity indexes, with an
exception only for the exact digest-bound Alignment ancestry authorized by the
grant. Create and revise write one immutable Alignment package with the current
source and private-audit fences in the same broker transaction. Exact retries
use the original command receipt; neither response reveals represented text.

Alignment records preserve supplied correspondences as proposals: the route
does not run an aligner, apply assessment, admit source, or grant semantic or
canon status. `inspect` remains a metadata-only owner view, not acceptance.

### Sign issuance through the shared source command

The local filesystem `source-commands` route accepts the separately delegated
`tos_local_sign_promote_owner_v1` configuration. The distinct PostgreSQL Growth
route requires Sign owner v2 and Assessment owner v5/v6; see the
[selected PostgreSQL Growth route](#op-doc-source-command-discovery-selected-postgresql-growth-route).
Run `tos-native-owner-command` with `source-commands --invocation ABSOLUTE_PATH`.
It has the public profile-creation fields (`uid`, `principal_id`, `source_root`,
`source_path`, `authority_ref`, `allowed_form_ids`, `allowed_operations`,
`expires_at`, `record_id`, `profile_type_id`, `maker_type`,
`provenance_event_id`) and exactly two additional owner-selected fields:
`promotion_assessment_owner_config` (protected absolute configuration path)
and `promotion_candidate_id` (one concrete Claim ID). `profile_type_id` is
`tos.entity.sign`; the delegated operation is `sign.promote`, not `source.create`.
The usual initial source directory, ID, provenance, source-copy form, size,
same-account and no-replace transaction constraints remain in force.

The selected assessment owner must be public source-bound v2/v3, use the same
source root, and select a `tos_source_occurrence_motif_claim_v1` Claim with
`semantic_interpretation` layer. Its scope independently requests
`sign-promotion` at moderate/high risk and uses authorized, competence-bound
assessment under the source-owned policy. Exact native reading and all member
dependencies are required. Inline targets and owner-local v4 configurations
cannot be used to publish a Sign description through this public adapter.

`describe` returns the shared operation/field catalog plus `promotion.eligible`,
current admission and, when eligible, `promotion.basis`. Copy this exact basis
into the proposed `tos_sign_description_record_v1` record's `promotion_basis`.
Use `prepare` for source-copy field selection and `prepare-create` for the
same record/forms transaction preview. Apply `sign.promote` with
`command_id`, `record`, `forms`, exact `expected_configuration`,
`expected_dependencies`, and null `expected_source`/`expected_revision`.
The caller cannot insert an assessment, scope, clock, grant or owner path into
the command request. Name/notes forms retain the complete issuance basis and
limits as required context; their wording retains its own assessment state.

The command reevaluates the selected assessment before preparation and again
at the publication edge, holding the same subject journal lock through final
evaluation and directory publication. Normal assessment append/withdraw cannot
interleave between these two steps. Lock order is corpus then journal; busy
writers return the existing bounded retry signal. External source/configuration
publishers must keep their owner inputs stable for the command duration, as in
the other same-account source operations. A changed candidate, dependency, journal, policy or
grant conflicts or closes the gate. Files, source-copy forms, provenance and
receipt appear together via the existing no-replace directory publication.
An exact replay verifies the historical creation, including original bytes
and retained revisions; it does not reissue the ID or return current admission.
An old receipt cannot authorize a different target or a second Sign from the
same candidate. The current source writer delegation still must be valid on
replay. Revocation does not erase the journal or historical Sign.

The registry marks this profile with an executable `creation_gate`; both
generic public `source.create` and generic private profile creation refuse it.
Ordinary descriptive revision cannot edit `promotion_basis`. This first
transition does not support arbitrary annotation candidates, private Sign
issuance, identity merges/splits or canon transitions. The read-only version route below exposes the exact historical candidate with
its original assertion and review context.

### Read-only exact Claim versions

`claim_version_reader.resolve_claim_version(root, exact_ref)` accepts an
absolute public repository root and exact `{id, version, digest}` Claim ref.
It requires no command delegation, assessment journal or model invocation.
For a build with several references, reuse one `ClaimVersionReader(root)` and
call `verify_current()` immediately before publishing the derived result.
This snapshot batches catalog and shared package reads for one invocation by
one caller. Detected drift raises
at final verification rather than refreshing a partially assembled snapshot.

The reader resolves only public declared `source-claims.jsonl` metadata. It
checks current catalog identity, version, canonical digest, visibility and line,
then verifies the complete retained shared correction history with the existing
archive/blob and predecessor/successor verifiers. Available replies carry exact
original record fields and catalog, stream-byte, archive, history and transition
provenance. Unknown source fields survive without a claim to understand them.
The [derived version contract](../../ToS/contracts/record-version-view.schema.json)
separates this evidence from Claim identity and current assessment/admission.

`missing`, `stale`, `corrupt`, `access-restricted` and `over-budget` replies
retain the requested exact reference but no record/provenance. There is no
latest-version fallback. Private/native payload routes, symlinks, non-flat
packages and mixed public/private Claim streams fail closed. The bounded reader
permits 32 MiB/32,768 catalog rows, 1 MiB Claim streams, 64-file/8 MiB source
packages, 128 retained corrections and 64 MiB cumulative preflight reads.
Limits may refuse a valid larger source; refusal is not truncation or corruption.
Only public current ownership/visibility can expose retained predecessors;
old public bytes do not bypass a current restriction. This read operation
performs no writes, assessment, admission, publication or source-schema migration.
Historical HumanForms, native annotation versions and other record families
are not supplied by this bounded Claim reader.

### Read-only exact metadata versions

`metadata_version_reader.MetadataVersionReader(root)` offers `resolve(exact_ref)`,
`resolve_typed(exact_ref)`, `exact_refs(record_id)`, `supports(record_type, source_ref=...)` and
`verify_current()`. It uses the same exact-ref/status envelope, with no command
configuration or current-use authority. Supported routes are native Corpus
Agent, Place, Organization, Work, Expression, Edition, Collection and Item,
native Artifact, Link and retained scholarly Composite, plus the registry's
declared metadata profiles and schema routes. Pass the locator to `supports`
when one catalog contains more than one native/declared representation.

An explicit `MetadataVersionReader(root, catalog_snapshot=SourceCatalogSnapshot(...))`
uses the [addressed catalog v2](../../ToS/source-witnesses/README.md#native-catalog-and-projection-routes)
for keyed record lookup. It verifies the catalog header's publication token and
generation against the same coherent live source snapshot, and checks exact
source raw bytes, length and record ref before opening retained history. There
is no legacy JSONL fallback or fabricated line number. Both current and
historical results carry the exact `CatalogRecord.provenance` in
`provenance.catalog`; global catalog roots and publication tokens remain outside
record/history/version provenance. An unrelated revision therefore does not
change an untouched record merely by changing a global catalog digest.

With an addressed snapshot, `resolve_source_bytes(path, raw_sha256, record_id=...)`
requires the explicit stable identity and checks that record's current/retained
source path and bytes; it never scans for a matching path. The default legacy
constructor and source-byte route remain unchanged. Catalog traversal uses the
supplied snapshot's `MutationLimits` separately from the source reader's existing
limits; either budget can refuse without partial evidence. An immutable catalog
alone grants no source currentness, reference closure, admission or publication.
The read-only `accounting` property exposes actual read bytes, the aggregate
byte ceiling and observed file/directory/record/contract counts. Counters report work performed; availability and admission use their explicit
result fields and checks.

`resolve_typed` returns the same envelope plus `descriptor`: the validated
owner `adapter`, `record_kind` (`subject`), `record_type`, `identity_field`,
`schema_version`, `schema_ref`, `source_basename` and `type_id`. It is not a
tuple. An unavailable result has `descriptor: null`, never an inferred type
or latest fallback. Claim references retain their separate Claim reader.
Artifact uses `artifact_id`; retained Composite uses `composite_id`. The descriptor identifies the record’s typed source route. The schema owns its
shape, the record describes the persistent subject, and assessment evaluates
its meaning. Identity transitions require their own operation grant.

`exact_refs` returns a continuous retained baseline through current, never
inventing versions before that baseline. Available results include `current_ref`,
`refs` and provenance. A gap returns null `current_ref`/provenance and empty
`refs`, with the explicit unavailable status. The source-navigation builder
emits `record_history` and separate `has_record_version` edges to closed
RecordVersion carriers; neither version nor description becomes the subject ID.

The reader verifies catalog/schema/current-record binding, every retained
metadata transition, manifest bindings and the selected record blobs. It does
**not** open current or archived HumanForms, unknown companions or private native
inventory. Accordingly provenance says `verification_scope=selected-record-chain`
and `all_package_bytes_verified=false`. Unread companion bytes may be missing
without making the selected record unavailable; selected bytes may not. Historical
records preserve all fields, including unknown language/context qualifications.
Historical versions retain their original semantic assessment state.

Work is bounded to 8 MiB/8,192 rows per catalog, 128 selected contracts, 128
corrections, existing package-manifest bounds and 64 MiB cumulative read work per
instance, including profile shape-reader rechecks. Reuse within one build and
verify before export; exceeding a bound refuses the record/history without
partial output. Native File bytes and textual representations remain outside
this descriptive metadata reader.

### Native Artifact, retained Composite and Link descriptions

The independent `tos_local_native_metadata_revision_owner_v1` grant selects
`record_type`, `record_id`, `record_schema_version`, the exact public owner
`source_path`, bounded `allowed_fields` and `allowed_form_ids`, plus the usual
local-account principal, authority, expiry and operations. It delegates
`record.revise` and optionally `record.recover`; existing Corpus and profile
grants do not gain these native shapes. `prepare-revise`, exact commit fields,
`expected_publication`, `inspect-version` and explicit recovery reuse the
[selected metadata transaction contract](README.md#selected-metadata-publication-and-recovery).

Artifact descriptive fields are `path_identity` (only its note may change),
`physical_description`, `find_context` and `bibliography`. Retained Composite
permits `preferred_label` and `editorial_object`. Link permits `preferred_label`,
`variant_labels`, `notes`, `source_refs` and `provider_label`. Exact typed
schema and persistent identity remain unchanged. No URI/observation,
membership, custody identity, rights, provenance, original maker or authority
change is delegated. Native v1/v2 Artifact and retained Composite formats
remain intact rather than being rewritten as Corpus records.

The writer reads and publishes only the record, adjacent form set and revision
history, preserves exact predecessor bytes, and explicitly rebinds every
current form. Descendants, representations and payloads are not enumerated.
Link forms include the exact URI, provider, observation and association
context; source-visible assessment separately evaluates the copied wording.

`resolve_source_bytes(original_source_path, raw_sha256)` joins a provenance
input to the exact current or retained metadata bytes at its original logical
source path. `raw_sha256` is the bare 64-hex digest, not the canonical record
digest. The catalog must uniquely locate that logical source, and the current
record or committed retained chain must bind the requested raw bytes. The reply contains the resolved exact record/ref and its byte provenance.
It never searches Git, orphan archives or a private store, and never substitutes
the latest version. This lets an immutable provenance input survive a later
descriptive correction without rewriting its old digest.

Both exact readers also hold a
[selected-metadata publication snapshot](README.md#selected-metadata-publication-and-recovery)
from construction through success **or unavailable** results. Pending or changed
source publication returns an explicit stale result; a catalog bound to another
publication cannot hide a newly created subject behind an old membership list.
The readers also perform independent exact-file/schema checks; the token
covers the selected publication protocol.

Access quotes only each available record's own notes (Claim: its own statement),
preserving its explicit language even when a requested translation is missing.
Missing language is null, never inferred from prose. Compact metadata versions
retain the entire exact record context with the quotation. Historical freeform HumanForms, current assessments, grants and publication
decisions retain their own source records and readers.

`rust/crates/tos-command/src/source_assessment_journal.rs` implements immutable source-owned assessment
batches and an atomic per-subject head pointer under an explicitly configured
owner directory. `ToS/contracts/knowledge-assessment-batch.schema.json` owns
their shape. The parent directory must already exist. A hash partitions assessment storage while the subject retains its ToS ID.
Corpus assertions remain in their source-owned records. Original assessment rationale and refs remain in the owned
batches; derived current admission can be rebuilt.

The native journal append operation
requires authenticated bindings and an agreed source snapshot from the command
owner. It records a valid assessment even when the judgment rejects, disputes
or defers use. It rejects the entire new batch on qualification failure or a
stale expected head. Replaying the exact command returns its old receipt and
fresh current admission separately. `inspect` materializes the selected subject’s complete committed history
through the local owner route.

Unix locking serializes writers with a bounded wait (five seconds by default);
`JournalBusy` means retry, not discard or restart the live writer. Immutable blobs are fsynced before atomic head
publication; interrupted unreferenced blobs are not active history and are not
silently deleted. Missing/corrupt committed data fails closed. Committed
supersession stays effective even after the old grant expires or is revoked.

Materialization currently bounds one history at 1,024 assessment events and
each batch at 1 MiB; it refuses truncation. Larger histories need a source-owned checkpoint/archive reader that retains
the complete history. Orphan retention, actual
corpus-adapter binding, cross-object transactions and research/UI integration
remain foundation work; the journal provides the assessment-history component of the Growth profile.

### Local account command contract

Owned Journal commands use the protected native invocation selected by the
command issuer. Public owner versions v1–v3 retain their inline, source-bound
and native TextUnit inputs respectively; confidential versions retain their
separate owner-local selection. See the
[local invocation release route](../../docs/RELEASING.md).

```bash
/absolute/installed-prefix/bin/tos-native-owner-command source-commands \
  --invocation /absolute/operator-selected/native-invocation.json < request.json
```

The installed command uses the protected invocation to bind the selected owner
configuration, native image, source and worker.

This is an explicit local owner operation, never part of read-only `access`.
The operator/command issuer selects the configuration independently of incoming
requests. It must contain already authorized policy, grants, calibrated
competence, source scope and an execution profile whose provenance the issuer
has checked. The issuer supplies those independently established records. Unix UID
authenticates the local account; execution-profile and competence records
identify and qualify the actual reviewer.
Processes sharing that UID share this trust boundary; use a separately owned
runtime adapter for mutually untrusted agents or remote callers.

The configuration supplies one bounded (8 MiB) agreed snapshot of source-owned
inputs. It has exactly these fields:

| Field | Owner value |
| --- | --- |
| `schema_version` | `tos_local_assessment_owner_v1` |
| `uid`, `principal_id` | actual local account UID and its delegated reviewer identity |
| `execution_profile` | exact current `{id, version, digest}`; independent of assessment prose |
| `policy` | one record envelope |
| `authorities`, `competencies`, `records` | bounded lists of current record envelopes (at most 1,024 each) |
| `subjects` | at most 1,024 IDs mapped to trusted scopes |
| `journal_directory` | an existing, dedicated absolute owner directory |

A record envelope is `{id, version, payload, origin_id}` (null origin is allowed
where independent-source support is not claimed). A subject scope is exactly
`{record, assertion_layer, risk, languages, maker_id, requested_use, access_allowed}`;
`record` is its exact ref. Unknown source fields are retained inside `payload`,
not interpreted as commands. Publish an updated configuration atomically when
source, permissions, policy or calibration changes. Each command samples it
once; configuration publishers must preserve the agreed snapshot during the
operation. The issuer must coordinate concurrent source edits to keep this agreed
snapshot stable. A subsequent call loads current configuration and rechecks admission.

The `inspect` and `append` request operations use `schema_version: tos_local_assessment_command_v1`,
`operation` (`append` or `inspect`), `subject_id`, `expected_subject` (exact ref)
and `expected_snapshot` (`sha256:` plus canonical-JSON configuration digest).
`append` additionally requires `command_id`, `expected_revision` (null for an
empty journal, otherwise its 64-character head hash) and `assessments` (raw
assessment objects, not caller-supplied authenticated submissions). No other
request fields are accepted. There is no submitted UID, clock, grant, risk,
configuration path, shell command or execution binding. The issuer provides
the snapshot reference to callers; request code never chooses its own trust root.

The CLI accepts at most 1 MiB of JSON on stdin, refuses duplicate keys and
nonfinite numbers, and never executes source instructions. Protected owner
paths must have no symlinks and no group/other write permissions; root-owned
sticky ancestors are allowed, but not as the final owner directory. Setuid
execution is refused. Owner directory contents must remain protected from
untrusted same-account processes; processes sharing the account can access that account’s protected files.
The local entrypoint also checks each accessed journal descendant; new subject
directories and locks are private even under a permissive process umask.

Success exits 0 with `tos_local_assessment_result_v1`, the owner snapshot,
`authentication: local-unix-account` and the journal `result`. A commit can
record rejection/defer/dispute without admitting the assertion. An exact replay
returns the historical receipt and **fresh** current admission separately.
Errors exit 2 with `tos_local_assessment_error_v1` and a nonreflective exception
class (`JournalConflict`, `AssessmentRejected`, `JournalBusy`, `PermissionError`,
etc.); source/configuration text is not echoed. Conflicts require rereading
current state; busy writers require retry, never deleting their journal.

`describe` takes exactly `{schema_version, operation, subject_id}` and returns
the current owner snapshot, subject and policy refs, trusted scope, journal
revision and freshly checked admission in `result.command_context` and the
usual journal result. `supported_operations` describes command grammar and retains
`grants_authority: false`; current execution depends on the selected owner
grant. A caller can use those exact refs
to construct `inspect` or `append`; it need not calculate the owner's snapshot
or guess file/JSON selectors. Read operations need no configured execution
profile: null is valid until an actual qualified writer binding exists.
`append` still requires the independent current execution profile and grants.

### Assessed form materialization

For an explicitly source-selected `tos_human_form_v1` with `content.kind` equal
to `source-copy` or `freeform`,
`describe` also discovers `materialize-form`. Its request has the same exact
fields as `inspect`, with `operation: materialize-form`; no proposed wording,
assessment, clock, permission or configuration path is accepted in the request.
This operation requires source-bound configuration (v2/v3 public source or
v4/v5 confidential owner source). It reads
the current source snapshot and the complete committed journal history once,
then calls the existing human-form materializer and assessment policy engine.
It performs no model call, journal append, source write or graph publication.

The selected form must be in the validated adjacent form set of its explicitly
selected subject, with exact source/set/form refs. Every content binding must
also resolve from explicitly selected source records; inline copies cannot
supply missing source dependencies. The complete subject is mandatory context
for this assessed lane. Source-copy additionally resolves the actual Claim or
metadata field catalogue: exact whole-field pointer, role and source-declared
language/script must match, and all field-specific context stays mandatory.
If that existing source-copy did not author a whole-subject binding, the owner
adds exact `owner:subject` context only in the assessed materialization. Field
guards remain required authored bindings; the extra context neither rewrites
the source form nor replaces any of its previous dependencies. Freeform still
requires its authored whole-subject binding. The complete packet remains bounded.
The set's exact retained `prior_forms`, read within the
same bounded source snapshot, support successor validation without treating
them as current assessment targets. The source-only writer/metadata reader
retains its separate ready-copy result with `admission: null`. Trusted template
admission is not expanded by this operation.

The adapter enables trusted `FormScope.require_current_assessment`; the adapter sets this internal owner input from its source-bound operation
contract. The same pure materializer then evaluates current form
admission for source-copy as well as freeform. V5 quality bases still come from
exact native closure and purpose under the existing target/dependency locks.
Their complete current payloads become mandatory `owner:quality:<index>` output
context and exact dependencies without changing authored form bindings. The
colon-separated slots cannot collide with authored binding names. Current
source limits and form limits are retained in the admission; the copied subject remains full context with its own separate admission state.

A changed current basis invalidates earlier dependent review evidence and
requires renewed review of the unchanged form. Existing explicit authored basis
refs remain exact: a stale binding is refused, not substituted with the new
owner context, even if a later assessment is otherwise usable. No inline basis,
submitted `can_use`, source rewrite or conversion to freeform supplies this
route. Freeform remains assessment-required even in the default pure scope.

V5 Claim forms also require the parent Claim's explicitly configured exact
scope: its real layer/maker, complete source-language closure, same requested
use and granted access. Its current admission is evaluated using its own exact
grounding and matching current quality dependencies under the same ordered
form/parent/layer locks. `describe` and materialization carry
`subject_assessment`: the current parent result/limits, observed journal head
and batch count, and historical withdrawals in the exact parent scope. That
observation participates in the expected owner snapshot and its head is rechecked
before return. No inline parent status can supply it.

The whole immutable Claim remains authored context; `subject_assessment` carries separately derived current context alongside the
unchanged authored `review_status`. A positive form admission permits qualified presentation of the Claim with its
actual state, including unreviewed, rejected, disputed or withdrawn. Consumers must retain both
the current parent state/limits and the form's distinct admission. Parent
withdrawal or dispute changes the observation and invalidates an older prepared
read; a fresh read exposes that state without inventing a new review or changing
wording. Layer-quality loss still closes the form's current-use gate. The
optional materialization schema companion is a closed-contract extension;
older strict readers must update or refuse it, never discard it. Existing
public freeform consumers retain their nonconfidential source route.

A source-form subject scope may additionally declare `form_language_context`:
an exact `{record, pointer}` binding or null. This is selected independently in
the protected owner configuration, not copied from the submitted form. A
non-null binding must refer to a selected source record other than the form;
the existing language-context schema, derivation source and binding checks
then apply. The owner still needs evidence for that linguistic declaration;
originality, translation and language competence require their own evidence.
Other subject scopes cannot carry this field.

The normal result adds `materialization` beside `revision`, `batch_count` and
`current_admission`. The latter is the same policy result carried by the form,
or null when structural/access/context validation prevented policy application.
Only a mechanically valid form with current scoped admission emits wording.
Missing assessment, revoked authority, withdrawal or a successor without a
current review emits no wording. The full-subject context and existing 64 KiB
materialization ceiling remain; the smaller public scene-selection budget requires its own consumer check. Source and journal snapshots, not an
old ready packet, govern each invocation.

This joins the local source/journal command to human-form output. The local
graph route below carries the result into the common reader; public snapshot
publication and runtime connection remain separate integration work.

### Local assessed graph candidates

The native owner command composes the maintained corpus index, bibliographic
claims graph, or both from one bounded repository capture. The explicit request
pins the current native assessment-read invocation by absolute path and SHA-256,
selects a bounded set of existing form IDs, and names a separate private JSON
candidate target. It never writes a standard export or admits source.

```bash
tos-native-owner-command corpus-assessed-candidate --request \
  /absolute/private/assessed-candidate-request.json
```

The request schema is `tos_native_assessed_candidate_request_v1`. Its
`projection` field is the same exact
`tos_native_corpus_projection_check_request_v1.projection` used by
`corpus-projection-check`; `native_invocation` contains `{path, sha256}` for a
protected `tos_local_native_assessment_read_invocation_v1` file;
`assessed_form_ids` contains at most 256 distinct IDs; `output` is an absolute
JSON path outside authored repository sources (or under `.git`). Choose
`product` as `bibliographic`, `corpus`, or `paired`, and `operation` as
`publish` or `check`.

`publish` stages a mode-0600 file and creates the target atomically without
replacement. After syncing the staged bytes and immediately before linking,
the command repeats the full native assessment batch and requires an identical
owner snapshot and reply set, rechecks the pinned invocation and captured
source, and then waits for the enclosing source-capture fence before committing
the target. A refusal removes its own staging file and any just-linked target.
`check` rebuilds the same candidate, compares the existing private file byte for
byte, and repeats the same currentness checks without creating directories,
temporary candidate files, or journal locks.

Use `paired` when one coherent candidate must carry overlapping metadata forms
in both products. It performs one assessment batch against one source capture;
forms present in both carriers must have identical form, subject, and source
bindings. Claim-only forms may appear in the bibliographic graph while the
paired corpus index remains ordinary for that selection. A form absent from the
selected product's own source carriers fails closed. Separate single-product
invocations remain separate transactions and can observe different snapshots.

These local research candidates retain their selected owner context. Public
export and publication require the corresponding review and owner decision. The
ordinary source-parity query reader intentionally does not load candidates as
standard exports. The route preserves the existing 64 KiB per-form materialized
packet ceiling, 256 KiB complete form-set limit, 1 MiB request limit, and 16 MiB
native assessment reply limit; narrow larger selections rather than truncating
them.

### Source-bound configuration v2

The protected source/journal input may also be used by the
[local assessed graph builders](#local-assessed-graph-candidates). This remains
separate from source modification and from public export or runtime admission.

`tos_local_assessment_owner_v2` retains the v1 fields and adds `source_root`
(protected absolute repository root) and `source_records` (at most 1,024
distinct `{path, record_id, origin_id}` bindings). Only explicit JSON/JSONL
metadata files under `ToS/source-witnesses/` are read; payload/local-content
paths, escapes and symlinks are refused. No directory discovery, network,
OCR, global graph build or source write runs. The unique source-file and
consumed-contract input budget is 8 MiB, each file selects at most 1,024 records, and inline plus
source-selected records share the existing 1,024-record snapshot bound.

The adapter understands the identity/version envelopes of corpus-record v1,
claim-packet v1, historical-claim v1 and human-form v1. Declared metadata
profiles (including historical records, Document and Letter) and the shared
`source-claims.jsonl` stream use the existing source-profile readers and their
exact registry/schema versions. A legacy schema name cannot bypass the
declared stream or metadata file's contract.

Finite public `history/**/historical-claims.jsonl` inputs retain their original
`tos_historical_claim_v1` schema and historical registry domains. They retain their original legacy adapters. Their historical subject and typed
object, or relative-date anchor, must be explicitly source-selected; these
exact dependencies also ground each selected adjacent HumanForm. The existing
orphan-form and exact current subject guards remain mandatory. Uncaptured
legacy records are readable: creation capture or a correction grant is not a
condition of source assessment. Evidence and counterevidence declarations keep
their actual schema and source qualification; this reader does not follow
their links or turn them into independently inspected evidence. Unknown
display extensions remain opaque; malformed known display v1 fields fail.
Historical contracts are charged before decoding through an invocation-local
bounded reader. Final raw rehash retains drift detection without charging an
already consumed input twice; other profile dependencies and native identity
reads still count against their existing budget. No source instances are
discovered to fill a missing historical dependency.

A JSONL record is selected by stable ID,
not line number; a human-form set selects only its current `forms`, never
`prior_forms`. Duplicate current IDs or selected bindings are errors. Source
payload fields, including unknown extensions and historical review fields,
retain the same JSON values without semantic promotion. Unknown
identity families cannot be selected through this adapter and remain in the
original source with an explicit unsupported-family error. Every endpoint of
a selected declared Claim must be selected explicitly in `source_records`;
inline records and corpus discovery cannot supply missing endpoints. Native
Corpus endpoints must satisfy their original schema, ID family and basename;
declared endpoints satisfy their metadata profile. Concrete inherited
domain/range, layer and visibility are then checked by the Claim profile.
Native `artifact-witness.json` v1/v2 and `composite-witness.json` v1 inputs use
their exact owner schema and actual identity field, retaining the unchanged
payload. Their schema digests join the source-bound snapshot. Typed endpoint
descriptors support declared Claim domain/range checks but never become
replacement source records. A legacy schema label cannot bypass either native
filename. Whole-corpus reference closure, provenance, rights and substantive source
assessment retain their own validation and review routes.

Claims and historical records must explicitly allow `public` or `public_metadata_only` visibility;
other or missing visibility requires a separately authorized adapter. A
source-bound claim's maker and assertion layer must agree with the configured
scope; a form binds its creator and the `human_projection` layer. Risk, use,
access and calibrated languages still belong to the trusted issuer. Inline
copies cannot shadow source-bound IDs. The issuer supplies each origin ID from the actual provenance of the source.

For declared Claims in `source-claims.jsonl`, the adapter also derives an exact
mandatory grounding set from the profile's identity endpoints. Each endpoint
must be cited in assessment `evidence` as support, challenge or context; merely
loading it into the snapshot is insufficient. A selected form of that Claim
inherits its Claim and the same endpoint grounding. Matching native TextUnit
views and layers are added for native-bound endpoints, using their exact full
binding and original origin. Unknown value fields and unrelated selected
records are not inferred dependencies. This closure covers the declared endpoints and native bindings. Other source
URLs and evidence references need explicit source selection and reading.
`describe.command_context.required_sources` exposes the exact required record
refs. Correction of an endpoint invalidates dependent use without rewriting
the Claim or prior assessment; an unrelated selected record does not invalidate
that assessment. Missing native reads still leave inspection possible but
cannot yield usable admission. An unrelated metadata-only native selection
does not block a Claim whose own complete grounding was read exactly.
A source form of a Claim requires that exact current Claim through a declared,
source-selected profile even when no other Claim is selected. An inline copy,
unsupported Claim family or non-exact subject reference cannot qualify it.

The owner snapshot binds the configuration, each exact source-file byte digest,
every selected full record, and consumed registry/schema byte digests. Source
and profile files together share the 8 MiB unique-input budget. Profile inputs
are ownership-checked and rehashed after resolution; observed drift is refused.
All source-bound commands recheck the protected configuration and selected
source snapshot at the journal lock, after blob creation before publication,
on replay, and before returning a current read. A concurrent journal-head
change also refuses that read. Unpublished blobs remain outside committed
history. In-read source modification is refused. This does not
make independently changing source files transactional: the issuer must keep
the agreed multi-file snapshot stable during the operation. A source change
invalidates an old command snapshot; a subject change also requires updating
its owner scope. Canonical record refs remain distinct from file-byte fixity.
`describe` exposes selected record refs, source paths, file digests and declared
origins, plus `source_contracts` with the exact consumed contract paths/digests,
not a second corpus body or permission to use the source text. Changing an
otherwise valid schema or registry invalidates the old command snapshot.

The integration test reads the real Jenseits Work, 1886 German Expression,
Work-to-Expression Claim and name form through this CLI, preserving empty
assessment history as `unreviewed`. Mutation/refusal checks use explicitly
temporary copies. This check verifies real source/command integration; the records retain their
unreviewed state.

Tests exercise the real CLI describe/inspect/error boundary and local command append,
replay, revocation, protected paths and adversarial requests with synthetic
review records. Real-language calibration and authenticated remote/model execution require
their own evidence.

### Native TextUnit return and assessment

`tos_local_assessment_owner_v3` preserves the v2 fields and adds exactly
`native_text_units`: at most 64 explicit `{binding, origin_id, read_scope}`
selections. `binding` follows
[`native-text-unit-binding.schema.json`](../../ToS/contracts/native-text-unit-binding.schema.json).
It identifies one frozen native packet and layer by path, native ID/version and
raw-byte digest, plus one segmentation, unit and ordered anchor sequence. It
also explicitly selects the Work/Expression/Edition/Item metadata of that
packet; no corpus search supplies missing dependencies. This adapter reads native packet v1 source-bearing units through their frozen
source bindings.

| `read_scope` | Read and command boundary |
| --- | --- |
| `metadata_only` | Validate native metadata closure; no content read. `describe` and `inspect` only. |
| `exact_public` | Additionally verify the exact UTF-8 representation, only when current recorded source/layer/packet rights gates allow public content. |
| `exact_owner_local` | Additionally verify private content under the protected selected unit scope's explicit `access_allowed: true`; no public permission follows. |

Every selected unit requires its own protected subject scope before any native
text is opened. The scope pins its exact derived record ref, source language,
selected segmentation maker and either `textual_observation` or
`linguistic_analysis`. Risk, research use, principal, competence and authority
retain the existing issuer/engine contract. Read permission alone does not
qualify a judgment. Each configured native selection must have an exact read
for `append`; metadata-only layer evidence cannot bypass that restriction by
being used for another subject. Supporting layers supply evidence; selecting a layer as an assessment target
requires its own subject scope. Existing ordinary source-form materialization
remains separate; a native unit requires its own explicit form adapter.

The read-only native resolver in
`rust/crates/tos-command/src/source_sign_native.rs` supplies separate metadata
and explicitly scoped exact-content reads to the native owner commands. Its
retained input snapshot binds the exact observed dependencies. It checks exact native schemas,
selected unit/segmentation membership and anchor order, text-layer ancestry,
editorial policy and declared maker configuration bytes, source-anchor/file
identity, Item manifest topology and recorded rights/publication dependencies.
It never reads the original Item payload, fetches a URL, executes a selector or
method, reruns OCR, or grants semantic, legal or publication authority.
Exact mode reads full representation bytes without universal-newline or
Unicode rewriting, then checks absolute Unicode-code-point half-open anchors,
span hashes, declared scope, coverage and gaps. Successful return verifies the selected metadata and exact representation
bytes. Original-payload fidelity, OCR/transcription quality and linguistic
correctness require their corresponding source-visible assessment.

The delivery owner in
`rust/crates/tos-command/src/source_native_text_read.rs` exposes
`read_native_unit` to the native access source reader; metadata resolution
remains text-free. It first verifies
metadata and public packet/layer/transport gates, then requires applicable
recorded redistribution `authorized` and derivative `allowed` postures before
opening content. Conditional rights are refused on this public route.
This read consumes the existing publication authority recorded in the source
closure.
The `tos_native_public_unit_return_v1` result contains the unchanged verification
summary, exact packet identity/version/digest, layer-record and representation
digests, opaque rechecked closure fingerprint, and ordered `spans`. Each span
has its native `anchor_ref`, unchanged absolute half-open Unicode-code-point
`selector`, `exact_sha256`, and exact `text`. Discontinuous anchors stay separate;
no inferred context, concatenation, Unicode/newline rewriting or partial output
is supplied. No content locator or supporting rights record is exported.
The complete compact UTF-8 JSON result must fit the requested limit (at most
1 MiB). Input bounds and snapshot refusal remain unchanged. Rights assessment, source admission and linguistic-boundary judgment remain
with their respective owner records. The optional access owner adapter consumes it
through an exact metadata handle and a separate `native_public_unit`
representation; generic metadata access cannot grant text permission.
Keeping delivery separate from the native binding resolver leaves the catalog's
verification processor unchanged when only the transport is extended.

The same native reader accepts a separately selected `LocalTextReadSelection`, whose protected-file
contract is `ToS/contracts/native-local-text-read.schema.json`. It is a current
owner configuration of existing local reading conditions, never a permission
inferred from a source text, expired construction delegation, or assessment.
Each finite selection pins the complete native binding, rights references,
reviewed conditions, license and attribution notices; issuer, account, source
root, mandate and at-most-one-day validity remain separately auditable.
No private representation is allowed. The local result retains all notices
within its bounded output; config/mandate/notice changes and expiry refuse the
read before return. This explicit route neither widens unconditional public reading nor
changes source rights, normalization, linguistic admission or publication.

For protected configuration preparation, the native `resolve_assessment`
and separately scoped owner-local route return two `records` and a text-free
`summary`. The canonical reference of `records[0]` pins the unit scope;
`records[1]` is its distinct native layer evidence. Do not inline either into
the v3 configuration: `native_text_units` selects them freshly on every call.
Both carry the issuer's same `origin_id`, not two independent sources.
The unit retains its native ID/version but its canonical digest describes
[`tos_native_text_unit_assessment_subject_v1`](../../ToS/contracts/native-text-unit-assessment-subject.schema.json):
the full unchanged packet, immutable binding, canonical layer ref, opaque
closure fingerprint and content-verification posture. The digest binds this complete derived assessment view; the packet retains its
native subject identity and separate raw-byte fixity. All packet fields survive;
the complete view must fit the existing 1 MiB Record limit. Metadata and exact
views deliberately have different assessment targets. Native historical review,
boundary and segmentation statuses are not rewritten by journal admission.

Each resolver bounds metadata and exact content separately at 8 MiB and 128
dependencies, with at most 1 MiB per metadata file and 16 predecessor levels.
A v3 command additionally shares a 16 MiB/128-distinct-file native budget
across all selections; native and ordinary records together retain the
existing 1,024-record and engine snapshot limits. Overflow is explicit refusal,
never partial evidence. These bounds limit native resolution work; consumers apply their own delivery
budgets.
No directory scan is performed for native-unit resolution. The ordinary v2
metadata-profile identity check, if selected separately, retains its own
documented bounded inventory and budget.

`describe` adds `command_context.native_text_units` (unit/layer record refs,
read posture and original native statuses) and `native_contracts` (public
schema paths/digests only). It does not return native packet bodies, source
text, private locators, rights inputs or short-span hashes. Opaque dependency
fingerprints bind those inputs inside the owner snapshot. This local owner response retains the selected disclosure scope; public export
requires its own clearance.

For v3 append the journal calls a snapshot guard after acquiring the subject
lock, before replay/evaluation, and again after writing an immutable blob but
before publishing the head. The guard rereads exact protected configuration
bytes, native dependencies and ordinary selected source inputs. A changed
packet, layer, rights gate, verified content or schema is a refusal. A failed
late check can retain an unreferenced blob but cannot publish a new history
head. The issuer must still keep the selected files stable: these checks do
not create a filesystem transaction or isolate hostile same-UID writers.
V1/v2 commands preserve their existing snapshot and replay contracts.

Native conformance cases in `tests/conformance/rust/command_text_cases.rs`,
`command_owner_text_cases.rs` and the access `source_read_native` integration
test separate synthetic
closure/admission checks from real source-visible review. No private native
packet is made public by this adapter, and no human-only historical record is
relabeled as an agent act.

### Explicit owner-local source transport

The native `source_text_owner::OwnerTextContext` reads the protected
`tos_owner_local_source_context_v1` configuration from an independently chosen
mode-0600 file. Its fields are `store_id` (`sid-` plus 32 lowercase hex digits),
`public_root`, `private_root` and `private_prefix`, in addition to
`schema_version`. Roots are absolute, normalized, existing, non-symlink and
disjoint; the prefix is exactly
`ToS/source-witnesses/owner-local/<store_id>/`. The private root must already
exist with mode 0700. The resolver does not create directories or select a
store on behalf of the caller.

The native owner commands select this context through the explicit invocation
and an independently protected owner configuration. The same native binding
keeps its source IDs and schemas. Metadata selection confers no exact-content
permission; the private reader requires a separately selected current grant.

The native transport selects exactly one physical root for each complete logical
reference. It retains file and directory identities and checks confidentiality
before and after each bounded read. Private files require 0600 and private directories 0700;
the root's ancestors also retain no-follow/account/write protections. A private
schema copy is never selected instead of its source-owned public contract.
There is no fallback, other-store discovery or permitted alias in the checkout.
The opaque snapshot binds context/schema bytes, physical root roles and root
identities as well as native dependency bytes. Default public snapshots do not
consult this context, and default native reads reject its reserved namespace,
including metadata-only representations that name a private content locator.

Any consumed private route sets an owner-local disclosure ceiling even when
the underlying layer is otherwise public. `owner_local_transport` reports
that boundary without exposing paths; exact private content still needs its
separate read selection. The explicit native and private profile writers below
use this transport interface; v1-v3 assessment command configurations and
public source/Claim/form commands do not implicitly acquire private-source
support. Each future writer/consumer needs its own explicit adapter.

### Confidential native TextLayer and first segmentation

The [native TextLayer construction contract](#private-native-textlayer-and-first-textunit-construction)
owns the extraction and derivation grants, first segmentation, exact source
bindings, budgets and recovery. The public Occurrence gate and
source-visible assessment remain separate owner routes.

### Public project-text native construction

This independent adapter implements bounded public source construction through
the `public-project-native-text-create` handler in
`rust/crates/tos-command/src/source_native_public_text_cli.rs` and the native
`source-commands` front door. Its closed, source-owned input contracts are
`ToS/contracts/public-native-text-create-owner.schema.json` and
`ToS/contracts/public-native-text-authority.schema.json`. Request discovery
uses the common grant-free handler grammar; the adapter requires its dedicated
public-construction grant.

#### Scope and authority

The protected current-account mode-0600 grant names one source root, disjoint
mode-0700 recovery root, new exact package path, maker, expiry, explicit
Work/Expression/Edition/Item/File records, Item manifest, existing authored
UTF-8 documentation bytes and code-point range, native identities, rights,
license evidence and retained operator-scoped publication authority. Only
`ToS/review-ledger`, `ToS/doctrine` and `docs` source selections are supported.
Reserved private, payload, local-content, catalog and hidden paths are refused.

The source must already have been retained by normal corpus commands. The
original Item rights remain mandatory inputs and are not rewritten. Every
output-bearing rights record must affirm the exact new layer/representation
File, public payload visibility, redistribution and derivation, with retained
license and publication-authority evidence. Denied, inactive, missing, foreign
or drifted evidence fails before source text is opened. Parsing does not
authenticate authorship or manufacture licensing: the source-owned authority
records a real operator delegation and exact authorized output scope.

#### Construction and downstream use

`describe` does not open source text. `prepare-create` returns exact prepared
configuration/dependency hashes and native bindings. `native-text.create`
requires those hashes, null expected prior source/revision and an exact
command ID. It constructs a literal UTF-8 range without newline, Unicode,
markup, spelling or punctuation rewrite; the source selector returns to the
original acquired File. The new layer remains an unreviewed machine
transcription with structural-extraction provenance. A separately supplied
finite partition covers the new representation with units and explicit gaps;
it does not infer language units or semantic significance.

The native resolver verifies the new public binding and exact content. A
separate `source.create` can then create an Occurrence with its own identity
and native binding. HumanForm creation, assessment, admission, canon and
external publication follow their own operations. Existing private historical
inputs remain private.

#### Persistence and limits

The common no-replace native package installer retains the exact request,
public plan (not the protected grant), runtime, inputs, execution provenance,
native bindings and receipt. The public plan omits the grant's source/recovery
root fields and local UID; the protected grant is represented only by its
digest. This is not a general redactor for authorized authored source text.
Up to 128 KiB source and output, 60 seconds, 128 distinct input files,
16 MiB input bytes, 12 output files and 2 MiB output bytes are allowed.

Source and recovery locks, no-follow reads, current authority/rights/input
rechecks and atomic new-directory publication protect commit. Exact retries
return retained bytes. `inspect-recovery` addresses one command and compares
an installed package with its retained plan before reporting committed. An
interrupted exact plan can resume through the same create request; torn or
foreign evidence is preserved and refused, never automatically deleted or
overwritten. The shared installer preserves original private writer controls.

#### Bounded PostgreSQL route

The ordinary `tos-native-owner-command source-commands --invocation ABS`
front door also accepts `tos_local_pg_public_text_invocation_v1`. Its exact
top-level fields are `schema_version`, `owner_config`, `native_executable`,
`native_executable_sha256`, `source_selection`, `software_capture`,
`software_restored_root`, `software_selection`, `software_components`,
`schema_worker`, and `budgets`. The invocation and create-request shapes are
captured in the [invocation](../../rust/crates/tos-command/tests/fixtures/source-public-text-pg-v1/invocation.example.json)
and [create request](../../rust/crates/tos-command/tests/fixtures/source-public-text-pg-v1/create.request.example.json) examples.
The zero digests and absolute paths in those examples are placeholders; select
the actual executable, capture, worker, source contract, store and current job
lease. Keep the invocation and protected owner configuration readable only by
the local owner account, and supply PostgreSQL 16 credentials privately via
`TOS_SOURCE_PG_URL`.

`source_selection` repeats the existing CMD `domain`, source contract digest,
private STO root and limits, plus an existing `job_id` and positive
`job_fence`. The route reads authored `ToS/` inputs from that selected PG/STO
epoch. A `docs/` source reference is selected separately through the exact
software capture and is rechecked before commit; it does not widen the CMD
readset. `software_selection` binds the exact capture commit, tree and manifest
digest, and `software_components` selects the implementation closure. The
schema worker and finite `budgets` use the ordinary PG source-owner profile.
`describe` selects only the three owner-grant schemas; construction and recovery
select the ten schemas consumed by public authority, endpoint, binding, and
provenance validation. Delegated Text IDs use batched point observations in the
PG Text identity index, with one fenced absence or exact-owner generation per
requested ID; the route does not walk the whole Text metadata namespace.

Send a `tos_local_source_command_v1` request on stdin. `describe` and
`prepare-create` are non-mutating. Then send `native-text.create` with a fresh
`command_id`, the exact `expected_configuration` and `expected_dependencies`
returned by `prepare-create`, and null `expected_source` and
`expected_revision`. `inspect-recovery` accepts only an exact retained command
ID. The PG writer commits all twelve package members as one source revision.
See the `tos-command` crate README for the CLI entry and fixture test.

#### Verification boundary

`test_source_public_native_commands.py` covers exact public native resolution,
distinct Occurrence validation, Unicode/newline preservation, no path leakage,
grant/rights/authority rejection, dependency drift, identity/partition errors,
precommit changes, interrupted resume, torn evidence and installed corruption.
Fixtures are synthetic mechanics evidence, not real corpus or rights review.
Existing discovery, private construction and pure proposal suites remain the
regression surface. Real source-visible judgment and retained operator scope
must be reviewed separately from green tests.

### Confidential native TextUnit creation

`tos-native-owner-command source-commands` dispatches the separately selected
`tos_local_text_unit_create_owner_v1` configuration to
`rust/crates/tos-command/src/source_text_unit_entry.rs`. Its operation is `text-unit.create`, producing one native proposed TextUnit
packet. Semantic descriptions, Occurrences, Lexemes, forms and segmentation
assessment use their own source routes. The output uses the existing
[`source-text-unit-packet-v1` schema](../../ToS/contracts/source-text-unit-packet-v1.schema.json).
The common native resolver can read the new packet immediately through the
same owner-local context and exact binding contract.

The protected mode-0600 owner configuration contains:

- `schema_version`, current local `uid`, `principal_id`, `authority_ref`,
  `expires_at`, and exactly `allowed_operations: ["text-unit.create"]`;
- independently selected absolute `source_context_ref`, and `source_path`
  within its private prefix, ending in a **new** package directory followed by
  `source-text-unit.v1.json`; its parent exists already with mode 0700;
- `source_binding`, the existing exact native binding; separate `source_access`
  with `read_scope: "exact_owner_local"`, `access_allowed: true` and its own
  `authority_ref`; neither the context nor a read grant grants derivation;
- `allowed_text_scope: {start, end}` in absolute Unicode code points within
  the selected existing contiguous native unit, not the whole file by default;
- opaque `packet_id`, `scheme_id`, `segmentation_id`, `scope_anchor_ref`,
  `unit_slots: [{unit_id, anchor_ref, unit_kind}]` and `gap_anchor_refs`;
  labels, offsets, text and ordering do not generate these identities;
- `scheme: {scheme_name, analysis_role, boundary_basis, policies}` and the
  existing native `method` object, plus `provenance_event_id`. The method's
  agent equals the principal, its event equals that delegated event, and its
  `configuration_ref` names the new package's
  `source-create-owner-configuration.json`. `synthetic_fixture` cannot claim
  a real `source_bound` method. The writer is a software executor; it does not
  impersonate the declared author of the segmentation method.

Use the same installed source-command entrypoint with the protected native
invocation. First submit `describe` in the `tos_local_source_command_v1`
envelope to discover delegated slots, scopes and request fields. Submit
`prepare-create` with the selected `spans` and `excluded_gaps`, then submit
`text-unit.create` with those same inputs, a command ID and the returned
`owner_configuration` and `expected_dependencies`. The initial
`expected_source` and `expected_revision` are null. Each span names its
`unit_id`, `start`, `end`, `certainty` and `status_reason`; each excluded gap
names its `anchor_ref`, `start` and `end`.

`describe` reads no source text. Preparation and application resolve the exact
metadata closure, check the recorded local-derivation rights gate **before**
reading text, then verify unchanged UTF-8 bytes without newline or Unicode
rewriting. A current exact-layer decision may be narrower than the aggregate
Item gate; a Work-wide positive statement does not lift that aggregate gate.
Inactive, denied, conflicting, unknown, permission-required or conditional
derivation routes cannot be cleared by a submitted read grant. This initial
writer supports `local_research_only` and unconditional `allowed`; conditional
use needs an explicit owner use decision, not an inferred satisfaction of terms.

The request supplies 1–256 ordered, nonoverlapping, positive-width spans, using
every delegated unit slot, and at most 257 gaps. Each confidence object has
the native `value` and maker-confidence `meaning`, never truth probability.
Gaps must be the exact explicit complement of the spans within the scope.
Booleans are not offsets; omitted coverage, overlap and undelegated IDs fail.
The pure constructor preserves source scope, layer and rights refs, computes
all exact slice hashes itself, and emits version-1 proposed units and
segmentation with empty review/projection histories. The packet is local-only
and never publication-authorized; stronger source restrictions survive.

The atomic mode-0700 directory contains the packet, retained owner
configuration, original request, runtime environment, native-aware provenance
event and the existing `tos_local_source_create_receipt_v1` envelope. Files are
0600. Dependencies bind the resolver/context closure, native identity
membership and bytes, implementation and contracts. Native owner discovery
covers prefix- and suffix-named text-unit/anchor JSON plus anchor and provenance
JSONL, excludes payload/local-content/catalog and pending directories, and
refuses aliases. It is bounded by 32,768 directory entries, 2,048 files,
64 MiB total, 32 MiB per native metadata file, 1 MiB per JSONL record and
65,536 top-level records (legacy text-unit JSON packets can exceed 1 MiB);
exceeding a limit is explicit refusal, never a truncated identity check.

Publication uses existing source-owner locks, protected private staging and
atomic no-replace rename. It rechecks source/delegation dependencies and exact
staged bytes before publication. Exact retry verifies every retained byte and
receipt binding before excluding its own package from collision discovery,
then freshly checks source and rights. It reproduces the exact original packet
and compares current dependency snapshots before returning. A competing
directory, corrupt history or altered output is never overwritten. Historical
retry is distinct from requiring every previous input byte to remain current;
see the shared replay contract below.
Unpublished process-loss staging remains outside source discovery and is not
accepted as a completed transaction. The issuer still owns source stability
against noncooperating external edits; this is not a cross-filesystem transaction.

This route creates and replays immutable native packets. Native revision,
source-layer construction, private descriptions and forms, assessment and
public projection retain their own explicit contracts; no public collector gains access to the private store.
Pure-constructor and command/CLI tests use synthetic evidence to verify
operation behavior; historical and linguistic acceptance require actual source
review.

### Confidential native translation alignment

This confidential native command route belongs to the existing
[`translation-alignment`
owner](../../ToS/contracts/translation-alignment-packet-v1.schema.json).
Its additive [`native record
contract`](../../ToS/contracts/native-translation-alignment-record-v1.schema.json)
reuses the v1 mapping, side, maker, evidence, granularity, rights and
authority definitions. The earlier packet retains its human-review and
public-use semantics; the native record carries an unassessed proposal.

#### Identity and exact history

An Alignment is a stable opaque subject, independent of its text labels and
current mapping. It has `alignment_id` and no independent version counter.
`record_id` identifies its descriptive record lineage; `record_version`
versions the whole immutable description. `claim_id` identifies one proposed
correspondence; `claim_version` versions its descriptive qualifications.

| Change kind | Record | Alignment subject | Claim |
| --- | --- | --- | --- |
| `initial` | New ID, version 1, no predecessor | New ID | New ID, version 1 |
| `describe` | Same ID, next version | Same ID | Same ID, next version; mapping unchanged |
| `remap` | Same ID, next version | Same ID | New ID, version 1; mapping actually differs |
| `competing` | New ID, version 1 | New ID | New ID, version 1; explicit alternative record refs |

Every descriptive successor has an exact predecessor
`{record_ref, record_id, record_version, sha256}`. The digest addresses the raw
record file, including its serialization. Its Claim predecessor is
`{claim_id, claim_version, sha256}`, with the digest computed over the
source-command canonical JSON serialization of the Claim **inside that exact
predecessor record**. An inline supplied predecessor Claim is never trusted.

Describe cannot change either native source selection, source side,
granularity, inherited rights, competing references or mapping. It must change
a descriptive qualification; a capture timestamp/event alone is not a delta.
Remap also preserves the two exact source scopes. A source-scope change is not
an ordinary descriptive revision: select a distinct source-bound proposal and
obtain the appropriate owner judgment rather than silently repointing history.

Competing links are one-way exact source refs in the newly authored record;
the old immutable alternatives retain their bytes. A reverse edge can be
derived from explicitly supplied records. Global reverse indexing has its own
projection route. Competition and supersession record relationships among
versions; preference requires source-visible judgment, and multiple
alternatives may remain unresolved.

The existing bounded source identity walk checks both selected public and
private metadata homes. A new version may reuse only IDs in its exact
predecessor chain; an already present sibling/later version prevents stale
forking. There is no separate identity catalog or mutable alignment-head file.
Verified historical replay can read its already-owned version after a later
version exists; it cannot use that exemption for a fresh write.

#### Source and proposal boundary

Each side is an ordered selection of existing native TextUnits from one exact
packet and segmentation. Every binding retains packet ID/version/raw digest,
segmentation ID/version, unit ID/version, ordered anchors, exact TextLayer
record ID/version/raw digest, and Work/Expression/Edition/Item owner refs.
The original File, UTF-8 representation, all selectors and anchor digests are
resolved through those existing native contracts. Source and target are
distinct Expressions, not merely two layer versions of one Expression.

The owner explicitly declares granularity and whether each selected frozen
segmentation also supplies tokenization. Phrase/token/morpheme/mixed
granularity requires both tokenization bindings. Alignment granularity,
accepted tokenization and linguistic quality retain separate declarations or
assessments. Unit selection supplies bounded context for one mapping;
omission/addition records identify its unaligned members.

Both side metadata and current recorded local-derivation rights are checked
before either representation is read. Current independent read/proposal grants
are checked again around each resolver read. UTF-8 bytes retain their exact
newlines and Unicode form. No tokenizer, aligner, model, network, OCR engine or
translation generator is called. Both source closures are retained as exact
input bindings, not reconstructed from labels or strings.

Mapping shape/cardinality, omissions/additions, reference and evidence closure,
technique combinations and rights use the complete existing v1 schema and
mechanical validator through a temporary, non-published validation view. V1's
ID-only lineage and in-packet reciprocal competition are not fabricated in
that view: the native wrapper checks exact immutable history separately.
Monotonic mappings preserve each native side's anchor order. `reordered`
retains the explicit proposed ordering; it is not an independently assessed
cross-record translation-order verdict. Rights are the strictest side or local
packet posture; all outputs remain private and publication unauthorized.

Supplied techniques, uncertainty, rationale and maker attribution retain
proposal status. The wrapper is `proposed` and
`unassessed_translation_proposal`. Provenance uses an annotation/capture event
and records supplied mapping alongside verified source bytes. Upstream aligner
execution requires its own evidence. An authorized competent reviewer assesses
translation through bilingual source-visible comparison, the required
independent baseline and a scoped assessment route. Quality, semantic use,
publication and canon each require the corresponding owner decision.

#### Independent configuration and requests

The common source front door selects
`tos_local_native_alignment_owner_v1` through
the native `owner-local-native-translation-alignment` handler in
`rust/crates/tos-command/src/source_text_alignment_entry.rs`; discovery advertises
`owner-local-native-translation-alignment` without reading any grant or source.
The protected mode-0600 configuration contains exactly:

- `schema_version`, current local `uid`, `principal_id`, `authority_ref`,
  `expires_at`, the independently selected absolute `source_context_ref`, and
  private logical `source_path` ending in a **new** package directory followed
  by `native-translation-alignment.v1.json`;
- `allowed_operations: ["alignment.create"]` for `initial`/`competing`, or
  `["alignment.revise"]` for `describe`/`remap`; the request cannot choose a
  different change kind;
- separate `source_access: {read_scope: "exact_owner_local", access_allowed:
  true, authority_ref, expires_at}` and `alignment_access:
  {derivation_allowed: true, authority_ref, expires_at}`;
- opaque `record_id`, `alignment_id`, `claim_id`, a fresh `provenance_event_id`,
  `change_kind`, exact `predecessor` or null, and `competing_records`;
- `native_bindings: {source: [...], target: [...]}`, `granularity`, and explicit
  `tokenization: {source: boolean, target: boolean}`;
- v1 `maker`, with `maker_kind: "imported_source"`, principal-bound
  `agent_ref`, supplied `made_at`/method, the delegated event ref and
  `method_output_posture: "proposal_not_truth"`. This attributes supplied
  material without impersonating its alleged upstream producer.

The existing mode-0700 private parent and source context are prerequisites.
The request uses the existing `tos_local_source_command_v1` envelope. The
following is shape-only pseudocode: angle-bracket values are declared
placeholders and must be replaced with values valid under
`native-translation-alignment-record-v1.schema.json` before transport. The
prepare operation is `prepare-create` for an initial/competing record and
`prepare-revise` for a describe/remap successor.

```text
{
  "schema_version": "tos_local_source_command_v1",
  "operation": <"prepare-create" | "prepare-revise">,
  "mapping": {
    "direction": <"source_to_target">,
    "correspondence_shape": <"one_to_one" | "one_to_many" | "many_to_one" | "many_to_many" | "source_omission" | "target_addition" | "unresolved">,
    "order_posture": <"monotonic" | "reordered" | "not_applicable" | "unresolved">,
    "ordered_source_anchor_refs": [<schema-valid anchor id>],
    "ordered_target_anchor_refs": [<schema-valid anchor id>]
  },
  "qualifications": {
    "translation_techniques": [<schema-valid technique>],
    "epistemic_status": <schema-valid epistemic status>,
    "certainty": {
      "value": <number from 0 through 1>,
      "meaning": "maker_declared_uncertainty_not_truth_probability"
    },
    "status_reason": <non-empty string>,
    "evidence": [<schema-valid evidence object>]
  }
}
```

Submit the prepared request through the native selected invocation:
`tos-native-owner-command source-commands --invocation /absolute/native-invocation.json`.
The apply request uses the same envelope. Its `mapping` and `qualifications`
values are the same schema-valid objects shown above. This block is likewise
shape-only pseudocode, rather than executable JSON:

```text
{
  "schema_version": "tos_local_source_command_v1",
  "operation": <"alignment.create" | "alignment.revise">,
  "mapping": <the schema-valid mapping object above>,
  "qualifications": <the schema-valid qualifications object above>,
  "command_id": <fresh command id>,
  "expected_configuration": <prepared owner configuration digest>,
  "expected_dependencies": <prepared dependency digest>,
  "expected_source": null,
  "expected_revision": null
}
```

The absent source/revision precondition describes the **new immutable version
package**, not the absence of a predecessor. That predecessor is separately
fixed by the protected configuration and the output's exact history.
`mapping` contains the five existing direction/shape/order/source-anchor/target-
anchor fields. `qualifications` contains existing `translation_techniques`,
`epistemic_status`, `certainty`, `status_reason` and `evidence`; request fields
cannot supply a maker, rights, source selection, identity, executable or review.

#### Inspection, replay and recovery

`describe` reads contracts and protected configuration. `inspect` checks the
exact delegated target, retained receipt and current metadata/source closure.
`inspect-version` takes `source` in the exact native record-ref shape and
accepts only a version in that selected history. Both return metadata-only,
redacted verification/version summaries; source strings, locators, selectors,
private IDs and short-span hashes remain withheld. Source-visible comparison
uses the separate assessment route.

An identical apply request replays the exact immutable package. Request,
configuration, source inputs, implementation/runtime pins, source receipt and
stored bytes remain checked. Changed source bytes, scope, current rights or
implementation cannot be accepted merely because an old receipt exists.

The existing shared/public-then-private writer lock order and retained native
construction primitive publish a flat mode-0700 package with mode-0600 files.
Exclusive writes and atomic no-replace directory publication preserve prior
versions. A durable exact plan precedes output writes. `inspect-recovery` with
the original `command_id` checks the retained plan; retrying the exact original
apply request fills only absent staged files. Torn files, a missing/torn plan,
foreign residue or changed authority fail closed and remain for owner review;
this route does not delete or overwrite recovery evidence.

Bounds include 256 units per side, 32 explicit competing records, at most 64
logical history steps, the shared native resolver's 128-input/byte budgets and
a 1 MiB canonical record ceiling and 60-second cooperative command deadline. The file/dependency budget may reject
a history before the logical depth bound. These are fail-closed limits, not a
complete-corpus mode or a preemptive filesystem/parser timeout.

The focused synthetic tests belong to `mechanics/growth-cycle/tests/` and the
existing `mechanics_local` lane. No real corpus bytes, bilingual assessment,
external acquisition, KAG projection, release or runtime installation is
established by their success.

### Confidential owner-local source profiles

`tos-native-owner-command source-commands` selects `rust/crates/tos-command/src/source_private_profile.rs` only for the
independently protected `tos_local_owner_profile_command_v1` configuration.
It does not relax the public source writer or catalog. The selected record is
`local_only`, uses its existing `semantic-metadata-v1` profile/schema, and lives
under the context's exact private logical prefix. The existing parent metadata
directory must be mode 0700; new flat packages and files use 0700/0600.

The configuration has these exact fields:

- `schema_version`, local `uid`, `principal_id`, `authority_ref`, `expires_at`;
- `source_context_ref`: independently selected absolute mode-0600 context path;
- `source_path`: private logical path ending with the profile's typed basename;
- `profile_type_id`, `record_id`: the declared semantic profile and stable subject;
- `source_access`: `{read_scope, access_allowed, authority_ref}`;
- `source_binding`: the complete canonical native binding for an Occurrence,
  or `null` for a profile without a native adapter;
- `allowed_operations`: a subset of `source.create`, `record.revise`,
  `form.create`, `form.revise`;
- `allowed_fields`: a subset of the common descriptive revision fields;
- `allowed_form_ids`: up to 32 exact source-form IDs;
- `provenance_event_id`: the delegated creation serialization event.

An optional `native_text_read_profile` field on the protected
`tos_local_native_source_invocation_v1` selects an independent
`tos_native_private_text_read_v1` grant for this private Profile owner. It is
valid only with this exact owner family and a native binding. The read grant's
owner-context path and source root must match the independently selected
private Profile context. Its exact binding, rights and local-only custody are
checked before content. The Profile snapshot binds the exact grant and grant
schema digests; current expiry, mandate and held source bytes are checked again
by the existing creation/replay guard before atomic publication. This explicit
selection reads binding metadata and support through that owner context;
schemas remain in the selected immutable source cut. Without the selection,
public-logical metadata remains in the cut. Missing inputs never fall through
to another transport, and dependency snapshots distinguish the authorities.
The selected legacy DTA adapter reads content through its protected exact body
mapping without changing the issued source return. No source text is written
to a receipt. The public Profile, public
compiler and public projections are unchanged. `describe` of an existing
private DTA-bound Occurrence requires that same independently selected read
grant; direct unit disclosure remains the separate `private-text-read` route.
The independent grant's three code dependencies (`source_native_text_read.rs`,
`source_sign_native.rs`, `text_rules.rs`) must be present in the exact selected
software capture as well as the established private Profile implementation set.

An Occurrence requires `exact_owner_local` source-read scope and the same full
native binding in its record. The command checks current local-derivation
rights before reading representation bytes. A non-native semantic profile
uses `metadata_only` and `source_binding: null`; adding a binding field does
not invent a native adapter for it. Record ID, native binding, scope/continuity
criterion and visibility cannot be changed by ordinary `record.revise`.
Identity transitions and publication need their actual owner routes.

Use the same `tos_local_source_command_v1` envelope on stdin:

```bash
/absolute/installed-prefix/bin/tos-native-owner-command source-commands \
  --invocation /absolute/private/native-invocation.json < /absolute/private/request.json
```

The discoverable operations are:

| Operation | Input and result |
| --- | --- |
| `describe` | Current source/form selectors, allowed operations, exact package revision and dependency snapshot; an absent target returns no source wording. |
| `prepare-create` | `record` and 1–32 `forms` selections `{form_id, field_id}`; returns proposed source, files, materializations and expected dependencies. |
| `source.create` | The prepared record/forms plus command ID, exact expected configuration/dependencies and null expected source/revision; creates one new package, never overwrites a competing directory. |
| `prepare-revise` | `fields`, `forms`, authored `reason`; returns the proposed successor and expected dependencies. Every current form must be explicitly rebound. |
| `record.revise` | The proposal plus command ID and exact expected configuration, source, package revision and dependencies; exchanges the whole package after retaining its exact predecessor. |
| `prepare` | One delegated `form_id` and discovered `field_id`; returns a version-bound common form change. |
| `apply` | Bounded `changes` and the same exact expected configuration/source/package/dependencies; changes forms only, retaining prior forms and common growth receipts. |
| `inspect-version` | An exact previous `source` ref from committed record history; returns its unchanged record and byte-bound private archive file locators. |

Creation stores the source, source-copy forms, protected owner configuration,
request, environment, annotation-serialization provenance and the existing
`tos_local_source_create_receipt_v1`. It records the actual source transaction and its serialization provenance. Revision uses the shared `tos_source_revision_history_v1` and
`tos_source_package_archive_v1` contracts. Its archive
reference starts with the selected private prefix followed by
`.record-revisions/`; merely changing the physical root of a public archive
reference would select the wrong owner and is refused.

Common form source/context bindings include the entire native return and
semantic scope. Source-copy forms remain mechanically ready, unassessed and
unadmitted; other supported form production modes remain unassessed proposals
until the assessment owner handles them. Private materializations cannot use
the public metadata renderer or public catalog reader. Results explicitly
state `visibility: local_only`, `publication_authorized: false` and
`grants_admission: false`.

One public-source lock followed by the selected private-store lock coordinates
these writers with native creation. Discovery scans only bounded public and
selected private metadata identity homes, excluding payload, local content,
catalogs, archives and staging. It reserves source/native subject, form and
creation provenance identities. Exact retry still checks current delegation,
source/rights/schema/configuration and package integrity; an old receipt is
not a current permission. Source-copy history, creation, revision and display
use the selected context's freshly read form, form-set, template and assessment
schemas, including after another renderer has warmed its runtime-global cache.
The same pure grammar compiler serves v4; public defaults are unchanged.
Atomic rename/exchange has no non-atomic fallback.
Interrupted unpublished staging is not a current source and a retry does not
silently delete that earlier invocation's evidence.

The common 64-file/8-MiB package, 2-MiB file, 1-MiB source/request and
128-record-correction budgets apply. Identity discovery is bounded to 32,768
directory entries, 2,048 selected metadata files and 64 MiB, with at most
32 MiB per older metadata file. Discovery cost follows the bounded metadata scan; transactions cover the
selected package and assume cooperating processes under the same account. The separate Claim writer
below and v4 assessment keep their own delegation. No public projection is created.

### Confidential source Claim growth

`tos_local_owner_claim_command_v1` selects `rust/crates/tos-command/src/source_private_claim.rs`
through the same `tos-native-owner-command source-commands` CLI and `tos_local_source_command_v1`
request envelope. It creates or corrects the explicitly selected private Claim package. The source context must independently
select the existing mode-0700 parent
`<private_prefix>/claims/`; the target is one new named child ending with
`source-claims.jsonl`. Its source, forms, requests, receipts and archives remain
mode 0600 inside mode-0700 directories.

The exact configuration fields are:

- `schema_version`, local `uid`, `principal_id`, `maker_type`, `authority_ref`,
  `expires_at`, `source_context_ref`, `source_path`, `provenance_event_id`;
- `allowed_operations`: a subset of `claims.create`, `claim.revise`,
  `form.create`, `form.revise`;
- `allowed_claim_ids`, `allowed_subject_refs`, `allowed_object_refs`,
  `allowed_predicates`, `allowed_evidence_refs`, `allowed_form_ids`;
- `allowed_fields`: a subset of the common Claim correction fields
  `qualifiers`, `evidence_refs`, `counterevidence_refs`, `alternative_claim_refs`,
  `supporting_quotes`, `epistemic_status`, `confidence`;
- `claim_selections`: exactly one independent selection for each delegated
  Claim: `{claim_id, relation_type_id, origin_id, source_access, source_records,
  native_bindings, verify_content}`. The source/native selectors are the same
  typed selectors used by the private Claim reader and v4 assessment below;
  no inline endpoint body or inferred type is accepted.

An explicit `source_records` selector may also name a native Corpus metadata
endpoint with `profile_type_id: tos.entity.<kind>` for Agent, Place,
Organization, Work, Expression, Edition, Collection or Item. This uses the existing `tos_corpus_record_v1` grammar and identity mapping.
The exact public metadata path must end in `<kind>.json`; `record_id` must
match its actual typed identity, `source_binding` must be null and access must
be `metadata_only`. Private Corpus aliases and visibility overrides are
refused. Full metadata, its version and freshly read schema enter the frozen
dependency closure; declared Corpus language, variant-label languages and
field languages enter assessment language scope. Linked records, Item
manifests and payloads are not followed. A TextUnit's native binding does not
implicitly select its Work or Expression as a Claim endpoint.

Claim and form scopes contain at most 32 identities; subject, object and
evidence scopes at most 128. The selected relation must have an understood
`semantic-relation-v1` or `identity-relation-v1` source profile. Temporal and
structured values, identity endpoint replacement and assertion-layer
transitions are not aliases for this correction route. Supported semantic predicates use the source-owned registry and profile
contract. Initial Claims are version 1, `local_only`, `unreviewed`,
without assessments or supersession. A correction preserves identity,
endpoints, predicate, maker, layer, visibility and all unpatched fields.

`tos_local_owner_claim_command_v2` adds required `allowed_object_values`
(at most 32 distinct exact JSON objects) and may include `object` in
`allowed_fields`. It additionally understands `structured-reference-value-v1`;
v1 does not acquire this reader through a registry change. Each declared
member must independently occur in `allowed_object_refs`, including the focal
subject when it also plays a member role. The subject and predicate remain
immutable; adding or removing members replaces the qualified value while retaining the
governing Claim identity. The exact next value must be allowed independently.

For this new reader only, a Claim's independently supplied `source_records`
and `native_bindings` may cover the finite union of present and proposed
closures. Every selector is permission-preflighted before private IO. Each
candidate or current Claim then receives only its exact declared identity and
evidence closure; unused allowed sources are not read or invented as evidence.
The full allowlist remains in the configuration digest. Assessment v4 still
requires its own exact stored-Claim selection, not this growth allowlist.
Retries of retained revisions revalidate both the current Claim and the
historical result of the request, so later removal of a member cannot hide
revoked access to that member. Original receipts and source history remain
unchanged. Temporal and ordinary structured-value writers are not added to
this private v2 contract.

Native grounding requires `verify_content: true` and independently delegated
`exact_owner_local` access for every selected native source. A non-native
source selector remains `metadata_only`. Each Claim's native closure checks
rights before reading its exact representation; a denial is not repaired by
the write grant. Quoted anchor identities also require an independent
`allowed_evidence_refs` entry. Source-visible assessment checks the authored quotation and its interpretation
against the exact read.
Alternatives in this bounded writer must name another Claim in the same
explicit batch; arbitrary existing Claim refs are not silently resolved.

| Operation | Input and result |
| --- | --- |
| `describe` | Bounded current Claim/form selectors, exact package revision, dependencies and allowed operations; an absent package has no source wording. |
| `prepare-create` | `claims` (1–32) and `forms` (`{claim_id, form_id, field_id}`, 1–32 total); returns proposed refs, file bindings, source bindings and materializations without creating a source file. Each Claim requires its complete `claim.statement` source-copy form. |
| `claims.create` | Prepared Claims/forms plus command ID, expected configuration/dependencies/inputs and null expected source/revision; one atomic no-replace package creation. |
| `prepare-revise` | `claim_id`, allowed `fields`, source-copy `forms` (`{form_id, field_id}`), authored `reason`; proposes one successor and rebinds every current form of that Claim. |
| `claim.revise` | The proposal plus command ID and exact expected configuration/source/package/dependencies/inputs; sibling Claim bytes and forms stay unchanged. |
| `prepare` / `apply` | Explicit `claim_id`; prepare selects one form/field, apply carries bounded common form changes and the same exact expected inputs. Source-copy forms keep the entire qualified Claim as context; freeform/template proposals receive no assessment or admission. |
| `inspect-version` | Explicit `claim_id` and exact previous `source`; returns unchanged predecessor bytes through committed Claim history, never through a caller-supplied archive path. |

Use `owner_configuration`, `source`, `revision`, `expected_dependencies` and
`source_bindings` from the corresponding preparation; the request fields are
`expected_configuration`, `expected_source`, `expected_revision`,
`expected_dependencies` and `expected_inputs`. Candidate grounding has no
fictional file: `OwnerLocalSourceClaimProfiles.prepare_candidate` freezes its
canonical body and uses the same actual source readers as stored-only `load`.

Creation retains the common `tos_local_claim_create_receipt_v1` plus exact
owner configuration and serialization provenance. Claim correction retains
`tos_claim_revision_history_v1`; the existing typed Claim history reconstructs
every stream successor through protected `tos_source_package_archive_v1`
bytes. The shared source/private locks, archive transport and atomic directory
exchange are reused. A command identity cannot be reused across creation,
Claim corrections, sibling Claims or form-only changes. Current source,
rights, configuration, grammar and complete package integrity are checked on
retry; the original receipt remains historical and is not rewritten after
unrelated corpus growth. All current forms keep their predecessors.

Identity-only discovery includes public and selected private source Claim
streams, native annotation identities, source profiles, forms and provenance;
it excludes payload, catalog, archive and staging homes. The same bounded
metadata scan and package/history ceilings apply as above. The writer’s cost follows that scan, and its trust scope includes cooperating
processes under the same account. Interrupted unpublished staging is not
adopted by a retry; committed history is not deleted by reader rollback.

Every result declares `visibility: local_only`, `publication_authorized: false`
and `grants_admission: false`. Public Claim/form readers retain their refusal
gates. To use the result for source-visible assessment, independently select
the stored Claim and its exact grounding through v4; assessment and publication retain their independently selected authority.

#### Historical retries while the source corpus grows

For both confidential writers, `expected_dependencies` is the whole-input
compare-and-swap gate for a **new** commit. It also remains immutable historical
evidence in the retained request and receipt. It is not a requirement that all
future unrelated identity inventory bytes or implementation files equal that
old snapshot. Such a requirement would prevent replay as soon as the newly
created native unit acquired its first legitimate Occurrence.

A repeated command must bind its original request, delegated configuration,
receipt and exact output/history. The command revalidates current source,
rights, declared identities, current selected grammar and collision freedom,
then compares snapshots from this retry and rechecks package bytes before
returning. Native creation reproduces its exact original packet; source
creation/revision reproduces the original record/form transition from retained
history. The response marks
`replay_input_posture: "historical_request_current_validation"` and returns the
original receipt without rewriting its time, digest, result or dependency hash.

This deliberately does **not** prove unchanged historical bytes for inputs
which were not individually pinned: for example compatible bibliographic notes,
manifest versions, harmless unpinned schema-byte changes or implementation
updates. Current validation does not supply missing past fixity. The native
packet/layer/content bindings and current configuration/context bindings stay
exact; semantic schema refusal, changed bound text, withdrawn rights, identity
collision and drift during the retry still fail. The content retains its recorded assessment and admission history when a
changed validator accepts the same original bytes. Existing
v1 receipts need no rewriting or fabricated retroactive dependency manifest;
new writes retain their full expected-snapshot gate.

The native owner-profile reader in
`rust/crates/tos-command/src/source_private_profile.rs`, dispatched by
`source_native_private_cli.rs`, is the separate read-only profile facade.
`validate` and `load` always inspect metadata only, even after an earlier exact
read. An exact content read requires its distinct selected scope; a snapshot
rechecks all previously consumed metadata and exact content. Public
registry/schema digests are visible, private source dependencies opaque. The
facade has no catalog or export method and does not itself grant derivation
rights or reserve identities. Commands provide those separate checks.

### Confidential source assessment v4

`tos_local_assessment_owner_v4` uses the same assessment policy, authority,
competence, subject, command and journal grammar. It adds confidential source selection to the shared engine under an explicit
local disclosure scope.
V1/v2/v3 remain supported. In particular, v3 already supports authorized private
representation bytes inside `source_root`; v4 adds the explicit authored-store
transport described above, rather than reclassifying that existing access.

V4 retains all common v1 configuration fields and `source_records`, replaces
`source_root` with the independently selected `source_context_ref`, and requires
both of these bounded lists (empty is allowed):

- `owner_local_source_records`: at most 64 selections of `{path, record_id,
  profile_type_id, origin_id, source_access, source_binding, form_ids}`. The
  profile is an existing `semantic-metadata-v1` type. The path must resolve to
  its typed metadata package in the selected store, and the actual record
  must be `local_only`. `source_binding` is the complete fixed native binding
  for Occurrence, otherwise `null`; non-native profiles use metadata-only
  scope. Claim selection uses the separate optional list below.
- `native_text_units`: at most 64 selections of `{binding, origin_id,
  source_access}`. Each native unit still requires the exact subject scope,
  maker and source language of the v3 contract. Both native records retain
  their same origin. Supporting layers do not become assessment targets.

The optional `owner_local_source_claims` list selects at most 32 Claims. Each
entry is exactly `{path, claim_id, relation_type_id, origin_id, source_access,
source_records, native_bindings, verify_content, form_ids}`. Omission preserves
the existing v4 configuration contract. `path` is one exact private
`source-claims.jsonl`; the selected Claim must be `local_only` and use an
existing understood semantic or identity relation profile. No private corpus
scan, new predicate registry or implicit interpretation of unknown fields is
performed. Adjacent Claim forms use the existing `claim_forms_path` convention
and the same current-form/history grammar.

`source_records` selects at most 16 actual public or private endpoint/evidence
records by `{path, record_id, profile_type_id, origin_id, source_access,
source_binding}`. Each is resolved by its existing profile reader; an inline
body or caller-supplied type tag cannot substitute for that source. Occurrence
contributes its independently supplied full native binding. `native_bindings`
selects at most 8 additional native evidence bindings by `{binding, origin_id,
source_access}`. All grants are checked before source metadata, all selected
native metadata and local-research rights before exact text. `verify_content`
is an explicit boolean; exact mode requires independently granted exact scope
for the Claim and every bound source. Non-native source profiles stay
metadata-only. Unused selections, ambiguous evidence aliases and unsupported
arbitrary file/event citations are refused, not guessed. Provenance,
alternatives and supersession fields remain authored data, not commands or
implicitly resolved evidence. A selected native packet, unit, layer or anchor
returns through the existing native adapter; no arbitrary path or copied text
becomes evidence.

The Claim's required assessment sources are its exact endpoint and evidence
closure, including the native unit view and layer with their original origin.
`describe.command_context.required_sources` exposes only their exact record
references. Every one must occur in assessment `evidence` as support, challenge
or context. Loading the records is not enough. The same closure accompanies
Claim freeforms, together with their exact Claim subject, through current
admission and materialization. Changed grounding invalidates current use
without rewriting the retained assessment, Claim or form. Native evidence
introduced by a Claim is supporting-only; independently selecting a native
assessment target still requires its own explicit unit scope.

In both lists `source_access` is exactly `{read_scope, access_allowed,
authority_ref}`. `read_scope` is `metadata_only` or `exact_owner_local`, access
must be the boolean `true`, and the authority reference must be nonempty.
This protected issuer input grants bounded reading only; assessment admission
still needs current policy, authenticated principal, delegated action and
competence. A command cannot supply or widen this scope.

Each `form_ids` list explicitly selects at most 32 distinct **current** forms
from the source's derived adjacent `<stem>.human-forms.json` path. A caller
cannot submit arbitrary private form paths, choose a predecessor as current,
replace the subject or import unselected neighboring forms. Source records,
whole form sets, unknown fields and prior form history remain unchanged.
The source-context root owns the exact assessment, policy, authority,
competence, batch, form, form-set and template schemas used for v4. Their byte
digests join the private snapshot. History checking, assessment engine, journal
and materializer receive the same freshly built validators through internal
dependencies, never request fields. A cached validator from another checkout
cannot override that selected grammar; v1/v2/v3 keep their existing defaults.

The description’s maker identifies its own descriptive act; the segmentation
records its own maker. A form retains its own `creator_id` and
`human_projection` layer. Private description/form scopes must cover their
actual authored languages, selected public or private form bindings and native source language;
omitting a language cannot evade competence requirements. Unknown language retains its explicit unknown state.

An Occurrence or form based on it requires an explicitly selected **same full
native binding** with verified exact text. Metadata-only selection permits
description and inspection but not append or assessed materialization. Any
metadata-only native evidence in an ordinary v4 selection also makes the current
assessment unusable for another subject; the unchanged supporting layer ID
cannot revive an old positive decision after exact reading is withdrawn.
Claim and Claim-form scopes instead use their declared exact grounding closure:
unrelated metadata-only units or languages do not contaminate that scope.
Private source-copy/freeform `materialize-form` uses the whole-subject binding
and current assessment route, returning wording only while that form assessment qualifies.
Reading a source-copy form or recording its assessment does not turn it into
an agent-authored freeform or mutate its source.

V4 owner configuration and context files require mode 0600. The independently
selected, pre-existing `journal_directory` must be a dedicated directory
inside the context's private root, not that root itself. Every private journal
directory is mode 0700; locks, batches and heads are mode 0600 even with a
permissive umask. Read and replay reject unsafe existing modes. The same
immutable batches, subject lock, head publication and retained-orphan recovery
apply; no alternative private history format is introduced.

The owner snapshot binds exact configuration bytes, context, selected public
records, private records/forms/grammar and native closure. V4 rechecks them
at the append/replay edges and before every returned current view, and checks
that the journal head did not change during the read. A source, form, schema,
context, rights or verified-content change is a conflict/refusal, not partial
acceptance. Metadata/profile selection shares an 8-MiB/128-distinct-file budget;
native resolution retains its separate 16-MiB/128-file aggregate budget. All
selected records together stay within the existing 1,024-record engine limit.
These bounds do not isolate hostile same-account editors or create a
cross-subject filesystem transaction.

Every v4 response carries `visibility: local_only` and
`publication_authorized: false`. `describe` exposes exact private record/form
refs and origins, not their bodies, private paths or native short-span hashes.
`owner_local_contracts` exposes only the public grammar and registry fixity.
Explicit freeform materialization and retained assessment prose can contain
private content; their local-only result must not enter a public projection.
The existing `AssessedFormSnapshot` graph adapter uses
`run_public_source_command`, which accepts only v1/v2/v3 immediately after
reading the protected version discriminator. V4 is refused before opening its
source context, private records, native content or journal. The ordinary
`source_records` selector likewise rejects the reserved owner-local namespace
before file reading. Public reading and publication retain their separate adapters and grants.

Reproduce with the synthetic checks in
`tests/conformance/rust/command_owner_text_cases.rs`, plus the existing
native and common assessment tests. Private Claim reader and journal integration
checks live in `tests/conformance/rust/command_private_claim_cases.rs`.
Successful synthetic qualification verifies the mechanics. Real-language
competence, source quality and rights depend on their actual owner evidence.

### Confidential text-layer quality v5

The additive `tos_local_assessment_owner_v5` configuration can assess one
exact raw native TextLayer against its original acquired bytes and require
current purpose-scoped quality for dependent units, Claims and human forms.
The [text-layer assessment contract](#exact-text-layer-comparison-and-current-quality-use)
owns selection, explicit comparison reading, protected grants, quality-basis
dependencies, withdrawal behavior and bounded journal coordination. It uses
the existing policy engine and immutable journal; v1–v4 and public adapter
boundaries remain unchanged.

The separate [image/OCR comparison v6](#exact-imageocr-comparison)
adds bounded retained-PDF-page and operator-created synthetic-PNG profiles,
each over an authenticated owner OCR layer. Historical pages remain
undisclosed by this profile; synthetic assistant disclosure needs its own
exact current grant. Quality admission requires the purpose-scoped comparison assessment.

### Descriptions bound to native text

The registry-declared `source-text-unit-v1` profile adapter connects a public
Occurrence description to the same native resolver. The ordinary source
creation contract verifies exact public content; catalog, form and descriptive
revision reads check the complete metadata closure without reading payload.
Their dependency fingerprints include the separately held opaque native text
snapshot. The fixed binding is outside the ordinary `record.revise` fields;
name and hover forms must retain it as context. Private source writes and release of operator-held text require their
respective owner routes.

For assessment, v2 metadata-only inspection remains available, but a source
record with `native_text_binding` requires a v3 exact selection of that same
complete binding for usable admission. This applies also when the occurrence
supports a selected Claim or source-bound form. No caller-supplied boolean
waives it. `describe` exposes `command_context.source_read` with `required`
and `ready`; a missing exact read limits supported operations to describe and
inspect. `append` refuses it. The owner derives the engine's
`SubjectContext.source_read_ready` (default true for unrelated subjects), whose
qualification failure is `subject.exact-source-unverified`, separately from
`subject.access-denied`. Thus metadata-only inspect cannot revive a prior
exact-read admission; retained review events and receipts are not erased.

Any assessment command selecting native-bound source metadata rechecks that
closure under the journal lock and again after blob creation, before head
publication. Creation/form/revision likewise bind their protected native
dependency snapshot. This closes observed stale-input edges; the issuer must keep the selected multi-file snapshot stable through the
operation.

## Human-form materialization

`rust/crates/tos-web-rules/src/human_forms.rs` provides the shared native/WASM
materialization rules for the source-owned `ToS/doctrine/HUMAN_FORMS.md` contract.
The native source owner binds each immutable form to its trusted scope, exact
access-filtered source records and owner-admitted template set. A source binding
identifies a whole JSON field. The source owner declares its
required context. Freeform rendering requires the existing assessment
engine and authenticated reviews/history against the current form and source
snapshot; it cannot use a submitted positive status as permission.

This pure operation writes nothing and does not call a model. The result's
wording and context are a single read contract; `standalone_reading: false`
forbids consuming the string as an unqualified assertion. The current output
budget is 64 KiB with explicit refusal, not semantic truncation. Missing,
restricted, stale and assessment-required states have no emitted wording.
Adapter authentication, template quality, reviewer language competence and
consumer behavior each require their corresponding validation or review.
`tests/conformance/rust/command_form_cases.rs` protects these boundaries.

Optional `FormScope.language_context` names exact source-owned language and
linguistic-derivation metadata. The form must bind the same object and any
translation/transliteration/adaptation source field. Both remain current
dependencies and mandatory context; the source-owned derivation record states originality or translation status. These checks preserve the source owner's declaration without
performing linguistic assessment or granting a submission its own scope.

## Local source growth commands

`tos-native-owner-command source-commands --invocation /absolute/native-invocation.json` is the explicit source-write entrypoint. Its first
adapter creates and revises forms adjacent to one bibliographic or historical source record;
that adapter does not mutate the subject. The separately delegated historical
creation adapter below publishes a new subject with its initial claims and
forms. Both write to the source-owned corpus through this explicit command route;
`access` remains read-only.
The same form-only delegation also supports the native physical-witness and
scholarly-composite paths described in
[`HUMAN_FORMS.md`](../../ToS/doctrine/HUMAN_FORMS.md#native-material-witnesses-and-scholarly-composites).
It verifies the native public-metadata schema and exact owner path on every
call, including replay, and exposes its digest in `source_contracts` and the
receipt. Schema drift invalidates prepared writes. Native identity and source
bytes stay unchanged; native source rewriting requires its own permission.
The normative identity/admission boundary remains in
`ToS/doctrine/HUMAN_FORMS.md`. The same CLI serves a human or an agent:

```bash
/absolute/installed-prefix/bin/tos-native-owner-command source-commands \
  --invocation /absolute/operator-selected/native-invocation.json < request.json
```

The independently selected protected configuration has exactly these fields:

| Field | Meaning |
| --- | --- |
| `schema_version` | `tos_local_source_command_owner_v1` |
| `uid`, `principal_id` | actual Unix account and delegated source-form creator |
| `source_root`, `source_path` | absolute protected repository root and exact relative `ToS/source-witnesses/.../*.json` corpus record |
| `authority_ref` | issuer-provided reference to the source-write delegation |
| `allowed_form_ids` | at most 32 exact existing or newly delegated `tos.form.*` identities |
| `allowed_operations` | a subset of `form.create`, `form.revise`; empty revokes writes |
| `expires_at` | timezone-aware exclusive expiry |

The issuer must allocate noncolliding form identities before delegating them;
this bounded adapter does not scan other subjects or issue globally unique IDs.
The subject must already be owned, public metadata. Historical-record v1
requires explicit `public` or `public_metadata_only` visibility on every call,
including replay. Historical truth requires source-visible assessment. The command reads the
declared public-metadata schema and preserves its visibility contract.

All requests use `schema_version: tos_local_source_command_v1`:

1. `{"schema_version":"tos_local_source_command_v1","operation":"describe"}`
   returns current exact source, configuration digest, form-set byte revision,
   current form refs, reader states, operation/ID scope and `source_fields`.
2. `prepare` adds `form_id` and `field_id` from that catalog. Select
   `metadata.preferred-name`, `metadata.source-note` or an advertised
   `metadata.variant-name:N`. The variant ordinal selects a field within this exact snapshot; the form
retains its independently allocated ID. Preparation returns one `prepared_change`, selecting create or
   revise and binding the full source field, current predecessor, creator,
   language and every mandatory qualifier. It performs no write or admission.
3. `apply` adds `command_id`, `expected_source`, `expected_configuration`,
   `expected_revision` and `changes`. Copy the expected values from the prepared
   result's `source`, `owner_configuration` and `revision` respectively.
   `changes` is a list of 1–32 unique-ID changes, each exactly
   `{operation, expected_form, form}`. A create expects null and version 1;
   a revision expects the exact current form, advances one version and binds
   it in `revises`. The caller can submit the complete form contract for a
   freeform/template proposal, but cannot submit admission, grant or scope.

Success is `tos_local_source_command_result_v1`, exit 0. `receipt` records the
historical commit and `replayed` distinguishes retries; the remaining refs and
materializations describe the current snapshot. `grants_admission` is always
false. A committed unsupported/freeform proposal remains unavailable in this
metadata-only reader and routes to the form/assessment owner. `describe` and
`prepare` never create a file. Errors exit 2 with
`tos_local_source_command_error_v1` and an exception class, not echoed input.

The sole content target is `<source-stem>.human-forms.json`. An atomic rename
commits all related changes, predecessors and the optional `growth_history`
receipt together; there is no detached receipt that can claim an absent write.
Duplicate commands must have identical canonical request digests. Current
delegation is checked before replay, and the old receipt never replaces fresh
reader state. Conflicts require fresh discovery, not overwriting another
writer's revision. Historical form sets without command receipts are supported
without inventing past authorization.

A stable source-root `.historical-create.writer.lock` is acquired before the
sibling `.<set-name>.writer.lock`. The former also coordinates whole-package
source revision, so exchanging a directory cannot split active writer locks.
Both coordinate cooperating local command writers with a
five-second bounded wait (`JournalBusy` means retry, not restart/delete).
Lock and unpublished staging files are local operation state and stay outside
source commits.
Protected-path and UID rules match the assessment entrypoint: no symlinks,
setuid execution or other-account writable paths; same-account hostile code is
outside the boundary. Temporary publication files are mode 0600 and fsynced
before rename; the parent is fsynced after it. On abrupt process loss, an
unpublished `.pending` file may remain and is never a committed form set.
Only an ordinary exception removes that invocation's own unpublished file.
Source/configuration publishers and ordinary editors must remain quiescent
during the operation; final rereads detect changes but are not a transaction
over independently edited files. Cross-subject transactions remain unimplemented.

Requests/source/configuration are bounded at 1 MiB each; the set at 2 MiB,
32 current and 256 prior forms, and 256 command receipts. Reaching a bound
refuses new work without deleting history. Use exact successor corrections for
semantic rollback; reverting a derived reader does not erase these sources.
Do not remove committed receipts to reuse a command ID. Graph/catalog rebuild,
publication, model execution and assessment are separate owner operations.

`tests/conformance/rust/command_form_cases.rs` tests the actual CLI,
creation/revision batches, restart/replay, concurrent writers, loss before/after
publication, revocation, protected paths, inert input, preparation and partial
source rebinding. Its catalog-wide test prepares existing public bibliographic
fields **in memory** and verifies source-byte preservation. This in-memory check covers preparation and byte preservation; actual
migrations and source/translation quality require their own evidence.

### Initial historical subject creation

The same CLI also accepts a separately selected
`tos_local_historical_create_owner_v1` configuration. It has the same account,
root, exact source path, authority, expiry and form-ID fields as the form
adapter, plus `record_id`, `allowed_claim_ids` (at most 32 exact `tos.claim.*`
IDs) and `maker_type` (`human`, `software` or `model`). Its
`allowed_operations` is a subset of `["historical.create"]`; the old form
configuration cannot grant this operation. Claim maker identity and kind must
match the issuer-selected principal and kind, including on replay.

The exact source basename must match `historical-event.json`,
`historical-process.json` or `historical-state.json` and the delegated ID.
Its subject directory must not exist; its parent must already be protected
and owned. The adapter will not create a directory hierarchy, merge into an
existing directory, change another subject, or change an existing version.
The issuer's storage, rights and public-metadata decisions remain necessary;
an operation scope does not itself establish those permissions.

Requests retain `schema_version: tos_local_source_command_v1`:

- `describe` returns operation/identity scope, source and claim schema refs,
  configuration digest and `target_exists`; it does not create a lock or file.
- `prepare` adds only `record`. It validates the proposed initial record and
  returns `prepared_source` and its semantic `source_fields`, without a write.
  A proposed exact source ref is not an existing source or a reservation.
- `prepare-create` adds `record`, `claims` and `forms` as below. It validates
  the complete proposal against current sources and returns `prepared_files`
  (byte counts/digests) and `expected_dependencies`, without a lock or write.
  Dependencies include the executing source/renderer implementation and form
  schemas as well as the selected source tree's historical contracts.
- `historical.create` adds `command_id`, `expected_configuration` from
  discovery, `expected_dependencies` from the complete preview,
  `expected_source: null`, `expected_revision: null`, the full
  `record`, `claims` (0–32 complete historical-claim records), and `forms`
  (1–32 `{form_id, field_id}` source-copy selections from preparation).
  At least one name form is required. Form IDs are allocated independently
  of their current wording or selected field; later changes use `form.revise`.

The initial record has version 1, `provisional` identity,
`no_equivalence_claim` and public-metadata visibility. Claims have version 1,
their own delegated IDs, the new subject, unchanged scholarly-report and
unreviewed posture, and no supplied assessment or supersession. Typed endpoints,
relative date anchors, existing provenance events, evidence/counterevidence
and alternative-claim closure are checked using the authored catalogs and
the existing graph reader's schema/registry rules. Evidence paths must name
protected metadata, not payloads or traversal paths. This step resolves the declared references. Source-visible review evaluates
historical truth and source quality under the applicable competence and
admission policy. Unknown source
extensions and claim qualifiers are retained verbatim as JSON values.

With the v1 configuration publication creates exactly four files in the new directory: the typed
record, `historical-claims.jsonl`, the adjacent human-form set, and
`source-create-receipt.json`. They keep the existing source formats and
readers; there is no new source envelope or duplicate graph store. The receipt
binds the canonical request, account/delegation, exact subject, observed
dependency digest and each output's bytes/digest. It records this source transaction; Claim research provenance and assessment
events retain their own source records.

The prepared dependency digest also binds the exact source-profile registry,
profile contract and source schemas consumed while reading existing metadata,
plus the shared reader implementation. Changing these after preparation
causes a conflict before publication, including a schema edit that leaves
the proposed source content otherwise valid. The declaration does not widen
the historical writer's delegated scope. General metadata creation uses its
own explicit configuration below.

New materialization can select `tos_local_historical_create_owner_v2` with
the same fields plus one exact `provenance_event_id` (`tos.event.*`). All new
claims must bind that ID, which must not exist in the authored provenance
index. The ID remains part of current delegated scope on replay. The v1 route
retains its existing-event meaning; it does not retroactively acquire v2 evidence.

V2 atomically adds `source-create-request.json` (canonical request including
the protected configuration's digest, not its contents),
`source-create-environment.json` (runtime facts without local paths), and
`source-create-provenance.jsonl`. `prepare-create` lists these under
`capture_at_apply`; their runtime bytes do not exist at preview time.
The creation receipt binds every output including the exact event bytes, and
the event points back to that receipt without a self-hash. Existing graph
readers preserve the original public-metadata v2 event alongside normalized
activity fields. Historical dating remains in historical claims.

The recorded activity is completed **buffer serialization**, before staging
and the atomic commit. Digests are captured from buffers; stored-byte fixity requires a separate read after publication. Its software executor, script/runtime
digests, Unicode version, withheld process-argv digest, request/environment
bindings, derivations and measured pre-commit wall duration are captured by
the command itself. The event is unsigned and only partially specified for
replay. Upstream source reading, author/model reasoning, substantive assessment and
rights decisions retain their own provenance and authority. The command
records its actual software serialization activity. A failed
publication exposes none of these source files; retry preserves the successful
event rather than generating a second historical execution record.

The files are staged under `ToS/.source-create-*.pending`, **outside** the
source-witness scanner root. Fsync precedes a Linux `renameat2(RENAME_NOREPLACE)`
commit of the complete directory; both parents are then synced. Missing
no-replace support or a cross-filesystem route fails without a copying fallback.
Even an empty competing destination is never replaced. A bounded global
`.historical-create.writer.lock` under source-witnesses coordinates these
create commands. Identity collision checks and dependency rereads currently
scan authored metadata; this is not yet an indexed or incremental writer.
Ordinary editors and other metadata/configuration publishers must remain
quiescent through the transaction. Final rereads detect drift, but do not
provide a transaction over uncooperative same-account writers.

`tos_local_historical_create_result_v1` returns the retained creation receipt
and `replayed`, never a current assessment. An exact retry after response loss
returns the old receipt without rewriting source files. Current operation,
maker and ID revocations still apply. An occupied target without that matching
receipt is a conflict, not a migration opportunity. The receipt describes the
initial creation; subsequent ordinary form/source changes do not become its
historical output. A reused command ID with changed input cannot overwrite it.

### Declared-profile subject creation

`tos_local_profile_create_owner_v1` delegates `source.create` through the same
CLI and atomic directory transaction, independently of the historical route.
Its exact fields are `schema_version`, `uid`, `principal_id`, `maker_type`,
`source_root`, `source_path`, `record_id`, `profile_type_id`, `authority_ref`,
`allowed_form_ids`, `allowed_operations`, `expires_at`, and `provenance_event_id`.
Account, path protection, expiry and ID limits remain as above;
`allowed_operations` is a subset of `["source.create"]`. No `allowed_claim_ids`
field is accepted. The selected concrete identity type must already declare
a valid `source_record_profile` in the source entity registry, with both
readers mapped. Neither the request nor a source field selects a profile,
executable, grant, path or schema not declared by that owner.

`describe` returns `tos_local_source_create_result_v1`, `profile_type_id`,
the complete `source_profile`, scope and configuration digest. `prepare`
takes `record` and returns the exact proposed subject and the ordinary
metadata field catalog. `prepare-create` takes only `record` and `forms`;
`source.create` adds the same command/configuration/dependency/absent-version
fields as historical creation. **Neither accepts `claims`, even an empty
list.** The record must satisfy its exact declared source schema version and
the shared metadata properties. Its ID, kind and basename must agree with
the independently delegated profile. Unknown versions, catalog/payload paths,
nonpublic metadata, pre-verified identities and noninitial versions fail.
Unknown fields permitted by the source schema retain their JSON values.

Creation emits six files: the native record, adjacent human-form set, request,
environment, serialization provenance and `source-create-receipt.json` with
`tos_local_source_create_receipt_v1`. There is no empty historical claim file
and no conversion to `tos_historical_record_v1`. At least one source-bound
name is required. Provenance identifies `source-profile-metadata-serialization`
and retains the historical route's explicit buffer-capture and unsigned
execution limits. Current revocations apply to exact retries. Dependencies
include the new subject's schema even before its profile has any instances;
changing it after preparation requires a new preparation.

Ordinary `form.create`/`form.revise` may subsequently address the same declared
metadata profile under their own existing form configuration. This does not
grant source creation to a form writer. Profile source-record revision uses
its separate [delegation below](#versioned-source-correction);
subject-specific relations, substantive assessment, identity merge/split,
publication and admission remain separate operations. Each follows its explicit source-owner contract.

### Native standalone identities

`tos_local_corpus_create_owner_v1` uses the same `source.create` request,
six-file transaction, metadata forms and serialization capture for the
existing native `Agent`, `Place`, `Organization` and initial `Work` families.
Its configuration replaces `profile_type_id` with `record_type`
(`agent`, `place`, `organization`, `work`);
the other declared-profile configuration fields remain required. Discovery
returns `record_type` and the exact native source descriptor. The existing `corpus-record.schema.json` owns these record shapes;
responsibility relations describe roles such as author or addressee.

The initial record must match the delegated typed ID/basename, version 1,
provisional identity and no equivalence claim. Supersession, pre-reviewed
labels/identifiers and metadata relation-link fields are refused. Native
Corpus metadata has no `visibility` extension; this route is explicitly
public-metadata-only and cannot store private payloads. Schema-permitted
unknown language qualifications are retained; unsupported fields are refused,
never silently dropped. At least one exact name form is required. The event records `source-corpus-metadata-serialization`; identity assessment
retains its own source record.

An initial Work must retain the Corpus-required `expression_claim_refs` as
an empty list. This records no supplied expression assertions, not evidence
that no expression exists. Nonempty expression refs and every other metadata
relation-link field are refused by this initial transaction. Authors, Expressions, publication dates, Editions and Items each use their own
source records and grounded Claims.
The existing `works/friedrich-nietzsche/` source home has stronger authorship
and chronology closure and is refused by this standalone Work route; that
source-home rule is not weakened to accommodate a new metadata record.
Version 1 does not create Expression, Edition, Collection, Item or physical
Artifact. Version 2 (`tos_local_corpus_create_owner_v2`) additionally creates
an empty provisional Collection; it does not add membership or publication
assertions. Its independently delegated
[Collection growth route](#op-doc-native-collection-growth) attaches an existing
Work through a qualified `contains_work` Claim. Other native kinds retain their
own source contracts and related-record closure.
Native [object-Link growth](#op-doc-native-object-link-growth) creates a Link
and qualified association Claim atomically under its own grant, including an
existing physical Artifact without changing that subject's metadata.
Relationship assertions use separately delegated `claims.create`; a label,
role word or metadata creation receipt cannot supply them.

Initial creation checks allocated form IDs against both metadata form sets
and declared Claim form sets, including retained predecessors. The consumed
sets include native Claims and finite public `history/**/historical-claims.jsonl`
locators without treating historical schemas as native write grants. Consumed
sets enter the preparation dependency digest, so new collisions or observed
changes before publication fail. This operation’s work follows the bounded metadata scan.

The same current/prior identity reservation covers native Claim correction,
excluding only that Claim's own adjacent set so legitimate successors remain
writable. It uses the existing public catalog locator authority: an incomplete
source root cannot establish ecosystem-wide uniqueness or confer authority
outside that root. Generic standalone form writers and partial-root metadata
correction retain their existing contracts; this finite companion does not
turn them into globally coordinated registries.

### Historical creation retries

Historical, declared-profile and native creation retries verify the original
request identity, receipt shape, principal/authority, exact source ref,
dependency/configuration bindings, non-admission flag, expected file set and
stored byte digests. A replay is not a fresh creation or current admission.
Changing the issuer authority requires its own handoff, not relabelling an
old receipt; scope revocation applies even to exact retries.

Later form versions are allowed only with valid retained lineage and the
original source-copy forms still present. Their initial byte representation
is checked with the existing initial serializer, not treated as the current
wording. An incompatible serializer change needs explicit compatibility work.
For historical source corrections the existing source-revision owner validates
the current history and all bound archives, recovering the initial source
bytes without restoring them over the current record. Creation receipts and
other initial files must remain unchanged. Missing archives, undocumented
source changes, corrupt outputs, unexpected package files and nonempty writer
locks fail closed. These bounded unsigned local-storage checks assume cooperating processes under
the same Unix UID.

An abrupt process loss before commit may leave an invisible staging directory;
retry does not delete or publish that abandoned directory. An ordinary exception
removes only its own unpublished staging files. Committed sources and receipts
are never removed by retry or derived-reader rollback. The request/configuration
budget is 1 MiB; each source output is bounded at 2 MiB, with the existing form
and rendering limits. These ceilings bound read and write volume; latency depends on the selected
data and storage.

Catalog/graph rebuild and public publication remain separate operations. A
builder traversing across a concurrent creation may need a fresh build; this
adapter does not make several catalog files one snapshot. Synthetic integration
tests cover the existing catalog → graph → access reader, scope and reference
refusals, competing writers, process loss before commit, response loss after
commit, and restart/replay. Actual historical episodes and reviewer quality require source-visible
assessment; subject revisions use the separate route below.

### Versioned source correction

`tos_local_source_revision_owner_v1` independently delegates `record.revise`
through the same `tos-native-owner-command source-commands` CLI. Its fields match the form-owner
configuration, plus exact `record_id` and `allowed_fields`. This historical
configuration keeps its existing schema and ID scope. A separate
`tos_local_profile_revision_owner_v1` adds `profile_type_id` and selects the
current declared `source_record_profile`, including Document, Letter or a
new supported metadata kind added through that registry. It uses the shared revision transaction. The exact declared basename,
ID prefix, source schema version and public-metadata visibility are required.
Existing form, creation and historical-only grants gain no new permissions.
The separate `tos_local_corpus_revision_owner_v1` uses `record_type` instead of
`profile_type_id` for the existing Agent, Place, Organization and Work source
descriptor. Corpus has no visibility field: the exact Corpus schema and this
explicit public-metadata route are mandatory, not a permissive missing-value
default for other schemas. It reuses the same package/history transaction.
Its `allowed_fields` is restricted to `preferred_label`, `notes`,
`field_languages` and `source_refs`; it cannot edit alternate-name judgments,
external identifiers, identity/equivalence status or any Claim/link field.
Correction requires its own explicit grant and preserves the native source
schema.
None of these revision configurations
revises claims, identities, rights, visibility or assessment decisions.
The profile route also cannot change the record kind, source schema version,
identity status or supersession links; those require their own transitions.
It does not
create or migrate an undeclared source format.
`allowed_operations` is a subset of `["record.revise"]` and `allowed_fields`
is a subset of `preferred_label`, `variant_labels`, `notes`, `field_languages`,
`source_refs`, `extensions`, `semantic_content`. The latter is available only
when the exact source schema permits it, as in thought-description profiles;
older profiles still reject it. The independently selected grant must name
the field. `semantic_scope` remains immutable under public v1. The explicit
`tos_local_profile_revision_owner_v2` configuration has the same fields and
transaction grammar and may additionally name `semantic_scope` in
`allowed_fields`. Its purpose is correction of scope wording for the same
referent and continuity criterion. Review must preserve that criterion’s
meaning; a change of referent or criterion requires the identity route.
Private v1 and the historical/Corpus/native grants retain their existing field
scopes. Changing any allowed value records an authored correction;
source-visible assessment evaluates its adequacy. Unselected fields and all
unselected companion bytes stay unchanged, including unknown extensions.

The request has `schema_version: tos_local_source_command_v1`:

- `describe` returns the current exact `source`, whole-package `revision`,
  `owner_configuration`, allowed fields/operations/forms and materializations.
  The profile route additionally returns `profile_type_id` and the exact
  `source_record_profile`; consumers do not guess schemas from a filename.
  The native Corpus route returns `record_type` and `source_profile`, including
  its exact schema, typed identity prefix and public-metadata-only scope.
- `prepare-revise` adds `fields` (nonempty field-value patch), `forms`
  (`{form_id, field_id}` selections) and a bounded authored `reason`.
  It returns the next source/form refs, prepared materializations and
  `expected_dependencies`, without creating locks, archives or source files.
  Profile dependencies include the consumed registry, registry schema, exact
  source schema and its declared local dependencies, plus reader/command/form
  implementation inputs. A changed dependency invalidates an uncommitted
  preparation, even when the source record itself is unchanged.
- `record.revise` adds `command_id`, `expected_source`, `expected_revision`,
  `expected_configuration`, `expected_dependencies`, copied from preparation.
  Source ID/type stay fixed and `record_version` advances exactly once.
  Every current form must have an explicit successor selection, validated by
  the actual source-copy reader; at least one name remains required. Old forms
  retain their exact prior source refs. This adapter does not automatically
  reinterpret freeform/template proposals or carry old assessment admission
  to a changed source digest.
- `inspect-version` adds exact `source: {id, version, digest}` and returns
  `record`, `inspected_source` and the archived byte bindings for a predecessor
  in committed history. Path selection is derived from the configured subject,
  never taken from submitted prose. Current source remains available through
  the ordinary reader; an uncommitted archive is not an addressable revision.

The active package stays at the original source path. Before any replacement,
the complete old flat package is durably stored under
`ToS/source-witnesses/.record-revisions/<subject-hash>-<package-hash>/`.
`manifest.json` binds original filenames, exact source ref, file sizes/digests
and content-addressed `.blob` files. These tracked bytes retain exact source history and remain addressable through
the version reader. The blob suffix prevents old record or
provenance filenames from becoming duplicate current identities in existing
scanners. `inspect-version` verifies every retained byte and reconstructs the
successor from its retained request; a missing/corrupt archive fails closed.
Initial creation receipts and provenance remain unchanged historical records,
whose exact original output bytes can now be returned through this archive.

Every new record correction verifies the complete retained predecessor chain
before archiving its current package and again at the publication edge. Each
archived ledger must contain the exact earlier receipt prefix, so truncating a
current ledger cannot silently redefine its baseline. This check follows only
the bounded declared archives, preserving each original flat or selected scope.
Protected source corrections use their own private archive reader. Selected
correction and compound-growth guards apply the same check during publication
and explicit recovery. Missing or damaged evidence leaves current source or
pending transaction state intact; restoring exact bytes permits the authorized
retry. A stored revision ledger contains at least one receipt; an emptied
ledger is corrupt. Baselines imported without a ledger keep their declared
version, and the first correction archives that exact baseline.
The independent exact-version reader accepts retained profile-scope corrections
made through public v2 and returns both original and successor records. Historical
inspection remains available after the writer grant expires; new scope writes
still require the separately selected v2 delegation.

`source-revision-history.json` in the current package records the complete
canonical request, its digest, issuer/delegation, reason, exact predecessor
and successor, dependency digest, form results and archive locator. The receipt records the correction transaction and its issuer. Admission and
upstream research execution retain their own evidence.
Existing historical claims, maker and provenance are not upgraded because a
record description was corrected. Exact retries return the original receipt
plus fresh current reader state; current scope revocation still applies.

After the stored archive is independently byte-checked, record, forms and
history are staged together outside the source scanner under
`ToS/.source-revision-*.pending`. Linux `renameat2(RENAME_EXCHANGE)` publishes
one complete directory namespace; there is no multi-file copying fallback.
The stable corpus writer lock covers creation, form-only writes and revision.
Readers opening several files independently still need snapshot/currentness
checks: directory exchange is not a transaction over an already-running
catalog traversal. Access stays read-only and builders/publication are separate.
Uncooperative editors and older writers must remain quiescent; final package,
configuration and implementation/schema rereads detect observed drift, not a
sandbox against hostile same-account code.

An ordinary exception removes only this invocation's uncommitted staging, or
an exact old staging copy already verified against the retained archive.
Abrupt process loss may leave an inactive staging directory; retry neither
publishes nor deletes it. If loss follows exchange, the current history proves
the commit and a retry returns it without rewriting. An archive published
before a failed commit remains retained but uncommitted; a retry with the same
old package verifies and reuses it. Recovery must inspect these exact artifacts,
not infer a live writer from their existence or delete committed history.

The adapter bounds a flat package to 64 regular files, 2 MiB per file and
8 MiB total; nested directories and symlinks are refused, not skipped. The
archive permits its one additional manifest. History stops at 128 revisions,
and form-history limits may bind earlier. No history is truncated. The caller
must obtain host capacity accounting for a large write; these ceilings do not
reserve storage or authorize publication. Reverting wording uses another
successor correction; switching a derived reader does not erase source history.

This representation was chosen to retain compatibility with existing source
paths while correcting related record/forms atomically. Separate file renames
would expose partial changes; a new pointer-only source store would require
migrating every existing reader. The accepted cost is bounded package copying and Linux-specific exchange.
General multi-subject changes, correction of other native record families and
automatic retirement of abandoned staging remain separate Growth work.

### Selected native correction and explicit recovery

`tos_local_corpus_revision_owner_v2` is an explicit new delegation over the same
native Corpus description fields. It supports Agent, Place, Organization, Work
and Expression. Its configuration has the v1 Corpus correction fields, with
`allowed_operations` limited to `record.revise` and `record.recover`. Neither a
v1 correction grant nor a form/creation grant gains these capabilities.
The current record's ID, type, schema, identity/equivalence posture, external
identifiers and relation refs remain outside descriptive correction scope.

`tos_local_corpus_revision_owner_v3` is a separate explicit grant version for
the same selected-file operation. It additionally supports Edition, Collection
and Item, completing descriptive correction for the eight native Corpus record
kinds. V1 and v2 retain their original type scopes. V3 permits only
`preferred_label`, `notes`, `field_languages` and `source_refs`, within the
issuer's narrower `allowed_fields`. Edition embodiment/publication/exemplar
refs, Collection membership, Item manifests, rights, payloads, identity status
and all other structural fields are preserved, not followed or modified by
this metadata command. Their semantic and source-quality judgments retain their own assessment state. Initial Collection creation and
membership growth remain separate operations; this grant does not supply them.

The v2/v3 revision unit is exactly the selected `<type>.json`, adjacent
`<type>.human-forms.json`, and `source-revision-history.json`. An initial form
set or history may be absent. Every successor has all three files. This route
does not enumerate, copy or exchange the parent directory: existing Expressions,
Editions, Items, payloads, unknown siblings and creation provenance stay outside
the selected unit. A request cannot supply extra paths or a larger file set.
The earlier v1 flat-directory package digest and archive semantics are unchanged.

Use the ordinary `describe`, `prepare-revise`, `record.revise` and
`inspect-version` requests. Both grant versions retain the v2 result envelope:
`tos_local_source_revision_result_v2`, `publication_protocol`, `selected_files`
and the current `publication_snapshot`. Preparation also returns
`expected_publication`; copy it into the revision request alongside the existing
configuration/source/revision/dependency bindings. Source-copy form successors
must explicitly cover every current form. Existing limitations on authored
freeform or template rebinding remain visible rather than silently converting
those forms or carrying admission to a different source.

The predecessor archive has explicit `tos_source_package_archive_v2` scope.
`tos_source_revision_history_v2` retains earlier v1 receipts unchanged and adds
a publication binding to each selected transition. The original archive retains
its original scope: a v1 full-package archive never becomes a selected one.
Historical creation replay checks exact creation files and initial forms even
after a v2 correction and later descendants; it performs no new creation or
descendant inspection. An uncommitted archive alone is not an addressable version.

Byte movement uses the internal
[selected-metadata transport](README.md#selected-metadata-publication-and-recovery), under the
existing corpus writer lock. Official readers and other common-lock writers
refuse a pending transaction. This publication barrier coordinates readers and writers that participate in
the common protocol. Raw filesystem readers must provide their own snapshot
checks. Catalog and graph
rebuilds remain separate derived operations; their failure does not undo source.

While pending, the exact original command may resume only under its unchanged
current delegation. Explicit recovery uses `schema_version:
tos_local_source_command_v1`, `operation: record.recover`, the exact
`transaction_id`, `decision: resume|rollback`, and current
`expected_configuration`. A renewed recovery-only grant must still select the
same principal, authority, source path, typed identity, fields and form IDs and
pass current schema/dependency checks. It authorizes recovery of the fully
reconstructed retained before/after plan, not arbitrary writes or new revisions.
Its evidence accompanies terminal publication without overwriting the original
authority binding. A third file state, revoked scope or changed dependency leaves
pending evidence intact. Rollback restores exact selected bytes but advances the
publication token, so a reader spanning the interrupted interval must restart.

The focused command checks live in
`tests/conformance/rust/command_metadata_publication_cases.rs`; transport and
reader-boundary checks are separate. Existing 8 MiB selected-package, 128-history
and source/form byte limits refuse rather than truncate. Semantic assessment, current use, rights, canon, release and deployment retain
their owner decisions.

<a id="native-work-expression-growth"></a>

### Native Work / Expression growth

The [native compound owner contract](#op-doc-native-work-expression-growth) owns the
Work/Expression grant, `work.expression.create`, pending recovery and its
selected-metadata transaction. Edition, Item, responsibility, assessment and
publication remain separate routes.

### Native Expression responsibility attachment

The [responsibility owner contract](#op-doc-native-expression-responsibility) owns
the dedicated grant, exact Claim attachment, forms/evidence and recovery. It
does not create or edit the Agent or the Expression's earlier topology Claim
stream; evidence URLs remain declared addresses, not remote source evidence.

### Declared source Claim creation

The same `tos-native-owner-command source-commands --invocation /absolute/native-invocation.json` entrypoint
dispatches separately delegated `claims.create` to `rust/crates/tos-command/src/source_claims.rs`.
It creates one atomic package of up to 32 declared source Claims
over existing subjects and identity or explicitly delegated typed values,
including different subjects and profiles in the same batch.
It neither creates those identities nor revises existing Claims. Read-only
access remains separate; this route requires its own delegation.

The protected `tos_local_claim_create_owner_v1` configuration has exactly:

- `uid`, `principal_id`, `maker_type`, `source_root`, `authority_ref`, `expires_at`;
- `source_path`: `ToS/source-witnesses/relations/<new-package>/source-claims.jsonl`;
- `provenance_event_id`: one new `tos.event.*` identity;
- `allowed_operations`: a subset of `["claims.create"]`;
- `allowed_claim_ids` and `allowed_predicates`: at most 32 exact values each;
- `allowed_subject_refs`, `allowed_object_refs`, `allowed_evidence_refs`: at
  most 128 exact values each. Evidence citation permission must already be
  authorized by the source owner; this allowlist is not rights clearance.

`tos_local_claim_create_owner_v2` has those same fields plus the required
`allowed_object_values`: at most 32 distinct exact JSON objects. This v2 grant
delegates temporal value objects; old v1 grants retain identity-only scope.
For a relative date, its `anchor_ref` must additionally appear in
`allowed_object_refs` and resolve as a historical situation. Writing requires
its own grant, independent of source prose, schema declarations and inspection.

`tos_local_claim_create_owner_v3` has the same exact fields and bounds as v2
and can additionally delegate `structured-value-v1` profiles. Its allowlist contains exact JSON values; the source profile still validates
the specific literal kind, shape and domain. Unknown nested references are
not resolved or executed. Temporal values retain their declared anchor scope.
An anchor needs object permission even when it is also the Claim subject.
A v2 configuration cannot acquire this wider reader through a registry update;
current value/profile permissions are checked on exact replays too.

`tos_local_claim_create_owner_v4` retains the v3 fields and additionally
delegates the explicit `structured-reference-value-v1` reader. Its fixed
`object.members` slot has the concrete types and bounds declared by the
source relation profile. Every member needs separate `allowed_object_refs`
permission, even when it is also the focal subject; the entire qualified
object must be in `allowed_object_values`. Unknown nested references in this
or older readers remain inert. V1/v2/v3 grants cannot gain member-bearing
semantics through a registry edit. Preparation, commit and exact retry bind
the current complete source/native closure of every member. A retained
receipt is not a substitute for currently available or permitted evidence.

The request uses `schema_version: tos_local_source_command_v1`:

- `describe` returns the actual delegated operations, exact scope, source
  Claim descriptors, configuration digest and target existence.
- `prepare-create` adds `claims` and returns prepared file digests,
  `expected_dependencies` and `source_bindings`, without source writes.
- `claims.create` adds `claims`, `command_id`, `expected_configuration`,
  `expected_revision: null`, `expected_dependencies` and `expected_inputs`
  (the exact prepared `source_bindings`). Unrecognized request fields fail.

Selected object bindings retain source path, raw and canonical record digests,
source schema version and record version when the native shape has one; absent
native record versions stay null. Evidence bindings retain path, digest, line
when relevant and evidence kind. These values travel in the request and receipt,
not merely an opaque digest. The broader dependency digest also binds the
catalog inputs, consumed profiles/schemas, evidence, anchors, provenance and
implementation. Changes before or during staging conflict. This operation reads a bounded scan of source metadata.
Temporal `source_bindings.values` separately names each Claim's exact value,
digest and declared range types. The identity bindings contain the subject and
any relative anchor, never a fabricated identity for the value. This distinction
also applies to the independently selected assessment source adapter.

Every Claim must have a delegated ID, predicate, endpoints and maker; its exact
schema and inherited domain/range must pass the shared profile reader. Initial
version is 1, review posture is unreviewed, visibility is public metadata, and
assessment/supersession are not granted. Evidence and counterevidence must be
authorized and resolve through the existing source reader; alternative Claim
IDs must resolve in existing sources or this complete batch. Unknown source
extensions are retained, never executed. One bad member rejects the whole batch.

Publication is five files: `source-claims.jsonl`, `source-create-request.json`,
`source-create-environment.json`, `source-create-provenance.jsonl` and
`source-create-receipt.json`. Shared account/path checks, source lock, fsync and
Linux no-replace directory commit are reused. Capture names the actual Claim
serializer and both implementation modules; it retains its unsigned pre-commit buffer-serialization scope. Upstream
research and content assessment keep their own provenance. The 1 MiB
request/Claim-stream ceilings do not reserve host storage.

Exact replay rechecks current delegation and verifies the original request,
Claim refs, selected source bindings, file closure and every stored byte digest.
It returns the old receipt, not current admission. Occupied destinations,
symlinks, corrupted receipts and reused identities fail closed. Abrupt loss
leaves any uncommitted staging outside source scanners; retry does not delete
it. A lost response after commit returns the same receipt without republishing.
Uncooperative same-account editors must still remain quiescent during the
transaction; this is not a sandbox or a cross-file reader snapshot.

Claim correction/history, scoped assessment/admission and human-form production
remain their own operations. Together these separately owned operations support source growth, review and
publication.

### Forms of a declared Claim

`tos_local_claim_form_owner_v1` is a separate delegation for the same
`describe`, `prepare`, `apply` and `form.create`/`form.revise` grammar. Its
fields equal `tos_local_source_command_owner_v1` plus one exact `claim_id`;
`source_path` must instead identify one existing `source-claims.jsonl` below
the source home, excluding catalog, payload and local-content paths. A
`claims.create` permission never implicitly grants form writing or vice versa.

Discovery advertises `claim.statement` only when the selected Claim has a
complete statement. Preparation copies it and binds the entire Claim as
mandatory context, using the same reader in
[`HUMAN_FORMS.md`](../../ToS/doctrine/HUMAN_FORMS.md). The target is the
adjacent `source-claims.<sha256-of-UTF-8-claim-id>.human-forms.json`, returned by
discovery; no caller path or source row number chooses it. The source Claim
bytes, IDs and initial review status are unchanged.

Resolution reads only the selected protected stream (at most 1 MiB) and
declared registry/schema inputs (at most 128, total snapshot at most 8 MiB).
It requires exactly one occurrence of the selected ID, the current declared
schema, predicate, assertion layer and public-metadata visibility. It does
not crawl endpoint records: source and graph validators retain endpoint
existence/domain/range checks. Claim validation and admission retain their separate validation and assessment
routes.
The source contracts appear in discovery and the new form receipt and are
bound into `expected_configuration`; changing a source contract invalidates
prepared writes. Current schema and visibility are also checked before replay.
The existing form-set limits, shared locks, atomic rename, exact predecessor,
source/conflict checks and local-account security boundary are reused.

An initial `claims.create` retry permits separately retained form sets and
their empty lock files only for its original Claim IDs. Their retained history
and subject binding are checked, while its original five files and receipt
stay byte-bound. Unknown extra files, aliases and mismatched form subjects
remain conflicts/corruption; a creation retry does not vouch for current form
quality. Declared Claim corrections use the separate operation below.

### Correction of a declared source Claim

`rust/crates/tos-command/src/source_claims.rs` implements `claim.revise` through the same explicit
source command entrypoint. The selected Claim keeps its ID, predicate,
identity endpoints, original maker/provenance and initial review flag. Ordinary
v1/v2/v3/v4 corrections also preserve its assertion layer; only the separate exact
layer-classification route below can correct that field.
Its `claim_version` advances once. The correction issuer and reason belong to
the new history receipt; the original maker retains attribution for the initial act, while each
correction records its own issuer. Assessment, identity
merge/split, reattribution, visibility and publication are separate operations.

The protected `tos_local_claim_revision_owner_v1` configuration contains exactly
`uid`, `principal_id`, `source_root`, `source_path`, `authority_ref`, `expires_at`,
`claim_id`, `allowed_operations`, `allowed_fields`, `allowed_evidence_refs` and
`allowed_form_ids`. The path selects an existing `source-claims.jsonl` below
`ToS/source-witnesses/`, outside catalog/payload/local-content. Only
`claim.revise` may be delegated. Allowed fields are a subset of `qualifiers`,
`evidence_refs`, `counterevidence_refs`, `alternative_claim_refs`,
`supporting_quotes`, `epistemic_status` and `confidence`; the exact Claim schema
still constrains their values. A qualifier patch merges its explicit top-level
keys and preserves unmentioned keys. Other selected fields are replaced as
explicit values. New or changed evidence lists require the independent exact
allowlist. Unchanged evidence remains checked and source-bound.

`tos_local_claim_revision_owner_v2` adds required `allowed_object_values` and
`allowed_object_refs` (the same bounded exact-value and anchor scopes as create
v2). It may include `object` in `allowed_fields` for correction of a declared
temporal value. Both old and new objects must be values, never identity
endpoints; the shared temporal contract and exact predicate profile still
apply. A date, interval, relative order or unknown can be corrected without
changing Claim identity, but independently competing dating judgments retain
separate Claim IDs. An object correction replaces the complete explicitly
supplied value; callers preserve unknown extensions in that value. The old
complete value remains retained in the exact predecessor archive. A v1 grant
cannot gain this operation through a new source schema or a replay.

`tos_local_claim_revision_owner_v3` retains the v2 configuration shape and
adds correction of declared structured values. It preserves the selected
Claim's predicate, subject, attribution, exact predecessor bytes and current
source-copy forms. Its old and new values must satisfy that same profile;
changing value kind is not a correction route. The exact allowlist and current
revocation checks apply to preparation, commit and replay. V2 remains temporal;
descriptive v1 correction still cannot replace any object value.

`tos_local_claim_revision_owner_v4` retains the v3 fields and additionally
supports qualified reference values. The current and proposed members must
all have the independent object-role permission, and the exact proposed
value must be allowed, including for wording-only correction. The focal
subject stays fixed and remains a member under the motif profile; removing
it requires a separately formulated successor Claim. Membership correction
retains the same qualified proposal's identity, exact predecessor bytes and
source-copy form history; motif assessment and Sign issuance retain their separate routes.
Current complete source/native grounding is checked again on retry, including
the historical result when a subsequent correction has removed one of that
request's members. Older grant versions do not gain this reader mode.

`tos_local_claim_layer_revision_owner_v1` explicitly delegates assertion-layer
correction; v1/v2/v3 retain their existing scopes. It has the v1 configuration fields plus mandatory
`allowed_layer_transitions`: at most 32 distinct exact `{"from": "…", "to": "…"}`
pairs. Wildcards, empty layer names, repeated pairs and `from = to` are refused;
an empty list revokes transitions. `allowed_fields` can contain only
`assertion_layer`. Both `prepare-revise` and `claim.revise` require an explicit
`layer_transition` matching one delegated pair, the actual predecessor layer
and the proposed field value. The corrected Claim must still satisfy its
existing schema and predicate profile. A grant cannot admit a forbidden layer.

This route corrects the layer classification of the **same already recorded
assertion**, preserving its proposition, attribution and original judgment. For example,
an assertion whose statement and basis already identify an analyst's inference
can have an erroneous `scholarly_report` label corrected to `linguistic_analysis`.
Changing “S reports P” into the operator's own “P” changes the assertion and
requires a new Claim and an explicit successor relation; this writer does not
implement that successor route. In the bounded layer transition, no qualifier,
evidence, endpoint, schema, maker, visibility or admission field may change at
the same time. The authored reason and exact request remain in the receipt.

The separate grant keeps existing editing authority from silently gaining
layer authority. Current revocation is checked before a retry; the historical
`from` is checked against its retained predecessor, not the now-corrected head.
Shared history is reconstructed from retained requests without borrowing a
current grant for a different Claim. The old full Claim and forms remain
inspectable, while every current selected-Claim form is explicitly rebound.
Previous source-bound assessments are not rewritten or transferred: their
exact source version is historical, and an assessment configuration retaining
the previous layer fails its existing source/scope check. Reassessment and
scoped admission remain separate owner operations.

Requests use `tos_local_source_command_v1` and the record-correction grammar:

- `describe` returns the selected exact source, whole-package revision and
  permitted operation/fields/forms without writing.
- `prepare-revise` takes `fields`, `forms` and an authored `reason`, and returns
  the proposed source/forms, `expected_dependencies` and `source_bindings`.
- `claim.revise` additionally takes `command_id`, `expected_configuration`,
  `expected_source`, `expected_revision`, `expected_dependencies` and
  `expected_inputs` (the exact prepared `source_bindings`).
- `inspect-version` takes an exact predecessor `source` and returns its Claim
  and every archived package byte binding. It does not select an archive path
  from request prose or return an uncommitted archive as history.

The current profile, inherited endpoint domain/range, source evidence and
alternative Claim refs are checked by the same grounding used for creation.
Selected source paths, raw/canonical digests and versions survive in request
and receipt, not just an opaque dependency digest. Existing selected-Claim
forms must all be explicitly rebound; source copies include a ready statement
and the full qualified Claim context. Prior forms and exact prior source refs
remain retained. Another catalog subject's current or prior form identity
cannot be reused, including historical Claims. Each newly written form retains its own calibration and assessment
requirements.

Only the selected JSONL row is serialized; all other rows retain exact bytes
and order. Stream, selected forms and `claim-revision-history.json` are committed
in one directory exchange using the existing stable source writer lock and
byte-verified `.record-revisions/` archive. Other companion bytes remain intact.
The shared history orders corrections of different Claims, reconstructs each
successive stream, and rejects unrecorded changes, missing predecessors and a
noninitial stream without history. The initial baseline has all Claim versions
1; importing a higher-version stream needs an explicit history migration,
not an invented version-1 origin. Initial creation replay verifies its original
five files through the earliest archive while leaving current corrections alone.

The same 64-file/8-MiB flat-package, 2-MiB file, 128-correction and existing
form-history limits apply; the Claim stream additionally stays within 1 MiB.
Exact retries return the old receipt and fresh current source/forms separately,
and current revocation still blocks a retry. Crash recovery, abandoned staging,
Linux exchange, same-account trust and concurrent-reader limits are those of
record correction above. Every archive remains retained, addressable source history. This implementation still scans
source metadata for grounding and can inspect up to 128 bounded archives. Its costs follow the metadata scan and selected archive chain; graph rebuilds
and cross-subject operations retain their own contracts.

### Selected private identity file budget

A protected filesystem Profile or Claim owner grant may include the optional integer
`max_held_identity_files`, from 257 through 4096. The selection bounds the
entire simultaneous held-file set, including identity files and other source
dependencies, and also bounds the collision inventory file count. It is a
resource ceiling only; it grants neither source access nor publication. With
no selection the existing 256 held files and 2048 inventory files remain. Byte,
entry, namespace, expiry, selected-cut, rights and atomic publication guards
remain unchanged. The protected grant bytes are part of the owner dependency
fence and are rechecked before publication and replay. Each held source file
also retains its parent descriptor, so the actual process descriptor limit must
cover roughly twice the selected held-file ceiling plus worker and process
headroom. This ceiling does not promise that the operation fits the time or byte
budgets. An independent protected text-read selection is still required for a
private-bound Occurrence; selecting this budget does not issue one. The PG
owner rejects this filesystem-only selection, including malformed values,
instead of silently ignoring it.

### Separately delegated identity proposals

`tos_local_identity_proposal_create_owner_v1` reuses the bounded public Claim
package transaction (`describe`, `prepare-create`, `claims.create`) exclusively
for `identity_transition_proposal`. Its configuration has the public v4 creation
fields plus `allowed_related_claim_refs`: at most 33 distinct exact Claim refs
for predecessor proposals and unresolved links. `allowed_object_values` grants
the entire exact plan, while `allowed_object_refs` separately grants every
participant role, including the focal subject. No other predicate is writable
under this configuration. Command discovery exposes this handler and its exact
source schema without granting its operations.

The separately selected `tos_local_identity_proposal_create_owner_v2` has the
same transaction grammar and exact allowlists, but writes exclusively
`subject_identity_transition_proposal` under the
[v2 subject proposal contract](../../ToS/contracts/subject-identity-transition-claim.schema.json).
It supports the prior concrete source identities plus explicitly opted-in
declared semantic subject metadata. Eligibility requires the exact source
profile capability and typed public metadata return, not an ID prefix or a
generic semantic role. The v1 and v2 configurations are not interchangeable.

`tos_local_identity_proposal_revision_owner_v1` similarly reuses `describe`,
`prepare-revise`, `claim.revise` and `inspect-version`. It has the v4 correction
fields plus the same related-Claim allowlist. The current and proposed whole
values and each participant must remain in scope, even on wording-only changes
or exact retries. Descriptive correction can change grounds, counterreading,
scope, unresolved-link questions, evidence and qualified statement. It cannot
change operation, participant IDs or exact versions, mapping, predecessor
proposal, focal subject, layer, maker or admission. A changed plan gets a new
Claim with an exact predecessor proposal, not a rewritten subject identity.
`tos_local_identity_proposal_revision_owner_v2` applies that same correction,
form and history engine only to v2 proposals. A new v2 proposal can supersede
an exact v1 or v2 proposal; old v1 revision and succession stay v1-only.

Preparation and publication resolve each participant through
`MetadataVersionReader.resolve_typed(exact_ref)` and related Claims through
`ClaimVersionReader.resolve(exact_ref)`. Missing, stale, corrupt, restricted or
over-budget returns fail closed; no current-version fallback is performed.
Both snapshots are checked for drift and their source provenance and descriptor
bindings survive in the receipt. These readers resolve metadata and retained history within their explicit byte
budgets. Existing source locks, no-replace publication, exact package archives,
form rebinding, configuration rechecks and retry validation remain the common
transaction engine rather than a second identity command store.

Proposal history and source-copy statement forms retain the complete plan.
The [source law](../../ToS/doctrine/semantic-interchange/README.md#identity-transition-proposals)
keeps proposed subject transitions, proposal succession, current research
assessment and actual identity changes separate. This implementation performs
only the first two kinds of source recordkeeping. It does not merge, split,
redirect, mint successor subjects, rewrite links or copy an old admission.

The current public assessment selector still selects current source records,
not arbitrary historical versions. If a participant advances, the frozen plan
remains readable through exact history but cannot be assessed by substituting
the later record. Historical assessment selection requires its own bounded
owner route; neither this proposal grant nor an old assessment grants it.

### Native Artifact metadata creation

`tos_local_artifact_create_owner_v1` delegates one new physical Artifact v2
metadata record under `artifacts/<tradition>/<site>/<physical-identity>/`. The
target directory must be absent; existing independent input records live
outside it. The actual `artifact_id` owns the physical identity; provider
labels remain attributed descriptive metadata.

The grant fixes the record ID/path, principal and maker kind, creation event,
source-copy form IDs, allowed operation and expiry. `source_bindings` contains
exactly `rights_ref`, `discovery_ref` and `research_ref`, each as
`{"ref": "ToS/...", "sha256": "<64 lowercase hex>"}`. The same whole binding
object is required in `prepare-create` and `source.create`; it survives in the
retained request and cannot be replaced by caller input or a later live grant.
All three inputs must already exist as public source metadata. Rights must
cover the Artifact with metadata-only visibility/redistribution. Discovery
must address that exact Artifact identity. Research bytes are bound, not
assessed, and no remote source or provider is fetched.

V2 catalog fingerprints distinguish retained (`captured: true`) and
unretained response bytes. A retained fingerprint must match a captured
snapshot in the independently bound discovery record: exact response URL,
SHA-256, acquisition SHA-256 and byte size. The snapshot's discovery event
remains distinct from native metadata serialization. The metadata check requires
the exact source account for a retained claim; private snapshots and capture
authentication remain outside its inspection. Public access and embedded HTML
transcription require their own admission.
Historical v1 fingerprints and all false authority fields stay unchanged.

Only a version-1, unreviewed v2 Artifact with no performed human review and an
empty `philosophy_planting_refs` is writable. The source schema retains false
text/semantic/graph/canon/publication authority. Text, transliteration,
translation, images, payloads, private inputs and owner-local paths are not
accepted into this metadata route. Earlier generic grants remain unchanged.

The shared source writer atomically publishes the Artifact, its source-copy
form set and the four conventional `source-create-*` capture companions.
The event records `native-artifact-metadata-serialization`. Its outputs are
metadata and forms; rights, discovery and research are exact inputs. The event
is unsigned mechanical evidence, distinct from discovery, acquisition, rights
assessment, planting and admission.

Creation replay and the read-only origin verifier preserve the initial bytes
through separately retained native descriptive corrections and committed
selected-file history. They do not enumerate or read Artifact descendants.
The origin verifier never consults mutable grants or runs the foundation
validator recursively. Missing/corrupt capture fails closed. If exact earlier
input bytes are no longer available, the original origin cannot be rebound to
today's rights or research; availability must be restored by their owner.
Legacy discovery provenance keeps its separate unchanged validation branch.

Run `rust/crates/tos-command/src/source_artifact_native.rs`, native
descriptive revision and exact metadata reader tests, then relevant source
foundation, discovery and topology checks. These synthetic checks cover
serialization and retained history.

## Native Growth behavior and legacy fate

The owner assessment is `native-behavior-coverage.json`; the source-command
catalog owns handler and operation membership. A complete mechanics lane runs
Growth. The bounded native cohort and Python comparison routes are separate;
reviewed assertion equivalence and an installed consumer establish their own
scope independently of file or module counts.

## Current disposition

The retained `test*.py` assertions contain native operational predicates,
Python adapter/API predicates, synthetic fixture/oracle mechanisms and semantic
assessment boundaries. Each follows its actual owner route below. Python APIs and
reference fixtures remain explicit comparison surfaces where their owners still
retain them; they do not gate native execution merely by existing. Retire a
Python assertion set only after its operation has moved to the native owner and
its unique bounded behavior has an active native route or a clearly scoped
external-evidence limit. Native operational failures stay executable in the
source-owned native pipeline. Native source-unit,
Conformance, installed invocation and actual consumer results keep their individual
scope. Selected Public, OCR or HTTP success does not accept sibling assertions.

The table names the shortest existing owner route for completing each unresolved
assertion set. “Native candidate” means source inspection found relevant code; it
does not claim that every negative control or lifecycle is covered. Reuse these
owners and existing cases before proposing a new engine or broad test cohort.

The Python owner-OCR callers and v6 assessment journal entry are retired. Owner
signed recording and comparison remain on the native Rust route. ToS authenticates
the independently pinned receipt and signer with its native P-256 verifier. The eight
`owner_page_ocr_preflight_tests` and five `source_text_owner_ocr::tests` are
synthetic Rust checks, including genuine signature verification and forged-signature refusal. They make no claim about a live owner process or
source-visible assessment. The two signed `command_owner_text_cases.rs`
comparisons remain ignored until their retained producer inputs and admitted
native products are selected.

| Reference assertion source (`tests/`) | Native candidate (`tos-command/src/`) | Existing conformance candidate | Review or completion still needed |
| --- | --- | --- | --- |
| `test_assessment_read_batch.py` | `source_native_private_assessment_cli.rs::run_public_v2_batch` and `rust/crates/tos-command/src/source_forms.rs` | `command_owner_text_cases.rs::native_public_v2_assessed_form_batch_matches_builder_and_rechecks_drift` | native parity, source/path/subject binding, duplicate rejection and withdrawal assertions present; selected grammar, actual expiry and post-fsync grant/publication controls now have real native invocation assertions; matching execution still required |
| `test_claim_version_reader.py` | `source_read_owner.rs` | `command_claim_cases.rs` | continuous interleaved history, later sibling corruption, catalog/current-rights drift and batched exact reads |
| `test_historical_claim_adapter.py` | `source_legacy_historical_claim.rs` | `command_legacy_claim_cases.rs` | interleaved record/claim predecessors and separate form replay under current grammar |
| `test_historical_claim_assessment.py` | `source_private_assessment_sources.rs` | `command_owner_text_cases.rs` | historical wording disclosure and exact historical source currentness |
| `test_human_forms.py` | `source_forms.rs` | `command_form_cases.rs` | language provenance, mandatory context, template amplification, quality basis and freeform revocation |
| `test_identity_proposals.py` | `source_claims.rs` | `command_claim_cases.rs` | both identity versions: corrections, exact participants, topology/delegation and cold replay |
| `test_knowledge_assessment.py` | `source_assessment_journal.rs` | `command_owner_text_cases.rs` | all journal replay/conflict/currentness and reviewer revocation predicates |
| `test_metadata_version_reader.py` | `source_read_owner.rs` | `command_record_cases.rs` | every current/historical semantic family and complete-chain negative controls |
| `test_native_layer_quality_journal.py` | `source_private_assessment_layers.rs` | `command_owner_text_cases.rs` | quality closure, own dependencies, renewed basis, deferred form refs and reviewer revocation |
| retired `test_native_page_ocr_assessment.py` | `source_private_assessment_layers.rs::owner_page_ocr_preflight_tests` | `command_owner_text_cases.rs::native_private_assessment_v6_retained_signed_ocr_comparison_preserves_source` and `native_retained_page_ocr_assessment_comparison_preserves_original_and_signed_capture` | eight active pure Rust cases cover exact image/disclosure grants, metadata-only refusal, image fixity drift, and the v5 boundary; signed end-to-end comparison remains ignored without retained producer inputs and admitted native products |
| retired owner-OCR bridge cases from `test_source_text_layer_commands.py` | `source_text_owner_ocr.rs::tests` | the same two signed owner-OCR comparisons above | five active Rust cases cover genuine pinned P-256 signatures, forged-signature and ambiguous-JSON refusal, UTF-8/source/receipt binding, metadata-only replay and retained-page capture posture; no external verifier or producer checkout is used |
| `test_native_text_assessment.py` | `source_private_assessment_layers.rs` | `command_owner_text_cases.rs` | initial extraction comparison, source-language scope and retained text mismatch |
| `test_native_text_layer_assessment.py` | `source_private_assessment_layers.rs` | `command_owner_text_cases.rs` | derived lineage comparison and exact current quality withdrawal |
| `test_occurrence_assessment_guard.py` | `source_private_assessment_sources.rs` | `command_private_claim_cases.rs` | native occurrence bound-source quality and late withdrawal |
| `test_occurrence_growth.py` | `source_private_profile.rs` | `command_private_profile_cases.rs` | exact occurrence binding, rights and lexical qualified endpoints |
| `test_owner_local_assessment.py` | `source_private_assessment_sources.rs` | `command_owner_text_cases.rs` | private/current source policy, missing disclosure before I/O, expiry and lock-time drift |
| `test_owner_local_claim_assessment.py` | `source_private_assessment_sources.rs` | `command_private_claim_cases.rs` | Claim whole-context read/current-quality closure and disclosure |
| `test_source_alignment_commands.py` | `source_text_owner.rs` | `command_text_cases.rs` | alignment create/replay plus exact source-layer/target-language rights |
| `test_source_artifact_commands.py` | `source_creation.rs` | `command_artifact_cases.rs` | artifact creation replay/history/forms and owner/storage/rights refusal |
| `test_source_claim_commands.py` | `source_claims.rs` | `command_claim_cases.rs` | all configured Claim create/revision families, scope/rights/current replay |
| `test_source_collection_commands.py` | `source_collection_membership.rs` | `command_collection_cases.rs` | ordered full/selected membership, bounds and exact predecessor replay |
| `test_source_command_discovery.py` | `source_native_cli.rs` | `command_form_cases.rs` | handler catalog selected-source currentness, no-I/O discovery, exact operations |
| `test_source_command_http.py` | `source_command_http.rs` | command HTTP unit/installed consumer | HTTP framing, held startup and native prepare/apply/lost-response consumer |
| `test_source_commands.py` | `source_forms.rs` | `command_form_cases.rs` | existing-source forms, canonical-node grants and retained predecessor refusal |
| `test_source_edition_commands.py` | `source_expression_edition.rs` | `command_edition_cases.rs` | edition source bindings, parent grammar, replay and rights |
| `test_source_expression_commands.py` | `source_work_expression.rs` | `command_work_cases.rs` | expression full exact source relationships and cold retained replay |
| `test_source_item_commands.py` | `source_item_adoption.rs` | `command_item_cases.rs` | item adoption exact companions/fixity/rights and pending/collision refusal |
| `test_source_item_deposit.py` | `source_item_adoption.rs` | `command_item_cases.rs` | actual payload custody/fixity, interrupted deposit and safe restore |
| `test_source_link_commands.py` | `source_object_link.rs` | `command_object_link_cases.rs` | endpoint authority, historical exact bindings and replay |
| `test_source_metadata_transactions.py` | `source_metadata_publication.rs` | `command_metadata_publication_cases.rs` | multi-file publication/rollback/response-loss and same-store recovery |
| `test_source_native_metadata_commands.py` | `source_revisions.rs` | `command_record_cases.rs` | native selected current topology and historical retry with current software grammar |
| `test_source_owner_claim_commands.py` | `source_private_claim.rs` | `command_private_claim_cases.rs` | private Claim lifecycle, current delegated scope and physical replay |
| `test_source_owner_profile_commands.py` | `source_private_profile.rs` | `command_private_profile_cases.rs` | private profile lifecycle, occurrence bindings and retained materialization replay |
| committed selected-record observation | `source_revision_observation.rs` | `command_metadata_publication_cases.rs` | detached exact before/after, current committed transaction and reverify before derived publication |
| `test_source_public_native_commands.py` | `source_public_text_owner.rs` | `command_public_text_cases.rs` | public current-rights/current-assessment create/replay and complete exact fixtures |
| `test_source_responsibility_commands.py` | `source_expression_responsibility.rs` | `command_responsibility_cases.rs` | qualified responsibility endpoints, external rights and retained replay |
| `test_source_revisions.py` | `source_revisions.rs` | `command_record_cases.rs` | flat/profile/native exact predecessor chain and independent current authority |
| `test_source_selected_revisions.py` | `source_revisions.rs` | `command_metadata_publication_cases.rs` | selected-file history/current publication, retained receipt and reaper/recovery |
| `test_source_text_layer_commands.py` | `source_text_owner.rs` | `command_text_cases.rs` | structural extraction, correction, normalization and supplied OCR/transcription grants; owner-signed recording has its portable native signature route above |
| `test_source_text_unit_commands.py` | `source_text_owner.rs` | `command_text_cases.rs` | private unit bootstrap, scopes, exact original retry and retention |

## Completion route

1. Select one named behavior from the existing source command catalog or the
   source-owned observer/assessment contract. Review actual positive and negative
   predicates in the reference assertions against the existing native owner.
2. Reuse the corresponding existing native unit or Conformance case. Add or adjust
   a case only for a concrete uncovered significant predicate. Keep real fixtures,
   rights, current authority, retained bytes and transaction boundaries intact.
3. Obtain a matching native product and run the maintained installed entry and
   consumer with its selected source/fixture binding. Preserve prior received
   results when applicability is proven, without relabeling unrun siblings.
4. Record old API, fixture and semantic fate at their owners. The actual native
   default executes the contract's `native_execution_sequence` together with
   other mechanics checks under one lane deadline. Its source-derived partition
   includes owned command classes, generic native residuals and exact isolated
   lifecycle cases. An assessment flag cannot replace that execution plan.

Plain execution runs this finite native pipeline. The
[mechanics validation guide](../VALIDATION.md) records the current two-home
`--native-contracts-only` mismatch and direct checks. Historical Python
assertions remain comparison evidence. Installed success requires a matching
Ops product and actual execution.

## Concrete source repair from the classification

`source_claims::family` selects exact published owner-schema strings within the
numeric protocol bounds. Its regression module covers canonical create/revision
versions and rejects `v01`, `v001` and `v+1` aliases.

## Native class planning

`tos-ops-mechanics-plan --repo-root ROOT --growth-native-plan` describes the
existing native assertion pipeline without building or executing it. The installed
Ops producer reads `native_test_routes` from this owner's contract, then reads the
actual Cargo target declarations and Rust module declarations. Conformance command
classes follow the runner's declared command modules; their number and method
membership are not copied into another maintained list. Additional selected
library routes resolve through real module declarations and explicit `#[path]`
attributes, including the selected Record publication/recovery class.

The plan contains package/target kind/name, actual source path/SHA, native class
filter and declared assertions. Ignored OCR cases remain explicitly marked and
need their retained producer inputs; a normal class run must not count them as
executed. This plan provides selectors for the existing conformance product and
library-test product routes. OPS retains product preparation and execution; there
is no Cargo child, interpreter fallback, installed product search or execution in
this source-plan command. The default consumes this same class authority through
the selected maintained sequence. A successfully described class is not an
executed or accepted assertion.

The first useful existing classes are the Claim retained historical reader and
exact protocol dispatch, command HTTP framing/trickle custody, and selected Record
publication/cold replay/recovery. Public/private assessment, quality withdrawal
and assessed batch classes follow in the existing Conformance runner. The method
matrix remains the reference-to-owner navigation: review genuine predicate gaps
within those classes instead of reproducing one test per Python method. Native
source pipelines, selected fixture/oracle dependencies and semantic assessment
retain their distinct owners.

## Maintained native assertion pipeline

`rust_workspace` now has one native Growth class marker rather than repeated
command-class lists and complementary hand-maintained skip lists. The native
validation executor expands that marker from the same source-derived plan. It
uses the actual test images selected from the successful current-lane Cargo
`--no-run --message-format=json` output. Conformance identity continues through
the existing product selector; the command library test image is separately
selected by actual Cargo package/target/profile metadata and hashed. A missing,
ambiguous or changed product refuses before dispatch. Class execution starts the
prepared test image directly under the existing pidfd/subreaper deadline and
cleanup owner, with no Cargo child in that class phase.

Generic workspace and Conformance remainder commands receive exclusions from
those same declared classes. The existing exact isolated source/Work/Artifact/
Claim/Collection steps retain their own source-owned selections and are excluded
from grouped class execution. A class fully owned by an isolated step adds no
second invocation. Other packages, other Conformance modules, segment and WASM
routes remain in the original lane. Native class results must report the actual
declared active/ignored scope; zero-test success or a mismatched prepared image
cannot pass. Ignored retained OCR inputs keep the source's explicit external
input posture, and their separate received executions keep their own scope.

Native pipeline execution, installed default use, method predicate coverage and
semantic/source-language judgment remain separate evidence.

## Assessed batch predicate disposition

The retained batch module exercises multiple owners. Its preparation/read-call
counts protect the Python reference session's cache implementation; they do not
require the native worker to make that number of Python calls. The maintained
`AssessedFormSnapshot` already requires a selected native invocation, while
`for_retained_reference_fixture` explicitly retains the comparison API. The
production-snapshot refusal and shared builder snapshot assertions protect that
Python adapter/consumer seam and remain applicable there.

Native `run_public_v2_batch` owns exact request identity, path/subject bindings,
no-partial result, current journal/configuration/source/grammar guards and final
currentness. The existing real native batch case compares retained ready/not-ready
results and withdrawal, and now sends invalid final version, cross-subject,
source-path, form-path and duplicate selections through the native invocation.
It asserts journal and source/form bytes remain unchanged after each refusal.
These predicates have executable native assertions; they are not pending native
implementation simply because their old Python methods remain available.

The same real consumer case now changes the selected immutable grammar object
and requires cached/fresh refusal, exercises the actual native clock with expired
authorities and requires no ready wording, and changes grant or pending/ready
publication state after staging fsync before the final native guard. It asserts
no candidate/staging visibility and no journal writes. These are executable native
predicates requiring matching execution, not semantic oracles. The Python
root-keyed validator-cache behavior remains a reference API mechanism; native
workers load the exact selected cut instead. Source schema/registry selection is
owned by that cut and the native source-currentness readers. Language adequacy and quality
judgment stay with the source-visible assessment route. Neither a source test
declaration nor parity with retained synthetic fixtures accepts those judgments.

## Finite native default and retained fate

The executable owner set is the selected native Cargo targets: declared command
Conformance modules, selected library reader/framing/publication regressions,
remaining workspace unit tests and exact isolated lifecycle cases. Class and
assertion names derive from source declarations; the residual tests preserve
unselected native owners. The lane prepares native/schema/access images, selects
current Cargo test products and executes them with the existing custody, output,
per-step and whole-lane limits. An unavailable producer input remains explicitly
ignored and cannot be reported as executed.

The matrix's operational rows route to these native owners. Its Python-only
contracts concern the explicit comparison/fixture API or maintained Python
projection adapter where identified; those APIs keep their own executable
checks. The actual batch case exercises the native/adapter boundary and complete
publication pipeline. Source-visible judgment, including adequacy of original
wording, translation, rights and authorized quality, stays outside mechanical
closure. The retained method census remains navigation; the default does not
consult its size, rename assertions into approval or claim their blanket removal.

## Selected metadata publication and recovery

The native `tos-command` owner maintains selected metadata publication and
recovery. `source_metadata_publication.rs` owns the transaction lifecycle,
`source_work_transaction.rs` owns the selected publication epoch, and
`source_revision_observation.rs` owns read-only proof for a committed selected
source correction. These Rust paths are the maintained implementation.

The committed-record observer retains the exact selected source and archive
bindings, requires a current committed transaction, and rechecks the publication
epoch while the caller reads. It does not replay the command, acquire a derived
publication lease, complete graph dependencies, or grant semantic, rights,
publication, or canon authority. A downstream publisher must retain its own
source-owner lock, check unrelated inputs, and reverify before committing a
derived output.

See [native behavior coverage](README.md#native-growth-behavior-and-legacy-fate) for the maintained
assertion and consumer routes. The former Python snapshot and transition
adapters are retired; historical conformance provenance remains evidence of
the prior implementation and is not an active runtime dependency.

## Explicit local source-command transport

`tos-native-owner-command http` exposes the source-command front door on
**127.0.0.1 only**. This separately started source-owner service dispatches
commands under existing delegations and their exact target, identity and
assessment requirements. Read-only access HTTP `/api`, WebMCP and native
access MCP keep their existing contracts.

The local operator selects one existing protected owner configuration, one
private credential file, one exact browser origin and one unused port:

```sh
tos-native-owner-command http \
  --owner-config /absolute/protected/owner.json \
  --native-invocation /absolute/protected/native-invocation.json \
  --token-file /absolute/protected/transport-token \
  --browser-origin http://127.0.0.1:44257 --port 44259
```

The protected native invocation selects the same owner configuration and the
installed owner executable identity. Every command enters the existing native
owner engine and rereads its current grant, rights, source and software fences.
The transport token grants no source authority. The native listener owns this
transport; the former Python listener is retired.

The native catalog is the authored packaged descriptor
`rust/crates/tos-command/src/source_command_catalog.json`. The installed
listener and CLI discovery read that same descriptor through
`tos_command::source_native_cli::discover_commands`. It needs no Python,
checkout, owner configuration or source target. Entries name their native
implementation and remain weaker than the selected owner and current
delegation. Update a descriptor with its owner grammar; the former Python
catalog builder is retired.

Use the host's resource launcher where required. Starting this process is
separate from installing, activating or restarting any existing application.
The credential file contains 64 lowercase hexadecimal characters from 32
cryptographically random bytes, optionally followed by one newline. It must
belong to the process account and have no group/other permission bits (0600).
Do not put the token in URLs, logs, repository files or browser persistence.
The caller must obtain it through its owner's private local channel.

## Shared wire grammar

Both humans and agents use these endpoints with a per-request HMAC-SHA256
authorization proof. The private token itself is **never sent**:

- `GET /commands/catalog` returns the unchanged `discover_commands()` grammar.
  Implemented operations are not currently granted operations.
- `POST /commands`, `Content-Type: application/json`, takes the exact existing
  source command object, without a wrapper or a client-selected owner path.
  Its successful response is the unchanged owner result or receipt.

For example, the configured handler's context is requested with
`{"schema_version":"tos_local_source_command_v1","operation":"describe"}`.
Choose subsequent preparation and mutation shapes from the handler-owned
[discovery contract](#op-doc-source-command-discovery), and retain the exact
prepared expected versions and original command ID. The transport never
replaces a stale expectation, synthesizes a grant or automatically retries.
The configuration is reread and its delegation rechecked by the existing
handler for every command, including replay. Token authentication is only
transport access: possessing it cannot enlarge the selected delegation.

The wire header is `Authorization: ToS-HMAC-SHA256 T:N:D:S`, where `T` is the
decimal Unix timestamp in seconds, `N` a fresh random 32-byte lowercase hex
nonce, `D` the SHA-256 of the exact request body (empty for GET), and `S` the
HMAC-SHA256 using the decoded 32-byte token over the compact JSON array
`["tos-request-v1",method,path,T,N,D]`. All digest/proof values are lowercase
hex. Requests must be within 30 seconds of the server clock. The server keeps
at most 1024 still-valid nonces, rejects reuse and fails closed at capacity.
An explicit owner-command replay uses a fresh transport nonce but the original
command ID and body; transport freshness is not source idempotency.

Every authenticated response includes `X-ToS-Response-Signature`: the HMAC of
the compact JSON array `["tos-response-v1",N,status,SHA256(body)]`, with numeric
HTTP status and the exact response bytes. The browser verifies this proof before
trusting a result or error envelope. A port impostor cannot learn the secret,
alter a request or forge a receipt. Missing, stale or invalid authentication may
produce an unsigned denial, which the client treats as unconfirmed delivery.
This protocol provides authenticity, **not encryption**: request contents remain
visible to a hostile local endpoint. Use it only for the intended local public
metadata workflow; confidential command payloads require an independently
authenticated encrypted owner transport. A hostile process with the same UID
can read the credential file and is outside this local-account boundary.

The browser client at
`access/web/constructor/source-command-client.mjs` uses a separately supplied
owner origin and an in-memory credential. It never writes through the read-only
access API, follows redirects, sends ambient cookies, saves the credential or
queues concurrent commands. A 30-second client transport deadline and explicit
close abort network waiting, never source execution or an already committed
change; the original command remains available for reconciliation.
`source-form-session.mjs` connects one bounded
source-copy workflow: describe, choose an owner-declared field/form, prepare,
inspect and apply. The owner preparation includes `prepared_materialization`
from the same reader used after apply, without publishing the in-memory
successor. The panel refuses apply when that readable exact-form preview is
unavailable; existing materializations never substitute for it. It retains the
exact command ID and expectations for explicit replay after uncertain delivery.
This is the source-copy form route only, not all human growth, interpretation,
assessment or profile-evolution operations. Real browser consumption and the
remaining human growth routes have their own acceptance checks.

## Boundaries and failure

Every request requires the exact numeric loopback Host and, if an Origin is
present, the single selected browser origin. CORS preflight allows only the
two routes and their corresponding methods/headers; it does not authenticate
or execute a command. All actual reads and writes require the private token.
Missing Origin is permitted for an explicitly authenticated non-browser agent.
The token is reread each time, so replacement, removal or unsafe permissions
revoke transport access. Browser same-origin compromise remains outside this
local-account boundary. Only the numeric `127.0.0.1` owner endpoint is accepted
by the client; the separately configured browser origin may use `localhost`.

Command bodies are capped at 1 MiB and responses at 4 MiB. Duplicate JSON keys,
ambiguous Content-Length, transfer encoding, non-JSON bodies and route/query
selectors are rejected before dispatch. The server handles one command at a
time, with a four-connection listen backlog, five-second idle socket timeout and
one 30-second absolute deadline over the request line, headers and body. Trickle
traffic cannot restart this total read deadline. This is **not a command execution deadline**: the selected owner operation keeps
its own work, locking, version, transaction and recovery bounds.

Errors are generic and do not reveal source prose, token or owner paths.
`outcome: not-dispatched` means rejection occurred before calling the owner.
Once dispatched, failure or missing/oversized delivery is `unconfirmed`, not
proof of rollback. An owner commit may precede a disconnected browser. Retain
the original request and reconcile through its existing idempotency/history or
recovery operation; do not generate another command ID as an automatic retry.
Reader publication, semantic acceptance, CI and deployment each require
verification of their own resulting state.

## Verification

`tests/conformance/rust/command_private_profile_cases.rs` exercises real HTTP
describe/apply/replay/revocation against an isolated copy of owner metadata and
forms. It also covers token rotation, permissions, cross-origin and rebinding
protection, duplicate headers, framing/budgets and uncertain delivery. The copied record supplies a controlled mechanics fixture under an isolated
test delegation. Browser client tests live beside the client module and
cover request/response proofs, impostor responses, nonce binding, deadlines and
explicit close. UI close/unload protection must cover in-flight and uncertain
commands, not just successful replies.

## Native Growth source operation contracts

<a id="op-doc-growth-structure"></a>
### Growth Structure

This route applies ToS growth doctrine through explicit choices about nodes,
branches, depth and relations, grounded in the source material being developed.

<a id="op-doc-growth-structure-core-rule"></a>
#### Core rule

Growth counts when structure becomes clearer.

Healthy growth strengthens source anchoring and makes the tree easier to
traverse. Assess added files, fragments and notes by those effects.

<a id="op-doc-growth-structure-three-main-growth-moves"></a>
#### Three main growth moves

<a id="op-doc-growth-structure-deepen-an-existing-node"></a>
##### Deepen an existing node

Deepen a node when:

- the same source route remains central
- the existing node can carry the added extraction, context, or interpretation cleanly
- the new material clarifies the current authored surface while keeping it readable

<a id="op-doc-growth-structure-create-a-new-node"></a>
##### Create a new node

Create a new node when:

- a distinct source, concept, or context needs its own authored home
- keeping the material in the current node would blur lineage or interpretation layers
- the new material deserves its own stable source anchor

<a id="op-doc-growth-structure-form-a-branch"></a>
##### Form a branch

Form a branch when:

- multiple nodes now justify a durable lineage or thematic path
- readers need a stable traversal route rather than scattered cross-links
- the structure is becoming important enough that naming the branch improves orientation

<a id="op-doc-growth-structure-structural-signs-of-bad-growth"></a>
#### Structural signs of bad growth

Pause when growth starts to look like:

- node explosion with weak lineage
- branch explosion with no clear rooted path
- repeated summaries that differ only cosmetically
- quantity metrics replacing structural judgment

<a id="op-doc-growth-structure-working-question"></a>
#### Working question

Before adding a new artifact, ask:

Should this deepen a node, become a node, form a branch, or stay out until the structure is clearer?

<a id="op-doc-human-curated-expansion"></a>
### Source-Visible Human and Agent Growth

This existing route now follows
[KNOWLEDGE_ASSESSMENT](../../ToS/doctrine/KNOWLEDGE_ASSESSMENT.md).
Current assessment uses the authority and competence scopes of that contract
through the existing growth mechanism. Historical human review records retain
their original reviewer, decision and source evidence.

<a id="op-doc-human-curated-expansion-core-rule"></a>
#### Core rule

Growth can draw on competent human and agent assessment. An agent with a
trusted grant may read sources, evaluate content, admit scoped use, limit,
reject, dispute or request reassessment. A grant may cover many records. Each
assessment retains its source and versioned review, with escalation governed
by actual authority and competence needs.

<a id="op-doc-human-curated-expansion-what-ai-may-propose"></a>
#### What AI may propose

AI may assist with:

- extraction from source-linked material
- clustering of related nodes or fragments
- cross-link suggestions
- contrast proposals
- lineage hypotheses
- candidate branch shapes

A distinct assessment accepts, revises, limits or rejects generated proposals
under the applicable policy. Review independence depends on the reviewer and
evidence route; repeated generation retains its original evidential basis.

<a id="op-doc-human-curated-expansion-concrete-human-and-stronger-owner-boundaries"></a>
#### Concrete human and stronger-owner boundaries

Operator direction, personal consent and author confirmation cannot be
fabricated. Rights, canon and publication require their own owner authority.
Substantive disputes may stay open without stopping the corpus. Escalate when
authority or suitable competence is actually absent, or when a consequential
direction choice cannot be resolved in the delegated scope; do not escalate
every linguistic, historical or philosophical judgment by default.

<a id="op-doc-human-curated-expansion-visibility-rule"></a>
#### Visibility rule

AI assistance should remain visible enough that a reviewer can tell:

- what came from source material
- what was extracted or suggested
- who assessed the exact record, under which competence and authority
- which scope of use is admitted now, and what changed or was withdrawn
- what remains provisional

Hidden pipeline authority is out of bounds. Source prose and model responses
are material to assess, never instructions granting tools or permissions.

<a id="op-doc-human-curated-expansion-failure-mode-to-avoid"></a>
#### Failure mode to avoid

Do not let a generator silently become its own admission authority or erase
the plurality of philosophical positions.

That failure can look like:

- polished summaries with weak source route
- node growth driven by suggestion volume instead of judgment
- cross-links that look intelligent but flatten genuine tension
- canon-facing claims that sever their review trail

<a id="op-doc-human-curated-expansion-review-question"></a>
#### Review question

The central review question is:

Can a reader see what was proposed, who judged it, why that judgment was within
scope, what may be used now, and where the source remains stronger than either
the proposal or the judgment?

<a id="op-doc-pre-expansion-soil"></a>
### Pre-Expansion Soil

This document records the current eighth-phase ToS doctrine for pre-expansion soil work.

This earlier bounded mechanics note prepares a later world-philosophy
corpus-soil contract named by the Zarathustra golden-kernel architecture. That
wider contract must cover source identity, witness, edition, language, time,
place, address and evidence status, with bibliographic ordering and semantic
canon each retaining their own meaning.

The wider world-thought horizon remains real.
It is simply not active yet.

<a id="op-doc-pre-expansion-soil-core-rule"></a>
#### Core rule

ToS may prepare the soil for later expansion without pretending that expansion has begun.

Allowed preparation in this phase includes:

- context-node template law
- manual corpus-entry gating
- review-route hardening for pre-expansion work
- additive schema alignment for already published node families

<a id="op-doc-pre-expansion-soil-why-preparation-still-matters"></a>
#### Why preparation still matters

Preparation is not decorative.

It helps ToS:

- keep growth structural rather than impulsive
- avoid fake plurality claims with no new source movement beneath them
- make later context work legible before later corpus growth arrives

<a id="op-doc-pre-expansion-soil-what-this-phase-does-not-do"></a>
#### What this phase does not do

This phase does not:

- add a second family
- add new branch nodes
- reopen wider world-thought expansion
- treat empty scaffolding as earned plurality

Plurality should arrive through real source-linked movement, not through symbolic expansion theater.

<a id="op-doc-native-collection-growth"></a>
### Native Collection growth

A Collection identifies a multi-work publication and carries its membership
assertions in the corpus.

<a id="op-doc-native-collection-growth-independent-boundaries"></a>
#### Independent boundaries

`tos_local_corpus_create_owner_v2` adds initial `collection` to the existing
standalone Corpus creator. Version 1 is unchanged. Creation requires one new
canonical `collections/.../collection.json`, provisional identity, no
equivalence, version 1 and an explicitly empty `membership_claim_refs`.
The empty list records that this transaction supplied no membership assertion.

`tos_local_collection_membership_owner_v1` delegates
`collection.work.attach` and separately scoped recovery. The request binds
one exact existing Collection, one unchanged existing Work and one new
`contains_work` relation Claim under a separate relation home. It appends only
that Claim reference, advances the Collection once, rebinds its source-copy
forms and retains the exact selected predecessor/history. Shared selected
metadata publication owns atomicity, pending barriers and recovery, not
membership meaning.

The grant fixes both IDs and source paths, predicate, new Claim/home/event,
principal, form IDs, evidence allowlist and expiry. Retained legacy membership
provenance streams must be selected explicitly when the parent already contains
legacy membership assertions. Native current closure is verified against its
committed compound capture; old batch bytes are never rewritten.

The selected PostgreSQL backend, exact mutation-replay rule and local-only
recovery boundary are documented in [source-command discovery](#op-doc-source-command-discovery-selected-postgresql-growth-route).

<a id="op-doc-native-collection-growth-qualified-membership-assertion"></a>
#### Qualified membership assertion

The source caller supplies the membership statement, statement
language/script, `membership_scope`, evidence and uncertainty. Competing
Claims may name the same Work while keeping distinct identities. The operation
binds metadata endpoints and declared evidence references; the new Claim
remains unreviewed. Source-visible assessment evaluates the evidence,
completeness and membership assertion.

Only the compound handler may introduce the parent membership relation.
Separately delegated Claim corrections preserve its exact compound origin and
continuous history; qualified wording can change without silently retargeting
the endpoints or deleting the membership qualification. Both current and
historical Collection versions remain available to exact metadata readers.

<a id="op-doc-native-collection-growth-verification"></a>
#### Verification

Run `tests/conformance/rust/command_collection_cases.rs`, which checks the
native `source_collection_membership.rs` and `source_native_collection_cli.rs`
implementations, then source-foundation and registry checks when their owner
inputs change. The fixtures exercise synthetic membership.

<a id="op-doc-native-expression-edition-growth"></a>
### Native Expression / Edition growth

The native handler `native-expression-edition`, implemented by
`rust/crates/tos-command/src/source_native_edition_cli.rs` and
`rust/crates/tos-command/src/source_expression_edition.rs`, owns the separately
delegated `expression.edition.create` and `expression.edition.recover`
operations through the `tos-native-owner-command source-commands` front door.

<a id="op-doc-native-expression-edition-growth-source-boundary"></a>
#### Source boundary

This first operation selects one existing Expression under its exact read-only
Work and creates one provisional Edition plus a distinct `embodied_by` Claim.
Only the Expression's version and appended `embodiment_claim_refs` change.
All other fields, lists, source-copy form history and earlier compound receipts
remain retained. Work metadata, existing editions and descendants are not
rewritten. The new Claim lives inside the new Edition home, never in the
Expression's immutable `has_expression` source carrier.

An Edition identifies a published or edited manifestation. The caller supplies
its explicit referent and evidence, distinguishing historical printing from an
ebook release, publication dates from download/update dates, and translator
responsibility from later editorial changes. Bibliographic assessment,
equivalence, rights, publication and canon follow their owner routes.

The initial Edition is version 1, provisional, with `no_equivalence_claim`,
no supersession, one explicit `embodies_expression_refs` backlink, unverified
identity variants and empty publication/provision, responsibility and exemplar
claims. Its source-owned name, optional edition statement and qualified notes
remain supplied input. The new Claim is initial, positive, observed, unreviewed
public metadata, with no assessment admission. `observed` describes the declared
record link only. Its evidence is exactly the two linked metadata paths and
its qualified statement has explicit language and script.

This operation writes one selected Expression. Existing Editions may embody
several Expressions or belong to a collection, and an Edition may await
acquisition of an Item. Attaching an existing Edition or creating an Item/File
requires the corresponding owner route and grant. Reuse an existing Edition
identity whenever the evidence identifies the same manifestation.

<a id="op-doc-native-expression-edition-growth-exact-delegation-and-abi"></a>
#### Exact delegation and ABI

The protected configuration schema is `tos_local_expression_edition_owner_v1`.
It contains `uid`, `principal_id`, `maker_type`, `source_root`, `authority_ref`,
`expires_at`, `allowed_operations`, and these exact targets:

- Read-only `work_id` and `work_source_path`.
- Existing `expression_id` and `expression_source_path` under that Work.
- New `edition_id` and `edition_source_path`, exactly
  `<expression-home>/editions/<slug>/edition.json`.
- New `claim_id`, `provenance_event_id`, and distinct bounded
  `allowed_expression_form_ids`, `allowed_edition_form_ids`, `allowed_claim_form_ids`.

Requests use `tos_local_expression_edition_command_v1`:

1. `describe` returns the selected Expression, revision, publication token,
   source fields and existing Expression/Edition/Claim owner profile handles.
2. `prepare-create` accepts `record` (the Edition), `claim`, `forms` (the
   Expression), `edition_forms`, `claim_forms`, and `reason`. It writes no source
   and returns `prepared_expression`, `prepared_edition`, `prepared_fields`,
   forms and exact dependency/publication bindings.
3. `expression.edition.create` adds `command_id`, those prepared `fields`,
   `expected_source`, `expected_revision`, `expected_configuration`,
   `expected_dependencies`, and `expected_publication` to the same proposal.
4. `expression.edition.recover` names the pending `transaction_id`, explicit
   `decision` (`resume` or `rollback`), and current `expected_configuration`.
   A recovery-only renewal never authorizes a new creation.

The selected PostgreSQL backend, exact mutation-replay rule and local-only
recovery boundary are documented in [source-command discovery](#op-doc-source-command-discovery-selected-postgresql-growth-route).

All current Expression source-copy forms must be explicitly rebound. New
Edition name and Claim statement forms are explicit source copies; each
language comes from its source field. Authored forms require their own route.
Keep confidential owner paths, grants and credentials out of public inputs;
the public receipt retains only bounded scope, principal and authority refs.

<a id="op-doc-native-expression-edition-growth-retained-evidence-and-recovery"></a>
#### Retained evidence and recovery

Preparation binds current catalog identity, the Expression's unique verified
Work origin, and its existing Edition forward/backlink closure. The exact
catalog must bind the current publication token. The adapter reads selected
catalog routes, not arbitrary descendants. New/resumed publication rechecks
current account delegation, expiry, grammar, implementation and dependencies.

The parent three-file delta and new Edition's eight metadata/form/capture/
receipt files use the [selected publication protocol](README.md#selected-metadata-publication-and-recovery).
The pending barrier closes participating reads. Recovery reconstructs all
original before/after bytes and refuses foreign or third-state changes.
Immutable predecessor archives preserve earlier source language and receipts.

`verify_compound` admits only an exact committed transaction, immutable Claim,
request/environment/provenance captures, the transition in current Expression
history and the original Edition bytes in current or verified retained history.
An orphan archive is not history. Exact completed retries remain valid after
catalog regeneration and later sibling growth, subject to current authority
and verified lineage. Unsigned runtime capture is not external execution proof.

Foundation closure joins verified native `has_expression` and `embodied_by`
Claims with unchanged legacy topology streams. The old batch counts and raw
digests retain their own historical scope; altered legacy Expression inputs
resolve only through their exact original path and committed retained bytes.
The read-only metadata version reader now resolves Edition records; this does
not enable generic Edition creation or revision grants. Earlier Work/Expression
and translator receipts remain independently verifiable after this append.

Synthetic command tests protect these boundaries without changing the corpus.
Catalog regeneration and wider downstream validation remain separate owner
steps; a successful command reports `grants_admission: false`. No Item, File,
payload acquisition, extraction, OCR, text-layer or rights transition is performed.

<a id="op-doc-native-expression-responsibility"></a>
### Native Expression responsibility attachment

The native handler `native-expression-responsibility`, implemented by
`rust/crates/tos-command/src/source_native_responsibility_cli.rs` and dispatched
through `tos-native-owner-command source-commands`, owns one separately delegated
`translated_by` attachment from an existing
Expression to an existing Agent. It uses the canonical relation registry and
`tos_source_relation_claim_v1`, with its own compound attachment grant.

<a id="op-doc-native-expression-responsibility-exact-source-change"></a>
#### Exact source change

The Expression keeps its identity, Work backlink, language, role, identity
posture, notes and all other fields. Its version advances once and
`responsibility_claim_refs` appends exactly one new Claim, preserving earlier
order. Only its record, forms and history are selected. All current source-copy
forms are explicitly rebound; old forms remain retained. Authored parent forms
that are not source copies require their own explicit rebind route.

The new qualified version-1 Claim and its statement form occupy a new standard
`ToS/source-witnesses/relations/<slug>/source-claims.jsonl` home. The existing
Expression's `has_expression` stream and immutable creation package are not
append targets. The Agent is an exact read dependency, never an output.

The Claim remains unreviewed public metadata, with no assessment or supersession
grant. `bibliographic_assertion` and `scholarly_report` use the canonical
registry's existing layers. Epistemic status and polarity remain explicit
authored values; the command does not convert them into positive certainty.
`qualifiers.statement`, `statement_language`, `statement_script` and
`attribution_scope` must all be explicit. The scope distinguishes provider
attribution from a historical printing or later electronic editing when those
limits matter. All other qualifiers and nested context remain intact.

Two Claims may name the same Expression, predicate and Agent. Competing reports
are not rejected by a person-uniqueness or endpoint-pair rule. Claim identities
remain unique and each current Claim must belong to the parent's reference
list. Other responsibility predicates need their actual owner type matrix and
explicit operation before this narrow command can grow into a family.

<a id="op-doc-native-expression-responsibility-attribution-evidence-is-not-endpoint-metadata"></a>
#### Attribution evidence is not endpoint metadata

`evidence_refs` and `counterevidence_refs` cite the attribution's declared
evidence. They are checked against an exact protected allowlist. HTTP(S) URLs
locate sources; fetching, source reading, reliability and witness independence
require their own evidence. Several pages from one provider retain that common
provenance.

The graph carries each external citation as a Claim-specific occurrence. Its
top-level path, line and canonical hash identify the local citing Claim; the
literal URL lives in `properties.evidence_ref` with `evidence_kind:
external_citation`, `resolved: false`, `remote_content_sha256: null` and
`observation_posture: address_only_not_observed`. Identity includes Claim ID
and literal URL, not Claim version, so another citing Claim is a separate
occurrence. The existing graph node schema retains its non-null local source
hash contract; no remote-content identity or first-class observed Link is made.

The Expression and Agent logical paths, exact record references and raw hashes
are separately bound by the receipt, parent before-package, Agent packet and
dependency snapshot. The endpoint metadata paths are not mandatory attribution
evidence: derived records created from the same report cannot manufacture
additional grounds for it. Explicit public metadata evidence paths are also
supported; anchor and other evidence carrier families require their owner route.

<a id="op-doc-native-expression-responsibility-protected-configuration-and-request"></a>
#### Protected configuration and request

Configuration schema: `tos_local_expression_responsibility_owner_v1`. In
addition to `uid`, `principal_id`, `maker_type`, `source_root`, `authority_ref`,
`expires_at` and `allowed_operations`, it names:

- exact `expression_id` / `expression_source_path` and existing `agent_id` /
  `agent_source_path`;
- `predicate: translated_by`, new `claim_id`, separate `claim_source_path`,
  and unique `provenance_event_id`;
- bounded `allowed_expression_form_ids`, `allowed_claim_form_ids` and
  `allowed_evidence_refs`.

Requests use `tos_local_expression_responsibility_command_v1`:

1. `describe` returns the exact Expression source/revision/publication, schema
   and type handles, form-selectable source fields, plus `agent_record` and its
   explicit logical-path/canonical-reference/raw-byte binding.
2. `prepare-attach` takes `agent`, `claim`, `forms`, `claim_forms` and `reason`.
   `agent` is the complete current source Agent packet, checked against the
   exact catalog locator and source bytes; it does not create or revise Agent
   truth. `forms` and `claim_forms` are explicit `{form_id, field_id}` selections.
   Preparation makes no source write and returns the required bindings.
3. `expression.responsibility.attach` submits the same inputs, `command_id`,
   returned `prepared_fields` as `fields`, `source` as `expected_source`,
   `revision` as `expected_revision`, `owner_configuration` as
   `expected_configuration`, and `expected_dependencies` / `expected_publication`.
4. `expression.responsibility.recover` requires its current explicit grant,
   exact `transaction_id`, `decision: resume|rollback`, and
   `expected_configuration`. A recovery-only renewal cannot start a new Claim.

The selected PostgreSQL backend, exact mutation-replay rule and local-only
recovery boundary are documented in [source-command discovery](#op-doc-source-command-discovery-selected-postgresql-growth-route).

The command discovers selected records and existing translator Claims through
exact current catalog routes. Catalog generation is a separate post-publication
step before the next growth operation. No descendant enumeration is needed.

<a id="op-doc-native-expression-responsibility-shared-publication-and-retained-reading"></a>
#### Shared publication and retained reading

The adapter uses the same common-lock, selected-metadata publication and
current-authority recovery as
[native Work/Expression growth](#op-doc-native-work-expression-growth). Shared
catalog, form, dependency and lifecycle mechanics live in the native selected
publication modules under `rust/crates/tos-command/`; each adapter owns its distinct scope, typed
delta and complete before/after reconstruction. Old Work/Expression receipt
serialization and historical replay remain unchanged.

The new home retains the exact request, runtime capture, version-2
serialization provenance and `responsibility-attachment-receipt.json`. This
unsigned capture records serialization; source reading requires its own
evidence. The committed transaction establishes the attachment. Current
authority, expiry and exact dependencies are rechecked during publication and
recovery; resume and rollback reject an external third state.

Historical verification requires the exact committed attachment plan, immutable
capture bytes and its transition in the current Expression lineage. A later
descriptive Agent correction must preserve the original raw bytes in continuous
owner history; selected corrections additionally require their committed
transaction. An arbitrary archive or matching ID cannot substitute for that
history. Ordinary separately delegated Claim correction remains available:
the existing Claim revision reader must reconstruct the continuous current
stream from the exact initial compound stream. The initial receipt never
pretends to certify corrected wording or later assessment.
The current generic Claim-writer URL branch is limited to descriptive correction
of such a verified native `translated_by` Claim. It explicitly labels the
proposed citation binding `candidate_claim`: its hash identifies the proposed
Claim declaration, not pre-existing source input or remote contents. Non-URL
writer bindings and older receipt reconstruction remain unchanged. Qualification
is checked again before publishing the Claim successor; completed attachment
replay reads that current verified successor while matching its original request.

The foundation validator joins unchanged legacy responsibility streams and
verified native translator attachments, then checks exact parent/ref closure.
Source review, attribution and translation judgment, equivalence, rights,
publication and canon follow their owner routes.

<a id="op-doc-native-expression-responsibility-verification"></a>
#### Verification

`tests/conformance/rust/command_responsibility_cases.rs`
exercises the actual adapter and transaction engine on bounded synthetic
metadata, including current grants, competing assertions, process
death/recovery, historical raw-byte resolution and independent Claim
corrections.

<a id="op-doc-native-item-adoption"></a>
### Native local Item adoption

The native handler `native-item-adoption` uses
`rust/crates/tos-command/src/source_item_adoption.rs` through the
`tos-native-owner-command source-commands` front door. It adds `item.adopt` and
`item.adoption.recover` and adopts one **already obtained
local file**, creates one provisional acquired Item and distinct
`exemplified_by` Claim, and appends the existing Edition's
`exemplar_claim_refs` with one version/form successor. Acquisition, text
processing, source assessment, rights and publication use their dedicated
routes. Artifact and scholarly Composite representations also retain their own
routes; the corpus supports each declared bibliographic topology.

<a id="op-doc-native-item-adoption-exact-delegation"></a>
#### Exact delegation

The protected owner configuration uses `tos_local_item_adoption_owner_v1`.
`--discover --handler native-item-adoption` exposes operation/request shapes
without selecting a grant. Configuration keys are exactly:

- Common metadata authority: `uid`, `principal_id`, `maker_type`, `source_root`,
  `authority_ref`, `expires_at`, `allowed_operations`, `schema_version`.
- Existing `edition_id`, `edition_source_path`; new `item_id`,
  `item_source_path` (`<edition-home>/items/<slug>/item.json`), `claim_id`,
  `provenance_event_id`; `allowed_edition_form_ids`, `allowed_item_form_ids`,
  `allowed_claim_form_ids` (distinct bounded form sets).
- Exact File: `file_id`, `payload_basename`, `original_basename`, `media_type`,
  `byte_size`, `sha256` (bare 64 hexadecimal digits).
- Separate source records: `rights_id`, `acquisition_event_id`,
  `inventory_event_id`. All three event IDs must differ.
- Separate local-copy authority: `input_path`, `payload_root`,
  `payload_authority_ref`, `payload_expires_at`, `recovery_root`.

All roots and input are canonical absolute, protected, owner-selected paths.
`source_root` is the metadata checkout. `payload_root` is the canonical
**source-witnesses directory**, not a repository root; no request or worktree
default may choose it. Destination is exactly
`payload_root / item-home-relative-to-ToS/source-witnesses / payload / payload_basename`.
The independent original input remains untouched. The existing Item manifest
v1 and legacy `payload/<basename>` grammar remain unchanged.

`recovery_root` is an existing account-owned mode-0700 directory outside the
metadata checkout, canonical source checkout/payload root and input. Its
mode-0700 `<transaction-sha256>/` companion contains a mode-0600
`item-deposit.json`. This is private continuation for the **same transaction
identity**, not another registry or authority. No absolute input/root paths,
copy-inode/ancestor pins, raw private grant or credentials enter the tracked source
transaction journal, public request, human forms or receipt. Recovery requires
the same explicit private companion; missing evidence is not reconstructed
from a same-hash destination.

Both metadata and payload authority expire. The Unix account must match the
grant. A grant is scoped transport permission, not copyright permission;
acquisition, retention, processing and public redistribution remain distinct.

<a id="op-doc-native-item-adoption-request-sequence"></a>
#### Request sequence

Requests use `tos_local_item_adoption_command_v1`.

1. `describe` reports the exact selected Edition/version and handler grammar.
2. `prepare-create` supplies `record` (new Item), `claim`, `rights`, `item_kind`,
   `forms` (all current Edition source-copy forms), `item_forms`, `claim_forms`,
   and `reason`. It reads/hash-checks the already obtained file and enumerates
   supported resources, but writes neither bytes nor metadata.
3. Keep that proposal unchanged, set `operation: item.adopt`, choose a stable
   `command_id`, and copy `prepared_fields` into `fields`, `source` into
   `expected_source`, `revision` into `expected_revision`,
   `owner_configuration` into `expected_configuration`. Copy returned
   `expected_dependencies`, `expected_publication`, `inventory`,
   `inventory_limitation`, and `fixity_verified_at` verbatim.
4. An explicit `item.adoption.recover` selects `transaction_id`, `decision`
   (`resume` or `rollback`) and the **current** `expected_configuration`.
   Exact-root/ID/input renewal may permit recovery but never new adoption.

The Item is version 1, `provisional`, with `no_equivalence_claim`, no
supersession, unverified identity variants, and exact `item_manifest_ref`.
The Claim is an initial positive, unreviewed, observed public-metadata
`bibliographic_assertion`; its evidence is exactly the Edition and Item record
paths. “Observed” denotes the declared record link, not accepted bibliography.
Standalone/generic Claim creation or correction cannot bypass the compound
handler, including when the separate payload root leaves a flat Item metadata
package. Changing this native topology Claim needs its compound owner route.

This handler serializes the supplied local-only rights record for the exact
Item/File: unreviewed status, empty permissions and layer clearances,
`not_authorized` redistribution and `local_research_only` derivatives.
Rationale and source references remain attributed to their supplier. Rights
assessment follows the rights-owner route.

<a id="op-doc-native-item-adoption-two-stages-not-cross-root-atomicity"></a>
#### Two stages, not cross-root atomicity

The byte stage hashes a stable no-symlink input descriptor and ancestor chain,
streams within the exact byte-size bound (hard ceiling 512 MiB), verifies the
original again, fsyncs and publishes with Linux `renameat2(RENAME_NOREPLACE)`.
The destination must be absent; equal hashes never authorize adopting a
foreign inode. A bound partial prefix may resume, while unbound/replaced
partials or changed ancestors fail closed. No byte or original is deleted by
rollback. A crash after file publication is recoverable through the retained
inode and fixity binding.

Only after a completed copy and available inventory does the adapter publish
the Edition delta plus Item metadata through the existing
[selected metadata protocol](README.md#selected-metadata-publication-and-recovery). Its explicit
v2 path profile permits only exact Item siblings `fixity.sha256` and
`forensic-report.md` in addition to ordinary selected JSON/JSONL. Legacy v1
transaction grammar stays unchanged. The Item manifest, rights, provenance,
resource inventory, forms, request/runtime captures and compound receipt are
metadata; payload bytes never enter selected blobs. Colocated payload-bearing
ancestors are not metadata rollback directories.

`item-deposit-receipt.json` exposes only bounded source-safe File identity,
fixity, separate actual observation and copy intervals, original/configuration digest, private-stage digest
and optional renewed-configuration digest. It explicitly does not attest a
metadata commit. The compound transaction separately binds its exact bytes.
The initial observation interval is retained once and supplies the forensic
inventory event; replay/recovery verification does not rewrite that interval.
The private state may say `prepared`, `copying`, `deposited`, or
`rolled-back-retained`; partial/deposited-only states are not acquired Items.
Rollback of an already committed adoption is rejected without changing stage,
source or publication state. Reversing acquired metadata needs another owner
operation, not recovery.

The original native inventory profile is bounded EPUB: at most 2048
members, 1 MiB central-directory bytes, 16 MiB per expanded member and 64 MiB
total expansion, and 256 KiB serialized inventory; ordinary single-disk ZIP
using only STORED or DEFLATED compression, no duplicate/unsafe members or
encryption. Other codecs are rejected before decoder creation (an LZMA member
can otherwise request a large dictionary before expanded-byte checks).
Actual member streams are bounded before invoking the existing
`build_file_inventory` enumerator. It emits resource structure, counts and one-way fingerprints. Source text
remains with the acquired File.

Inventory generator version 2 uses a concise purpose statement. Retained version-1
receipts reconstruct their original descriptive bytes through the explicitly
versioned producer; current source construction emits version 2.

The separate `plain_utf8_file_v1` profile supports `text/plain` and
`text/markdown` up to 128 KiB. It enumerates one complete inert file with exact
raw-byte fixity and extent, strict UTF-8 encoding/BOM, code-point count
(including any BOM), CRLF/lone-CR/lone-LF counts, terminal newline and observed
Unicode normalization form/version. Nothing normalizes, renders, executes,
follows links, expands Markdown includes, extracts language units or outputs
source text. Invalid encoding, NUL-bearing, empty and over-budget files are
outside this profile. Raw bytes remain with the acquired File; normalized or
segmented text requires a separate source event.

Other formats, unsupported ZIP profiles
and parse/budget failures return an explicit limitation, retain the copied
file and private continuation, and create **no acquired Item metadata**. The
next owner is the inventory profile route; no fake empty inventory is emitted.

Before mutation/recovery, current authority, exact metadata snapshot, grammar,
implementation and source dependencies are checked again. Item IDs, File
ownership and Edition backlinks are read from catalog-bound Item manifests
(bounded to 96 current Items), not inferred from a directory or a nonexistent
File catalog. Readers preserve exact Edition and older Expression/Edition
compound history; stable identity never implies bibliographic or textual truth.

The same Unix-account trust boundary remains: hashes and unsigned local
receipts detect mismatches, not malicious self-forgery by that account. The command's transaction covers the selected Item package. Execution
assessment, payload resolution, legacy migration, publication and source
review follow their respective owner routes.
Catalog/derived regeneration, wider validation, checkpoint review and landing
remain separate owner steps. A first Item adoption may target an Edition whose
predecessor has no `source-revision-history.json`; the first `item.adopt`
transition creates that history. Any history already present is checked before
the adoption proceeds. PG recovery accepts exactly the two-member record/forms
predecessor or the three-member record/forms/history predecessor; fixity and
receipt checks remain exact, and committed successors still require history.

Before source-foundation closure, create the
new Item's exact future-server plan under
`ToS/source-witnesses/server-import/plans/` and its provenance in the existing
`server-import/provenance.jsonl`, following `SERVER_IMPORT_PROTOCOL.md`.
The plan covers the committed manifest and rights digests; current local
Items remain metadata-only with payload transfer and content-bearing server
derivatives prohibited. This owner companion is not part of the adoption
transaction and must not alter its committed bytes or imply server upload.

<a id="op-doc-native-item-adoption-current-claim-projection-after-revisions"></a>
#### Current Claim projection after revisions

The retained `edition-item-receipt.json` and the original adoption transaction
remain immutable evidence for the Item's first Edition backlink. A current
read-only Claim projection must not reinterpret that historical 3-member
Edition and 15-member Item package as the current package after later record
revisions.

The V2 current projection verifies the complete retained Edition and Item
history with the record-revision owner, including each predecessor archive,
its exact prefix and selected bytes. It locates the one Edition `item.adopt`
transition bound by the compound receipt, then reconstructs the adopted
Edition output from the next transition's predecessor archive (or the current
tip when adoption is still the tip) and the adopted Item v1 from its first
ordinary-revision archive (or the current Item when it remains v1). The strict
V1 adoption contribution is derived from those original packages without
rewriting the receipt.

Current Edition and Item tips are carried separately, with exact member
digests and their full revision chains. The current Item history member is
optional only for an unrevised Item v1. This keeps the historical transaction
closure and present-day source binding independently reviewable in both full
and streamed inventory routes.

<a id="op-doc-native-object-link-growth"></a>
### Native object-Link growth

`object.link.create` publishes one native Link and one qualified association
Claim in separate new source homes under one selected-metadata transaction.
The existing subject is read-only: Work, Expression, Edition, Collection, Item
or physical Artifact. An Artifact retains `artifact_id` and its native witness
schema; it is never recast as a CorpusRecord or an ArtifactLink entity.

The independent owner grant is `tos_local_object_link_create_owner_v1`.
It specifies the exact source subject ID/path/type, new Link and Claim IDs and
paths, one predicate, provenance event ID, URI, observation ref, disjoint Form
allowlists, evidence allowlist, account, maker, principal, authority and expiry.
It grants `object.link.create` and optionally `object.link.recover`. Existing
Corpus, flat Claim and descriptive-revision grants gain no creation power.

Commands use `tos_local_object_link_command_v1`:

- `describe` exposes the exact selected scope and supported source contracts.
- `prepare-create` takes `subject`, `link`, `claim`, `forms`, `claim_forms` and
  a bounded authored `reason`. It binds the current catalog, subject bytes,
  metadata evidence, grammar and implementation; it writes nothing.
- `object.link.create` adds `command_id`, `expected_configuration`,
  `expected_dependencies` and `expected_publication` from preparation.
- `object.link.recover` selects an exact pending `transaction_id`, an explicit
  `resume` or `rollback` decision and the current `expected_configuration`.
  Renewal does not rewrite the original maker or capture. Changed dependencies,
  a mismatched scope or foreign third-state bytes prevent either action.

The selected PostgreSQL backend, exact mutation-replay rule and local-only
recovery boundary are documented in [source-command discovery](#op-doc-source-command-discovery-selected-postgresql-growth-route).

New homes are exactly `ToS/source-witnesses/links/<home>/link.json` and
`ToS/source-witnesses/relations/<home>/source-claims.jsonl`. Each has its own
source-copy Forms. The Claim home retains immutable request, runtime,
serialization provenance and compound receipt. The publication write set
consists of those new homes. Shared [selected transaction
mechanics](README.md#selected-metadata-publication-and-recovery) own byte movement and reader
barriers; source-visible assessment evaluates the association.

`tos_source_link_v1` stays unchanged. New association Claims use the additive
[`tos_object_link_claim_v2`](../../ToS/contracts/object-link-claim-v2.schema.json)
contract on the existing shared SourceClaimRecord. The four exact reified
predicates are `described_by`, `metadata_at`, `downloadable_at` and
`rights_statement_at`. Their domain is the six subject types above and their
range is Link; the older direct source-navigation `link-access` projection
and `tos_object_link_claim_v1` remain intact.

The Claim carries `forensic_observation` as its layer and a caller-authored
qualified statement with language, script, link role and explicit
`availability_is_rights_conclusion: false`. It records the supplied
observation and its evidence. Source URLs locate material; remote observation
requires a separate execution record. Rights-statement links point to the
rights basis, which the rights owner assesses. Source assessment, equivalence,
publication and canon follow their respective owner routes.

Independent native Link revision changes only its allowed descriptive fields;
URI, observation, identity, association and original provenance remain fixed.
The exact current and historical Link versions use MetadataVersionReader.
The qualified Claim uses the ordinary Claim correction/Form/history route;
its native compound origin and qualification must remain verifiable. Old Form
and source versions stay historical, not current assessments. A forged receipt,
partial capture, missing creation baseline or uncommitted correction is not
accepted as a native origin.

After successful creation, correction or rollback, rebuild source-owned
derived catalogs before preparing a new catalog-dependent operation.
Historical retry returns the exact original receipt. Indexing and graph
projection consume the resulting source metadata through their own builders.

Focused validation:
`tests/conformance/rust/command_object_link_cases.rs`, shared discovery,
Claim/metadata history tests, source-witness foundation and the explicit
semantic-registry transition baseline. All fixtures are synthetic; green
mechanics never constitutes a real source assessment or rights acceptance.

<a id="op-doc-native-work-expression-growth"></a>
### Native Work / Expression growth

The native handler `native-work-expression` is implemented by
`rust/crates/tos-command/src/source_native_work_cli.rs` and
`rust/crates/tos-command/src/source_work_expression.rs`. It is the separately
delegated owner adapter for one current native Work, one newly named child
Expression and one explicit `has_expression` Claim. Its command is dispatched
through the `tos-native-owner-command source-commands` front door. Shared
catalog/form/dependency and publication lifecycle mechanics live in the native
selected-publication modules; typed scope and retained reconstruction remain
with this adapter. [Expression responsibility attachment](#op-doc-native-expression-responsibility)
uses that same transport with its own separate delegation and attribution law.
The `has_expression` source reader profile is not a standalone creation or
revision grant: ordinary public and private Claim writers refuse that predicate.

<a id="op-doc-native-work-expression-growth-scope-and-source-law"></a>
#### Scope and source law

The parent keeps every authored field and existing list order. Its version
advances exactly once and `expression_claim_refs` appends the exact new Claim.
Only its record, human forms and revision history are selected; nested sources,
unrelated companions and payloads are neither enumerated nor changed.

The caller supplies an initial version-1 provisional Expression, explicit Work
backlink, language and expression role, unverified identity variants, and
empty responsibility, embodiment and derivation bindings. The Claim starts as
positive, observed, unreviewed public metadata. Its observation scope is the
declared record link. The caller supplies the qualified statement and exact
two metadata evidence paths. Bibliographic assessment, equivalence and
downstream use follow their owner routes.

Every current parent source-copy form is explicitly rebound with its own
successor; old forms remain retained. New Expression name and Claim statement
forms are explicit source copies. Their language follows the exact field's
declaration, not `Expression.language`. Authored non-source-copy parent forms
require another explicit owner rebind route and fail closed here.

<a id="op-doc-native-work-expression-growth-delegation-and-commands"></a>
#### Delegation and commands

The protected local owner configuration uses
`tos_local_work_expression_owner_v1`. It names the local account, principal,
maker, source root, authority, expiry and exact allowed operations, plus:

- `work_id` and canonical `work_source_path`;
- `expression_id` and an exact `expressions/<slug>/expression.json` child path;
- `claim_id` and `provenance_event_id`;
- separate bounded `allowed_work_form_ids`, `allowed_expression_form_ids` and
  `allowed_claim_form_ids`, distinct across the three subjects.

Requests use `tos_local_work_expression_command_v1`:

1. `describe` returns the selected parent, exact revision and publication token,
   existing Work/Expression/Claim `source_profiles` with owner type/schema
   handles, and the current Work `source_fields` for explicit form selection.
2. `prepare-create` validates `record`, `claim`, `forms`, `expression_forms`,
   `claim_forms` and `reason` without a source write. It returns the exact parent
   fields and source, revision, configuration, dependency and publication
   bindings needed by the next command.
3. `work.expression.create` supplies those same authored inputs, `command_id`,
   prepared `fields`, `expected_source`, `expected_revision`,
   `expected_configuration`, `expected_dependencies` and `expected_publication`.
4. `work.expression.recover` names the exact pending transaction, an explicit
   `resume` or `rollback` decision and current `expected_configuration`.
   A recovery-only renewal does not authorize new creation.

The selected PostgreSQL backend, exact mutation-replay rule and local-only
recovery boundary are documented in [source-command discovery](#op-doc-source-command-discovery-selected-postgresql-growth-route).

Configuration is not embedded in public evidence. The immutable request and
receipt do retain the bounded public authority reference and principal; private
owner paths, credentials and private source content must not be placed there.

<a id="op-doc-native-work-expression-growth-publication-recovery-and-historical-evidence"></a>
#### Publication, recovery and historical evidence

The adapter binds exact catalog routes and the selected current Work's existing
forward/backlink closure. The catalog must bind the current participating
publication token. The adapter does not scan descendants to discover sources.
All new or resumed mutations recheck current delegation, expiry, contracts,
implementation and selected dependency bytes throughout publication.

The parent three-file delta and every new child metadata/form/capture/receipt
file use the shared selected-metadata transaction protocol. That protocol
publishes a pending barrier before file replacement: participating readers fail
closed until exact completion or rollback. Recovery reconstructs the complete
retained before/after plan from its original request, not an arbitrary caller
plan. Changed third-state files are never overwritten by recovery. See
[the selected transaction contract](README.md#selected-metadata-publication-and-recovery).

The parent before-package is retained in the exact record-revision archive,
with committed publication required for admission to history. The child
retains its request, runtime description, version-2 serialization provenance
and compound receipt. The unsigned provenance captures prepared buffer
serialization; a separate check verifies the committed transaction. Execution
authentication, historical correctness and source acceptance require their own
evidence.

An exact retry of a completed request observes its committed operation without
reapplying its former catalog dependency guard: later catalog regeneration or a
sibling addition does not invalidate a retained exact receipt. Current scope,
expiry and typed continuous lineage still apply. Historical reader admission
requires committed publication, exact immutable Claim/capture bytes, the parent
transition in current verified history and the original Expression bytes in
its current record or verified retained successor archive.

Foundation closure joins the three legacy topology streams with verified
native `has_expression` carriers and separately delegated [`embodied_by`
carriers](#op-doc-native-expression-edition-growth). Each legacy event retains its
original stream digests and batch counts. Revised public metadata inputs
resolve by original logical path and exact raw digest through committed
source-owner history. Current global closure checks both carrier families,
including duplicate pairs and one-Work-per-Expression.

<a id="op-doc-native-work-expression-growth-verification-and-limits"></a>
#### Verification and limits

The synthetic tests live in
`tests/conformance/rust/command_work_cases.rs` and the pure
closure boundary in `rust/crates/tos-compiler/src/source_bibliographic.rs`. They exercise
the actual command, transaction transport, protected reader, restart and
recovery on bounded temporary metadata, never historical source mutation.

Translation assessment, printing identification, Edition/Item creation,
rights, publication, semantics and canon use their respective owner routes.
Catalog regeneration and downstream validation follow source publication
before wider use of the new records.

<a id="op-doc-source-command-discovery"></a>
### Source-command discovery

Ask the existing source-command front door what it implements before selecting
any protected delegation or source target:

```sh
tos-native-owner-command source-commands --discover
tos-native-owner-command source-commands --discover --handler native-expression-responsibility
```

The Rust API is `tos_command::source_native_cli::discover_commands(handler)`.
An optional handler ID selects one exact handler from the complete catalogue.
Unknown selectors and additional CLI arguments refuse. Discovery ignores stdin
and takes no owner configuration or invocation. Execution uses the separate
`source-commands --invocation ABSOLUTE_PATH` entry and its protected inputs.
HTTP `GET /commands/catalog` uses the same function under the listener's
existing request authentication.

<a id="op-doc-source-command-discovery-meaning-of-the-result"></a>
#### Meaning of the result

`tos_source_command_discovery_v1` contains the connected handler IDs, owner
configuration schema tags, request schema tags, owner routes, definitions,
typed source schema/profile handles, preconditions and operation shapes.
`handler_count` counts the returned handlers, not currently authorized grants.

Each `request_shape` is an exact top-level JSON Schema: required fields,
constant envelope/operation tags and `additionalProperties: false`. Empty
property schemas delegate nested validation to the named handler and its typed
contracts. Profile handles point to the canonical type/relation registries.
Discovery exposes those handles; execution loads applicable contracts and
checks the independently issued writer grant.

`mutation` names the operation's possible source effect under an independently
valid grant. `none` means the command does not publish source changes; the
ordinary delegated `describe`/preparation/version commands may still read
protected inputs and require configuration. `delegated_operation_names` names
the grant vocabulary that the handler checks, not a set of permissions held
by the current caller. Form batches check each actual form operation.

The result always says `implemented_capabilities_not_authorized_now`,
`authorization_status: not_evaluated` and `grants_admission: false`. It does not
check or reveal account authority, grants, expiry, private contexts, targets,
source text, current versions, identity availability, rights or admission.
Execution still requires its separate current owner configuration and the
handler's full scope, version, dependency, idempotency and recovery checks.
For `sign.promote`, obtain independent assessment admission first.

<a id="op-doc-source-command-discovery-one-implementation-grammar"></a>
#### One implementation grammar

The authored packaged descriptor is
`rust/crates/tos-command/src/source_command_catalog.json`. Rust CLI and HTTP
embed it directly. Each entry names the native implementation that owns
execution and nested validation; descriptor edits accompany a change to that
owner's grammar. Discovery describes the connected implementation and supplies
no delegation. The former Python descriptor builder is retired.

Currently connected families include public source/Claim forms;
historical, declared-profile and standalone native creation; Sign promotion;
public record and Claim correction; selected native correction/recovery;
private TextUnit, metadata-profile and Claim transport; native Work/Expression
growth; qualified Expression translator attachment; and separately delegated
[Expression/Edition growth](#op-doc-native-expression-edition-growth). Distinct owner-schema
versions retain their actual typed-value and recovery boundaries.

The separately selected [private native extraction and first-segmentation
route](README.md#private-native-textlayer-and-first-textunit-construction) exposes `owner-local-text-layer-create`
and adds the v2 layer-only input to `owner-local-text-unit-create`. Discovery
lists implemented grammar only, never an existing source, usable payload,
private selector/slot, current grant, rights decision or accepted TextLayer.
The same route's separate `owner-local-text-layer-derive` handler exposes
correction, Unicode normalization and recording supplied OCR/transcription.
One independent grant chooses one operation; reported upstream methods are not
provider execution receipts and new layers do not inherit prior quality.

The translation-alignment owner exposes its [native record
route](README.md#confidential-native-translation-alignment) through
`owner-local-native-translation-alignment`: exact private create/revise,
metadata-only version inspection and retained native recovery. Alignment
subject identity, descriptive record and Claim versions each retain their own
role. The operation captures supplied mappings; aligner execution and
translation assessment require their respective evidence. Packet-v1 review
semantics remain unchanged.

<a id="op-doc-source-command-discovery-selected-postgresql-growth-route"></a>
#### Selected PostgreSQL Growth route

The same native front door accepts a separately configured
`tos_local_pg_growth_invocation_v1` invocation for these maintained Growth
owners: Work/Expression, Expression/Edition, Expression responsibility,
Collection membership, ObjectLink and Sign promotion. PG Sign requires its v2
owner configuration and a separately protected Assessment owner v5/v6. Their
`describe` and `prepare-*`
operations read and validate the selected, fenced PostgreSQL/STO source
revision without publishing a change. Their mutation operation commits the
owner package and its source indexes in that PostgreSQL revision; it does not
publish a local filesystem transaction or mirror the PG change into local
authored files.

The local `*.recover` operations remain available only through the local
filesystem owner route. PostgreSQL recovery is exact replay of the original
mutation command: retry the same request with the same `command_id`. A PG
invocation does not accept the local transaction-ID/resume/rollback request.
The compatible legacy Sign v1 owner remains on its existing local filesystem
route; PG Sign uses the v2 protected Assessment history guard and typed commit
package. Backend selection does not broaden owner grants or replace assessment,
rights, publication or canon authority.

The distinct `owner-local-text-layer-record-owner-ocr` handler records one
authenticated signed OCR result under a separate protected grant.
ToS verifies the pinned signer and exact receipt natively without producer code,
interpreter selection or OCR execution;
copied signed metadata remains independently verifiable without reading text.
The distinct `owner-local-text-layer-record-owner-page-ocr` handler retains
the original PDF identity and separately pinned retained page image, authenticates
the owner's new execution/capture receipt and never claims a fresh render.
Neither handler grants image disclosure or textual quality.

Assessment-journal and semantic-registry evolution use their explicit owner
routes. Access CLI, HTTP, WebMCP and native MCP retain read-only access
contracts. Source commands execute through the separately configured
source-owner front door.

<a id="op-doc-source-command-discovery-verification-and-limits"></a>
#### Verification and limits

Discovery reads the packaged descriptor and performs no owner/configuration,
source-target, credential, clock or network reads and writes no source files.
Its output is deterministic for the loaded implementation, with no target
currentness or remote/runtime availability claim.

The native catalogue regression checks all handler selectors and the shared
authority boundary. Family conformance owns request validation, authority,
byte history, conflict, replay and recovery. Existing committed receipts keep
their historical request and serialization meaning; a preparation whose
implementation dependencies changed must be renewed.

<a id="op-doc-source-command-discovery-bonsai-candidate-handoff"></a>
#### Bonsai candidate handoff

`bonsai/lib/source-requests.mjs` forms a native `prepare-create` request from
an explicitly selected, already typed candidate and its source-copy forms. It
forms the write request only from the owner's returned configuration,
dependency and source bindings. The caller selects the exact protected owner
invocation and current source cut for each operation, records the completed
request and receipt for exact replay, and separately handles assessment. It
neither builds a cut nor invents grants, source bindings, Tree identities,
TextUnit records or review decisions. The lossless bonsai philosophy export
contains stand material, not native source records; its transformation to typed
candidates requires source-qualified identity and text bindings.

The same adapter can prepare an Occurrence from a unit-scoped stand use once
the caller supplies its exact native TextUnit binding, source references and
form ID. It refuses a binding to a different unit or a missing layer and
retains the original stand use, its review and token count. Its default is
local-only; the selected owner still checks the binding, rights and visibility.

## Native TextLayer operation contracts

### Private native TextLayer and first TextUnit construction

The initial source-owner route implements two separately delegated steps:
`text-layer.create` extracts a bounded representation from an already acquired
exact EPUB member; `text-unit.create` with its v2 owner configuration constructs
the first explicit interval partition of one real TextLayer. Both operations
start from their exact source bindings.

Existing source-text-layer v1, source-anchor v2, source-text-unit packet v1 and
provenance-event v2 contracts own the output grammar. The new
[`native-text-layer-binding` contract](../../ToS/contracts/native-text-layer-binding.schema.json)
selects the existing source record for construction and assessment.
Existing v1 unit-construction and public Occurrence gates keep their contracts.

The separate additive derivation grant below now implements correction,
Unicode normalization and recording of supplied source-bound OCR/transcription
results. Each operation requires its own supported handler and explicit derivation
grant.

#### Separately selected extraction grant

The native `tos-native-owner-command source-commands` front door dispatches the
protected mode-0600 `tos_local_text_layer_create_owner_v1` configuration to
the `owner-local-text-layer-create` handler in
`rust/crates/tos-command/src/source_native_text_cli.rs`, backed by
`source_text_layer_entry.rs`. Its exact top-level fields are:

- `schema_version`, current local `uid`, `principal_id`, `authority_ref`,
  `expires_at`, and `allowed_operations: ["text-layer.create"]`;
- `source_context_ref`: an independently selected absolute protected context
  configuration; `source_path`: a new private-prefix package ending in
  `source-text-layer.v1.json`; the containing collection exists, but the new
  layer package directory must not already exist;
- `source_scope`: `work_ref`, `expression_ref`, `edition_ref`, `item_ref`,
  `file_ref`, `file_sha256`; File identity is `tos.file.sha256.<raw SHA-256>`;
- `source_record_refs` and `source_record_sha256`: exact maps for `work`,
  `expression`, `edition`, `item`; refs select authored corpus records, and
  `manifest_sha256` separately pins the Item's manifest bytes;
- `source_access`: `read_scope: "exact_acquired_file"`, `access_allowed: true`,
  absolute `payload_root`, exact positive `byte_size`, its own `authority_ref`
  and `expires_at`;
- `derivation_access`: `derivation_allowed: true`,
  `operation: "structural_extraction"`, `rights_record_refs: [{ref, sha256}]`,
  `content_visibility: "local_only"`, its own `authority_ref` and `expires_at`;
- `member: {member_path, member_sha256}`, exact `selector: {type, scheme, value}`,
  and the complete supported versioned `policy` object;
- `identities`: opaque independently delegated `layer_id`, `anchor_id`,
  `passage_id`, `provenance_event_id`, each `tos.<kind>.sid-<32 lowercase hex>`;
- `maker: {maker_type, agent_ref, method, version}` with `maker_type: "software"`
  and `agent_ref` equal to the principal; `language`; and
  `limits: {max_output_bytes, max_seconds}`.

The context still has exactly its existing public/source-contract and private
roots. The explicitly granted payload root is a separate input adapter, never
a fallback namespace. It must be disjoint from the private output root. The
original physical file is selected as `payload_root / <Item metadata parent
relative to ToS/source-witnesses> / <manifest payload relative_path>`. No search,
download, source acquisition, directory creation for missing parents, alias
resolution or implicit choice of payload root occurs.

Metadata topology, raw metadata/manifest hashes, the local-only EPUB manifest
entry, separate current reading/derivation grants, exact Item/File rights and
an exact decision for the **new layer identity** are checked before original
payload bytes are opened. Item-only permission is not sufficient for this new
content-bearing output. Supported derivative posture is `local_research_only`
or unconditional `allowed`; denied, conflicting, inactive, conditional and
permission-required decisions fail closed. These are selected owner rights.

#### Narrow, versioned extraction profile

`rust/crates/tos-command/src/source_text_layer_xml.rs` owns the pure extractor;
`source_text_layer_proposal.rs` owns its versioned policy and layer constructor, tagged `tos_xhtml_text_extraction_policy_v1`. Passing
a different policy object is unsupported; the retained policy's raw bytes and
digest are bound by the layer. The command selects no caller-supplied code.

The input is one strict UTF-8 XHTML document in the XHTML namespace. Supported
selectors have `type: "structural"` and either:

- `scheme: "tos.xhtml.element-ordinal.v1"`, `value: "p:2"` for a one-based,
  document-order ordinal of an exact allowed XHTML local element name;
- `scheme: "tos.xhtml.element-id.v1"`, an exact unique `id` or XML `id` value.

Zero or multiple matches fail. The selected element may contain supported
transparent inline markup and `br`, which contributes one LF. Text nodes are
concatenated in document order without the selected root's tail, trimming,
space collapse, inserted block separators or Unicode normalization. NFD,
existing LF and spaces survive. Bounded comments are omitted; CDATA contributes
literal character data. Unknown markup inside the selected element fails.

This first profile deliberately rejects raw CR, **all ampersands** (including
predefined/numeric references), DTDs and processing instructions other than an
XML 1.0 declaration with optional UTF-8 encoding. Unsupported input is rejected; rendering, OCR, correction and normalization
use their dedicated routes.

Resource ceilings are explicit: original File 512 MiB; ordinary non-ZIP64,
single-disk ZIP with at most 2,048 members, 1 MiB central directory, 1,024-byte
names, 16 MiB per expanded ZIP member and 64 MiB declared aggregate expansion.
Actual directory counts are checked before native ZIP entry allocation. Selected
stored/deflated member data is streamed with bounded expansion, local/central
name checks, exact size, CRC and raw SHA-256; encrypted, duplicate, escaping or
symlink members fail. The stricter XHTML profile accepts at most 8 MiB, 65,536
elements, depth 64, 16 KiB markup tokens and 128 attributes per start tag,
including namespace declarations. Token checks precede parser allocation.
Output is at most 8 MiB, further narrowed by the grant. The command's monotonic
budget is at most 60 seconds; bounded parser work is checked before/after, not
an operating-system hard-timeout guarantee.

#### Request and immutable output

Use the common `tos_local_source_command_v1` envelope. `describe` and
`prepare-create` accept no source or extraction fields from the request.
`describe` opens no payload. Preparation computes the exact dependency token.
`text-layer.create` adds only `command_id`, `expected_configuration`,
`expected_dependencies`, `expected_source: null`, `expected_revision: null`.
The first two tokens come from preparation. Extra request fields fail.

The new mode-0700 package contains mode-0600 files:

- `source-text-layer.v1.json`, `source-anchor.v2.json`, `content.txt`,
  `extraction-policy.json`;
- retained `source-create-owner-configuration.json` and
  `source-create-inputs.json`;
- `source-create-request.json`, `source-create-environment.json`,
  `source-create-provenance.jsonl`, `source-create-receipt.json`.

The layer starts as unreviewed `machine_transcription` from
`structural_extraction`. Its anchor returns through
the exact original File to the selected member and structural selector. The
event is `native_extraction`, distinct from later `segmentation`. Whole-text SHA-256, exact UTF-8 scope, policy/configuration, maker,
input/output provenance and recorded rights remain private. Source-visible
assessment owns extraction quality judgment.

Public command responses expose only operation/status information, opaque
configuration/dependency/receipt digests and `grants_admission: false` with
`content_disclosure: "withheld"`; no path, selector, content, native identity,
span or raw private receipt is returned. Grant-free discovery reads no context,
grant, rights, payload, selector or private target.

#### First segmentation from the real layer

`tos_local_text_unit_create_owner_v2` retains the v1 unit grant's exact top-level
shape, declared source-reading authority, IDs, explicit scope, method and
bounded spans/gaps. Only `source_binding` selects the new layer-only schema:
`schema_version: "tos_native_text_layer_binding_v1"`,
`text_layer: {record_ref, record_sha256, layer_id, layer_version}` and
`source_record_refs: {work, expression, edition, item}`. No packet, unit or
segmentation identity is supplied as a predecessor.

`NativeTextBindingResolver.resolve_layer` validates the actual layer, lineage,
policy/configuration, source anchors, corpus/manifest and recorded rights.
Exact private reading is independently explicit; rights precede representation
I/O. It never opens the original payload and never claims public availability
or applied assessment. The delegated interval must lie inside the source layer's declared scope. New packet fields are derived directly
from that verified layer and binding, with ordinary explicit span/gap partition.

V2 responses withhold slots, scope, paths, private receipt and prepared file
hashes. The ordinary private packet has an additional `source-create-inputs.json`
binding consumed raw metadata, representation, context, implementation and
runtime bytes. V1 replay remains historical-request/current-validation; V2 and
layer creation require their exact pinned construction inputs still to match.
Replay checks the selected historical source bytes, current collisions, and
source, rights and context bindings. Unrelated identity growth remains
compatible.

#### Additive correction, normalization and supplied-result recording

`owner-local-text-layer-derive` is a separate discoverable native handler in
`rust/crates/tos-command/src/source_native_text_cli.rs`, backed by
`source_text_layer_derived_entry.rs`. Its protected mode-0600 grant is
`tos_local_text_layer_derive_owner_v1`. It selects exactly one operation:

- `text-layer.correct`: apply explicitly supplied ordered code-point edit
  proposals to one exact predecessor, preserving everything outside them;
- `text-layer.normalize`: actually execute one declared NFC, NFD, NFKC or NFKD
  transformation under the named declared Unicode database version;
- `text-layer.record-transcription`: record supplied manual or model
  transcription bytes, without executing the reported upstream transcription;
- `text-layer.record-ocr`: record supplied OCR bytes, without launching an OCR
  engine or producing an OCR execution receipt.

All four create a new private `source-text-layer.v1.json`, never an update to
the input. Correction/normalization has one exact predecessor, a new opaque
layer identity, version `previous.layer_version + 1`, and `supersedes_layer_ref`
equal to the predecessor identity. The raw predecessor record/content and
original acquired File remain unchanged. Supplied OCR/transcription instead
starts a new version-1 layer from an already acquired File and existing exact
source anchor.

##### Derivation grant and fixed policy

The common top-level fields retain the extraction grant's exact account,
principal/authority/expiry, source context/path, source scope, four metadata
refs/digests, manifest digest, language, maker and limits. The differences are:

- `allowed_operations` is exactly one of the operations above; `identities`
  contains only independently selected `layer_id` and `provenance_event_id`.
  Existing source anchors keep their identities and exact bytes.
- `derivation_access` keeps separate current authority, expiry, visibility and
  exact rights bindings, with `operation` equal to `correction`,
  `unicode_normalization`, `manual_transcription`, `model_transcription` or
  `ocr` as selected by the fixed policy. Both Item/File and the **new layer**
  need their own applicable existing derivation-rights basis.
- `policy` must equal `source_text_layer_proposal.derivation_policy(operation,
  unicode_form=...)`. Normalization requires an explicit supported form; other
  operations use `none`. For `record-transcription`, the optional
  `transcription_method` selects `manual_transcription` (default) or
  `model_transcription`. This helper returns the supported versioned policy values. Normalization maker must be software, with method
  `tos.unicode.normalize.v1` and version equal to the policy's actual Unicode
  database version.
- `member` and `selector` do not occur in this grant. `input` and `material`
  have the exact operation-specific shapes below.

For correction/normalization, `input` is `{kind: "text_layer", binding: ...}`,
where the binding uses existing `tos_native_text_layer_binding_v1` and agrees
with the independently pinned metadata refs. `source_access` contains
`read_scope: "exact_text_layer"`, `access_allowed: true`, exact positive
`byte_size`, `authority_ref` and `expires_at`, with no payload root. The input
must be the whole exact UTF-8 representation with scope `[0, length)` and the
same source/language as the new layer. Its own recorded rights are checked
before its content is opened. No original payload is opened by these methods.

Correction `material` is `{edits: [...]}`. Each edit has exactly `start`, `end`,
`input_exact`, `input_sha256`, `output_exact`, `reason`, and `confidence` in
`[0, 1]`. Spans are ordered, nonoverlapping, half-open Unicode code points;
the input bytes/digest must match the predecessor. Insert/delete/replace are
derived from the supplied spans and strings, never from an implicit diff.
Every emitted edit retains input/output coordinates, exact text and hashes,
the configured responsible maker and original anchor refs, with `proposed`
status. Review status records the separate source-visible assessment, while maker
records the correction supplier. Source-near correction of an already normalized layer is unsupported;
it cannot silently erase that predecessor's normalization posture.

Normalization `material` is `{}`. The executor records one explicit whole-text
Unicode operation, including when the result bytes happen to be unchanged.
The receipt records the applied normalization. The new layer is
`normalized_text`; source fidelity and diplomatic fidelity require separate
assessment. No trimming, whitespace collapse or unrelated editorial
rewrite is performed.

For supplied OCR/transcription, `input` is `{kind: "acquired_file", anchor:
{anchor_id, record_ref, record_sha256}}`. The existing v2 anchor must bind the
same Item/File/digest and manifest media type. Supported exact Files are EPUB,
PDF, PNG, JPEG, TIFF and WebP; the command verifies original File fixity but
does not execute the anchor selector, render a page or inspect glyphs. Its
`source_access` has the extraction grant's exact acquired-File fields,
including the explicit disjoint payload root and byte size.

Here `material` contains exactly `content_ref`, `content_sha256`, `byte_size`,
`access_allowed: true`, its independent `authority_ref`/`expires_at`,
`provider_execution: "not_observed"`, and `reported_maker:
{maker_type, agent_ref, method, version}`. The content ref selects an existing
private file outside the new package. Its exact strict UTF-8 bytes are copied
without newline or Unicode rewriting. Manual transcription requires a reported
human maker; model transcription a reported model; OCR a reported software,
model or mixed producer. These are **supplied declarations**, distinct from
the command principal that records them. This route does not authenticate their
identity, method execution, competence, completeness or fidelity to the anchor.
It cannot substitute a declared provider name for observed execution evidence.

##### Bounds, retained evidence and downstream route

Input and output text are each at most 128 KiB, further narrowed by the grant;
correction has at most 128 edits, with per-reason length bounded at 2,048
characters. This also bounds explicit input/output strings in the layer record.
No quadratic edit search runs. Original acquired Files retain the 512 MiB
streamed-fixity ceiling; no image/PDF decompression or provider process is run.
Existing metadata-count, lineage-depth and cumulative resolver budgets remain;
the derivation writer allows two bounded text representations for independent
immediate-predecessor delta verification. The 60-second cooperative deadline,
12-file/12-MiB package and 18-MiB retained-plan ceilings still apply.

The package contains the new layer, `content.txt`, `derivation-policy.json`,
and the existing retained configuration/inputs/request/environment/provenance/
receipt files. It references the existing source anchors. Request grammar, expected configuration/dependencies, absent initial
target, protected locks, exact idempotent retry and interruption recovery are
the same construction route described below. An expired or changed grant,
predecessor, supplied result, rights, implementation or third-state file blocks
retry; evidence is retained for explicit owner action, not erased or reissued.

Provenance records correction and normalization as executed operations.
Supplied-result capture uses `annotation`, with `provider_execution:
"not_observed"`; its layer method records the reported origin. Outputs retain
unreviewed status and predecessor uncertainty. Assessment, accepted use and
publication follow their separate owner routes.

`NativeTextBindingResolver` verifies exact predecessor record/content/version,
source scope and retained policy/configuration. Exact reads independently replay
the explicit edits; they do not replay a historical OCR provider or use the
current Unicode implementation as evidence of historical execution. Existing
first segmentation can consume the real new layer without inheriting quality.

The current `NativeLayerAssessmentSources._metadata` adapter has a separate
exact derived-layer comparison for the bounded EPUB/XHTML
source-view profile, documented in [native assessment](#exact-text-layer-comparison-and-current-quality-use).
Images/PDFs and unsupported selectors still need their own source renderer.
A prior extraction assessment is not a quality basis for its successor.
The source-visible reviewer must assess the exact new layer against its
predecessors and original source, then use the existing assessment journal for
a separate current purpose-scoped quality basis. Source-visible assessment owns acceptance and the scope of any quality claim.

#### Authenticated OCR of a retained PDF page

The distinct `tos_local_text_layer_record_owner_page_ocr_v1` protected profile
selects only `text-layer.record-owner-page-ocr`. Its input kind is
`retained_pdf_page`; original `source_scope` and whole-page anchor still name
the exact acquired PDF, never the derived PNG. It retains all independently
issued source, derivation, output and authenticated-evidence grants of the
owner OCR route, plus exact `material.input_representation` binding to the
retained image, one-based page, frozen render manifest/sample plan and unsigned
historical receipt. Native ToS verifies the pinned signature and exact source,
output and retained-page capture assertions. Signature authentication does not
independently prove OCR fidelity or producer execution. ToS never
runs a renderer or OCR provider through this recording operation. This profile
is served only by the native Rust command; Python layer construction and
metadata readers do not accept owner-OCR configurations.

These owner-OCR handlers authenticate portable signed evidence locally. The
protected grant pins the receipt, signature, public key, exact output and source
binding. Producer commit and adapter digests are provenance; no checkout is
opened or executed. Native ToS uses P-256/SHA-256 message-signature verification,
without Python, Pillow, Cosign, a host trust store or network discovery. A legacy
`owner_source_root` field is inert; new grants need none. Invalid signatures,
changed evidence and mismatched source/output/capture assertions refuse the
selected operation. Recording neither runs OCR nor admits textual quality.

The immutable 12-file package includes copied signed receipt, signature and
public key. Metadata-only source resolution authenticates these three files
without opening the original PDF, PNG or OCR text. Exact content reads bind
the signed output digest. Recording/recovery cannot reexecute OCR, sign an old
rendering event retroactively or mark raw OCR reviewed. The separate
[image/OCR assessment v6](#exact-imageocr-comparison) owns current
source-visible comparison and disclosure bounds; old supplied `record-ocr`
continues to mean supplied unverified origin, not observed execution.

#### No-replace commit, replay and retained recovery

Both new routes use existing source-owner locks and atomic no-replace directory
publication. A deterministic private `.native-construction-<digest>.pending`
control per exact target/command retains a mode-0600 bounded `plan.json` before
staged output writes. It records the complete exact byte set, request digest and
target. Package limits are 12 files/12 MiB; the base64 recovery plan is at most
18 MiB. The confidential recovery plan remains outside the final source package,
including after completion.

Retry verifies the plan, original request/receipt, every retained input/output,
current rights/grants and directory identities. It fills only absent staged
files and can resume after any fully written file. A torn file, absent/torn
plan, changed control/ancestor, foreign residue or occupied destination fails
closed and remains for explicit owner review. Nothing is overwritten or
automatically erased. Repeating the same command does not multiply stages.
Operators should reuse that exact command after recoverable interruption, not
create an unbounded series of new command IDs. Changed implementation/input
requires a new explicitly chosen route, not rewriting committed evidence.

Exact retry preserves the committed package and returns the same receipt
digest. The issuer still owns stability against noncooperating same-UID edits;
path/digest rechecks are not isolation or a cross-filesystem transaction.

#### Verification boundary

Focused synthetic tests cover bounded parser/resource refusal, exact text,
separate authority before payload I/O, layer-only bootstrap, stable input
binding, redacted results, no-replace replay, interruption/torn recovery and
the existing private Occurrence consumer/public-denial boundary. Real witness
assessment, rights decisions and source acceptance follow their owner routes;
release reporting records actual CI, landing and runtime verification
separately.

### Exact text-layer comparison and current quality use

This confidential source-owner extension uses the native assessment journal
`rust/crates/tos-command/src/source_assessment_journal.rs` through the owner
route `rust/crates/tos-command/src/source_native_private_assessment_cli.rs` and
the `tos_local_assessment_command_v1` request envelope. Source creation belongs
to [native layer construction](#private-native-textlayer-and-first-textunit-construction).

#### Protected configuration v5

`tos_local_assessment_owner_v5` retains all v4 fields and adds exactly:

| Field | Contract |
| --- | --- |
| `native_text_layers` | At most eight exact selections described below. Each selected layer has its own entry in `subjects`. |
| `quality_dependencies` | Object keyed by configured target ID, with at most eight `{layer_id, use}` entries per target. No duplicate layers, extra groundings or self-dependencies. |

Each layer selection has `binding`, `origin_id`, `source_access` and
`payload_access`. The `tos_native_text_layer_binding_v1` binding selects exact
layer metadata and its Work/Expression/Edition/Item source locators directly.
The target's assessment reference uses the canonical digest of the unchanged
raw layer; raw metadata-file fixity has its own digest.

`source_access` has exactly `read_scope`, `access_allowed: true` and an
independently issued `authority_ref`. A `metadata_only` selection requires
`payload_access: null`. An `exact_owner_local` selection requires a separate
current payload grant with `read_scope: exact_acquired_file`,
`access_allowed: true`, `authority_ref`, timezone-aware `expires_at`, absolute
`payload_root` and exact `byte_size`. The acquired payload root must be
separate from both public metadata and private derivative roots. Grants are
checked before source I/O; every selected layer's exact source and layer rights
are checked before opening any selected original or representation bytes.
The retained construction configuration is source data, not reading authority:
its historical expiry does not replace or invalidate a separately issued
current reading grant.

Each layer subject has the ordinary exact `record`, `risk`, `languages`,
`maker_id`, `requested_use` and `access_allowed` fields. Its assertion layer
is `textual_observation`, language is the actual single source language, and
maker is the actual selected layer maker. The four separate requested uses are:

- `text-layer:citation`
- `text-layer:linguistic-analysis`
- `text-layer:semantic-analysis`
- `text-layer:search-projection`

Policy v3 supplies `text-layer-quality` for low/moderate risk and
`text-layer-quality-high` for high risk. The latter requires two independent
reviewer groups. The existing engine still requires source-visible judgment,
current delegated authority, calibrated competence, independent execution
binding, exact evidence and counterevidence posture. No command or comparison
grants those qualifications. One configuration selects one use per layer;
separately scoped configurations may share its immutable journal.

#### Compare, assess, inspect

`describe` returns exact target, policy, comparison and required-dependency
references, current readiness, supported operations and owner snapshot. It
does not return source text. The independently selected exact-read configuration
authorizes reading and rechecking bytes while computing that snapshot.

`read-layer-comparison` uses the ordinary non-describe request fields:
`schema_version`, `operation`, `subject_id`, `expected_subject` and
`expected_snapshot`. It returns the selected comparison record and payload,
including the bounded original UTF-8 XHTML member, selected expected text,
actual representation, selector, policy, maker and input fixity. Its entire
return is local-only and publication remains unauthorized. The caller must not
forward it to a public projection. Metadata-only selection cannot use this
operation or append a quality assessment.

`append` uses unchanged command/event/batch v1 fields. The quality reviewer
must cite the comparison as evidence; a matching extraction or a successfully
verified hash is not itself an assessment. The structural-extraction adapter
retains its original v1 comparison contract and exact XHTML profile.
Original and selected-output disagreement remains visible
in the comparison and prevents usable quality admission. It does not prevent
a qualified rejection, dispute, deferral or withdrawal based on that available
exact comparison. The adapter separates source availability from positive-use
eligibility; `describe` exposes both comparison `ready` and
`positive_use_allowed`. Metadata-only selection still cannot append any decision.

##### Native derived-layer comparison

The same v5 selection can address a layer created under the independently
versioned `tos_local_text_layer_derive_owner_v1` profile: explicit correction,
Unicode normalization, or recording supplied manual/model transcription or
OCR text. The source view is still one exact acquired EPUB/XHTML member and
one supported structural anchor. An image/PDF or another anchor needs a
separately implemented source renderer; its File digest cannot substitute for
source visibility. This does not run an OCR or transcription provider.

`native-text-layer-derivation-comparison.schema.json` adds a comparison route
alongside the retained extraction schema. The returned
`tos_native_text_layer_derivation_comparison_v1` record contains the original
member and selected character data, complete oldest-to-newest layer records
and representations, exact policies/configuration digests, declared operations
and unverified supplied-producer metadata. Source and output can intentionally
differ. Every native delta is independently replayed, and
`source_text_equals_output` records byte comparison. `positive_use_allowed`
identifies evidence from which an authorized competent reviewer can reach a
positive or negative judgment. The assessment engine requires this exact
comparison in review evidence and applies current purpose, risk, authority and
competence policy.

Each predecessor's current rights are checked in the metadata pass before any
selection's content is opened. Retained construction grants, even expired or
revoked, remain source data; only independently selected current reading
authority authorizes this read. Grant validity and a shared cooperative time
budget are rechecked around every resolver file read, including recursive
lineage and snapshot reads, and between parsing and comparison-building steps.
An expired grant stops the next content read rather than waiting for the next
top-level layer. A valid predecessor assessment does not admit its successor;
the new layer needs its own exact comparison, assessment and scoped quality
basis. Withdrawal and subsequent reassessment follow the existing dependent
quality route below, without restoring obsolete dependent evidence.

#### Dependent claims and forms

The adapter derives the required layers from exact selected native grounding,
including the declared Claim source closure and human-form context. Protected
`quality_dependencies` must match that closure, not merely name a convenient
quality journal. Textual/forensic observations, bibliography and scholarly
reports require citation quality; linguistic and translation judgments require
linguistic-analysis quality; semantic interpretations require semantic-analysis
quality. A human form follows its subject's judgment layer. Search-only quality
does not qualify a semantic or citation route.

Before evaluating the dependent target, the adapter reads the current quality
admission under the same target/dependency journal locks. It derives a
`tos_native_text_layer_quality_basis_v1` record containing exact layer,
comparison, purpose, content scope, policy, active assessment references,
current status and inherited limits. `describe.required_admissions` exposes
its reference and permission, not confidential text. This record must occur in
dependent assessment evidence. The explicit assessed source-copy/freeform
consumer retains the whole current basis as owner-resolved context and an exact
dependency alongside the immutable authored subject/field bindings. It requires
current admission of the form itself; ordinary source-only readiness remains
`admission: null`. Changing the basis requires renewed form review but does not
rewrite unchanged wording. An already authored explicit basis ref still has to
match exactly and remains visibly stale after replacement, even if a new review
otherwise qualifies. Caller-supplied basis records cannot shadow this
runtime-owned result. See [assessed form materialization](parts/branch-growth-cycle/README.md#assessed-form-materialization).

A v5 Claim form additionally resolves current parent Claim admission under an
explicit exact same-use parent scope and the combined form/parent/layer locks.
The complete `subject_assessment` companion retains current state, limits and
historical withdrawals; its observed head/current result joins the owner
snapshot and is checked before return. Display distinguishes initial source
posture from current parent assessment. Qualified display remains possible
with an unreviewed or negatively assessed parent, with that state and its
restrictions included as mandatory context. Form admission covers the form's
selected use; Claim assessment and the layer quality basis retain their
separate evidence and use conditions.

Withdrawal, revocation, scope expiry, changed comparison or replaced quality
evidence makes the affected dependent result unusable while source and history
remain retained. A renewed positive quality event changes the exact basis and
requires a corresponding dependent assessment. Unrelated journal events leave
semantic evidence unchanged. Current quality and target history are rechecked
before returning; historical receipts retain their original commit scope.

An authorized negative review or explicit withdrawal remains available after
the quality-use gate closes. These acts require exact current source and
quality-basis evidence and ordinary supersession authority. Every required
original comparison must remain available, including for a negative append.
This preserves the evidence route for recording why positive use must stop.

#### Bounds and compatibility

Selections conservatively charge original `byte_size` per layer, up to 16 MiB
in total. The reader bounds metadata closure, original extraction and each
comparison (1 MiB); oversized comparisons fail closed and need a narrower
supported source route, never truncation. A derived comparison contains at
most 16 lineage layers within the same aggregate content/metadata budgets.
One comparison construction or later snapshot call has a 30-second cooperative
budget; nested checks cannot renew that call's deadline. This is not a
preemptive timer for one filesystem or parser operation. Locks use a shared bounded deadline
and stable order. They coordinate current reads and one target append, not an
atomic multi-target write transaction.

Configurations v1–v4 retain their meaning, source bytes,
`human_review_performed` fields and review status. Public assessed graph
builds and Sign issuance reject confidential v4/v5 configurations before
opening the private source context. Publication, diplomatic fidelity, full
translation verification and canon follow their owner routes. Synthetic tests
cover comparison and journal mechanics; real source-visible quality assessment
requires applicable language competence.

The separate [image/OCR comparison v6](#exact-imageocr-comparison) accepts
one authenticated retained PDF-page raw OCR layer or one explicitly selected
operator-created synthetic PNG result. It leaves this v5 EPUB comparison
unchanged, separates local reading from assistant disclosure, and carries
method-specific limits into current layer admission and dependent quality.

### Exact image/OCR comparison

The confidential assessment-owner v6 profile adds one method-specific image
comparison to the existing [quality journal](#exact-text-layer-comparison-and-current-quality-use).
It does not broaden the EPUB-only v5 profile. The native Rust owner in
`rust/crates/tos-command/src/source_private_assessment_layers.rs` is read-only:
it uses the native ToS **receipt verifier**, never Tesseract, a PDF renderer,
a model, or a remote service. The receipt and signer are independently pinned;
ToS verifies P-256 signatures locally. Producer runtime admission remains with
the producer and is not a ToS dependency.
The retired Python journal path is not part of the maintained route. The native
assessment owner accepts v6 through the handler above. Comparison availability
is not a quality verdict.

#### Independently selected profiles

`tos_local_assessment_owner_v6` has the same top-level fields as v5, with at
most one `native_text_layers` selection. In addition to the existing binding,
origin, source and original-payload grants, that selection requires:

| Field | Exact scope |
| --- | --- |
| `comparison_profile` | `tos_retained_page_ocr_image_comparison_v1` or `tos_operator_synthetic_png_ocr_image_comparison_v1`. |
| `image_access` | A separately issued current exact image grant, or `null` for metadata-only selection. |
| `disclosure_access` | `null` for historical retained pages. The synthetic PNG profile alone may carry an exact current operator-issued assistant-session disclosure grant. |

The image grant has `read_scope: exact_retained_page`, `access_allowed: true`,
nonempty `authority_ref`, timezone-aware future `expires_at`, absolute `path`,
`byte_size`, `sha256`, `page_number`, `source_file_ref`, `source_file_sha256`,
`width_pixels`, `height_pixels`, and `processing_boundary: local_only`.
Metadata-only selection requires both image and payload grants to be null,
and cannot disclose, read a comparison, or append any quality decision.

The retained-page profile accepts only a separately authenticated
`text-layer.record-owner-page-ocr` raw layer: original acquired PDF identity,
one-based whole-page anchor, independently pinned PNG, original frozen sample
plan, historical unsigned rendering receipt and new signed verification
capture. Original PDF, retained PNG, raw OCR and subsequent judgments remain
distinct. The original PDF is streamed for exact fixity up to 128 MiB; PNG
input is at most 10 MiB/12 million pixels, single RGB image, and raw OCR at
most 128 KiB. This route does not rerender or independently establish historical
page-render fidelity. Comparison limits preserve that uncertainty in current
layer admission and every derived quality basis.

The separate synthetic PNG profile accepts only an authenticated
`text-layer.record-owner-ocr` result over one complete acquired PNG and its
whole-image pixel anchor. Its scope is operator-created material: original
File identity and image digest refer to that PNG. Historical-source and
PDF-rendering claims use their corresponding routes.

#### Local access is not model disclosure

The retained-page profile rejects **any** non-null disclosure grant before
context or source I/O. A positive public-domain assessment, local processing
permission, successful OCR, signer identity or exact hash cannot open that
boundary. Source-visible historical review may remain explicitly unreviewed
without blocking completion of the bounded local processing route.

For the synthetic profile, an optional protected `disclosure_access` has
`allowed: true`, nonempty `authority_ref`, future timezone-aware `expires_at`,
`read_scope: exact_source_image_and_ocr`,
`basis: operator_created_synthetic_source`,
`processing_boundary: current_assistant_session`, exact `source_file_ref`,
`source_file_sha256`, `image_sha256` and native `layer_record_sha256`.
This declaration is supplied by the independently selected trusted issuer,
never by source material or an assessment request. Software validates its
scope and expiry; authorship and the operator's will require their own evidence.
No grant means local comparison only. Another source, changed bytes or an
expired disclosure grant requires a new explicit owner choice.

#### Reading and judging

`describe` returns metadata and readiness only. `read-layer-comparison`
returns `tos_native_page_ocr_comparison_v1`: exact source/anchor/input binding,
private image locator, raw OCR, authenticated execution/capture identity,
fixity, disclosure posture and limits. It never embeds or transfers image bytes.
The local caller must honor the declared disclosure boundary before sending
the image or OCR to an assistant or other server. `deterministic_text_match`
is null and `source_visible_judgment` is `not_performed`: OCR-to-pixel quality
cannot be inferred from matching hashes or the existence of this packet.

The engine requires exact source-visible evidence, current assessment
authority, calibrated competence and execution binding. A read-only
configuration may omit authority, competence and execution profiles and
supplies comparison data only. Any actual review applies to the exact
image/raw layer and selected use; broader accuracy, diplomatic fidelity,
translation quality, historical-source clearance, publication and canon
require their own scoped evidence and authority.

Original rights, metadata, original File identity, retained image identity,
grants and disclosure scope are rechecked through the snapshot and at the
journal return/commit edge. No retry executes OCR or rendering. Public graph
and Sign consumers continue to reject confidential v4/v5/v6 configurations
before private context I/O.

## History

Former `ToS/doctrine/` growth and curation process notes remain recoverable in
immutable Git history. Current work starts from `parts/branch-growth-cycle/`;
no package-local legacy archive is maintained.

The former method-routing shell remains at commit
`0486a9a457e0291eb7716c330b7521f96912aa89`, path `mechanics/method-growth/`.
Node, relation and template contracts retain their existing source owners.

[Exact prior provenance](https://github.com/8Dionysus/Tree-of-Sophia/blob/b54cbf8e30ff46aeddeec5c47a5e7aa1f370977d/mechanics/growth-cycle/PROVENANCE.md) preserves the pre-compaction route.

## Growth conditions

Add branch-growth payload or split deepen-node, create-node, and form-branch operations when recurring reviewed receipts need a clearer route. Judge growth by traceable branch formation.

Node, relation and template changes return to the [node contract](../../ToS/doctrine/NODE_CONTRACT.md), [relation-pack contract](../../ToS/doctrine/RELATION_PACK_CONTRACT.md), and [contract home](../../ToS/contracts/AGENTS.md). Update the owning validator with the contract.
