# Branch Growth Cycle

## Find a source-owner command

Start with `python mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py --discover`.
It returns handler-owned JSON operation and request shapes without a private
grant or source target. Use `--discover --handler HANDLER_ID` for one exact
family. [Discovery contract](docs/SOURCE_COMMAND_DISCOVERY.md) explains the
compact API, typed owner handles and limits: implemented is not authorized-now,
and access adapters remain read-only.

[Native Expression/Edition growth](docs/NATIVE_EXPRESSION_EDITION_GROWTH.md)
adds one separately delegated provisional Edition and exact `embodied_by`
Claim without creating an Item/File or changing global bibliographic cardinality.

## Operating Card

| Field | Route |
| --- | --- |
| role | distinguish deepen-node, create-node, and form-branch moves |
| input | source pressure, review state, branch need |
| output | growth route or return-to-review |
| owner | `mechanics/growth-cycle/parts/branch-growth-cycle/` |
| next route | `ToS/philosophy/` or `ToS/canon/` after review |
| tools | `mechanics/growth-cycle/parts/branch-growth-cycle/docs/GROWTH_STRUCTURE.md`, `ToS/philosophy/philosophy.manifest.json`, `mechanics/growth-cycle/parts/branch-growth-cycle/scripts/knowledge_assessment.py` |
| check | `python scripts/validate_philosophy_topology.py` |

## Assessment policy application

`scripts/knowledge_assessment.py` applies the source-owned
`ToS/doctrine/KNOWLEDGE_ASSESSMENT.md` law to authenticated owner inputs without
network, model calls or source writes. It distinguishes substantive assessment
from current admission. The caller supplies trusted policy, grants, competence,
current exact records and complete bounded subject history; submitted prose
cannot provide its own authority. This pure engine is not a command
adapter or proof of agent competence. Local invariant checks belong to
`mechanics/growth-cycle/tests/test_knowledge_assessment.py` and the existing
`mechanics_local` discovery lane.

## Source-owner journal

### Sign issuance through the shared source command

The existing `source_commands.py --owner-config /absolute/owner.json` entry
accepts the separately delegated `tos_local_sign_promote_owner_v1` configuration.
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
This restriction does not grant permission to disclose protected material.

`describe` returns the shared operation/field catalog plus `promotion.eligible`,
current admission and, when eligible, `promotion.basis`. Copy this exact basis
into the proposed `tos_sign_description_record_v1` record's `promotion_basis`.
Use `prepare` for source-copy field selection and `prepare-create` for the
same record/forms transaction preview. Apply `sign.promote` with
`command_id`, `record`, `forms`, exact `expected_configuration`,
`expected_dependencies`, and null `expected_source`/`expected_revision`.
The caller cannot insert an assessment, scope, clock, grant or owner path into
the command request. Name/notes forms retain the complete issuance basis and
limits as required context; their wording is not automatically assessed prose.

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
issuance, identity merges/splits or canon transitions. The exact historical
candidate is inspectable through the read-only version route below, not an
implied accepted fact.

### Read-only exact Claim versions

`claim_version_reader.resolve_claim_version(root, exact_ref)` accepts an
absolute public repository root and exact `{id, version, digest}` Claim ref.
It requires no command delegation, assessment journal or model invocation.
For a build with several references, reuse one `ClaimVersionReader(root)` and
call `verify_current()` immediately before publishing the derived result.
This invocation-local snapshot batches the catalog and shared package reads;
it is not a persistent cache or concurrent-reader object. Detected drift raises
at final verification rather than refreshing a partially assembled snapshot.

The reader resolves only public declared `source-claims.jsonl` metadata. It
checks current catalog identity, version, canonical digest, visibility and line,
then verifies the complete retained shared correction history with the existing
archive/blob and predecessor/successor verifiers. Available replies carry exact
original record fields and catalog, stream-byte, archive, history and transition
provenance. Unknown source fields survive without a claim to understand them.
The [derived version contract](../../../../ToS/contracts/record-version-view.schema.json)
separates this evidence from Claim identity and current assessment/admission.

`missing`, `stale`, `corrupt`, `access-restricted` and `over-budget` replies
retain the requested exact reference but no record/provenance. There is no
latest-version fallback. Private/native payload routes, symlinks, non-flat
packages and mixed public/private Claim streams fail closed. The bounded reader
permits 8 MiB/8,192 catalog rows, 1 MiB Claim streams, 64-file/8 MiB source
packages, 128 retained corrections and 64 MiB cumulative preflight reads.
Limits may refuse a valid larger source; refusal is not truncation or corruption.
Only public current ownership/visibility can expose retained predecessors;
old public bytes do not bypass a current restriction. This read operation
performs no writes, assessment, admission, publication or source-schema migration.
Historical HumanForms, native annotation versions and other record families
are not supplied by this bounded Claim reader.

### Read-only exact metadata versions

`metadata_version_reader.MetadataVersionReader(root)` offers `resolve(exact_ref)`,
`exact_refs(record_id)`, `supports(record_type, source_ref=...)` and
`verify_current()`. It uses the same exact-ref/status envelope, with no command
configuration or current-use authority. Supported routes are native Corpus
Agent, Place, Organization, Work, Expression and Edition, plus the registry's declared metadata
profiles and schema routes. Pass the locator to `supports` when a catalog also
contains a different native representation, such as scholarly Composites.

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
Current schema validation is not retroactive semantic admission of old records.

Work is bounded to 8 MiB/8,192 rows per catalog, 128 selected contracts, 128
corrections, existing package-manifest bounds and 64 MiB cumulative read work per
instance, including profile shape-reader rechecks. Reuse within one build and
verify before export; exceeding a bound refuses the record/history without
partial output. Native Item/File/Link and native
Artifact/Composite representations are not supported by this reader yet.

`resolve_source_bytes(original_source_path, raw_sha256)` joins a provenance
input to the exact current or retained metadata bytes at its original logical
source path. `raw_sha256` is the bare 64-hex digest, not the canonical record
digest. The catalog must uniquely locate that logical source, and the current
record or committed retained chain must bind the requested raw bytes. The reply
contains the exact record/ref and byte provenance, not arbitrary file contents.
It never searches Git, orphan archives or a private store, and never substitutes
the latest version. This lets an immutable provenance input survive a later
descriptive correction without rewriting its old digest.

Both exact readers also hold a
[selected-metadata publication snapshot](docs/SELECTED_METADATA_TRANSACTIONS.md)
from construction through success **or unavailable** results. Pending or changed
source publication returns an explicit stale result; a catalog bound to another
publication cannot hide a newly created subject behind an old membership list.
The token does not replace the readers' independent exact-file/schema checks
and does not certify arbitrary manual or legacy writes.

Access quotes only each available record's own notes (Claim: its own statement),
preserving its explicit language even when a requested translation is missing.
Missing language is null, never inferred from prose. Compact metadata versions
retain the entire exact record context with the quotation. No historical freeform
HumanForm, current assessment, grant, meaning change or publication is inferred.

`scripts/assessment_journal.py` implements immutable source-owned assessment
batches and an atomic per-subject head pointer under an explicitly configured
owner directory. `ToS/contracts/knowledge-assessment-batch.schema.json` owns
their shape. The parent directory must already exist. A hash partitions storage;
it does not replace the subject's ToS ID. No corpus assertions are copied into
a second database. Original assessment rationale and refs remain in the owned
batches; derived current admission can be rebuilt.

`append(engine, context, reviews, command_id=..., expected_revision=..., now=...)`
requires authenticated bindings and an agreed source snapshot from the command
owner. It records a valid assessment even when the judgment rejects, disputes
or defers use. It rejects the entire new batch on qualification failure or a
stale expected head. Replaying the exact command returns its old receipt and
fresh current admission separately. `inspect` materializes one subject's
complete committed history; it is not a corpus-wide scan or a public endpoint.

Unix locking serializes writers with a bounded wait (five seconds by default);
`JournalBusy` means retry, not discard or restart the live writer. Immutable blobs are fsynced before atomic head
publication; interrupted unreferenced blobs are not active history and are not
silently deleted. Missing/corrupt committed data fails closed. Committed
supersession stays effective even after the old grant expires or is revoked.
Tests cover these mechanics with synthetic records, not OS power-loss hardware
proof, trusted runtime identity, semantic quality or a deployed growth API.

Materialization currently bounds one history at 1,024 assessment events and
each batch at 1 MiB; it refuses truncation. Larger histories need a source-owned
checkpoint/archive reader, not deletion of history. Orphan retention, actual
corpus-adapter binding, cross-object transactions and research/UI integration
remain foundation work; this journal alone does not close the Growth profile.

### Local account command contract

The journal also offers `run_local_command(owner_config, request)` and a CLI:

```bash
python mechanics/growth-cycle/parts/branch-growth-cycle/scripts/assessment_journal.py \
  --owner-config /absolute/operator-selected/owner.json < request.json
```

This is an explicit local owner operation, never part of read-only `access`.
The operator/command issuer selects the configuration independently of incoming
requests. It must contain already authorized policy, grants, calibrated
competence, source scope and an execution profile whose provenance the issuer
has checked. The command does not create these records or approve its own model.
Unix UID authenticates an account, **not** a model invocation or competence.
Processes sharing that UID share this trust boundary; use a separately owned
runtime adapter for mutually untrusted agents or remote callers.

The configuration is one bounded (8 MiB) agreed source snapshot, not a second
authoritative corpus. It has exactly these fields:

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
operation. This is not a cross-file transaction with a concurrently edited
corpus. A subsequent call loads current configuration and rechecks admission.

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
untrusted same-account processes; filesystem ownership is not a sandbox.
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
usual journal result. `supported_operations` describes command grammar, not
authorization (`grants_authority: false`). A caller can use those exact refs
to construct `inspect` or `append`; it need not calculate the owner's snapshot
or guess file/JSON selectors. Read operations need no configured execution
profile: null is valid until an actual qualified writer binding exists.
`append` still requires the independent current execution profile and grants.

### Assessed form materialization

For an explicitly source-selected `tos_human_form_v1` with `content.kind=freeform`,
`describe` also discovers `materialize-form`. Its request has the same exact
fields as `inspect`, with `operation: materialize-form`; no proposed wording,
assessment, clock, permission or configuration path is accepted in the request.
This operation is available only with source-bound configuration v2. It reads
the current source snapshot and the complete committed journal history once,
then calls the existing human-form materializer and assessment policy engine.
It performs no model call, journal append, source write or graph publication.

The selected form must be in the validated adjacent form set of its explicitly
selected subject, with exact source/set/form refs. Every content binding must
also resolve from explicitly selected source records; inline copies cannot
supply missing source dependencies. The complete subject is mandatory context
for this freeform lane. The set's exact retained `prior_forms`, read within the
same bounded source snapshot, support successor validation without treating
them as current assessment targets. Existing metadata source-copy and trusted
template contracts are not expanded by this operation.

A source-form subject scope may additionally declare `form_language_context`:
an exact `{record, pointer}` binding or null. This is selected independently in
the protected owner configuration, not copied from the submitted form. A
non-null binding must refer to a selected source record other than the form;
the existing language-context schema, derivation source and binding checks
then apply. The owner still needs evidence for that linguistic declaration;
the command does not infer originality, translation or language competence.
Other subject scopes cannot carry this field.

The normal result adds `materialization` beside `revision`, `batch_count` and
`current_admission`. The latter is the same policy result carried by the form,
or null when structural/access/context validation prevented policy application.
Only a mechanically valid form with current scoped admission emits wording.
Missing assessment, revoked authority, withdrawal or a successor without a
current review emits no wording. The full-subject context and existing 64 KiB
materialization ceiling remain; this is not a guarantee that every result fits
the smaller public scene-selection budget. Source and journal snapshots, not an
old ready packet, govern each invocation.

This joins the local source/journal command to human-form output. The local
graph route below carries the result into the common reader; public snapshot
publication and runtime connection remain separate integration work. A
synthetic admitted test proves this boundary's behavior, not a real reviewer's
competence or actual corpus admission.

### Local assessed graph builds

The existing bibliographic and corpus-index builders accept an explicit
protected source-bound assessment configuration and a bounded selection of
freeform IDs. Without these flags they keep the ordinary metadata-only public
build and its source-parity check. With them, a separate new local JSON target
is mandatory; no standard export or other repository source can be overwritten.

```bash
python scripts/build_source_witness_bibliographic_graph.py \
  --assessment-owner-config /absolute/private/assessment-owner.json \
  --assessed-form-id tos.form.example.research-hover-ru \
  --output /absolute/private/claims-candidate.json
python scripts/build_tos_corpus_index.py \
  --assessment-owner-config /absolute/private/assessment-owner.json \
  --assessed-form-id tos.form.example.research-hover-ru \
  --output /absolute/private/corpus-candidate.json
```

The paths and ID above are placeholders for owner-selected inputs, not issued
grants. Reserve storage through the host owner before a large artifact write.
Each output is atomically created with mode `0600` and no replacement; adding
`--check` compares an existing candidate to current source/journal inputs and
writes nothing. These are local research candidates, not public-safe artifact
bundles, live grants, reader switches or publication decisions. The ordinary
source-parity query reader intentionally does not load them as standard exports.

For coherent in-process assembly, create one
`source_witness_human_forms.AssessedFormSnapshot(owner_config, form_ids)` and
pass that same instance as `assessed_forms` to both existing `build_payload`
functions (`source_witness_bibliographic_graph_common` and
`tos_corpus_index_common`). Pass both resulting projections to the common
`tos_access.knowledge.build_knowledge_graph`, then call `verify_current()`
before returning or persisting the result. Separate CLI invocations are not a
transaction across both files; common-reader carrier parity rejects a mixed
pair. Double collection observes source/configuration and committed per-form
journal changes; it is not a lock across all subjects or a runtime lease.

Selected forms must resolve exactly once on each projection's existing source
carrier, with matching source/form refs and owner-selected paths. Current
policy admission governs wording; pending, withdrawn or restricted forms stay
nonready without blocking unrelated source copies. The full source context and
assessment observation travel together under the existing output limits. See
the [source contract](../../../../ToS/doctrine/HUMAN_FORMS.md#local-assessed-research-snapshots)
for budgets and consumer compatibility.

### Source-bound configuration v2

The protected source/journal input may also be used by the
[local assessed graph builders](#local-assessed-graph-builds). This remains
separate from source modification and from public export or runtime admission.

`tos_local_assessment_owner_v2` retains the v1 fields and adds `source_root`
(protected absolute repository root) and `source_records` (at most 1,024
distinct `{path, record_id, origin_id}` bindings). Only explicit JSON/JSONL
metadata files under `ToS/source-witnesses/` are read; payload/local-content
paths, escapes and symlinks are refused. No directory discovery, network,
OCR, global graph build or source write runs. The total source-file read
budget is 8 MiB, each file selects at most 1,024 records, and inline plus
source-selected records share the existing 1,024-record snapshot bound.

The adapter understands the identity/version envelopes of corpus-record v1,
claim-packet v1, historical-claim v1 and human-form v1. Declared metadata
profiles (including historical records, Document and Letter) and the shared
`source-claims.jsonl` stream use the existing source-profile readers and their
exact registry/schema versions. A legacy schema name cannot bypass the
declared stream or metadata file's contract.
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
filename. This is not a replacement for whole-corpus reference, provenance,
rights or substantive source assessment.

Claims and historical records must explicitly allow `public` or `public_metadata_only` visibility;
other or missing visibility requires a separately authorized adapter. A
source-bound claim's maker and assertion layer must agree with the configured
scope; a form binds its creator and the `human_projection` layer. Risk, use,
access and calibrated languages still belong to the trusted issuer. Inline
copies cannot shadow source-bound IDs. An origin ID is issuer-owned provenance,
not manufactured from a file path or a count of copies.

For declared Claims in `source-claims.jsonl`, the adapter also derives an exact
mandatory grounding set from the profile's identity endpoints. Each endpoint
must be cited in assessment `evidence` as support, challenge or context; merely
loading it into the snapshot is insufficient. A selected form of that Claim
inherits its Claim and the same endpoint grounding. Matching native TextUnit
views and layers are added for native-bound endpoints, using their exact full
binding and original origin. Unknown value fields and unrelated selected
records are not inferred dependencies. This is the declared endpoint/native
closure, not automatic resolution of arbitrary source URLs or evidence prose.
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
temporary copies. This is real source/command integration, not a positive
historical, linguistic or calibration verdict.

Tests exercise the real CLI describe/inspect/error boundary and local command append,
replay, revocation, protected paths and adversarial requests with synthetic
review records. No real-language calibration or authenticated remote/model
execution is inferred from these checks.

### Native TextUnit return and assessment

`tos_local_assessment_owner_v3` preserves the v2 fields and adds exactly
`native_text_units`: at most 64 explicit `{binding, origin_id, read_scope}`
selections. `binding` follows
[`native-text-unit-binding.schema.json`](../../../../ToS/contracts/native-text-unit-binding.schema.json).
It identifies one frozen native packet and layer by path, native ID/version and
raw-byte digest, plus one segmentation, unit and ordered anchor sequence. It
also explicitly selects the Work/Expression/Edition/Item metadata of that
packet; no corpus search supplies missing dependencies. This adapter is for
native packet v1 source-bearing units, not synthetic laboratory packets,
semantic annotation v2, unbound spans, a new TextLayer or a source writer.

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
being used for another subject. Supporting layers are evidence, not implicitly
authorized assessment targets. Existing ordinary source-form materialization
remains separate; a native unit requires its own explicit form adapter.

The read-only `scripts/native_text_binding.py` library provides
`NativeTextBindingResolver(root).resolve(binding, verify_content=False,
allow_private_content=False)` and `snapshot()`. It checks exact native schemas,
selected unit/segmentation membership and anchor order, text-layer ancestry,
editorial policy and declared maker configuration bytes, source-anchor/file
identity, Item manifest topology and recorded rights/publication dependencies.
It never reads the original Item payload, fetches a URL, executes a selector or
method, reruns OCR, or grants semantic, legal or publication authority.
Exact mode reads full representation bytes without universal-newline or
Unicode rewriting, then checks absolute Unicode-code-point half-open anchors,
span hashes, declared scope, coverage and gaps. Successful return is not proof
of the original payload, OCR/transcription quality or linguistic correctness.

For protected configuration preparation, the same resolver's
`assessment_records(binding, origin_id=..., verify_content=...,
allow_private_content=...)` returns `records` and a text-free `summary`.
Construct `Record.from_payload(**records[0]).ref` for the pinned unit scope;
`records[1]` is its distinct native layer evidence. Do not inline either into
the v3 configuration: `native_text_units` selects them freshly on every call.
Both carry the issuer's same `origin_id`, not two independent sources.
The unit retains its native ID/version but its canonical digest describes
[`tos_native_text_unit_assessment_subject_v1`](../../../../ToS/contracts/native-text-unit-assessment-subject.schema.json):
the full unchanged packet, immutable binding, canonical layer ref, opaque
closure fingerprint and content-verification posture. It is neither a new
authored subject nor a raw packet-file digest. All packet fields survive;
the complete view must fit the existing 1 MiB Record limit. Metadata and exact
views deliberately have different assessment targets. Native historical review,
boundary and segmentation statuses are not rewritten by journal admission.

Each resolver bounds metadata and exact content separately at 8 MiB and 128
dependencies, with at most 1 MiB per metadata file and 16 predecessor levels.
A v3 command additionally shares a 16 MiB/128-distinct-file native budget
across all selections; native and ordinary records together retain the
existing 1,024-record and engine snapshot limits. Overflow is explicit refusal,
never partial evidence. These are defensive bounds, not measured UI budgets.
No directory scan is performed for native-unit resolution. The ordinary v2
metadata-profile identity check, if selected separately, retains its own
documented bounded inventory and budget.

`describe` adds `command_context.native_text_units` (unit/layer record refs,
read posture and original native statuses) and `native_contracts` (public
schema paths/digests only). It does not return native packet bodies, source
text, private locators, rights inputs or short-span hashes. Opaque dependency
fingerprints bind those inputs inside the owner snapshot. This is a local
owner response, not automatic public-safe export clearance.

For v3 append the journal calls a snapshot guard after acquiring the subject
lock, before replay/evaluation, and again after writing an immutable blob but
before publishing the head. The guard rereads exact protected configuration
bytes, native dependencies and ordinary selected source inputs. A changed
packet, layer, rights gate, verified content or schema is a refusal. A failed
late check can retain an unreferenced blob but cannot publish a new history
head. The issuer must still keep the selected files stable: these checks do
not create a filesystem transaction or isolate hostile same-UID writers.
V1/v2 commands preserve their existing snapshot and replay contracts.

Tests in `tests/test_native_text_binding.py` and
`mechanics/growth-cycle/tests/test_native_text_assessment.py` separate synthetic
closure/admission checks from real source-visible review. No private native
packet is made public by this adapter, and no human-only historical record is
relabeled as an agent act.

### Explicit owner-local source transport

`OwnerLocalSourceContext.load(context_path)` reads the protected
`tos_owner_local_source_context_v1` configuration from an independently chosen
mode-0600 file. Its fields are `store_id` (`sid-` plus 32 lowercase hex digits),
`public_root`, `private_root` and `private_prefix`, in addition to
`schema_version`. Roots are absolute, normalized, existing, non-symlink and
disjoint; the prefix is exactly
`ToS/source-witnesses/owner-local/<store_id>/`. The private root must already
exist with mode 0700. The resolver does not create directories or select a
store on behalf of the caller.

Use the existing native binding without changing its source IDs or schema:

```python
from pathlib import Path
from source_owner_context import OwnerLocalSourceContext
from native_text_binding import NativeTextBindingResolver

context = OwnerLocalSourceContext.load(Path(context_path))
resolver = NativeTextBindingResolver(context.public_root, owner_context=context)
metadata = resolver.resolve(binding)
# Only after independently confirming the caller's exact owner-local read scope:
exact = resolver.resolve(binding, verify_content=True, allow_private_content=True)
fixed_inputs = resolver.snapshot()
```

`context.path(ref)` selects exactly one physical root, preserving the complete
logical path. `context.read_bytes(path, limit, read_bytes=protected_reader)`
can retain caller byte-budget accounting while checking confidentiality before
and after the read. Private files require 0600 and private directories 0700;
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
separate read selection. Context use does not grant source writing, reviewer
competence, assessment, publication or canon. The explicit native and private
profile writers below use this transport interface; v1-v3 assessment command configurations and
public source/Claim/form commands do not implicitly acquire private-source
support. Each future writer/consumer needs its own explicit adapter.

### Confidential native TextUnit creation

`source_commands.py` dispatches the separately selected
`tos_local_text_unit_create_owner_v1` configuration to
`source_text_unit_commands.py`. Its operation is `text-unit.create`; this does
not create a semantic Description, Occurrence, Lexeme, accepted segmentation
or human-form set. The output uses the existing
[`source-text-unit-packet-v1` schema](../../../../ToS/contracts/source-text-unit-packet-v1.schema.json).
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

Use the existing source command envelope and CLI, or `run_local_command`:

```python
description = run_local_command(owner_config, {
    "schema_version": "tos_local_source_command_v1", "operation": "describe"})
# description exposes the delegated unit slots, scope, gap IDs and request fields.
proposal = {
    "schema_version": "tos_local_source_command_v1",
    "operation": "prepare-create",
    "spans": spans,  # each: unit_id, start, end, certainty, status_reason
    "excluded_gaps": gaps,  # each: anchor_ref, start, end
}
prepared = run_local_command(owner_config, proposal)
created = run_local_command(owner_config, {
    **proposal, "operation": "text-unit.create", "command_id": command_id,
    "expected_configuration": prepared["owner_configuration"],
    "expected_dependencies": prepared["expected_dependencies"],
    "expected_source": None, "expected_revision": None,
})
```

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

This is immutable native packet creation and retry, not native revision,
source-layer bootstrap, private Description/Occurrence/form creation, private
assessment admission or public projection. Those routes retain their own
explicit contracts; no public collector gains access to the private store.
Pure-constructor and command/CLI tests use synthetic evidence, not historical
or linguistic acceptance.

### Confidential owner-local source profiles

`source_commands.py` selects `source_owner_profile_commands.py` only for the
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

An Occurrence requires `exact_owner_local` source-read scope and the same full
native binding in its record. The command checks current local-derivation
rights before reading representation bytes. A non-native semantic profile
uses `metadata_only` and `source_binding: null`; adding a binding field does
not invent a native adapter for it. Record ID, native binding, scope/continuity
criterion and visibility cannot be changed by ordinary `record.revise`.
Identity transitions and publication need their actual owner routes.

Use the same `tos_local_source_command_v1` envelope on stdin:

```bash
python mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py \
  --owner-config /absolute/private/source-owner.json < /absolute/private/request.json
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
`tos_local_source_create_receipt_v1`. It records no model call or linguistic
review that did not happen. Revision uses `tos_source_revision_history_v1`
and `tos_source_package_archive_v1`, not a private history grammar. Its archive
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
32 MiB per older metadata file. This is not an indexed or constant-cost writer,
a cross-subject transaction, a hostile-same-account security boundary, or a
private Claim/assessment implementation by itself. The separate Claim writer
below and v4 assessment keep their own delegation. No public projection is created.

### Confidential source Claim growth

`tos_local_owner_claim_command_v1` selects `source_owner_claim_commands.py`
through the same `source_commands.py` CLI and `tos_local_source_command_v1`
request envelope. It creates or corrects an explicitly selected private Claim
package, not a public relation file. The source context must independently
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

Claim and form scopes contain at most 32 identities; subject, object and
evidence scopes at most 128. The selected relation must have an understood
`semantic-relation-v1` or `identity-relation-v1` source profile. Temporal and
structured values, identity endpoint replacement and assertion-layer
transitions are not aliases for this correction route. A new supported
semantic predicate uses its existing registry/profile contract, not another
private registry. Initial Claims are version 1, `local_only`, `unreviewed`,
without assessments or supersession. A correction preserves identity,
endpoints, predicate, maker, layer, visibility and all unpatched fields.

`tos_local_owner_claim_command_v2` adds required `allowed_object_values`
(at most 32 distinct exact JSON objects) and may include `object` in
`allowed_fields`. It additionally understands `structured-reference-value-v1`;
v1 does not acquire this reader through a registry change. Each declared
member must independently occur in `allowed_object_refs`, including the focal
subject when it also plays a member role. The subject and predicate remain
immutable; adding or removing members replaces the qualified value, not a
Group or Sign identity. The exact next value must be allowed independently.

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
`allowed_evidence_refs` entry. Source/schema validation and exact byte reading
do not authenticate the authored quotation or accept its interpretation.
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
metadata scan and package/history ceilings apply as above. This is not an
indexed writer or a same-UID sandbox. Interrupted unpublished staging is not
adopted by a retry; committed history is not deleted by reader rollback.

Every result declares `visibility: local_only`, `publication_authorized: false`
and `grants_admission: false`. Public Claim/form readers retain their refusal
gates. To use the result for source-visible assessment, independently select
the stored Claim and its exact grounding through v4; creation is neither
assessment nor permission to publish.

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
collision and drift during the retry still fail. A changed validator accepting
the same original bytes is not a new content assessment or admission. Existing
v1 receipts need no rewriting or fabricated retroactive dependency manifest;
new writes retain their full expected-snapshot gate.

`OwnerLocalSourceRecordProfiles(context, source_access, source_binding)` is the
separate read-only Python facade. `validate` and `load` always inspect metadata
only, even after an earlier exact read. `validate_native_binding(...,
verify_content=True)` needs distinct exact-read scope; `snapshot()` rechecks
all previously consumed metadata and exact content. Public registry/schema
digests are visible, private source dependencies opaque. The facade has no
catalog or export method and does not itself grant derivation rights or reserve
identities. Commands provide those separate checks.

### Confidential source assessment v4

`tos_local_assessment_owner_v4` uses the same assessment policy, authority,
competence, subject, command and journal grammar. It adds confidential source
selection, not a second assessment engine or implicit permission to publish.
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

The description's maker is an issuer-owned descriptive act, not automatically
the segmentation maker. A form retains its own `creator_id` and
`human_projection` layer. Private description/form scopes must cover their
actual authored languages, selected public or private form bindings and native source language;
omitting a language cannot evade competence requirements. Unknown language
remains unknown, not an invented translation or linguistic classification.

An Occurrence or form based on it requires an explicitly selected **same full
native binding** with verified exact text. Metadata-only selection permits
description and inspection but not append or assessed materialization. Any
metadata-only native evidence in an ordinary v4 selection also makes the current
assessment unusable for another subject; the unchanged supporting layer ID
cannot revive an old positive decision after exact reading is withdrawn.
Claim and Claim-form scopes instead use their declared exact grounding closure:
unrelated metadata-only units or languages do not contaminate that scope.
Private freeform `materialize-form` uses the existing whole-subject binding
and assessment route, returning wording only while that assessment qualifies.
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
before file reading. No UI, public-reader or publication route is added here.

Reproduce with the synthetic checks in
`mechanics/growth-cycle/tests/test_owner_local_assessment.py`, plus the existing
native and common assessment tests. Private Claim reader and journal integration
checks live in `tests/test_source_owner_claim_profiles.py` and
`mechanics/growth-cycle/tests/test_owner_local_claim_assessment.py`.
Successful synthetic qualification proves
the mechanics, not real-language competence, source quality or legal permission.

### Descriptions bound to native text

The registry-declared `source-text-unit-v1` profile adapter connects a public
Occurrence description to the same native resolver. The ordinary source
creation contract verifies exact public content; catalog, form and descriptive
revision reads check the complete metadata closure without reading payload.
Their dependency fingerprints include the separately held opaque native text
snapshot. The fixed binding is outside the ordinary `record.revise` fields;
name and hover forms must retain it as context. This is not a private source
writer or a grant to release an operator-held text.

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
dependency snapshot. This closes observed stale-input edges; the issuer still
owns a stable multi-file snapshot, not a filesystem transaction.

## Human-form materialization

`scripts/human_forms.py` renders the source-owned
`ToS/doctrine/HUMAN_FORMS.md` contract. `materialize_form` takes an immutable
form record, trusted `FormScope`, exact access-filtered source records and an
owner-admitted template set. `SourceBinding` identifies a whole JSON field, not
an executable expression. Required context comes from the source owner, not
the proposed wording. Freeform rendering requires the existing assessment
engine and authenticated reviews/history against the current form and source
snapshot; it cannot use a submitted positive status as permission.

This pure operation writes nothing and does not call a model. The result's
wording and context are a single read contract; `standalone_reading: false`
forbids consuming the string as an unqualified assertion. The current output
budget is 64 KiB with explicit refusal, not semantic truncation. Missing,
restricted, stale and assessment-required states have no emitted wording.
Rendering mechanics do not prove an adapter's authentication, a template's
semantic quality, an agent's real-language competence or UI consumption.
`mechanics/growth-cycle/tests/test_human_forms.py` protects these boundaries.

Optional `FormScope.language_context` names exact source-owned language and
linguistic-derivation metadata. The form must bind the same object and any
translation/transliteration/adaptation source field. Both remain current
dependencies and mandatory context; source-copy is not a declaration of
originality. These checks preserve the source owner's declaration without
performing linguistic assessment or granting a submission its own scope.

## Local source growth commands

`scripts/source_commands.py` is the explicit source-write entrypoint. Its first
adapter creates and revises forms adjacent to one bibliographic or historical source record;
that adapter does not mutate the subject. The separately delegated historical
creation adapter below publishes a new subject with its initial claims and
forms. Neither exposes writes through `access` or creates a second corpus database.
The same form-only delegation also supports the native physical-witness and
scholarly-composite paths described in
[`HUMAN_FORMS.md`](../../../../ToS/doctrine/HUMAN_FORMS.md#native-material-witnesses-and-scholarly-composites).
It verifies the native public-metadata schema and exact owner path on every
call, including replay, and exposes its digest in `source_contracts` and the
receipt. Schema drift invalidates prepared writes. Native identity and source
bytes stay unchanged; form permission does not grant native source rewriting.
The normative identity/admission boundary remains in
`ToS/doctrine/HUMAN_FORMS.md`. The same CLI serves a human or an agent:

```bash
python mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py \
  --owner-config /absolute/operator-selected/source-owner.json < request.json
```

The independently selected protected configuration has exactly these fields:

| Field | Meaning |
| --- | --- |
| `schema_version` | `tos_local_source_command_owner_v1` |
| `uid`, `principal_id` | actual Unix account and delegated source-form creator |
| `source_root`, `source_path` | absolute protected repository root and exact relative `ToS/source-witnesses/.../*.json` corpus record |
| `authority_ref` | issuer-provided reference to the source-write delegation, not a semantic assessment |
| `allowed_form_ids` | at most 32 exact existing or newly delegated `tos.form.*` identities |
| `allowed_operations` | a subset of `form.create`, `form.revise`; empty revokes writes |
| `expires_at` | timezone-aware exclusive expiry |

The issuer must allocate noncolliding form identities before delegating them;
this bounded adapter does not scan other subjects or issue globally unique IDs.
The subject must already be owned, public metadata. Historical-record v1
requires explicit `public` or `public_metadata_only` visibility on every call,
including replay. The command
does not validate its historical truth, reclassify private text as metadata or
authorize a generic new corpus schema.

All requests use `schema_version: tos_local_source_command_v1`:

1. `{"schema_version":"tos_local_source_command_v1","operation":"describe"}`
   returns current exact source, configuration digest, form-set byte revision,
   current form refs, reader states, operation/ID scope and `source_fields`.
2. `prepare` adds `form_id` and `field_id` from that catalog. Select
   `metadata.preferred-name`, `metadata.source-note` or an advertised
   `metadata.variant-name:N`. The variant ordinal is snapshot-local, not name
   identity. Preparation returns one `prepared_change`, selecting create or
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
Lock and unpublished staging files are local operation state, not corpus
records or files to include in a source commit.
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

`mechanics/growth-cycle/tests/test_source_commands.py` tests the actual CLI,
creation/revision batches, restart/replay, concurrent writers, loss before/after
publication, revocation, protected paths, inert input, preparation and partial
source rebinding. Its catalog-wide test prepares existing public bibliographic
fields **in memory** and verifies source-byte preservation. It is not evidence
of an actual whole-corpus migration or substantive source/translation quality.

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
protected metadata, not payloads or traversal paths. This resolves references,
not historical truth, source quality, competence or admission. Unknown source
extensions and claim qualifiers are retained verbatim as JSON values.

With the v1 configuration publication creates exactly four files in the new directory: the typed
record, `historical-claims.jsonl`, the adjacent human-form set, and
`source-create-receipt.json`. They keep the existing source formats and
readers; there is no new source envelope or duplicate graph store. The receipt
binds the canonical request, account/delegation, exact subject, observed
dependency digest and each output's bytes/digest. It records this source
transaction, not a claim's research provenance or an assessment event.

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
and the atomic commit. Digests are captured from buffers; independent stored-byte
fixity is explicitly not attested. Its software executor, script/runtime
digests, Unicode version, withheld process-argv digest, request/environment
bindings, derivations and measured pre-commit wall duration are captured by
the command itself. The event is unsigned and only partially specified for
replay. Upstream source reading, author/model reasoning and substantive
assessment are not captured or impersonated. No model is invoked. Publication,
rights, competence and admission are not granted by this event. A failed
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
publication and admission remain separate operations. A metadata declaration
does not implement them or declare an entire subject profile finished.

### Native standalone identities

`tos_local_corpus_create_owner_v1` uses the same `source.create` request,
six-file transaction, metadata forms and serialization capture for the
existing native `Agent`, `Place`, `Organization` and initial `Work` families.
Its configuration replaces `profile_type_id` with `record_type`
(`agent`, `place`, `organization`, `work`);
the other declared-profile configuration fields remain required. Discovery
returns `record_type` and the exact native source descriptor. The existing
`corpus-record.schema.json` is authoritative; no new Person schema or
author/addressee subclass is introduced.

The initial record must match the delegated typed ID/basename, version 1,
provisional identity and no equivalence claim. Supersession, pre-reviewed
labels/identifiers and metadata relation-link fields are refused. Native
Corpus metadata has no `visibility` extension; this route is explicitly
public-metadata-only and cannot store private payloads. Schema-permitted
unknown language qualifications are retained; unsupported fields are refused,
never silently dropped. At least one exact name form is required. The event
names `source-corpus-metadata-serialization`, not an identity assessment.

An initial Work must retain the Corpus-required `expression_claim_refs` as
an empty list. This records no supplied expression assertions, not evidence
that no expression exists. Nonempty expression refs and every other metadata
relation-link field are refused by this initial transaction. Names and notes
do not create an author, Expression, publication date, Edition or Item.
The existing `works/friedrich-nietzsche/` source home has stronger authorship
and chronology closure and is refused by this standalone Work route; that
source-home rule is not weakened to accommodate a new metadata record.
Expression, Edition, Collection, Item and physical Artifact are not created
by this route. They need their existing source contracts and related-record
closure in a multi-subject creation transaction.
Relationship assertions use separately delegated `claims.create`; a label,
role word or metadata creation receipt cannot supply them.

Initial creation checks allocated form IDs against both metadata form sets
and declared Claim form sets, including retained predecessors. The consumed
sets enter the preparation dependency digest, so new collisions or observed
changes before publication fail. This is still a metadata scan, not proof of
an indexed or incremental writer.

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
locks fail closed. These are bounded unsigned local-storage checks, not
authentication against a hostile process with the same Unix UID.

An abrupt process loss before commit may leave an invisible staging directory;
retry does not delete or publish that abandoned directory. An ordinary exception
removes only its own unpublished staging files. Committed sources and receipts
are never removed by retry or derived-reader rollback. The request/configuration
budget is 1 MiB; each source output is bounded at 2 MiB, with the existing form
and rendering limits. These are safety ceilings, not latency guarantees.

Catalog/graph rebuild and public publication remain separate operations. A
builder traversing across a concurrent creation may need a fresh build; this
adapter does not make several catalog files one snapshot. Synthetic integration
tests cover the existing catalog → graph → access reader, scope and reference
refusals, competing writers, process loss before commit, response loss after
commit, and restart/replay. They do not establish a real historical episode,
atomic revisions of an existing subject, all-profile growth or real-agent
assessment quality.

### Versioned source correction

`tos_local_source_revision_owner_v1` independently delegates `record.revise`
through the same `source_commands.py` CLI. Its fields match the form-owner
configuration, plus exact `record_id` and `allowed_fields`. This historical
configuration keeps its existing schema and ID scope. A separate
`tos_local_profile_revision_owner_v1` adds `profile_type_id` and selects the
current declared `source_record_profile`, including Document, Letter or a
new supported metadata kind added through that registry. It uses the same
revision transaction, not another per-kind writer. The exact declared basename,
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
Creation permission is not correction permission. No native source is migrated
to a declared-profile schema to gain this operation.
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
the field. `semantic_scope` and its referent criterion remain immutable in
this correction route. Changing an allowed value is an explicit authored
correction, not a judgment by the serializer. Unselected fields and all
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
and content-addressed `.blob` files. These are tracked source-history bytes,
not a cache or another current corpus. The blob suffix prevents old record or
provenance filenames from becoming duplicate current identities in existing
scanners. `inspect-version` verifies every retained byte and reconstructs the
successor from its retained request; a missing/corrupt archive fails closed.
Initial creation receipts and provenance remain unchanged historical records,
whose exact original output bytes can now be returned through this archive.

`source-revision-history.json` in the current package records the complete
canonical request, its digest, issuer/delegation, reason, exact predecessor
and successor, dependency digest, form results and archive locator. The receipt
grants no admission and does not impersonate upstream research/model execution.
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
migrating every existing reader. The accepted cost is bounded package copying
and Linux-specific exchange, not global corpus copying or an indexed writer.
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

The v2 revision unit is exactly the selected `<type>.json`, adjacent
`<type>.human-forms.json`, and `source-revision-history.json`. An initial form
set or history may be absent. Every successor has all three files. This route
does not enumerate, copy or exchange the parent directory: existing Expressions,
Editions, Items, payloads, unknown siblings and creation provenance stay outside
the selected unit. A request cannot supply extra paths or a larger file set.
The earlier v1 flat-directory package digest and archive semantics are unchanged.

Use the ordinary `describe`, `prepare-revise`, `record.revise` and
`inspect-version` requests. V2 result envelopes identify
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
[selected-metadata transport](docs/SELECTED_METADATA_TRANSACTIONS.md), under the
existing corpus writer lock. Official readers and other common-lock writers
refuse a pending transaction. This is a cooperative publication barrier, not
filesystem-wide atomic visibility for arbitrary raw readers. Catalog and graph
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
`mechanics/growth-cycle/tests/test_source_selected_revisions.py`; transport and
reader-boundary checks are separate. Existing 8 MiB selected-package, 128-history
and source/form byte limits refuse rather than truncate. Readiness here grants
no semantic assessment, current use, rights, canon, release or deployment.

### Native Work / Expression growth

The separate `tos_local_work_expression_owner_v1` delegates one exact existing
Work, one new Expression and one distinct `has_expression` Claim, together with
their selected form identities. `source_commands.py` dispatches
`work.expression.create` and explicit pending recovery through the
[native compound owner contract](docs/NATIVE_WORK_EXPRESSION_GROWTH.md).
The parent gains only a version increment and the appended Claim reference;
existing descendants and all other Work fields stay outside the change.
New source-copy forms, exact predecessor history and serialization provenance
travel together through the selected-metadata protocol. A committed metadata
link is not bibliographic or textual admission, an Edition/Item, a responsibility
assertion or publication permission. Standalone Claim grants cannot write or
revise this topology predicate merely because a read profile understands it.

### Native Expression responsibility attachment

The separate `tos_local_expression_responsibility_owner_v1` delegates one exact
existing Expression and Agent, `translated_by`, one new Claim home and selected
forms/evidence. `prepare-attach`, `expression.responsibility.attach` and explicit
recovery are described in the
[responsibility owner contract](docs/NATIVE_EXPRESSION_RESPONSIBILITY.md).
Only Expression version and the appended responsibility ref change. A qualified
unreviewed Claim keeps attribution evidence separate from endpoint metadata,
and competing Claims may name the same Agent. Its standalone source profile
does not grant flat creation; separately authorized descriptive Claim correction
must preserve the exact initial compound lineage and qualified scope.

External evidence URLs remain declared addresses. Their derived citation
occurrences return to the exact local citing Claim; no remote bytes, reading,
independence, review or truth are manufactured. The command neither creates nor
edits the Agent or the Expression's earlier immutable topology Claim stream.

### Declared source Claim creation

The same `source_commands.py --owner-config /absolute/owner.json` entrypoint
dispatches separately delegated `claims.create` to `scripts/source_claim_commands.py`.
It creates one atomic package of up to 32 declared source Claims
over existing subjects and identity or explicitly delegated typed values,
including different subjects and profiles in the same batch.
It neither creates those identities nor revises existing Claims. Read-only
access remains separate; a metadata/form delegation does not grant this route.

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
`allowed_object_refs` and resolve as a historical situation. Source prose,
schema declarations and the ability to inspect a value do not grant writes.

`tos_local_claim_create_owner_v3` has the same exact fields and bounds as v2
and can additionally delegate `structured-value-v1` profiles. Its exact-value
allowlist is data, not a predicate language; the source profile still validates
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
implementation. Changes before or during staging conflict. This still scans
source metadata; it does not prove an incremental or indexed writer.
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
serializer and both implementation modules; it remains unsigned pre-commit
buffer serialization, not upstream research or content assessment. The 1 MiB
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
remain their own operations. Initial creation alone does not complete Growth,
all subject profiles, source quality, publication or the full foundation.

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
[`HUMAN_FORMS.md`](../../../../ToS/doctrine/HUMAN_FORMS.md). The target is the
adjacent `source-claims.<sha256-of-UTF-8-claim-id>.human-forms.json`, returned by
discovery; no caller path or source row number chooses it. The source Claim
bytes, IDs and initial review status are unchanged.

Resolution reads only the selected protected stream (at most 1 MiB) and
declared registry/schema inputs (at most 128, total snapshot at most 8 MiB).
It requires exactly one occurrence of the selected ID, the current declared
schema, predicate, assertion layer and public-metadata visibility. It does
not crawl endpoint records: source and graph validators retain endpoint
existence/domain/range checks. A form write is not Claim validation or admission.
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

`scripts/claim_revisions.py` implements `claim.revise` through the same explicit
source command entrypoint. The selected Claim keeps its ID, predicate,
identity endpoints, original maker/provenance and initial review flag. Ordinary
v1/v2/v3/v4 corrections also preserve its assertion layer; only the separate exact
layer-classification route below can correct that field.
Its `claim_version` advances once. The correction issuer and reason belong to
the new history receipt; the original maker is origin attribution, not a claim
that the original actor authored every later correction. Assessment, identity
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
source-copy form history; it does not accept a motif or mint a Sign.
Current complete source/native grounding is checked again on retry, including
the historical result when a subsequent correction has removed one of that
request's members. Older grant versions do not gain this reader mode.

`tos_local_claim_layer_revision_owner_v1` is a separate grant, not an expansion
of v1/v2/v3. It has the v1 configuration fields plus mandatory
`allowed_layer_transitions`: at most 32 distinct exact `{"from": "…", "to": "…"}`
pairs. Wildcards, empty layer names, repeated pairs and `from = to` are refused;
an empty list revokes transitions. `allowed_fields` can contain only
`assertion_layer`. Both `prepare-revise` and `claim.revise` require an explicit
`layer_transition` matching one delegated pair, the actual predecessor layer
and the proposed field value. The corrected Claim must still satisfy its
existing schema and predicate profile. A grant cannot admit a forbidden layer.

This route corrects the classification of the **same already recorded
assertion**, not its proposition, attribution or act of judgment. For example,
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
remain retained. A sibling's form identity cannot be reused. This route does
not turn a newly written form into a calibrated or admitted interpretation.

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
record correction above. Every archive remains source history, not a duplicate
current Claim catalog or a cache to discard. This implementation still scans
source metadata for grounding and can inspect up to 128 bounded archives. It
does not prove an indexed writer, constant-cost history access, an incremental
graph rebuild or global cross-subject transaction support.
