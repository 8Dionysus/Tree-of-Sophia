# Portable partitioned projection utility

`tos_access.projection_store` provides the independent
`tos_partitioned_projection_v1` transport utility. Its structural schema is
[`ToS/contracts/partitioned-projection.schema.json`](../../ToS/contracts/partitioned-projection.schema.json).
Logical record schemas and authored sources retain their stronger authority.

This adoption includes the portable module, schema, and synthetic tests only.
It does **not** switch this checkout's corpus/bibliographic files to manifests,
adopt the partitioned compiler/query store, change source builders or adapters,
rebuild exports, or activate a source-to-prepared transition.

## Storage and explicit operations

Each small manifest contains its logical header, collection descriptors, and
format limits. Arrays have explicit stable key and ordering fields; mappings
have string keys. Scoped identities use an ordered list of key fields encoded
as a compact JSON string array. Hash partition placement is independent of
source order; full materialization restores the declared ordering.

A SHA-256 radix tree selects independently compressed JSONL leaves. Each
descriptor binds kind, hash prefix, exact content-addressed relative path,
stored and decoded SHA-256, stored and decoded byte counts, and record count.
The root is limited to 256 KiB, an index to 128 KiB, a decoded leaf to 8 MiB,
and a key to 4,096 UTF-8 bytes. The normal target leaf size is 1 MiB. A single
oversized record is refused; fields are never silently omitted to fit it.

`ProjectionReader` exposes `metadata()`, `get(collection, key)`,
`iter_items(collection)`, `iter_collection(collection)`, `closure_paths()`,
`materialize()`, and `require_current()`. Exact-key reads validate their selected
leaf completely but do not read unrelated leaves. Collection iteration and
closure traversal are explicit potentially complete operations.
`materialize()` and the legacy `load_projection()` helper may assemble a whole
logical document; they are not hidden bounded-query paths. `get()` returns
`None` both for absence and for a mapping value of JSON null; callers requiring
presence distinction must use the [diff contract](projection-diff.v1.md) or
explicit keyed iteration, not infer absence from `get()`.

Readers reject malformed descriptors, path escapes, symlink parts, wrong
digests, wrong counts, duplicate keys, and misplaced keys on visited parts.
Their decoded-byte cache is bounded. A checksum proves selected transport
integrity, not provenance, semantic acceptance, rights, or independent admission.
`snapshot_digest` identifies exact root bytes; `require_current()` rechecks
those bytes. This is not a source publication generation or an ABA fence.

`write_projection(path, header, collections, ...)` is an explicit offline
writer, not an automatic read side effect. It stages the complete provided
record streams in temporary SQLite, writes immutable parts, then atomically
replaces the root. The caller owns the selected output path and scratch
admission. Failure does not publish a partial root. The default does not prune;
explicit `prune=True` retires only valid digest-named unreferenced objects in
that exact part namespace. Retiring old parts can make old snapshots unavailable.
This whole-input writer is not an incremental source builder.

## Dependencies and adoption boundary

The module uses only Python's standard library, including `sqlite3`, `gzip`,
`hashlib`, and `tempfile`. There is no `DiskCollections`, compiler, QueryStore,
network service, runtime binding, schema-validator runtime dependency, or
package-version change. All store and diff fixtures are synthesized in their
focused test modules; no corpus payload or generated manifest/part is adopted.

The [bounded Merkle diff](projection-diff.v1.md) is a separate read-only layer
over two explicitly bound readers. It relies on an independently admitted
baseline asserted by the caller and does not certify skipped target parts.
The adopted store/schema/tests are exact files from source-owner commit
`78e628f00932cbebecbc3fc0e6f4433cb119a7ff`; the diff module/tests/contract are
exact files from reviewed helper commit
`395caa2f818ca99395155af31924c658b34724fb`. This isolated composition does not
adopt those commits' unrelated source builders, compiler, or runtime changes.

Source snapshot pairing, command-to-projection completeness, dependency-complete
normalization, assessment/journal and availability guards, and atomic prepared
publication remain the next owners' obligations. No admission or activation is
inferred from passing these utility tests.
