# `tos-command`

The crate owns source command preparation and durable PostgreSQL/STO transaction mechanics. Source, semantic, rights, publication and canon admission remain separate owner gates. Canonical writes remain closed without those gates.

`DurablePgCoordinator` binds registered attempts and exact STO locators to atomic current/history/receipt/log/outbox writes. Current rights, rule/schema contracts, job leases, retention pins and audit/restore fences are checked at commit and replay. Private source creation also binds owner-derived exact, absent, identity and namespace-range observations to the registered package; predicate validation and invalidation commit together. Mutable external authored trees remain `FullOnly`.

The former CMD1 `PgCoordinator`, synthetic byte references, independent SQL schema and protocol-only test target are retired. Their useful conflict, ordered commit, current authority, retained history, rollback, replay and closed-cut integrity obligations execute on the durable path. Real managed creation exercises form-identity and source-home phantom generations, definition/completeness refusal and unchanged projections on refusal. The earlier synthetic prefix/reverse/interval names did not establish any maintained source reader or completeness law; their removal grants no corresponding affected admission.

The current managed source preparation still uses a complete v1 immutable manifest and complete authored input inventory. Explicit successor selection performs O(N) manifest/hash/fixity work and retained-base-chain opening outside commit locks. Creation reads still bind the entire verified current file set and globally invalidate the consumed inventory. Addressed durable predicates alone do not make the maintained command scalable. A versioned indexed source carrier and owner-derived dependency contract are still required before that coupling can change; no cap increase or weaker v1 root substitutes for them.

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
