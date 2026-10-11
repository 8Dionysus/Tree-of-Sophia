# TOS-D-0071 Transactional source updates and independent projections

## Index Metadata

- Decision ID: TOS-D-0071
- Original date: 2026-10-11
- Surface classes: access/runtime, corpus/contract, docs/route-law
- ToS layers: source-witnesses, contracts, derived-exports, docs
- Tree classes: corpus, graph projection, generated companion
- Guard families: source-first authority, exact-byte provenance, compatibility, recovery
- Posture: accepted direction by explicit operator instruction; implementation in progress

## Context

The growing Site/knowledge path rebuilds a complete normalized snapshot and
search index for an ordinary source change. Its searchable representation
includes the serialized carrier and its technical metadata. Compression reduces
stored bytes while leaving the repeated expansion and indexing work. Separate
delta writers and a dependency cache do not connect ordinary source commands to
an incremental serving path. A failed final export can discard costly preparation.

The Operator selected a persistent transactional central system and a real
source-change/recovery slice as the next acceptance boundary. A successful full
Site build alone cannot satisfy this boundary.

## Decision

Use the existing Rust/PostgreSQL source coordinator as the central transactional
contour. PostgreSQL keeps current records, revisions, relationships and the
transactional change queue. Exact source bytes and historical locators remain in
the existing immutable source store. JSONB, when useful, represents derived
attributes; it never replaces exact source custody.

Update dependent projections from committed source revisions. Each projection
has an explicit version, dependency scope and durable progress position. Commit
the affected output and its progress atomically. An interruption preserves
completed work, and retry resumes from committed progress. Publishing or selecting
a ready projection remains distinct from preparing it. Source admission, current
rights, semantic review and canon retain their existing authorities.

Search indexes declared content fields and text fragments under a new explicit
contract. Start with PostgreSQL text indexes; introduce a separate search engine
only when measurement warrants it. IDs support exact lookup independently of
text matching. Technical JSON keys, paths, provenance envelopes and duplicated
source records are not the default searchable text. Existing substring readers
retain their versioned behavior until their consumers migrate explicitly.

Full SQLite/Site builds are portable exports and offline products. Their file
budgets constrain those products, not the central corpus. Browser and Worker
adapters receive appropriate derived projections. Shared Rust domain rules do
not require a shared physical database across native, WASM and D1 hosts.

This refines TOS-D-0069's compiler snapshot boundary: a full compiled snapshot
is no longer the required unit of readiness for an ordinary central update.
It retains TOS-D-0069's Rust scope, compatibility and exact-history obligations,
TOS-D-0059's bounded exports and TOS-D-0062's separate release identities.

## Options Considered

- Continue repairing full-build limits as the primary update mechanism. This
  leaves every ordinary edit dependent on repeating corpus-wide work.
- Move the existing serialized carrier wholesale into another full-text index.
  This retains unnecessary searchable input and changes query semantics without
  an explicit consumer migration.
- Use PostgreSQL transactions and separately maintained field projections,
  keeping snapshot exports independent. This is the selected direction.
- Add Tantivy immediately. Defer this until the selected PostgreSQL path has a
  measured need for a separate search engine.

## Rationale

The unit of ordinary work should follow the changed source and its actual
dependencies. The existing coordinator already keeps commit, history, receipt
and outbox atomic; PostgreSQL supplies transactional indexes and recovery. Rust
keeps the domain rules and owner interfaces. The central state can grow without
forcing every consumer or export to carry the complete corpus.

## Consequences

The transition requires both source-command integration and projection consumers.
An outbox worker alone is not the completed system. Current managed Agent creation
is narrower than the complete source command set; filesystem revision routes,
whole-input source-selection work and full knowledge consumers remain migration
obligations. A selected source-text profile is not the full graph API.

Before claiming the replacement usable, run an existing-data source change
through its real owner command, transaction, affected projections, search and
exact old/new reads; interrupt and resume it. Repeat with a larger baseline and
show that unchanged records are not recompiled. Then migrate the remaining
consumers and prove compatibility, installation and restoration. This decision
does not claim that performance or migration acceptance has already been proved.

## Source Surfaces

- `rust/crates/tos-command/src/durable_adapter.rs`
- `rust/crates/tos-command/src/durable_schema.sql`
- `rust/crates/tos-command/src/source_revision_publication.rs`
- `rust/crates/tos-command/src/durable_projection.rs`
- `rust/crates/tos-command/src/source_search_projection.rs`
- `access/README.md`, `access/PROCESSING_DEPENDENCIES.md`
- `docs/decisions/TOS-D-0069-rust-execution-foundation.md`

## Validation

Regenerate and check decision indexes and validate records through
`docs/VALIDATION.md`. Verify the source/projection transaction and recovery path
against PostgreSQL and the immutable store. Field-extraction checks and a green
export build do not establish end-to-end source integration or performance.
