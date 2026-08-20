# Artifact Witness Spine

## Index Metadata

- Decision ID: TOS-D-0024
- Original date: 2026-08-12
- Surface classes: source-witness, domain-topology, contracts, research, docs/route-law
- ToS layers: source-witnesses, philosophy, research-packets, contracts, derived-exports, docs
- Tree classes: source, witness, context, claim, relation, growth route
- Guard families: source-first authority, identity separation, provenance, rights, semantic boundary, projection boundary
- Posture: accepted

## Context

The philosophy atlas names works, corpora, inscriptions, archives, and source
witnesses as first-class growth material. The existing source-witness corpus,
however, is optimized around the bibliographic
`work -> expression -> edition -> item -> file` ladder. Planting A01's exact
`P000015` clay tablet exposes a real category boundary: a physical ancient
artifact is not a Work, Expression, Edition, or library Item, and its current
CDLI record is not the physical object.

Forcing the tablet into the Nietzsche-shaped ladder would silently merge its
physical identity with modern catalog, inscription, composite, photograph,
line-art, and interpretive layers. Keeping only a URL in a backlog would avoid
that error but would leave the atlas without a stable, rights-aware source
foundation.

## Decision

Add a separate tracked artifact-witness identity family rooted at
`ToS/source-witnesses/artifacts/`. Route it by physical, excavation, or custody
identity rather than by digital provider. Link exact philosophy backlog rows
to those records through branch-local source plantings.

Every artifact packet must keep the physical object, mutable digital catalog
record, inscription or transliteration, scholarly composite, photograph, line
art, rights record, and interpretive classification distinct. The initial
packet may publish public-safe metadata only. It grants no source-text,
semantic, graph, canon, or publication authority.

## Options Considered

- Force ancient artifacts into `work -> expression -> edition -> item`. This
  reuses mature machinery but creates false bibliographic identities and
  destroys layer boundaries.
- Keep artifact sources only as external links in atlas backlogs. This avoids
  bad modeling but leaves no stable ToS identity, currentness evidence,
  provenance, rights separation, or source-returnable planting.
- Add a parallel artifact-witness spine and connect it through branch-local
  source plantings. This preserves physical identity while allowing works,
  corpora, inscriptions, archives, and later sign layers to connect without
  becoming one object.

## Rationale

ToS needs stable identifiers before it can build trustworthy relations, but
stability must attach to the right kind of thing. A provider-independent
artifact ID can survive catalog URL and metadata revisions. Exact provider
records can then remain dated evidence rather than hidden identity owners.

This shape also prepares the semantic future without claiming it. A later sign
system can cite an inscription version, visual region, catalog state, and
physical witness separately. One sign ID may stabilize reference while
multiple readings, uses, periods, and interpretations remain visible.

The branch-local planting is the tree-facing seam. It records which source
need is now grounded and which risks remain. The artifact packet is the
source-facing authority. Generated corpus, graph, and KAG views may index this
route later, but none becomes the source of truth.

## Consequences

The first real A01 source can be planted without calling a tablet a book or
calling accounting philosophy. Other inscriptions, archaeological objects,
and manuscript objects can reuse the category when they share the same
physical-versus-digital pressure.

Artifact packets must remain content-free when rights or exact source identity
has not been handled. Images, line art, transliterations, translations, and
composites require their own acquisition, fixity, rights, quality, and review
routes. Public availability never alone establishes redistribution.

Source planting does not consume or delete the backlog row. It binds the row
to a source witness and records remaining controls. No generated graph edge,
semantic label, catalog classification, or validator green state may promote
the artifact to philosophy or canon.

This decision creates no human task, source-text admission, semantic claim,
graph fact, canon promotion, public payload, external message, or server
transfer.

## Source Surfaces

- `CHARTER.md`
- `BOUNDARIES.md`
- `ToS/doctrine/CORPUS_FOUNDATION.md`
- `ToS/doctrine/NODE_CONTRACT.md`
- `ToS/philosophy/atlas/multilingual/text-bearing-nodes.contract.json`
- `ToS/philosophy/eras/bronze-age/regions/west-asia/traditions/proto-cuneiform-accounting-ontologies/sources/source-anchor-backlog.jsonl`
- `ToS/source-witnesses/README.md`
- `ToS/source-witnesses/artifacts/README.md`
- `ToS/contracts/artifact-source-witness.schema.json`
- `ToS/contracts/philosophy-source-planting.schema.json`
- `ToS/research-packets/foundation-laboratory-2026-07/A01_CDLI_P000015_SOURCE_PLANTING_RESEARCH_2026-08-12.md`

## Validation

Decision-index regeneration routes through `docs/decisions/AGENTS.md`.
Checked index parity and record validation are owned by the
`generated_parity` sequence in `docs/validation/validation_lanes.json`.

The source-witness foundation validator checks artifact schema, path identity,
layer separation, rights, discovery, provenance, and exact reference closure.
The philosophy topology validator checks branch-local planting schema,
backlog-line identity, source-witness closure, and negative authority gates.
Neither validator proves archaeological interpretation, sign meaning,
philosophical status, rights beyond the recorded evidence, or human review.
