# Branch Growth Cycle

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

### Source-bound configuration v2

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
This is not a replacement for whole-corpus reference, provenance, rights or
source assessment; native non-Corpus artifact inputs still need their adapter.

Claims and historical records must explicitly allow `public` or `public_metadata_only` visibility;
other or missing visibility requires a separately authorized adapter. A
source-bound claim's maker and assertion layer must agree with the configured
scope; a form binds its creator and the `human_projection` layer. Risk, use,
access and calibrated languages still belong to the trusted issuer. Inline
copies cannot shadow source-bound IDs. An origin ID is issuer-owned provenance,
not manufactured from a file path or a count of copies.

The owner snapshot binds the configuration, each exact source-file byte digest,
every selected full record, and consumed registry/schema byte digests. Source
and profile files together share the 8 MiB unique-input budget. Profile inputs
are ownership-checked and rehashed after resolution; observed drift is refused.
In-read source modification is refused. This does not
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
Neither revision configuration
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
General multi-subject changes, native non-profile record correction and
automatic retirement of abandoned staging remain separate Growth work.

### Declared source Claim creation

The same `source_commands.py --owner-config /absolute/owner.json` entrypoint
dispatches separately delegated `claims.create` to `scripts/source_claim_commands.py`.
It creates one atomic package of up to 32 declared identity-relation Claims
over existing subjects/objects, including different subjects in the same batch.
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
endpoints, assertion layer, original maker/provenance and initial review flag.
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
