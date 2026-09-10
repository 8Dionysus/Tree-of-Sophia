# Typed historical-time source wording in Claim navigation

## Index Metadata

- Decision ID: TOS-D-0062
- Original date: 2026-09-10
- Surface classes: doctrine/ontology, corpus/contract, access/backend, docs/architecture
- ToS layers: doctrine, contracts, source-witnesses, derived-exports, access
- Tree classes: claim graph, human forms, semantic interchange, navigation descriptor
- Guard families: source-first authority, human wording, claim reification, exact dependency binding, lossless projection
- Posture: accepted

## Context

The real Jenseits 1886 commissioning DateClaim has source wording
`03. 06.1886`, attributed qualifications and a separately normalized date value.
Its structured object made the identity-only reader from TOS-D-0057 refuse
navigation, even though adjacent participant and Work Claims were readable.
Neither the source wording nor a readable template supplies a new dating,
assessment, historical corroboration or substantive HumanForm.

## Decision

Partially supersede TOS-D-0057's identity-only eligibility limit through the
existing template's version-2 opt-in `historical-time-source-wording-v1` adapter.
The adapter is eligible only for the existing historical-temporal source profile
and an understood temporal range. It copies the complete source-wording text and
preserves declared language. It binds the full Claim/version/digest, exact
literal ID, source file/line and whole structured-value digest. No temporal value
is promoted to a source identity. Version-1 templates retain the prior refusal.

## Options Considered

- Keep the identity-only refusal: retained whenever the adapter is absent or
  the source typing, wording or binding cannot be understood.
- Render a formatted normalized date: not selected because it can erase source
  wording and imply a calendar or precision that the source did not establish.
- Copy exact source wording inside the existing nonstandalone field catalogue:
  selected, without changing the source Claim or supplying a substantive reading.

## Rationale

The source already owns the human text needed to find this Claim. The existing
finite template, source profile and independent access verifier can expose it
without inventing names in the UI or adding a second registry. Explicit opt-in
and a version increment preserve the earlier reader boundary and make the
eligibility change inspectable. Canonical value comparison preserves distinctions
such as false versus zero, including in unknown source fields.

## Consequences

Date, interval, relative-order and unknown-date source wording can participate
without calendar conversion or precision inference. Navigation remains marked
as nonstandalone and not a supplied source title. Full qualifications, attribution,
uncertainty, alternatives and original review posture remain in the Claim; compact
reading is not made complete by a descriptor. Missing/ambiguous/unsupported inputs
and over-budget titles fail closed. Unknown calendars and source extensions stay
raw context, not newly understood facts.

## Source Surfaces

- `ToS/doctrine/HUMAN_FORMS.md`
- `ToS/doctrine/semantic-interchange/relation-types.v1.json`
- `ToS/contracts/semantic-relation-type-registry.schema.json`
- `scripts/source_witness_bibliographic_graph_common.py`
- `access/src/tos_access/knowledge.py`
- `tests/test_source_witness_bibliographic_graph.py`
- `access/tests/test_knowledge_contract.py`

## Validation

Run the focused Claim-navigation tests and exact pre-change semantic-registry
transition check. Regenerate decision indexes using
`scripts/generate_decision_indexes.py`, then check parity and decision records.
The integration owner must regenerate source projections and test the actual
consumer. CI, merge, deployment and whole-Foundation acceptance remain separate.
