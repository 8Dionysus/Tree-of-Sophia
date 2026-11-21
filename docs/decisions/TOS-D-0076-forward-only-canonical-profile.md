# TOS-D-0076 Forward-Only Canonical Profile

## Index Metadata

- Decision ID: TOS-D-0076
- Original date: 2026-11-02
- Surface classes: docs/route-law, ToS/contracts, rust/foundation
- ToS layers: contracts, docs
- Tree classes: none
- Guard families: artifact integrity, compatibility, source-first authority
- Posture: accepted; implementation pending

## Context

The re-baseline dry run mapped 24,272 occurrences, reported 31,082 missing
inputs and identified 167 occurrences containing numbers refused by JCS.
Historical digests preserve fixity evidence for the bytes and profiles that
produced them. This decision supersedes the full re-baseline and migration-map
route in TOS-D-0075; its JCS byte and number rules remain in force.

## Decision

Every new canonical-JSON digest uses `jcs-rfc8785-v1` and carries the `jcs1`
profile tag alongside the hash. The owning carrier defines whether the digest
covers an appended LF. New records encode precision-sensitive numbers that
JCS refuses as strings. Historical records retain their recorded digest and
profile; an untagged historical digest uses the established profile for its
field family. Verifiers dispatch from the tag or that historical family
profile and reject a mismatched or unknown tag.

The three historical canonical profiles remain in a frozen verification module
until no tracked record or operator store uses the respective profile. The
full re-baseline and its tooling remain outside main; `claude/canon-map`
retains the investigation record. Exact-byte hashes keep their own contracts.

## Options Considered

- Re-baseline existing records and references to JCS.
- Preserve historical digests and introduce tagged JCS digests forward-only.

## Rationale

Forward-only adoption preserves historical fixity while giving new records an
explicit interoperable canonical profile. Field-family attribution and frozen
verification retain the context needed to interpret older values.

## Consequences

Writers, schemas and verifiers need coordinated changes at each digest family.
Historical inputs and operator stores continue to require their original
verification profiles. Carrier contracts specify LF coverage and exact-number
string encodings. Retirement follows the per-profile usage condition.

## Source Surfaces

- `docs/decisions/TOS-D-0075-jcs-canonical-bytes.md`
- `rust/crates/tos-foundation/src/jcs.rs`
- `rust/crates/tos-command/src/bin/digest-field-classes.v1.json`
- `docs/RETIREMENT.md`

## Validation

Review this record against the operator decision and validate each affected
writer, historical verifier and source contract through its owner route.
