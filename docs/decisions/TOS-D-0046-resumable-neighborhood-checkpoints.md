# Resumable neighborhood checkpoints

## Index Metadata

- Decision ID: TOS-D-0046
- Original date: 2026-09-04
- Surface classes: access/backend, docs/architecture
- ToS layers: derived-exports, access
- Tree classes: constructor backend, read model
- Guard families: source-first authority, snapshot consistency
- Posture: accepted

## Context

A delivery cursor divides an already bounded LensResult. It does not allow a
constructor to keep exploring beyond that result. Re-running larger lenses on
every gesture also repeats work and confuses final counts with discoveries.

## Decision

Extend the local access service with a separate resumable-neighborhood contract.
Retain the BFS frontier, visited identities, current adjacency position, fixed
query and graph revision in disposable server checkpoints. Existing LensSpec
compilation remains stateless. This extends, without replacing, the constructor
boundary in [TOS-D-0043](TOS-D-0043-backend-defined-knowledge-lenses.md).

The first implementation is process-local: bounded cache, fixed expiry, opaque
random cursors and immutable replay. It is available to local HTTP and native
MCP, not yet Cloudflare or a one-shot CLI. A lost checkpoint requires an explicit
restart; it cannot substitute current data or claim the neighborhood is complete.
A snapshot digest includes execution version and normalized content identities,
not only source revision. Context endpoints repeat for page closure; primary
nodes and emitted edges do not. The consumer keeps camera/layout state.

Access owns execution state only. No new corpus facts, source history, accepted
interpretations or review authority are created. Checkpoints never enter the
public data allowlist; request/result schemas do.

## Alternatives and consequences

- Recompiling a larger LensResult was rejected for this operation because it
  does not preserve pending work and scales with unrelated already-read data.
- A client-supplied visited/frontier document was rejected: it grows with the
  walk and requires integrity validation to justify inclusion traces.
- Durable shared checkpoints are deferred until the edge adapter has explicit
  storage, expiry, concurrency and revision-consistent adjacency-read tests.
  In-memory state must not be advertised as surviving Worker isolates/restarts.
- Local graph indexing costs one full snapshot pass. Pages use adjacency, but
  checkpoint serialization still costs proportionally to visited state. TTL,
  cache, per-page work and session limits remain explicit capacity boundaries.

## Verification and limits

Generated cyclic multigraphs compare paginated results with an independent BFS
across direction, depth and page size. Tests protect endpoint closure, exact
coverage, replay, concurrency, expiration, eviction and revision conflicts.
HTTP and native MCP tests exercise actual adapters. These are mechanical query
guarantees, not deployment, real UI acceptance, corpus completeness or semantic
review. The corpus dependency scheduler remains a separate unfinished step.

Current contract: [constructor access](../../access/README.md#resumable-neighborhood-exploration).
