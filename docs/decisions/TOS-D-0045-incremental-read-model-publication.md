# Incremental read-model publication

## Index Metadata

- Decision ID: TOS-D-0045
- Original date: 2026-09-04
- Surface classes: access/backend, access/deployment, docs/architecture
- ToS layers: derived-exports, access
- Tree classes: constructor backend, incremental projection, read model
- Guard families: source-first authority, lossless projection, snapshot consistency
- Posture: accepted

## Context

Backend-defined lenses must remain cheap to evolve as text metadata grows.
Skipping an unchanged whole snapshot is insufficient: one changed source must
not require uploading every unchanged row. Partial publication must not expose
entities and relations from different revisions.

## Decision

Keep source and review authority with ToS. Give the disposable access read model
two reuse boundaries: completed pure normalization steps and deterministic SQL
rows. Step keys bind processor bytes, inputs and normalization dependencies.
Row digests bind complete chunked values and an explicit storage schema version.
These caches accelerate local builds; they are not source history or acceptance.

Publish a row delta only against its exact serving baseline. Stage changed rows
and removal keys separately. One guarded SQLite trigger statement publishes all
affected tables and the target revision atomically; missing staging rows or a
stale baseline abort the transaction. Replay rebuilds staging and skips
publication only if the serving revision is already the target. A historical
receipt does not establish currentness. Multi-query knowledge reads check the
revision before and after execution and reject crossed publication for retry.

Full SQL remains a bootstrap/schema-change recovery path. Sequential table swaps
do not have the delta's multi-table atomicity guarantee; use a maintenance route.
Deployment selects a delta only for a matching baseline and schema, and verifies
the serving target revision before recording a successful row-index baseline.

## Alternatives and consequences

- Whole-corpus reload on every change was rejected as the normal path because
  transfer cost grows with unrelated material.
- In-place per-row publication was rejected because interruption exposes mixed
  revisions and potentially partial large fields.
- No external queue or second authoritative graph store is introduced.
- Source discovery, graph validation and SQL comparison still run. This is
  incremental normalization and delivery, not a complete dependency scheduler.
  Cache history consumes ignored disk and can be discarded. Very large deltas
  may exceed database execution limits; they must fail and roll back rather
  than silently split publication into partial commits.

## Verification and boundary

Python tests cover unchanged rows, removals, escaped keys, schema changes, stale
baselines and replay. An isolated Worker D1 binding test covers interrupted
staging and atomic replay. These checks do not prove deployment, load capacity,
semantic review or public acceptance.

Implementation: [edge read model](../../access/deploy/cloudflare-worker/README.md).
Semantic identity remains with [TOS-D-0059](TOS-D-0059-stable-semantic-interchange-registry.md)
and its stronger ToS registry sources.
