# TOS-D-0062 Independent Software, Corpus and Integration Releases

## Index Metadata

- Decision ID: TOS-D-0062
- Original date: 2026-09-14
- Surface classes: access/runtime, docs/route-law, kag/provider, scripts/validation, github/checks
- ToS layers: access, source witnesses, derived exports, local KAG provider
- Tree classes: none
- Guard families: source-first authority, artifact integrity, compatibility, scoped validation
- Posture: accepted by explicit operator instruction

## Context

Software landing currently requires the repository's complete corpus,
generated documentation inventories and a matching KAG source family. Corpus
growth therefore changes the cost and outcome of an unrelated product fix.
The integrated source family exceeded the pinned KAG builder's global byte
ceiling; repeating generation cannot admit it under the same limit.

The operator requested a complete production boundary redesign, authorized
implementation and landing, and prioritized unblocking other sessions in the
first delivery. Permanent local payload custody plus permitted private R2
copies already exists and remains unchanged. Public activation is deferred.

## Decision

Software changes, corpus admission, data compilation and downstream
integration are separate operations with distinct exact inputs and failure
boundaries. ToS software builds, tests and packages without production corpus,
R2 credentials or sibling AoA checkouts. Reader contracts and executable web
assets belong to the installed software. An explicitly selected data root
cannot replace those subjects, and an absent data selection cannot silently
discover another checkout through the working directory.

The required `Repo Validation` check validates software and applicable
authored changes. It does not require a new complete corpus, generated
currentness inventory, KAG family or stats federation for each software SHA.
Program tests use representative fixtures; complete data validation belongs
to the exact data release. Failed, cancelled or missing required software
checks must still reject landing.

Corpus versions preserve exact source identity, provenance, rights and review
events. Data artifacts bind their corpus, schemas and compiler inputs; the
reader selects a compatible format independently of the producer Git SHA.
Existing full data tools remain explicit during migration. Non-payload source
tracking ends only after exact export, locator migration and verified restore;
the corpus ignore is not blanket deletion authority.

KAG and stats integrations consume exact ToS exports independently. They show
their own revision and failure/currentness state. A failed downstream build
withholds its new artifact and does not block ToS software. This is permanent
release separation, not a frozen family or a stale-currentness exception.
AbyssOS admission remains specific to an AbyssOS consumer.

## Supersession

- Supersedes D0044's matching KAG generation and required KAG CI for every
  tracked source change. Exactness/integrity of each KAG release remains;
  an old family cannot silently claim currentness.
- Supersedes the universal release-gate and mandatory generated-inventory
  obligations in D0009, D0010, D0032 and D0041. Authored ownership, explicit
  command authority and meaningful behavioral checks remain.
- Supersedes D0026, D0030 and D0035 only where ecosystem provider alignment
  blocks an independent software release. Historical published identities
  and exact downstream provenance remain.
- Extends D0038 to independent build, test, package and release. D0059's
  explicit offline compilation and partition integrity are retained.

Historical decision files are not rewritten to pretend this was their rule.

## Options Considered

- Detect failures earlier, reorder jobs or reuse more outputs while keeping
  the common release transaction.
- Temporarily freeze or exempt KAG while keeping universal corpus/currentness
  obligations elsewhere.
- Separate the operations and remove the common obligation, preserving
  integrity and source authority at the corresponding boundaries.

The operator selected the third approach. Existing verified reuse remains
useful inside it; moving the same aggregate after merge is insufficient.

## Rationale

Source authority depends on exact identity, provenance and reviewed scope.
It does not depend on regenerating every derived carrier after an unrelated
software edit. A product needs compatible verified data, not an identical
Git revision for all its independent producers.

## Consequences

- First delivery lands independent software checks and release. Other
  sessions use that route while data migration is completed.
- Second delivery completes immutable corpus/data storage, historical
  resolution, downstream adapters and retirement of transitional readers.
- Unique discovery evidence is preserved; reproducible reports and indexes
  become artifacts. Curated authored meaning can remain Git-backed.
- Rollback selects a verified compatible software/data pair; it cannot undo
  a rights revocation. Local payload copies remain after R2 upload.
- No distributed service, universal validation registry or Git history
  rewrite is required by this decision.
- Public site/Worker/D1 deployment remains deferred by the operator.

## Source Surfaces

- `AGENTS.md`, `VALIDATION.md`, `docs/RELEASING.md`
- `.github/workflows/repo-validation.yml`
- `access/AGENTS.md`, `access/contracts/`, `access/packaging/`
- `access/src/tos_access/locations.py`, `access/src/tos_access/core.py`
- `ToS/source-witnesses/`, `ToS/doctrine/CORPUS_FOUNDATION.md`
- `docs/validation/validation_lanes.json`
- `kag/AGENTS.md`, `kag/VALIDATION.md`, `stats/`

## Validation

Validate decision shape and generate lookup indexes through the owner
builder. Verify software without corpus or sibling repos; reject bad schemas,
integrity bindings and formats; exercise readers with fixed data and verify
restoration of migrated sources. Complete implementation requires both
deliveries and actual landed CI; this decision alone does not prove it.
