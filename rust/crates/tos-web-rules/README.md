# Browser and Worker rules

`tos-web-rules` provides Rust rules through native and optional WASM bindings.
DOM, Fetch, storage, platform I/O and WebMCP registration remain host duties.
These private bindings are not source selection, rights, review or canon.

## Private temporal continuation

`TemporalSession` / WASM `TemporalReplaySession` run the same
`tos_query::compare_temporal_operands` used by selected native temporal access.
The Rust core validates the original request, selects exact Claim/value/subject
lookups and computes the result. The host never parses carrier JSON or derives
semantic lookup IDs. Published carriers use the FND strict codec; original
numeric kinds and retained source fields survive result emission. Final bytes
use the native selected finalizer's `SourceRecordDigestV1` canonical profile.

The caller supplies its selected source revision, the source graph registered
for the bibliographic Claim adapter, raw request bytes and positive admission
caps. An outstanding need must be answered once with full verified exact
carrier bytes, or verified exact absence. Identity mismatch, malformed carriers,
out-of-order/duplicate responses and budget exhaustion terminate the session.

The current temporal core reads at most Claim, temporal value and documentary
subject for each of two operands. The continuation allows six distinct IDs and
seven executions. It caps retained source bytes, the sum of request plus retained
input bytes reconsidered across replays, and emitted packet bytes. `JsonLimits`
bound each parsing/canonical operation. `work()` and WASM getters report replay
executions, retained carrier bytes, replayed input bytes and exact lookups.
These are separate from physical D1 queries, scanned rows and fetched bytes.
Aggregate canonical bytes and actual CPU instructions are not measured by the
shared temporal API; no precise aggregate CPU accounting is claimed.

[`selected-temporal-runtime.ts`](../../../access/deploy/cloudflare-worker/src/selected-temporal-runtime.ts)
owns only the async host loop and disposal. The supplied selected access owner
must verify membership, digest, exact uniqueness, visibility, physical admission
and the unchanged model/policy scope around every read. Its disclosure callback
must bind `tos.knowledge.temporal.compare`,
`read_only_public_knowledge_temporal_compare_v1`, the selected model receipt and
every consulted carrier, and hold/recheck the current lease through private
capture. `captureSelectedTemporal` refuses a callback's returned `Response`:
response construction finishes before Worker platform body consumption/enqueue.
Captured values are not public-delivery admission. This private selected profile
remains distinct from the maintained published D1 snapshot route.
Cancellation is cooperative around platform awaits; the platform
reader owns cancellation within its I/O. Replay needs and finished bytes grant
no disclosure authority by themselves.

The maintained Worker temporal POST imports the generated module directly and
uses `respondTemporalSnapshot` over the existing verified D1 publication reader.
The publisher/import selects public data; this path does not require or invent
the native selected profile's disclosure issuer. It verifies the supported
publication header/indexes and exact emitted row digests/identity, then passes
raw retained bytes to Rust. Epoch/data_revision checks surround reads and run
again immediately before whole-body enqueue/close. A demand-driven stream with
zero high-water mark emits no bytes during Response construction; cancellation,
request abort and snapshot failure discard the buffered packet. This is a
publication consistency check before body handoff, not a transaction held
through remote network flush.

The optional fifth WASM constructor argument `published_output: true` selects
FND insertion-ordered Python compact emission without a final LF. It changes
serialization only. Absent/false retains native canonical emission and the
accepted private capture ABI. Physical D1 admission stays bounded by the
existing reader; replay retains at most six 1 MiB rows, seven executions and a
16 MiB packet, with the existing JSON depth/visit/integer limits. Replay byte
admission follows those explicit roles and caps, not claimed CPU measurement.
Published core canonical work uses the declared 16 MiB output cap separately
from the 1 MiB request/carrier parser cap: Python float spelling can expand a
valid retained row. The common temporal core still decides its 262 KiB exact
source-binding limit and emits its undetermined reason. The private canonical
profile retains its existing limits.

Root accepted local published default-route parity on corrected products:
104 existing oracle controls passed, including actual Miniflare, and the four
unchanged body controls were retained. The exclusively replaced Worker TS
algorithm/store and dead entry are removed; independent host presentation and
browser temporal controls stay. The shared request-shape validator is additionally
exposed by `validate_temporal_request_wasm_v1(raw, admission)` before any D1 access,
using the same bounded parser and budget decoder as the replay session. Comparison
uses that same QRY validator and matches actual publication revision later.
The changed pre-I/O boundary and migrated unique controls passed their narrow
actual check; prior full oracle/body evidence is retained without an unchanged
rerun. Local acceptance does not establish a deployment.

## Full published inspection continuation

WASM `InspectionSession` drives the same `tos_query::InspectPlan` used by the
selected native node/relation consumer. `validate_inspect_request_wasm_v1`
checks the original bounded JSON request before any D1 access. Concrete needs
are lookup with exact/entity/native selector and admitted complete match count,
node incident count/selection, or exact relation endpoints. Each batch resumes
once; execution never replays or loads a complete graph.

The published host authenticates full retained rows and enforces its 1 MiB
per-row byte cap before parsing. Raw JSON array envelopes preserve each original
carrier substring; FND strict parsing and the plan retain exact numeric kinds,
source member order and full unknown fields. Actual envelope bytes charge the
plan's 16 MiB selected input cap. The shared core validates producer phase,
matching identity, ordered complete sets, incident count and endpoint closure,
then constructs full packets and source targets. FND's existing insertion-order
Python compact writer emits the bounded result. Physical SQL admission and
logical value visits have separate caps; neither claims aggregate CPU accounting.

The actual Worker GET/HEAD routes use the mandatory build-owned generated
module and the existing published snapshot reader. They reuse the accepted
demand-driven whole-packet Response lifecycle: selection checks surround needs
and precede final enqueue/close; abort/cancel discard bytes, and HEAD runs full
admission before an empty response. No native current-policy issuer is added.
The existing inspection Python/Miniflare harness and unique source-target
controls are routed through that real consumer. OPS built the actual generated
product; typecheck, 13 existing inspection/CSV, two overflow and two readable
context controls passed, including real Miniflare. The replaced exclusive TS
inspection algorithm and source-target projection are removed. The stronger
native consumer's independent fixture/parity status remains separate. Temporal
greens were retained without repeating them for this new family; no deployment
or general WASM coverage is claimed.

## Full published lens/focus/stored continuation

`LensSession` drives the same concrete `tos_query::LensPlan` used by selected
native lens execution. `validate_lens_request_wasm_v1` checks bounded original
compile/focus/stored input before D1 access; verified metadata later binds
properties. The published software-v7 vocabulary and native descriptor law
remain explicit separate profiles. The compiler retains one continuation and
one outstanding concrete storage need; no replay or generic provider is added.

Needs preserve existing indexed source/identity keysets, dimensional and
membership counts, covered compact reads, ordered incidence, alias/source and
exact payload closure. Rust owns selectors, recursive bounded path witnesses,
traversal, sorting/grouping/presentation/fingerprint/cursor and packet assembly.
The host authenticates original lexical rows and physical index/header framing.
Per-row source size, physical D1 costs, logical work and final emission caps are
separate. Rust's sole parsed cache and suspended/current row references are
lexically bounded; transient host/UTF8/WASM copies are not an RSS claim.

The actual default POST compile and GET/HEAD focus/stored routes consume the
mandatory static product and existing verified publication/catalog/auxiliary
seams. They reuse the accepted snapshot Response lifecycle and cooperative
cancellation. FND emits Python compact insertion order and native numeric kinds.
Matched products/typecheck and23 existing lens plus3 affected knowledge controls
passed, including real Miniflare compilation/pagination. The exclusive TS lens
algorithm and dead entries are removed; genuinely used exploration presentation,
JSON/transport and browser preview helpers remain. Native independent parity,
deployment, scale and remaining portable families are separate acceptance gates.

The existing `tests/domain-wasm-host.mjs` exercises the real generated binding
and async driver with a maintained selected-packet oracle carrier, plus
cancellation, withdrawal, exact absence and terminal duplicate refusal. That
fixture intentionally yields unsupported temporal comparison, preserving the
fact that an unmapped node is not a temporal Claim. Full temporal source-role
parity uses an optional native capture. `tests/domain-worker-host.mjs`
uses the same driver and generated binding in local workerd for unsupported
carrier retention, cooperative cancellation, withdrawal, exact absence and
returned-Response refusal. Its test-only `Response` is constructed after private
capture and lease release. This tests packet transport through workerd, without
claiming the lease holds through actual Worker body delivery.
Writing these harness cases does not establish their execution or public use.

The Node domain harness accepts a third argument and the workerd domain harness
a fifth argument: an absolute path to `temporal_transport.json` captured by the
existing QRY selected native test. The capture contains selected source revision
and Claim source graph, `{id, raw}` full carrier strings and `{request, packet}`
strings from actual native temporal execution. Host code preserves those raw
strings and asserts identical privately captured packet bytes; it neither rebuilds Claims nor
round-trips carrier bodies through JS objects. The harness reports
`genuine_temporal_cases: 0` when no capture was supplied, and
`temporal_public_delivery: false` regardless of fixture parity.

The existing workerd harness supports `--temporal-body-only` to exercise deferred
whole-packet consumption, body cancellation, request abort and changed-snapshot
refusal without repeating historical private captures. The maintained Worker
`test/native-temporal.test.mjs` exercises the actual default route against the
existing published Python oracle, including retained number kinds/member order,
publication damage and ABA. No new test framework is introduced.
