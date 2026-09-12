# Bounded projection mutation candidates

`tos_access.projection_mutation.stage_projection_changes` creates a complete
copy-on-write candidate over one explicitly selected
[`tos_partitioned_projection_v1`](projection-store.v1.md) baseline. It writes
immutable parts in that baseline's existing namespace and returns root bytes.
It never replaces the selected root, publishes source or prepared storage,
scans/prunes a namespace, stages all input rows, or starts a bootstrap.

```python
candidate = stage_projection_changes(
    before_reader,
    expected_before_sha256=selected_root_digest,
    trusted_baseline_sha256=independently_admitted_baseline_digest,
    changes=[ProjectionChange(
        collection="records", key="stable-key",
        before_present=True, before_sha256=old_value_digest,
        after_present=True, after_value=complete_new_value,
    )],
    limits=MutationLimits(),
)
view = candidate.snapshot()
state = view.lookup("records", "stable-key")
```

## Exact changes and result

Both digests must bind the exact selected root bytes. Baseline trust is an
explicit **caller assertion**, not established by this function. The caller
independently establishes its applicability/integrity and retains reused parts.
Unchanged subtrees are not opened; success does not certify their availability.

Each `ProjectionChange` selects a unique `(collection, key)`. Presence flags
are actual booleans. Absent before requires a null digest; present before
requires SHA-256 of `canonical_bytes(value)`. Present after requires a complete
JSON value, including literal `None` for JSON null. Absent after requires the
`MISSING` default, never a value. Absent-to-absent and duplicate targets fail.
The row's scalar or composite identity must equal its addressed key. A key
change is explicit delete plus insert, not implicit relocation.

Only exact JSON types are accepted: string-keyed dictionaries, lists, finite
numbers, strings, booleans and null. Python tuple/custom-object/key coercions
are refused. Input nesting is at most 128 levels. Object insertion order is
irrelevant, arrays preserve order, Unicode is not normalized, and false, zero,
float zero and negative zero retain their distinct canonical encodings.

`ProjectionHeaderChange(before_sha256, after_value)` replaces the complete
bounded logical header under its exact prior value digest. Collection/header
overlap is refused. A different logical schema requires bootstrap; collection
set, key fields and declared ordering cannot be changed through this API.
Empty change lists and byte-equivalent replacements are permitted checked
no-ops; a candidate does not manufacture a publication generation.

`ProjectionCandidate` contains immutable `root_bytes`, original
`namespace_path`, exact before/after digests, canonical `delta_bytes`, created
part paths and accounting. `delta()` returns a detached complete change packet
with before presence/digests and complete after values. It explicitly states
`published: false`, `establishes_epoch: false` and
`target_closure_verified: false`. No partial candidate or lazy change stream
escapes on failure.

## Candidate view and ordering

`ProjectionSnapshotView` binds immutable root bytes to the **original selected
root path**, so paths still resolve in `<original-stem>.parts`. It provides
`metadata`, presence-distinguishing `lookup`, explicit `iter_items` and explicit
whole-document `materialize`. Returned JSON is detached. The view is not a
`ProjectionReader`: it cannot enter selected-reader diff/mutation routes and
`require_current()` always refuses. Its parts still require owner retention;
immutable root bytes do not promise immutable external availability.

The mutation profile requires a total declared array order. Default key or
composite-key ordering qualifies. Nonempty `order_fields` must include every
key field; otherwise `ProjectionMutationRequiresBootstrap` refuses even an
empty mutation. This is a conservative, bounded structural condition, not a
whole-corpus uniqueness scan. It preserves v1 string-conversion sorting;
it does not introduce numeric sorting or native source-order tokens. Mapping
collections have no array-order requirement. Unknown source order remains an
integration-owner responsibility.

The reason for this stricter profile is that existing v1 materialization
stable-sorts by declared fields. Ties otherwise inherit radix/leaf traversal,
which a split can change. The generic full writer/reader semantics are not
silently altered to add a tie-breaker.

## Work and budgets

Only addressed paths and complete touched leaves are read, using the diff's
strict descriptor/index/leaf validation. Updates sharing a path share that
traversal. All prior states are checked before any part installation. Leaves
split using the existing SHA-256 radix rule and 1 MiB default target; individual
row framing must fit the 8 MiB hard leaf limit. Deletes remove empty branches;
an empty collection gets an empty data-root. Single-child indexes remain;
there is no implicit coalesce or reading of unrelated siblings. Full-writer
and COW results may have different transport bytes with identical logical rows.

All limits are explicit nonnegative integers; boolean limits are refused.

| Counter | Default | Accounting |
| --- | --- | --- |
| changes | 4096 | Every supplied unique target; duplicates fail. |
| input bytes | 16 MiB | Complete canonical change frames and optional new header. |
| opened parts | 256 | Every touched input part and existing immutable destination comparison. |
| stored read bytes | 16 MiB | Declared stored input sizes and existing-destination bytes, each with sentinel. |
| decoded bytes | 16 MiB | Touched decoded input sizes plus sentinel; fixed root cap plus sentinel for the initial and two final root reads. |
| keys | 4096 | All declared rows of touched leaves, including unchanged rows. |
| written parts | 256 | Prospective unique content-addressed outputs, including existing identical parts. |
| written decoded bytes | 16 MiB | Decoded bytes of those unique outputs. |
| written stored bytes | 16 MiB | Stored bytes of those unique outputs. |
| result bytes | 16 MiB | Complete canonical delta packet plus root bytes. |

Existing root/index/leaf/key format bounds remain unchanged. Reservations
precede part opening/decompression, extra-row parsing and installation writes.
Output bytes are bounded before installation. These are refusal budgets, not
RSS forecasts, host write permission or automatic retry allowances.

## Installation, failures and publication boundary

Parts use exclusive temporary files, file fsync and atomic no-replace hard
links within pinned no-follow namespace directory descriptors. Existing digest
destinations must be regular files with exactly matching bounded bytes; corrupt
content and symlinks fail rather than being overwritten. Files and relevant
directories are fsynced. Only the function's own temporary basename is removed.
No root replacement or old-part deletion occurs.

The caller owns the root parent and cooperative writer stability. This is not
an adversarial same-UID filesystem service. Detected baseline root drift before
or after installation fails; those byte checks are not an atomic CAS or ABA
fence. Interrupted/failed staging can leave unselected immutable parts. Their
existence is not publication, recovery authority or permission to clean them.
Power-loss durability relies on ordinary filesystem hard-link/fsync guarantees.

Source-to-prepared integration remains unimplemented. Its owner must preserve
the existing selected-metadata `PublicationSnapshot` and corpus-lock protocol,
exact source guards for nonparticipating edits, shared assessment/journal
guards, dependency-complete normalization, total ordering, and paired
source/projection/prepared bindings. Selected-metadata publication tokens are
not projection root digests. Prepared SQLite transactions/epochs are not
transactions across source files and multiple projection roots. This utility
creates no source authority registry and broadens no transaction path profile.

Focused synthetic checks: `PYTHONPATH=access/src:access/tests python -m unittest
test_projection_mutation test_projection_store test_projection_diff`.
Test ownership is declared in `tests/test_inventory.json`.
