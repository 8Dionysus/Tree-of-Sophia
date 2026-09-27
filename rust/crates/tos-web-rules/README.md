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

Existing TS domain functions remain pending retirement until actual maintained
published Worker parity is accepted. Inspection/lens/exploration selected native
executors remain separate. Source wiring does not establish a deployment.

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
