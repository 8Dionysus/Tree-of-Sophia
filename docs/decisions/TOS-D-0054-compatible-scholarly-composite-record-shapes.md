# Compatible scholarly composite record shapes

## Index Metadata

- Decision ID: TOS-D-0054
- Original date: 2026-09-07
- Surface classes: contracts, source-witness, access/backend, docs/architecture
- ToS layers: doctrine, contracts, source-witnesses, derived-exports
- Tree classes: composite, knowledge foundation
- Guard families: source-first authority, identity preservation, projection boundary
- Posture: accepted

## Context

[TOS-D-0025](TOS-D-0025-scholarly-composite-witness-spine.md) establishes the
scholarly composition as a referent distinct from its ancient original,
physical witnesses and provider representations. The current native v1 schema
requires physical member observations. That shape serves the existing witness
packets but cannot describe a textual editorial arrangement before its physical
witness chain has been identified. Inventing artifacts would manufacture
evidence; a second reconstruction identity family would duplicate the existing
semantic distinction merely to avoid its record format.

## Decision

Extend the existing composite family with an explicit descriptive metadata
profile, while retaining the native witness adapter unchanged. The new
`composite.json` and existing `composite-witness.json` are supported shapes of
the same `tos.composite.*` identity family, not two current records of one
subject. The catalog and command collision checks cover both shapes together.
The source registry explicitly declares `retained_native_adapter`; ordinary
profiles cannot capture this protected namespace by choosing its filename.

The descriptive profile requires an attributed composition account, editorial
method, coverage limits and referent criterion. It uses the shared metadata
creation, correction, human-form and assessment routes. Native records retain
their full bytes and source-specific limits; their unsupported human-form or
assessment routes are not fabricated by adding common metadata fields.

Both formats stay within the canonical scholarly-composites owner subtree.
Current source schema and exact catalog mapping choose the reader; neither
display labels nor the presence of a newly declared profile can reinterpret a
native witness. Compiler, reconstructed object, publication, membership and
ordering remain independently grounded relationships, not implicit edges from
descriptive prose.

## Alternatives and consequences

- Populate native physical-member fields with textual placeholders: rejected;
  this would assert material identities and evidence not established by study.
- Replace or bulk-convert native records: rejected; their source fields,
  rights, provenance and historical identities already have an owner contract.
- Add another textual-reconstruction identity family: rejected; representation
  format alone does not distinguish the referent from a scholarly composite.
- Retain the native adapter and add an explicit common-metadata profile:
  chosen. This adds dual-format dispatch and compatibility checks, but reuses
  the catalog, identity model and source commands instead of a second store.

The new schema is not an upgrade of a native v1 record. A later migration of an
existing subject needs an explicit source-history transition and must preserve
its references and prior bytes; creating another current record is refused.
Rolling back a reader leaves both source formats and history intact. An old
reader may reject the unfamiliar profile; it must not silently discard it.

## Owner and verification boundary

Current law belongs to `ToS/doctrine/CORPUS_FOUNDATION.md`, the semantic
registry, `ToS/contracts/scholarly-composite-record.schema.json`, the retained
witness schema and the source-witness owner. This decision extends the record
representation without superseding TOS-D-0025's identity and evidence law.

Synthetic contract checks exercise coexisting records, exact source retention,
shared focus identity, duplicate refusal, required descriptions and owner-home
guards. Command checks exercise correction, retained history and source-bound
forms. These mechanics do not prove historical authenticity, completeness,
linguistic quality, rights, source-text admission, canon or publication. Real
reconstructions require their own source-visible research and scoped review.
