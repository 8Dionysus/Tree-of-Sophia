# Cloudflare edge deployment

This directory is the permanent, repository-driven production profile for
`treeofsophia.com`. A Cloudflare Worker serves the checked-in web application,
uses Workers Static Assets for precomputed high-volume packets, and uses D1 for
bounded search and graph queries. The existing scale-export routes stream
CSV/JSONL from normalized D1 rows so the browser's download controls do not
depend on the former Python origin.

The edge is a generated read model. It does not own philosophical meaning,
review state, rights, or canon. `scripts/build_runtime.py` reads only the
standalone inputs already allowlisted by `Tree-of-Sophia`:

- `ToS/derived-exports/tos_corpus_index.min.json`
- `ToS/derived-exports/philosophy_graph_projection.min.json`
- `ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json`
- `ToS/doctrine/semantic-interchange/entity-types.v1.json`
- `ToS/doctrine/semantic-interchange/relation-types.v1.json`
- `ToS/derived-exports/epistemic_evidence_projection.min.json`
- `ToS/philosophy/graph-workbench/review-packets/table-i-post-planting-audit.json`

The public source-gap ledger is copied through its existing allowlist route.
The generated D1 revision binds the source inputs, actual per-item normalized
content revisions, capability data, and the explicit read-model schema
version. API, LensSpec grammar, catalog, documentation, and Worker-only code
changes do not force a large row import when the rows are unchanged. Any
producer change that alters normalized rows changes their content revisions; a
structural SQL change must increment the read-model schema version.

KAG is not a build input or runtime dependency. The edge build must not
regenerate or silently strengthen any KAG surface.

The source-bound Zarathustra word-analysis provider is also local-only. The
edge build records an explicit unavailable capability without importing that
provider or its private SQLite dependencies.

## Local verification

From this directory, install the Worker dependencies with `npm ci` and the web
dependencies with `npm run ci:web`. `npm run check:local` then rebuilds the web
assets and generated edge data, regenerates Cloudflare binding types,
type-checks the Worker, runs pure unit tests, imports the generated read model into a
local D1 instance, and compares representative HTTP packets against the Python
`ToSAccessCore` contract.

The backend-defined knowledge surface is stored as indexed normalized node and
relation rows. `/api/knowledge/catalog`, unified search and inspect routes,
the public `/api/knowledge/contracts` schema bundle, focused neighborhoods,
stored lenses, and arbitrary `tos_lens_spec_v1`
compilation use the same
display/provenance envelopes as local Python. D1 applies validated declarative
filters and bounded traversal; a Worker request never materializes the full
knowledge graph in memory. The lens `POST` is a structured read query and does
not create server state.

Execution v6 resolves node `property_id` selectors through the snapshot's
`knowledge_top.query_properties`, including path steps. SQL enforces the
property's declared type scope and missing-value semantics; pure TypeScript
and Python use the same contract. The read-model revision includes these
bindings so a code-only introduction of serving metadata cannot be skipped as
an API-only rebuild. Existing row data is not reinterpreted; the staged metadata
update remains revision-guarded. An older snapshot without a binding rejects
the selector until the matching read model is supplied. This is not automatic
deployment authorization.

Large lossless JSON fields are inserted in deterministic UTF-8 chunks only
when one statement would exceed the D1 statement ceiling, then reconstructed
in the staged row before table swap. Contract tests use isolated synthetic D1
databases, not a corpus import; `load:local` and the revision-aware sync are final
read-model checks rather than inner-loop steps.

Generated `dist/`, `runtime/`, local D1 state, and dependencies are ignored.
Only source, configuration, lockfiles, tests, and generated binding types are
tracked.

The edge build stages the Vite bundle under the shared `/static/assets/` URL
contract and fingerprints the HTML asset references before applying immutable
cache headers. This keeps the local HTTP and Worker paths aligned without
leaving browsers pinned to a stale fixed-name asset.

## Production flow

Cloudflare Workers Builds watches the repository's `main` branch with this
directory as its root. Its production build command is `npm run build:ci`, and
its production deploy command is `npm run deploy:edge`.

The deploy command first compares the generated `data_revision` with the live
D1 metadata. That digest covers the allowlisted inputs, normalized row content,
capability data, and the explicit read-model schema version. Unrelated
contract, catalog, documentation, and code-only rebuilds skip the large row
import when that revision is already current. A changed source, normalized
row, capability, or read-model schema revision imports the newly generated D1 read model before
deploying the Worker and static assets.
`npm run deploy:edge:plan` performs the read-only revision check without
importing or deploying, while `npm run load:remote` is the explicit recovery
command. Operators can set `TOS_D1_MAX_SYNC_STATEMENTS` when an environment
needs an additional write-size ceiling; paid D1 is not artificially capped by
the repository default.

Completed normalization steps are cached in ignored `runtime/normalization.sqlite`
using processor definitions, inputs and dependencies. `read-model.rows.json` records
complete row digests; `read-model.delta.sql` omits unchanged rows for a compatible
baseline. Delta staging is replayable and one revision-guarded statement publishes
all changed tables atomically. Incomplete staging or a stale baseline leaves
serving rows unchanged. Knowledge reads that cross publication return HTTP 409.

`load:local` uses the same revision-aware selection against local D1. Full SQL
bootstrap uses a streaming SQLite transaction when there is exactly one known
local D1 store, with serving revisions verified through Wrangler on both sides.
Run it with local dev servers stopped. `TOS_D1_LOCAL_SQLITE=0` disables this
development-only accelerator. It never targets remote databases. Local bootstrap
and large Wrangler imports share Python/SQLite statement framing: literal CR/LF,
whitespace, quotes and trigger-body semicolons are preserved, incomplete input is
rejected, and the producer's 100,000-byte SQL limit excludes its final record
separator. Other large imports prepare one bounded file at a time using a source
byte offset; the next file is not written until the caller has consumed the
previous file. Owned scratch is removed on completion, error or cancellation.
Python is therefore also required by this large-file deploy path. No giant
JavaScript string, new SQL serialization, key grammar or read-model schema is
introduced; already-generated multiline SQL remains compatible.
Input must remain the trusted, immutable producer file throughout the import.
The chunker rejects observed file replacement or metadata changes between reads;
framing alone is not SQL syntax/safety validation or a cryptographic integrity
check. Local execution retains SQLite's separate single-statement validation.

Remote full SQL
remains available for bootstrap/schema changes, but its sequential table swaps
are not atomic across tables: use a maintenance/bootstrap route for full recovery.
Normalization-step reuse alone does not skip source discovery, graph validation or SQL generation. Cache
files can be discarded; source history does not depend on them. See the
[publication rationale](../../../docs/decisions/TOS-D-0045-incremental-read-model-publication.md).

The offline normalization cache now executes an explicit dependency DAG:
source record / semantic type -> normalized node -> endpoint title -> relation.
Only output digests propagate downstream. A changed node whose title stays the
same does not force a new relation normalization. Each successful pure task is
committed independently; failed runs do not publish the active dependency index,
and a later run can reuse their completed tasks. Publication of that execution
index uses a baseline guard; it is not publication of the graph itself.

`manifest.processing` reports run status, executed/reused steps and retired task
IDs. The cache stores run/task/dependency metadata alongside completed outputs.
Those are disposable build records, not source lifecycle or review decisions.
The processor digest follows normalizers, transitive helpers, module constants,
cache/dependency helpers and Python major/minor version. Editing a lens query
body alone no longer invalidates all normalization steps. The new key format
requires one initial cache warm-up. Output and run-history retention are bounded
as described below; operators may discard the ignored cache when no longer useful.

This DAG currently covers normalized access projection, not source acquisition,
OCR, segmentation, translation, alignment or semantic review. Claim-trace and
view-membership indexing, global invariants, source-file reading and SQL row
comparison still run when their build stage is invalidated. See [TOS-D-0048](../../../docs/decisions/TOS-D-0048-incremental-normalization-dependencies.md).

### Incremental checks and cache retention

Final nodes bind their normalized record, Claim enrichment and inherited views.
Per-record semantic checks bind actual record bytes, the registry digest and
every referenced endpoint, Claim, evidence and exact-version review. Missing
references participate in the key so that later additions invalidate the check.
Duplicate IDs bypass per-ID reuse. Global identity, cardinality, registry and
index/aggregation passes still run; caching never bypasses a failed invariant.

`manifest.processing` includes `steps_by_kind`, bounded samples/counts in
`input_changes`, `input_coverage` and cache accounting. The offline Python reader
`processing_input_changes(db, run_id, after='', limit=100, source_only=True)`
returns ID-ordered pages (maximum 1,000 items), before/after digests and
`next_after`. Only completed scans report removals. Default filtering selects
source records and registry types; `source_only=False` includes internal inputs.
Run/baseline retirement raises `KeyError` rather than fabricating a delta.
This is execution metadata, not an HTTP/UI ABI or a source change ledger.

`NormalizationCache` accepts `max_cache_bytes`, `max_cache_entries`, `keep_runs`:
defaults are 1 GiB of serialized output payload, 500,000 outputs and the latest
three run receipts plus the active published receipt if older. Oldest-used
outputs are evicted independently of receipts; oversized outputs are computed
without caching. Every reused payload is checked against its stored digest;
legacy unverified or corrupt entries recompute. These are accidental-integrity
checks, not protection against a malicious cache writer.

The cache uses an exclusive Unix file lock; process exit releases it and a later
builder marks abandoned runs interrupted. Completed pure work remains reusable.
On exit, retired execution metadata is removed, the WAL is truncated and a
substantially empty SQLite file is compacted. These limits do not cap total DB
size, indexes, graph RAM, temporary compaction space or one run's task metadata.
A working set exceeding the output budget is correct but can repeatedly miss;
configure `build_runtime.py --cache-max-mib N --cache-max-entries N
--cache-keep-runs N` for offline workloads accordingly. These positive integer
options affect cache admission, not graph meaning or stage identity. Maintenance
runs only when the normalization cache opens, not on completed-stage reuse.

See [TOS-D-0050](../../../docs/decisions/TOS-D-0050-incremental-checks-bounded-cache.md).

### Resumable build stages

`build:data` stores disposable stage checkpoints in ignored
`runtime/build-stages.json`. SQL and static responses have separate inputs and
outputs. Reuse requires matching source, contract and producer byte digests,
Python version, path configuration, directory membership and output digests.
Missing/corrupt outputs or checkpoint metadata cause recomputation. File size
and mtime alone never admit reuse. The SQL stage also binds the deployed row
baseline; changing only `access/web/dist` invalidates static responses, not SQL.

`manifest.build_stages` reports `computed`/`reused` per stage. When both stages
are reused, graph construction, normalization, graph validation and SQL writing
are skipped. `processing.status=not-run` and zero current normalization counters
make that distinction explicit; `origin_run_id` refers to earlier processing.
Input and output bytes are still read for integrity, so this is not constant-time
source discovery. A static-only rebuild can still materialize the graph.

The offline builder uses a fail-fast Unix OS lock per runtime directory, released
on process exit. Use one output directory per runtime and serialize deployment
after successful build completion; concurrent external writers/deployments are
not supported. Inputs must remain quiescent while building: rechecks reject
detected changes but are not immutable filesystem snapshots. Keep the output
and runtime directories separate. CLI guards reject roots, home, source-tree
outputs and overlapping paths before the producer can replace the output.

Completion manifests are invalidated before rebuilding and written last via
atomic file replacement. A failed static stage leaves a completed SQL checkpoint
reusable on retry, but no deployable completion manifest. Checkpoints are not
source history, signatures, review or publication receipts; their checksums
detect accidental corruption, not a malicious cache writer. Removing a specific
stage entry forces that stage to rebuild. Full D1 bootstrap remains a separate
maintenance operation; this does not make it transactional.

The changed-corpus path still reads monolithic source files, assembles indexes
and runs global checks. Per-record checks and finalization are incremental;
streaming assembly and source acquisition scheduling remain outside this slice. See
[TOS-D-0049](../../../docs/decisions/TOS-D-0049-content-verified-build-stages.md).

Focus reads indexed adjacency per frontier instead of all relation headers.
Legacy source descent and Work/Link dossiers retain a bibliographic overview;
dense text-packet members are served by the knowledge routes, not bundled into
one growing static navigation file. The asset builder rejects oversized files.
Broad independent queries and global counts can still scan larger sets.
`profile=overview` excludes dense text-unit membership; `profile=all` expands it.
The catalog publishes the exact exclusions. LensSpec `detail=compact` keeps
identity, display and source routes, omits raw records and clears attributes in
the returned carrier; inspection by ID retrieves the complete record.

The Worker owns the apex custom domain and redirects `www.treeofsophia.com` to
the apex. `api`, `assets`, and `docs` remain unassigned for later bounded
profiles.

The web build also publishes the operator-facing AoA Social Connector service
information at `/apps/aoa-social-connector/`, with directly linked privacy and
terms pages. These static pages describe the external connector project and do
not become an authority for Tree of Sophia source, review, or canon.

No Cloudflare credential belongs in this repository. Local Wrangler OAuth and
Cloudflare's encrypted Workers Builds token are operational credentials owned
by Cloudflare and the operator account.

## Resumable exploration storage

`POST /api/knowledge/explore` continues an actual BFS frontier using shared D1
checkpoints. `/api/knowledge/explore/capabilities` reports readiness; the contracts
route returns the shared request/result schemas with target-specific capabilities.
The additive `migrations/0001-exploration.sql` installs checkpoint tables, a
publication clock and composite adjacency indexes. Deployment installs it on
full, delta and unchanged-data paths; no HTTP request executes DDL. The builder
adds small `knowledge_exploration_top` metadata. Its content version changes the
data revision without invalidating the compatible row-delta schema.

Each page reads indexed endpoint/ID ranges; it does not load the full graph or
the large `knowledge_top` packet. Primary reads, a publication epoch and guarded
atomic D1 batches protect retry and concurrent successor admission. Eight
simultaneous continuations are tested to return one page/next cursor. The epoch
also rejects a publication that changed away and back between page reads.
Full bootstrap still requires the documented maintenance boundary.

The D1 cache is disposable but survives Worker isolate restarts: 15-minute fixed
TTL, 128 records and 32 MiB total, at most 1 MiB per state or replay response.
Eviction and expiry delete only checkpoint rows, never knowledge tables. Cleanup
runs during admission; the storage bound holds even without a cleanup cron.
An oversized checkpoint returns 413 before admission. 409 means changed
publication, 410 means expired/evicted state, 503 means migration or metadata is
missing. Restart from focus or narrow the request as appropriate.

At most 24 adjacency queries and 512 graph work units run per page; bounded
metadata/admission queries are additional. D1 may pause earlier than Python.
Counts are discoveries, not global totals; an empty paused page may still advance
past excluded edges. Runtime-specific cursors cannot be transferred to local
HTTP/native MCP. Ordinary LensSpec delivery pagination remains stateless.

This is a query execution cache, not source history, a saved user workspace or a
corpus processing scheduler. See [TOS-D-0047](../../../docs/decisions/TOS-D-0047-shared-d1-exploration-checkpoints.md).

## Acceptance boundary

Green local checks prove source-to-edge contract parity for the sampled API
surface. Green GitHub checks prove the pushed revision builds. A successful
Workers Build proves deployment from that revision. Public acceptance requires
the apex health packet, HTML and static assets, representative corpus and
philosophy queries, and the `www` redirect to succeed from outside the origin
host.

The former Cloudflare Tunnel profile remains a temporary recovery and local
preview route only. It is not the production availability architecture because
it requires a continuously powered origin machine.
