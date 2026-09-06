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
claim-packet v1 and human-form v1. A JSONL record is selected by stable ID,
not line number; a human-form set selects only its current `forms`, never
`prior_forms`. Duplicate current IDs or selected bindings are errors. Source
payload fields, including unknown extensions and historical review fields,
retain the same JSON values without semantic promotion. Unknown
identity families cannot be selected through this adapter and remain in the
original source with an explicit unsupported-family error. The adapter is not
a replacement for the corresponding source validator or full corpus mapping.

Claims must explicitly allow `public` or `public_metadata_only` visibility;
other or missing visibility requires a separately authorized adapter. A
source-bound claim's maker and assertion layer must agree with the configured
scope; a form binds its creator and the `human_projection` layer. Risk, use,
access and calibrated languages still belong to the trusted issuer. Inline
copies cannot shadow source-bound IDs. An origin ID is issuer-owned provenance,
not manufactured from a file path or a count of copies.

The owner snapshot binds the configuration, each exact source-file byte digest
and every selected full record. In-read modification is refused. This does not
make independently changing source files transactional: the issuer must keep
the agreed multi-file snapshot stable during the operation. A source change
invalidates an old command snapshot; a subject change also requires updating
its owner scope. Canonical record refs remain distinct from file-byte fixity.
`describe` exposes selected record refs, source paths, file digests and declared
origins, not a second corpus body or permission to use the source text.

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
adapter creates and revises forms adjacent to one bibliographic source record;
it does not expose writes through `access`, create a second corpus database or
mutate the subject. The normative identity/admission boundary remains in
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
The subject must already be owned, public bibliographic metadata. The command
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

A sibling `.<set-name>.writer.lock` coordinates command writers with a
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
