# Traceable research registry imports and local source planting

## Index Metadata

- Decision ID: TOS-D-0060
- Original date: 2026-09-08
- Surface classes: source-witness, contracts, scripts/validation, docs/route-law
- ToS layers: research-packets, source-witnesses, philosophy, contracts, mechanics
- Tree classes: corpus, source
- Guard families: source-first authority, rights boundary, fixity, local payload boundary, source traceability
- Posture: accepted

## Context

The Operator requested complete normalization of 106 XLSX/DOCX pairs, including
all registry and gaps fields, then preparation and actual first source planting.
The two present tables and future differently structured corpora need independent
identity namespaces. Imported research claims about versions, access and rights
cannot become owner-reviewed facts merely because their syntax is normalized.
The existing chronological discovery queue remains useful, but its order alone
does not answer which concretely identified and permitted text can be acquired.

TOS-D-0040 separated scholarly-composite identity from exact File representation.
Its materialized-payload clause nevertheless required tracked bytes, even when
the stronger local storage and rights boundary permitted only local use. That
combination cannot describe an acquired local-only representation consistently.

## Decision

Retain exact research originals and immutable normalized snapshots under
`ToS/research-packets/source-registries/`. Separate corpus, document, logical
record and versioned occurrence identity. Bind every raw field and reported
normalization to its exact OOXML locator and original file digest. Adapter
mappings own heterogeneity; unsupported fields fail explicitly. Anonymous edited
rows require reviewed identity mapping and must not be merged by title.
Deterministic gzip is a storage encoding of the JSON carrier, not a new evidence
layer or a replacement for retained original bytes.

Keep reviewed source identity, rights, provenance and acquired files with
`ToS/source-witnesses/`; branch planting remains in `ToS/philosophy/`. Extend the
existing discovery queue with an explicit readiness selection mode and frozen
source-bound plans, preserving its chronological mode and historical receipts.
Version, access, intended-use rights, current local file presence and branch
linkage are independent checks. A reported research assertion cannot make a
readiness check verified.

Materialized scholarly-composite representations use the same local ignored,
untracked payload posture as source Items, with tracked metadata, rights,
provenance and fixity. This **partially supersedes TOS-D-0040 only in its mandatory
Git-tracking clauses for materialized payloads**. Its separate composite/File
identity, per-file rights, provenance, multi-volume and negative-authority law
remain in force. Absence stays explicit; a historical receipt does not prove
that another checkout contains bytes.

## Options Considered

- Keep link-only reports: insufficient for the requested exact local source
  planting and future reproducible import.
- Import registry permissions directly into rights records: rejected because a
  research statement is not an evidence-backed permission determination.
- Replace chronology with readiness: rejected; both selections answer distinct
  questions, and historical choices must remain reconstructible.
- Require tracked composite bytes while declaring them local-only: rejected
  because repository transport and the intended storage boundary conflict.

## Rationale and Consequences

A future reader can recover why normalized facts retain a lower authority than
source assessment, why shared titles or archives do not merge objects, and why
local availability must be checked independently of acquisition history.
Source updates retain old versions and emit reviewable deltas. Anonymous edits
can require a human or competent agent to assign continuity explicitly. Stored
originals and snapshots cost space, so large writes remain subject to the host
storage owner. No import, queue result or green validator admits text semantics,
canon, public transfer or deployment.

## Owner Surfaces and Validation

- `ToS/research-packets/source-registries/README.md`
- `mechanics/source-witnessing/parts/witness-route/config/registry-normalization.v1.json`
- `ToS/source-witnesses/discovery/candidates/README.md`
- `ToS/source-witnesses/LOCAL_STORAGE_BOUNDARY.md`
- `ToS/contracts/scholarly-composite-file-representation.schema.json`

Run the focused registry, queue and storage tests and corresponding validators;
regenerate decision indexes with `scripts/generate_decision_indexes.py`, then
check parity and `scripts/validate_decision_records.py`. Source-link integrity,
source catalogs, branch topology and affected generated projections remain
separate closeout checks. CI, merge and publication require their own route.
