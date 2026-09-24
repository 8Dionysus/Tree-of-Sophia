# Corpus Foundation

This document defines the durable evidence floor beneath Tree of Sophia.

It defines source identity, evidence layers and the conditions for knowledge
claims. Physical routes and payload
handling are owned by `ToS/source-witnesses/`; repeatable extraction and
experiments are owned by mechanics and `abyss-stack`.

## Foundation thesis

The foundation makes sources identifiable, evidence retrievable, observation
distinguishable from inference, and the history of judgment inspectable.

```text
identity -> fixity -> address -> observation -> assertion -> review
```

Semantic growth builds on this evidence through interpretation and review.

## Four stability postures

| Posture | Meaning | Examples |
| --- | --- | --- |
| immutable | the recorded value never changes; a change creates another object/event | acquired file bytes, SHA-256 digest, signed receipt, review event |
| persistent identity | the identifier remains while correctable descriptions accumulate | work, expression, edition, item, text layer, passage, occurrence, lexeme, sign, concept, relation, annotation, claim |
| versioned assertion | content may be superseded without erasing lineage | title attribution, date, lemma, etymology, translation, concept boundary, relation |
| derived projection | safely rebuildable from stronger exact source surfaces | search index, vector index, graph store, KAG export, visualization |

Acceptance is an immutable review event pointing to a versioned, correctable
assertion.

## Corpus identity ladder

ToS uses an LRM-shaped local profile without claiming full IFLA conformance.

| Class | Meaning in ToS | Evidence and continuity |
| --- | --- | --- |
| `work` | an intellectual creation recognized as one work | identification across its separately described realizations |
| `expression` | a language/textual responsibility state of a work, including a translation | language, responsibility and textual history |
| `edition` | a published or edited manifestation embodying one or more expressions | publication statements and embodiment claims |
| `item` | a physical or digital copy/container as acquired | acquisition, custody and copy-specific evidence |
| `file` | an immutable byte sequence | media type, size and digest |
| `text-layer` | an immutable textual representation of an exact source scope | role, derivation, fixity and purpose-specific assessment |
| `collection` | an aggregate publication or container | evidence-bearing membership in the aggregate |
| `research-corpus` | a persistent research selection | purpose, scope, continuity criterion and membership claims |

One content-addressed File may have exact membership in more than one Item
when each Item manifest binds the same media type, byte size and SHA-256
digest. The File identity describes shared bytes. Each Item-to-File membership
retains its own manifest, acquisition event, payload path, original basename
and fixity observation; shared bytes do not transfer Item identity, custody,
rights or source attribution.

Authorship, translation responsibility, edition identity, date, place, and
container membership are claims with evidence status. A filename can supply a discovery lead; evidence-bearing claims establish
bibliographic relationships.

Research-corpus metadata declares purpose and selection criteria. Separate
membership Claims identify exact members. `research_corpus_membership` Claims state a bounded
selection, source scope, coverage and optional order. Existing collection `contains_work` packets retain their own exact closure. A corpus may include another corpus through a scoped Claim; further
containment relationships require their own grounds.

`intellectual_part_composition` uses the same scoped member grammar for proper
intellectual parts of a Work, Document, textual fragment or other declared
IntellectualObject. Its subject is the intellectual organization of the whole into parts. The whole cannot be its own
proper part. Competing divisions remain distinct versioned Claims.

`physical_part_composition` is a distinct scoped account of proper material
parts of physical Artifacts, including a fragmentary physical ensemble. Its members are physical material components. The source must qualify whether a component is attached,
detached, conjecturally joined or known only through a reported inventory;
joins, placement, original completeness and restoration each require evidence
beyond membership. Custody and ownership have their own claims.
Any relative order retains its physical/source basis and temporal scope.

`collection_member_order` orders Works whose membership is already recorded
through `contains_work`. Its whole value binds one exact
Collection metadata version and exactly one positive membership Claim version
for each selected Work. Every membership must be declared by that Collection
version. Those versions may be retained historical versions; later changes leave the order bound to those exact versions.
A missing exact version is reported as unavailable.
Legacy `collections/<owner>/<collection>/membership-claims.jsonl` has a narrow
current-only exact reader, scoped to retained current bytes and their actual provenance. Its absent polarity retains the declared
positive legacy membership meaning; native Claims require explicit polarity.
Preparation and projection verify the same basis. Membership truth and ordering quality require substantive assessment alongside
structural verification. The entire exact value,
including its version bindings, requires a separately scoped write grant.

The shared `scoped-members-v1` adapter treats `/object/members` as an unordered
typed dependency set, bounded at 128 members per Claim. Its separate ordering
is `unordered`, `partial` or `total`, with a source-stated basis and explicit
precedence pairs. Every pair must address distinct members; cycles are rejected
inside this Claim. A total order must compare every member transitively;
unresolved comparisons remain explicit. Partial orders preserve incomparability. Every order retains its stated basis,
with historical time, influence and causation described by their own claims.
Competing order Claims remain independently inspectable.

Coverage is `partial`, `exhaustive-within-scope` or `undetermined`. The second records source-attributed exhaustive coverage within its stated
scope. Unlisted objects retain unknown membership.
Large corpora use separately evidenced bounded membership Claims with explicit
scopes; readers must not label their union complete or choose between rival
scopes silently. A changed membership/order judgment creates a Claim revision;
the corpus identity and member metadata retain their existing records. Unknown
extension values survive without being interpreted as extra members.

The identity ladder is both structurally declared and claim-addressable. A
Work's Expressions, an Expression's Editions, and an Edition's Items remain
visible in their owner records, while `has_expression`, `embodied_by`, and
`exemplified_by` claim packets carry the relation's own ID, exact evidence,
maker, provenance, visibility, and review state. The structural declarations and evidence-bearing Claims must agree exactly. These predicates describe bibliographic topology. Textual identity, authorial
state, critical status and semantic equivalence require their own assessment.

Additional source families provide identities suited to their objects. A
physical artifact has its own
`tos.artifact.*` identity, independent of the catalog that currently describes
it. A documentary, critical, or synoptic reconstruction across witnesses has
its own `tos.composite.*` identity, independent of the provider that currently
renders it. Artifact, catalog record, member transcription, composite,
editorial coordinate, translation, and interpretation remain separate. Stable membership and Q-numbers support return and comparison. Reconstructions
retain their editorial origin and correctable readings.

`tos.document.*` identifies a persistent communicative or documentary
intellectual object without requiring it to be a Work or follow a linear
Work/Expression/Edition chain. `tos.letter.*` is its addressed-correspondence
subtype. Physical carriers, catalog records, transcriptions, editions and digital
representations have their own identities. Sender, addressee and author are
contextual roles linking people to the document. Date/place of composition, dispatch,
receipt, custody and later reading remain separate claims. Documentary identity can be established with unknown participants or for an
unsent letter. Relationships between copies require documentary and physical
evidence.

The source metadata and its exact human forms use the declared reader and
`source.create` contract. The shared metadata schema reuses Corpus field law;
the document schema adds only its own identity constraints. Language and genre are described through their own metadata and claims. Description correction preserves
the subject ID and previous record versions; a change of referent or an
incompatible kind requires an explicit identity transition, never an edited
prefix. Sender, language and dating assertions remain explicit Claims with their
evaluation history.

An exact `{id, version, digest}` reference selects one description of a
persistently identified subject. A derived
[record-version view](semantic-interchange/README.md#exact-record-version-views)
may expose verified retained public Claim bytes, or an explicit availability
gap with the same reference. The selected version remains exact. Current use and substantive judgment are
assessed separately from retained-byte integrity and historical context.

The [fragment/quotation profiles](semantic-interchange/README.md#textual-fragments-and-quoting-passages)
identify a textual portion separately from a particular passage transmitting
it. Each profile declares the textual referent and its location in the
transmitting source. Source-described extent and location
remain qualified research accounts; source anchors and versioned text layers
own exact wording. Translated quotations and reconstructions retain their linguistic and
editorial provenance; equivalence requires separate evidence.

The [declared metadata profile](semantic-interchange/README.md#declared-source-metadata-profiles)
binds compatible source kinds and schema versions to the common reader without
promoting that metadata shape into a universal ontology. The profile belongs
to the source type registry; catalogs and projections execute it, retain the
exact record, with source writing, claim authoring and admission assigned to their
respective commands.

Initial native Work creation may record a provisional identity with the
schema-required expression-claim list empty. The empty list records an open assertion set. Authorship, realizations,
publications and copies enter through their own source routes.
The bounded Nietzsche source home's stronger authorship and chronology closure
remains outside standalone Work creation. The executable operation and limits
live in the [source-owner creation contract](../../mechanics/growth-cycle/parts/branch-growth-cycle/README.md#native-standalone-identities).

An existing native Work can gain one separately identified provisional
Expression through the explicitly delegated
[compound source command](../../mechanics/growth-cycle/parts/branch-growth-cycle/docs/NATIVE_WORK_EXPRESSION_GROWTH.md).
The Work's appended ref and the distinct `has_expression` Claim describe the same declared metadata link, with attribution and textual equivalence
assessed separately. Selected parent history and existing descendants are preserved.
Participating readers require committed publication evidence and a matching
catalog before combining the legacy and native topology carriers. Bibliographic, textual and rights assessment follow their own review routes.

An existing Expression can likewise gain a separately identified provisional
Edition through the [exact Edition creation route](../../mechanics/growth-cycle/parts/branch-growth-cycle/docs/NATIVE_EXPRESSION_EDITION_GROWTH.md).
The Expression's appended embodiment ref and distinct `embodied_by` Claim
preserve one declared metadata relation. Earlier Work origin, responsibility
Claims and source-copy form history remain intact. This command writes one Expression’s relation at a time; existing
multi-Expression and collection Editions remain valid, including those with
no acquired Item. Printed ancestry, file acquisition, format equivalence and rights require
separate evidence and operations. Attaching an existing Edition requires its
own route.

An existing Expression may gain a qualified `translated_by` Claim to an existing
Agent through a separately delegated
[responsibility attachment](../../mechanics/growth-cycle/parts/branch-growth-cycle/docs/NATIVE_EXPRESSION_RESPONSIBILITY.md).
The appended responsibility ref and versioned Claim remain distinct from
descriptive metadata and the earlier Work/Expression creation stream. Competing attributions retain their own evidence. Endpoint metadata bindings
provide exact source return for assessing the role.
An external evidence URL records a citation. Its derived occurrence binds the
exact local citing Claim; examination of remote content requires a separately
recorded read.

A provisional Collection may start with no supplied membership assertions.
The independently delegated
[Collection growth route](../../mechanics/growth-cycle/parts/branch-growth-cycle/docs/NATIVE_COLLECTION_GROWTH.md)
attaches an existing Work by publishing one qualified `contains_work` Claim
and appending only its identity to `membership_claim_refs`. The Work is not
rewritten. Native and retained legacy Claims must close exactly over current
Collection refs; competing accounts retain distinct identities. Completeness, membership truth and source assessment retain their own evidence
and judgments.

An existing bibliographic object or physical Artifact may receive a native
[Link association](../../mechanics/growth-cycle/parts/branch-growth-cycle/docs/NATIVE_OBJECT_LINK_GROWTH.md)
through a separately delegated two-home transaction. Link and qualified Claim
are distinct records; the subject's metadata is not revised. Additive
`tos_object_link_claim_v2` extends the explicit domain to physical Artifact
without changing legacy v1. Remote retrieval, content identity and rights are established through their
respective evidence routes. The older direct navigation projection retains its
separate contract.

Source-near semantic descriptions use the explicit
[concept/conception profile](semantic-interchange/README.md#concepts-situated-conceptions-and-transformations),
with their own persistent semantic subjects. Their declared research scope and
continuity criterion are contestable and accompany human forms. A situated conception identifies a particular account of a philosophical
subject; its descriptions and assessments have their own versions.
Conceptual membership, attribution and transformation remain evidence-bearing
Claims with their own source, interpretation and assessment posture. Reclassifying an existing scoped Concept node requires an explicit source
decision.

The [physical-artifact adapter](semantic-interchange/README.md#physical-artifacts-existing-source-adapter)
projects native v1/v2 artifact metadata into the shared catalog and exact
focus/inspection reader without replacing the authored record or inventing
relations to Works, texts, composites or visual representations.

Native Artifact v2 metadata can be created through a separately delegated
[creation route](../../mechanics/growth-cycle/parts/branch-growth-cycle/docs/NATIVE_ARTIFACT_GROWTH.md).
Its existing exact rights, discovery and research records provide the evidence
inputs. The resulting record preserves `artifact_id`, empty initial planting refs and
unreviewed status. Content assessment, rights, canon and publication retain
their own decisions. Verified
native serialization origin is distinct from retained legacy discovery
provenance; neither can be substituted for a missing or corrupted other route.
Its catalog-response fingerprint may report retained or unretained bytes;
`captured` describes byte preservation, with availability and permission
recorded separately.
The separately bound discovery input records the exact snapshot and its
access limits. Inscription text encountered in an HTML snapshot requires its own source
layer, assessment and publication decision. Existing v1
records and their historical uncaptured fingerprints remain unchanged.

The [scholarly-composite adapter](semantic-interchange/README.md#scholarly-composites-existing-source-adapter)
likewise retains native composite identities and complete v1 records in both
readers. Source-reported members and coverage remain observations inside their
original records; membership assessment, ancient-object identification and exact text-layer
resolution remain separate operations.
The compatible descriptive `composite.json` profile uses that same identity
family for modern textual reconstruction, collation and arrangement. It owns
an explicit composition account, editorial method, coverage limits and
referent criterion without requiring invented physical members. The native witness format remains available. Each ID has one current record
across the compatible formats, with readings assessed separately.

Responsibility claims retain their role-specific subject and Agent object:
Work author, Expression translator, Edition editor, paratext author, designer,
publisher, copyist, corrector and rights holder each retain their specific
relation and evidence. A role not yet admitted by the governing
schema and validator remains an explicit research need rather than an
untyped edge.

## Chronology law

A Work’s chronology comprises several distinct kinds of event. Composition, inscription, dispatch,
printing, title-page year, private issue, public sale, posthumous editing,
reception, preservation, and digitization are different temporal claims.
Ordering a corpus therefore requires a named facet and keeps the claim that
supplied it source-returnable.

The first bounded source profile is `first_publication_chronology`. Its object
retains a Gregorian interval, the meaning of its boundaries, one event or an
ordered sequence of stages, availability posture, precision, and an explicit
ordering warning. A staged Work may have different earliest-publication and
sequence-completion boundaries. Private completion, public availability, authorial completion and posthumous
first printing retain their respective temporal meanings.

`chronology_claim_refs` link a Work to these evidence-bearing packets without
putting a mutable year into identity. Derived timelines may sort by interval
start or end only when they declare the chosen facet and uncertainty law. The profile’s scope is first-publication chronology; other temporal facets and
assessments retain their own claims.

## Identifier law

Corpus identifiers use the local family:

```text
tos.<class>.<stable-local-name>
```

The owning source contract declares `<class>` and its identity conditions.
For declared source profiles, the
[semantic registry](semantic-interchange/README.md#identity-and-vocabulary)
binds the family, prefix and schema to an understood reader. Native source
contracts retain their own identity grammar. Families such as `work`,
`artifact`, `document`, `text-layer`, `occurrence` and `claim` illustrate this
shared pattern. A new family enters through an explicit contract and mapping;
readers preserve unsupported vocabulary with its original identity and status.

The historical-situation profile adds `historical-event`, `historical-process`
and `historical-state`. These identities are separate from provenance
`event` records and authored semantic Event/State nodes. Their participants,
places and associated Works remain evidence-bearing Claims; historical explanation and admission require their own evidence and review. The executable source and
consumer contract is in [semantic interchange](semantic-interchange/README.md#historical-situations-source-profile).

The documentary profile adds `document` and `letter`. Their ID prefixes name referent families; language, repository, shelfmark and
projection data remain separately described.

An explicitly delegated initial historical creation can publish one new
provisional identity, its separately identified initial claims and source-bound
human forms together through the [source-owner command](../../mechanics/growth-cycle/parts/branch-growth-cycle/README.md#initial-historical-subject-creation).
The creation receipt binds the exact initial files. Claims retain separate
research provenance and assessment, and existing subjects retain their current
versions. Catalog and graph publication remain weaker, separate operations.

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

Every source-bearing assertion returns to at least one anchor. An anchor combines exact identity, selectors, context and source provenance.

The preferred bundle contains:

1. anchor ID and exact item/file version;
2. structural passage path, when defensible;
3. exact selected text with prefix and suffix context;
4. character/token positions for efficient resolution;
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
reproduces. A tracked nonpublic anchor carries locators or a digest-bound receipt for a
private selector. Resolving the selection requires access to that selector and
its source.
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
withheld operation stream needs its own governed receipt. Normalization creates a successor while preserving its predecessor.

`structural_extraction` is the bounded no-model route from one exact
machine-readable witness structure to a source-near immutable text layer. It
must name the selector and extraction policy, preserve every declared source
feature, fail on an unexpected element, and retain the result as an unreviewed
machine transcription. The transformation is recorded as structural extraction, with later linguistic
assessment kept separate. The first real use is one DTA paragraph at
`Za-I-Vorrede-1`: seven TEI `lb`-delimited print lines and six line breaks,
stored privately and projected only as text-free tracked identity, digest,
range, provenance, and authority records.

An independently delegated public project-text constructor may instead select
an exact UTF-8 range of an already retained project-authored document. Its
original File, new representation File, TextLayer and first segmentation keep
distinct identities and rights scope. Positive output rights plus exact
operator-scoped authority are required before text access; the authority decision must cover that exact output scope. This route preserves the selected code points literally. Assessment and
external deployment require separate owner decisions. Its operation and recovery belong to the
[public construction contract](../../mechanics/growth-cycle/parts/branch-growth-cycle/docs/PUBLIC_NATIVE_TEXT_CONSTRUCTION.md).

PDF embedded text is a different source observation. For the exact
Antonovsky/Prometey 1911 page-6 opening paragraph, pinned Poppler bbox output
is retained as a private diagnostic byproduct and its mechanically selected
text as a private `raw_ocr` layer. Six layout lines and five line breaks may be
addressed, but the layer remains lossy and unreviewed: visual inspection found
one print-joined historical word split into four embedded-text tokens. The observed spacing discrepancy remains recorded as uncertainty in this raw
OCR layer. Diplomatic transcription, Russian textual assessment and
cross-language alignment each require their own evidence.

An explicit alignment proposal is a separate claim layer. The first real
question-scoped use selects only the exact first `U+002E`-terminated span from
each of those private paragraphs, freezes the two partial sentence
segmentations with exact excluded remainders, and binds them to one opaque
one-to-one claim. The result remains `proposed`, with the Russian spacing uncertainty preserved.
Linguistic analysis, translation judgment, review, projection, canon and
publication require separate operations and authority.
A stronger state requires source-and-target-visible, competence-appropriate
evidence for the exact claim.

Mechanical validation, source-visible review, reviewer language competence,
accepted use, and rights/publication authority are separate gates. In
particular, an unreviewed diplomatic candidate can match an anchored source
selection exactly and still have no accepted use; a normalized successor
cannot claim diplomatic or source-fidelity authority. The public synthetic
A/B/C laboratory proves only byte/digest closure, edit replay, and explicit
NFD-to-NFC succession. Substantive source and language assessment remain separate from this synthetic
mechanical evidence.

Dividing one frozen layer is an additive assertion layer of its own. The
`tos_source_text_unit_packet_v1` contract binds opaque packet, scheme,
segmentation, unit, review, and projection identities to one exact immutable
text layer. Those identities are issued independently and remain stable as labels and
analyses change. Physical lines, source-observed structure,
orthographic tokens, linguistic words or sentence-like units, punctuation,
whitespace, graphemes, model subwords, milestones, and non-surface analytic
nodes remain distinct kinds. Segmentation retains its frozen input and adds addressable units.

Every source-bearing unit returns through ordered exact anchors. Declared
coverage, gaps, overlap, punctuation, whitespace, line breaks, hyphenation,
normalization posture, parent/child membership, competing alternatives, and
supersession remain explicit. Coverage accounts for every character, including material ignored by an
algorithm. Source-observed layout and linguistic analysis have separate
assessment scopes. A sampled review qualifies a method within its declared
sample and scope. Acceptance requires a separate source-visible
human or agent assessment over the exact frozen layer, declared scope, and
relevant competence under [KNOWLEDGE_ASSESSMENT](KNOWLEDGE_ASSESSMENT.md).
Research admission can cover a method-qualified batch without a human signature
per unit; it must not extrapolate a sample beyond the admitted scope. Existing
human-only packet formats preserve their history and require an explicit
assessment adapter. Occurrence, lexeme, sense, sign, concept and relation identities require their
respective source and assessment routes.

`tos_native_text_unit_binding_v1` returns to an existing native unit through
exact dependencies. It pins packet, layer, selected segmentation,
unit versions and ordered anchors; byte fixity and canonical assessment-record
digests remain different. Its bounded resolver can validate metadata without
opening source text, or explicitly verify the frozen UTF-8 representation.
The v3 local assessment adapter retains the native unit identity and historical
packet fields. A separate layer record supplies evidence of the same origin;
linguistic boundary assessment remains a separate judgment.
Research assessment is recorded separately under
[KNOWLEDGE_ASSESSMENT](KNOWLEDGE_ASSESSMENT.md), without fabricating an old
human review or widening access/publication. The executable read scopes and
currentness limits live in the
[growth-cycle contract](../../mechanics/growth-cycle/parts/branch-growth-cycle/README.md#native-textunit-return-and-assessment).

TEI, CoNLL-U, Web Annotation, ISO/LAF-family JSON, retrieval chunks, and graph
forms are status-preserving derived projections. The public-synthetic A/B/C
laboratory proves only range, digest, reference, coverage, gap, competition,
review, visibility, and projection mechanics. Real linguistic boundaries and substantive interpretations require
source-visible assessment. The historical `tos-local-sentence-segmentation-v1` string remains a
legacy proposed method label until one concrete source question justifies a
bounded migration.

## Execution provenance

Every materialized acquisition or transformation is an Activity over
immutable Entities with separately named responsible Agents. Inputs, outputs and diagnostic byproducts remain distinct; every claimed
derivation has its own directed edge. A successful
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
rights, publication, semantic, and canon authority. A schema-valid, hash-closed unsigned receipt establishes mechanical closure.
Producer authentication, occurrence of the reported run, output quality and
downstream permission each require their own evidence. Existing `tos_provenance_event_v1` records retain their
historical meaning. A v2 successor is created for new materialized work or a
question-triggered migration, never for a bulk version-count increase.

Historical schema inputs and current schema authority are separate. Required
prior public schema bytes may be retained immutably in
[`contracts/history/`](../contracts/history/README.md), addressed by their exact
SHA-256 and original `$id`. This resolves recorded inputs without restamping
old events; the active schema retains current contract authority. Current record/output validation remains independent.
Public metadata inputs resolve through their exact committed source-owner
revision lineage. Recorded builder inputs can retain exact public source bytes
through the separate [builder input archive](../research-packets/retained-builder-inputs/README.md).
Each archive serves its declared family and original path. Validation reads
retained source bytes; current execution and current-record checks use active
scripts and contracts.
The rationale and rejected alternatives are in
[TOS-D-0052](../../docs/decisions/TOS-D-0052-historical-contract-input-bytes.md).

Before extending a source-record lineage, verify every predecessor package
declared by its retained history. Each archived ledger must retain the exact
earlier receipt prefix. Missing or damaged predecessor evidence stops new
publication; exact restoration enables the authorized transition to continue.
The existing retained baseline and each archive's original scope stay explicit.

## Sign ladder

Languages, linguistic varieties, scripts and transliteration conventions may
themselves become source-described research subjects through the
[linguistic profile](semantic-interchange/README.md#languages-varieties-scripts-and-transliteration-schemes).
The source's language, an inscription's attributed language/script, the
notation convention, an exact text layer, a sign reading and the language of
its description remain distinct. Applying a scheme requires an explicit transliteration or translation event.
Dialect, sign inventory and reading judgments require linguistic evidence.

ToS describes signs through a layered family of addressable records.

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
Moving from an occurrence to a concept requires an explicit interpretation and
assessment.

The [lexical metadata profile](semantic-interchange/README.md#lexemes-written-forms-and-contextual-senses)
gives lexical groupings, written representations and situated senses separate
source descriptions and grounded membership Claims. Written-form identity
retains the supplied spelling and notation scope without normalization; native source addresses and human forms retain their own identities.
Description correction preserves the written-form referent. The existing native
occurrence and exact-text contracts remain stronger for attestation.

The semantic identities remain distinct:

- `occurrence_id` identifies one addressable appearance in an exact witness;
- `lexeme_id` identifies a linguistic normalization whose membership remains
  a versioned claim;
- `sign_id` is assigned only after an evidence-bearing, competence-scoped
  promotion decision over a concrete candidate; truth assessment and canon decisions retain their own records;
- `concept_id` identifies a contestable interpretation family;
- `claim_id` identifies one versioned assertion with maker, time, method,
  evidence, alternatives, uncertainty, and review;
- `relation_id` identifies one typed relation record whose claim and evidence
  remain separately resolvable.

Before sign promotion, the stable candidate identity is an `annotation_id` or
`claim_id`, never a prematurely minted `sign_id`. Labels remain mutable and do
not determine any of these identities.

The [qualified motif proposal](semantic-interchange/README.md#qualified-motif-proposals-and-explicit-member-dependencies)
uses one Claim ID over a complete declared set of exact Occurrences. Its focal occurrence supplies an entry to the hypothesis; the
interpretation, every member and native grounds must be read together.
Member-return edges expose structural context. Revising the set retains its
Claim identity and triggers the relevant assessment route; Sign issuance is a
separate transition.

The additive `tos_semantic_annotation_packet_v2` contract materializes this
law as stand-off records. Its opaque stable IDs are issued independently of
labels and readings; exact anchors bind source-near observations; every
interpretive assertion remains a maker-, method-, evidence-, uncertainty-,
alternative-, and review-bearing claim. A relation retains a separately resolvable supporting Claim; graph projection
follows that Claim’s acceptance scope. The current public-synthetic A/B/C proves these mechanics and
fail-closed promotion controls only. The synthetic laboratory exercises these mechanics while the existing semantic
ladder and canon retain their source-owned state.

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

An assertion may cite another layer but cannot disguise its own posture. Lived witness may explain sustained attention and salience. Textual,
bibliographic, etymological and interpretive claims require evidence
appropriate to their layers.

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

Confidence records the maker's declared uncertainty and its stated basis.

Legacy embedded human-review fields retain the meaning of their original
schema. New agent assessments bind the exact assertion through the current
[assessment contract](../contracts/knowledge-assessment.schema.json), not by
relabeling that history. An accepted research use need not wait for canon.

A generated claim catalog may expose subject, predicate, object, evidence,
maker, provenance, review posture, exact source line, and canonical source
digest for query and graph preparation only when the claim's visibility
permits that tracked projection. It remains a projection of the authored claim
packet: assessment and interpretation remain with the authored Claim and its review
history.

## Translation law

A translation has an Expression identity or an Expression proposal linked to
its source. Its packet must be able to retain:

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

Automatic metrics and model self-ratings are evidence for review within their
measured scope. Authority and competence are established independently. Independent,
source-visible human or agent assessment can own scoped translation admission
under [KNOWLEDGE_ASSESSMENT](KNOWLEDGE_ASSESSMENT.md). A verified method may be
reused within its scope; independence is not inferred from repeated calls to
one model. Rights and publication remain separate decisions.

## Rights and visibility inheritance

Rights are evaluated separately for work, edition, item, source bytes,
metadata, transcription, translation, annotation, and export. Redistribution of digitized text requires evidence for that text layer and
intended use.

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

Rights review actively seeks the permissions and conditions applicable to the
intended use. Public-domain,
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

Curated public source metadata remains Git-backed. Bulk imported metadata and
its provenance, fixity, rights and review evidence may live in an explicitly
selected immutable corpus revision with exact historical locators and verified
restore. Permanent local custody and permitted private R2 copies are separate
from public delivery. Generated catalogs and projections are data artifacts;
their removal from Git is not source retirement. Content-bearing private annotations,
native token packets, human forms and their operation/assessment history need
a separate explicitly selected confidential source store outside the public
checkout. They retain the same ToS identities and source contracts. A private store is a governed location of authored source, with the same
source contracts and explicit preservation and publication decisions.

The [owner-local context contract](../contracts/owner-local-source-context.schema.json)
partitions logical refs by `ToS/source-witnesses/owner-local/<store_id>/`.
That prefix has exactly one private physical root; all other source/contract
refs use the explicitly selected public source root, either a Git checkout or
a verified corpus view. `public_root` selects a source location; read and publication permissions
remain separately scoped. There is no search,
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
fields. The contract establishes a trusted local-account boundary. Process isolation,
encryption, backup and portable artifact trust require their respective
mechanisms.

The bounded native reader can use the context explicitly. Without it, the
reserved namespace is unsupported; the public source/profile/catalog reader
does not discover or consume a private store. Native exact reading requires its own owner-local selection; linguistic
quality and admission require assessment. A private transport cannot become public because its
underlying text layer has a positive public declaration. Native writer,
private source/Claim/form commands and private assessment-source integration
must each opt into this contract and retain their own authority checks;
each operation requires its own implementation and explicit delegation.

## Projection boundary

Translation alignment follows the same evidence law before projection. Each
side resolves through an exact Work/Expression/Edition/Item/File, frozen text
layer, source-text-unit packet or other frozen segmentation/tokenization
artifact, and ordered source anchors. The
mapping receives a stable opaque identity and a separate versioned claim;
cardinality, order, omission/addition, technique, certainty, maker,
competition, supersession and review retain separate fields. Admission authority comes from the trusted source owner. Acceptance requires a distinct
source-and-target-visible assessment with trusted authority, declared
competence and a frozen unassisted baseline, by a human or qualified agent.
Historical human-only formats are adapted explicitly, not silently retyped.
The most restrictive source, target, or packet visibility follows every
derivative. TEI, Web Annotation, XLIFF, TMX and graph views are rebuildable projections of
source-owned alignment and translation records.

The additive [native translation-alignment record](../contracts/native-translation-alignment-record-v1.schema.json)
keeps that owner's mapping and rights grammar while separating an unversioned
Alignment subject from exact descriptive record and Claim versions. Ordinary
description preserves the subject, source scope and mapping; a remapping uses
a new Claim, and a competing proposal a distinct Alignment. Exact predecessor record bytes and the Claim inside them establish succession. Its [native command route](../../mechanics/growth-cycle/parts/branch-growth-cycle/docs/NATIVE_TRANSLATION_ALIGNMENT.md)
captures supplied proposals only; historical v1 reviews remain intact, and translation quality requires
substantive assessment.

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
remain claim-scoped literals rather than false stable identities. This graph shape supports navigation back to the Claim and its separate review
and canon decisions.

## Growth rule

The Zarathustra kernel may teach agents how to preserve source return,
distinguish layers, expose uncertainty, and review proposals. Each new work supplies its own concepts, predicates, etymological evidence and
branch structure through source reading. Resistance from another source is evidence that the contract
may need to grow.
