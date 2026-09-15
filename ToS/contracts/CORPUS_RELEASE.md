# Corpus admission and data releases

Software, accepted source revisions, compiled reader snapshots, and downstream
exports have independent identities under TOS-D-0062. A software commit does not
invalidate accepted source bytes or require their recompilation.

## Source transaction

`scripts/corpus_store.py` stores exact SHA-256 objects, immutable revision
manifests and an atomically replaced local current/previous pointer outside the
software checkout. The caller supplies an exact base revision and one complete
batch of byte-bound updates and explicit retirement events. A failed batch may
leave unreferenced immutable objects; it cannot advance the accepted pointer.

Each revision binds its exact file membership, byte sizes, modes, validator and
schema identity, stable identity index, incoming dependency index, base revision
and retirement history. Deleting a source path needs an exact event file path
and SHA-256. That file must exist in the complete candidate, including an update
in the same batch, and cannot itself be removed by that batch. Each retirement
preserves the retired source digest and the event's path, digest and byte size.
Historical verification reads both immutable objects even if the event path is
retired later. An unresolved string cannot stand in for an event. Byte binding
does not establish the event's meaning or authorization; source-owner validation
and review remain responsible for those judgments.
Earlier objects and revisions remain addressable. Retired IDs cannot silently
be reassigned to a different source path. An explicit identity/path migration
requires a source-owner contract change; the transport does not infer one.

Admission computes the transitive incoming closure of changed paths, validates
the candidate, verifies its bytes again and compares the current base under an
exclusive lock before promotion. Competing batches must re-admit against the
new base. Unchanged inputs with unchanged validator identity keep their existing
revision. Different schema/validator identity requires full source validation.

The validator receives a `CorpusCandidate` with exact manifest membership and
the accepted base's indexes. `read_bytes(path)` verifies the selected object's
SHA-256. `materialize(paths)` gives existing path-based tools private copies of
an explicitly selected closure. Accessed objects, materialized inputs and changed
members are verified again before promotion; unrelated accepted objects are not
copied or rehashed by the transaction itself. This is not a new whole-store
health assertion: full `load(..., verify_objects=True)` and restore still check
all selected revision bytes. The current source adapter still requests the full
view until its owner checks are scoped; the storage API alone does not complete
that source-validation transition.

### Source retirement event

The source adapter applies the existing `provenance-event.schema.json` contract
through `scripts/corpus_source_retirement.py`. An event is a JSON record under
`ToS/source-witnesses/retirements/`, has a `tos.event.*` ID, `event_type: migration`,
and a completed status. Its method is `corpus-source-retirement`, version `1`.
The method configuration contains exactly:

- `base_revision`: the exact accepted predecessor;
- `retirements`: path-sorted `{path, sha256}` objects, exactly matching the
  source batch targets bound to this event;
- `reason`: a nonempty source-visible explanation;
- `review_ref` and `review_sha256`: an exact, nonempty retained review under
  `ToS/review-ledger/`.

Provenance inputs are the same ordered target bindings with role `retired_source`,
followed by the exact review binding with role `source_owner_review`. The one
output names the retained event itself with role `corpus_retirement_event`;
its enclosing corpus manifest binds the event's digest without a circular
self-digest. `receipt_refs` returns to the same review. Missing, changed or
misbound review/event bytes, a different base, failed operation or different
target set reject the whole batch. Earlier source and event bytes remain in
history. The source owner reviews the proposed membership change and its actual
authority; the validator checks those bindings and operation shape, never
interprets an arbitrary review string as approval or grants rights or canon status.

The command is `scripts/corpus_admit.py`: it requires an exact canonical batch, input root, grammar root and store root. The batch binds every update digest, size and mode; source files cannot select validation code.

The program-owned adapter is `scripts/corpus_source_validation.py`. It uses the
existing source foundation, source profile and catalog validators on an isolated
view; it does not import code from the selected corpus. The first implementation
retains a conservative full source audit within this data operation. Dependency
indexes support affected-source diagnosis and future narrowing; they do not
justify skipping an existing invariant. This audit is absent from software CI.
Catalogs produced for validation remain disposable data, outside authored source
membership. Authored Markdown route cards inside generated directories remain
source. Existing exact provenance may refer to earlier derived bytes: an explicit
verified historical captures and paired restored roots can supply those bytes as retained
validation evidence. Its complete selected membership is bound into the validation
identity; it cannot replace an admitted source member or become a new public export.
Historical surrounding ADRs and other referenced repository files are evidence
inputs too. Repeated `--historical-capture` and `--historical-root` arguments pair
in order; overlapping packs are rejected. Pack order and local storage paths do
not affect identity. Only the validator's actual transitive local program imports,
runtime dependencies and schemas contribute to its program identity.
The snapshot validation context checks exact input membership instead of asking
Git whether the materialized view is tracked or ignored. The legacy Git-backed
inspection route keeps its own tracking checks. No mechanical admission changes semantic, review, rights or canon
status. The relevant owner judgment remains explicit in the unchanged sources.

## Compiled data

`scripts/corpus_build_worker.py` compiles an accepted revision in a dedicated
process and disposable view. Catalogs, philosophy projections, bibliographic
graph, corpus projection and Evidence Lens are produced by their existing owner
builders. Source discovery uses the admitted manifest rather than Git population.
The data packager at `access/packaging/build_data_snapshot.py` binds only the
exact runtime allowlist and its verified partitioned closures.

A data snapshot contains no executable program or browser build. Its manifest
binds the corpus revision, projection inputs, actual query compiler identity,
query schema/version, complete file membership and every output digest. Its
identity excludes unrelated software Git commits and wall-clock timestamps.
The reader must verify compatibility before selecting the data. Incomplete or
corrupted outputs cannot replace a previously verified release.

`build_data_snapshot.py build --reuse-snapshot PATH` verifies a previous snapshot
completely before considering its compiled query store. Reuse requires identical
compiler bytes, ABI and exact compiler input bindings. Different valid inputs
are compiled normally; a corrupt cache is rejected. Cache location, Git HEAD and
wall-clock time never participate in the data identity.

## Custody and historical preservation

The operator selected permanent local storage plus permitted private Cloudflare
R2 copies. Payload custody remains governed by its existing exact File bindings,
rights reviews and transfer receipts. This contract does not relocate, delete,
publish or grant additional processing rights for those bytes.

Public source metadata and its private backup are evaluated as their own layer.
A payload's `local_only` posture is not a prohibition on preserving its public
bibliographic identity or text-free provenance metadata; see the layer rule in
`ToS/doctrine/CORPUS_FOUNDATION.md`. Conversely, payload availability or bucket
access does not authorize copying protected source-bearing annotations.

A complete local historical capture may preserve exact tracked Git bytes without
claiming new admission. A private metadata/evidence backup explicitly excludes
all `payload` path components, the reserved `owner-local` namespace, and
`ToS/derived-exports/lexical-search/`. Protected owner-context stores and private
operational credentials/receipts outside Git are outside this transfer. Local
excluded bytes remain preserved and appear in the exact disposition manifest;
exclusion from a remote copy is never reported as complete remote preservation.

`scripts/corpus_archive.py` records exact Git commit/tree, member Git blob IDs,
SHA-256, sizes and modes. Restore verifies the archive and complete manifest
before and during extraction. `scripts/corpus_r2.py` transports bounded chunks
using existing private R2 custody infrastructure. Existing objects and every new
upload are freshly read back; the complete manifest is published last. A failed
or unavailable remote copy remains incomplete. It does not block software work
or authorize deletion of the local source.

Historical Git refs and corpus object locators retain their exact byte identity.
A source-return locator never substitutes a mutable `latest` path for a recorded
commit, revision, object or provenance input. Git history is preserved; removal
of current bulk tracking requires independent restoration evidence and the
source-owner transition, not merely a green archive command.

## Selection, rollback and withdrawal

`access/packaging/release_pair.py` prepares a pair from a clean verified software
archive and a complete data snapshot. It reads the archive reader ABI without
executing archive code. The pair binds both artifact digests, data and corpus
revisions, manifest digest and reader compatibility. Local absolute storage
bindings are separate from the portable pair identity.

Promotion compares the exact expected current pair and publishes only after
verification. The previous verified compatible pair is the rollback target.
Immutable withdrawal records live outside this pointer; data, corpus or software
withdrawal cannot be undone by rollback. A long artifact verification holds no
release-state lock; promotion rechecks withdrawals under the pointer lock.

Set `TOS_RELEASE_ROOT` to the local release state when serving a managed pair.
The installed reader verifies the selected data before serving. It checks
withdrawal before producing a packet and again before returning it, and rejects
changed or undeclared data members. An external `TOS_QUERY_STORE_PATH` cannot
substitute another database for the selected snapshot. Program assets and API
schemas remain software-owned. Selecting a pair never installs or deploys code.

Private R2 transport and local release-state selection do not activate public
site, Worker or D1 delivery. Those deployments remain a separate operator action.
