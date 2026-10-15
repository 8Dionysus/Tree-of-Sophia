# `tos-command`

The crate owns source command preparation and durable PostgreSQL/STO transaction mechanics. Source, semantic, rights, publication and canon admission remain separate owner gates. Canonical writes remain closed without those gates.

Managed-source compatibility and the durable PostgreSQL schema use explicit,
versioned descriptors in `src/durable_compatibility.rs`. Routine Rust edits do
not change those identities. The source owner advances its descriptor when
carrier profiles, semantic indexes or predicates, readset/concurrency rules,
or temporal inventory-overlay semantics change; the durable owner advances
the schema ABI when PostgreSQL tables, keys, constraints, access paths, or
audit behavior change. `durable_schema.sql` remains the initializer, and cold
verification keeps its metadata, receipt, STO, and complete-cut audits. The
private profile does not attest arbitrary out-of-band DDL or trigger changes;
that remains outside its trust boundary. Domains carrying the former
source-code or bundled-DDL file fingerprints remain incompatible and are
refused rather than silently rebound. No automatic migration or installed
production PostgreSQL compatibility is claimed.

The retained CMD outbox stores a nullable PostgreSQL `enqueued_at` clock on
each new event. The additive schema initializer leaves earlier rows NULL so
their age stays explicitly unknown; it does not estimate a timestamp from the
sequence or from the client. The selected projection owner can query the oldest
event after its durable cursor through the bounded `queue-age` operation.

`DurablePgCoordinator` binds registered attempts and exact STO locators to atomic current/history/receipt/log/outbox writes. Current rights, rule/schema contracts, job leases, retention pins and audit/restore fences are checked at commit and replay. Private source creation also binds owner-derived exact, absent, identity and namespace-range observations to the registered package; predicate validation and invalidation commit together. Mutable external authored trees remain `FullOnly`.

The former CMD1 `PgCoordinator`, synthetic byte references, independent SQL schema and protocol-only test target are retired. Their useful conflict, ordered commit, current authority, retained history, rollback, replay and closed-cut integrity obligations execute on the durable path. Real managed creation exercises form-identity and source-home phantom generations, definition/completeness refusal and unchanged projections on refusal. The earlier synthetic prefix/reverse/interval names did not establish any maintained source reader or completeness law; their removal grants no corresponding affected admission.

The v1 cut-based managed source preparation uses a complete immutable manifest
and authored input inventory. Successor selection performs O(N)
manifest/hash/fixity work and retained-base-chain opening outside commit locks;
its creation reads bind the entire verified file set. The explicit PostgreSQL
record-revision and initial-Agent routes below select a bounded source readset
instead. Other source-command families still use their declared cut-based route.

Mixed managed Agent dependency commitments bind the raw configuration and the
canonical Agent creation template: its selected Agent profile/schema map,
creation contracts, and implementation inputs. The fixed registry/schema
baseline is bound from the selected context even when the mixed source contains
no current Agent. Existing Agent records, events, anchors, and forms are not
creation inputs; the complete mixed root and current head, epoch, rights, and
audit fences establish source currentness separately, while durable uniqueness
and absence predicates continue to protect global identities and homes. The
addressed inventory uses commitment version 3 and retained coverage uses
version 4; older Agent-only versions require a fresh mixed-source bootstrap.

An active addressed source selection uses the explicit addressed cold-reselection
API. `select_current_source_cut_addressed` performs the source/index/custody audit
and a bounded complete compatibility export, then restores the addressed selection.
The legacy cold/export entry refuses an active addressed protocol. These explicit
restore/export operations do not replace the ordinary addressed mutation path.
A crash may leave the audit marker active with a selected V1 checkpoint and no
published projection root. Explicit cold reselection must positively verify
that V1 descriptor's complete source-OID-bound cut and both membership streams
before rebuilding the inventory. An invalidated V2 selection or unknown
format remains refused; restore never grants an unverified old selector new
database authority.

Addressed navigation replaces complete selected Claim streams. Its range
manifest keeps causal reverse dependencies separate from the complete
published predecessor inventory of those streams. The additive
`stream_predecessors` field binds unchanged companions as well as directly
affected Claims; publication re-reads the inventory under its held transaction
and checks exact bootstrap or event-cohort roots. Previously persisted range
manifests remain readable through their authenticated roots. The reader accepts
the exact earlier 13-field shape and the 14-field shape with a validated stream
inventory; older readers reject the latter and must be upgraded with the
producer. New publication requires the complete inventory. Subsequent events recognize both Record and
Claim range-reconciliation cohorts as predecessor owners.
Historical record nodes retain their archived source references and exact
carrier bytes, while bootstrap groups them with the current record that owns
their history view. A prior bootstrap with archive-file ownership requires the
explicit source-backed bootstrap route before addressed updates.

Streamed mixed-source bootstrap resolves declared native TextUnit bindings through
its held source epoch and the shared native metadata resolver. It retains the
same schema, identity, rights and exact-input checks as the complete inventory,
with at most 128 metadata inputs and 8 MiB per binding closure. Payload bytes
are not read by this metadata projection.

The addressed audit journal covers the same 19 metadata tables as the cold
source audit, plus the semantic domain header. IDs 1..12 retain their original
keys and commitments; IDs 13..20 add publication current/history/import, Text
and annotation identities, Claim dependencies and versioned navigation
references, and Item File assignments. Publication current uses its domain
primary key; Claim navigation keys include the nullable interval start with
`NULLS NOT DISTINCT` semantics.

The complete-coverage profile
`6be233b81ce472b8b9829b50eb345dd8c84ead1418f16143731c9d564b352623`
accepts the reviewed predecessor activation markers
`471f7513e831693621cd20dd28a9afd0b74b4af544fa414eccce042d2208924c`
and `c9e1f70c15eb8d522e2bf1954a0565269fc5eacfb8cc4688172b444a2af98211`
only through explicit cold reselection. After authenticating the retained
selection and verifying all source, indexes and custody, that transaction
installs the additive Item Claim constraints and complete journal triggers,
then publishes a fresh complete metadata tree under the current profile.
The original activation marker, baseline and journal remain immutable; old
gaps stay in history. Warm operations require a current-profile descriptor
and start from its new cold cut. Backup/restore may preserve these exact
reviewed markers; unknown profiles refuse. Further schema changes require
their own compatibility review.

## Private PostgreSQL source-text projection

[TOS-D-0071](../../../docs/decisions/TOS-D-0071-transactional-source-and-independent-projections.md)
selects persistent transactions and independently updated projections as the
central update direction. `DurablePgCoordinator::apply_projection_batch` consumes
the existing ordered outbox. For each event it reads that event's exact source
revisions from STO, replaces only the changed paths' derived documents, and
commits those rows and its progress position together. Failed extraction or
publication leaves the event pending; a fresh process resumes committed progress.
The source history and outbox are retained. This initial direct-source profile
has no inferred cross-record dependencies.

Source reads verify the selected STO frame and its envelope against the exact
committed locator, fenced intent and sealed pin. They return bytes without
constructing a new durability receipt. Commit admission and cold recovery retain
whole-segment verification; damage to an unrelated frame is detected by that
frame's read or a cold audit. Work counters distinguish selected source bytes,
segment bytes including envelopes, custody metadata and committed-member rows.
These are logical read counts; device I/O is measured separately.
Initial predicate completion uses the existing transaction and audit triggers.
New predicates are inserted ready only after the complete semantic seal, while
still invisible outside that transaction; retained incomplete predicates are
completed in keyset batches of at most 256. Already complete rows are not
rewritten. The bootstrap result reports completion rows and batches separately;
this does not change source admission or the final domain readiness fence.


The native owner entry is:

```sh
tos-native-owner-command source-projection --selection /absolute/private/selection.json < request.json
```

Supply the PostgreSQL 16 connection privately through `TOS_SOURCE_PG_URL`.
The transport uses the existing local NoTls profile: one numeric TCP loopback
host, an explicit user/database and at most one port. Credentials are never
accepted from the request or printed. The selection file must be owned by the
invoking user, protected from other users and located under protected parents:

```json
{
  "schema_version": "tos_source_projection_selection_v1",
  "domain": "owner-selected-existing-domain",
  "source_contract_sha256": "<exact existing domain contract SHA-256>",
  "consumer": "source-text-v1",
  "profile": "tos_source_selected_text_v1",
  "store_root": "/absolute/private/existing-store",
  "store_limits": {
    "max_segment_bytes": 8388608,
    "max_frame_bytes": 1048576,
    "max_frames": 64,
    "max_journal_bytes": 1048576
  }
}
```

These store limits illustrate the existing small source profile. Select the
actual store's limits; this command neither creates nor admits source data.
Requests are strict JSON objects:

- `{"operation":"init"}` explicitly creates derived tables/indexes. It does
  not initialize the source schema, domain or source store.
- `{"operation":"apply","max_events":8}` resumes up to that many events.
- `{"operation":"status"}` reports current, pending or stale progress.
- `{"operation":"queue-age"}` reports the selected consumer's sequence lag
  and oldest unprocessed event. Its millisecond age and UTC Unix-microsecond
  enqueue time are present only when that event has a persisted server clock;
  migrated rows without one report
  `oldest_unprocessed_age_status:"unknown_historical"` and null
  time/age. An uninitialized consumer starts at effective cursor zero. This is
  telemetry for that projection consumer, not a global delivery acknowledgement.
- `{"operation":"search","query":"Socrates","limit":20}` returns at most
  50 ranked hits, each with its exact source path and source revision.
- `{"operation":"exact-source","source_path":"ToS/path/record.json","revision":1}`
  reads the retained exact bytes under current local rights and returns their
  Base64 representation and SHA-256. Historical source selection is independent
  of the newest search result.

The field selection lives in `source_search_projection.rs`: corpus and historical
record titles, variant labels and declared descriptive fields; historical claim
quotes; current freeform HumanForms. Only `.json` and `.jsonl` sources participate.
Technical keys, source paths, provenance and arbitrary nested objects are not
search text. Projected document-type labels are family-qualified exact identifiers:
`corpus_record/<record_type>`, `historical_record/<record_type>`,
`historical_claim`, and `human_form/<role>`. A supported claim without quotes
still has exact ID lookup. IDs are matched exactly before ranked text matches.
PostgreSQL `pg_catalog.simple` and
`plainto_tsquery` define token matching; GIN indexes the selected text. Title,
alias and body use weights A, B and C, followed by stable byte-order ties. This
is a separate search contract from whole-JSON substring search. There is no
pagination or total-count claim in this first profile.

The explicit `tos_source_selected_text_v2` profile adds current source-copy
HumanForms for Agent records. It resolves only the sibling pair
`ToS/source-witnesses/agents/<slug>/agent.json` and
`agent.human-forms.json`, using the shared source-copy materializer. The form set
must name the exact subject identity, version and digest. Freeform and direct
record text keep the v1 field selection; templates and prior forms remain
outside the profile. A missing or stale subject produces no source-copy text.

Select v2 with a new consumer ID and replay from zero, or use the explicit
initial `bootstrap-text` route below; changing the profile of an existing
consumer is refused. The worker reads paired sources at the event's
commit sequence, follows bounded reverse dependencies and includes the exact
sibling candidate when a record changes. It publishes the affected documents
and cursor atomically. A direct document has one source dependency; a
materialized source-copy form has the form file and record as two dependencies.
The per-event limits still bound the selected roots, input bytes and output.

A consumer is bound to its explicit profile/version/source contract. Search
requires its committed cursor to reach the current source head and holds current
local rights during the read. Pending or stale output is not delivered as current
hits. Uncommitted attempt bookkeeping does not invalidate source-text freshness.
External rights and semantic/source admission remain with the actual owner.

The CLI's initial event limits are 64 changed members, 256 documents and 16 MiB
of exact source input, with 1 MiB per document. They bound one compound event,
not total corpus size. `bootstrap-text` provides the separate initial route for
compound seed events that exceed those routine limits. It requires a new
consumer and an explicit `bootstrap` object in the protected selection:

```json
{
  "max_source_work_bytes": 134217728,
  "max_call_work_bytes": 536870912,
  "max_json_visits": 1000000,
  "max_json_state_bytes": 134217728,
  "timeout_ms": 60000
}
```

These values illustrate a selected initial workload, not default routine
limits or a claim that every corpus fits. Reserve the actual peak storage and
memory through the host supervisor before running. Parser workspace bounds
apply to the same strict JSON grammar and exact field selection; the 16 MiB
source, depth, line and output limits remain unchanged. Routine `apply` keeps
its existing parser and event limits. Source work includes exact payload,
custody metadata, physical segment bytes and projected documents; it is not a
measurement of device I/O.

Call `{"operation":"bootstrap-text","max_sources":32}` repeatedly. Each call
seeks from a durable source-path position and commits each source with its
selected companions, documents and position atomically. A failed source leaves
earlier committed work available for resume; reopening or retrying after a
lost reply continues from that position. The ordinary consumer cursor stays
zero and search reports pending until EOF publishes the complete captured head.
The final call is replayable. Ordinary event application is refused while that
consumer's initial build is unfinished. After completion it continues from the
captured head using the existing dependency and publication engine.

A changed source, rights, schema or audit fence refuses the partial build;
select a new consumer to build the new cut. Previously completed consumers are
retained. Neither initial construction nor its progress grants source admission,
rights or publication authority. Deadlines are cooperative around database and
filesystem calls; retain the normal host resource supervisor.

The source-text projection is not a general source-command migration, does not
resolve every knowledge-graph join and does not replace CLI/HTTP/MCP/Worker
knowledge readers. Full Site exports and their existing versioned readers
retain their compatibility contract while that migration is implemented.

## Native PostgreSQL CMD-local rights revocation

The protected `tos_local_pg_rights_revoke_invocation_v1` owner route accepts
`describe` and `rights.revoke` only. Its owner configuration binds the invoking
UID, one private source root, one CMD domain, an expiry, and the sole allowed
operation `rights.revoke`. Revocation uses a stable command ID and expected
current rights version; an exact retry recovers a committed result, while a
changed request under that ID conflicts. A new command after revocation is
refused. The command has no grant operation.

This route changes only CMD's current-use rights bit for the selected
PostgreSQL domain and keeps the supplied reason private; it does not decide ToS
rights, consent, publication, or source admission. ToS policy and disclosure
leases remain independently enforced by their owner. Projection readers check
the current CMD rights version and refuse revoked or stale output.

Source-navigation preparation, reads and policy operations bind the exact
current CMD rule and rights versions. Both start at zero for a fresh domain;
they are independent of the navigation profile version. A version match still
requires current allowed rights, the selected contract and a live job fence.

## Native PostgreSQL record revision

The ordinary `--invocation` route also supports a flat corpus
`tos_local_corpus_revision_owner_v1` owner for `describe`, `prepare-revise` and
`record.revise`. It uses the existing PostgreSQL 16 NoTls numeric-loopback
connection selected privately through `TOS_SOURCE_PG_URL`. The invocation's
`source_selection` has exactly `domain`, `source_contract_sha256`, `store_root`,
`store_limits`, `job_id` and `job_fence`; the owner configuration's protected
`source_root` must name that same existing private STO root.

`job_id` and positive `job_fence` select one existing current PG job lease for
the selected domain. The reader checks that exact pair when opening and closing
its read epoch, and registration and commit check it again. The route refuses an
absent or changed lease. It never creates, increments, repairs or substitutes a
job lease; the real-source lab fixture uses its already registered
`private-job` fence `1`.

The reader opens the bounded domain/cohort descriptor in a read-only repeatable
read transaction, then reads only the current owner record's declared source
references, its immediate flat package members, selected HumanForms and
publication/revision carriers needed by this request, and the selected
record/revision schema closure. It reads the explicitly captured software components
separately. `max_members`, `max_member_bytes` and `max_total_bytes` bound that
exact combined command context; the selected package is refused if it has more
than 64 immediate children or any nested directory. This route does not cold
reopen or materialize the complete source cohort. Current rights, rule version,
contract and audit/source epoch are checked again at durable registration,
commit and replay.

`describe` and `prepare-revise` return the maintained owner response without a
durable attempt. `record.revise` runs the same sealed semantic preparation,
registers the exact proposal against its PG readset, seals the proposed bytes in
the selected STO and commits through the ordinary durable owner transaction.
An exact command retry returns the retained commit receipt. Its result envelope
has `grants_admission: false`: source admission, external rights, semantic
acceptance, publication and canon remain separate owner decisions.

The optional selected-record history table is provisioned only through the
explicit `record.revision-schema.provision` request on this invocation route.
It requires the protected selected owner configuration to grant that exact
token in `allowed_operations`. Work owners may continue using the compatible
`work.revision-schema.provision` token. Provisioning takes the core schema
initializer advisory lock, checks the existing CMD.2 schema, creates the
existing `cmd2_work_revision_history_v1` extension in one bounded transaction
when absent, and verifies its exact v1 shape before commit. An already present
but unknown extension is refused. Ordinary selected-record revisions never run
DDL implicitly. A selected `record.revise` with a null `expected_publication`
is refused before preparation unless the extension is already provisioned;
other publication-bound revisions keep their existing route. The provision
result has `grants_admission: false`. Receipts remain scoped to the domain,
store, source path and record identity, with exact request, configuration and
member-version bindings. Work receipts keep their existing encoding. Replay and
ready-attempt recovery still require the current owner authorization.

The independent bounded source-change path awaits focused runtime verification.
Record revisions and PG Agent creations commit against their exact source reads
and affected predicate generations, together with the current source epoch,
rights, rule/contract and selected job lease; an unchanged global head is not a
prerequisite. Revision commits persist structural path indexes for the
owner-produced archive and source-revision-history members and invalidate only
the affected structural predicates. Explicit cold reopen validates each member's
bootstrap or controlled creation lineage and all retained successors. It can
rebuild derived inventory projections after a command invalidates their root;
a still-published projection must continue to match the verified source exactly.
Rebuilt inventory values retain their effective commit sequence separately from
source history. A historical cut selects only projections effective at that cut;
cold recovery does not overwrite an older history row with newer shared inputs.

## Native PostgreSQL initial Agent creation

The ordinary `--invocation` route also has a bounded PG profile for initial
Agent creation from `tos_local_corpus_create_owner_v2` owner configuration. It
accepts only `prepare-create` and `source.create` requests under the existing
`tos_local_source_command_v1` grammar. The owner configuration and current
software capture remain separately selected, protected inputs.
The invocation schema is `tos_local_pg_agent_creation_invocation_v1`.

Its explicit `source_selection` names one existing domain, the exact source
contract, private STO root and limits, plus an existing `job_id` and positive
`job_fence`. The route uses the same PostgreSQL 16 NoTls numeric-loopback
connection as the revision entry. It refuses an absent or changed lease and
does not create or substitute one.

Creation starts from the selected cohort's real bootstrap `SourceRevision`,
kept distinct from the current typed PG generation identity. The adapter selects
only the Agent request's addressed source references, schema closure and exact
software components; it does not reopen a full `CorpusCutReader` or claim a
complete authored inventory. The owner checks the requested source-home range
and stable form/provenance identities through bounded PG observations, then
binds those predicates to durable registration and commit. `prepare-create`
returns the maintained preview without a durable attempt. `source.create`
registers and commits the exact serialized package, with exact retries returning
the same durable result fields and created paths from the retained attempt.
Replay reports zero ordinary source-selection work separately from durable
receipt metadata. The result has `grants_admission: false`; source admission,
external rights, publication and canon remain separate decisions.

## Native PostgreSQL source bootstrap

The protected `source-commands --invocation ABS < request.json` route also
accepts `tos_local_pg_source_bootstrap_invocation_v1` with the bounded
`source.bootstrap` request. It selects one immutable `CorpusCutReader` revision
and complete membership, a source contract and private STO root, an initial or
already-held exact job fence, plus a separately selected Rust software capture,
component set and schema worker. The private owner configuration must repeat
the selected domain, cut identity, job selection and allowed operation exactly.
The route imports that cut through the existing durable coordinator, cold
reopens it and returns the selected complete generation. It does not grant
source admission, publication or canon authority. Bootstrap resumption after a
partial import is unsupported; the selected store and domain must be unused.

Mixed source bootstrap selects the existing owner for Global, Growth, Item and
historical Claims, including their retained forms and history. The ten older
nonhistorical `*-claims.jsonl` families use an explicit `imported_read_only`
contribution that preserves source locators, line numbers, claim digests and
catalog fields. Import does not invent a creation receipt or turn a recorded
rights assertion into an access grant. Private and `local-content` members keep
their exact custody while contributing no public records, events, anchors or
forms.

Addressed inventory uses the same declared Claim stream families as the source
owner. Other retained source packets, including laboratory `graph-claims.jsonl`,
keep their exact bytes and source identity without acquiring a command Claim
owner or catalog membership from their filename suffix.

For a larger exact cut, the separate
`tos_local_pg_source_bootstrap_streamed_invocation_v1` profile streams the
selected revision without requiring a current pointer and retains only one
bounded durable batch at a time. Its fixed batch body limit is 32 MiB, and the
selected source member limit cannot exceed that value. It uses the exact
`source-bootstrap` lease at fence `1`. `initialization_mode: existing` with
`initialize_schema: false` can finish an interrupted assessment after all
ORIGINAL batches have committed. It verifies original EOF, the retained
membership and history, exact lease and initial authority, then runs the same
cold and semantic assessment without rewriting source batches. If assessment
already committed and packing stopped before selection, it instead verifies
the saved projection transcript against current, historical and path-index
rows and finishes the addressed trees. It reuses the completed schema work;
the result reports `completed_assessments_reused`. Partial original writes,
incomplete predicates, changed projection bytes and changed authority are refused.
Results declare this bounded resume
scope and whether the invocation resumed. Source frame and segment limits must
fit the selected largest member and batch framing; the derived index budget
covers metadata and index work, independently of total source body bytes.
Its validated `generation.max_elapsed_seconds` also bounds the streamed
command's cooperative whole-invocation deadline; an enclosing supervisor may
apply a stricter wall-clock limit.
Only this initial-assessment invocation interprets `budgets.max_schema_receipts`,
`max_schema_receipt_bytes` and `worker_cpu_seconds` as the selected full-pass
envelope: at most 65,536 checks, 16 MiB of accounted receipts and 300 worker CPU
seconds. The caller selects smaller positive bounds. Schema execution also
retains the 128 MiB raw-input and 288 MiB wire bounds, a 1 GiB worker address
space ceiling and the lesser of the invocation deadline and one hour. Receipt
count and worker envelope are returned in `schema_work`; failed assessment
reports its completed receipt count. Ordinary command profiles keep their own
smaller limits. This initial-work allowance does not replace addressed updates,
skip checks, reset cumulative accounting or grant completeness before success.
Claim projection reads each owner's bounded current package, referenced
archives and schema inputs. The completed disk identity pass supplies the
corpus-wide identity checks; reading one Claim does not collect the whole
source-witness subtree into memory.
The result also exposes a finite `logical_source_work` object: returned original,
current and software body counts/bytes, fresh-bootstrap PG field/row counters,
authenticated-tree logical read/write/allocated bytes, and private derived-index
input, returned-row, logical-byte and high-water counters. The index row and
logical-byte budgets bound cumulative work, including repeated metadata reads;
they are not unique source counts or physical file-size caps. A refused streamed
assessment preserves its numeric work counters in stderr for cost diagnosis.
The source-index preflight counts logical field bytes, with independent key
and projection-value limits. The complete cold audit has a different contract:
its metadata-byte budget counts every byte of the retained PostgreSQL JSON
encoding, including bytea hex expansion. Streamed cold audit transfers that
encoding in 64 KiB fragments and preserves the existing complete-state digest;
it does not allocate an entire encoded row in Rust. The caller must still
select and reserve a sufficient aggregate initial-audit profile. Ordinary
addressed updates do not repeat this complete audit, and the legacy whole-row
reader retains its 1 MiB row limit.
Text identity assessment retains per-member identifier/path/digest rows while
discarding each parsed body. The existing owner parser and point-index
comparison still govern identity and fixity; metadata row/byte budgets bound
the retained contribution set independently of the summed source text size.
These are logical owner counters, not whole-operation or physical-I/O
measurements; process and cgroup accounting remains separate.

Initial Source Navigation preparation reopens that completed addressed
generation under its current PostgreSQL, rights and custody fences. Its
explicit export visits bounded metadata pages and delivers one source body at
a time; it does not repeat source assessment or build a complete body map.
The export carries the exact original history and verifies current membership
through EOF before compilation. Protected export bytes/inodes and retained
manifest/history budgets govern this initial operation independently of the
ordinary command batch and portable model budgets. A stale selection or a
restored database still requires explicit cold assessment before export.

`source_selection.initialize_schema: true` must also be present in the
protected owner configuration, with `initialization_mode: "initialize"` and
the initial `job_fence: 1`, to provision the schema. It initializes the
existing Rust DDL only when the CMD.2 namespace is absent, under a transaction
advisory lock. Any existing CMD.2 object makes that choice refuse; it never
silently applies the idempotent DDL as a migration. When the choice is absent
or false in both selections, the route requires the provisioned CMD.2 tables,
indexes, functions and audit triggers and performs no DDL. This describes
native provisioning capability, not evidence of a live database setup.
The streamed profile also requires the addressed audit companion. Its authorized
initialization installs that companion through the existing opt-in initializer;
otherwise it verifies the existing SQL bindings read-only. This check precedes
private STO initialization and all imported source writes. Streamed tree limits
are likewise checked against the STO representation before input is consumed.

The transport selects PostgreSQL 16 through the existing private NoTls
numeric-loopback profile and `TOS_SOURCE_PG_URL`. Deploy this private
coordinator in a dedicated database whose ordinary query roles cannot bypass
adapter rights, write coordinator tables or alter DDL and audit triggers. The
database owner remains trusted; schema setup alone does not prove those live
role grants or runtime behavior.

Provide a nonempty `TOS_CMD_POSTGRES_URL` for a dedicated isolated PostgreSQL instance and run `cargo test -p tos-command --features postgres-lab --test postgres_durable_lab --locked -- --nocapture` from the OPS-integrated workspace. Explicit PG target execution fails closed without the URL. Destructive child/restore probes remain explicitly ignored and are not part of ordinary CI execution. Tests establish bounded mechanical behavior, not source or rights admission.

## Native PostgreSQL public Text creation

The ordinary `source-commands --invocation ABSOLUTE_INVOCATION < REQUEST_JSON`
entry dispatches the independently selected
`tos_local_pg_public_text_invocation_v1` profile to the bounded public Text
adapter. The invocation has a closed top-level shape and selects the protected
`tos_public_native_text_create_owner_v1` grant, current executable digest, one
existing CMD domain/STO root/job fence, an exact software capture/component
set, schema worker and budgets. It needs `TOS_SOURCE_PG_URL` for the existing
PostgreSQL 16 local transport; credentials stay outside the invocation and
request.

The shape fixtures are
[`invocation.example.json`](tests/fixtures/source-public-text-pg-v1/invocation.example.json)
and
[`create.request.example.json`](tests/fixtures/source-public-text-pg-v1/create.request.example.json).
Their placeholders are not runnable credentials or owner selections. The
module's source regression test checks the closed invocation and request
grammar. Full fields, command order, source-selection boundary and recovery
semantics live in the
[public construction contract](../../../mechanics/growth-cycle/parts/branch-growth-cycle/docs/PUBLIC_NATIVE_TEXT_CONSTRUCTION.md).
Construction selects only the ten contract schemas it consumes and checks
delegated Text identities with batched, generation-fenced point lookups; it
does not census the full Text metadata namespace.

## Explicit native backup and restore

`tos-native-owner-command backup` and `restore` select the bounded PostgreSQL/STO
transport directly; the existing `--invocation` source-command route is separate.
Set `TOS_BACKUP_PG_URL` through the operator's private environment for the selected
PostgreSQL 16 database. The current transport uses `NoTls` and accepts exactly
one numeric TCP loopback host (`127.0.0.1` or `::1`), an explicit user and database,
and at most one port. `sslmode=disable` or the default `prefer` is accepted;
TLS is not used. Required TLS, `hostaddr`, socket or multiple-host selection,
and non-default `target_session_attrs` are refused before backup writes. This
is a local operator route, not remote or TLS backup support. Do not put
credentials in command arguments or receipts.
Use an explicitly selected, SHA-256-bound `pg_dump` for backup and `pg_restore`
for restore. Pin tools from the selected PostgreSQL 16 runtime, not ambient
`PATH`; obtain their version and digest from owner-held runtime custody. The
selected executable is checked before and after its operation. An explicit
container wrapper must additionally bind the owned container/image and inner
tool digest; its own digest is the CLI `--pg-tool-sha256`. Dump bytes travel on
stdout into the host's fresh file; restore reads the verified dump on stdin.
There is no extra-descriptor or directory-path transfer across a container.
The commands do not initialize a missing schema or select restored data for
readers.

Both operations require these option/value pairs:

```text
--domain DOMAIN
--store-root ABSOLUTE_STORE
--backup-root ABSOLUTE_BACKUP
--pg-tool ABSOLUTE_PG_TOOL
--pg-tool-sha256 LOWERCASE_SHA256
--max-segment-bytes POSITIVE_INTEGER
--max-frame-bytes POSITIVE_INTEGER
--max-frames POSITIVE_INTEGER
--max-journal-bytes POSITIVE_INTEGER
--max-seconds POSITIVE_COOPERATIVE_DEADLINE
```

Both operations optionally accept `--profile ABSOLUTE_JSON`. The CLI opens a
normalized absolute path without following symlinks, and accepts only an
invoking-user-owned, single-linked regular file no larger than 4 KiB. The JSON
must contain the exact `BackupProfile` fields; unknown fields and values that
fail the existing profile validator are refused. If omitted, the existing
default profile remains in force. The selected limits cover store and dump
bytes, file and entry counts, history/log/member/metadata rows, metadata bytes,
and receipt bytes. The selected dump-byte limit must be at least the process's
soft and hard `RLIMIT_FSIZE`; the transport does not silently substitute the
old fixed 64 MiB ceiling when an explicit profile is supplied.

V9 receipts bind the selected profile. Restore must receive the same profile
that produced the receipt, and the existing exact stored-profile comparison
rejects a mismatch. For older receipts without a profile, omit `--profile` to
retain the compatibility default; supplying a different profile does not
reinterpret their historical bounds.

`--max-seconds` supplies the cooperative deadline checked by the transport.
Run the command under an owned supervisor that applies the outer hard timeout
to the entire process tree (for example, the admitted service with
`KillMode=control-group`) and stops/removes/reaps its exact owned container,
if used. A timeout that only signals the CLI process group is insufficient:
tools use their own process groups, abrupt CLI termination does not run Rust
destructors, and a container's tool may survive its client. Synchronous
filesystem or database calls may block between deadline checks. Expiry is a
failed, partial operation, never permission to select the destination.

Backup additionally requires `--confirm-quiescent-owner yes`: the owner must
actually keep this database and store quiescent across the operation. This flag
records that precondition; it does not stop writers or acquire their authority.
Restore additionally requires `--confirm-fresh-target-owner yes` and
`--receipt-sha256 EXPECTED_BACKUP_RECEIPT_SHA256`, selected independently from the
supplied backup. Keep the selected completed receipt digest outside the backup
selection, then supply it explicitly for restore. The source database and store
must represent the same quiescent cut. Select a fresh target database with an
empty public schema and a different database name; the checked SQL guard
rejects existing public tables, while the owner confirmation supplies the
stronger fresh-database precondition. Its store must be an empty,
independent directory. Source, backup and target roots must be private owned
mode0700 directories without symlink traversal; store members must be regular
files without hardlinks. Restore verifies the receipt/dump and copied store,
then restores the schema/data in one PostgreSQL transaction and checks the
native committed-history cut. A missing dump schema is not filled in by lab
initialization.

New receipts use `tos_cmd2_quiescent_backup_v10`. They also authenticate the transactional navigation cohort-map roots and immutable nodes, while V1–V9 readers retain their historical census rules. Their bounded census includes
the current durable-schema profile, publication/import lineage, source indexes,
and the complete source-navigation projection, policy and processing state.
The V3 navigation census canonicalizes database row keys before counting and
hashing them, so physical column order after a schema upgrade does not change
its identity. Backup and restore cold-verify the cut before recording or
comparing these roots from the same quiescent state. Cold verification is
read-only.

Restore retains explicit wire readers for `tos_cmd2_quiescent_backup_v1` through
`tos_cmd2_quiescent_backup_v8`, including their original census hash rules;
each receipt version carries only its declared
census coverage. A receipt's `v1` label is not SQL-schema compatibility: it
describes the old receipt/census wire, not permission to restore an old
`durable_schema.sql` ABI. Every restored database must still pass the current
CMD.2 schema and active audit-marker profile checks. The
`cmd2_audit_delta_v1_domain` marker remains V1. Its `profile_digest` must be
current or one of the explicitly reviewed cold-transition identities above.
Restore preserves the marker and journal exactly; cold reselection binds a
fresh descriptor to the target database after the complete audit and any
reviewed trigger upgrade. Unknown historical SQL profiles refuse. Where
retained authored source inputs are available, the current source import or
bootstrap route can build a fresh domain; a SQL backup receipt is not that
export route.
Unknown receipt versions are refused.
Do not point either operation at another owner's live resources.

All selected store and backup directories must already exist, belong to the
invoking user and have mode `0700`. Create the backup destination and restore
store as new empty directories under an owned parent; the transport opens them
without creating missing roots. It refuses nonempty destinations. Do not reuse
a failed partial destination or change permissions on another owner's directory.

The compatibility default transport profile is bounded to 64 MiB of store files
and 64 MiB of dump data, with 256 files and 512 filesystem entries. An explicit
profile may select different finite values. Actual file writes remain bounded
by the invoking process's soft and hard `RLIMIT_FSIZE`, both of which must be
no greater than the selected dump cap; use matching explicit byte units in the
host launcher. The host dump writer and any inner
tool must retain the admitted limits. Each native tool has a 60-second ceiling
within the remaining whole deadline. Segment limits do not increase the
selected transport profile. Reserve aggregate physical space, PostgreSQL
working space and filesystem overhead through the host route before execution.
These bounds do not establish the migration's billion-record capacity target.

A successful result preserves the selected mechanical cut; source admission,
current rights and reader activation retain their separate routes. On any
failure, preserve the partial destination for owner-directed recovery and do
not select it as restored data. A nonzero exit is not a completed restore even
if some files or database objects were created. Existing ignored export/verify
probes exercise the same transport and remain separate from ordinary CI. The
installed operator path uses the same module: export an isolated quiescent
fixture, invoke the installed `backup`, select a different fresh database/store,
invoke installed `restore`, then run the existing cold verifier. Do not also
set the optional direct-transport test variables for that sequence.

For the existing finite three-history-member profile, the store selectors are
`--max-segment-bytes 8388608 --max-frame-bytes 1048576 --max-frames 64
--max-journal-bytes 1048576` (each flag and value are separate arguments).
Select each tool's own digest and keep `TOS_BACKUP_PG_URL` private for the
currently selected source or target database. Those fixture limits are an
example, not an arbitrary-data capacity guarantee. A completed
`backup_complete` receipt and `restore_verified` cut do not activate a reader,
change source admission or grant current-use rights.

The same ignored export/verify pair also accepts lab-only
`TOS_CMD2_RESTORE_REVISIONS` (2..64 A revisions) and optional
`TOS_CMD2_RESTORE_PAYLOAD_BYTES` (1..65536 bytes per A revision). Select identical
values for export and verify. With both unset, the original two-commit,
three-history-member bytes remain unchanged. Extra revisions use real commits
with immediate predecessors, and the verifier checks every historical address,
retained prepare intent, original orphan and current rights fence. The profile
lines report actual commit/history counts, framed bytes and elapsed phase times.
These parameter ceilings bound fixture preparation; they do not change the
transport guards or establish corpus capacity. A larger whole operation needs
its own complete disk/time admission, including export, PostgreSQL/WAL, dump,
three store copies, restore and cold verification. It is independent of the
Agent source/archive restore and prepared graph's N/E/history measurement.

## Installed Agent correction and recovery

An installation containing the native command products exposes the owner command
at `PREFIX/software/native/bin/tos-native-owner-command`. It consumes an explicit
owner-selected invocation and a JSON request on stdin:

```sh
"$PREFIX/software/native/bin/tos-native-owner-command" \
  --invocation "$ABSOLUTE_INVOCATION" < "$REQUEST_JSON"
```

The invocation binds the source, prepared database, software and resource limits;
the request cannot substitute paths or grant itself authority. The Agent route
accepts `describe-agent-execution`, `reviewed-agent-execution-bootstrap`,
`publish-agent-correction` and `inspect-agent-publication`. Bootstrap requires the
reviewed transition selected by the owner. Publication and inspection carry the
original `record_request` and `recorded_at`; retain them with the source result.

Source commit and prepared publication are separate outcomes. After an interrupted
publication, `inspect-agent-publication` reconciles the retained source observation
with the committed prepared binding, source vector and catalog in a read-only
transaction. It does not roll back the source, accept an arbitrary replacement
binding or grant a new writer. Resolve its result before selecting a successor or
retrying publication; repeating source creation is not recovery.

Read the resulting publication through installed `PREFIX/bin/tos` with the exact
`--prepared-read-model` and `--prepared-binding` pair, including the existing
CLI, `serve` and `mcp` routes. Software installation does not select this data.
A verified source archive restore is a separate operation; successful publication
inspection alone does not establish backup completeness or recovery on another
machine.

Maintained Agent, Claim/Metadata publication and exact-source owner reads use
the native source owner. The former Python module remains historical evidence;
it is not a maintained runtime or publication dependency.

## Private native Original candidate

An installation containing the matching native owner command exposes the explicit
`native-original-produce` entry. The command consumes one strict JSON request on
stdin; it does not require a source checkout or a Python producer:

```sh
"$PREFIX/software/native/bin/tos-native-owner-command" \
  native-original-produce < "$REQUEST_JSON"
```

The request schema is `tos_native_managed_original_produce_request_v1`. It declares
`tmpfs_quota_bytes`, `tmpfs_inode_limit`, `working_ram_bytes`,
`persistent_write_cap_bytes`, `max_build_seconds`, `max_state_bytes`,
`max_json_visits`, `cold_open`, `process_limits`,
`data_directory`, `private_release_directory`, and `evidence_refs`. The latter
contains exactly one absolute path and SHA-256 for each of `admission`, `built`,
and `verified`. Held references are evidence inputs, not grants of authority.
Select fresh disjoint child names under the host-selected private persistent
store; the command refuses reused destinations.

This initial entry is bound to the admitted historical corpus revision
`5bf2c949b2cec6c758bda0bc3fbe51d03c89ac70088e2e3e34893600304c5183`
and the exact retained runtime input census compiled into
[`native_snapshot_manifest`](../tos-compiler/src/native_snapshot_manifest.rs).
It excludes the historical SQLite output and builds a new model and two Original
receipts through the maintained native capture/compiler route. Historical data
provenance and the executing software fingerprint remain separate identities.
It is not a general arbitrary-corpus admission command.

The host selects an authentic sealed private-stage ticket with a persistent store
outside its tmpfs, passes `ABYSS_STAGE_TICKET_FD` and `ABYSS_STAGE_ROOT`, and applies
the whole-process resource envelope before execution. The request must match the
selected ticket. The staging quota is at least 2,112 MiB; the complete persistent
candidate is capped at 512 MiB and its manifest at 1 MiB. These are refusal bounds,
not measured fit. `max_state_bytes` is an explicit Rust/SQLite state allowance,
separate from model bytes and no greater than the original process address-space
ceiling; `max_json_visits` bounds aggregate capture, writer and cold JSON visits.
The same dedicated SQLite heap and held creation state cover the full writer and
the copied fs-verity cold reader. `cold_open.max_file_bytes` also narrows the
writer's live main and temporary database ceilings before VACUUM. The held Linux
cgroup verifies an actual finite total ceiling and zero swap, retains that
ceiling through cold verification, and requires room for the same declared
tmpfs quota plus process working RAM as the stage issuer. The result reports
this actual total ceiling separately from the working RAM ticket; ticket
metadata alone does not provide that custody. The declared process limits are checked during cold
verification and do not install limits for earlier capture/build phases. Use the
owned supervisor's whole deadline and process-tree cleanup in addition to the
cooperative build deadline.

A successful `tos_native_managed_original_produce_result_v1` result identifies
only a fresh private data candidate, full typed selection and actual fs-verity
cold-open witness. Capture up to 4 MiB of JSON plus the final LF. The command
leaves the private release child absent. Verify and promote the pair through the
existing release holder, then use a matching installed Access product with
`--release-root` for CLI, HTTP or MCP. The reader must support the produced
captured-runtime Original profile; an older installed product cannot be relabeled
as compatible. Production and cold-open success do not select a public release,
accept source/rights/canon, or establish installed consumer acceptance.

For a committed native V2 publication whose later read/restore failed, run
`corpus-admit --store PATH --input-root PATH --verify-committed-revision SHA256
--grammar-root PATH --invocation PATH` with a fresh restore target in the selected
V2 case. This consumes the existing selected revision, native completion proof,
original validator and exact history/source record. It cannot create a candidate
or publish a new revision, including on a lookup miss. A newer installed reader
can complete verification of an older admitted writer's result. The receipt names
`verification_only`, `publication_performed: false` and the original validator.

Cold V2 restore partitions its original state between fixed receipt/decoder
state, the SQLite spill and one bounded tree stream. The stream admits its live
stack and each node decoder before allocation; cumulative traversal ceilings
remain separate. The restored target is selected only after full physical and
historical closure succeeds. Copy holds the source lock and checks each file's
identity and bytes for changes. The complete authenticated walk runs on the new
copy, including packed frames, history, identities, membership and dependencies;
no source-side verification result is reused as proof for the target. A corrupt
copy may occupy its bounded reserved space but receives no current selector.

## Protected private owner operations

`tos-native-owner-command private-text-read --invocation ABSOLUTE_INVOCATION`
also dispatches the protected PostgreSQL invocation profiles
`tos_local_pg_private_text_invocation_v1`,
`tos_local_pg_private_owner_invocation_v1` (Profile/Claim),
`tos_local_pg_private_assessment_invocation_v1`, and
`tos_local_pg_private_alignment_invocation_v1`. Each profile selects its own
request grammar, exact source/software inputs, owner context and operation
grant. The command name is retained for compatibility; these profiles include
their explicitly granted writes. Ordinary `source-commands` and HTTP do not
expose these private profiles. Connection selection remains private through
`TOS_SOURCE_PG_URL`; an invocation does not grant rights or semantic approval
beyond its selected owner contract.

The PostgreSQL Text profile also accepts `prepare-create` for its selected
TextLayer, derived-layer or TextUnit write grant. Preparation uses current PG
source dependencies and returns the exact owner configuration and dependency
binding that the corresponding write must carry. A created TextUnit exposes
its binding from the actual retained packet and layer bytes.

For separately authorized PG reads, the invocation selects `owner_config`
using [`native-pg-private-text-read.schema.json`](../../../ToS/contracts/native-pg-private-text-read.schema.json).
It pins the source domain/contract, owner context, mandate, exact binding,
rights references and each private package revision, digest and command ID.
`native-text.read-private` additionally names `source_path`, `revision`,
`package_sha256`, `command_id` and `binding_ref`, alongside `binding` and
`max_return_bytes`. Current source rights are checked before opening private
content, including for historical package reads. The grant and selected
source/private generations are checked before returning the bounded spans.
The PublicProfile creation invocation can select the same independent grant
through `native_text_read_profile` when its `source-text-unit-v1` adapter needs
private content verification; fresh commits and recovered retries both check
the grant. These paths retain the protected local process boundary.

### Protected private TextUnit reading

`tos-native-owner-command private-text-read --invocation ABSOLUTE_INVOCATION`
reads a request from stdin with `schema_version` set to
`tos_local_native_private_text_read_request_v1`, operation
`native-text.read-private`, an exact native `binding`, and `max_return_bytes`
from 1 through 1048576. The existing protected native source invocation selects
`owner_config` with `tos_native_private_text_read_v1`, its matching
`owner_context`, current source/software captures and schema worker. The
selection is independently issued, expires within one day, and names exact
bindings, recorded rights and a mandate. Library callers use
`PrivateTextReadSelection` and `read_private_native_unit`.

Returned spans preserve source coordinates, exact UTF-8 and full rights records.
The operation rechecks the grant, context, sources and schema before return;
it neither opens original Item payloads nor changes source or publication
state. The ordinary source-command/HTTP dispatcher refuses this operation.
