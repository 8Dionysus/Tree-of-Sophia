# Cloudflare edge deployment

This directory is the permanent, repository-driven production profile for
`treeofsophia.com`. A Cloudflare Worker serves the checked-in web application,
uses Workers Static Assets for precomputed high-volume packets, and uses D1 for
bounded search and graph queries. The existing scale-export routes stream
CSV/JSONL from normalized D1 rows so the browser's download controls do not
depend on the former Python origin.

The edge is a generated read model. It does not own philosophical meaning,
review state, rights, or canon. The offline Rust `build:data` producer reads
only the standalone inputs already allowlisted by `Tree-of-Sophia`:

- `ToS/derived-exports/tos_corpus_index.min.json`
- `ToS/derived-exports/philosophy_graph_projection.min.json`
- `ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json`
- `ToS/doctrine/semantic-interchange/entity-types.v1.json`
- `ToS/doctrine/semantic-interchange/relation-types.v1.json`
- `ToS/derived-exports/epistemic_evidence_projection.min.json`
- `ToS/philosophy/graph-workbench/review-packets/table-i-post-planting-audit.json`

The public source-gap ledger is copied through its existing allowlist route.
The original `scripts/build_runtime.py` remains an independent Python oracle
until the full native output has been compared and accepted. It is not invoked
by `npm run build:data`.

The maintained `deploy_edge.mjs` entry invokes installed `tos` for bounded
`edge-sql-stream` framing and `edge-import-local` offline SQLite bootstrap.
Set `TOS_ACCESS_BIN` to an explicit native executable when it is outside PATH.
Import never launches Cargo. Node owns Wrangler process invocation and the
explicit local-store selection; Rust owns SQLite statement completeness,
byte-exact chunking, statement/value budgets, baseline and target revision
checks, and the one-transaction import/rollback. Stop the local Worker before
bootstrap. Remote imports continue through Wrangler and require their own
publication authority.

For chunked imports, set a positive whole operation
`TOS_D1_SQL_STREAM_MAX_SECONDS`; it includes the time the consumer holds each
chunk while Wrangler imports it. The native child reads each chunk through
a clone of the original source descriptor, produces it only after the consumer
requests it, and checks descriptor and pathname currentness before and after
reading. EOF cancels the reader. Node uses one five-second monotonic cleanup
deadline for EOF, SIGTERM, SIGKILL and exact-child close, separately from the
native operation deadline. It removes its temporary district only after close.
If close cannot be observed, it reports the unreaped PID and retains that exact
directory. Deadline expiry never unlinks a handed-off chunk under a consumer. Direct
`edge-sql-stream` callers supply an empty directory and `--max-seconds N` and
own its cleanup after the child exits. `edge-sql-chunk` remains a bounded,
single-chunk tool for independent parity checks.

`scripts/sql_stream.py` and `scripts/import_local_sqlite.py` remain independent
parity oracles and are no longer called by the maintained deployment entry.
The private prepared-pair, catchup and source-navigation Python capture APIs
remain retained runtime paths: the public builder/import cutover does not
replace their owner-selected snapshots or grant selected-D1 admission.

`npm run build:data` invokes installed `tos build-data` (or the explicit
`TOS_ACCESS_BIN`) and requires a positive
whole-build `TOS_BUILD_MAX_SECONDS` environment variable. Direct CLI callers
may override it with `--max-build-seconds N`; the CLI refuses a missing,
invalid, or overflowing deadline before locking the runtime directory or
changing completion markers. For example, from this directory, set an
operation-specific deadline and run `TOS_BUILD_MAX_SECONDS=3600 npm run build:data`.
The value is a caller decision, not a maintained default or a
promise that a particular corpus fits. The Rust route always computes the full
disposable v9 SQL, row baseline, and static outputs. It does not install a
native-current read model or grant publication authority. Its SQLite page and
work limits are per component and do not form an aggregate host-disk quota.
Prepare the native software product through the separate release route before
running this profile; the maintained build/import entries never invoke Cargo.
The explicit deadline begins in native `build-data` and
covers the producer, not Cargo compilation. An invalid CLI deadline refuses
before output lock, directory creation, or completion-marker changes.
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

## Shared Rust temporal product

The maintained `/api/knowledge/temporal/compare` POST executes the shared Rust
temporal core over digest-verified exact D1 carriers. The v1 published contract
selects the `source-claims` profile. The publisher/import owns public-data
selection; publication metadata is not a native current-policy grant.
The shared Rust request-shape validator runs before any D1 access, preserving
400 for malformed requests even when the read model is unavailable. The same
bounded parser/admission and validator are reused by comparison; actual source
revision matching follows publication selection. Epoch/data_revision checks surround operand reads and run again immediately
before the demand-driven whole-body enqueue. Cancellation, request abort or a
changed snapshot discards the pending packet. Final enqueue/close is body
handoff, not remote network flush; a change after Response headers were returned
fails its body rather than replacing those headers with another HTTP status.

Builds require the unmodified `wasm-bindgen --target web --out-name tos_web_rules`
products from `rust/crates/tos-web-rules`: `generated/tos_web_rules.js`, its
`.d.ts`, `tos_web_rules_bg.wasm` and its declarations. The build owner admits and
verifies those exact products outside the checkout and supplies the generated
directory handoff. Generated bytes are ignored outputs, not authored source;
the ignore rule provides no storage admission. Static imports and type checking
fail when the mandatory product is absent. There is no runtime fetch or TS
fallback. The predecessor Worker TS temporal algorithm/store and unused entry
are removed; host/D1 transport, browser display controls and independent Python
oracle remain. `initSync` owns module initialization, with no second host cache.
Existing build/deploy authorization and data-release procedures remain separate.

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
and incidence; the shared Rust lens plan evaluates general predicates,
sort/count and traversal before limiting results. A Worker request never materializes the full
knowledge graph in memory. The lens `POST` is a structured read query and does
not create server state.

Execution v7 resolves node `property_id` selectors through the v9 snapshot's
`knowledge_lens_top.query_properties`, including path steps. FND values
retain raw JSON number kinds, unsafe integers and source member order through
matching, grouping, v7 fingerprints, pagination and the first wire serialization.
The read-model revision includes these
bindings so a code-only introduction of serving metadata cannot be skipped as
an API-only rebuild. Existing row data is not reinterpreted; the staged metadata
update remains revision-guarded. An older snapshot without a binding rejects
the selector until the matching read model is supplied. This is not automatic
deployment authorization.

Compile, focus and stored-open use the mandatory generated WASM `LensSession`
and shared pre-D1 request validation. Concrete needs resume once; the native
consumer uses the same Rust plan. Host code retains indexed SQL, original row
bytes and publication/digest admission. The exclusively replaced TS lens
algorithm is removed; shared exploration carrier and browser preview helpers
remain. A demand-driven Response checks the selected epoch/revision before
whole-body enqueue/close; abort/cancel discard bytes and HEAD admits the same
packet without a body. Cooperative cancellation surrounds synchronous WASM;
there is no in-WASM interruption or remote-flush claim.

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
optional stores retain the bounded full-row path; stale or incompatible installed
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

Ordinary capture can hold predecessor and successor WAL read transactions on
the **same prepared file**; it does not require a full file copy. Keep the old
read transaction open through the committed source transition and SQL capture,
then close it so WAL pages can be reclaimed.

For a lagging local D1 reader after those old read transactions have closed,
`build_prepared_catchup_sql` is a separate, explicitly selected offline
reconciliation route. The caller admits the exact D1/source-input predecessor
and current prepared successor and holds both SQLite read transactions. It
streams their digest manifests (default combined limit 400,000 rows), checks
native-row coverage and framing, and retains at most the declared changed-row
budget. Equal row bytes with changed search tie ordering are also included.
It needs no second prepared database and does not normalize or import a full
graph. This manifest scan is not an addressed per-edit latency claim or an
automatic fallback when ordinary delta capture fails.

Predecessor source revision, reader/catalog/lens binding, unchanged normalizer
and nonparticipating source scopes remain required. Changed native navigation,
search postings and address ties, row digests, lens and auxiliary stores use
the same forward/reverse capture and atomic publication guards as the ordinary
delta. Missing/orphan/malformed digest entries, incompatible profiles, stale
stores and exceeded scan/retention/SQL budgets refuse before final output.
The receipt distinguishes manifest reconciliation from an exact one-parent
transition and reports rows scanned. Only the successor prepared/source pairing
is mechanically verified on this route. Predecessor source admission is an
external prerequisite: matching its source revision does not authenticate the
caller-supplied roots, dependencies or publication token. The aggregate and
predecessor pairing flags are therefore false, while successor verification
and external predecessor admission are separately explicit. Ordinary delta
capture still verifies both selected prepared bindings. It does not fabricate an old prepared
binding, grant source admission, apply SQL or authorize deployment. Digest
comparison relies on the admitted producer manifests; it is not a complete
forensic rehash of every stored JSON body against a malicious database owner.

The full SQL producer now emits both stores from the exact normalized source
rows and seals them against the actual bootstrap epoch. Its row-index companion
includes a versioned auxiliary publication descriptor. Subsequent full-producer
deltas maintain both stores atomically, using the same predecessor guards and
successor seals as the addressed writer. A historical descriptor-less baseline
returns `auxiliary_migration=\"lens-auxiliary-initial-migration-required\"` and
`delta.available=false`: the complete SQL is an explicit initial migration
candidate, not an applicable incremental update. Malformed or mismatched
descriptors refuse without replacing final output files. Full bootstrap remains
a maintenance operation with sequential swaps, not live atomic publication.
The producer bounds encoded auxiliary row values to 1 GiB and membership rows
to 2,000,000 by default (`max_lens_auxiliary_bytes`, `max_lens_memberships`).
These are offline output budgets, not storage reservations or whole-import RAM
limits. Existing row budgets remain in force. Readers never install, repair or
silently re-admit stores; this does not establish full-corpus deployment.
Full SQL posting/statistics INSERTs are bounded both by encoded bytes and by
512 VALUES rows. A small encoded statement can still contain enough short rows
to exhaust D1's statement compiler; the row cap preserves every posting while
bounding that preparation pressure. This is not a whole-import memory budget.
Delta staging uses the same byte/row bounds for independent data and key
INSERTs. It keeps at most two pending statement buffers, flushes before chunk
UPDATEs or publication/control statements, and never changes the per-source-row
digest or baseline shape. Changed and removed keys remain exact; partial-stage
refusal, replay and the single publication trigger are unchanged. These bounds
limit encoded pending SQL, not total producer RAM. Measure staging separately
from the atomic commit: grouping SQL removes per-row statement overhead without
claiming that a full-producer rebuild is an addressed source-processing path.

Published Python/D1 lens cursors additionally bind the complete admitted
publication snapshot, including data revision, epoch and emitted-header digest.
The v7 result fingerprint itself stays content-scoped. A same-snapshot runtime
restart preserves continuation; a changed publication or ABA rollback returns
409 even when its selected rows are identical. Existing unbound published lens
cursors require a fresh first page. This changes opaque cursor identity, not
source records, human forms, non-paginated packets or graph-only continuation.
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
committed, source-paired `delta-history` transition from two
caller-held prepared snapshots and the exact admitted D1 predecessor snapshot.
The caller supplies `expected_d1_revision`, `before_binding`, `after_binding`,
a fresh SQL `target`, and optionally a distinct fresh `rollback_target`.
All three connections must already hold read transactions. Publisher address
indexes must be explicitly prepared first. Capture does not commit, mutate D1,
invoke source commands, build a graph, switch consumers or deploy anything.
The prepared connections may retain before/after read snapshots of **one WAL
file** around the source publisher's committed write. A second complete
prepared database is not required. Establish the old read snapshot before the
write and release it promptly after capture, so retained WAL frames can be
reclaimed; the normal writer/source guards still apply.

The initial full D1/prepared pairing is an independently verified caller
prerequisite, not established by a few matching rows. Capture additionally
checks reader/catalog/lens metadata and every affected predecessor row,
digest, order row, search document and expected posting through exact keys.
It does not globally scan for hidden corruption or recount all postings.
Nonparticipating source roots and dependencies must remain unchanged; only
source-catalog/bibliographic-claims/source-navigation and their explicit Claim
or initial-metadata publication profiles may move. An unrelated corpus, philosophy, evidence, capability or
schema migration must use its owning broader publication route. Source-pairing
verification does not establish live source currentness or semantic acceptance.

The complete changed rows and relocated case-tie members supply node/relation,
digest, lens-order, search-document, posting, affected posting-count and shared
metadata changes to the existing atomic staged publisher. No full row-index
baseline is emitted: its selected-row comparison index is internal only. This
also permits an explicitly admitted full-only bootstrap to receive an addressed
successor without constructing a whole-corpus row-index companion.

Changed source-navigation roots use the shared bounded immutable-root diff.
Only changed parts are opened; this does not newly admit unchanged closure.
For an installed native navigation product, exact predecessor nodes, edges,
rights and overflow payloads are compared against the admitted raw source;
the full producer's shared row kernel emits their replacements. Counts and
all changed rows join the same revision-guarded publication and reverse SQL.
JSON field order is not source identity, but the exact predecessor
serialization is retained for reversal. Native queries order by stable IDs:
the unused legacy `ord` column retains its predecessor value, or zero for an
insertion, instead of renumbering unrelated rows. Full-build positional `ord`
is not asserted as addressed-publication parity.

A producer-declared unavailable native product (`source_navigation_top={}`
and all six native tables empty) remains unavailable. Metadata growth does not
create an incomplete native dossier. A populated product with an unsupported
header, a partial unavailable product, mismatched raw counts, changed header
policy, corrupted selected row or exhausted diff budget refuses the whole
capture. Native product bootstrap remains a separate complete-source route.
The receipt distinguishes `maintained`, `unchanged` and `unavailable`; it does
not equate knowledge-query availability with native source-dossier readiness.

The target revision uses `tos_prepared_source_d1_delta_v2` lineage: exact
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

### Initial native navigation without rebuilding knowledge

`scripts/source_navigation_bootstrap_runtime.py::build_source_navigation_bootstrap_sql`
fills only an explicitly absent native product in an already admitted
D1/prepared pair. It reads the complete immutable `nodes`/`edges` projection
bound by the prepared source vector, not a nearby corpus export with different
provenance. It also requires an independently admitted immutable rights
projection, exact expected/trusted SHA-256, and a fresh mandatory reverse SQL
target. Both databases remain in caller-held read transactions during capture.

The rights projection uses logical schema `tos_source_navigation_rights_v1`,
one `rights` collection keyed and ordered by `rights_id`, and a
`navigation_header` containing the full producer's original
`tos_source_navigation_v1` header. Its counts must exactly describe the two
retained collections and the complete rights collection. The source owner must
verify the rights input inventory, bytes and projection before admitting this
snapshot; passing a digest alone is not a rights assessment. Additional input
provenance may be retained in its header and is covered by that digest.

Capture verifies every selected part and emits native rows with the same row
kernel as the full producer. Knowledge bodies, search postings and prepared
rows do not change. The D1 revision and reader/auxiliary bindings advance in
the existing atomic publication trigger; the lineage binds the old D1
revision, prepared/source pair, navigation and rights roots, and executable
implementation. The trigger rechecks that all six native tables are empty,
so a product inserted after capture cannot be overwritten. Replay and reverse
use the ordinary revision guards. Native `ord` is zero; consumers order by
stable identity as in addressed maintenance.

This is an explicitly bounded **initial full-product** scan, not an addressed
update and not a second full D1 import. The caller supplies projection read
budgets plus the existing D1 retained-row/SQL budgets and host reservation.
Subsequent source changes use `build_prepared_delta_sql`. Neither operation
admits source meaning, establishes global currentness, activates consumers or
grants deployment permission. Unknown/restricted rights remain unchanged.

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
When matching postings exceed the text-verification budget, indexed search
verifies an ordered candidate prefix per request. Rank metadata is charged
inside the same character budget before JSON evaluation; native text is joined
only after the prefix is materialized. A continuation may advance beyond a
verified prefix containing no full-string matches. Consumers must continue
from `page.next_cursor`, not interpret an empty page as exhaustion. The cursor
retains the same global rank/ID/position and snapshot boundaries. An individual
document or rank-metadata set that cannot fit still returns 413; caps are not
raised. `work.rank_chars` and `work.verified_chars` describe bounded logical
character spans, not total repeated SQL character visits or physical disk IO.
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

Node/relation inspection independently supports published v8/v9 rows. The
maintained GET/HEAD routes use the shared Rust `InspectPlan` through the same
mandatory generated WASM product. Request validation precedes any D1 access.
The common bounded SQL/digest reader passes original full row bytes to Rust;
alias resolution, endpoint closure, counts, source refs and exact source targets
belong to that shared plan. It does not execute a lens, read its histogram or
catalog, scan a whole graph, or reread selected full rows. Existing packet
schemas, exact/entity/native resolution precedence, code-point ID order,
relation_limit 0..1000 (default 200), exact counts and the shared publication
clock/ABA guard remain intact. V9 required migration indices are checked without
loading the lens ordered carrier.

The host executes only concrete lookup, incident and endpoint needs. Lookup
lookahead refuses an oversized alias set before payload loading. Each selected
full row is read once; no complete graph is loaded. Raw batch envelopes retain
integer/float kinds, source object order and original carrier lexemes. Rust emits
the insertion-ordered Python compact packet under the existing 16 MiB response
ceiling. The shared snapshot response driver checks epoch/data_revision again
before whole-body enqueue/close, handles cancel/abort, and computes the same
bounded packet for HEAD before returning an empty body. This is optimistic
publication consistency before body handoff, not remote network flush or a
native current-policy grant. The inspection D1 reader checks request abort
before and after each queued SQL operation.

Physical D1 limits remain separate from Rust logical work admission: the shared
plan caps accumulated supplied JSON value visits at 200,000; each FND batch parse
has depth 64, 300,000 visits and 4300 integer digits. Aggregate raw batch input
and output are capped at 16 MiB. These bounds are not CPU instruction accounting.
The inspection product/typecheck and existing affected actual route controls
passed: 13 inspection/CSV including real Miniflare, two overflow and two readable
context cases. The exclusively replaced TS inspection algorithm and source-target
projection are removed. Shared publication/header/SQL host transport remains.
These local checks establish their bounded scope, not deployment or every WASM
family; temporal's prior accepted evidence is retained without a repeated run.

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

The Rust `build:data` caller always prepares the complete SQL and row baseline.
It attempts a delta when a bounded, compatible v9 deployed row baseline
(or, if absent, the current row baseline) has the exact auxiliary publication
descriptor. An absent or incompatible predecessor, or an unrepresentable delta
key, leaves `counts.delta` null
and retires any older local delta file; `deploy_edge.mjs` chooses the delta
only when the live D1 revision equals its declared base. The build manifest
binds measured public input labels, lengths, digests, ledger membership and
partition-part closure for the local verifier. The Python producer remains an
independent oracle until the Rust route passes the complete consumer check.

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
Fresh full bootstraps use the posting table's composite primary-key index for
gram lookup and ordered positions; they no longer build a second identical
`(kind,n,gram,position)` index. Row bytes, posting membership, query order and
v9 delta compatibility are unchanged. Existing databases are not altered by
this producer correction; any removal of their redundant index and physical
space reclamation is a separate, explicitly admitted maintenance operation.
This removes duplicate storage, not the full-corpus D1 capacity admission gate.
The full producer explicitly closes acquired SQL streams and disposable row
index/baseline connections on success, budget refusal and Python cancellation,
even while an exception traceback remains retained. Failed `.next` SQL remains
diagnostic and is not marked finished or published. This is resource cleanup,
not resumable SQL generation or recovery from process termination/power loss;
the existing grouped final-file publication and rollback boundary is unchanged.
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

The independent Python oracle `scripts/build_runtime.py` stores disposable stage checkpoints in ignored
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

The offline builders use a fail-fast Unix OS lock per runtime directory, released
on process exit. Use one output directory per runtime and serialize deployment
after successful build completion; concurrent external writers/deployments are
not supported. Inputs must remain quiescent while building: rechecks reject
detected changes but are not immutable filesystem snapshots. Keep the output
and runtime directories separate. CLI guards reject roots, home, source-tree
outputs and overlapping paths before the producer can replace the output.

Completion manifests are invalidated before rebuilding and written last via
atomic file replacement. In the Python oracle, a failed static stage leaves a completed SQL checkpoint
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

## Native offline prepared-pair capture

The native access source exposes `PREFIX/bin/tos edge-offline-capture
--request ABS.json` as a bounded offline SQL-capture entry. The exact request
schema is `tos_edge_offline_capture_request_v1`; it has no defaults for resource
limits. All selected SQLite inputs and output paths are absolute. Input files
must be regular, non-symlink files whose held SQLite identity matches the
selected path. Pair operations hold D1 and prepared transactions; integrity
operations hold the D1 transaction and explicit navigation/rights snapshots.
Immediately before SQL emission, each held SQLite descriptor, identity guard
and selected path are checked against the same file identity. This check does
not establish a live selection lease. The selected output paths (`forward_sql`, optional `rollback_sql`, and
`manifest_json`) must be distinct and fresh. SQL and manifest targets may have
different exclusively owned parents. Delta and catch-up may omit rollback;
bootstrap and integrity require it. Capture only returns unapplied artifacts; it does not mutate D1,
switch a consumer or publish to Cloudflare.

The transition/bootstrap request has exactly `schema`, `operation`,
`d1_database`, `before_prepared_database`, `after_prepared_database`,
`expected_d1_revision`, `before_binding`, `after_binding`,
`before_source_inputs_json`, `rights_root`, `forward_sql`, `rollback_sql`,
`manifest_json` and `limits`. It does not receive source-maintenance catalog
inputs: the held prepared database supplies its persisted source state and
reader/catalog/lens metadata, checked against the exact caller binding.
`source-navigation-integrity` instead requires exactly `schema`, `operation`,
`d1_database`, `expected_d1_revision`, `expected_source_revision`,
`navigation_root`, `rights_root`, `header_only`, `forward_sql`, `rollback_sql`,
`manifest_json` and `limits`. Each integrity root has `expected_sha256`,
`namespace_path` and `root_json`; that digest must match the exact root bytes.
`header_only` is a required boolean only on the integrity route.

The request file is bounded by 10 MiB before decoding. Each selected prepared
binding is bounded by the existing 1 MiB source-state law. Catch-up source-input
JSON is at most 1 MiB before outer-string escaping; each retained projection
root is at most 256 KiB. Strict JSON preflight rejects duplicate members and
bounds depth at 128, visits at 6,500,000, integer digits at 4,300 and logical
parser state at 2 GiB before constructing the request tree. The preflight tree
is released before the serde request tree is built. This is not an allocator
or RSS guarantee; native capture JSON conversion also has the visit limit.

`limits.prepared` contains the nine positive `PreparedD1DeltaLimits` fields:
`max_changes`, `max_row_bytes`, `max_metadata_bytes`, `max_read_bytes`,
`max_rows`, `max_retained_bytes`, `max_sql_bytes`, `max_postings` and
`max_manifest_rows`. Existing API defaults include a 4 MiB row-read ceiling and
32 MiB metadata ceiling. Emitted SQL retains the independent 2,000,000-byte
SQL-literal row boundary, including quote escaping and fixed framing.
Bootstrap and integrity also accept `limits.projection`, containing the ten
nonnegative `MutationLimits` fields: `max_changes`, `max_input_bytes`,
`max_opened_parts`, `max_stored_read_bytes`, `max_decoded_bytes`, `max_keys`,
`max_written_parts`, `max_written_decoded_bytes`, `max_written_stored_bytes`
and `max_result_bytes`. Read dimensions accrue across the complete operation;
zero permits no work in that dimension. These routes do not stage COW parts or
a projection delta, so COW-only dimensions are validated without charging
nonexistent writes. Transition routes derive their addressed projection budget
from prepared limits and do not take a caller `projection` or `pair` object.
Unknown or missing request fields refuse before capture artifacts are created.

`operation` selects one of five source routes:

- `prepared-delta` and `source-navigation-delta` require the held predecessor
  and successor prepared snapshots plus their exact bindings.
- `prepared-catchup` omits the predecessor prepared file and binding;
  it requires the exact predecessor `before_source_inputs_json` and the held
  successor prepared snapshot.
- `source-navigation-bootstrap` omits all predecessor prepared/source fields
  and requires exactly the native `nodes`/`edges` root plus a separate
  `rights_root` containing only `rights`; collection key/order fields must
  match their source identities. The rights root must carry the complete
  `navigation_header` with schema, authority boundary and counts. Its request
  object contains `namespace_path`, exact `root_json`, and
  explicit matching `expected_sha256`/`trusted_sha256` values. These digests
  check bytes and do not assess or admit rights.
- `source-navigation-integrity` requires the held D1 source/revision, explicit
  complete navigation `nodes`/`edges` and separate `rights` snapshots, and the
  existing native navigation product. Full mode refuses any existing row or
  header digest companion, compares every native base row and payload with the
  source snapshots, checks table counts for orphan rows, and emits per-row plus
  header checksums. With `header_only: true`, it compares the complete header
  policy and counts and requires the existing row-digest inventory count to
  match those counts. It deliberately does not read source rows, inspect row
  digest contents, or repeat their audit; the result reports
  `verified_source_rows: {}`. Both modes emit only checksum and reader-revision
  metadata, leaving navigation rows unchanged.
  Native capture additionally rejects a multipart row-digest companion inventory.
  A complete `tos_source_navigation_v1` root header must equal the persisted
  header; an Agent row root instead requires its exact minimal schema-only
  header, as specified by the selected-Agent source contract. These checks
  close shared gaps in the retained Python oracle and are explicit native
  refusal boundaries, without reading row companions or granting authority.

Every operation requires `expected_d1_revision` to match the held D1
predecessor. Bootstrap additionally requires the native navigation product to
be wholly absent. Transitions reuse the maintained D1 row, knowledge, Lens and
metadata producers, including auxiliary-store guards, exact changed-row
predecessors, search posting counts and reversible row closure. The command's
stdout is a `tos_edge_offline_capture_result_v1` envelope containing the
selected `operation` and its actual `receipt`. The receipt schemas are
`tos_edge_native_prepared_delta_receipt_v1`,
`tos_edge_native_prepared_catchup_receipt_v1`,
`tos_edge_native_source_navigation_delta_receipt_v1`,
`tos_edge_native_source_navigation_bootstrap_receipt_v1` and
`tos_edge_native_source_navigation_integrity_receipt_v1` (both integrity modes).
Each receipt retains source/revision bindings, actual artifact hashes and bytes,
native accounting, auxiliary-store facts and its explicit lineage schema.
Transition address plans identify actual before/after positions and high-water
marks; `changed_ids` describes changed addresses. These versioned native fields
do not relabel Python recorder metrics or promise the old Python receipt shape.
The receipt reports held-snapshot binding checks, while
`selected_pair_owner_admitted`, `source_currentness_verified`,
`rights_admission`, `semantic_acceptance`, `d1_applied` and
`consumer_switched` remain false. Offline source-input equality is not a live
selection or owner lease.

The Rust emitter binds target revisions to its private `d1.rs` lineage and
included Rust implementation bytes. Integrity lineage identifies its mode,
source revision, navigation/rights root digests and native implementation
digests. That identity is distinct from the retained Python modules'
`execution_profile()` digest and lineage packet; equal logical rows do not
imply equal revision or audit metadata bytes. Native/Python fixture comparison
must therefore retain and report complete metadata and audit-row differences
rather than normalizing them away.

Example invocation:

```sh
/absolute/prefix/bin/tos edge-offline-capture --request /absolute/scratch/request.json
```

For an addressed delta, `request.json` has this shape; each binding
must be the complete value read from the selected prepared snapshots, and the
revision must be read from the held D1 predecessor:

```json
{
  "schema": "tos_edge_offline_capture_request_v1",
  "operation": "prepared-delta",
  "d1_database": "/absolute/scratch/selected-d1.sqlite",
  "before_prepared_database": "/absolute/scratch/before.sqlite",
  "after_prepared_database": "/absolute/scratch/after.sqlite",
  "expected_d1_revision": "<64 lowercase hex characters>",
  "before_binding": { "exact": "selected predecessor binding object" },
  "after_binding": { "exact": "selected successor binding object" },
  "before_source_inputs_json": null,
  "rights_root": null,
  "forward_sql": "/absolute/scratch/capture/forward.sql",
  "rollback_sql": "/absolute/scratch/capture/rollback.sql",
  "manifest_json": "/absolute/scratch/capture/manifest.json",
  "limits": {
    "prepared": {
      "max_changes": 10000,
      "max_row_bytes": 2097152,
      "max_metadata_bytes": 2097152,
      "max_read_bytes": 67108864,
      "max_rows": 10000,
      "max_retained_bytes": 33554432,
      "max_sql_bytes": 67108864,
      "max_postings": 100000,
      "max_manifest_rows": 10000
    }
  }
}
```

`prepared.max_sql_bytes` bounds the combined forward/reverse SQL bytes. With
no reverse target it bounds forward SQL alone; no hidden reverse artifact is
created. The `exact` binding values above are explanatory placeholders, not an
accepted binding schema. Keep each selected output parent exclusively owned
through the invocation. Output targets and the legacy `.next` names must be
fresh and distinct. Inode checks protect held output custody; they are not an
interprocess lock or a retained directory identity check. The caller owns each
parent exclusively; newly created empty parent directories may remain after a
failed capture.

The returning Python APIs live in `scripts/prepared_delta_runtime.py` and
`scripts/source_navigation_bootstrap_runtime.py`. Their four public functions
select the Rust capture operation through an explicit `native_capture` keyword.
Supply a `tos_access.native_edge_capture.NativeCaptureContext` containing an
absolute installed `prefix`, an exclusively owned existing `scratch` directory,
one original absolute monotonic `deadline`, a cumulative `max_snapshot_bytes`
budget and a per-stream `max_stream_bytes` budget. `max_snapshot_bytes` bounds
actual encoded typed frames, including descriptors and hashes. Optional
`max_schema_allocation_bytes` separately bounds cumulative schema metadata
allocation before copies; its default derives from the frame allowance, and a
caller may select a smaller explicit computational budget. These are transport
resource selections, not source admission or measured RSS. The caller must also
account for two private stream files of at most `max_stream_bytes + 1` bytes
each and at most 16 KiB of diagnostic metadata in the same owned scratch.
Retained failure scopes need explicit owner disposition or an additional
coexistence budget before retries. Missing context refuses;
there is no automatic software discovery or Python fallback. Algorithmic limits
keep their existing defaults. Delta and catch-up still permit
`rollback_target=None`; bootstrap and integrity require a reverse target.

These functions are host source adapters, not members of the standard native
software archive. Keep the exact Python source package and script district
available to the caller. The bridge runs the existing installed-prefix verifier
in an owned Linux child, then executes its verified ELF. The fresh isolated
child arms `PR_SET_PDEATHSIG(SIGKILL)` and checks its exact expected caller PID
before verifier dispatch; the request-v2 native entry rearms and checks it before
request parsing. This preserves caller signal handlers and threaded use. Abrupt
caller death does not execute Python cleanup: the whole owner supervisor must
still terminate/reap descendants and retain or dispose scratch under its accepted
lifetime contract. Cooperative cleanup keeps the unreaped leader through final
group signals, then performs bounded reap. The request-v2 bridge reads
exact typed rows through each borrowed SQLite connection, including its selected
uncommitted view. It carries raw TEXT bytes with validated UTF-8/UTF-16le/UTF-16be
encoding and storage-class tags, preserves table presence and row order, and
carries schema evidence without executing source DDL. Only native owner-known
schemas are imported; unknown ordinary tables remain inert evidence. Virtual or
shadow tables cannot be reconstructed by this finite transport. The bridge does
not serialize a stale memdb backing buffer, reopen the current database pathname,
or modify the caller transaction. Its independently checked `snapshot_transport`
inventory binds wire custody, not physical page identity or source currentness.
The physical-file request-v1 CLI route remains separate. The bridge returns the
actual native operation receipt and removes its exclusive transport directory
after successful validation and bounded child cleanup. Failures after the private scope is created retain that scope and whatever
request, typed frames and capped private stdout/stderr were materialized. Fixed-size private metadata records phase,
status, counts, SHA and EOF; exception text contains no native payload, while
its notes reference the owned evidence directory. Evidence metadata may be incomplete
if its original deadline or write bound prevents completion; no crash durability
is claimed. An unreleased child/group also retains its selected input directory
under the whole owner supervisor's custody contract. WAL and dirty-view interpretation, default limits,
optional rollback and complete receipts require the corresponding installed
consumer checks; a source candidate or direct CLI run does not establish them.

The retained reference functions are `build_prepared_delta_sql_oracle`,
`build_prepared_catchup_sql_oracle`,
`build_source_navigation_bootstrap_sql_oracle` and
`build_source_navigation_integrity_sql_oracle`. Their bodies remain independent
Python implementations, and fixtures select these names explicitly. The
internal `scripts/source_navigation_delta_runtime.py` composition remains an
independent reference helper. No Python oracle retirement, accepted API cutover,
owner-selected D1 pair, global currentness or publication authority is claimed
by this source candidate. Review the complete six-mode differential and real
returning callers before changing that disposition.
