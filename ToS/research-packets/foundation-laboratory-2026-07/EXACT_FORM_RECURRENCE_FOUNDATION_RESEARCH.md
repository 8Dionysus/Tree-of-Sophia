# Exact-Form Recurrence as Semantic Soil

Evidence refresh: 2026-08-02

Scope: the first transition from the exact four-part DTA lexical observation of
*Also sprach Zarathustra* toward later sign work

Authority posture: model-authored method research; no accepted German,
linguistic analysis, sign candidate, sign, concept, relation, canon judgment, or
human task

## Result first

Tree of Sophia should materialize recurrence as an **empirical observation
tuple**, not as a score for philosophical importance.

For every exact-form hash already present in the tracked lexical projection,
the next derived surface should retain separately:

- absolute occurrence count;
- range across the four published parts;
- range across source divisions;
- range across source pages;
- part-size-aware deviation of proportions (`DP`);
- maximum concentration in one part;
- source-editorial and unsectioned residue counts.

No weighted sum, rank fusion, stopword list, morphology result, embedding, or
LLM judgment should collapse those dimensions into `sign-likeness`. A stable
sign remains a later source-returnable interpretation requiring context,
translation evidence, alternatives, and a real-human promotion decision.

This resolves a current contract pressure without weakening the semantic gate:
exact recurrence can become queryable before German morphology or meaning is
accepted, while `initial-sign-packet.v3.json` remains content-free and blocked.

## Local evidence and exact pressure

The current source-withholding lexical projection already records 86,287 exact
occurrences, 11,352 exact-form rows, four source Items, 506 body pages, and 213
TEI divisions. Each form row carries its total count and per-Item page/division
hits, but downstream consumers have no named recurrence observation contract.

The semantic ladder correctly rejects frequency as sufficient evidence, but
its all-or-nothing packet gate currently makes mechanical recurrence look as if
it cannot exist until language competence and an accepted sign question exist.
Those are different epistemic layers:

1. exact form and distribution are source observations;
2. morphology, lemma, and lexical sense are linguistic analyses;
3. motif/sign and concept are interpretations;
4. promotion is a human authority event.

The new projection occupies only layer 1. It neither opens nor partially fills
the initial sign packet.

## I. Classical and official documentation

### OntoLex FrAC

The January 2025 public-review draft of the W3C Ontology-Lexica Community
Group's [Frequency, Attestation and Corpus Information
module](https://ontolex.github.io/frequency-attestation-corpus-information/)
models frequency and attestation as observations made in a named data source.
It distinguishes an observable form, sense, entry, or concept from the
observation about it and requires the counting unit and source to remain
explicit. Relative frequency is derived from absolute counts only when the
source total and unit are compatible.

ToS should adopt that separation but not claim FrAC conformance: an exact form
hash is the current observable; its source projection, count, segmentation
unit, and method remain explicit; no lexical entry, sense, or concept is
created.

### Source addressing

The existing TEI divisions and page-break resources are not interchangeable
documents. Part, division, and page range therefore stay as three different
observations. Resource identifiers are counted within their source Item so
same-looking local labels from different Items are never collapsed.

## II. Established method

### Frequency is insufficient

Stefan Th. Gries's 2008
[*Dispersions and adjusted frequencies in
corpora*](https://doi.org/10.1075/ijcl.13.4.02gri) demonstrates why total
frequency alone is misleading and defines `DP` as half the sum of absolute
differences between each corpus part's expected share of all tokens and its
observed share of the target form. Values near zero mean distribution roughly
proportional to part sizes; values near one mean strong concentration contrary
to those sizes.

The same paper warns that multiplying frequency and dispersion loses
information. Its proposed two-dimensional treatment is closer to the ToS
requirement than an adjusted-frequency leaderboard.

Gries's 2024 monograph
[*Frequency, Dispersion, Association, and
Keyness*](https://doi.org/10.1075/scl.115) strengthens that conclusion. It
argues that many familiar measures confound frequency with the property they
claim to measure and proposes **tupleization**: retain several interpretable
dimensions rather than force one number to carry all of them.

### Range remains useful

Sönning and colleagues' 2024
[*Evaluation of keyness metrics: performance and
reliability*](https://doi.org/10.1515/cllt-2022-0116) distinguishes
frequency- and dispersion-related dimensions and finds material reliability
differences among metrics. This supports keeping part/page/division range
visible instead of hiding it behind one composite measure. Their target is
keyword comparison across corpora, not philosophical signs; ToS borrows the
measurement caution, not the task definition.

## III. Freshest relevant evidence

### Simple range remains a serious baseline

Nohejl and Watanabe's 2025 multi-language study
[*Dispersion Measures as Predictors of Lexical Decision Time, Word
Familiarity, and Lexical Complexity*](https://arxiv.org/abs/2501.06536)
reports that log range was a stronger predictor than log frequency in their
tasks and the strongest addition to frequency among the tested measures. The
paper does not validate philosophical recurrence, but it prevents ToS from
dismissing simple structural range in favor of a fashionable complex score.

### Formulaicity is a separate question

Yoffe, Segev, and Sober's 2025 open-access
[*Unsupervised information-theoretic identification of formulaic
clusters*](https://doi.org/10.1017/chr.2025.10011) uses self-information to
identify structurally repetitive textual regions. It is relevant as a later
section-level challenger, especially for refrains and compositional forms. It
does not turn a dispersed word form into a motif and therefore does not belong
in the first exact-form observation projection.

### Motif detection still depends on motif evidence

Alyami and Finlayson's March 2026 preprint
[*Automated Motif Indexing on the Arabian
Nights*](https://arxiv.org/abs/2603.19283) is the freshest directly relevant
computational motif study found in this refresh. Its five method families were
trained and evaluated against 2,670 manually identified expressions of 200
motifs. That is strong evidence against treating unsupervised word recurrence
as motif truth: even a modern retrieval/embedding/LLM comparison begins with a
separately authored motif inventory and manual expression labels.

### Historical pipelines expose epistemic choices

David West Brown's May 2026
[*Reconsidering Corpus Methods in
HEL*](https://doi.org/10.1177/00754242261432217) emphasizes that every stage of
a historical-corpus pipeline embeds decisions about data, processing, and
interpretation. ToS therefore records segmentation, source totals, formula,
rounding, and excluded semantic effects in the projection itself.

### Fresh methods not adopted

The 2026 issue version of Hellström et al.'s
[*From keywords to key embeddings*](https://doi.org/10.1515/cllt-2024-0070)
combines text-dispersion keyness with multilingual word embeddings for web
register comparison. It is useful evidence for a later cross-language
challenger, not for the current single-work exact-form floor: it requires a
target/reference-corpus question and adds semantic model behavior before ToS
has accepted German forms or translations.

The freshness search through 2026-08-02 found no later directly relevant
method that justifies replacing the frequency/range/`DP` tuple or skipping the
manual sign boundary.

## Frozen A/B/C observation views

These are views over the same 11,352 hash-only form rows. They are not three
semantic annotators and do not compete for a winner yet.

| View | Retained evidence | What it can show | What it cannot show |
| --- | --- | --- | --- |
| A — frequency | absolute occurrence count | repetition volume | distribution, sense, motif, importance |
| B — structural range | part, division, and page range | breadth over declared source structures | proportionality to unequal part sizes, context, meaning |
| C — tupleized dispersion | A + B + part-size-aware `DP` + maximum part concentration | frequency and distribution without one fused score | lemma identity, contextual recurrence, sign stability |

All rows remain ordered by exact-form SHA-256, not by an output-sensitive rank.
Any future sample must freeze its question and selection rule before seeing
model or interpretive outputs.

## Calculation law

For form `w` and source parts `i = 1..4`:

```text
expected_i = tokens_in_part_i / tokens_in_all_parts
observed_i = occurrences_of_w_in_part_i / occurrences_of_w
DP(w)      = 0.5 * sum_i(abs(expected_i - observed_i))
```

The projection stores `DP * 1,000,000` as an integer, rounded to nearest with
ties to even. This avoids platform-dependent binary floating-point text while
keeping sufficient diagnostic precision. Maximum part concentration uses the
same parts and scale. Missing parts contribute an observed share of zero.

## Admission and stop lines

The recurrence projection may be generated because it reads only the current
tracked hash/count/resource projection. It stores no exact strings, sequence,
snippets, context, occurrence positions, morphology output, or model output.

It does not:

- accept or correct German;
- create an authoritative occurrence, lemma, or lexeme;
- select a sign candidate;
- schedule human work;
- open semantic A/B/C execution;
- authorize public/site use;
- alter the local-only source-payload rule.

The next semantic step is not “take the highest score.” It is: declare one
exact question, bind one source-returnable form/occurrence bundle, then inspect
context, linguistic evidence, translation correspondences, negation,
variation, and counterreadings before proposing a sign.
