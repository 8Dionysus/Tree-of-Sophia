# Corpus Foundation

This document defines the durable evidence floor beneath Tree of Sophia.

It is knowledge law, not an ingestion procedure. Physical routes and payload
handling are owned by `ToS/source-witnesses/`; repeatable extraction and
experiments are owned by mechanics and `abyss-stack`.

## Foundation thesis

The first stable layer of philosophy is not a final inventory of concepts. It
is the ability to identify a source, return to the exact evidence, distinguish
what was observed from what was inferred, and retain the history of judgment.

```text
identity -> fixity -> address -> observation -> assertion -> review
```

Semantic growth begins on this floor. It is not collapsed into it.

## Four stability postures

| Posture | Meaning | Examples |
| --- | --- | --- |
| immutable | the recorded value never changes; a change creates another object/event | acquired file bytes, SHA-256 digest, signed receipt, review event |
| persistent identity | the identifier remains while correctable descriptions accumulate | work, expression, edition, item, passage, occurrence, lexeme, sign, concept, relation, annotation, claim |
| versioned assertion | content may be superseded without erasing lineage | title attribution, date, lemma, etymology, translation, concept boundary, relation |
| derived projection | safely rebuildable from stronger tracked surfaces | search index, vector index, graph store, KAG export, visualization |

An accepted assertion is not rewritten into an immutable fact. Acceptance is
an immutable review event pointing to a versioned assertion.

## Corpus identity ladder

ToS uses an LRM-shaped local profile without claiming full IFLA conformance.

| Class | Meaning in ToS | Must not be confused with |
| --- | --- | --- |
| `work` | an intellectual creation recognized as one work | a file, edition, or title string |
| `expression` | one language/textual responsibility state of a work, including a translation | every copy carrying it |
| `edition` | a published or edited manifestation that embodies one or more expressions | one acquired scan or download |
| `item` | one physical or digital copy/container as acquired | its metadata record or every file extracted from it |
| `file` | one immutable byte sequence with media type and digest | the work itself |
| `collection` | an aggregate publication or container holding multiple works/expressions | a single contained work |

Authorship, translation responsibility, edition identity, date, place, and
container membership are claims with evidence status. A filename may seed a
lead but cannot settle any of them.

Responsibility claims retain their role-specific subject and Agent object:
Work author, Expression translator, Edition editor, paratext author, designer,
publisher, copyist, corrector, and rights holder are not interchangeable
variants of a generic creator field. A role not yet admitted by the governing
schema and validator remains an explicit research need rather than an
untyped edge.

## Identifier law

Corpus identifiers use the local family:

```text
tos.<class>.<stable-local-name>
```

where `<class>` is one of `agent`, `work`, `expression`, `edition`,
`collection`, `item`, `file`, `passage`, `region`, `anchor`, `occurrence`,
`lexeme`, `annotation`, `sign`, `concept`, `claim`, `relation`, `rights`,
`review`, or `event`.

Rules:

- an ID is never reused for a different referent;
- an ID does not depend on the current filesystem path;
- labels, transliterations, dates, and attribution may change without changing
  the ID when the referent is continuous;
- a materially different referent receives a new ID and an explicit relation;
- file IDs are content-addressed by a declared digest;
- aliases and external identifiers are properties with source and confidence,
  not replacements for the ToS ID;
- unresolved identity is explicit; objects are not merged merely because
  titles, translators, or text samples look similar.

## Address law

Every source-bearing assertion returns to at least one anchor. An anchor is a
bundle, not a naked character offset.

The preferred bundle contains:

1. anchor ID and exact item/file version;
2. structural passage path, when defensible;
3. exact selected text with prefix and suffix context;
4. character/token positions as accelerators, not identity;
5. page or IIIF-like Canvas identity;
6. page-region coordinates when visual evidence exists;
7. selector method/version and extraction provenance;
8. status: proposed, verified, superseded, unresolved, or rejected.

If OCR changes, the visual region and old selector remain. A new text anchor
may supersede the old one while preserving the relationship.

## Text-bearing layers

The following layers never overwrite one another:

1. immutable source bytes;
2. reproducible rendered page or unpacked container member;
3. diplomatic transcription or raw OCR;
4. reviewed source text;
5. explicitly normalized text;
6. structural segmentation;
7. translation or aligned witness;
8. lexical and semantic annotation;
9. reviewed claim;
10. search, graph, and downstream projections.

Unicode NFC belongs only in an explicitly normalized layer. Historical
spelling, typography, punctuation, whitespace, glyph uncertainty, and OCR
errors remain visible in source-near layers.

## Sign ladder

ToS treats a sign as a layered family of addressable records rather than one
timeless semantic entity.

| Layer | Object | Posture |
| --- | --- | --- |
| occurrence | exact glyph/token sequence at an anchor | source-near observation |
| form | spelling, case, punctuation, typography | observation or reviewed transcription |
| linguistic analysis | lemma, morphology, compound segmentation | versioned proposal |
| recurrence | membership in a declared occurrence set | reproducible result plus review |
| sense | contextual lexical meaning | interpretive assertion |
| etymology | historical/formative account with sources | scholarly assertion |
| translation | aligned target rendering and tension | versioned translation assertion |
| motif/sign | cross-passage recurrence or image | interpretive proposal |
| concept | philosophical abstraction | reviewed, contestable claim family |

The identity of a record may be stable while its interpretation is corrected.
No model may silently lift an occurrence into a concept.

The semantic identities remain distinct:

- `occurrence_id` identifies one addressable appearance in an exact witness;
- `lexeme_id` identifies a linguistic normalization whose membership remains
  a versioned claim;
- `sign_id` is assigned only after an evidence-bearing human promotion
  decision over a concrete candidate;
- `concept_id` identifies a contestable interpretation family, not a hidden
  synonym for a sign;
- `claim_id` identifies one versioned assertion with maker, time, method,
  evidence, alternatives, uncertainty, and review;
- `relation_id` identifies one typed relation record whose claim and evidence
  remain separately resolvable.

Before sign promotion, the stable candidate identity is an `annotation_id` or
`claim_id`, never a prematurely minted `sign_id`. Labels remain mutable and do
not determine any of these identities.

## Assertion layers

Each annotation or claim declares exactly one primary layer:

- `forensic_observation`;
- `bibliographic_assertion`;
- `textual_observation`;
- `linguistic_analysis`;
- `translation_alignment`;
- `translation_judgment`;
- `semantic_interpretation`;
- `scholarly_report`;
- `lived_witness`;
- `canon_judgment`.

An assertion may cite another layer but cannot disguise its own posture. Lived
witness may explain sustained attention and salience; it cannot settle source
text, bibliography, etymology, or necessary meaning.

## Claim law

A claim records:

- its own ID and version;
- typed subject, predicate, and object/body;
- exact source anchors and supporting material;
- assertion layer and epistemic status;
- maker and method, including software/model/configuration where applicable;
- creation time and rights/visibility constraints;
- alternatives, counterevidence, and relations to prior versions;
- human review state, reviewer, rationale, and decision event.

Allowed review states include `unreviewed`, `accepted`, `accepted_with_limits`,
`rejected`, `ambiguous`, `deferred`, and `superseded`.

Confidence is the maker's declared uncertainty, not an objective probability
that the claim is true.

A generated claim catalog may expose subject, predicate, object, evidence,
maker, provenance, review posture, exact source line, and canonical source
digest for query and graph preparation only when the claim's visibility
permits that tracked projection. It remains a projection of the authored claim
packet: catalog presence, projection parity, or graph emission cannot accept,
reject, or reinterpret the claim.

## Translation law

A translation is an expression or expression proposal, never a language field
on the source string. Its packet must be able to retain:

- verified original segment and context;
- diplomatic and normalized forms;
- morphology, literal gloss, etymological sources, and ambiguity notes;
- independently produced human, AI, and AI+human drafts;
- exact model, prompt, aids, and human interventions;
- recognized translations revealed only after independent drafts freeze;
- `1:1`, `1:n`, `n:1`, `n:m`, omission, addition, reordering, and unresolved
  alignment states;
- separate judgments for fidelity, semantics, terminology, ambiguity, voice,
  rhythm, imagery, syntax, fluency, and intervention;
- accepted, rejected, and unresolved alternatives with reviewer rationale.

Automatic metrics and LLM judges are diagnostics. Independent human review
owns promotion.

## Rights and visibility inheritance

Rights are evaluated separately for work, edition, item, source bytes,
metadata, transcription, translation, annotation, and export. A freely
queryable catalog record does not make the digitized text redistributable.

Derived objects inherit the most restrictive relevant visibility constraint
unless a documented legal or permission decision says otherwise. Unknown,
conflicting, permission-requested, research-only, local-only, and public are
valid explicit states.

The relevant constraint is evaluated per layer and per content carried. A
`local-only` file may still be cited as the research witness behind a
public-safe bibliographic record or provenance edge; that reference does not
redistribute the file. A derivative that reproduces, transforms, quotes, or
otherwise carries protected source content receives its own explicit rights
and visibility decision. Source payload, metadata, provenance, transcription,
translation, annotation, and export therefore remain separately governable
even when they share one lineage.

The rights gate is evidence-seeking, not presumptively closed. Public-domain,
open-license, permission-granted, and conditional noncommercial routes are
positive outcomes when verified for the exact layer, object, jurisdiction, and
intended use. Conditions such as attribution, noncommercial use, no
derivatives, share-alike, or source-site terms remain machine-readable
restrictions rather than being flattened into either “free” or “forbidden”.

## Projection boundary

The authoritative chain is:

```text
tracked identity/claim/review records
  -> reproducible projection receipt
    -> lexical or vector index / RDF / property graph / KAG / UI
```

A projection may be deleted and rebuilt without deleting knowledge. No graph
database, vector store, annotation-service database, workbook, or model cache
may be the sole copy of an assertion or review decision.

## Growth rule

The Zarathustra kernel may teach agents how to preserve source return,
distinguish layers, expose uncertainty, and review proposals. Its concepts,
predicates, etymologies, and branch shape do not automatically transfer to
another work. Resistance from another source is evidence that the contract
may need to grow.
