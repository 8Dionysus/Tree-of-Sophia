# Content-verified build stages

## Index Metadata

- Decision ID: TOS-D-0049
- Original date: 2026-09-05
- Surface classes: access/backend, access/deployment, docs/architecture
- ToS layers: derived-exports, access
- Tree classes: constructor backend, incremental projection, read model
- Guard families: source-first authority, snapshot consistency
- Posture: accepted

## Decision

Extend [TOS-D-0048](TOS-D-0048-incremental-normalization-dependencies.md) with
coarse completed-stage reuse in the offline edge builder. Normalization caching
alone still rebuilds and validates the graph and rewrites SQL on every invocation.
SQL and static-response stages therefore bind their complete declared public
inputs, producer code, contracts, Python version and output byte digests.
Membership and missing files participate. Web assets belong only to the static
stage; the deployed delta baseline belongs only to SQL.

Prefer conservative invalidation over a guessed narrow dependency: all access
Python helpers and build scripts participate. Do not use mtime/size as content
identity. Keep stage checkpoints private and disposable, outside source and
review records. The checksum protects accidental integrity, not authentication.

Reuse of a completed stage means its former result is still applicable, not that
normalization or validation ran again. The completion manifest reports those
states separately and carries no semantic acceptance. Invalidated stages retain
full graph validation and the existing row-delta publication guard.

## Failure and concurrency boundary

Use a Unix OS lock for one runtime directory rather than persistent PID markers
that block restart after a crash. One runtime owns one output directory, and
deployment follows build completion rather than running concurrently. Remove
only the exact generated completion manifests before work and write them last.
A failed stage cannot reuse a stale success; completed other stages survive.

Read inputs again before saving stage success and before build completion.
This detects ordinary concurrent edits; it does not provide an immutable source
snapshot or prevent ABA edits. The build requires quiescent inputs. No change is
made to source publication, full-bootstrap atomicity or runtime availability.

## Limits and verification

Whole-input byte hashing and output integrity reads remain linear. Changed
corpora still require full graph assembly, global validation and SQL comparison;
a static-only rebuild can still materialize the graph. This is a bounded build
optimization, not record-addressed source ingestion or the OCR/review pipeline.

Fixture tests cover cold/warm execution, no graph call on a no-op build,
same-size/mtime source changes, schema/membership changes, corrupt/missing
outputs and metadata, independent UI/SQL invalidation, failure/restart and
competing builders. Real builder fixture SQL is loaded into SQLite to check
that changed source labels actually reach the generated read model.

Current behavior belongs to the [edge builder](../../access/deploy/cloudflare-worker/README.md#resumable-build-stages).
