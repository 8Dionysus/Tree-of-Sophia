# Expression Derivation Is Not Equivalence

## Index Metadata

- Decision ID: TOS-D-0022
- Original date: 2026-08-01
- Surface classes: source-witness, contracts, derived-graph, scripts/validation, docs/rationale
- ToS layers: source-witnesses, contracts, derived-exports, scripts, docs
- Tree classes: source, claim, relation
- Guard families: expression identity, derivation lineage, non-equivalence, source return, validator restraint, manual review
- Posture: accepted

## Context

The corpus evidence spine already distinguishes Work, Expression, Edition,
Item, and File. The Antonovsky revision history creates a narrower pressure:
different historical states of one translation need relations to identified
source Expressions without being flattened into equivalent texts or inferred
from dates and edition labels.

The exact 1911 translator preface names the 1900, 1903, and 1907 predecessors
and reports that each of those three editions was a new reworking of its
predecessor. It supports `1903 -> 1900` and `1907 -> 1903`. It does not state
the same categorical relation for the present 1911 edition. The 2007 witness
says that a translation first published in 1898 was used as its basis, but it
does not identify the 1911 Expression as that source.

A later exact-identity pass resolves the apparent date conflict without adding
an edge. Antonovsky himself calls the 1898 appearances journal excerpts and
counts his editions from 1900. A source-visible 1898 journal-office title page,
an official 1899 *Reader* record, and a contemporary report of an uncredited
reprint establish additional bibliographic boundaries. They do not establish
textual equivalence, a distinct collated Expression for the 1899 Edition, or a
direct 1900-to-1898 derivation. The already admitted two-edge revision chain
therefore remains unchanged.

A subsequent Lithuanian National Library record resolves the bibliographic
identity of one Saint Petersburg 1900 Edition and one physical holding. It
still adds no derivation edge: no source pages or bytes were acquired, the
holding is not a ToS Item, and neither the catalog record nor edition sequence
establishes collation, equivalence, or a 1900-to-1898/1899 textual dependency.

Classical FRBR distinction between derivation and equivalence, current LRMoo
`R76 is derivative of`, and current implementation evidence all point in the
same direction: missing lineage must remain missing rather than be completed
by sequence, title, translator, or graph convenience.

## Decision

Represent evidenced textual dependence between two distinct Expressions with
the directed predicate `is_derivative_of`, from the derived Expression to one
identified source Expression. For the v1 ToS profile, both endpoints must be
materialized Expressions of the same Work, and the reified claim must retain
derivation kind, source directness, statement basis, collation status,
evidence, provenance, review, and visibility posture.

The relation is non-transitive, asymmetric, irreflexive, and does not assert
textual equivalence. Multiple claims may identify multiple source Expressions;
the v1 profile does not impose a one-source cardinality limit. A missing or
underspecified source endpoint produces no edge. Consequently ToS admits the
two source-reported Antonovsky predecessor claims but does not infer
`1907 -> 1900`, `1911 -> 1907`, `2007 -> 1911`, or interchangeability among
any of those textual states.

The active contract and source claim packets own current structure and
evidence. This decision records only the rationale and remains weaker than
those source-owned surfaces.

## Options Considered

- Treat every dated Antonovsky edition as one Expression. This would hide
  reported reworking and make textual difference impossible to address.
- Model edition order as a transitive predecessor chain. This would invent
  edges not stated by the witnesses and confuse sequence with derivation.
- Use an equivalence or replacement predicate. This would claim
  interchangeability or metadata supersession rather than historical textual
  dependence.
- Admit only directed, source-returnable Expression derivation claims and keep
  every unsupported endpoint absent.

## Rationale

The selected route preserves stable bibliographic identity beneath later
collation and interpretation. It lets ToS trace a historical family without
choosing a canonical string, treating a revision as a mere metadata version,
or allowing graph traversal to strengthen the evidence.

Explicit negative edges are as important as the two admitted edges. They keep
future OCR, alignment, translation, semantic, and graph work from inheriting a
false textual equivalence before the missing witnesses are found and reviewed.

## Consequences

New derivation claims require materialized endpoints and direct evidence; a
chronological gap remains a gap. Collation may later refine or challenge a
reported relation, but it must create new versioned evidence and review rather
than silently rewriting the original claim.

The generated catalog and bibliographic graph may project the relation only
through its claim node. Validators may prove endpoint closure, cycles,
qualifiers, evidence return, and projection parity. They cannot accept the
bibliography, establish textual identity, judge translation quality, clear
rights, or promote semantics and canon.

## Source Surfaces

- `docs/decisions/TOS-D-0020-corpus-evidence-spine-and-witness-storage.md`
- `ToS/research-packets/foundation-laboratory-2026-07/EXPRESSION_DERIVATION_RELATION_RESEARCH.md`
- `ToS/research-packets/foundation-laboratory-2026-07/ANTONOVSKY_1898_1900_BIBLIOGRAPHIC_IDENTITY_RESEARCH.md`
- `ToS/research-packets/foundation-laboratory-2026-07/ANTONOVSKY_1900_LNB_HOLDING_RESEARCH.md`
- `ToS/source-witnesses/discovery/runs/antonovsky-1900-lnb-physical-holding.2026-08-01.v1.json`
- `ToS/contracts/expression-derivation.schema.json`
- `ToS/contracts/claim-packet.schema.json`
- `ToS/source-witnesses/relations/expression-derivation/expression-derivation-claims.jsonl`
- `ToS/source-witnesses/relations/expression-derivation/provenance.jsonl`
- `ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json`
- `scripts/validate_source_witness_foundation.py`

## Validation

Run decision-index regeneration and validation, source-witness catalog and
graph parity checks, source-foundation validation, focused tests, the manual
positive and negative graph queries, and the repository release lane.

Green structural checks do not alter the `reported`, `unreviewed`, and
`not_collated` posture of the two admitted claims.
