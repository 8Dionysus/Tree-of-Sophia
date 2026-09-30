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
Run the command under an outer hard timeout that covers the entire operation
and its child processes: synchronous filesystem or database calls may block
between deadline checks. On the host laboratory route, use the admitted
supervisor with `KillMode=control-group`, and stop/remove/reap the exact owned
container in cleanup when a tool bridge is used. Killing a CLI client alone is
not proof that a container's PostgreSQL tool has stopped. Expiry is a failed,
partial operation, never permission to select the destination.

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
