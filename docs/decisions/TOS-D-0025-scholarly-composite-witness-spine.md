# Scholarly Composite Witness Spine

## Index Metadata

- Decision ID: TOS-D-0025
- Original date: 2026-08-12
- Surface classes: source-witness, domain-topology, contracts, research, docs/route-law
- ToS layers: source-witnesses, philosophy, research-packets, contracts, derived-exports, docs
- Tree classes: source, witness, composite, context, claim, relation, growth route
- Guard families: source-first authority, identity separation, provenance, rights, semantic boundary, projection boundary
- Posture: accepted

## Context

The first A01 planting established a provider-independent identity for the
physical tablet `P000015`. The next exact backlog row points from that tablet
to DCCLT `Q000023`, *Archaic Vessels and Garments*. ORACC defines a Q-text as
a documentary composite rather than physical-object structure. CDLI exposes
the same modern composite as `P471693` with a current witness list and direct
P000015 membership relation.

The artifact packet correctly kept its composite layer separate, but ToS had
no tracked identity family for the composite itself. Treating Q000023 as the
tablet would merge a scholarly reconstruction with one physical witness.
Treating it as a Nietzsche-shaped Work, Expression, Edition, or Item would
invent a bibliographic publication ladder that does not describe this source
object. Leaving it only as an external URL would lose stable identity,
representation differences, rights, provenance, and exact branch planting.

## Decision

Add a separate tracked scholarly-composite witness family rooted at
`ToS/source-witnesses/scholarly-composites/`. Route its speaking path by the
composition, genre or method, and tradition rather than a current provider.
Give the composition a persistent `tos.composite.*` identity while retaining
Q-numbers, P-numbers, provider records, APIs, and page paths as sourced
coordinates.

A composite record must keep these layers distinct:

- the modern scholarly composition identity;
- each mutable provider representation;
- every physical or textual member witness;
- member transliterations and physical structure;
- reconstructed composite lines and documentary coordinates;
- translations, sign readings, glosses, and interpretations;
- third-party images, line art, PDFs, and publication plates;
- philosophy, graph, and canon effects.

Provider coverage differences are first-class dated observations. A shorter
visible member list is not silently declared complete, and an absent member on
that page is not promoted into a negative fact when a stronger exact API
relation reports membership.

## Options Considered

- Reuse the physical artifact identity. This destroys the distinction between
  the surviving object and a modern reconstruction assembled from many
  objects.
- Force the composite into `work -> expression -> edition -> item -> file`.
  This reuses mature book machinery but invents inappropriate bibliographic
  entities and hides exemplar membership.
- Keep only provider URLs in the artifact record or atlas backlog. This keeps
  layers separate but provides no stable ToS identity, rights route,
  provenance, currentness comparison, or exact planting.
- Add a separate scholarly-composite spine linked to physical artifacts by
  evidence-bearing member observations. This preserves both identity families
  and lets later text, variant, sign, semantic, and graph work cite the exact
  layer it used.

## Rationale

The foundational invariant is not that every source fits one universal
ontology. It is that stable identity attaches to the correct referent and that
later interpretation can always return to the layer that supplied its
evidence.

Scholarly composites are valuable because they stabilize documentary
coordinates and expose relationships across witnesses. The same power makes
them dangerous if their editorial completeness is mistaken for an ancient
original or their line labels are mistaken for physical layout. A separate
identity family preserves the benefit without erasing mediation.

This shape also supports future semantic work. A sign occurrence may cite one
physical member, its exact transliteration version, a composite coordinate,
and competing readings independently. The stable composite ID does not make a
reading, translation, sign identity, or concept stable by decree.

## Consequences

The philosophy source-planting contract now admits either a physical artifact
or a scholarly composite witness and resolves each to its own spine. The
discovery contract admits `scholarly-composite` targets and `tos.composite.*`
identifiers. The source-foundation and philosophy-topology validators must
check the new schema, path, reference, rights, discovery, provenance, member,
and planting closure without pretending that a green result validates the
edition.

DCCLT's explicit CC BY-SA 3.0 license is preserved as a positive future
publication route for its authored content. It does not apply automatically to
linked third-party media. The first Q000023 packet remains text-free and
public-metadata-only while later exact-version acquisition is compared through
the prepared A/B/C laboratory.

The decision creates no source-text admission, philological judgment, human
task, sign reading, translation, semantic claim, graph fact, canon promotion,
external message, or server transfer.

## Source Surfaces

- `CHARTER.md`
- `BOUNDARIES.md`
- `ToS/doctrine/CORPUS_FOUNDATION.md`
- `ToS/source-witnesses/README.md`
- `ToS/source-witnesses/scholarly-composites/README.md`
- `ToS/contracts/scholarly-composite-witness.schema.json`
- `ToS/contracts/philosophy-source-planting.schema.json`
- `ToS/contracts/material-discovery-record.schema.json`
- `ToS/research-packets/foundation-laboratory-2026-07/A01_DCCLT_Q000023_SCHOLARLY_COMPOSITE_RESEARCH_2026-08-12.md`
- `ToS/philosophy/eras/bronze-age/regions/west-asia/traditions/proto-cuneiform-accounting-ontologies/sources/source-anchor-backlog.jsonl`

## Validation

Decision-index regeneration routes through `docs/decisions/AGENTS.md`.
Checked index parity and record validation remain owned by the repository's
generated-parity lane.

The source-foundation validator checks composite schema, path identity,
member-reference closure, coverage observations, rights, discovery,
provenance, and content-free posture. The philosophy-topology validator checks
the exact composite planting and backlog-line closure. Neither proves that the
composite is philologically correct, complete, ancient, philosophical,
semantically stable, accepted for publication, or canonical.
