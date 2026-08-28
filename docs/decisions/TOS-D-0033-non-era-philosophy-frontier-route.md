# Non-Era Philosophy Frontier Route

## Index Metadata

- Decision ID: TOS-D-0033
- Original date: 2026-08-27
- Surface classes: philosophy/topology, research-packet, scripts/validation, docs/route-law
- ToS layers: philosophy, research-packets, scripts, docs
- Tree classes: frontier, era, region, tradition, dossier
- Guard families: chronology boundary, provenance preservation, manual review, source-first authority
- Posture: accepted

## Context

The complete Table I A-series intake adds eighteen prepared dossiers. Seventeen
can be given bounded era/region/tradition homes, although the low-confidence
writing-frontier rows still require manual review. A48 cannot: it deliberately
combines Andean khipu with Rapa Nui rongorongo, and rongorongo lies outside the
Table I chronology. Treating that row as one era or tradition would turn a
research guardrail into a false historical classification.

The evidence for this decision is the non-authoritative operator-local DOCX
packet, its tracked fixity/coverage records, the Table I row spine, and the
current philosophy route law. Those surfaces motivate the topology; they do
not establish source, semantic, or canon authority.

## Decision

Add `ToS/philosophy/frontiers/` as a narrow non-era philosophy route. Use it
only when a prepared case cannot be represented honestly as one
era/region/tradition because chronology, geography, readability, or evidence
posture is itself the object under review.

Route A48 to
`frontiers/writing-and-information-thresholds/khipu-rongorongo-comparative-boundary`.
Keep khipu and rongorongo as distinct component cases, state that rongorongo is
outside Table I chronology, and require manual review. The route is a
pre-canon guardrail, not a tradition, source witness, readable philosophical
corpus, or canon claim.

## Options Considered

- Force A48 into an era/region/tradition path. Rejected because no single era
  or region describes both cases and the path would conceal the exact anomaly.
- Split the dossier during mechanical planting. Rejected because that would
  rewrite the supplied packet and invent two reviewed dossiers without an
  owner-visible normalization decision.
- Leave A48 only in the research packet. This preserves caution but loses the
  requested tree-visible guardrail and makes the chronology problem easier for
  downstream readers to miss.
- Add one bounded non-era frontier route with explicit constraints and manual
  review. Accepted.

## Rationale

Tree-first orientation requires honest roots, not era-shaped folders at any
cost. A frontier route makes the reason for non-placement visible while
preserving the stronger owner chain: packet evidence remains research-only;
the route map owns current placement; human review owns any later split,
historical interpretation, or promotion.

The selected decision lenses are owner/source, placement, evidence state,
lifecycle, and handoff. They keep the ADR weaker than current philosophy route
law, keep generated branches and exports derived, and make the manual-review
stop line visible.

## Consequences

The philosophy root now has one exceptional topological family alongside the
primary era body. Validators and projections must accept that explicit route
without generalizing every uncertain dossier into a frontier. The route map
must carry `route_kind`, review posture, and A48 constraints so regeneration
cannot erase them.

A future human review may split A48 into separate Andean and Rapa Nui packets,
move either case to a better owner, or defer it entirely. Such a review must
change the source route first and regenerate companions. This decision does
not review the DOCX claims, verify citations, accept labels as historical
truth, or promote any node or relation.

## Source Surfaces

- `ToS/philosophy/AGENTS.md`
- `ToS/philosophy/README.md`
- `ToS/philosophy/philosophy.manifest.json`
- `ToS/philosophy/atlas/dossiers/prepared-dossier-routes.json`
- `ToS/research-packets/deep-research/philosophy/packet-contract.md`
- `ToS/research-packets/deep-research/philosophy/dossiers/table-i-docx-intake.manifest.json`
- `ToS/research-packets/deep-research/philosophy/dossiers/table-i-docx-extraction-coverage.json`

## Validation

Run the decision index generator/check and decision validator, then the
prepared-dossier tests, philosophy topology, atlas/graph generated parity, and
the repository release lane. Green mechanics do not satisfy the manual review
required by A44–A48 or create source/canon acceptance.
