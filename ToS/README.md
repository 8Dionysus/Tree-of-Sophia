# ToS Source Home

`ToS/` is the home organ for Tree of Sophia.

It gathers the old ToS-owned root layers into one tree-shaped topology so the
repository no longer has a separate root-level `tree/` competing with the ToS
home.

## Map

| Path | Use For | Stronger Than |
| --- | --- | --- |
| `doctrine/` | knowledge law, node contracts, templates, interpretation ladder, and authored route doctrine | review notes and generated readers |
| `source-witnesses/` | tracked Agent/Place/Organization and work/expression/edition/item catalog, explicit identity-ladder, chronology, responsibility, publication, and provision claims, immutable local payload route, fixity, provenance, rights, and source-facing witness material | intake, canon summaries, exports |
| `zarathustra/` | first golden growth kernel plus public-entry orientation for the bounded Zarathustra path | generic orientation and generated readers |
| `research-packets/` | non-authoritative research scaffolds and capture metadata | philosophy branch review |
| `philosophy/` | growing domain tree of philosophy: trunk, eras, regions, traditions, works, figures, concepts, transmissions, local graph workbenches | candidate intake and generated readers |
| `candidate-intake/` | provisional extraction and promotion ledgers | generated exports |
| `canon/` | canonical authored nodes, relations, and registries | public mirrors and derived exports |
| `public-compatibility/` | ToS-owned public-safe examples, mirrors, and tiny-entry compatibility | derived exports |
| `derived-exports/` | ToS-owned generated downstream-facing read models | nothing source-owned |
| `contracts/` | ToS-owned schemas and public structural contracts | generated payloads |
| `review-ledger/` | dated review and inspection notes | generated readers only |

## First Contour

This pass is a structural migration, not final ontology.
The first contour makes the source-home symmetry real while preserving the
current bounded Zarathustra route:

- current public root: `README.md`
- current tiny-entry surface: `ToS/public-compatibility/tos_tiny_entry_route.example.json`
- current capsule: `ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY.md`
- current source canon: `ToS/canon/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json`
- current bounded export: `ToS/derived-exports/kag_export.min.json`
- current source-gated lexical projection:
  `ToS/derived-exports/lexical-search/zarathustra-dta-first-editions-parts-1-4-v1.min.json`
- current source-returnable bibliographic claim graph:
  `ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json`
- current exact source-return query route:
  `scripts/query_source_witness_bibliographic_graph.py`

The next contour grows `ToS/philosophy/` as the domain-shaped philosophy
branch. It is the authored philosophical body whose local branches may later
prepare nodes and relation packs for review and promotion into `ToS/canon/`.

The two contours are coupled rather than sequential substitutes. The
world-philosophy branch prepares corpus soil; the Zarathustra kernel develops
the first full source-to-canon learning path. See
`ToS/zarathustra/GOLDEN_GROWTH_KERNEL.md` for the transfer and authority
boundary.

The corpus soil is specified by
`ToS/doctrine/CORPUS_FOUNDATION.md` and physically routed by
`ToS/source-witnesses/README.md`. Only item payload bytes are gitignored;
identity, catalogs, digests, provenance, rights, forensic evidence, anchors,
and review remain tracked. Search, vector, graph, and KAG surfaces are
rebuildable projections. The bibliographic graph keeps claims as explicit
nodes and does not flatten an unreviewed assertion into a direct fact edge.
Its read-only query route proves source parity before returning a claim bundle
and persists no result or judgment. The current source spine gives all 55
declared Work→Expression→Edition→Item steps their own claim IDs, evidence,
maker, provenance, and review posture while keeping textual equivalence open.
Each of the seven current Nietzsche Works also closes over its own documentary
`authored_by` claim; folder naming is navigation, never authorship evidence.
The exact 1913 Antonovsky Expression separately closes over one
`translated_by` claim whose evidence returns to a proposed, fixity-bound PDF
page-7 title-page anchor. The displayed initials do not become an accepted
expanded person identity, and the 1913, 1996, and 2007 Expressions remain
distinct.
Each Work separately closes over one first-publication chronology profile,
preserving staged/private/public/posthumous boundaries without inventing one
universal Work year or treating a model-made ordering claim as accepted truth.
The original bounded provision experiment keeps two Edition imprint statements
separate from one normalized Leipzig Place, two historical publisher
Organizations, and their statement-date facets. A separate post-experiment
extension admits only the exact 1883 first-part *Zarathustra* authority
statement, one Chemnitz Place, and the historical Schmeitzner publishing
Organization. A following exact-record pass independently admits the part-II
1883 and part-III 1884 statements from their own DTA records and an e-rara
holding crosscheck; it does not substitute the Person Ernst Schmeitzner or
infer either claim from part I, repeated labels, or the shared shelfmark. All
identities and claims remain
provisional or unreviewed, and the graph reaches them only through reified
claim nodes.

## Operating Card

| Field | Route |
| --- | --- |
| role | source-home entrypoint for ToS-authored philosophical work |
| input | source witnesses, Zarathustra route surfaces, research packets, domain branches, candidate extraction, canon objects, ToS-owned public examples, generated exports, contracts, doctrine, and review evidence |
| output | branch-shaped ToS surface with a visible owner, source posture, and validation route |
| owner | `ToS/AGENTS.md` and `ToS/source_home.manifest.json` |
| next route | witness or research packet -> zarathustra, philosophy, or candidate intake -> canon -> public compatibility -> derived export |
| tools | nearest nested `AGENTS.md`, source-home manifest, branch manifest, schema, generator, or validator |
| check | `python scripts/validate_tos_source_home.py` plus the owning branch validator |

## Boundary Routes

- ToS-owned active material routes into `ToS/`, not back into root-level active
  homes.
- Source-facing evidence routes to `source-witnesses/`; bounded Zarathustra
  route orientation routes to `zarathustra/`; non-authoritative research
  scaffolds route to `research-packets/`; domain growth routes to
  `philosophy/`; provisional extraction routes to `candidate-intake/`;
  authored nodes and relation packs route to `canon/`.
- Public examples route to `public-compatibility/`; generated read models route
  to `derived-exports/`; schemas route to `contracts/`.
- Current ToS knowledge law stays in `ToS/doctrine/`; operational process
  documents stay in `mechanics/`.
- AoA federation, runtime, SDK, memory, eval, and KAG-substrate authority stay
  with their owning repositories or layers.
