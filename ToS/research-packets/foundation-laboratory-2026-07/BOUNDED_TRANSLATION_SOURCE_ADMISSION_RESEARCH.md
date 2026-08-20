# Bounded Translation Source Admission Research

Status: source-ordered method research and one bounded implementation
decision; no German philological acceptance, translation result, semantic
annotation, rights clearance, or canon promotion

Research snapshot: 2026-07-29

## Question

Can Tree of Sophia use an exact local German source slice for reproducible
AI translation-method experiments before a German-competent editor accepts the
slice, without weakening the authority boundary that protects source,
translation, semantics, and canon?

The answer is **yes, but only through a separate non-promotional research-input
posture**. Admission to a local experiment is not textual acceptance. The
input must preserve exact identity, selector, derivation, fixity, source-layer
responsibility, rights posture, and every blocked claim. Its outputs remain
laboratory candidates even if several models agree.

## I. Classical and current primary documentation

### TEI P5

The current [TEI P5 Guidelines 4.12.0, chapter 12](https://www.tei-c.org/release/doc/tei-p5-doc/en/html/PH.html)
were released on 2026-07-28. They keep digital facsimiles, transcriptions,
logical text structure, corrections, normalizations, and critical apparatus
readings distinct. Editorial interventions can carry `source`, `resp`,
`cert`, and evidence posture. A transcription of one witness may be used
inside a later critical apparatus, but it does not become the critical
reading merely by being encoded.

Consequence for ToS: a provider transcription, a local normalized slice, a
machine comparison, and a philologically accepted reading are separate
entities and activities. The source string used by an experiment must remain
traceable to the provider transcription and must not overwrite it.

### W3C PROV

[PROV-O](https://www.w3.org/TR/prov-o/) distinguishes entities, activities,
and responsible agents and provides explicit derivation, primary-source,
quotation, revision, and attribution relations. A derived entity does not
inherit every property or authority of its input.

Consequence for ToS: the local experiment input is a derived entity. It may
inherit exact source bytes through a recorded transformation, but it does not
inherit critical-edition authority, rights clearance, linguistic competence,
or editorial acceptance.

### DTA-Basisformat

The current [DTABf documentation](https://deutschestextarchiv.github.io/dtabf/einfuehrung.html)
defines a constrained TEI P5 profile for coherent annotation of historical
printed works. The exact DTA part-I item registered by ToS carries provider
evidence for OCR plus native-speaker checking and an exact TEI header,
manifest, payload digest, and page structure.

Consequence for ToS: DTA is a strong provider transcription and a lawful local
research route under the currently recorded item posture. It is not an eKGWB
critical edition and not a ToS human acceptance.

### MLA scholarly-edition guidance

The [MLA Statement on the Scholarly Edition in the Digital Age](https://www.mla.org/content/download/52050/1810116)
requires transparency, accuracy, appropriate method, responsible
documentation, visible mediation, and qualitative review for a scholarly
edition. It also explicitly recognizes that edition data and transcriptions
may support collation, repositories, corpora, and analyses beyond the
edition's own editorial intention.

Consequence for ToS: a clearly bounded transcription may support an
experiment before it supports an edition or accepted translation. The
experiment must not advertise the authority of the later editorial product.

## II. Established scholarship and review criteria

- Patrick Sahle's *What Is a Scholarly Digital Edition?* and Elena Pierazzo's
  documentary-edition work distinguish document, text, work, transcription,
  representation, and critical editorial argument.
- [RIDE's review criteria](https://ride.i-d-e.de/reviewers/catalogue-criteria-for-reviewing-digital-editions-and-resources/)
  treat sources, method, transparency, sustainability, and textual quality as
  separately reviewable questions.
- Wout Dillen's
  [Paradata for Digitization Processes and Digital Scholarly Editions](https://doi.org/10.1007/978-3-031-53946-6_8)
  argues that useful, usable paradata should make the production and
  evaluation of digital reproductions accountable without demanding an
  infeasible record of everything.
- James Cummings'
  [The Future Is Already Here](https://doi.org/10.12795/PH.2025.v39.i02.07)
  supports AI for transcription, collation, parsing, and analysis while
  retaining human editorial judgment. Machine output is not thereby an
  edition.

Consequence for ToS: the right unit of admission is not “the German is true.”
It is “this exact derived string is fit for this exact local experiment, under
these explicit unresolved conditions.”

## III. Fresh edge checked for present relevance

The following 2026 work was selected after the standards and established
literature, not used to replace them:

- Barker, Cronk, and Roe,
  [Digital Scholarly Editing in Practice](https://doi.org/10.1017/9781009364355)
  (2026), treats the Oxford University Voltaire edition as a transition from
  a large reviewed print edition to a sustainable digital research resource.
- Elizabeth Williamson,
  [Digitized archives, content providers, and slow scholarship](https://doi.org/10.1093/llc/fqag062)
  (2026), shows that imaging, digitization, transcription, metadata,
  presentation, and publication form a layered digital provenance that must
  be interrogated rather than smoothed into one apparently factual object.
- Haoze Guo and Ziqi Wei,
  [From OCR to Analysis: Tracking Correction Provenance in Digital Humanities Pipelines](https://aclanthology.org/2026.nlp4dh-1.1/)
  ([DOI 10.18653/v1/2026.nlp4dh-1.1](https://doi.org/10.18653/v1/2026.nlp4dh-1.1),
  NLP4DH 2026), preserve span-level edit lineage, source, confidence, and
  review status because correction paths can change downstream extraction and
  interpretation.
- The 2026 work on AI and the Danish digital scholarly edition of Grundtvig's
  works continues the human-centred convergence model: computational methods
  extend rather than silently replace philological judgment.

Freshness does not make any single 2026 proposal authoritative. The common
signal is narrower: provenance and editorial state must remain first-class
inputs to downstream computation.

## IV. ToS source-use ladder

The current corpus decision `TOS-D-0020` already requires immutable source
bytes, versioned transformations, source-near observations, separate
interpretive assertions, and human-owned promotion. This research applies
that decision through the following non-collapsing ladder:

1. `registered_item`: exact acquired bytes, identity, fixity, provenance, and
   rights posture exist;
2. `provider_transcription_registered`: an external provider's transcription
   is preserved as that provider's entity and claim;
3. `machine_corroborated_slice`: a bounded selector and deterministic
   comparison establish exact or explicitly residual correspondence among
   witnesses;
4. `bounded_local_research_input`: a derived local-only string is admitted
   for one named experiment and no other authority;
5. `philologically_accepted_source`: a competent review accepts a reading,
   uncertainty, and editorial rationale;
6. `accepted_translation_or_semantics`: later review may promote a
   translation, sign, relation, concept, or canon object.

Passing step 4 does not imply step 5. Passing either does not imply step 6.

## V. Unit 001 evidence

The first source-review unit points to the first complete sentence candidate
in Naumann EPUB member `EPUB/page_55.html`, within eKGWB
`Za-I-Vorrede-1`.

The source-aware comparison now supports a narrower text-free result:

- the selected DTA part-I sentence has 20 alphabetic tokens;
- the corresponding eKGWB sentence has the same 20-token normalized sequence;
- the Naumann automatic EPUB begins with the same 20-token sequence;
- DTA and eKGWB whitespace-collapsed strings are byte-identical;
- this admission route copies no source string into its tracked packet.

A manual negative-control search found that the matching German sequence
already exists inside the pre-existing authored canon node
`ToS/canon/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json`
and its public compatibility mirror, alongside Russian and English authored
witnesses. This is not new leakage from the present route, but it is a real
blindness and authority constraint. The packet therefore digest-binds both
surfaces, requires them to remain invisible to model runs, and does not
revalidate their `canonical_source`, working-translation, or bridge-
translation roles. “No tracked source text” in this route means no copied
text in the admission packet, not global absence from the repository.

The local experiment string is derived from the DTA provider transcription,
not from the eKGWB response. eKGWB supplies one-way corroboration only. This
avoids treating the unencrypted BY-NC-ND candidate as a transformable corpus.
The DTA item remains local-only because its ToS rights record preserves
conflicting current provider fields and has no human rights review.

## VI. Allowed experiment and blocked claims

One `bounded_local_research_input` may open:

- local, blind AI-only translation-method calibration;
- A/B/C comparison of model, prompt, decoding, runtime, cost, and speed;
- source-return checks against the exact selector and digest;
- Russian readability, rhythm, usefulness, and uncertainty feedback from the
  solo operator;
- machine-proposed morphology, syntax, literal alternatives, and etymology
  questions when every external claim remains sourced or unresolved.

It does not open:

- the existing `human_only`, accepted `ai_only`, or `ai_human` translation
  lanes;
- a claim of German orthographic, grammatical, semantic, or philological
  correctness;
- a claim of translation fidelity or accepted translation;
- recognized-comparator reveal before experimental candidates are frozen;
- visibility of the pre-existing authored Russian or English translation
  surfaces during a blind machine run;
- public redistribution of any local source or derivative;
- semantic sign acceptance, relation/concept promotion, graph truth, or
  canon.

The operator may judge the Russian result as Russian prose and as a useful
personal reading aid. The operator is not asked to certify the German source
or cross-language fidelity.

## Decision for the laboratory

Admit exactly one local DTA-derived source slice as
`eligible_for_local_machine_calibration` when its deterministic packet closes
over:

- exact Work/Expression/Edition/Item/File identity;
- source selector and input/output digests;
- the eKGWB/DTA/Naumann triangulation packet;
- source and rights records;
- zero German acceptance, zero accepted translation lanes, zero semantic
  work, zero human debt, and zero promotion;
- digest-bound exclusion of the pre-existing authored canon and public-
  compatibility translation surfaces from the blind run.

This is an implementation of `TOS-D-0020`, not a new durable decision and not
a relaxation of the frozen accepted-translation plan.
