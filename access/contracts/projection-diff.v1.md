# Bounded projection differences

`tos_access.projection_diff.diff_projections` compares two explicitly selected
`ProjectionReader` snapshots. It is an offline, read-only extension of
`tos_partitioned_projection_v1`, not a source builder or a prepared publisher.
It establishes a logical record delta under a caller-supplied baseline trust
assertion. It neither establishes that trust nor verifies target availability.

```python
packet = diff_projections(
    before_reader, after_reader,
    expected_before_sha256=before_root_digest,
    expected_after_sha256=after_root_digest,
    trusted_baseline_sha256=independently_admitted_before_root_digest,
    limits=DiffLimits(),
    include_rows=False,
)
```

The three digests are exact lowercase SHA-256 root-byte identities. The caller
must have independently established the baseline closure's integrity and
applicability. Repeating an arbitrary digest is not admission. The function
checks that the trust assertion selects the exact before binding but cannot
authenticate the external admission decision. There is no default trust grant.

### Retained immutable roots

`diff_projection_snapshots` accepts two explicit `ProjectionSnapshotView`
values with the same digest, limits and row-selection arguments. It shares
the complete comparison kernel, but reads root bytes from those values; their
namespace paths locate parts only. It neither writes a temporary root file nor
calls the selected-root API through a disguised reader. The result schema is
`tos_projection_snapshot_diff_v1`, with
`selected_root_currentness_verified: false`. The caller must separately bind
the roots to its before/after committed snapshots. Root decoding reserves the
actual retained byte lengths; part, key and output limits are unchanged. Root
file changes or absence do not invalidate retained bytes. Changed parts must
still pass the same exact content checks; unchanged closure remains dependent
on the explicit baseline admission.

## Result and refusal

Success returns `tos_projection_diff_v1` with `complete: true`, exact before and
after bindings, the caller's baseline assertion, `header_change`, and `changes`.
Each change names a collection, stable logical key, `insert|replace|delete`, and
before/after states. States explicitly distinguish `present: false` from a
present JSON `null`; absent states have a null digest, present states have the
SHA-256 of the value in the existing `canonical_bytes` encoding. With
`include_rows=True`, every present state includes its complete `row`.
Digest-only mode omits all rows; budget pressure never silently strips rows.

The complete before/after bounded headers accompany a changed header, along
with their digests. Collections and changes are ordered by collection/key.
The output deliberately states `target_closure_verified: false` and
`establishes_epoch: false`. A packet is not a target deployment, source
transaction, publication generation, prepared binding, semantic assessment,
rights decision, or source admission.

`ProjectionDiffRequiresBootstrap` explicitly reports `requires-bootstrap` for
a changed logical schema version, collection set, key field, or declared order.
This function never initiates that bootstrap. A malformed projection fails
closed with a projection error. `ProjectionDiffBudgetExceeded` means no
complete delta was established. No partial packet or lazy change iterator is
returned on any failure, including failures after some changes were computed.

## Traversal and bounds

Equal subtree descriptors are skipped only when all descriptor fields except
their validated relative transport path agree: kind, hash prefix, stored and
decoded digests, stored and decoded sizes, and count. Paths may differ when
roots have different names. Changed index contents are checked recursively;
changed leaves are completely decoded and validated before comparison. Tree
split/coalesce compares by logical key, never by position or matching leaf
shape. No materialization, whole-collection fallback, filesystem discovery,
source resolver, graph compilation, or hidden index creation is used.

Skipped parts are NOT read, even if they have disappeared or been corrupted
since the baseline admission. This deliberate trust boundary allows a bounded
logical diff and means consumers must separately establish any required target
closure availability/integrity. Changed leaves cannot exploit an unchanged
content hash with altered size/count metadata to bypass validation.

Default limits, shared across both snapshots:

| Limit | Default | Accounting |
| --- | --- | --- |
| opened parts | 256 | Each changed index/data open; reader caches are disabled. |
| decoded bytes | 16 MiB | Declared decoded part sizes plus one overflow sentinel per part, and the fixed 256 KiB + sentinel cap reserved for each of four initial/final root reads. |
| keys | 4,096 | All declared leaf keys in opened leaves, including unchanged keys on both sides. |
| output bytes | 4 MiB | Complete canonical JSON packet, including headers, framing, and optional rows. |

All limits are explicit nonnegative integers. A part's bounds are reserved
before opening or decompressing it; its actual stored size is checked before
read. A lying count cannot cause excess rows to be parsed beyond its key
reservation. Small root reads have their own format-size precheck. Both roots
are rechecked before successful return; content identity is not an epoch and
does not detect an A-to-B-to-A publication cycle.

The comparison retains the writer's existing canonical decoded JSON identity:
dictionary insertion order is irrelevant, arrays retain order, unknown nested
fields survive, `false`, `0`, `0.0`, and `-0.0` remain distinct encodings, and
Unicode is not normalized. These are projection value digests, not digests of
original source-file bytes, JSON whitespace, or source-language authority.

## Integration boundary

This API is not activated in CLI/core, source builders, or prepared storage.
A future source-to-prepared owner still needs paired source/projection snapshot
bindings, complete command-to-carrier impact, normalization dependencies,
assessment and availability guards, and a publication transaction. A complete
projection diff cannot stand in for those stronger obligations.
