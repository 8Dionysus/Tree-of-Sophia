# Source-described conceptions without retyping existing Concept nodes

## Index Metadata

- Decision ID: TOS-D-0053
- Original date: 2026-09-07
- Surface classes: contracts, access/backend, docs/architecture
- ToS layers: doctrine, contracts, source-witnesses
- Tree classes: knowledge foundation, concept
- Guard families: source-first authority, identity preservation
- Posture: accepted

## Context

The Foundation dialogue distinguishes a concept that persists across divergent
accounts, a situated conception, textual occurrence and transformation between
conceptions. Current canon Concept nodes may instead describe an explicitly
bounded interpretation, such as becoming in Zarathustra's prologue. Reusing
their IDs as cross-historical invariants would silently enlarge their meaning.
The declared metadata reader also required the bibliographic/historical
identity family, so it could not faithfully represent a situated account.

## Decision

Keep the existing Concept family and its mappings unchanged. Add a specific
CrosscuttingConcept subtype and a separate Conception semantic family. Each
source-described referent has substantive notes and an explicit, contestable
scope and continuity criterion. A new semantic reader mode reuses the existing
schema-bound metadata, catalog, graph, human-form and separately authorized
command pipeline; it does not turn semantic subjects into bibliographic
identities or make a new store of accepted philosophical truth.

Membership, thinker attribution, expression and the eight named transformation
distinctions are evidence-bearing Claims with explicit relation bases. Changing
a description increments its record version; historical conceptual change
relates distinct conceptions. Claims, source records and their language forms
never share identity by convenience. Scope remains mandatory reading context.

## Alternatives and consequences

- Reinterpret every existing Concept as cross-historical: rejected because
  existing source scopes and IDs would drift without a migration or review.
- Make Conception a record version: rejected because record correction is not
  historical change of thought and record timestamps are not historical dates.
- Place new semantic subjects under bibliographic Identity to reuse the reader:
  rejected because implementation convenience would dictate ontology.
- Create a parallel semantic database and writer: rejected because the existing
  declared-profile boundary already provides exact schemas, provenance,
  permission checks, transaction history and source-preserving readers.
- Add explicit semantic modes and source-described subjects: chosen. This
  preserves old routes but adds contract dependencies and requires coordinated
  reader/registry upgrades. Historical modes retain their narrower guards.

Source-described records remain weaker than source interpretation assessment,
scoped admission and canon. A valid nonempty criterion is not a proved criterion.
The ordinary correction command cannot change identity scope, but still needs
the content-assessment route to judge whether a prose correction preserves its
referent. Existing records are not automatically classified into the subtype.
Reader rollback retains new source files and history; an old reader must fail
closed on unknown modes, not discard or misclassify them.

## Source and verification boundaries

Current meaning is owned by `ToS/doctrine/semantic-interchange/README.md`, its
type/relation registries and the exact semantic record/Claim schemas. This ADR
records rationale, not source truth or an assessment policy. Synthetic checks
exercise both source readers, typed Claims, mandatory human-form context,
source creation/correction, exact prepared dependencies and rejected authority
or mode substitutions. Real historical accounts, occurrence mappings and
substantive assessment require their own source-visible evidence; they are not
established by this architecture decision or a passing test.
