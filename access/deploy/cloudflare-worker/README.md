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
structural change to the base SQL model must increment its read-model schema
version. Optional read stores have independent versioned admission and never
silently become part of the base-model contract; see the bounded lens extension
below.

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
display/provenance envelopes as local Python. D1 narrows identities, dimensions
and incidence; the bounded native-v7 lens plan evaluates general predicates,
sort/count and traversal before limiting results. A Worker request never materializes the full
knowledge graph in memory. The lens `POST` is a structured read query and does
not create server state.

Execution v7 resolves node `property_id` selectors through the v9 snapshot's
`knowledge_lens_top.query_properties`, including path steps. Native references
retain raw JSON number kinds, unsafe integers and source member order through
matching, grouping, v7 fingerprints, pagination and the first wire serialization.
The read-model revision includes these
bindings so a code-only introduction of serving metadata cannot be skipped as
an API-only rebuild. Existing row data is not reinterpreted; the staged metadata
update remains revision-guarded. An older snapshot without a binding rejects
the selector until the matching read model is supplied. This is not automatic
deployment authorization.

The lens/focus route requires matching v9 publication metadata, row digests,
Unicode 16.0.0 and ordered indexes; older or damaged publication carriers fail
closed. A publication clock/revision guard rejects changes during the read.
Request/cursor JSON retains Python last-member-wins behavior while published
source rows reject duplicates. The public result remains plain LensResult JSON,
with a 16 MiB response ceiling. Generic scans, decoded bytes, callbacks, sorting,
cache and path work have explicit bounds, documented in
[`NATIVE_SEMANTICS.md`](../../shared/NATIVE_SEMANTICS.md).
Generic candidate scans explicitly use the published source indexes before
ordering IDs, avoiding repeated global identity-index walks for a small source
scope. Exact identity conjuncts keep their narrower identity indexes. Filters
still run against digest-verified native rows; budgets and packet semantics
are unchanged. This does not make broad non-indexed property filters cheap.

The native lens reader can also consume explicitly installed, versioned compact
seeds and exact `view_ids`/`graph_layers` membership indexes. Their offline owner
API and transaction rules are in
[`LOCAL_PREPARED_PUBLICATION.md`](../../LOCAL_PREPARED_PUBLICATION.md).
The extension binds the complete v9 publication header and epoch. Missing
optional stores retain the old bounded plan; stale or incompatible installed
stores return 503, and invalidation observed during a read returns 409. Native
numbers, source member order, human-form selection, full inspection and
uncovered-field predicates retain their existing authority and representation.
Positive membership groups use bounded indexed counts/keysets. SQL expansion
respects D1's [100-parameter statement limit](https://developers.cloudflare.com/d1/platform/limits/).

The addressed `prepared_delta_runtime.py` route maintains explicitly installed
stores in its forward/reverse transaction. Exact predecessor seeds, complete
selected memberships and current store bindings are checked before publication;
the successor is sealed only after base/search/metadata updates, using the actual
publication epoch. Rollback restores source-derived content with a fresh epoch.
Partial staging or a store invalidated since capture refuses atomically. The
existing capture, retention and SQL budgets include these added rows. Missing
stores keep the older base-only path; this route never installs tables.

Initial full-builder emission and the legacy full-producer delta route are still
unintegrated. Their changes invalidate old store bindings; a reader never repairs
or silently re-admits them. The bounded locally tested writer/reader seam is not
a deployed full-corpus growth capability.
Full SQL posting/statistics INSERTs are bounded both by encoded bytes and by
512 VALUES rows. A small encoded statement can still contain enough short rows
to exhaust D1's statement compiler; the row cap preserves every posting while
bounding that preparation pressure. This is not a whole-import memory budget.
Execution/response budget exhaustion returns 413 on lens compilation and
stored-lens/focus GET/HEAD, matching the published Python adapter. Invalid
compilation input remains 400; unavailable or damaged publication data is 503.
`/api/knowledge/catalog` and stored-lens selection read the atomically published
D1 `knowledge_catalog`, not the separately deployed static asset. The reader
verifies its digest and source revision against the selected publication header.
Stored-lens selection and execution share one outer revision/epoch guard,
including ABA refusal. Exact native numbers and source member order survive
catalog delivery and execution. Missing, damaged or mismatched metadata returns
503 without static fallback; the 8 MiB catalog ceiling returns 413. GET/HEAD
catalog responses are not cached across publications. No new D1 import or
schema migration is needed for this reader correction.
D1 metadata and selected row JSON are length/type
guarded inside SQL before text delivery; metadata is read one bounded chunk
at a time. Identity/order/header cells also have a 1 MiB SQL guard and a
cumulative remaining-byte guard. Per-request D1 delivery admission is serialized
so concurrent endpoint streams cannot each spend the same remaining allowance.
The shared clock/revision prefix keeps its single-statement snapshot and checks
revision metadata type/aggregate 1 KiB size before concatenation or delivery.
Indexed relation endpoints are checked against their authoritative
row headers before traversal, including queries returning zero relations.
D1 rows-read accounting is post-statement, not SQLite VM-step interruption.
The local synthetic differential tests cover native packets; they do not claim
full-corpus deployment or Cloudflare runtime acceptance.

For an explicitly selected offline bootstrap, the Python producer's
`build_read_model_sql` accepts `max_search_postings` (default 10,000,000)
and `emit_delta_baseline=False`. The caller must measure/admit storage and
resource demand before widening the posting budget. Full-only mode emits the
same complete SQL, including all search postings, but no delta SQL or row-index
companion; it requires fresh output paths without an existing/deployed baseline.
It is not a delta deployment input or a substitute for remote D1 capacity
admission. The ordinary build/CLI retains its existing defaults and companions.

### Addressed search publication preparation

The offline `incremental_runtime` module also exposes
`prepare_search_address_indexes_transaction` and
`plan_search_addresses_transaction`. These are explicit caller-transaction
helpers, not a serving fallback or a completed all-lane delta producer.
Preparation adds two optional publisher indexes over existing search documents
(exact identity and lower-ID tie group); it leaves serving rows, postings,
metadata and the v9 row schema unchanged. The caller must admit storage and
index-build resources separately. Default preparation permits 200,000 documents.
Wrong existing index definitions refuse rather than being silently replaced.

The planner takes the exact current D1 revision and complete desired successor
ID groups, ordered by the verified source publisher. Public search orders by
rank, lower-ID and only then position. Thus physical posting addresses need
not be dense or globally monotonic with source order: only equal lower-ID
groups require the source-relative tie order. Content-only changes and deletion
retain addresses; insertion or reordering relocates only that whole bounded
tie group above the current high water. Unrelated postings need no renumbering.
The caller must regenerate every moved member's complete postings, including
unchanged source rows in a moved group, in the same guarded publication.

Planning defaults to 64 groups, 512 predecessor/successor members and 1 MiB of
key material, with SQL-side cumulative masking before key delivery. It is
read-only and requires previously prepared exact indexes; no global document
or posting scan is substituted. It refuses stale revision, malformed or
incomplete predecessor membership, exhausted safe-integer address space and
oversized groups. A plan is not source closure evidence, persisted reservation,
admission or publication: source-order/group completeness, exact changed rows,
all metadata/search/lens lanes, current-revision CAS and replay remain the
responsibility of the joining publisher. Existing dense full-build/delta inputs
and the serving ABI remain supported. This preparation alone does not claim
successor D1 parity.

Delta publication deletes by driving primary-key lookups from the staged key
set and deleting the resulting rowids. It does not correlate each serving row
against the stage, which would scan unchanged posting tables for a tiny patch.
This uses the registered producer's ordinary rowid tables and retains null-safe
`IS` key matching. The revision guard, complete-stage check and single-statement
publication remain unchanged. Focused tests enforce both the indexed plan and
bounded SQLite VM work, alongside atomic replay and missing-stage refusal.
Selected search payload verification likewise drives `(kind, position)` seeks
from the small selected-identity list. Optional publisher identity indexes must
not cause SQLite to scan all documents of a kind while delivering one match.
Indexed search preflight and ranking also start from the already budgeted rare
gram posting set, then seek document addresses and exact row IDs. Source/type
filter indexes must not reverse that order into a whole-source document walk.

### Joining an exact prepared transition to D1

`scripts/prepared_delta_runtime.py::build_prepared_delta_sql` captures one
committed, source-paired bibliographic `delta-history` transition from two
caller-held prepared snapshots and the exact admitted D1 predecessor snapshot.
The caller supplies `expected_d1_revision`, `before_binding`, `after_binding`,
a fresh SQL `target`, and optionally a distinct fresh `rollback_target`.
All three connections must already hold read transactions. Publisher address
indexes must be explicitly prepared first. Capture does not commit, mutate D1,
invoke source commands, build a graph, switch consumers or deploy anything.

The initial full D1/prepared pairing is an independently verified caller
prerequisite, not established by a few matching rows. Capture additionally
checks reader/catalog/lens metadata and every affected predecessor row,
digest, order row, search document and expected posting through exact keys.
It does not globally scan for hidden corruption or recount all postings.
Nonparticipating source roots and dependencies must remain unchanged; only
source-catalog/bibliographic-claims and their explicit claim-publication
profile may move. An unrelated corpus, philosophy, evidence, capability or
schema migration must use its owning broader publication route. Source-pairing
verification does not establish live source currentness or semantic acceptance.

The complete changed rows and relocated case-tie members supply node/relation,
digest, lens-order, search-document, posting, affected posting-count and shared
metadata changes to the existing atomic staged publisher. No full row-index
baseline is emitted: its selected-row comparison index is internal only. This
also permits an explicitly admitted full-only bootstrap to receive an addressed
successor without constructing a whole-corpus row-index companion.

The target revision uses `tos_prepared_bibliographic_d1_delta_v1` lineage: exact
base D1 revision, before/after prepared bindings and source-input digests, and
the publisher implementation digest. This is distinct from the full producer's
content-revision algorithm; it is not claimed to have the same hash for the
same logical rows. Serving v9 semantics remain unchanged. A successful receipt
records this lineage, counts, byte costs and the still-unapplied state.

Default budgets are 512 changed rows, 100,000 old/new observed postings,
200,000 retained old/new rows, 4 MiB per selected row, 32 MiB per metadata value,
128 MiB cumulative reads, 128 MiB retained row bytes and 128 MiB total forward
plus optional reverse SQL. SQL-side masking precedes selected text delivery.
The caller separately admits runtime memory and storage; these byte counters
are not claims about Python heap size or remote D1 capacity.

The optional reverse package is built from the same exact selected predecessors
before returning success. It requires the successor D1 revision, restores the
old reader rows and metadata, and preserves new source records and prepared
history. Forward and reverse replay use the existing CAS and complete-stage
checks. Offline import remains an explicit caller operation with workerd
stopped; remote publication is separate authorization. Failed capture retains
diagnostic `.next` files; an interrupted pair rename may leave an unadmitted
complete artifact. Only a successful receipt admits both packages. Do not
reuse failed output paths or treat file existence alone as publication proof.

Lens and the other native published readers share a 64 KiB
`knowledge_reader_top` boundary. It must match Python compact emitted framing,
including numeric spelling and valid Unicode in every string and object key;
equivalent non-emitted JSON or escaped lone surrogates return 503. Source rows
are not rewritten. Lens source-size refusals are typed 413: 1 MiB rows/lens
metadata, 1 KiB digests, 128 KiB metadata chunks and at most 256 chunks, plus the
existing aggregate/execution/response bounds. Missing or malformed carriers
remain 503; publication ABA remains 409. v9-only lens admission, execution v7,
cursor ABI, traversal and the existing lens index-name policy are unchanged.

### Lossless search compatibility

Legacy v1 and indexed v2 search use the v8/v9 emitted reader header, native
selected-row digests and source references through the first HTTP serialization.
Unknown fields, large integers, float kinds, negative zero and source member
order are retained in full node/relation packets and authority metadata.
Selected search-carrier rank fields, document length and digest are verified
against Python semantics; no full graph/catalog or lens histogram is loaded.

Both modes use the existing producer's Python-lower ID/display ranking carrier,
including scalar and multilingual forms and source-position ties. Legacy keeps
exact counts/offsets and empty/short-query behavior; its explicit carrier-first
scan plus primary-key source lookups is still a global scan. Indexed keeps the
rarest 3-gram, 50000 candidate and 16000000 verification-character limits, and
now checks the bounded posting/document/stat closure before rank evaluation.
No request introduces DDL, rebuilds a carrier or falls back to a static graph.

Query stripping/lowercase and lengths use Python Unicode semantics. Legacy
checks 256 characters after strip; indexed checks raw and lowercased lengths
too. HTTP comma-separated filters retain literal whitespace, and numeric URL
parameters use whole Python-integer parsing with default/clamp behavior instead
of accepting JavaScript numeric prefixes. Indexed continuation retains exhausted
kinds. Its private D1 cursor v3
invalidates old v2 tokens with 409/restart; the public search schema remains v2,
not the separate local compressed-search v3 product.
Generated continuation tokens must fit the private 8 KiB decode limit or the
request returns 413 before emitting an unusable page.

The admission envelope is 1 MiB per row, 1 KiB per digest, 64 KiB per emitted
reader header, 16 MiB aggregate native delivery/response, and 16000000 selected
search-document verification characters. Selected IDs and source bodies are
masked in SQL before oversized text delivery and share one aggregate byte
allowance. Explicit size/work refusals return 413, damaged/missing carriers 503,
bad request inputs 400, and stale/crossed publication 409. Legacy global
count/selection work does not acquire the native payload rows-read quota.
This does not prove hidden global-index completeness, hard VM interruption,
or full-corpus runtime parity. Shared header/status alignment is described above.

### Lossless inspection compatibility

Node/relation inspection independently supports published v8/v9 rows. It uses
the common bounded SQL/digest reader and retains full `NativeRef` rows through
the first HTTP serialization. It does not execute a lens, read its histogram or
catalog, scan a whole graph, or reread selected full rows. Existing packet
schemas, exact/entity/native resolution precedence, code-point ID order,
relation_limit 0..1000 (default 200), exact counts and the shared publication
clock/ABA guard remain intact. V9 required migration indices are checked without
loading the lens ordered carrier.

The following are explicit compatibility corrections to the older D1
inspection implementation, matching the authoritative published Python reader:

- Aggregate `source_refs` includes only nonempty strings, without coercion.
- Missing relation endpoints and duplicate source JSON members return 503.
- More than 128 alias matches return 413 before selected bodies are loaded.
- IDs contain 1..4096 Unicode code points after Python Unicode stripping
  (including U+0085/U+001C, excluding U+FEFF), otherwise 400.
- The small reader header must use declared Python compact emitted framing;
  equivalent but non-emitted whitespace/escape/float spelling returns 503.
  Source row bytes are never rewritten by this check.
- Explicit row/digest/header and aggregate delivery/response budgets return
  413. Inspection uses 1 MiB rows, 1 KiB digests, 64 KiB headers, 128 KiB metadata
  chunks, 16 MiB aggregate delivery/response, and at most 4096 delivered rows.

Unavailable/damaged publication data remains 503; absent IDs are 404;
publication changes during successful packet construction are 409. HEAD has
the same admission/status checks and no body. A relation packet returns all
matched full relations and up to 256 unique full endpoints, or refuses; it
never silently drops an endpoint. The common emitted-header guard and typed
lens source-size statuses are described above; inspection remains independent.

The D1 statement/rows-read budgets are not Python SQLite VM interruption or
hard-limit emulation. A tested valid row of 1048577 bytes yields 413 on both
readers; a 2 MiB row hits Python's earlier SQLite hard limit (503) while D1's
explicit pre-delivery size refusal is 413. Neither route returns the row.
The exact status distinction and remaining lossless-route gaps must not be
reported as complete local/D1 runtime equivalence.

Large lossless JSON fields are inserted in deterministic UTF-8 chunks only
when one statement would exceed the D1 statement ceiling, then reconstructed
in the staged row before table swap. Contract tests use isolated synthetic D1
databases, not a corpus import; `load:local` and the revision-aware sync are final
read-model checks rather than inner-loop steps.

Temporal comparison retains the old historical role and the separately
declared `catalogue-assigned-document-date` role. The latter requires the exact
Document subject, current source profile, full Claim/value hashes and literal
identity. A bounded `semantics.claim.source_canonical_json` companion preserves
source canonical bytes (262144 UTF-8 bytes maximum); a missing or inconsistent
companion refuses comparison as `undetermined`. The temporal D1 adapter retains
native references for complete Claim, value and normalized-time carriers through
the first HTTP serialization. Large integers, float kinds, negative zero,
unknown fields and source object-member order are not round-tripped through
plain JavaScript packets. Escaped JSON member names retain decoded identity;
duplicate source names are damaged publication (503), not an alternate binding.

Source-binding JSON equality distinguishes booleans from numbers while comparing
integer/float values exactly. Documentary hashes use Python's sorted canonical
JSON and numeric spelling, separately from unchanged returned source carriers:
equivalent float spellings do not invent a different source digest. The Claim's
source line must have integer JSON kind; a positive unsafe integer is not rounded
or rejected merely because it exceeds JavaScript's safe integer range. Source
references use Python code-point ordering, and accepted request selection member
order is retained. This changes no temporal request/result schema or comparison
role.
Full inspection retains the companion, while compact carriers omit only this
field from Claim semantics. Original metadata and
its roles remain unchanged. Unknown calendars stay unknown; two different
otherwise-comparable roles are `unsupported`.

This route uses exact indexed node identities only: at most six logical lookups
for a documentary pair (the two Document-subject checks included), versus four
for a historical pair. Each unique payload is fetched once; header, index,
identity and emitted-digest queries are additional bounded SQL work. There is
no alias fallback, inspection execution, lens histogram scan or full graph load.
The existing v8/v9 published header/index admission and pre/post publication
clock guard apply, including A -> B -> A refusal. SQL masks oversized payload,
identity and metadata text before delivery. The 1 MiB source-row and 16 MiB
delivery/response ceilings return 413; invalid source/header/digest is 503,
unknown selected Claim is 404, and stale selection/publication is 409. Structural
source limits remain depth 64 and 300000 values per row; the native response
writer also has depth/visit budgets. D1's 200000 rows-read guard is not Python's
SQLite VM-step interrupt and does not prove an identical exhaustion domain.

Tests compare complete raw Worker responses against the current
`PublishedKnowledgeReadModel.temporal_compare` on the same SQLite publication,
including every ordered key, number kind and float representation. A real
Miniflare D1 HTTP/ABA smoke complements the synchronous SQLite facade. These
checks do not import the corpus or deploy the Worker. A row-changing normalizer
update requires the matching read-model rebuild under the normal publication
route; old document rows without the companion fail closed until then.

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
capability data, the published catalog, and the explicit read-model schema
version. Unrelated documentation and Worker-only code rebuilds skip the large
row import when that revision is already current. A changed source, normalized
row, capability, catalog, or read-model schema revision imports the newly generated D1 read model before
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
JavaScript string is introduced, and the bounded statement framing and key
grammar remain stable. Producer read-model v9 retains the v8 search posting and
gram-stat rows as bounded multi-row `_next` inserts so the incremental recorder
can stage them, and publishes the cold-reader metadata/catalog plus exact
emitted-JSON digests for each knowledge node and relation through the same
chunked `edge_meta` transaction. The corresponding schema version invalidates
older row baselines. Already-generated multiline SQL remains compatible with
the local bootstrap parser.
V9 additionally publishes checksum-bound `knowledge_lens_top` dimensional count
metadata and `knowledge_lens_order` native-order/incidence carriers. They are
derived from the same normalized rows and staged/swapped with those rows, not
maintained by request-time writes. Python Unicode version and native execution
version are explicit metadata, and the data revision includes these bytes.
V8-to-v9 requires a normal full schema bootstrap; subsequent v9 row deltas
include changed/deleted order carriers and changed histogram metadata in the
existing atomic compare-and-swap publication. This schema change does not
activate a remote deployment or give D1 the Python native lens executor.
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
Legacy source descent and bibliographic-carrier/Link dossiers retain a
bibliographic overview;
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
checkpoints. `/api/knowledge/explore/capabilities` reports migration/metadata
presence, not row-integrity or full publication validation; the contracts
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

Exploration now uses the published v8/v9 header/index boundary and emitted-row
digests before compact projection. Arbitrary retained source values stay native
through the **first** response serialization, including unsafe integers, integer
versus float JSON kinds, negative zero and source member order. The final page
is serialized once before the checkpoint batch; first delivery, persisted replay
and concurrent CAS-winner delivery return identical JSON text. The compact
field omissions and scene semantics remain unchanged. Traversal state holds only
structural query/identity/counter data, validated before its private clone.

Public execution remains `tos-exploration-d1-execution-v6` with the existing
v1/v2 schemas. The private checkpoint version is now
`tos-exploration-d1-execution-v6/native-json-v1/selected-relation-first-v1`:
old potentially rounded v6 records and pre-fix scene continuations/replays
return 409 and require a fresh exploration. Selected raw relations now remain
explicit even with coincident Claim focus, matching native scene selection.
This private invalidation changes no
source rows or table schema and performs no request-time migration. Checkpoint
and metadata text is bounded before D1 delivers it; split header reads bracket
the publication clock, including replay. All cache batch writes, cleanup and
eviction included, are conditional on the unchanged epoch.

The D1 cache is disposable but survives Worker isolate restarts: 15-minute fixed
TTL, 128 records and 32 MiB total, at most 1 MiB per state or replay response.
Eviction and expiry delete only checkpoint rows, never knowledge tables. Cleanup
runs during admission; the storage bound holds even without a cleanup cron.
An oversized checkpoint returns 413 before admission. 409 means changed
publication or incompatible private cache version, 410 means expired/evicted state, 503 means migration or metadata is
missing. Restart from focus or narrow the request as appropriate.
Invalid emitted-row digests, inconsistent index identity or damaged checkpoint
state also return 503. The existing 1 MiB source-row, 1 KiB digest, 64 KiB header
and 16 MiB request-delivery budgets apply. Ordinary traversal still excludes
edges with absent endpoints; mandatory origin and returned-page closure refuse
missing rows. This does not establish completeness of an arbitrarily corrupted
index. Shared header/status alignment does not change exploration scheduling.

The same additive publication-clock migration is a readiness prerequisite for
all D1 knowledge reads (lenses, legacy and indexed search, node/relation
packets, and temporal comparison). Each read performs one statement before and
after its work that validates the singleton clock and the complete contiguous
`data_revision` chunk set. A missing or malformed clock/revision returns 503;
an epoch or digest change returns 409, including an A->B->A publication whose
final digest is unchanged. Apply `migrations/0001-exploration.sql` before
serving these routes; no request performs DDL. Indexed search cursors use
`tos_knowledge_search_indexed_cursor_v3`, which carries the publication epoch and
requires native-search normalization. Recognized v2 cursors require a fresh
search (409); unrecognized/malformed cursor schemas are invalid input (400).
A cursor from another publication epoch must restart the search (409).

At most 24 adjacency queries and 512 graph work units run per page; bounded
metadata/admission queries are additional. D1 may pause earlier than Python.
Counts are discoveries, not global totals; an empty paused page may still advance
past excluded edges. Runtime-specific cursors cannot be transferred to local
HTTP/native MCP. Ordinary LensSpec delivery pagination remains stateless.

The native tests compare full-stream selected carriers against the actual
`PublishedExplorationService` on the same tiny SQLite file and compare each
page's scene against Python. They preserve original raw numbers through actual
Worker HTTP and real D1 restart/concurrent replay. Page boundaries, work units,
snapshot hashes and tokens remain runtime-specific. The D1 1 MiB replay ceiling
can reject a page accepted by Python's larger cache, and D1's post-statement
rows-read guard is not SQLite VM interruption. This is not a whole-corpus or
universal local/D1 parity claim.

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
