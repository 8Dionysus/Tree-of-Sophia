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

`DurablePgCoordinator` binds registered attempts and exact STO locators to atomic current/history/receipt/log/outbox writes. Current rights, rule/schema contracts, job leases, retention pins and audit/restore fences are checked at commit and replay. Private source creation also binds owner-derived exact, absent, identity and namespace-range observations to the registered package; predicate validation and invalidation commit together. Mutable external authored trees remain `FullOnly`.

The former CMD1 `PgCoordinator`, synthetic byte references, independent SQL schema and protocol-only test target are retired. Their useful conflict, ordered commit, current authority, retained history, rollback, replay and closed-cut integrity obligations execute on the durable path. Real managed creation exercises form-identity and source-home phantom generations, definition/completeness refusal and unchanged projections on refusal. The earlier synthetic prefix/reverse/interval names did not establish any maintained source reader or completeness law; their removal grants no corresponding affected admission.

The v1 cut-based managed source preparation uses a complete immutable manifest
and authored input inventory. Successor selection performs O(N)
manifest/hash/fixity work and retained-base-chain opening outside commit locks;
its creation reads bind the entire verified file set. The explicit PostgreSQL
record-revision and initial-Agent routes below select a bounded source readset
instead. Other source-command families still use their declared cut-based route.

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
search text. A supported claim without quotes still has exact ID lookup. IDs are
matched exactly before ranked text matches. PostgreSQL `pg_catalog.simple` and
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

Select v2 with a new consumer ID and replay from zero; changing the profile of
an existing consumer is refused. The worker reads paired sources at the event's
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
not total corpus size. Oversized initial bootstrap events require a resumable
bootstrap path; increasing these limits is not that implementation. Run under
the normal host resource supervisor: deadlines are cooperative around database
and filesystem calls.

The source-text projection is not a general source-command migration, does not
resolve every knowledge-graph join and does not replace CLI/HTTP/MCP/Worker
knowledge readers. Full Site exports and their existing versioned readers
retain their compatibility contract while that migration is implemented.

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

Provide a nonempty `TOS_CMD_POSTGRES_URL` for a dedicated isolated PostgreSQL instance and run `cargo test -p tos-command --features postgres-lab --test postgres_durable_lab --locked -- --nocapture` from the OPS-integrated workspace. Explicit PG target execution fails closed without the URL. Destructive child/restore probes remain explicitly ignored and are not part of ordinary CI execution. Tests establish bounded mechanical behavior, not source or rights admission.

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
Do not point either operation at another owner's live resources.

All selected store and backup directories must already exist, belong to the
invoking user and have mode `0700`. Create the backup destination and restore
store as new empty directories under an owned parent; the transport opens them
without creating missing roots. It refuses nonempty destinations. Do not reuse
a failed partial destination or change permissions on another owner's directory.

The initial transport profile is bounded to 64 MiB of store files and 64 MiB of
dump data, with 256 files and 512 filesystem entries. The invoking process must
have both soft and hard `RLIMIT_FSIZE` at most 64 MiB; use explicit byte units in
the host launcher. The host dump writer and any inner tool must retain the
admitted limits. Each native tool has a 60-second ceiling within the remaining
whole deadline. Segment limits do not increase those transport bounds. Reserve aggregate physical space, PostgreSQL
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

The Python `source_agent_publication` module remains a dependency of maintained
Claim/Metadata publication and exact-source owner reads. A successful native Agent
projection does not authorize removing those consumers. Retire the old module
only as those operations acquire working replacements and their unique controls
move to the replacements.

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

## Protected private TextUnit reading

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
