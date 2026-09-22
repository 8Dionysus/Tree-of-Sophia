# AGENTS.md

This file applies to primary witness and source files under
`ToS/source-witnesses/`.

## Role

`ToS/source-witnesses/` holds the corpus evidence spine that grounds authored
ToS routes: bibliographic work/expression/edition/item identity, parallel
physical-artifact identity, immutable local payloads, primary-language text,
translations, aligned witnesses, fixity, provenance, rights, and forensic
evidence.

## Operating Card

| Field | Route |
| --- | --- |
| role | primary witness and source-facing evidence surface |
| input | acquired item, primary-language text, translation, collection membership, source-page metadata, or provenance/rights evidence |
| output | addressable reviewable witness surface with explicit identity, fixity, source, and rights posture |
| owner | `ToS/source-witnesses/AGENTS.md` and nearest exact object/claim source; generated catalogs provide navigation |
| next route | source witness -> `ToS/philosophy/` branch or `ToS/candidate-intake/` pass -> `ToS/canon/` review |
| tools | manual corpus gate, source route docs, witness manifests |
| check | route validator when the witness feeds a current public or export surface |

## Boundary Routes

- Open `README.md` when its human provenance or usage explanation is relevant;
  use `ToS/doctrine/CORPUS_FOUNDATION.md` for corpus law before adding or
  moving corpus material.
- Use the identity ladder `work -> expression -> edition -> item -> file`.
  Route multi-work publications through `collections/` and evidence-bearing
  membership claims.
- Route a physical artifact through
`artifacts/<tradition>/<site>/<physical-identity>/`, using its physical
referent for durable identity and its provider records for access and
representation.
- Route a modern documentary, critical, or synoptic composite through
  `scholarly-composites/<genre-or-method>/<tradition>/<composition-identity>/`.
  Keep its durable identity independent of the provider and retain member
  assertions and representation coverage as dated evidence.
- Keep physical artifact, catalog record, inscription or transliteration,
  scholarly composite, photograph, line art, and interpretation as separate
  layers. Readings, translations, semantic Claims and canon state require their own
source grounds and review.
- Keep a composite, its physical members, member transliterations, editorial
  lines, translations, and provider pages separate. Retain each provider list’s observed coverage alongside the exact source-owned
membership Claims.
- Treat paths as navigation and stable ToS IDs as identity. Never merge two
  objects only because their paths, titles, translators, or sampled text look
  similar.
- Treat authored claim packets and provenance events as relation authority.
  The generated claim catalog is source-returnable navigation only. Project
  only `public` or `public_metadata_only` claims; fail closed on less-visible
  packets until a reviewed public-safe derivative exists.
- Keep every Work in the current `works/friedrich-nietzsche/` corpus closed
  over its explicit `authored_by` packet to the Nietzsche Agent. Do not infer
  this claim from the responsibility path. A future anonymous, disputed,
  collaborative, or differently authored Work needs its own evidence-bearing
  responsibility design instead of inheriting the bounded Nietzsche rule.
- Keep those seven Works closed over exactly one source-owned
  `first_publication_chronology` packet. Preserve interval start and end,
  stage sequence, precision, availability, and the ordering warning; never
  replace this profile with an untyped Work year. Composition, printing,
  title-page, public-sale, posthumous editorial, reception, and digitization
  time require distinct claims.
- Keep Edition `provision_activity_claim_refs` in exact sibling-file closure.
  A provision activity groups its literal statement, typed activity, role-
  specific places and agents, temporal assertion, evidence, provenance, and
  review posture. Preserve transcription separately from normalized Place or
  Organization identity; never infer a publisher from an Edition label,
  substitute a printer or modern successor, turn a statement year into a
  public-release date, or emit a direct Edition-to-identity graph fact.
- Keep the declared identity ladder and its outgoing claim refs in exact
  closure: Work `expression_claim_refs`, Expression
  `embodiment_claim_refs`, and Edition `exemplar_claim_refs` must resolve to
  the retained three legacy streams under `relations/` or a declared native
  `has_expression`, `embodied_by`, or `exemplified_by` Claim with its verified
  compound publication evidence.
  Their union must agree with `work_ref`, `embodies_expression_refs`, and
  item-manifest `embodiment_ref`. Topology writes require the declared compound-operation grant and preserve
legacy batches. Textual equivalence requires separate comparison evidence.
- Keep curated authored sources and contracts in Git. Bulk imported records,
  claims, manifests, fixity, provenance, rights and review evidence belong to
  explicit immutable corpus revisions after exact preservation and verified
  restore. Their source authority is unchanged by the storage location.
  Build catalogs and projections into the selected data artifact. Preserve
  permanent local payload custody and each actual private R2 permission;
  publication requires its own rights and owner decisions.
- Preserve original bytes. OCR, correction, normalization, segmentation, and
  translation are new versioned layers and must cite the input digest.
- Use structural + quote + digest + visual-region anchors; bind positions to their exact source representation and digest.
- Keep witness material distinct from philosophy branches, intake tables, canon
  nodes, and public mirrors.
- Keep canonical-source, working-translation, and bridge-translation posture
  explicit.
- Preserve translator, editor, donor, and uncertainty notes where they matter.
- Responsibility references close over unchanged legacy carriers and explicitly
  verified native attachments. Native `translated_by` uses a separate Claim home
  and exact Expression append, never the immutable `has_expression` stream.
  Each attribution retains its actual evidence and review state alongside
endpoint metadata bindings.
- Keep Collection membership refs in exact closure over retained legacy and
  verified native `contains_work` Claims. Native attachment appends to the
  Collection and publishes a separate Claim; the existing Work and legacy
  membership streams remain unchanged. Empty initial refs record that this source record currently supplies no
membership Claims.
- Keep Link and its qualified association Claim in separate exact homes.
  Native object-Link creation leaves its existing subject unchanged and uses
  additive v2 Claims for the explicit Artifact-inclusive domain. Legacy v1
  remains intact; rights decisions remain bound to the actual source and permitted use.
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
export validators. Select the `source_home` route in
[`ToS/VALIDATION.md`](../VALIDATION.md) after the witness object or claim
surface is known.
