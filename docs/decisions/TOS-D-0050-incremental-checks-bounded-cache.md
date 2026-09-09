# Incremental checks with disposable bounded cache

## Index Metadata

- Decision ID: TOS-D-0050
- Original date: 2026-09-05
- Surface classes: access/backend, docs/architecture
- ToS layers: derived-exports, access
- Tree classes: constructor backend, incremental projection, read model
- Guard families: source-first authority, snapshot consistency
- Posture: accepted

## Decision

Extend [TOS-D-0048](TOS-D-0048-incremental-normalization-dependencies.md) and
[TOS-D-0049](TOS-D-0049-content-verified-build-stages.md): cache final node
materialization and local semantic checks using explicit content dependencies,
while retaining global identity, registry and cardinality checks on each graph.
This supersedes their full per-record validation and unbounded cache-history
implementation limits, not their publication or source-authority boundaries.

Checking only changed relation bytes is insufficient: endpoint identity, missing
evidence and exact-version review can change without changing the relation.
Bind those dependencies, including absent references, and use actual content
digests rather than trusting a supplied revision. Duplicate IDs must not share a
per-ID cached verdict. Keep one implementation of each rule for cached and full
execution, tested for equivalent results across changes and failures.

## Retention is not source history

Use independent output byte/count budgets and bounded execution-run retention,
not append-only storage of every build. Eviction only increases recomputation;
it must not change graph meaning. Verify cached payload integrity before reuse.
Use exclusive OS locking to permit interrupted-run recovery and safe local GC.
No source, review or canon record is deleted by this cache policy.

Expose paginated input changes with before/after digests, but never infer removal
from an interrupted scan. A retired baseline makes its delta explicitly
unavailable. An execution receipt is not a durable change ledger: consumers
needing historical source changes must use the source owner's version history.

## Accepted costs and limits

The budget constrains serialized completed outputs, not total SQLite size or
graph RAM. Run metadata grows with the current graph, not unlimited build
history; compaction may need temporary space. A budget smaller than the working
set can cause cache thrashing without changing correctness.

Whole source-file reads, dependency discovery, graph indexes, global checks and
SQL comparison remain linear. This choice does not claim fully incremental
source acquisition, OCR, translation, alignment or review. Those belong to their
source owners. No UI rendering or public query contract changes here.

Current behavior and limits belong to the
[access builder](../../access/deploy/cloudflare-worker/README.md#incremental-checks-and-cache-retention).
`access/tests/test_processing.py` checks full/cached parity, dependency changes,
missing references, partial scans, pagination, failure, corruption and eviction.
