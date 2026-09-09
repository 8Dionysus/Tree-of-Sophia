# Claim-scoped reference values

## Index Metadata

- Decision ID: TOS-D-0056
- Original date: 2026-09-08
- Surface classes: contracts, source-witness, docs/architecture
- ToS layers: doctrine, contracts, source-witnesses, derived-exports
- Tree classes: source, claim, relation, knowledge foundation
- Guard families: source-first authority, identity preservation, projection boundary
- Posture: accepted

## Context

A motif hypothesis can compare several exact occurrences before there is any
accepted Sign. The Sign ladder already permits a Claim ID as candidate
identity. Pairwise Claims do not, by themselves, identify one common hypothesis
or make its third and later participants mandatory reading. A bibliography
Collection, ProblemFamily, thought move or rhetorical figure has a different
referent and should not be repurposed as a generic candidate container.

Structured values are useful when content needs no independent identity.
Their current `structured-value-v1` reader deliberately interprets no embedded
identity references. Teaching that reader to follow member-looking fields
would silently change old source meaning and could widen existing writer
delegations. The same distinction must survive public and confidential source
transports, complete human reading, graph projection and assessment.

## Decision and rationale

Add an explicit `structured-reference-value-v1` reader. Its registry profile
declares one fixed `/object/members` slot, specific member type families,
finite bounds and whether the focal subject belongs to the set. All members
are mandatory exact source dependencies. No arbitrary JSON walk, extension
field, qualifier or source instruction can add a reference.

The first application is a qualified motif-proposal value attached to one
focal-bound Claim. The Claim ID is the candidate identity; the literal's
projection ID is not a Sign, Collection or separate candidate owner. Equal
value bytes in two Claims do not establish candidate identity. Sign promotion
still requires its own competence-scoped source-visible decision.

Require complete proposed signification, grouping basis, scope, contrast and
limitations. Label the research formulation as a paraphrase; preserve exact
source quotations separately. Reading and assessment concern the whole set
and interpretation, not merely the focal-to-value binary entrance.

Graph member returns carry the Claim context and digest. They do not assert
independent accepted membership. A compact entrance is permissible only while
all declared member nodes and edges are present in its mandatory reading
closure. Omitted participants prevent folding rather than being concealed.

Keep old value readers and writer grants unchanged. New public v4 and private
v2 grants separately allow exact object values and every member in its member
role. A subject grant does not also grant the focal as a member. Member-set
correction is an explicitly authorized object change with exact history,
not a free qualifier patch. Private writer selector allowlists may cover the
old and proposed sets; each frozen read still receives only the exact complete
closure for that particular version. Extra selectors do not become evidence.

## Alternatives and consequences

- Mint a Sign or generic group first: rejected for provisional hypotheses;
  identity issuance would precede the promotion decision or misuse another
  semantic kind.
- Use several pairwise Claims: insufficient for a single N-member candidate;
  there is no common hypothesis identity or unavoidable full-set context.
- Place membership in ordinary qualifiers: rejected; old descriptive grants
  can revise qualifiers and must not acquire new reference authority.
- Make old structured JSON reference-bearing: rejected; that breaks its
  explicit inert-data contract and old grants.
- Introduce an array-valued semantic endpoint reader: semantically possible,
  but not selected. A qualified proposal is naturally Claim-scoped content,
  and the existing literal-with-Claim model already preserves that distinction.
  The chosen shape still needs explicit member edges and full-set assessment;
  ordinary binary display is not a substitute for them.

The focal and candidate identity stay immutable. Removing the focal requires
a successor or reformulated proposal; ordinary correction cannot quietly
change the referent. Member order is serialization order, not chronology.
The first motif execution is bounded to two through eight members by the
native-closure budget. Larger sets need an explicit bounded storage/continuation
contract; this limit is not a universal ontology or evidence of completed
corpus-scale motif processing.

## Authority and verification boundary

This decision is made within the Operator's Foundation v1 implementation
mandate. Current meaning belongs to `ToS/doctrine/CORPUS_FOUNDATION.md`,
`ToS/doctrine/semantic-interchange/`, and
`ToS/contracts/source-occurrence-motif-claim.schema.json`; growth, assessment
and access retain their existing owner boundaries. This record neither admits
a real hypothesis nor grants an assessor competence, publication rights,
runtime activation or canon authority.

Verification must cover malformed and mistyped members, omission of a later
participant, inert old/unknown fields, independently scoped focal membership,
creation, revision, replay and old-grant refusal, full native grounding,
source/graph/human preservation, compact-context completeness and local/Worker
parity. Real source-visible assessment and corpus-scale performance require
their own evidence; a synthetic green test does not establish either.
