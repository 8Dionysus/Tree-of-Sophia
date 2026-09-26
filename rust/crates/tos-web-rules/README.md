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
numeric kinds and retained source fields survive result emission.

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
every consulted carrier, and hold/recheck the current lease through complete
byte delivery. Cancellation is cooperative around platform awaits; the platform
reader owns cancellation within its I/O. Replay needs and finished bytes grant
no disclosure authority by themselves.

There is no production Worker binding or public route activation here. Existing
`temporal-comparison.ts`, `native-temporal-store.ts` and their live route retain
their current execution until an authentic selected publication/current-policy
provider and actual Worker parity are admitted. The new TS module is byte
transport; it does not retire those domain functions. Inspection, lens and
exploration selected native executors remain separate from this temporal seam.

The existing `tests/domain-wasm-host.mjs` exercises the real generated binding
and async driver with a maintained selected-packet oracle carrier, plus
cancellation, withdrawal, exact absence and terminal duplicate refusal. That
fixture intentionally yields unsupported temporal comparison, preserving the
fact that an unmapped node is not a temporal Claim. Full temporal source-role
parity requires its owner fixture evidence. `tests/domain-worker-host.mjs`
uses the same driver and generated binding in local workerd for unsupported
carrier retention, cooperative cancellation, withdrawal and exact absence.
Writing these harness cases does not establish their execution or public use.
