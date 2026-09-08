# Source-owned Claim navigation without manufactured assertion wording

## Index Metadata

- Decision ID: TOS-D-0057
- Original date: 2026-09-08
- Surface classes: doctrine/ontology, corpus/contract, access/backend, docs/architecture
- ToS layers: doctrine, contracts, source-witnesses, derived-exports, access
- Tree classes: claim graph, human forms, semantic interchange, navigation descriptor
- Guard families: source-first authority, human wording, claim reification, exact dependency binding, lossless projection
- Posture: accepted

## Context

Legacy responsibility and topology Claims are independently addressable objects,
but many have no authored statement or adjacent HumanForm adapter. Their graph
labels consequently fall back to technical IDs, including real expression,
edition and translator routes. The source endpoints and exact predicate mapping
already provide useful navigation material. They do not provide a reviewed
sentence asserting the relation, a full personal name, or permission to promote
the source's initial review status into current assessment authority.

The Foundation goal authorizes implementation of source-owned human/machine
contracts while preserving the separate UI owner. The accepted choice below
belongs to that scope; it changes no historical source Claim, grant or canon.

## Decision

Keep one optional, versioned finite Claim-navigation template in the existing
source relation registry. Export a separately named descriptor on a Claim,
bound to the full source Claim, used template and predicate mapping, and exact
typed endpoint source records. Render a labelled field catalogue rather than
a subject-predicate-object assertion. Require both declared source statuses and
whole endpoint names; refuse missing, ambiguous, unknown or over-budget inputs.

Access independently verifies the descriptor before normalizing it. The display
can use its title with explicit `navigation-template` provenance, but must report
`source_title_available: false`. Descriptor availability does not make compact
reading complete. The entire Claim remains mandatory inspection context, with
its evidence, alternatives, qualifiers, maker, versions and actual assessments.

The owner syntax is non-executable literal/slot data. Source text cannot select
tools, expressions, registry changes or an assessment outcome. Native artifact
inventory labels use their existing exact adapter; arbitrary carrier pointers
do not create new name mappings. Identity and Claim-object separation are intact.

## Options Considered

- A UI heuristic parsing known IDs or special-casing affected Claims would make
  the consumer a second naming authority. It was rejected; defensive missing-name
  UI handling can remain but does not provide source content.
- Generating a fluent assertion directly from endpoint labels would hide missing
  qualifications and assessment. It was rejected as a substitute for HumanForms.
- Requiring complete legacy HumanForm migration before any readable navigation
  would conflate finding a record with having its full wording. The two operations
  remain separate, and the migration is still needed for substantive reading.
- A separate template registry would duplicate the exact predicate owner's
  versioning and labels. The bounded optional member of the current registry
  supplies the required contract without another source of truth.

## Consequences

Templates and used names are exact display dependencies. Unrelated registry
edits do not change a descriptor, while changed endpoint records invalidate the
dependent Claim and relation display tasks. No OCR or model invocation is needed.
The finite output budget refuses the whole title rather than truncating it.

The first reader covers identity-to-identity Claims only, not all structured
values or every legacy statement. Russian/English syntax does not translate
endpoint names or establish their original language. An initial source status
remains a source declaration. No descriptor accepts evidence, permits publication,
grants access, supplies absent prose or completes Foundation v1.

Old exports without a descriptor retain missing-title behavior. A malformed or
stale present descriptor is refused by the current reader. Canonical export
validation compares against the source rebuild; previous/current registry checks
enforce version increments and reject silent identity/reader repurposing.

## Owner Surfaces and Follow-up

- `ToS/doctrine/HUMAN_FORMS.md`
- `ToS/doctrine/semantic-interchange/relation-types.v1.json`
- `ToS/contracts/semantic-relation-type-registry.schema.json`
- `scripts/source_witness_bibliographic_graph_common.py`
- `access/src/tos_access/knowledge.py`
- `tests/test_source_witness_bibliographic_graph.py`
- `access/tests/test_knowledge_contract.py`

The UI owner consumes the ordinary display contract without changing spatial
composition or inventing names. Worker/D1 preserves the normalized carrier, not
a second renderer. Real legacy source-copy forms and broader human-form coverage
remain with the source owner; CI, merge and deployment are separate outcomes.
