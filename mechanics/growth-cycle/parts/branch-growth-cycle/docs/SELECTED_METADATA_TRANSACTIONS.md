# Selected metadata publication and recovery

This part-local transport moves an adapter's exact bounded source-metadata plan
without exchanging or copying a nested source home. Current authored files still
own source meaning. Retained manifests and blobs are byte/recovery evidence;
neither their presence, a transaction ID, a completion receipt nor a ready token
grants bibliographic, textual, rights, assessment, admission or canon authority.

The earlier flat-package revision/exchange route remains a separate contract.
Its whole-directory package digest must not silently become a digest of selected
files. An adapter adopting this transport owns its explicit v2 scope and history
bindings. This library supplies no public arbitrary-write CLI.

## Read-only publication snapshot

`scripts/source_metadata_snapshot.py` exposes:

```python
snapshot = PublicationSnapshot(absolute_source_root)
token = snapshot.token       # sha256 string, or None for the legacy baseline
generation = snapshot.generation
snapshot.verify_current()
```

The guard reads only the fixed, protected
`ToS/source-witnesses/.metadata-publication.json` control record. It never scans
the source or transaction tree, creates a lock, repairs a transaction or resolves
source identity. `read_publication_state(root)` also exposes pending state for an
explicit owner dispatcher; ordinary `PublicationSnapshot` construction refuses it.

One instance must cover the **whole** logical read: before its first selected or
related source read, through the final verification before returning/publishing a
result. Do not refresh only a portion of an assembled result. Multi-stage builders
must retain the guard until their publication edge; a guard inside one helper does
not cover earlier scans or the caller's later write.

`PublicationPending`, `PublicationChanged` and other `PublicationStateError`
failures mean the consumer cannot establish the protocol's read boundary.
Malformed, duplicate-key, over-budget, digest-inconsistent and non-ready control
records fail closed. Symlink/unprotected-path failures also fail closed. An absent
legacy control is accepted without creating anything. Once initialized, the writer
never deletes the control. External removal is outside this cooperating protocol.

The token covers **participating selected-metadata transactions only**. It does
not certify unchanged manual edits, legacy writers, contracts, filesystem metadata
or runtime state. Retain existing exact-byte/currentness checks for those inputs.
Legacy writers sharing the corpus lock must refuse a pending selected transaction;
merely sharing a lock does not make their mutations participants in this epoch.

A catalog can bind this source token to detect stale membership after a new
selected subject appears, but a token does not itself commit the catalog's files.
That derived owner's hashes/publication guard remain necessary. A stale catalog
after a successful source commit is not authority to roll back that source.

## Internal writer API and caller responsibilities

`source_metadata_transactions.py` exposes:

```python
apply_transaction(root, plan, expected_snapshot=snapshot,
                  authorization_guard=guard, transaction_id=stable_id)
read_pending_transaction(root)
inspect_transaction(root, stable_id)
resume_transaction(root, authorization_guard=guard, transaction_id=stable_id,
                   recovery_authorization=None)
rollback_transaction(root, authorization_guard=guard, transaction_id=stable_id,
                     recovery_authorization=None)
```

All mutation calls require the caller to hold the existing
`source_commands._locked(root / 'ToS/source-witnesses/historical-create')` lock.
The library deliberately does not acquire it recursively. Independent adapters
must preserve that same lock locator and order. Arbitrary unlocked callers are
outside this internal API contract.

The adapter provides an exact plan, not a directory discovery request:

```python
plan = {
    'authorization': {
        'owner_configuration': 'sha256:...',
        'dependencies': 'sha256:...',
        'command_id': 'owner-selected-command',
    },
    'files': [
        {'path': 'ToS/source-witnesses/works/example/work.json',
         'before': previous_exact_bytes, 'after': proposed_exact_bytes},
    ],
    'new_directories': [],
}
```

`before=None` asserts absence; `after=None` explicitly selects file removal.
Zero bytes and absence are different states. Unchanged selected companions are
permitted when they close an adapter's exact input package, but the plan needs at
least one actual change. File/new-directory lists are canonicalized before binding.

The adapter validates source schemas, field/identity/topology scope, form/history
closure and public disclosure. The engine does not infer these from JSON filenames.
Authorization bindings must be public-safe compact evidence, normally digests and
opaque authority/command handles, never credentials or confidential owner configs.
The manifest retains them unchanged.

`guard(original_authorization, plan_summary)` must return **exactly `True`** only
while the caller's current authenticated delegation, explicit scope and dependency
checks authorize that entire retained transaction. Exceptions and every other
return value deny it. The summary contains exact before/after digest/byte bindings,
not executable paths to run. It is copied before each callback. The guard runs
before retention, before pending, during directory/file movement and before ready.
It must revalidate authoritative inputs from their owner, not return a cached
positive verdict. Selected source files are expected to be partly before/after
during recovery; the adapter can revalidate its source record/request using the
retained exact before bytes instead of misreading that intermediate mixture.

Transaction IDs are `sha256:<64 lowercase hex>`. The adapter should derive a stable
ID from its canonical command ID, request digest and owner configuration. It must
not depend on after bytes when a retained source receipt itself contains this ID.
The manifest has a separate exact digest, avoiding a digest cycle. Omitting the ID
creates a fresh random-derived ID and is unsuitable for a caller needing exact
lost-response replay. Reusing an ID for a different bound plan is a conflict.

## Budgets and exact territory

- At most 64 selected regular files, with 8 MiB aggregate before bytes and 8 MiB
  aggregate after bytes; at most 64 explicitly declared new directories.
- Only canonical relative JSON/JSONL metadata paths under `ToS/source-witnesses/`,
  at most 1,024 UTF-8 bytes and 24 path components. No absolute paths, backslashes,
  dot components, hidden/control/history-store paths or ancestor file collisions.
- `payload`, `private`, `local-content`, `owner-local` and `catalog` components are
  prohibited. Caller source schemas may impose strictly smaller byte/path limits.
- Authorization bindings are at most 64 KiB; a manifest at most 512 KiB; publication
  control at most 8 KiB. Recovery renewal evidence is at most 4 KiB.
- Missing parent directories must all be explicitly listed, parent-first after
  canonicalization, and contain an explicitly new selected file. Existing parents
  are bound by device/inode/owner/mode, not volatile directory timestamps.

The library opens protected parent directory descriptors without following any
symlink component and publishes/removes selected basenames relative to those
pinned descriptors. Existing descendants and unknown siblings are not enumerated,
read, copied, exchanged or deleted. Rollback can `rmdir` only explicitly declared
new directories; it never recursively removes one. An unselected child makes
that rollback stop pending for its actual owner.

The local Unix account/root is trusted. Other local users and symlink targets are
not. Same-UID hostile code is not isolated by this library. Cooperating editors
must keep selected files, parents, configuration and dependencies stable during a
writer operation; detected third states or parent changes stop it. Pinned handles
prevent a swapped pathname from redirecting a write into its replacement tree,
but this is not an adversarial same-UID compare-and-swap filesystem service.

## Durable state and recovery

Retained evidence lives in the exact
`.metadata-transactions/<transaction-id-hex>/manifest.json` plus digest-named
`.blob` files. The manifest binds the plan, exact original bytes, predecessor
publication and existing parent identities. Publication proceeds:

1. Validate current authorization and the exact before snapshot; make all before
   and after blobs plus the immutable manifest durable.
2. Atomically publish and fsync a `pending` control, with a new generation and
   unique token, before the first selected source mutation.
3. Create only declared directories and atomically replace/remove only selected
   files. Before each move, every selected path must equal its exact before or
   after binding, and current authority/dependencies must still permit the plan.
4. Verify retained evidence and the complete selected terminal side. Fsync visible
   selected files and parent directories, including already-applied recovery
   writes, before atomically publishing the durable `ready` terminal state.
5. Write completion evidence only **after** ready. If the response/process is lost
   in this interval, the current ready head plus exact manifest still identifies
   the terminal transition. Before a later transaction supersedes that head, the
   engine makes its completion evidence durable.

Both commit and rollback finish with a fresh generation/token. Restoring the old
bytes does not restore the old token: a reader spanning before/partial/rollback
must reject that ABA interval. Control is retained permanently rather than cleared
or unlinked at terminal completion.

`read_pending_transaction(root)` is read-only and returns `{state, manifest, plan}`
with verified exact selected bytes **only** when the control currently selects
that manifest. It returns `None` for no pending transaction. It never adopts an
orphan directory as recovery authority. This route lets the adapter run current
scope/schema/dependency checks before its ordinary current-record inspection.

Resume and rollback are distinct explicit calls. They do not interpret timeout,
dead process, lock release, old timestamp or expired reservation as permission.
Any unknown third file state, missing/corrupt blob, changed parent, malformed
control or denied current guard stops with pending/evidence retained. Recovery
never patches an unknown editor's bytes to make a transaction appear complete.
Already committed transactions cannot be rolled back through this pending-only API.

When original delegation expired, an owner may explicitly issue new recovery
authority for the old exact scope. The adapter supplies that decision through its
current guard and optional `recovery_authorization` evidence. Original manifest
authority is never replaced. The new evidence is bound into both terminal control
and completion, surviving a crash immediately after ready. Supplying renewal
evidence alone does not authorize recovery or bypass the guard.

`inspect_transaction` distinguishes an unselected orphan, selected pending,
committed and rolled-back evidence. Historical completion does not certify today's
selected source bytes. Current-terminal replay verifies the selected terminal
bytes; replay after later transactions returns historical completion without
rewriting newer source and marks `current_selected_bytes_verified=False`. Current
authorization remains required even for a retry. A rolled-back ID cannot silently
become a new commit: a new owner command/ID is necessary.

Unreferenced manifest/blob/staging leftovers are not enumerated, automatically
executed or cleaned. An explicit retry may reuse the exact original retained plan
only after fresh authority, unchanged before publication/bytes and parent checks.
That authority comes from the caller's new invocation, not the orphan. Cleanup of
unneeded evidence remains an explicit owner action outside this library.

## Verification and boundary

The focused synthetic suite is
`mechanics/growth-cycle/tests/test_source_metadata_transactions.py`. It covers
exact selected movement with untouched descendants; scope/budget/alias refusal;
authority drift; actual child-process interruption around selected writes and
control publication; fresh-process rollforward/rollback; ready-before-completion
loss; preserved completion before a later head; rollback ABA; renewed recovery
evidence; corrupted/missing evidence; and refusal to overwrite a third state.

These are transport invariants, not source acceptance or a filesystem-wide
atomicity claim. Arbitrary raw readers can observe intermediate selected files.
Only participating readers using the full-operation publication guard reject that
interval. Power-loss durability relies on the filesystem's ordinary atomic rename
and fsync guarantees; process-interruption tests do not emulate hardware failure.
