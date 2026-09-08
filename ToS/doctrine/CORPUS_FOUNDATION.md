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
| persistent identity | the identifier remains while correctable descriptions accumulate | work, expression, edition, item, text layer, passage, occurrence, lexeme, sign, concept, relation, annotation, claim |
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
| `text-layer` | one immutable, role-bearing textual representation of an exact source scope | accepted text, the source file itself, or a silently mutable OCR field |
| `collection` | an aggregate publication or container holding multiple works/expressions | a single contained work |

Authorship, translation responsibility, edition identity, date, place, and
container membership are claims with evidence status. A filename may seed a
lead but cannot settle any of them.

The identity ladder is both structurally declared and claim-addressable. A
Work's Expressions, an Expression's Editions, and an Edition's Items remain
visible in their owner records, while `has_expression`, `embodied_by`, and
`exemplified_by` claim packets carry the relation's own ID, exact evidence,
maker, provenance, visibility, and review state. The two representations must
close exactly; neither is allowed to drift into a second truth. These are
bibliographic topology predicates. `embodied_by` does not entail that two
texts are identical, author-final, critical, accepted, or semantically
equivalent.

The ladder does not absorb every source kind. A physical artifact has its own
`tos.artifact.*` identity, independent of the catalog that currently describes
it. A documentary, critical, or synoptic reconstruction across witnesses has
its own `tos.composite.*` identity, independent of the provider that currently
renders it. Artifact, catalog record, member transcription, composite,
editorial coordinate, translation, and interpretation remain separate. Stable
membership or a stable Q-number supports return and comparison; it does not
make the reconstruction an ancient original or its readings semantically
fixed.

`tos.document.*` identifies a persistent communicative or documentary
intellectual object without requiring it to be a Work or follow a linear
Work/Expression/Edition chain. `tos.letter.*` is its addressed-correspondence
subtype. These are not the physical manuscript, catalog record, transcription,
published edition or digital representation. Sender, addressee and author are
contextual roles, not types of people. Date/place of composition, dispatch,
receipt, custody and later reading remain separate claims. Unknown participants
or an unsent letter do not invalidate its documentary identity. A copy does not
by itself settle either shared documentary identity or physical identity.

The source metadata and its exact human forms use the declared reader and
`source.create` contract. The shared metadata schema reuses Corpus field law;
the document schema adds only its own identity constraints. Language/genre
combinations do not create more subclasses. Description correction preserves
the subject ID and previous record versions; a change of referent or an
incompatible kind requires an explicit identity transition, never an edited
prefix. Claims and their evaluations are not hidden in metadata convenience
fields such as `sender_ref`, `language` or a mutable document year.

An exact `{id, version, digest}` record reference is distinct from that record's
persistent identity and from the described subject. A derived
[record-version view](semantic-interchange/README.md#exact-record-version-views)
may expose verified retained public Claim bytes, or an explicit availability
gap with the same reference. A later description is never substituted for the
selected version. Byte integrity and historical assessment context do not
grant current use or establish the truth of the recorded assertion.

The [fragment/quotation profiles](semantic-interchange/README.md#textual-fragments-and-quoting-passages)
identify a textual portion separately from a particular passage transmitting
it. Neither is a physical fragment, an editorial designation, the act of
citation or an exact text-layer unit. Source-described extent and location
remain qualified research accounts; source anchors and versioned text layers
own exact wording. A translated quotation does not establish original-text
equivalence, and a reconstruction does not become an ancient original.

The [declared metadata profile](semantic-interchange/README.md#declared-source-metadata-profiles)
binds compatible source kinds and schema versions to the common reader without
promoting that metadata shape into a universal ontology. The profile belongs
to the source type registry; catalogs and projections execute it, retain the
exact record, and cannot infer claims or grant source-write/admission powers.

Initial native Work creation may record a provisional identity with the
schema-required expression-claim list empty. That means no expression
assertions have been supplied, not that no expression exists. This operation
does not create an author, language realization, publication or physical copy.
The bounded Nietzsche source home's stronger authorship and chronology closure
remains outside standalone Work creation. The executable operation and limits
live in the [source-owner creation contract](../../mechanics/growth-cycle/parts/branch-growth-cycle/README.md#native-standalone-identities).

Source-near semantic descriptions use the explicit
[concept/conception profile](semantic-interchange/README.md#concepts-situated-conceptions-and-transformations),
not the bibliographic identity family. Their declared research scope and
continuity criterion are contestable and accompany human forms. A situated
conception is not a record revision, word, Claim, or canon admission.
Conceptual membership, attribution and transformation remain evidence-bearing
Claims with their own source, interpretation and assessment posture. Existing
scoped Concept nodes are not silently reclassified as crosscutting concepts.

The [physical-artifact adapter](semantic-interchange/README.md#physical-artifacts-existing-source-adapter)
projects native v1/v2 artifact metadata into the shared catalog and exact
focus/inspection reader without replacing the authored record or inventing
relations to Works, texts, composites or visual representations.

The [scholarly-composite adapter](semantic-interchange/README.md#scholarly-composites-existing-source-adapter)
likewise retains native composite identities and complete v1 records in both
readers. Source-reported members and coverage remain observations inside their
original records; catalog inclusion does not create accepted membership edges,
identify the reconstructed ancient object, or resolve an exact text layer.
The compatible descriptive `composite.json` profile uses that same identity
family for modern textual reconstruction, collation and arrangement. It owns
an explicit composition account, editorial method, coverage limits and
referent criterion without requiring invented physical members. The native
witness format is retained, not converted; one ID cannot have two current
records across these formats. Neither metadata shape accepts its readings.

Responsibility claims retain their role-specific subject and Agent object:
Work author, Expression translator, Edition editor, paratext author, designer,
publisher, copyist, corrector, and rights holder are not interchangeable
variants of a generic creator field. A role not yet admitted by the governing
schema and validator remains an explicit research need rather than an
untyped edge.

## Chronology law

A Work has no single self-evident date. Composition, inscription, dispatch,
printing, title-page year, private issue, public sale, posthumous editing,
reception, preservation, and digitization are different temporal claims.
Ordering a corpus therefore requires a named facet and keeps the claim that
supplied it source-returnable.

The first bounded source profile is `first_publication_chronology`. Its object
retains a Gregorian interval, the meaning of its boundaries, one event or an
ordered sequence of stages, availability posture, precision, and an explicit
ordering warning. A staged Work may have different earliest-publication and
sequence-completion boundaries. A private completion does not become public
availability, and a posthumous first print does not become authorial
completion or an author-final text.

`chronology_claim_refs` link a Work to these evidence-bearing packets without
putting a mutable year into identity. Derived timelines may sort by interval
start or end only when they declare the chosen facet and uncertainty law. The
current first-publication profile creates no composition chronology, universal
canonical order, human acceptance, semantic relation, or canon.

## Identifier law

Corpus identifiers use the local family:

```text
tos.<class>.<stable-local-name>
```

where `<class>` is one of `agent`, `work`, `expression`, `edition`,
`collection`, `item`, `artifact`, `composite`, `file`, `text-layer`, `passage`, `region`, `anchor`, `occurrence`,
`lexeme`, `annotation`, `sign`, `concept`, `claim`, `relation`, `rights`,
`review`, or `event`.

The historical-situation profile adds `historical-event`, `historical-process`
and `historical-state`. These identities are separate from provenance
`event` records and authored semantic Event/State nodes. Their participants,
places and associated Works remain evidence-bearing Claims; a historical
description is neither a cause nor an admission. The executable source and
consumer contract is in [semantic interchange](semantic-interchange/README.md#historical-situations-source-profile).

The documentary profile adds `document` and `letter`. Their ID prefixes name
referent families, not languages, archival repositories, mutable shelfmarks or
the shape of one graph projection.

An explicitly delegated initial historical creation can publish one new
provisional identity, its separately identified initial claims and source-bound
human forms together through the [source-owner command](../../mechanics/growth-cycle/parts/branch-growth-cycle/README.md#initial-historical-subject-creation).
The creation receipt binds the exact initial files; it is not the claims'
research provenance, an assessment or historical acceptance. This bounded
operation does not revise existing subjects or turn a directory into corpus
identity. Catalog and graph publication remain weaker, separate operations.

Catalog wording may carry explicit per-field language/script declarations in
`field_languages`; these are distinct from an Expression's `language` and from
the linguistic originality or translation of a human form. The shared
[human-forms adapter](HUMAN_FORMS.md#bibliographic-metadata-adapter) retains each
declaration and its qualifications as exact source-bound context. A corrected
declaration advances the record and successor form bindings, not the subject ID.

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

An anchor's persistent identity, the immutable target file, the exact
representation state, and the selector are separate. Text offsets count
Unicode code points in logical order and use a half-open `[start,end)`
interval; byte offsets use a different selector type. Every text selector
declares the normalization and digest of the representation it addresses.
Independent alternatives and an ordered refinement chain are never encoded as
one ambiguous list. Mechanical resolution and human source-visible review are
separate states.

Copied quote text follows the rights and visibility of the exact layer it
reproduces. A tracked nonpublic anchor therefore carries locators or a
digest-bound receipt for an ignored private selector, not copied source text.
A digest-only receipt is inspectable provenance but is not itself resolvable.
The additive `tos_source_anchor_v2` contract exercises this law on public
synthetic fixtures only. Existing `tos_source_anchor_v1` records retain their
historical meaning until one concrete source question justifies a bounded
successor; no bulk reinterpretation is permitted.

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

The additive `tos_source_text_layer_v1` contract makes this separation
first-class. Each layer binds one exact Work/Expression/Edition/Item/File
scope and `tos_source_anchor_v2`, an immutable UTF-8 artifact and digest, its
role, language, Unicode form, storage and publication posture, explicit
digest-bound rights and publication-authority refs, and the exact
predecessor record/content digests where derivation exists. Explicit
code-point edit operations are half-open and independently replayable; a
withheld operation stream needs its own governed receipt. Normalization is a
successor, never a rewrite.

`structural_extraction` is the bounded no-model route from one exact
machine-readable witness structure to a source-near immutable text layer. It
must name the selector and extraction policy, preserve every declared source
feature, fail on an unexpected element, and retain the result as an unreviewed
machine transcription. It is not an identity copy, OCR, manual transcription,
or linguistic analysis. The first real use is one DTA paragraph at
`Za-I-Vorrede-1`: seven TEI `lb`-delimited print lines and six line breaks,
stored privately and projected only as text-free tracked identity, digest,
range, provenance, and authority records.

PDF embedded text is a different source observation. For the exact
Antonovsky/Prometey 1911 page-6 opening paragraph, pinned Poppler bbox output
is retained as a private diagnostic byproduct and its mechanically selected
text as a private `raw_ocr` layer. Six layout lines and five line breaks may be
addressed, but the layer remains lossy and unreviewed: visual inspection found
one print-joined historical word split into four embedded-text tokens. A
source-visible discrepancy must be recorded as uncertainty, not silently
repaired. This route is not diplomatic transcription, accepted Russian,
translation correspondence, or a reason to infer alignment merely because a
German layer exists nearby.

An explicit alignment proposal is a separate claim layer. The first real
question-scoped use selects only the exact first `U+002E`-terminated span from
each of those private paragraphs, freezes the two partial sentence
segmentations with exact excluded remainders, and binds them to one opaque
one-to-one claim. `proposed` is the ceiling: the method does not tokenize,
translate, resolve the Russian spacing uncertainty, adjudicate technique or
fidelity, accept either language layer, create review or projection work, or
authorize semantics, graph/canon promotion, redistribution, or publication.
Any stronger state requires source-and-target-visible, competence-appropriate
evidence for the exact claim, not confidence in the deterministic builder.

Mechanical validation, source-visible review, reviewer language competence,
accepted use, and rights/publication authority are separate gates. In
particular, an unreviewed diplomatic candidate can match an anchored source
selection exactly and still have no accepted use; a normalized successor
cannot claim diplomatic or source-fidelity authority. The public synthetic
A/B/C laboratory proves only byte/digest closure, edit replay, and explicit
NFD-to-NFC succession. It creates no accepted transcription, German
competence, translation, sign, semantic claim, graph truth, canon effect, or
bulk migration obligation.

Dividing one frozen layer is an additive assertion layer of its own. The
`tos_source_text_unit_packet_v1` contract binds opaque packet, scheme,
segmentation, unit, review, and projection identities to one exact immutable
text layer. Those identities do not derive from text, labels, ordinals,
offsets, or the current analysis. Physical lines, source-observed structure,
orthographic tokens, linguistic words or sentence-like units, punctuation,
whitespace, graphemes, model subwords, milestones, and non-surface analytic
nodes remain distinct kinds. A segmentation never edits its input.

Every source-bearing unit returns through ordered exact anchors. Declared
coverage, gaps, overlap, punctuation, whitespace, line breaks, hyphenation,
normalization posture, parent/child membership, competing alternatives, and
supersession remain explicit. No character may disappear merely because an
algorithm ignores it. Source-observed layout is not accepted linguistic
analysis; machine, model, imported, and synthetic results cannot accept
themselves. A sampled review may calibrate a method but cannot silently accept
a complete segmentation. Acceptance requires a separate source-visible
human or agent assessment over the exact frozen layer, declared scope, and
relevant competence under [KNOWLEDGE_ASSESSMENT](KNOWLEDGE_ASSESSMENT.md).
Research admission can cover a method-qualified batch without a human signature
per unit; it must not extrapolate a sample beyond the admitted scope. Existing
human-only packet formats preserve their history and require an explicit
assessment adapter. Model subwords and virtual nodes cannot promote themselves to an
occurrence, lexeme, sense, sign, concept, relation, or graph fact.

`tos_native_text_unit_binding_v1` is an exact return to an existing native unit,
not another textual subject. It pins packet, layer, selected segmentation,
unit versions and ordered anchors; byte fixity and canonical assessment-record
digests remain different. Its bounded resolver can validate metadata without
opening source text, or explicitly verify the frozen UTF-8 representation.
The v3 local assessment adapter retains the native unit identity and historical
packet fields. A separate layer record supplies evidence of the same origin;
neither adaptation nor exact-byte verification accepts a linguistic boundary.
Research assessment is recorded separately under
[KNOWLEDGE_ASSESSMENT](KNOWLEDGE_ASSESSMENT.md), without fabricating an old
human review or widening access/publication. The executable read scopes and
currentness limits live in the
[growth-cycle contract](../../mechanics/growth-cycle/parts/branch-growth-cycle/README.md#native-textunit-return-and-assessment).

TEI, CoNLL-U, Web Annotation, ISO/LAF-family JSON, retrieval chunks, and graph
forms are status-preserving derived projections. The public-synthetic A/B/C
laboratory proves only range, digest, reference, coverage, gap, competition,
review, visibility, and projection mechanics. It establishes no real German
boundary, token, word, human review, translation, semantic claim, or canon
effect. The historical `tos-local-sentence-segmentation-v1` string remains a
legacy proposed method label until one concrete source question justifies a
bounded migration.

## Execution provenance

Every materialized acquisition or transformation is an Activity over
immutable Entities with separately named responsible Agents. Inputs, outputs,
and diagnostic byproducts remain distinct; co-occurrence does not imply
derivation, so every claimed derivation has its own directed edge. A successful
event requires at least one authoritative output. A failed or stopped event
retains its exit state and may retain diagnostics as byproducts, but it must not
promote partial material to output.

The additive `tos_provenance_event_v2` contract captures exact argv and
configuration, software and runtime identity, optional model invocation,
responsibility, manual-change receipts, measurements, timestamps, terminal
state, rights/visibility, review, authentication, and a bounded replay
classification. Unknown or unavailable evidence remains explicit rather than
being represented as a false zero or a generic success. Exact receipt bytes are
bound by an external manifest to avoid a self-referential self-hash.

Five planes remain independent: byte and lineage closure; replay
specification; evidence authentication; human/source/language review; and
rights, publication, semantic, and canon authority. A schema-valid, hash-closed
unsigned receipt proves mechanics only. It does not authenticate the producer,
prove that the reported run occurred, establish output quality, or authorize
downstream use. Existing `tos_provenance_event_v1` records retain their
historical meaning. A v2 successor is created for new materialized work or a
question-triggered migration, never for a bulk version-count increase.

Historical schema inputs and current schema authority are separate. Required
prior public schema bytes may be retained immutably in
[`contracts/history/`](../contracts/history/README.md), addressed by their exact
SHA-256 and original `$id`. This resolves recorded inputs without restamping
old events; it neither replaces a missing active schema nor makes the old
contract current. Current record/output validation remains independent.
Ordinary source/evidence paths do not gain this schema-only fallback.
The rationale and rejected alternatives are in
[TOS-D-0052](../../docs/decisions/TOS-D-0052-historical-contract-input-bytes.md).

## Sign ladder

Languages, linguistic varieties, scripts and transliteration conventions may
themselves become source-described research subjects through the
[linguistic profile](semantic-interchange/README.md#languages-varieties-scripts-and-transliteration-schemes).
The source's language, an inscription's attributed language/script, the
notation convention, an exact text layer, a sign reading and the language of
its description remain distinct. Describing a scheme does not perform a
transliteration or translation. Artifact period and provider language labels
do not automatically establish a dialect, sign inventory or accepted reading.

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

The [lexical metadata profile](semantic-interchange/README.md#lexemes-written-forms-and-contextual-senses)
gives lexical groupings, written representations and situated senses separate
source descriptions and grounded membership Claims. Written-form identity
retains the supplied spelling and notation scope without normalization; it is
not a native source address or a human display-form identity. Description
correction does not silently replace that referent. The existing native
occurrence and exact-text contracts remain stronger for attestation.

The semantic identities remain distinct:

- `occurrence_id` identifies one addressable appearance in an exact witness;
- `lexeme_id` identifies a linguistic normalization whose membership remains
  a versioned claim;
- `sign_id` is assigned only after an evidence-bearing, competence-scoped
  promotion decision over a concrete candidate; issuing identity is not proof
  of truth or a canon decision;
- `concept_id` identifies a contestable interpretation family, not a hidden
  synonym for a sign;
- `claim_id` identifies one versioned assertion with maker, time, method,
  evidence, alternatives, uncertainty, and review;
- `relation_id` identifies one typed relation record whose claim and evidence
  remain separately resolvable.

Before sign promotion, the stable candidate identity is an `annotation_id` or
`claim_id`, never a prematurely minted `sign_id`. Labels remain mutable and do
not determine any of these identities.

The [qualified motif proposal](semantic-interchange/README.md#qualified-motif-proposals-and-explicit-member-dependencies)
uses one Claim ID over a complete declared set of exact Occurrences. Its
focal occurrence is an entry to the hypothesis, not its only source; the
interpretation, every member and native grounds must be read together.
Member-return edges are structural context, not separate accepted membership
assertions. Revising that set does not mint a Sign or bypass assessment.

The additive `tos_semantic_annotation_packet_v2` contract materializes this
law as stand-off records. Its opaque stable IDs are issued independently of
labels and readings; exact anchors bind source-near observations; every
interpretive assertion remains a maker-, method-, evidence-, uncertainty-,
alternative-, and review-bearing claim. A relation is not its supporting
claim, and a graph may project only an accepted claim without becoming its
authority. The current public-synthetic A/B/C proves these mechanics and
fail-closed promotion controls only. It adds no accepted sign, concept,
semantic relation, canon example, source reading, human review, or model act;
the existing semantic ladder and canon therefore remain unchanged.

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

The exact first-person body, capture conditions, experience/capture time,
author confirmation, privacy, permission, and revision lifecycle belong to
the dedicated `ToS/zarathustra/lived-witness/` route and
`ToS/contracts/lived-witness-packet.schema.json`. A generic claim may cite an
author-confirmed packet only through its own evidence, visibility, and review
route; it does not replace that authored record.

## Claim law

A claim records:

- its own ID and version;
- typed subject, predicate, and object/body;
- exact source anchors and supporting material;
- assertion layer and epistemic status;
- maker and method, including software/model/configuration where applicable;
- creation time and rights/visibility constraints;
- alternatives, counterevidence, and relations to prior versions;
- assessment history, reviewer kind and identity, rationale, exact decision
  event, competence, authority and policy; current admission remains separate.

Allowed review states include `unreviewed`, `accepted`, `accepted_with_limits`,
`rejected`, `ambiguous`, `deferred`, and `superseded`.

Confidence is the maker's declared uncertainty, not an objective probability
that the claim is true.

Legacy embedded human-review fields retain the meaning of their original
schema. New agent assessments bind the exact assertion through the current
[assessment contract](../contracts/knowledge-assessment.schema.json), not by
relabeling that history. An accepted research use need not wait for canon.

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

Automatic metrics do not themselves make a substantive decision. A model
self-rating is not evidence of authority or competence. Independent,
source-visible human or agent assessment can own scoped translation admission
under [KNOWLEDGE_ASSESSMENT](KNOWLEDGE_ASSESSMENT.md). A verified method may be
reused within its scope; independence is not inferred from repeated calls to
one model. Rights and publication remain separate decisions.

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

Local-only Item and scholarly-composite File bytes share the same custody
boundary: an exact ignored `payload/` beside tracked identity, fixity, rights
and provenance metadata. A Composite File remains a separate representation,
not a bibliographic Item or accepted source text. A recorded acquisition or
materialization does not prove present bytes in another checkout. Verification
must distinguish the retained declaration from current local availability;
neither state grants publication authority. The exact paths and checks remain
with [the source storage owner](../source-witnesses/LOCAL_STORAGE_BOUNDARY.md).

The rights gate is evidence-seeking, not presumptively closed. Public-domain,
open-license, permission-granted, and conditional noncommercial routes are
positive outcomes when verified for the exact layer, object, jurisdiction, and
intended use. Conditions such as attribution, noncommercial use, no
derivatives, share-alike, or source-site terms remain machine-readable
restrictions rather than being flattened into either “free” or “forbidden”.

An Item-level `rights.json` therefore has two distinct jobs. Its top-level
status is the conservative admission posture for the exact Item and File as a
whole. Optional `layer_assessments` preserve narrower conclusions for the
original work, translation, preface, commentary, editing, edition
presentation, digital scan, embedded text, annotation, or metadata. A positive
layer does not lift the aggregate Item/File gate: every content-bearing public
or server route must select the exact layer it carries, the reviewed
jurisdiction, and the intended use. Unknown marginalia or scan production can
therefore keep the exact PDF local while a separately reviewed public-domain
text layer remains a real positive finding rather than being erased.

## Owner-local source contexts

Public source metadata remains tracked under the current corpus rule; no new
ignored metadata subtree is introduced. Content-bearing private annotations,
native token packets, human forms and their operation/assessment history need
a separate explicitly selected confidential source store outside the public
checkout. They retain the same ToS identities and source contracts. A private
store is another governed location of authored source, not a parallel ontology,
an automatically publishable catalog or a cache whose deletion is harmless.

The [owner-local context contract](../contracts/owner-local-source-context.schema.json)
partitions logical refs by `ToS/source-witnesses/owner-local/<store_id>/`.
That prefix has exactly one private physical root; all other source/contract
refs have the existing checkout as their owner. `public_root` names a location,
not permission to read or publish every file there. There is no search,
fallback, root shadowing or copied private schema authority. An alias under
the checkout's reserved owner-local home is refused, even with identical bytes.
The native v1 binding can retain its logical refs because this transport is
explicit and singleton; a portable multi-owner reference would require its
own later contract, not an inferred fallback.

Context configuration and private files require mode 0600; directories from
the dedicated private root inward require 0700. Account ownership, no-follow
ancestors and current configuration/contract/root identities are checked
separately. The context enters an opaque dependency snapshot; its absolute
locations, short-span hashes and source-bearing bodies are not public export
fields. This is a trusted local-account boundary, not isolation from hostile
same-account code, encryption, backup or a portable artifact-trust decision.

The bounded native reader can use the context explicitly. Without it, the
reserved namespace is unsupported; the public source/profile/catalog reader
does not discover or consume a private store. Native exact reading still needs
the separate owner-local read selection and does not establish linguistic
quality or admission. A private transport cannot become public because its
underlying text layer has a positive public declaration. Native writer,
private source/Claim/form commands and private assessment-source integration
must each opt into this contract and retain their own authority checks;
creating or opening a context does not implement or delegate those operations.

## Projection boundary

Translation alignment follows the same evidence law before projection. Each
side resolves through an exact Work/Expression/Edition/Item/File, frozen text
layer, source-text-unit packet or other frozen segmentation/tokenization
artifact, and ordered source anchors. The
mapping receives a stable opaque identity and a separate versioned claim;
cardinality, order, omission/addition, technique, certainty, maker,
competition, supersession, and review are not compressed into one confidence
score. An imported memory, exchange file, generated answer or green validator
cannot grant itself admission authority. Acceptance requires a distinct
source-and-target-visible assessment with trusted authority, declared
competence and a frozen unassisted baseline, by a human or qualified agent.
Historical human-only formats are adapted explicitly, not silently retyped.
The most restrictive source, target, or packet visibility follows every
derivative. TEI, Web Annotation, XLIFF, TMX, and graph views are therefore
rebuildable projections, never the authority for the alignment or translation.

The authoritative chain is:

```text
owner-held identity/claim/review records (tracked public or explicitly private)
  -> reproducible projection receipt
    -> lexical or vector index / RDF / property graph / KAG / UI
```

A projection may be deleted and rebuilt without deleting knowledge. No graph
database, vector store, annotation-service database, workbook, or model cache
may be the sole copy of an assertion or review decision.

The tracked bibliographic graph profile reifies the claim between subject and
object. Every projected edge begins at that claim and preserves its canonical
digest, evidence nodes, maker, provenance event with time and method, and
review posture. Literal dates, edition-state objects, and unresolved statuses
remain claim-scoped literals rather than false stable identities. This graph
shape improves navigation; it cannot accept the claim or turn it into canon.

## Growth rule

The Zarathustra kernel may teach agents how to preserve source return,
distinguish layers, expose uncertainty, and review proposals. Its concepts,
predicates, etymologies, and branch shape do not automatically transfer to
another work. Resistance from another source is evidence that the contract
may need to grow.
