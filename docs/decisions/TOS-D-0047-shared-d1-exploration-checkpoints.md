# Shared D1 exploration checkpoints

## Index Metadata

- Decision ID: TOS-D-0047
- Original date: 2026-09-05
- Surface classes: access/backend, access/deployment, docs/architecture
- ToS layers: derived-exports, access
- Tree classes: constructor backend, read model
- Guard families: source-first authority, snapshot consistency
- Posture: accepted

## Context and decision

The process-local implementation in [TOS-D-0046](TOS-D-0046-resumable-neighborhood-checkpoints.md)
cannot retain a frontier across Worker isolates. This decision supersedes only
its deferral of shared storage: the Worker keeps disposable, TTL-bound query
checkpoints in separate D1 tables. It does not write entities or relations.
Local HTTP/native MCP retain their process-local behavior. Cursors are not
portable between these services; clients discover capabilities on the target.

Store immutable input checkpoints and replay responses. A conditional update
elects one winner for concurrent continuations. One D1 batch publishes its
response and successor, performs bounded cache eviction and reads the winner.
Losing requests cannot create orphan branches. Oversized records are rejected
before admission. Source/read-model tables remain outside cache cleanup.

Use primary D1 reads, not asynchronously replicated reads. A monotonic clock
incremented by data-revision publication detects crossed publication, including
an A -> B -> A change. Check the clock before and during admission. An unchanged
source revision alone is not sufficient. Deployment must maintain the existing
atomic read-model publication route; full bootstrap remains a maintenance path.

Adjacency uses composite endpoint/ID indexes and bounded keyset seeks, not
whole-graph reads or OFFSET over a high-degree node. Per-page SQL/work limits
can produce smaller pages than Python without altering BFS discovery order.

## Alternatives and consequences

- Worker isolate memory was rejected because requests do not share its lifetime.
- Unsigned client-held frontier documents were rejected for size and integrity.
- Durable Objects and another graph database are unnecessary for this bounded
  cache: existing D1 transactional batches can admit one response/successor.
- D1 writes have an explicit cost and capacity: 128 retained checkpoints / 32
  MiB, 1 MiB per stored record and 15-minute expiry. Expired rows are reclaimed
  on admission; no unbounded growth requires a new background scheduler.
- An additive migration and a small metadata entry support both bootstrap and
  delta publication. A content-version change does not force a row-schema
  change or discard row-delta eligibility.

## Evidence and limits

Mechanical checks cover Python/D1 order, index query plans, real HTTP replay,
concurrent requests, durable local D1 across isolate restart, expiration,
crossed publication and rejected oversized admission. Local D1 simulation is
not production deployment, load qualification or UI acceptance. The corpus
dependency scheduler is a separate owner-local processing step.

The implementation follows D1's [transactional batch contract](https://developers.cloudflare.com/d1/worker-api/d1-database/#batch),
[primary-read behavior](https://developers.cloudflare.com/d1/best-practices/read-replication/),
and [platform limits](https://developers.cloudflare.com/d1/platform/limits/).
Current operational boundary: [edge access](../../access/deploy/cloudflare-worker/README.md).
