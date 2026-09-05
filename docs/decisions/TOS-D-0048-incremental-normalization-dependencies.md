# Incremental normalization dependencies

## Index Metadata

- Decision ID: TOS-D-0048
- Original date: 2026-09-05
- Surface classes: access/backend, access/deployment, docs/architecture
- ToS layers: derived-exports, access
- Tree classes: constructor backend, incremental projection, read model
- Guard families: source-first authority, lossless projection, snapshot consistency
- Posture: accepted

## Context and decision

A whole-file processor hash invalidated every normalization step after an
unrelated lens-query edit. Independent key/value cache hits also did not expose
the dependency structure, interruption state or removal of processing subjects.

Extend [TOS-D-0045](TOS-D-0045-incremental-read-model-publication.md) with a
demand-driven, resumable DAG of pure normalization tasks. Inputs retain stable
task IDs, values and content digests. Tasks bind handler version, parameters and
ordered dependency output digests. The concrete route is source/type -> node ->
endpoint title -> relation. If a node changes without changing its title, only
the node and title task need reconsideration; the relation result can be reused.

Retain completed outputs independently from execution runs. Commit completed
pure work even if a later task or graph validator fails. Failed, cyclic,
conflicting-identity, empty/skipped and superseded runs cannot replace the active
dependency index. Compare-and-swap that index against the run's starting
baseline. It records successful processing, not semantic acceptance or serving
graph publication; the staged D1 row-publication boundary remains separate.

Hash the normalizers' transitive authored helpers and referenced constants,
plus dependency/cache helpers and Python major/minor version. Do not bind every
query function in the module to a normalization task. Stable IDs and records
remain owned by stronger ToS contracts; processing metadata cannot invent them.

## Alternatives and limits

- Whole-module invalidation was rejected for unrelated query edits, while
  explicit transitive helper changes still invalidate affected processor keys.
- Eager invalidation of every descendant was rejected: an intermediate output
  can remain identical even when its inputs changed.
- The graph assembler still requests tasks in domain-defined order. The
  scheduler handles dependency readiness and reuse, not an external job fleet.
- The existing ignored SQLite cache stores outputs and execution metadata;
  history can be discarded and is not an archive of source or review events.
  No task output is installed into a public source owner automatically.
- Source discovery, claim/view assembly, global validation and SQL comparison
  remain full steps. Acquisition, OCR, segmentation, alignment and review are
  not covered. This is the normalization DAG, not the completed corpus pipeline.

## Verification

Tests compare cached and uncached normalized graphs, unchanged-title reuse,
interrupted-run restart, processor changes, retired tasks, cycle/identity
rejection and competing publication. A green execution report does not prove
corpus completeness, source acceptance, deployment or UI behavior.

Current implementation boundary: [edge build](../../access/deploy/cloudflare-worker/README.md).
