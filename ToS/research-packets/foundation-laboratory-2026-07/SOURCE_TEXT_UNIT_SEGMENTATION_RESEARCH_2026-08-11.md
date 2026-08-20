# Source text unit and segmentation identity research

Snapshot: 2026-08-11.

Question: what must Tree of Sophia preserve before a frozen source-text layer
can be divided into layout units, sentences, orthographic tokens, linguistic
words, or model-input pieces without turning one algorithm, exchange format,
or mutable ordinal into source truth?

This report follows the required order: official and classical documentation,
then established and high-impact work, then the freshest relevant 2025–2026
work. It concerns identity and evidence mechanics. It does not accept a real
German sentence boundary, token, word, morphology, translation, sign, concept,
or semantic claim.

## 1. Official and classical documentation

### Unicode text segmentation

- Current stable reference:
  [Unicode Standard Annex #29, revision 47, Unicode
  17.0.0](https://www.unicode.org/reports/tr29/tr29-47.html), dated 17 August
  2025.
- UAX #29 specifies default boundaries for extended grapheme clusters, words,
  and sentences. It explicitly separates boundary detection from the segments
  produced by those boundaries, permits conformant tailoring, and notes that
  text alone cannot always resolve language- and convention-dependent
  ambiguity.
- The standard publishes conformance tests, but passing default-boundary tests
  proves implementation conformance only. It does not prove that the default
  is the correct editorial or linguistic segmentation of one historical
  witness.
- ToS adoption: bind the exact Unicode version, revision, boundary kind,
  tailoring, locale, implementation, configuration, and input digest for every
  UAX-derived result.
- ToS non-adoption: a Unicode word or sentence boundary is not an attested
  source division and does not mint a lexeme, sense, sign, or concept.

### ICU boundary analysis

- Official implementation guidance:
  [ICU Boundary Analysis](https://unicode-org.github.io/icu/userguide/boundaryanalysis/)
  and [ICU Break Rules](https://unicode-org.github.io/icu/userguide/boundaryanalysis/break-rules.html).
- ICU implements UAX #29 and UAX #14 defaults with locale-sensitive filters,
  dictionary support for some scripts, and custom rule tailoring. Its own
  documentation warns that the efficient defaults are insufficient for some
  languages and applications.
- ToS adoption: ICU is a reproducible baseline when its release, locale,
  rules, dictionaries, sentence filters, and configuration are fixed.
- Boundary: ICU output remains a method result. A German abbreviation filter,
  dictionary, or custom rule is evidence about a candidate boundary, not a
  human competence substitute.

### TEI P5

- Current documentation snapshot:
  [TEI P5 4.12.0](https://www.tei-c.org/release/doc/tei-p5-doc/en/html/index.html),
  updated 28 July 2026.
- Relevant chapters:
  [Linking, Segmentation, and Alignment](https://www.tei-c.org/release/doc/tei-p5-doc/en/html/SA.html),
  [Simple Analytic Mechanisms](https://www.tei-c.org/release/doc/tei-p5-doc/en/html/AI.html),
  and [Non-hierarchical Structures](https://www.tei-c.org/release/doc/tei-p5-doc/en/html/NH.html).
- TEI provides generic `seg` and `anchor`, specialized sentence-like `s`, word
  `w`, punctuation `pc`, and stand-off mechanisms. It requires the principles
  and significance of a segmentation to be documented. It also preserves the
  fact that document, page, sentence, phrase, and other hierarchies can overlap
  rather than forcing them into one tree.
- ToS adoption: keep layout, editorial, linguistic, and analytic divisions as
  separately named layers; permit stand-off, overlapping, discontinuous, and
  competing analyses; preserve method description and responsibility.
- ToS non-adoption: an XML element name, `xml:id`, nesting position, or ordinal
  is not a durable ToS unit identity. TEI is an import/export projection, not
  the source of ToS authority.

### W3C Web Annotation

- Final Recommendation:
  [Web Annotation Data Model](https://www.w3.org/TR/annotation-model/).
- Text Position selectors use zero-based half-open character ranges; Text
  Quote selectors can add exact/prefix/suffix evidence; States bind a selector
  to an intended representation. The Recommendation warns that naked text
  positions are brittle when the resource changes.
- ToS adoption: bind every unit to an immutable text-layer digest and retain
  source return. Multiple selectors may be retained as independently
  checkable locators.
- ToS extension: a stable unit ID is independent of its selector, current
  text, ordinal, label, and segmentation interpretation.

### Universal Dependencies and CoNLL-U

- Current official format:
  [CoNLL-U Format](https://universaldependencies.org/format.html) and
  [Tokenization and Word Segmentation](https://universaldependencies.org/u/overview/tokenization.html).
- CoNLL-U distinguishes surface tokens from syntactic words, represents
  multiword tokens and empty syntactic nodes separately, forbids overlapping
  multiword-token ranges, and preserves missing spaces through `SpaceAfter=No`.
- ToS adoption: distinguish surface token, syntactic word, multiword grouping,
  and non-surface analytic node; preserve punctuation and whitespace posture
  needed for deterministic source return.
- ToS non-adoption: CoNLL-U integer IDs are sentence-local serialization order,
  not persistent unit identity. An empty syntactic node is not a source span.

### ISO linguistic annotation standards

- [ISO 24612:2012 LAF](https://www.iso.org/standard/37326.html) remains a
  published pivot framework for annotations over primary data.
- [ISO 24611-1:2025 MAF](https://www.iso.org/standard/85152.html) is the current
  word-sized morphosyntactic model. Its scope explicitly does not choose the
  linguistic boundaries that identify tokens or their language- and
  context-specific relations to word forms.
- ToS consequence: even the current morphosyntactic standard begins after a
  tokenization decision. Token identity, word-form identity, and morphology
  must remain separate.
- [ISO/CD 23096 JLAF](https://www.iso.org/standard/87354.html), registered in
  June 2026, proposes a JSON serialization of LAF. It is a Committee Draft, not
  a final foundation authority. ToS should watch it for interchange parity but
  must not freeze its draft shape into owner truth.

## 2. Established and high-impact research

### Corpus annotation as an authored layer

- Marcus, Santorini, and Marcinkiewicz, 1993,
  [Building a Large Annotated Corpus of English: The Penn
  Treebank](https://aclanthology.org/J93-2004/), established the practical
  importance of explicit corpus annotation conventions and layered correction.
- ToS consequence: a prestigious corpus convention remains a declared scheme,
  not a language-independent definition of word or sentence.

### Sentence-boundary baselines

- Kiss and Strunk, 2006,
  [Unsupervised Multilingual Sentence Boundary
  Detection](https://aclanthology.org/J06-4003/), demonstrated a reusable
  abbreviation-sensitive baseline across languages without making sentence
  boundaries trivial or universal.
- ToS consequence: keep a classical deterministic/statistical baseline in
  experiments, preserve its training state, and retain ambiguous alternatives
  rather than allowing a newer model to erase them.

### Cross-lingual word segmentation

- Nivre et al., 2020,
  [Universal Dependencies v2](https://aclanthology.org/2020.lrec-1.497/),
  separates linguistically motivated word segmentation, morphology, and
  syntax across a large multilingual collection.
- ToS consequence: a surface occurrence, orthographic token, syntactic word,
  lemma, and model subword need different identities and promotion routes.

### No singular universal tokenizer

- Mielke et al., 2021,
  [Between words and characters: A Brief History of Open-Vocabulary Modeling
  and Tokenization in NLP](https://arxiv.org/abs/2112.10508), surveys units from
  bytes and characters through words and multiword expressions and finds no
  singular solution for every application.
- ToS consequence: the contract must host multiple declared granularities and
  competing schemes. A model vocabulary is an implementation artifact, not a
  canonical decomposition of a philosophical text.

### Domain-specific sentence boundaries

- Sanchez, 2019,
  [Sentence Boundary Detection in Legal
  Text](https://aclanthology.org/W19-2204/), shows that general-purpose
  sentence tokenizers degrade in a punctuation- and syntax-specific domain.
- ToS consequence: evaluation must be witness- and use-specific. A global green
  score cannot replace manual inspection of bounded difficult cases.

## 3. Fresh 2025–2026 research

Freshness is not authority. The items below are classified by fit and
publication state as of the snapshot date.

### Directly relevant evidence

- Rosensweig et al., ALP 2025,
  [Automatic Text Segmentation of Ancient and Historic
  Hebrew](https://aclanthology.org/2025.alp-1.1/).
  - Fit: historical texts whose source tradition does not supply modern
    sentence punctuation.
  - Adoption: record grammatical-unit boundaries as method proposals and keep
    absence of source punctuation visible. Do not retrofit modern punctuation
    into a diplomatic layer.
- Fang et al., NLP4DH 2025,
  [A Comparative Analysis of Word Segmentation, Part-of-Speech Tagging, and
  Named Entity Recognition for Historical Chinese Sources,
  1900–1950](https://aclanthology.org/2025.nlp4dh-1.1/).
  - Fit: historical language change, absent orthographic word boundaries, and
    an explicit accuracy/cost comparison between LLM and conventional tools.
  - Adoption: benchmark quality, speed, and cost on the same frozen inputs;
    method leadership does not turn a segmentation into source truth.
- Kanerva et al., RESOURCEFUL 2025,
  [OCR Error Post-Correction with LLMs in Historical Documents: No Free
  Lunches](https://aclanthology.org/2025.resourceful-1.8/).
  - Fit: historical OCR and segment-length sensitivity.
  - Adoption: segmentation and correction are separate events over separate
    text layers. A better downstream character-error rate cannot authorize
    silent source mutation.
- Haslett, *Computational Linguistics* 2025,
  [Tokenization Changes Meaning in Large Language Models: Evidence from
  Chinese](https://aclanthology.org/2025.cl-3.3/).
  - Fit: token boundaries can systematically distort morphology- and
    form-sensitive representations, including effects in European languages.
  - Adoption: LLM tokens may be retained as model-input units with exact model
    provenance, but never substituted for orthographic, etymological, or
    semantic units.

### Fresh challengers, not foundation authority

- Hudspeth, Burns, and O'Connor, EACL 2026,
  [Contextual morphologically-guided tokenization for Latin encoder
  models](https://aclanthology.org/2026.eacl-long.270/).
  - Fit: a classical language with rich expert lexical resources and strong
    out-of-domain pressure.
  - Use: challenger evidence that morphology-aware and context-aware schemes
    deserve their own lab lane. Their output still starts as a proposal.
- Verkijk and Vossen, LoResLM 2026,
  [Out-of-Tune rather than Fine-Tuned](https://aclanthology.org/2026.loreslm-1.45/).
  - Fit: Early Modern Dutch, non-standard spelling, HTR noise, tokenization,
    and semantic similarity.
  - Use: direct warning that later fine-tuning does not repair every
    tokenization mismatch and may introduce spurious correlations.
- Li et al., Findings of ACL 2026,
  [BoundRL](https://aclanthology.org/2026.findings-acl.1733/).
  - Fit: efficient boundary-only generation and exact reconstruction checks
    for long structured text.
  - Use: later experimental challenger. A reconstruction reward is valuable
    mechanics evidence but cannot establish linguistic or editorial truth.
- [ISO/CD 23096 JLAF](https://www.iso.org/standard/87354.html), June 2026.
  - Fit: possible future JSON interchange for linguistic annotation.
  - Status: Committee Draft under development. Track, do not adopt as final.

### Relevant but deliberately not adopted for this foundation

- Wang et al., Findings of ACL 2025,
  [Document Segmentation Matters for Retrieval-Augmented
  Generation](https://aclanthology.org/2025.findings-acl.422/), and Liu et al.,
  ACL 2026,
  [Think in Sentences](https://aclanthology.org/2026.acl-long.104/), show that
  downstream model behavior changes with chunk and sentence boundaries.
- These results justify retaining task-specific model-input segmentations, but
  they do not define source, sentence, token, or philosophical meaning. RAG
  chunks and reasoning delimiters therefore remain disposable derived views.

## 4. Contract conclusions

The foundation needs a new additive owner contract. Existing source-anchor,
source-text-layer, lexical-plan, translation-alignment, and numbered-unit
contracts bind useful inputs or local methods but do not own stable text-unit
identity or competing segmentations.

The contract must preserve independently:

1. Exact Work, Expression, Edition, Item, File, and frozen text-layer identity,
   digest, language, Unicode form, and code-point addressing law.
2. Opaque packet, scheme, segmentation, unit, review, and projection IDs that
   do not derive from text, labels, ordinals, offsets, or current analysis.
3. Scheme version, boundary kind, basis, implementation, software/model,
   configuration, locale, Unicode version, tailoring, and maker provenance.
4. Distinct layout, source-structural, orthographic, linguistic, model-input,
   and interchange layers.
5. Distinct physical line, paragraph, verse, sentence-like unit, clause,
   phrase, surface token, syntactic word, punctuation, whitespace, grapheme,
   model subword, milestone, and virtual analytic node kinds.
6. Ordered exact anchor members for every source-bearing unit, including
   discontinuity and source return; non-surface analytic nodes must say that
   they have no source span.
7. Explicit coverage, gaps, overlap, punctuation, whitespace, line-break,
   hyphenation, and normalization posture. Nothing may disappear silently.
8. Source text and normalized/dehyphenated/corrected text in separate immutable
   text layers. Segmentation never edits its input.
9. Proposed, source-observed, ambiguous, partially reviewed, accepted,
   rejected, and superseded states without collapsing them.
10. Reciprocal competing segmentations and explicit supersession. Alternatives
    remain independently addressable.
11. Unit parent/child membership only through reciprocal references; order and
    hierarchy are relations, not identity.
12. Machine, model, imported, and synthetic results restricted to proposal or
    source-observation posture. Accepted linguistic/editorial boundaries need
    separate real-human, source-visible, competence-declared review over the
    exact frozen layer and declared scope.
13. Full versus sampled review scope. A sample may calibrate a method but cannot
    silently accept an entire segmentation.
14. Model subwords and empty syntactic nodes explicitly barred from automatic
    occurrence, lexeme, sense, sign, concept, or graph promotion.
15. TEI, CoNLL-U, Web Annotation, ISO/LAF-family JSON, retrieval chunks, and
    graph outputs as status-preserving derived projections only.
16. Most-restrictive rights and visibility inheritance from the source layer,
    packet, and projection destination.
17. Public-synthetic A/B/C fixtures for mechanics only, plus manual byte,
    code-point, range, digest, coverage, gap, competition, and rebuild checks.
18. No bulk reinterpretation of the historical
    `tos-local-sentence-segmentation-v1` string. It remains a legacy proposed
    method label until a concrete source question justifies migration.

Owner law remains: tree for orientation, graph for relation, source for
authority. Validators prove mechanics, not the truth of a boundary.
