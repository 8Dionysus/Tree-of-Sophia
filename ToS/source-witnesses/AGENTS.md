# AGENTS.md

This file applies to primary witness and source files under
`ToS/source-witnesses/`.

## Role

`ToS/source-witnesses/` holds the corpus evidence spine that grounds authored
ToS routes: work/expression/edition/item identity, immutable local payloads,
primary-language text, translations, aligned witnesses, fixity, provenance,
rights, and forensic evidence.

## Operating Card

| Field | Route |
| --- | --- |
| role | primary witness and source-facing evidence surface |
| input | acquired item, primary-language text, translation, collection membership, source-page metadata, or provenance/rights evidence |
| output | addressable reviewable witness surface with explicit identity, fixity, source, and rights posture |
| owner | `ToS/source-witnesses/AGENTS.md`, `README.md`, nearest object/claim record, and generated tracked catalog |
| next route | source witness -> `ToS/philosophy/` branch or `ToS/candidate-intake/` pass -> `ToS/canon/` review |
| tools | manual corpus gate, source route docs, witness manifests |
| check | route validator when the witness feeds a current public or export surface |

## Boundary Routes

- Read `README.md` and `ToS/doctrine/CORPUS_FOUNDATION.md` before adding or
  moving corpus material.
- Use the identity ladder `work -> expression -> edition -> item -> file`.
  Route multi-work publications through `collections/` and evidence-bearing
  membership claims.
- Treat paths as navigation and tracked IDs as identity. Never merge two
  objects only because their paths, titles, translators, or sampled text look
  similar.
- Treat authored claim packets and provenance events as relation authority.
  The generated claim catalog is source-returnable navigation only. Project
  only `public` or `public_metadata_only` claims; fail closed on less-visible
  packets until a reviewed public-safe derivative exists.
- Keep the declared identity ladder and its outgoing claim refs in exact
  closure: Work `expression_claim_refs`, Expression
  `embodiment_claim_refs`, and Edition `exemplar_claim_refs` must resolve to
  the three owned files under `relations/` and agree with `work_ref`,
  `embodies_expression_refs`, and item-manifest `embodiment_ref`. Never infer
  textual equivalence from this bibliographic topology.
- Keep only item `payload/` content gitignored. Track manifest, SHA-256,
  provenance, rights, forensic report, and catalog entry.
- Preserve original bytes. OCR, correction, normalization, segmentation, and
  translation are new versioned layers and must cite the input digest.
- Use structural + quote + digest + visual-region anchors; offsets alone are
  not a durable source address.
- Keep witness material distinct from philosophy branches, intake tables, canon
  nodes, and public mirrors.
- Keep canonical-source, working-translation, and bridge-translation posture
  explicit.
- Preserve translator, editor, donor, and uncertainty notes where they matter.
- Route commentary to doctrine, review, candidate intake, philosophy, or canon
  according to owner.
- Route extraction runtimes, model caches, benchmarks, and large working
  derivatives to the `abyss-stack` laboratory and host storage owners.

## Validation

Use the source-foundation validator for object/claim catalog parity,
source-line/digest return, object/claim closure, payload-ignore, fixity, rights,
and reference mechanics. Use the review checklist for source identity,
edition, relation truth, translation, rights, or interpretation judgments. If
the witness participates in the current bounded route, also use tiny-entry and
export validators.
