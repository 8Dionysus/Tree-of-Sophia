# Corpus Evidence Spine And Witness Storage

## Index Metadata

- Decision ID: TOS-D-0020
- Original date: 2026-07-22
- Surface classes: source-home, source-witness, contracts, docs/route-law, storage-boundary
- ToS layers: doctrine, source-witnesses, research-packets, contracts, candidate-intake, derived-exports
- Tree classes: source, claim, relation
- Guard families: source-first authority, provenance, rights, storage boundary, projection boundary, manual review
- Posture: accepted

## Context

The Zarathustra golden growth kernel requires more than a bounded trilingual
example. Tree of Sophia needs a corpus floor able to distinguish the work,
language expression, translation responsibility, edition, aggregate
collection, acquired item, immutable file, addressable passage, observation,
claim, and review event.

Three local seed files expose why this distinction matters. One is a
vector-outline PDF with no text layer, one is an ABBYY image/OCR collection
whose shallow file probe reports the wrong page count, and one is an automatic
Internet Archive OCR EPUB with weak metadata and no usable navigation. Their
filenames and embedded metadata are leads, not sufficient bibliographic or
semantic truth.

The repository also needs to hold private or redistribution-constrained source
bytes locally without committing them, while retaining a public, reviewable
index of exactly what grounded later work.

## Decision

Adopt a corpus evidence spine under `ToS/source-witnesses/` with a local
LRM-shaped identity ladder:

`work -> expression -> edition -> item -> file`.

Aggregate publications route through a sibling `collections/` tree with
evidence-bearing membership claims. Stable ToS IDs own identity; speaking
filesystem paths own navigation and may be migrated without changing IDs.

Only bytes under an item's `payload/` directory are ignored by Git. Catalogs,
object records, fixity, acquisition and transformation provenance, rights and
visibility posture, forensic reports, anchors, and reviewed source-near text
remain tracked. Raw bytes are immutable after intake. Large laboratory
derivatives and model/runtime caches stay with `abyss-stack` and host storage,
not in the corpus tree.

The semantic floor is also layered. Anchored occurrences and forms may be
source-near observations; lemmatization, recurrence membership, etymology,
translation, sense, motif, concept, and relation are separately identified,
versioned assertions. Human review events own promotion. Search, vector,
graph, KAG, and UI surfaces remain replaceable projections.

## Options Considered

- Store all books in one ignored `books/` folder and track one filename list.
  This is simple but collapses work, edition, item, rights, and provenance and
  makes later source return depend on filenames.
- Store source bytes in Git or Git LFS. This makes clones heavy, leaks
  redistribution-constrained material, and binds preservation to repository
  hosting policy.
- Store only external links. Links decay, access changes by jurisdiction, and
  later OCR/translation claims cannot prove which bytes they saw.
- Organize everything only by author and title. This is readable for the first
  seed but fails on anonymous, disputed, collective, translated, and aggregate
  material.
- Use a speaking work/expression/edition/item tree plus tracked global
  catalogs, stable IDs, collection routes, and narrowly ignored item payloads.
  This preserves human navigation and machine identity without pretending the
  first folder contour is permanent ontology.

## Rationale

The chosen route makes the filesystem tell the truth visible at its current
resolution while keeping identity independent of that filesystem. It allows
the corpus to be reorganized as understanding improves without breaking
anchors, claims, or external references.

The tracked/public and local/private split is narrow enough to audit. A future
agent can see an item's digest, source, rights, transformations, and unresolved
metadata even when it cannot receive the payload. A future server can ingest
the same item through a manifest/checksum receipt rather than a hidden manual
copy.

Separating observation from interpretation gives stable entities where the
evidence genuinely supports stability and versioned interpretive entities
where philosophy requires contestability. It also makes Zarathustra useful as
a transferable method without making Nietzsche's ontology universal.

## Consequences

The existing bounded Zarathustra witness path should migrate into the new
`works/.../alignments/` route rather than remain a second source-witness head.
Active references and validators move with it; dated historical review notes
may retain their original path evidence.

Every new payload intake needs an item record, SHA-256, provenance event,
rights posture, forensic report, and catalog entry. Unknown or conflicting
bibliography and rights remain explicit states.

Validators may prove catalog/object/payload mechanics. They cannot determine
that an edition identification, OCR text, translation, etymology, concept, or
relation is correct. Those decisions require source-visible manual review and
recorded rationale.

Graph databases and annotation tools may be replaced without migrating
authority out of the tracked claim/review records.

## Source Surfaces

- `DESIGN.md`
- `README.md`
- `AGENTS.md`
- `ToS/doctrine/CORPUS_FOUNDATION.md`
- `ToS/doctrine/KNOWLEDGE_MODEL.md`
- `ToS/source-witnesses/README.md`
- `ToS/source-witnesses/AGENTS.md`
- `ToS/research-packets/foundation-laboratory-2026-07/`
- `ToS/contracts/`
- `scripts/validate_source_witness_foundation.py`
- `docs/decisions/TOS-D-0019-zarathustra-golden-growth-kernel.md`

## Validation

Decision-index regeneration routes through `docs/decisions/AGENTS.md`.
Catalog, source-foundation, source-home, and route-card checks route through
`scripts/AGENTS.md` and the release lane declared by the validation manifest.
The review checklist remains required for bibliographic, rights, translation,
and semantic judgments outside validator authority.
