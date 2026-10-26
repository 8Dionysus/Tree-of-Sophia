# AGENTS.md

This card applies to `ToS/source-witnesses/`.

## Role

This branch owns primary witness, source-facing corpus identity and evidence: Works, Expressions, Editions,
Items, physical Artifacts, scholarly composites, exact representations, Claims,
fixity, provenance and rights. Read [Corpus Foundation](../doctrine/CORPUS_FOUNDATION.md)
for identity/address law and the exact object or Claim before changing it.
The [README](README.md) explains current source profiles and native operations.

## Boundary Routes

- Use the identity ladder `work -> expression -> edition -> item -> file` for bibliographic
  identity; `collections/` owns aggregate membership. Physical artifacts use
  `artifacts/<tradition>/<site>/<physical-identity>/`; documentary composites
  use `scholarly-composites/<genre-or-method>/<tradition>/<composition-identity>/`.
  Providers describe access and observed coverage, not durable identity.
- Keep originals immutable. Bind each correction, transcription, translation,
  alignment or other derivative to its exact input digest. Structural, quote
  and visual-region anchors belong to a specific representation.
- Authored Claims and provenance events own relations. Catalogs only navigate
  them. Public projection admits `public` or `public_metadata_only` packets;
  other visibility requires a reviewed public-safe derivative.
- Preserve the declared source profiles and exact outgoing-reference closure:
  the seven Nietzsche Works each have an explicit `authored_by` Claim and one
  `first_publication_chronology` packet. Different responsibility or chronology
  needs its own evidence-bearing design, not inference from path or year.
- Edition `provision_activity_claim_refs` retain literal statements, typed
  roles, places, agents, temporal assertions and review separately. A printer,
  modern successor or statement year cannot silently become publisher or
  public-release evidence.
- Work `expression_claim_refs`, Expression `embodiment_claim_refs` and Edition
  `exemplar_claim_refs` close over retained legacy streams or verified native
  Claims. Their union agrees with `work_ref`, `embodies_expression_refs` and
  `embodiment_ref`. Topology writes use their compound-operation grant;
  bibliographic embodiment does not establish textual equivalence.
- Native `translated_by` appends to the exact Expression with a separate Claim;
  Collection `contains_work` appends to the Collection without rewriting the
  Work. Preserve legacy streams and actual evidence/review state. Empty
  membership refs state absence of supplied Claims, not absence of members.
- Link and qualified association Claim have separate homes. Object-Link
  creation leaves its existing subject unchanged; additive v2 supports the
  declared Artifact domain while legacy v1 stays intact.
- `record-revisions.storage.json` and `metadata-transactions.storage.json`
  select exact immutable historical cuts for reads and restoration. Keep
  curated sources/contracts in Git; bulk retirement follows verified corpus
  membership and restore. Local payload custody, private R2 permissions and
  publication decisions remain separate obligations.
- Preserve source/working/bridge language roles, translator/editor attribution
  and uncertainty. Interpretation returns to doctrine, research, candidate
  intake, philosophy or canon; source evidence remains here.
- Select explicit data artifacts for catalogs and native operations. Extraction
  runtimes, model caches and large working derivatives stay outside Git;
  consumers own their orchestration.

## Validation

Select `source_home` in [ToS/VALIDATION.md](../VALIDATION.md) for object/Claim
closure, source return, fixity, rights shape and catalog parity. Add the public
entry/export route only when that companion changes. The source-foundation validator checks mechanics; source witness identity,
textual fidelity, translation, rights and relation truth require owner review.
