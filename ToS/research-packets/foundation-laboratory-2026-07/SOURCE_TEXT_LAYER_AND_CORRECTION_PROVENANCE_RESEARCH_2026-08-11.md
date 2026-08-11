# Source text layers and correction provenance research

Date: 2026-08-11

Scope: the missing contract between an exact source anchor and later OCR,
translation, lexical, semantic, graph, and publication work. This packet asks
how ToS should preserve machine transcription, diplomatic transcription,
reviewed reading, normalization, correction, uncertainty, provenance, review,
competence, and downstream admission without silently overwriting evidence.

No operator-held source was opened. No model or OCR runtime was run. The only
executable evidence is a public invented Unicode fixture. This packet creates
no accepted source text, human review, translation, linguistic claim, semantic
claim, graph truth, canon effect, public source route, or routine human task.

## Result first

ToS needs a first-class immutable source-text layer between source anchors and
all downstream interpretation. A layer is not “the text.” It is one exact,
versioned representation with:

- an exact Work -> Expression -> Edition -> Item -> File -> Anchor return;
- immutable representation bytes, digest, media type, character encoding,
  language, text scope, Unicode form, storage, visibility, and publication
  posture;
- explicit digest-bound source/layer rights-record refs and distinct
  publication-authority refs; a boolean alone is not authority;
- a declared editorial/transcription purpose;
- an explicit predecessor chain;
- reproducible corrections or a digest-bound private correction receipt;
- span-level responsibility, confidence, evidence anchors, and uncertainty;
- mechanical, human-review, language-competence, and downstream-use states
  that cannot substitute for one another.

Raw OCR, a diplomatic candidate, an accepted reading, and a normalized search
projection remain different layers. A later layer never overwrites an earlier
one. Agreement with a source anchor can prove exact string equality at that
anchor; it cannot prove that the transcription policy, language, surrounding
structure, completeness, or downstream interpretation is correct.

## 1. Current official and normative foundations

### 1.1 TEI P5 4.12.0

The current TEI release is P5 4.12.0, updated 2026-07-28. Its primary-source,
editorial-change, critical-apparatus, header, responsibility, certainty, and
Unicode guidance provides the closest mature representation model for the
problem. TEI separates the witnessed form from editorial intervention through
constructs such as `sic`/`corr`, `orig`/`reg`, `choice`, `gap`, `unclear`,
`supplied`, `app`/`rdg`, responsibility, certainty, source, and revision
history. The editorial declaration records whether and how correction and
normalization occurred; this is a method statement, not a generic “clean text”
flag.

Consequences for ToS:

1. correction, normalization, omission, damage, uncertainty, and choice must
   remain typed and explicit;
2. responsibility and certainty belong to the intervention, not merely the
   containing file;
3. an editorial policy must be digest-bound to each layer;
4. a diplomatic layer and a normalized layer are distinct representations;
5. validation of markup does not establish that an editorial judgment is
   true.

Sources:

- [TEI P5 4.12.0 title and release date](https://tei-c.org/release/doc/tei-p5-doc/en/html/TitlePageRecto.html)
- [TEI representation of primary sources](https://tei-c.org/release/doc/tei-p5-doc/en/html/PH.html)
- [TEI critical apparatus](https://tei-c.org/release/doc/tei-p5-doc/en/html/TC.html)
- [TEI header and editorial practices declaration](https://tei-c.org/release/doc/tei-p5-doc/en/html/HD.html)
- [TEI Unicode and normalization guidance](https://tei-c.org/release/doc/tei-p5-doc/en/html/CH.html)

### 1.2 OCR-D Ground Truth and provenance

OCR-D makes the transcription goal explicit and keeps text and layout ground
truth in PAGE-XML. Its transcription guidance preserves the historical
language state, minimizes unavoidable typographic interpretation, and does not
silently correct printing errors. Its provenance model records processors,
versions, start/end times, inputs, outputs, and parameters at page and document
levels through PROV-compatible records.

Consequences for ToS:

1. “ground truth” is task- and policy-relative, not timeless textual truth;
2. text, layout, and structure quality must remain separately measurable;
3. every automated layer needs exact input/output and method/configuration
   closure;
4. confidence is useful triage evidence, never a substitute for source-visible
   review.

Sources:

- [OCR-D Ground Truth Guidelines](https://ocr-d.de/en/gt-guidelines/trans/)
- [OCR-D transcription principles](https://ocr-d.de/en/gt-guidelines/trans/transkription.html)
- [OCR-D provenance specification](https://ocr-d.de/en/spec/provenance.html)

### 1.3 W3C Web Annotation and PROV

The Web Annotation Data Model separates body, target, source state, selector,
creator, generator, lifecycle, motivation, rights, and multiple forms of
selection. Its `Choice`, ordered `List`, and `refinedBy` semantics distinguish
alternative targets from staged narrowing. PROV separates entities,
activities, agents, usage, generation, derivation, and qualified roles.

Consequences for ToS:

1. an edit points to exact source and output spans; positions are accelerators
   inside immutable representation states;
2. alternatives are not flattened into one selected string;
3. human creator and software/model generator remain separately named;
4. rights of the source, layer, annotation, and downstream derivative remain
   independently expressible.

Sources:

- [W3C Web Annotation Data Model](https://www.w3.org/TR/annotation-model/)
- [W3C PROV-O](https://www.w3.org/TR/prov-o/)

### 1.4 IIIF Presentation 3.0

IIIF models the Canvas as the frame of reference for a physical or born-digital
view and uses annotations for images, OCR, manual transcription, translations,
and commentary. These bodies can be kept in separate Annotation Collections.

Consequence for ToS: page/canvas evidence, transcription, translation, and
commentary can share a target while remaining different bodies with different
makers, rights, and authority. A viewer projection must not collapse them.

Source: [IIIF Presentation API 3.0](https://iiif.io/api/presentation/3.0/)

### 1.5 Unicode Normalization Forms

Unicode Standard Annex #15 version 17.0.0 defines NFC, NFD, NFKC, and NFKD and
their stability/conformance rules. Canonical normalization can give equivalent
strings a stable binary form; compatibility normalization can erase
distinctions that matter and must not be applied blindly.

Consequences for ToS:

1. the source-near bytes and their exact code-point sequence remain immutable;
2. a normalization form is declared per representation;
3. NFC creates a successor access layer, not an in-place rewrite;
4. NFKC/NFKD require a separate declared purpose and explicit loss posture;
5. offsets count Unicode code points in a named representation and use
   half-open intervals.

Source: [Unicode Standard Annex #15, revision 57](https://www.unicode.org/reports/tr15/)

## 2. Established scholarly foundations

### 2.1 Sahle and IDE review criteria

The established IDE criteria define a scholarly digital edition as a critical
representation of historical documents or texts governed by an explicit
editorial method. The method must be justified, its rules visible, and its
implementation assessable. A digitized book plus transcription and indexing
does not automatically become a scholarly edition.

ToS implication: source storage, OCR, correction, indexing, and an accepted
reading are different achievements. The repository must be able to say which
one exists without borrowing authority from the others.

Source: [Criteria for Reviewing Scholarly Digital Editions, version 1.1](https://www.i-d-e.de/publikationen/weitereschriften/criteria-version-1-1/)

### 2.2 Pierazzo on documentary editions and modelling

Pierazzo's work shows that “diplomatic transcription” is not a self-defining
neutral copy. Digital media expands what may be represented and therefore
forces the editor to declare where representation stops, what features count,
and how document, text, and model relate.

ToS implication: `diplomatic_transcription` names a declared goal and policy,
not a quality verdict. Typography, layout, spelling, punctuation, lineation,
damage, and uncertainty need explicit postures even when the project chooses
not to encode some of them.

Sources:

- [A rationale of digital documentary editions](https://kclpure.kcl.ac.uk/portal/en/publications/a-rationale-of-digital-documentary-editions/)
- [Modelling Digital Scholarly Editing: From Plato to Heraclitus](https://www.openbookpublishers.com/books/10.11647/obp.0095/chapters/10.11647/obp.0095.03)

### 2.3 OCR-D/DTA ground-truth practice

The established OCR-D/DTA approach demonstrates a useful practical baseline:
preserve historical language, minimize interpretation, declare transcription
levels, and retain exact evaluation pairs. Its value for ToS is methodological,
not universal: the same page may need a glyph-oriented layer, a lexical layer,
and a normalized access layer for different questions.

## 3. Fresh relevant work, checked 2026-08-11

### 3.1 Guo and Wei, NLP4DH 2026

*From OCR to Analysis: Tracking Correction Provenance in Digital Humanities
Pipelines* is the most directly relevant fresh result. It records correction
lineage at span level, including edit type, correction source, confidence, and
revision status. Its experiments show that correction pathways change
downstream named entities and document-level interpretation; provenance helps
identify unstable output and prioritize review.

ToS implication: correction provenance is a first-class analytical layer. It
must survive into later retrieval and semantic work instead of being reduced
to the newest clean string.

Source: [ACL Anthology, NLP4DH 2026](https://aclanthology.org/2026.nlp4dh-1.1/)

### 3.2 HIPE-OCRepair 2026

The 2026 ICDAR competition provides a reproducible multilingual framework for
LLM-assisted post-correction of historical OCR. It reports useful gains but
also recurring over-correction on low-noise inputs. Its primary evaluation is
retrieval-oriented and participants correct coherent text units without source
images.

ToS implication: this is strong evidence for a correction challenger and a
search-use metric, but it cannot establish diplomatic fidelity. Retrieval
quality, character accuracy, historical-form preservation, and source-visible
fidelity remain separate axes.

Source: [HIPE-OCRepair 2026](https://arxiv.org/abs/2607.08143)

### 3.3 Levchenko, LM4DH 2025

The study adds historical-character preservation, archaic-insertion,
contamination, and stability controls to ordinary OCR evaluation. Multimodal
models can outperform classical OCR while also inserting historically wrong
archaic characters; post-correction can degrade results.

ToS implication: a lower CER does not prove historical fidelity. A model may
produce a more plausible-looking but less faithful layer. Period-specific
preservation and false historicization require explicit error classes.

Source: [ACL Anthology, LM4DH 2025](https://aclanthology.org/2025.lm4dh-1.7/)

### 3.4 Kanerva et al., RESOURCEFUL 2025

The open-weight LLM study finds language- and corpus-dependent post-correction
benefits and failures rather than a general winner.

ToS implication: method admission stays question-, language-, source-, and
noise-bound. No model output silently replaces the baseline layer, and a
negative or null result has equal standing.

Source: [OCR Error Post-Correction with LLMs in Historical Documents: No Free Lunches](https://aclanthology.org/2025.resourceful-1.8/)

### 3.5 Scalable confidence-assisted quality assessment

Recent large-scale work over nearly five million historical pages finds OCR
confidence useful when combined with sampled ground truth.

ToS implication: confidence can route review and stratify samples; it does not
become source truth, accepted text, or a replacement for independently checked
content metrics.

Source: [How Scalable is Quality Assessment of Text Recognition?](https://anthology.ach.org/volumes/vol0003/how-scalable-is-quality-assessment-of-text-of/)

## 4. Architecture decision for ToS

The additive `tos_source_text_layer_v1` contract is the reusable bridge:

```text
immutable source bytes
  -> exact source anchor v2
  -> raw OCR or machine transcription layer
  -> explicit diplomatic correction layer
  -> reviewed source-text layer, only after real review
  -> declared normalization/access layer
  -> translation / lexical / semantic tasks, only within accepted use scope
```

Each arrow is a derivation with exact predecessor record/content digests. Each
edit is replayable over half-open Unicode-code-point spans or represented by a
digest-bound private receipt. Every layer preserves its own bytes and status.
Rights-record refs travel with the representation, while downstream
publication requires separately non-empty authority refs. The synthetic
fixture deliberately keeps both ref sets empty and the publication gate false.

The contract adds `tos.text-layer.*` as the identity of one immutable textual
representation record. Its referenced `tos.file.*` remains the byte identity.
The layer ID is not a passage, occurrence, lexeme, sign, concept, claim, or
relation ID.

## 5. Public synthetic A/B/C laboratory

The source is the existing public synthetic source-anchor v2 Unicode selection
`cafe` + COMBINING ACUTE ACCENT. It contains no Nietzsche text.

| Variant | Exact layer | Question | Result ceiling |
| --- | --- | --- | --- |
| A | raw OCR candidate `cafe` | can a lossy machine candidate stay immutable and source-returnable? | candidate only |
| B | diplomatic candidate in NFD | can one explicit source-anchored insertion replay A and match the exact source selection? | exact mechanical equality, still unreviewed |
| C | NFC normalized access layer | can normalization replay B without overwriting or relabeling B? | access projection, still unreviewed |

The laboratory must independently read real fixture bytes, replay the edit
spans, recompute every digest, verify Unicode form, resolve the upstream source
anchor, and reject at least:

1. correction without an edit/receipt;
2. predecessor record drift;
3. span or exact-value digest mismatch;
4. accepted use on an unreviewed layer;
5. diplomatic use claimed by a normalized layer;
6. translation/linguistic/semantic use without competence evidence;
7. tracked nonpublic text;
8. publication use without publication authority;
9. self-supersession.

## 6. Promotion and stop rules

Promote only the mechanics if all fixture bytes, exact edit replays, source
return, schema gates, semantic negative controls, focused tests, full owner
validation, and independent manual byte/code-point checks pass.

Do not migrate current source records in bulk. Do not relabel the synthetic B
match as human-reviewed diplomatic text. Do not create human work merely to
exercise the contract. A real successor may open only for a concrete source
question and must preserve the local/private/public boundary of that exact
source layer.
