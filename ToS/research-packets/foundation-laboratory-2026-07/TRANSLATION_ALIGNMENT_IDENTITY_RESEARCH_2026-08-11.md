# Translation alignment identity and provenance research

Snapshot: 2026-08-11.

Question: what must Tree of Sophia preserve before a source passage and a
translation passage can be related without turning a tool output, mutable text,
exchange file, or green validator into translation truth?

This report follows the required order: official and classical documentation,
then established research, then the freshest relevant 2025–2026 work. It is a
contract-design report, not a claim that any real translation alignment has
been accepted.

## 1. Official and classical documentation

### TEI P5 linking, segmentation, and alignment

- Current release note: [TEI P5 4.12.0, 28 July
  2026](https://www.tei-c.org/release/doc/tei-p5-doc/readme-4.12.0.html).
- Normative guidance used:
  [Linking, Segmentation, and Alignment](https://tei-c.org/release/doc/tei-p5-doc/en/html/SA.html).
- The chapter treats alignment as a special case of correspondence and offers
  `link`, `linkGrp`, `seg`, `anchor`, and stand-off annotation. Stand-off links
  are especially relevant when relations overlap or cannot safely be embedded
  in one text.
- ToS adoption: preserve stand-off alignment and explicit anchors.
- ToS non-adoption: a TEI link identifier is not the stable ToS alignment or
  claim identity. TEI is an export/projection surface, not owner truth.
- Version note: the chapter page may display a release label behind the current
  4.12.0 release note. The report preserves that documentation lag rather than
  silently treating every rendered page as the same release snapshot.

### W3C Web Annotation Data Model

- Final W3C Recommendation:
  [Web Annotation Data Model](https://www.w3.org/TR/annotation-model/).
- A `SpecificResource` separates the identity of a resource from the selector,
  state, style, and scope used to address it.
- Multiple direct bodies or targets apply independently. Composite/List
  constructions are not a sufficient normative foundation for ToS many-to-many
  translation semantics.
- ToS adoption: keep resource identity, frozen state, and selector distinct;
  retain source return.
- ToS extension: encode explicit ordered source and target member sets and a
  correspondence shape instead of relying on ambiguous multi-target behavior.

### OASIS XLIFF 2.1

- Final standard:
  [XLIFF Version 2.1](https://docs.oasis-open.org/xliff/xliff-core/v2.1/xliff-core-v2.1.html).
- XLIFF provides bilingual units, segments, source and target content, and
  permits a target to be added after the source. Its processing model also
  anticipates that target segments may be ordered differently.
- ToS adoption: use XLIFF as a possible derived interchange form and preserve
  reorder posture.
- ToS non-adoption: XLIFF unit/segment IDs do not replace Work/Expression/
  Edition/Item/File, frozen text-layer, anchor, alignment-claim, and review
  identity.

### TMX 1.4b

- Final specification:
  [TMX 1.4b](https://www.ttt.org/oscarStandards/tmx/tmx14b.html).
- A translation unit contains language variants and segments; segmentation
  rules are external to TMX, and `tuid` semantics are not a sufficient stable
  identity contract for ToS.
- ToS adoption: TMX may be generated for bounded exchange.
- ToS non-adoption: imported translation-memory grouping is evidence or a
  proposal, never automatic lexical equivalence or accepted alignment.

### OntoLex-Lemon VarTrans

- Community specification:
  [OntoLex-Lemon final model, Variation and Translation](https://www.w3.org/community/ontolex/wiki/Final_Model_Specification).
- Source and target roles are directional for asymmetric relations. The model
  distinguishes a sense-level translation relation from context-dependent
  translatability.
- ToS adoption: preserve direction and do not collapse contextual span
  correspondence into lexical or sense equivalence.

### W3C PROV-O

- Final W3C Recommendation:
  [PROV-O](https://www.w3.org/TR/prov-o/).
- Entity, Activity, and Agent support generation, attribution, and derivation
  records.
- ToS adoption: the alignment result, the activity that produced it, and the
  responsible maker remain separate.
- Boundary: provenance can show how a proposal arose; it does not establish
  that the mapping or translation is correct.

## 2. Established and high-impact research

### Classical statistical baselines

- Gale and Church, 1993,
  [A Program for Aligning Sentences in Bilingual
  Corpora](https://aclanthology.org/J93-1004/): sentence-length evidence is a
  classical, fast baseline. It is not content truth and must not be omitted
  merely because a newer neural method exists.
- Och and Ney, 2003,
  [A Systematic Comparison of Various Statistical Alignment
  Models](https://aclanthology.org/J03-1002/): word alignment is a relation over
  positions; null and unaligned cases are part of the model rather than broken
  records.

### Alignment meaning and scarce gold

- Xu and Yvon, 2016,
  [Novel elicitation and annotation schemes for sentential and sub-sentential
  alignments](https://aclanthology.org/L16-1099/): hand-aligned resources for
  difficult and non-literal bitext are scarce, and link semantics can be
  underspecified.
- ToS consequence: shape, order, technique, certainty, maker, evidence, and
  review status must be explicit, and ambiguity must remain first class.

### Multilingual embedding aligners

- Jalili Sabet et al., 2020,
  [SimAlign](https://aclanthology.org/2020.findings-emnlp.147/).
- Dou and Neubig, 2021,
  [Word Alignment by Fine-tuning Embeddings on Parallel
  Corpora](https://arxiv.org/abs/2101.08231) (`awesome-align`).
- ToS consequence: these are useful challenger methods whose outputs enter as
  proposals with exact software/model/configuration provenance. Performance in
  a benchmark does not authorize acceptance in a philosophical witness.

### Literary and document-level evidence

- Thai et al., 2022,
  [Exploring Document-Level Literary Machine
  Translation](https://aclanthology.org/2022.emnlp-main.672/): paragraph-aligned
  public-domain literature showed a large human-reference preference while
  automatic metrics failed to track that literary judgment reliably.
- Jiang et al., 2023,
  [A Discourse-Centric Evaluation of Document-level Machine Translation With a
  New Parallel Corpus of Novels](https://aclanthology.org/2023.acl-long.435/):
  entity, terminology, coreference, and quotation consistency require document
  context beyond isolated sentences.
- Kazantseva et al., 2019,
  [An Experiment in Automatic Alignment of Parallel Literary
  Texts](https://aclanthology.org/W19-2505/): literary resource construction
  benefits from semi-automatic post-processing rather than unchecked automatic
  projection.

### Translation techniques are claims, not intrinsic link types

- Zhai et al., 2018,
  [A Multilingual Corpus of Automatically Extracted Translation
  Relations](https://aclanthology.org/W18-3814/).
- Zhai et al., 2020,
  [Sub-sentential Translation Techniques and Their
  Classification](https://aclanthology.org/2020.lrec-1.496/).
- Literal, idiomatic, generalizing, particularizing, modulating, compensating,
  and paraphrastic descriptions can be useful. ToS records them as contestable
  claim attributes with evidence, not as unquestionable properties of text.

## 3. Fresh 2025–2026 research

Freshness is not authority. Each item is classified by publication state and
fit to the ToS foundation.

### Directly relevant and adopt now

- Janssen, Lendvai, and Jouravel, RANLP 2025,
  [Analysing the Performance of Automated Text Alignment for Historical
  Documents](https://aclanthology.org/2025.ranlp-1.55/).
  - Fit: historical manuscript transcriptions, editions, and translations,
    evaluated against sentence-level human gold.
  - Result relevant to method design: Hunalign outperformed the evaluated deep
    methods on this material.
  - Adoption: every ToS alignment experiment must retain a classical baseline;
    model recency alone is not a selection rule.
- Akbik et al., Findings of EMNLP 2025,
  [TransAlign](https://aclanthology.org/2025.findings-emnlp.1129/).
  - Fit: a strong multilingual encoder challenger for word alignment and label
    projection.
  - Adoption: challenger lane only, with exact revision/configuration and no
    automatic acceptance.
- LITEVAL, NAACL 2025,
  [A Simple and Effective Evaluation Method for Literary
  Translation](https://aclanthology.org/2025.naacl-long.548/).
  - Fit: evaluation design for literary translation.
  - Adoption: prefer small, comprehensible human decisions with explicit
    competence and visible source/target evidence. A complex rubric or an
    automatic score must not manufacture a trustworthy decision.

### Diagnostic baseline

- [Degree Zero of Translation: An Interlinear Baseline for Measuring
  Intervention](https://aclanthology.org/2026.latechclfl-1.22/), 2026.
- Fit: an interlinear baseline can expose translator intervention.
- Adoption: diagnostic baseline only. It is neither semantic truth nor the
  ideal translation and must remain separate from recognized translations.

### Watchlist, not foundation authority

- June 2026 preprint,
  [Reader-Centered Evaluation of Literary Machine
  Translation](https://arxiv.org/abs/2606.26040): reader preferences over
  aligned novel chunks are highly relevant, but the item remains a preprint.
- June 2026 preprint,
  [ClinicalAligner](https://arxiv.org/abs/2606.08673): domain-specific alignment
  may inform later specialization, but clinical data are not evidence that the
  method fits historical philosophy.
- ACL Industry 2026,
  [Multi-aspect Literary Translation](https://aclanthology.org/2026.acl-industry.22/):
  relevant to translation generation/evaluation, peripheral to durable
  alignment identity.
- [LoResMT 2026 proceedings](https://aclanthology.org/volumes/2026.loresmt-1/):
  useful for tracking manually aligned low-resource corpora, not a replacement
  for ToS source-specific proof.

## 4. Contract conclusions

The foundation must preserve all of the following independently:

1. Exact source and target Work/Expression/Edition/Item/File identities.
2. Exact file and frozen text-layer digests on both sides.
3. Frozen segmentation and, where applicable, tokenization artifacts.
4. Ordered anchor members resolving against those exact layers; no naked
   offsets or detached token positions.
5. Opaque alignment and claim IDs that do not derive from text, labels,
   translations, or the current mapping interpretation.
6. Direction, granularity, correspondence shape, and order posture as separate
   dimensions.
7. Source omission, target addition, and unresolved members as first-class
   outcomes, not missing data.
8. Technique descriptions as contestable claims.
9. Maker, method, time, software/model/configuration, provenance, certainty,
   evidence, status, competing mappings, supersession, and review refs.
10. Reciprocal competing mappings without forced collapse.
11. Machine/software/model output restricted to proposal states.
12. Acceptance only through a separate source-and-target-visible real-human
    review with declared source-language, target-language, and translation
    competence plus a frozen unassisted baseline.
13. The most restrictive source, target, or packet visibility inherited by all
    derivatives.
14. TEI, Web Annotation, XLIFF, TMX, and graph outputs treated only as
    reproducible, rights-safe projections of accepted owner claims.

## 5. A/B/C laboratory decision

- A: one deterministic one-to-one structural proposal over two invented
  private-use language tags. Valid mechanics; no translation act or truth.
- B: reciprocal one-to-one and one-to-many proposals over the same invented
  source member. Both remain proposed and no winner is chosen.
- C: a synthetic/model-shaped proposal is marked accepted without review or
  competence. It must fail both schema and semantic closure.

The laboratory uses no Nietzsche text, no private source, no real translation,
no model invocation, and no human review. Green results can establish only that
the mechanical boundary fails closed.
