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
PostgreSQL 16 database. Do not put credentials in command arguments or receipts.
Use an explicitly selected, SHA-256-bound `pg_dump` for backup and `pg_restore`
for restore. The commands do not initialize a missing schema or select restored
data for readers.

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
between deadline checks. Expiry is a failed, partial operation, never permission
to select the destination.

Backup additionally requires `--confirm-quiescent-owner yes`: the owner must
actually keep this database and store quiescent across the operation. This flag
records that precondition; it does not stop writers or acquire their authority.
Restore additionally requires `--confirm-fresh-target-owner yes` and
`--receipt-sha256 EXPECTED_BACKUP_RECEIPT_SHA256`, selected independently from the
supplied backup. Its database and store must be fresh, independent destinations.
Do not point either operation at another owner's live resources.

The initial transport profile is bounded to 64 MiB of store files and 64 MiB of
dump data, with 256 files and 512 filesystem entries. Segment limits do not
increase those transport bounds. Reserve aggregate physical space, PostgreSQL
working space and filesystem overhead through the host route before execution.
These bounds do not establish the migration's billion-record capacity target.

A successful result preserves the selected mechanical cut; source admission,
current rights and reader activation retain their separate routes. On any
failure, preserve the partial destination for owner-directed recovery and do
not select it as restored data. A nonzero exit is not a completed restore even
if some files or database objects were created. Existing ignored export/verify
probes exercise the same transport and remain separate from ordinary CI.
