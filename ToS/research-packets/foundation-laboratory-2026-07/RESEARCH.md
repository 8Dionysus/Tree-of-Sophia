# Corpus Foundation, Semantics, and Laboratory Research

Status: non-authoritative research synthesis
Snapshot: 2026-07-22; semantic-annotation method and direct-visual execution
route refreshed 2026-07-29; exact standalone Nietzsche source-route research
and the *Ecce Homo* / *Zarathustra* / *Jenseits von Gut und Böse*
authorial-witness routes refreshed 2026-07-30
Scope: corpus identity, source witnesses, provenance, anchors, signs,
translation, annotation, retrieval, graph projection, discovery, and rights

## Outcome first

The proposed foundation is a source-returnable, claim-bearing corpus rather
than a graph of prematurely frozen concepts.

The minimum useful unit is not `concept:overman`. It is closer to:

```text
digital item + digest
  + edition and expression identity
  + durable passage/region address
  + exact observed form
  + observation or assertion type
  + agent and method provenance
  + review state and history
```

This permits stable signs without pretending that their meanings are
immutable. For example, the occurrence of `Übermensch` in a fixed German
witness at a fixed passage is evidence. A proposed lemma, semantic sense,
etymology, Russian equivalent, motif membership, or relation to another
passage is a separately identified assertion. It may be accepted, split,
revised, disputed, or rejected without changing the witness.

The graph is therefore a projection of reviewed claims, not the container of
truth. Search indexes are also rebuildable projections. The tracked corpus
catalog, contracts, claims, decisions, and receipts remain authoritative.

### Golden-kernel source correction

The 2026-07-30 authorial-witness refresh strengthens this conclusion at the
heart of ToS. The current archive and critical route exposes no one immutable
four-part *Zarathustra* original:

- the print manuscripts for parts I-III are not preserved according to the
  current critical sigla;
- D 17 (`GSA 71/25`) is the surviving authorial print manuscript for part IV;
- notebooks and loose leaves mix *Zarathustra* stages with *Fröhliche
  Wissenschaft*, *Jenseits von Gut und Böse*, *Ecce Homo*, and other work;
- the corrected Peter Gast private-print ensemble (`GSA 71/25a`) preserves a
  distinct late-correction route that the holding evidence says did not enter
  later editions;
- the whole-work eKGWB surface supplies 181 stable critical addresses and 47
  correction popups, not manuscript leaves or an admitted author-final text.

The foundation is therefore stable witness identity plus region-level,
versioned provenance and responsibility claims. Exact words become stable
observations only inside that addressable witness. Meaning, preferred reading,
authorial finality, translation, and conceptual relation remain separately
reviewable assertions.

The freshest directly relevant 2026 development is not a new manuscript
identity but the forthcoming August 2026 Stanford translation by Paul S.
Loeb and David F. Tinsley. It is registered as a deferred metadata candidate
until publication, manifestation, rights, and passage-level suitability are
actually verified.

### First cross-work source correction

The same law now reaches the first prepared work beyond the golden kernel.
*Jenseits von Gut und Böse* is not one line from notebook to final text:

- W I 3-W I 8 contain region-specific preparatory material alongside other
  work and therefore cannot be admitted wholesale;
- D 18 (`GSA 71/26`) is a cut, assembled, reordered, and repeatedly
  renumbered print manuscript with several compositional stages;
- HAAB C 4615 is a partial correction proof, while C 4619 is Nietzsche's
  complete correction copy and carries later authorial changes;
- the 1886 first print, later numbering history, and the 302-address eKGWB
  surface remain distinct witnesses;
- ORES/Kalliope and DFGA expose different leaf, canvas, sheet, image, and
  logical-address counts that require a future crosswalk, not normalization.

Axel Pichler's 2025 textual-genetic use of §22 gives ToS its first exact
textual-genetic method candidate outside the kernel: W I 7 leaves 44v-45r ->
D 18 leaf 27r -> published §22. It remains deferred until exact source access,
rights, independent diplomatic reading, German competence, and a blind
stage-comparison protocol exist. The three witness stages must not be confused
with the no-kernel / method-only / reviewed-kernel A/B/C variants. No text,
semantics, transfer unit, or human task was admitted.

## Research method

Sources were examined in the requested order. Primary standards, official
documentation, official model cards, official repositories, and publisher
paper pages were preferred. Freshness was treated as evidence to evaluate,
not as an automatic rank.

The investigation also inspected the three proposed local witnesses without
altering them. Their bytes, structure, and extractability materially shaped
the recommendations.

---

## Layer I — classical standards and official documentation

### 1. Bibliographic and object identity

[IFLA Library Reference Model (LRM), 2024 edition](https://repository.ifla.org/items/214c74cb-c075-4428-a138-39f8d06c55aa)
provides the durable distinction between Work, Expression, Manifestation, and
Item. ToS should adapt this distinction internally because a philosophical
work, a German text state, a 2007 Russian edition, and one acquired PDF are
not the same object.

[LRMoo 1.1.1](https://cidoc-crm.org/lrmoo/fm_releases), the current Official
(IFLA) release from November 2025, connects the bibliographic model to
event-centric cultural-heritage modeling. It is useful where creation,
translation, publication, item production, digitization, and acquisition need
explicit and non-equivalent events.

[BIBFRAME 2.0](https://www.loc.gov/bibframe/docs/bibframe2-model.html) is a
necessary library-data crosswalk, especially for external catalog exchange.
It should not be the sole internal authority because its operational
Work/Instance/Item shape is less expressive than the edition, translation,
and textual-witness distinctions required here.

[CIDOC CRM version history](https://cidoc-crm.org/versions-of-the-cidoc-crm)
shows 7.3.2 as the newest draft and 7.1.3 as the current Official (ISO
Correspondence) implementation line in the 2026-07-31 refresh. CRM/LRMoo
should guide event and cultural-object interoperability, not force the first
local storage representation or turn a draft number into claimed conformance.

**Decision:** internal identities follow a local LRM-shaped profile; external
BIBFRAME and LRMoo/CRM mappings are explicit projections.

### 2. Addressing text and pages

[Canonical Text Services URNs 2.0.rc.1](https://cite-architecture.github.io/ctsurn_spec/)
offer a valuable, technology-independent pattern for work hierarchies and
scholarly passage citation. The specification is an old release candidate,
so ToS should borrow the principle, not claim CTS conformance.

[TEI P5 4.11 stand-off markup](https://tei-c.org/release/doc/tei-p5-doc/en/html/ref-standOff.html)
separates annotations from the base text. [TEI critical apparatus](https://tei-c.org/release/doc/tei-p5-doc/en/html/TC.html)
supports lemmas, readings, witnesses, and external apparatus. These patterns
fit editions, alternative readings, and translation comparison.

[W3C Web Annotation Data Model](https://www.w3.org/TR/annotation-model/)
supplies targets, selectors, bodies, motivations, and provenance-friendly
annotation objects. Text position selectors alone are too fragile; quote and
structural selectors should travel with them.

[IIIF Presentation API 3.0](https://iiif.io/api/presentation/3.0/) models a
page-like Canvas and regions on it. This gives scanned pages and PDF-rendered
surfaces a durable visual fallback when OCR changes.

**Decision:** an anchor combines a local structural passage ID, exact quote
with prefix/suffix, witness-version digest, and, when present, page/Canvas and
region coordinates. Byte or character offsets are conveniences, never the
only address.

### 3. Provenance, preservation, identity, and normalization

[PROV-O](https://www.w3.org/TR/prov-o/) distinguishes entities, activities,
and agents. [PREMIS 3.0](https://www.loc.gov/standards/premis/v3/index.html)
adds preservation events, objects, agents, and rights. Together they cover
acquisition, extraction, OCR, correction, normalization, alignment,
translation, annotation, review, and export without collapsing them into one
opaque pipeline run.

[BagIt, RFC 8493](https://www.rfc-editor.org/info/rfc8493/) provides a simple
transfer-package pattern with manifests. The local corpus need not be a Bag
internally, but every future server upload should be able to emit or accept a
BagIt-like inventory with checksums and receipts.

[SPDX 3.0.1 AI profile](https://spdx.github.io/spdx-spec/v3.0.1/model/AI/AI/)
is relevant to model and dataset bills of materials. It does not replace
work-level copyright research.

[JSON-LD 1.1](https://www.w3.org/TR/json-ld11/) permits claim packets to stay
reviewable JSON while carrying resolvable graph semantics.

[SHACL 1.0](https://www.w3.org/TR/shacl/) is the stable validation baseline.
[SHACL 1.2 Core](https://www.w3.org/TR/shacl12-core/) is still a draft in this
snapshot and must not silently become the contract baseline.

[Unicode Normalization UAX #15, Unicode 17](https://unicode.org/reports/tr15/)
supports NFC for a normalized text layer. Original bytes and diplomatic text
must remain unnormalized so typography, historical spelling, decomposed
characters, and OCR behavior are not erased.

### 4. Document formats and OCR interchange

[EPUB 3.3](https://www.w3.org/TR/epub-33/) is the applicable publication
standard; the recommendation was republished in January 2026 without making
automatically generated OCR reliable.

[OCR-D METS conventions and workflows](https://ocr-d.de/en/workflows),
[ALTO XML](https://altoxml.github.io/), and
[PAGE XML](https://primaresearch.org/tools) establish useful patterns for
page, region, line, word, coordinates, confidence, and processing steps.
ToS should accept PAGE/ALTO as laboratory interchange while keeping its own
source item and anchor identities above either format.

### 5. Lexical, semantic, translation, and rights description

[OntoLex-Lemon](https://www.w3.org/2016/04/ontolex/) supplies useful
LexicalEntry, Form, Sense, and translation/relation patterns. It is a W3C
Community Group report, not a W3C Recommendation. It can guide a lexical
projection but must not turn a disputed philosophical sense into an
authoritative dictionary fact.

[MQM 2.0](https://themqm.org/error-types-2/typology/) supplies a current,
explicit translation error typology. ToS should extend it with literary and
philosophical dimensions rather than treating one MQM score as truth.

[W3C ITS 2.0](https://www.w3.org/TR/its20/) includes translation provenance
and localization-quality issue metadata and supports stand-off use.

[ISO 17100:2015](https://www.iso.org/standard/59149.html) was under revision
at the snapshot date and explicitly excludes raw machine translation plus
post-editing from its scope. ToS must not claim ISO 17100 compliance. Its
separation of translation and independent revision remains a useful human
process principle.

[RightsStatements.org](https://rightsstatements.org/en/statements/) gives
standardized rights categories, while [ODRL 2.2](https://www.w3.org/TR/odrl-model/)
can express permissions, prohibitions, duties, and constraints. A rights
statement records a researched posture; it does not manufacture permission.

### Classical-layer synthesis

The stable floor has four different kinds of stability:

| Kind | Stable object | What may change |
| --- | --- | --- |
| fixity | acquired item bytes and digest | a later item is a new version/item |
| identity | persistent work/expression/edition/item IDs | metadata assertions about them |
| address | passage/region identity within a declared witness version | offsets and derived segmentation |
| history | append-only proposal/review/preservation events | current accepted state |

Concepts and semantics do not belong in the fixity column. They become durable
through identity, evidence, version history, and review, not through a claim
that interpretation has ended.

---

## Layer II — leading papers and established tools

### OCR and document structure

- [DocLayNet](https://arxiv.org/abs/2206.01062) established a large,
  human-annotated layout-analysis benchmark across diverse document classes.
- [Docling technical report](https://research.ibm.com/publications/docling-technical-report)
  describes document conversion that keeps layout and reading order.
- [eScriptorium](https://escriptorium.eu/about/) and
  [OCR4all](https://arxiv.org/abs/1909.04032) represent mature
  human-in-the-loop historical OCR/transcription paths.
- [OCR-D and OCR4all integration](https://ceur-ws.org/Vol-2981/short2.pdf)
  demonstrates why standardized page/workflow exchange matters.
- [PreP-OCR](https://aclanthology.org/2025.acl-long.749/) treats OCR
  post-correction as an evaluated model problem rather than harmless cleanup.
- [LLM post-OCR correction](https://aclanthology.org/2024.lt4hala-1.14/) and
  [CuReD](https://aclanthology.org/2024.ml4al-1.14/) show potential but also
  reinforce that corrections need aligned before/after evidence.

The leading-tool conclusion is a staged path: native extraction first,
layout-aware parsing second, OCR only where needed, model correction only as
a proposal, and visible-source review before promotion.

### Translation and evaluation

- [MQM expert evaluation](https://aclanthology.org/2021.tacl-1.87/) supports
  explicit expert error annotation and demonstrates the importance of
  context.
- [COMET](https://aclanthology.org/2020.emnlp-main.213/) is a strong learned
  MT metric, but remains a metric rather than an adjudicator.
- [Literary translation with document context](https://aclanthology.org/2023.wmt-1.41/)
  shows that critical errors can be missed by sentence-local evaluation.
- [Multi-aspect translation evaluation](https://aclanthology.org/2024.tacl-1.13/)
  supports separate dimensions rather than a single opaque score.
- [Literary Translation Evaluation with Humans and LLMs](https://aclanthology.org/2025.naacl-long.548/)
  finds that generic automatic metrics and unmodified MQM are inadequate for
  literary translation; expert best-worst judgments are more discriminating.
- [NLLB](https://arxiv.org/abs/2207.04672) and
  [MADLAD-400](https://arxiv.org/abs/2309.04662) are useful multilingual MT
  baselines, but license, domain, context, and hardware fit must be evaluated
  separately.

ToS therefore needs a translation packet containing original text,
diplomatic and normalized forms, interlinear gloss, etymological notes,
independent AI/human drafts, recognized translation comparators, error labels,
and adjudication. Fluency, faithfulness, semantics, terminology, rhythm,
imagery, ambiguity, and intervention are separate axes.

### Alignment

[Vecalign](https://aclanthology.org/D19-1136/) provides a linear-time
sentence-alignment method based on bilingual sentence embeddings. It is a
useful proposal generator after structural divisions are aligned. It cannot
decide that two philosophically non-equivalent sentences are equivalent.

The preferred order is edition structure, headings and paragraphs first;
sentence embeddings second; token/phrase alignment third; human acceptance
last. One-to-many, many-to-one, omission, addition, reordering, and
unresolved mappings must be first-class states.

### Retrieval

- [BEIR](https://arxiv.org/abs/2104.08663) demonstrates that retrieval models
  often fail to generalize across domains.
- [MTEB](https://arxiv.org/abs/2210.07316) and
  [MMTEB](https://proceedings.iclr.cc/paper_files/paper/2025/hash/fc0e3f908a2116ba529ad0a1530a3675-Abstract-Conference.html)
  establish broad and multilingual embedding evaluation.
- [MIRACL](https://aclanthology.org/2023.tacl-1.63/) is a multilingual
  retrieval benchmark relevant to German/Russian/English search.
- [BGE-M3](https://aclanthology.org/2024.findings-acl.137/) combines dense,
  sparse, and multi-vector retrieval modes.
- [ColPali](https://arxiv.org/abs/2407.01449) motivates page-image retrieval
  when OCR loses layout or visual evidence.

No public benchmark represents ToS questions. A tiny expert-authored query
set with exact relevant passages, hard lexical distractors, implicit semantic
matches, and explicit non-matches is required before choosing an index.

### Annotation and knowledge representation

- [INCEpTION](https://aclanthology.org/C18-2002/) provides customizable span,
  relation, chain, recommender, agreement, and curation workflows.
- [CATMA](https://catma.de/) supports exploratory, qualitative literary
  annotation and TEI export.
- [Nanopublications](https://arxiv.org/abs/1809.06532) provide an assertion,
  provenance, and publication-info pattern suitable for small reviewable
  claims.
- [GraphRAG](https://arxiv.org/abs/2404.16130) shows the utility of graph-based
  retrieval summaries but does not justify giving a generated graph source
  authority.

The current official [INCEpTION documentation](https://inception-project.github.io/documentation/)
exposes multi-user annotation, curation, knowledge bases, agreement, custom
layers, and recommenders. Its built-in versioning remains operational support,
not the ToS canonical record. [CATMA 7](https://catma.de/documentation/technology-and-versions/)
uses a Git-backed project representation and remains a valuable qualitative
counterpoint. Both should export into ToS claim packets; neither owns canon.

---

## Layer III — freshest relevant evidence and current software

### Fresh papers that change the method

The following 2026 results are especially consequential:

- [From OCR to Analysis: Tracking Correction Provenance in Digital Humanities Pipelines](https://aclanthology.org/volumes/2026.nlp4dh-1/)
  makes span-level correction type, source, confidence, and revision history
  first-class because different correction paths change later entity and
  interpretive results.
- [Matching Meaning at Scale: Evaluating Semantic Search for 18th-Century Intellectual History through Locke](https://aclanthology.org/volumes/2026.nlp4dh-1/)
  finds implicit reception that lexical search misses, while also exposing
  lexical gatekeeping and requiring an expert relevance taxonomy.
- [Scaling Sentence Similarity for Classical Tibetan](https://aclanthology.org/volumes/2026.nlp4dh-1/)
  shows that silver ensembles and LLM committees become useful only when
  bounded by domain gold data.
- [HisDoc-OCR](https://aclanthology.org/2026.findings-acl.301/) reports
  historically dangerous VLM OCR behavior: language priors can hallucinate
  plausible text and introduce semantic drift. Visual grounding cannot be
  optional.
- [Beyond Literal Mapping](https://aclanthology.org/2026.acl-long.205/) shows
  inconsistent evaluation of non-literal translation by metrics and LLM
  judges.
- [Beyond Reproduction](https://aclanthology.org/2026.findings-acl.2030/)
  separates textual comprehension from creative reproduction; fluent output
  can remain literal or contextually wrong.
- [Fluency and Faithfulness](https://aclanthology.org/2026.nlp4dh-1.17/)
  reports a real trade-off between the two and sensitivity to segment length.
- [Degree Zero of Translation](https://aclanthology.org/2026.latechclfl-1.22/)
  uses an interlinear baseline to make translator intervention measurable.
- [Better Literary Translation](https://aclanthology.org/2026.acl-industry.22/)
  supports multi-aspect refinement but also reports that preference
  optimization can degrade results.
- [GraphRefine](https://aclanthology.org/2026.acl-long.1353/) shows that
  extracted knowledge graphs require delete/edit/rewrite refinement. ToS must
  preserve superseded and rejected proposals instead of silently mutating
  graph truth.
- [Stoic LLM](https://aclanthology.org/volumes/2026.nlp4dh-1/) shows that a few
  hundred high-fidelity examples can align a small model while important
  philosophical failure modes persist. This supports a small golden kernel
  and rejects automatic ontology transfer.

These results converge on the planned manual laboratory: small gold before
scale; visible correction provenance; multiple evaluation dimensions; blind
independent drafts where possible; explicit rejections; and no LLM-as-judge
promotion gate.

### Current OCR and document software

At the snapshot date:

- [PaddleOCR 3.7.0](https://github.com/PaddlePaddle/PaddleOCR/releases/tag/v3.7.0)
  is the current June 2026 release. PP-OCRv6 does not cover Russian in this
  release, so it is not a fair main route for the German/Russian packet. The
  admitted C route instead uses `PP-OCRv5_server_det` with
  `latin_PP-OCRv5_mobile_rec` for German and
  `eslav_PP-OCRv5_mobile_rec` for Russian, as listed in the official
  [multilingual PP-OCRv5 documentation](https://paddlepaddle.github.io/PaddleOCR/latest/en/version3.x/algorithm/PP-OCRv5/PP-OCRv5_multi_languages.html).
  PaddleOCR-VL remains a separate structure/VLM candidate, not a trusted
  transcriber and not OCR C.
- [Tesseract 5.5.2](https://github.com/tesseract-ocr/tesseract/releases/tag/5.5.2)
  remains the deterministic classical A baseline and can emit plain text,
  hOCR, TSV, and ALTO. Fedora 44 currently carries 5.5.2 plus German and
  Russian 4.1.0 language packs, but the laboratory must still isolate and
  checksum the binary and `traineddata` rather than mutate the host.
- [Kraken 7.0.2](https://github.com/mittagessen/kraken/releases/tag/7.0.2)
  plus [Party](https://github.com/mittagessen/party) at commit
  `c2589b1b515ed690f883c6afaef6c01ce29bf72d` is the trainable historical-print
  B route. The exact multilingual Party v4 model is
  [Zenodo 10.5281/zenodo.20642057](https://zenodo.org/records/20642057), not the
  older concept record linked by some documentation. Its reported held-out
  German and Russian CER values are model-card context, not evidence on ToS.
  The model mixes public and access-restricted/private training witnesses and
  inconsistent transcription conventions, so any later fine-tuning or
  transfer requires a dataset-level provenance and normalization audit.
- Kraken 7.0.2 is also a security-sensitive freeze: its release pins
  Lightning 2.6.1 after compromised 2.6.2/2.6.3 packages. The laboratory must
  resolve that exact safe dependency rather than accept an unconstrained
  newest version.
- [OCRmyPDF 17.8.1](https://github.com/ocrmypdf/OCRmyPDF/releases/tag/v17.8.1)
  may package a winning OCR stream into a searchable derivative. It is not a
  recognition contestant or rasterizer, and a derived PDF must never replace
  the source item.
- [Docling 2.114.0](https://github.com/docling-project/docling/releases/tag/v2.114.0)
  was released 2026-07-20. Versions before 2.91 are excluded because of
  [CVE-2026-44022](https://nvd.nist.gov/vuln/detail/CVE-2026-44022).
  [Docling pipeline guidance](https://docling-project.github.io/docling/examples/agent_skill/docling-document-intelligence/pipelines/)
  supports standard, hybrid, and VLM paths; forcing available backend text is
  safer than gratuitous OCR.

### Current local-model and runtime direction

- [Qwen3-Embedding-0.6B](https://huggingface.co/Qwen/Qwen3-Embedding-0.6B)
  is a compact multilingual embedding candidate already present locally.
- [gte-multilingual-base](https://huggingface.co/Alibaba-NLP/gte-multilingual-base)
  is a 305M, 75-language historical challenger. Its repository was last
  modified in July 2025 and its documented SentenceTransformers route asks
  callers to enable remote model code, so it is not the best independent C
  route for this host.
- [BGE-M3](https://huggingface.co/BAAI/bge-m3) remains an important
  dense+sparse+multi-vector reference, but its roughly 2.27 GB weight artifact
  and multi-mode integration would add both storage and a second experimental
  variable to this 24-passage pilot.
- [Granite Embedding 311M Multilingual R2](https://huggingface.co/ibm-granite/granite-embedding-311m-multilingual-r2)
  is the selected independent text challenger for Retrieval C. IBM published
  the repository in April 2026, updated it in May 2026, and documents Apache
  2.0 licensing, more than 200 languages, explicit German/Russian training
  improvements, 32K input support, CLS pooling, L2 normalization, and official
  OpenVINO exports. The accompanying [R2 paper](https://arxiv.org/abs/2605.13521)
  and [owner repository](https://github.com/ibm-granite/granite-embedding-models)
  make this a fresher, independent family than the resident Qwen baseline.
- [Qwen3-VL-Embedding-2B](https://huggingface.co/Qwen/Qwen3-VL-Embedding-2B)
  is the 2026 visual-document retrieval challenger in a separately frozen
  experiment; it is not substituted for the text challenger. The exact
  revision, 20-query/36-page-image plan, acquisition, isolated CPU runtime,
  source-to-image crosswalk, image-use audit, and offline boundary are now
  fixed. After a preserved memory-gated preflight, a fresh owner admission
  returned `ready` without force and audit-complete r9 executed all 36 images
  and 20 queries. Mechanical ranking, anchor, normalization, latency, memory,
  swap, and route-recovery evidence now exists; human image relevance,
  hard-negative correctness, quality, winner, adoption, and promotion do not.
- [Qwen3.5-4B](https://huggingface.co/Qwen/Qwen3.5-4B) is the plausible fresh
  small general-model challenger. Its very large default context should be
  reduced locally.
- [Qwen3.6](https://github.com/QwenLM/Qwen3.6) begins at a much larger total
  weight even for the 35B-A3B mixture; it is not a first-wave fit for this
  machine.
- [Gemma 4 E2B](https://huggingface.co/google/gemma-4-E2B-it) and
  [Gemma 4 E4B](https://huggingface.co/google/gemma-4-E4B-it) are current
  small open-weight candidates already represented by host GGUFs.
- [TranslateGemma 4B](https://huggingface.co/google/translategemma-4b-it) is a
  specialized, license-gated translation candidate with a short input limit;
  it belongs after the license and baseline gates, not in the default path.
- [OpenVINO 2026.2](https://docs.openvino.ai/2026/about-openvino/release-notes-openvino.html)
  supports current Intel CPU/GPU/NPU inference families. The
  [NPU guide](https://docs.openvino.ai/2026/openvino-workflow-generative/inference-with-genai/inference-with-genai-on-npu.html)
  requires compatible low-bit conversion and model-specific constraints.
- [llama.cpp SYCL](https://github.com/ggml-org/llama.cpp/blob/master/docs/backend/SYCL.md)
  supports Intel GPU, while [llama.cpp](https://github.com/ggml-org/llama.cpp)
  also exposes Vulkan and an evolving OpenVINO backend.

Fresh model availability does not displace existing resident models. A new
artifact enters only after the resident route has produced a concrete result
and an independent-family comparison is required by the frozen A/B/C method.

#### Retrieval C admission frozen before execution

The Retrieval C method is frozen as IBM
`granite-embedding-311m-multilingual-r2` at repository revision
`44399559930365213510b1ee2eb15ded83374f0e`. Only the official quantized
OpenVINO IR, tokenizer, pooling/configuration files, model card, and license
metadata are admitted. The remote dry run reports about 350 MB for that
selected set, rather than the approximately 4.1 GB full repository containing
duplicate PyTorch, ONNX, and full-precision OpenVINO exports.

The executed route uses the host's OpenVINO 2026.2 on CPU, a small isolated
tokenizer runtime, owner-routed cache and runtime roots, and no remote-code
execution. The selected 11 files total 348,082,051 bytes. This selection was
based on recency, license, language fit, independent model family, official
Intel artifact, and bounded storage—not on unseen Retrieval C quality. The
same frozen 20 queries, 24 non-empty passages, anchors, and manual review
protocol used by A/B remain authoritative. Two post-freeze executions produced
identical indexes and rankings, but human relevance remains unreviewed.
BGE-M3 and gte-multilingual-base stay documented comparators;
Qwen3-VL-Embedding-2B has entered the distinct visual-search experiment with
an exact host-owned runtime. Audit-complete r9 now supplies direct-page-image
ranking, anchor, normalization, and resource evidence, while the five-query
human relevance trigger remains unscheduled and content-quality, adoption,
promotion, and winner evidence remain absent.

### Current graph and retrieval stores

- [PyOxigraph 0.5.9](https://pypi.org/project/pyoxigraph/), released
  2026-06-18, is the exact reviewed and installed Python binding for the
  lightweight MIT/Apache Oxigraph RDF/SPARQL engine. Its
  [current API](https://pyoxigraph.readthedocs.io/en/stable/) supports
  in-memory and disk stores, named graphs, and SPARQL 1.1. The project itself
  still warns that development is active and query evaluation is not yet
  optimized, so the successful 13-claim pilot cannot establish large-corpus
  maturity.
- [Neo4j operations documentation](https://neo4j.com/docs/operations-manual/current/)
  governs the service route. The laboratory actually used the resident Neo4j
  5.26.26 Community service; that makes it a useful property-graph trial but
  does not provide isolated store costing or the multi-user authorization
  posture needed for a canon authority.
- [Qdrant releases](https://github.com/qdrant/qdrant/releases) make it a
  current vector-index candidate. It remains a retrieval projection only.

RDF-star 1.2 is not adopted as the authoritative claim model while its
specification and quoted-triple semantics remain less stable than explicit
claim resources. JSON-LD claim packets with explicit assertion identity,
provenance, review, and named-graph projection are safer.

---

## Semantic annotation assurance refresh — 2026-07-29

This focused refresh repeated the requested order rather than letting the
July 2026 date of a paper substitute for methodological weight.

### I. Current primary standards and official documentation

- [TEI P5 4.12.0 release notes](https://www.tei-c.org/release/doc/tei-p5-doc/readme-4.12.0.html)
  establish 4.12.0 as the 2026-07-28 release. Some generated `standOff` and
  responsibility pages still displayed 4.11.0 during the refresh, so ToS
  records the release note as current version evidence without pretending
  that every deployed page had already converged. TEI stand-off annotations,
  `@source`, `@resp`, `@cert`, spans, and structured uncertainty remain useful
  interchange patterns, not ToS semantic authority.
- The [W3C Web Annotation Data Model](https://www.w3.org/TR/annotation-model/)
  and [Selectors and States](https://www.w3.org/TR/selectors-states/) keep an
  annotation body distinct from its target and support quote, position,
  fragment, and state selectors. ToS should carry quote plus structural and
  witness-digest context; position alone is too brittle.
- [PROV-O](https://www.w3.org/TR/prov-o/) keeps entities, activities, agents,
  derivations, quotations, revisions, and responsibility explicit. A label,
  rationale, alternative, model suggestion, human baseline, and later
  promotion are therefore different provenance-bearing entities or events.
- [OntoLex-Lemon](https://www.w3.org/2016/05/ontolex/) keeps lexical entries,
  forms, senses, and concepts distinct. The 2016 core and 2019 Lexicography
  module are W3C Community Group reports, while the current Frequency,
  Attestation and Corpus Information work remains emerging community work.
  It can guide a projection of attestations and sense relations, but it cannot
  make a ToS sign or philosophical sense true.
- [CRMinf 1.2.1](https://cidoc-crm.org/crminf/ModelVersion/crminf-1.2.1),
  released April 2026 as the current stable extension, models argumentation,
  evidence, belief adoption, provenance assessment, and meaning
  comprehension. It is useful as an interoperability projection for
  evidence-bearing arguments. ToS claim packets remain the owner record.
- The current official
  [INCEpTION documentation](https://inception-project.github.io/documentation/)
  identifies 41.2 as the latest release. Its curation, agreement,
  recommenders, and active-learning suggestions are useful experimental
  surfaces, but accepting a suggestion remains an annotation action rather
  than semantic proof.
- [CATMA 7](https://catma.de/documentation/technology-and-versions/) remains
  the current qualitative-literary counterpoint. It attaches typed key-value
  annotations to passages, exports stand-off TEI, and stores CATMA 7 projects
  in one Git repository with per-user branches and an integrated main state.
  These are valuable interchange and inspection properties, not canon
  ownership.

### II. Established annotation research

- [Berzak et al. 2016](https://aclanthology.org/D16-1239/) show that reviewing
  parser or tagger output introduces anchoring and can inflate apparent
  agreement. A human judgment intended as an independent baseline must
  therefore be frozen before model proposals are shown.
- [Plank 2022](https://aclanthology.org/2022.emnlp-main.731/) argues that
  human label variation may express subjectivity, ambiguity, or multiple
  plausible answers rather than noise. A semantic packet needs
  `ambiguous`, `defer`, alternatives, and counterreadings instead of forcing
  consensus.
- [Fleisig et al. 2024](https://aclanthology.org/2024.naacl-long.126/)
  extend this perspectivist warning: disagreement is evidence only when the
  actual annotator perspectives and conditions are preserved. Three model
  outputs are model variation, not three human perspectives.
- [Bernhard et al. 2025](https://aclanthology.org/2025.law-1.14/) show that
  pre-annotation can improve throughput and that correction effort changes
  with perceived pre-annotation quality. This supports AI assistance after
  the baseline, while strengthening the requirement to measure anchoring and
  corrections rather than counting accepted suggestions as quality.

### III. Freshest peer-reviewed evidence that changes this method

- [Gu et al. 2026](https://aclanthology.org/2026.findings-acl.4/) evaluate a
  complete expert workflow in AI-only, AI-assisted, and human-only modes.
  Experts adopted AI-extracted arguments about 60 percent of the time and
  worked about 25 percent faster, but the models remained poor independent
  annotators, strongly hallucinated missing variables, and required
  task-specific codebooks. The paper also explicitly treats over-reliance on
  suggestions as automation bias. For ToS, assistance may earn a speed claim
  only after a hidden unassisted baseline and source-visible error review.
- [Ni et al. 2026](https://aclanthology.org/2026.eacl-long.3/) test 60
  configurations over three disagreement tasks and find that RLVR-style
  reasoning degrades human-disagreement modeling while naive chain-of-thought
  helps some RLHF models. Reasoning labels do not license replacement of
  human disagreement evidence.
- [Hong et al. 2026](https://aclanthology.org/2026.findings-acl.1342/) find
  cases where labels disagree although explanations are semantically similar.
  ToS must preserve bounded rationale separately from the selected label and
  compare both.
- [Hossain et al. 2026](https://aclanthology.org/2026.law-main.18/) show in
  a restricted-access human-in-the-loop audit that declared uncertainty
  strongly identifies difficult cases. The paper uses expert adjudication;
  solo+AI ToS must not imitate that authority. It may instead use uncertainty
  as a trigger for rare competent review and leave the case unresolved when
  such review is unavailable.

### Method decision from the refresh

The historical universal requirement to complete 30 German source rows and
15 double-checked gold rows before any semantic task was a useful fail-closed
prototype, but it converts packet cardinality into unrelated human work. It
is retained as provenance and removed from current scheduling and execution
authority.

The current semantic evaluation method is task-specific:

1. materialize no task until its exact source anchors, accepted source digest,
   source-review event, local content reference, and content digest exist;
2. keep occurrence/context observations, morphology/lemma analyses, and
   interpretive sign/readings as different epistemic layers;
3. require language-competence evidence for execution of morphology/lemma
   tasks; machine agreement cannot fill a declared competence gap;
4. freeze an unassisted operator baseline before model suggestions only for
   the materialized sign and competing-reading tasks;
5. compare labels, rationales, alternatives, uncertainty, refusal, and
   source-return behavior across the same frozen A/B/C tasks;
6. use a second human or external expert only when triggered by a competence
   gap, high-impact canon promotion, persistent source-grounded ambiguity, or
   operator-baseline instability;
7. if the required competent review does not exist, preserve `ambiguous`,
   `defer`, or unresolved rather than infer acceptance;
8. treat the real-human sign decision in a semantic-ladder packet as a
   promotion checkpoint for that sign, not a demand to review every prepared
   occurrence, concordance row, or model proposal;
9. keep all model outputs, INCEpTION/CATMA exports, indexes, and graph
   projections non-authoritative until a separate promotion decision.

The current packet therefore freezes only a content-free 20-task shape:
five tasks in each of four families, balanced 10 random and 10 hard. It
materializes zero tasks, schedules zero human work, runs no model or
annotation tool, and creates no semantic result.

---

## Local source-ground inspection

The three operator-provided files were inspected read-only. A future tracked
forensic record must preserve the same facts with acquisition receipts.

### 2007 Antonovsky volume

Source path basename: `Так говорил Заратустра П Антоновского.pdf`

- 162,778,307 bytes; SHA-256
  `49614015865f8c65a68746b1a6dc9a8b7f036d817b2e04b1f3b94fec8ef7b0c2`.
- PDF 1.7, 432 A4 pages, not encrypted, not tagged.
- CorelDRAW/Corel PDF metadata; visible title page identifies volume 4 of the
  thirteen-volume complete works, *Thus Spoke Zarathustra*, translated by
  Yu. M. Antonovsky, Cultural Revolution, Moscow, 2007.
- No fonts and no usable text layer; `pdftotext` yields only one form-feed byte
  per page. The visible letters are vector outlines.
- This is a structure/rendering/OCR case, not a native-PDF extraction case.

### 1996 collected works, volume 2

Source path basename: `Ницше собрание сочинений.pdf`

- 24,126,572 bytes; SHA-256
  `ce16b68089dee0fc53a31d9e97723991b292dd8835c43bbbb40a9373c9a436aa`.
- `file` incorrectly reports four pages; PDF-aware inspection reports 831
  pages. This is a concrete warning against shallow format validators.
- ABBYY FineReader 10/iTextSharp provenance, 835 image objects and a hidden
  OCR text layer; approximately 3,573,309 extracted bytes.
- Visible metadata identifies *Works in Two Volumes*, volume 2, Mysl,
  Moscow, 1996, with editor K. A. Svasyan and several translators including
  Yu. M. Antonovsky.
- The OCR contains spacing and character confusions and must be compared to
  page images before it can anchor claims.

### 1893 German EPUB derivative

Source path basename: `Nietzsche_Also_sprach_Zarathustra_1893.epub`

- 2,017,690 bytes; SHA-256
  `adc7eeae2d5a5cf2b225ef81170b595c77664167a539f0cb1a478184aca8e9de`.
- ZIP integrity passes; EPUB 3 container with 541 entries, 529 page XHTML
  files and five images.
- Generated by Internet Archive `hocr-to-epub`; the package explicitly warns
  of automated OCR, reading-order, structure, and character errors.
- OPF metadata has a UUID and accessibility warning but lacks usable title,
  author, language, edition, publication, and source identifiers; navigation
  is empty.
- Page text identifies a Google scan from Cornell University and contains
  obvious OCR residue. The `1893` filename is a lead, not verified edition
  metadata.
- This is a born-derived OCR/structure/provenance-recovery case, not a clean
  digital original.

Together these items deliberately exercise three different failure surfaces:
vector-outline PDF, image-plus-OCR PDF, and automatically segmented OCR EPUB.

---

## Material discovery and rights research

### Discovery sources

- [Nietzsche Source eKGWB](https://doc.nietzschesource.org/en/ekgwb) provides
  critically edited Colli/Montinari text with stable addresses. Its
  [CC BY-NC-ND 4.0 posture](https://doc.nietzschesource.org/en/rights) allows
  consultation and citation but prevents treating it as a freely modifiable
  derivative corpus without permission.
- [Nietzsche-Wörterbuch](https://www.degruyterbrill.com/serial/nietzwtb-b/html)
  records occurrences, senses, contexts, history, and reception. Access is
  restricted through Nietzsche Online/De Gruyter and should become a concrete
  written access-request case.
- [DNB SRU](https://www.dnb.de/EN/sru) is a free bibliographic/authority API;
  [DNB SPARQL beta](https://wiki.dnb.de/spaces/LINKEDDATASERVICE/pages/449878933/DNB%2BSPARQL%2BService%2BBETA)
  adds a current linked-data route.
- [KVK](https://www.bibliothek.kit.edu/english/kvk-help.php) is a meta-search,
  not the source catalog. ToS must preserve the originating catalog record.
- [WorldCat Search API 2.0](https://www.oclc.org/developer/api/oclc-apis/worldcat-search-api.en.html)
  requires an eligible subscription and should be optional.
- [Europeana licensing framework](https://pro.europeana.eu/page/europeana-licensing-framework)
  separates object-content rights from metadata rights.
- [Google Books API](https://developers.google.com/books/docs/v1/reference/volumes)
  exposes country-dependent viewability and public-domain flags; these are
  discovery evidence, not a global rights judgment.
- [Internet Archive metadata](https://doc-tools.readthedocs.io/en/ia-test-gsod/metadata.html)
  is useful catalog evidence but does not grant rights by itself.
- [HathiTrust copyright review](https://help.hathitrust.universityofcalifornia.edu/support/solutions/articles/9000207726--1-introduction)
  combines algorithms and human review, illustrating why publication date
  alone is insufficient.
- [Project Gutenberg terms](https://www.gutenberg.org/policy/terms_of_use.html)
  warn non-US users to determine local legal status and direct automation to
  mirrors/offline catalogs.
- [MediaWiki Action API](https://www.mediawiki.org/wiki/API%3AAction_API/en)
  and [German Wikisource dumps](https://dumps.wikimedia.org/dewikisource/latest/)
  offer reproducible discovery and acquisition routes when item licensing is
  compatible.

### Required discovery chain

```text
search lead
  -> originating bibliographic or authority record
    -> digital-object record
      -> edition and item reconciliation
        -> jurisdiction-aware rights determination
          -> access route or written request
            -> immutable acquisition receipt
```

No system may infer `public domain` only from a date, a repository badge, a
download link, or a filename. Unknown and conflicting rights are valid states.

### Exact standalone Nietzsche source-route refresh — 2026-07-30

The transfer-work source search was repeated in the same ordered style rather
than treating a recent corpus hit or a convenient download button as truth.

1. **Classical and official identity layer.** Current
   [DNB/GND](https://d-nb.info/gnd/4226521-6) work authority was reconciled
   with the originating
   [MDZ/B3Kat *Der Fall Wagner* object](https://www.digitale-sammlungen.de/en/details/bsb11827837),
   while [DNB/GND](https://d-nb.info/gnd/4501140-0) was reconciled with the
   originating
   [MDZ/B3Kat *Götzen-Dämmerung* object](https://www.digitale-sammlungen.de/en/details/bsb00069119).
   IIIF manifests, MARC, RDF, URNs, holding institutions, call numbers, and
   visible title pages agree on the standalone Naumann 1888 and 1889 item
   identities.
2. **Established scholarly layer.** Andreas Urs Sommer's official HAdW
   historical-critical commentary,
   [Band 6/1](https://doi.org/10.11588/diglit.70913), changes the
   interpretation of apparently simple dates. The 1888 *Fall Wagner* run
   comprised 1000 copies whose latter half was nominally labelled
   `Zweite Auflage`; the exact Bamberg title page is unlabelled. The
   1889-dated *Götzen-Dämmerung* reached booksellers at the end of January
   1889, while Köselitz's textually changed second edition followed in 1893.
3. **Fresh current-service layer.** The live DTA Nietzsche author corpus
   contains neither target. Current TextGrid records are useful Kolimo+
   corpus-text challengers, but their 2026 repository issue does not create
   historical-copy or critical-text identity. Google Books supplied useful
   bibliographic leads, yet its tested download responses were HTML challenge
   or not-found pages; no source bytes were admitted and no bypass was used.
   The official MDZ immediate-PDF service supplied the selected objects after
   their current rights notices were acknowledged.

The acquired PDF for `bsb11827837` has SHA-256
`37fc9eb2d26886be936efe06c7fbeaf9f6dab231b3a81bc2cb5e824e98f984ed`,
75 PDF pages, and 74 ordered source canvases after one provider cover. The
acquired PDF for `bsb00069119` has SHA-256
`f41a1dee091edd895d1f18a510dc73b48949257e882d3390c6c8f72beeb8d086`,
165 PDF pages, and 164 source canvases after one provider cover. Neither
contains accepted book OCR.

Rights remain object- and layer-specific. MDZ reports
[`NoC-NC 1.0`](https://rightsstatements.org/vocab/NoC-NC/1.0/) for the
*Fall Wagner* object and
[`CC BY-NC-SA 4.0`](https://creativecommons.org/licenses/by-nc-sa/4.0/)
for the *Götzen-Dämmerung* object. ToS preserves those positive statements,
keeps both operator-held PDFs local, and authorizes only public-safe metadata
until current conditions, attribution, jurisdiction, and explicit operator
review support a separate public route.

### *Ecce Homo* foundation-witness refresh — 2026-07-30

The last *Mysl* member without its own exact historical German tree was
researched in the same order, with a stricter editorial-strata question.

1. **Classical and official layer.** The current
   [DNB/GND work authority](https://d-nb.info/gnd/4115395-9) separates
   production beginning in 1888 from publication in 1908. The exact
   [DNB 1908 bibliographic record](https://d-nb.info/999650270) identifies
   Leipzig, Insel-Verlag, Nietzsche, Raoul Richter's afterword, Henry van de
   Velde, Friedrich Richter, and a catalog extent of 155 pages. DTA has no
   Nietzsche *Ecce Homo* object; its exact-title hit is an unrelated 1650
   sermon. TextGrid's current Kolimo+ TEI is a useful modern corpus challenger,
   not a historical copy or critical edition.
2. **Established scholarly layer.** Andreas Urs Sommer's
   [HAdW commentary, Band 6/2](https://doi.org/10.11588/diglit.70914),
   shows that the 1908 first edition substantially follows the Köselitz copy,
   that material had been removed or destroyed in the family editorial
   history, and that the later Colli/Montinari reconstruction is
   philologically stronger. It also corroborates Richter's editing, van de
   Velde's design, and the 1,250-copy issue.
3. **Fresh current layer.** Groddeck's
   [2025 study](https://doi.org/10.1515/nietzstu-2024-0035) treats the book as
   completed yet transmitted through a complex editorial object and makes
   paratextual composition relevant to later semantics. Röllin's
   [2025 study](https://doi.org/10.1515/nietzstu-2025-0024) adds current
   evidence from a Köselitz copy and revises the chronology of one important
   section toward early January 1889. Parkhurst's restricted
   [self-quotation chapter](https://doi.org/10.1515/9783110688436-006)
   remains an abstract-only semantic lead, but the current bibliographic
   record places it in a 2022 book. A 2026 copyright/download stamp on the
   current publisher surface was not a fresh publication date. No restricted
   full text was bypassed.
4. **Exact digital object.** Getty's physical copy was digitized into
   Internet Archive item
   [`eccehomo00niet_0`](https://archive.org/details/eccehomo00niet_0) and
   preserved as permanent
   [Commons revision 1074063624](https://commons.wikimedia.org/w/index.php?oldid=1074063624).
   The acquired 9,703,400-byte PDF has SHA-256
   `f3058ff02611cc961de1f7c4fbf0ebc0e4427a913da718189b33a0dde11339bc`,
   166 PDF pages, and 498 image/mask resources. The current Internet Archive
   PDF differs bytewise and is retained as lineage rather than silently
   substituted.

The visual item makes the semantic boundary concrete. Nietzsche's body ends
at printed page 130; a separate editor-title leaf, a blank leaf, and Richter's
afterword follow from printed page 133 through 154. Any future transcription,
lexical index, graph, quotation analysis, or semantic annotation must retain
that change of responsibility. The DNB extent `155 S.` and the
IA/Open Library `154 p.` claim also remain distinct.

Commons applies `PD-US-expired`, not a jurisdiction-complete global license.
Richter's editorial contribution, van de Velde's binding and ornaments, the
Getty scan, the IA derivative, Commons revision, OCR, and later derivatives
remain separate rights layers. The exact local PDF is research soil only; its
server plan exposes metadata and transfers no payload.

### *Ecce Homo* authorial-witness follow-up — 2026-07-30

The stronger route required by the 1908-witness limitation was then resolved
without downloading a manuscript corpus or creating a false locally held
item. The complete evidence and conflict register lives in
`ECCE_HOMO_AUTHORIAL_WITNESS_ROUTE.md`.

1. **Official archive and edition layer.** Current GSA ORES resolves
   Nietzsche's print-ready manuscript as
   [GSA 71/32, D 25](https://ores.klassik-stiftung.de/ords/f?p=401:2:::::P2_ID:75105)
   and the Köselitz/Peter Gast print-ready copy as
   [GSA 71/33, D 25a](https://ores.klassik-stiftung.de/ords/f?p=401:2:::::P2_ID:75106).
   It also exposes the surviving
   [replacement-section copy](https://ores.klassik-stiftung.de/ords/f?p=401:2:::::P2_ID:1730),
   an undigitized
   [three-leaf deletions/change file](https://ores.klassik-stiftung.de/ords/f?p=401:2:::::P2_ID:1731),
   and the Z II 1, W II 9, W II 10, and Mp XVI draft complexes. DFGA provides
   matching digital-book identities and codicological metadata. eKGWB provides
   74 stable critical-text blocks and two correction popups; three current
   HTTP responses were byte-identical, but no body was retained or admitted.
2. **Established layer.** HAdW commentary connects those archive records to
   the production and posthumous edition history and explicitly maps D 25 to
   current GSA 71/32.
3. **Current layer.** Röllin's
   [2024 manuscript chronology](https://doi.org/10.1515/nietzstu-2024-0001)
   makes inscription-layer uncertainty explicit and distinguishes early
   W II 9 composition, successive print manuscripts, proof correction, and
   late-December/early-January additions. The two 2025 studies then sharpen
   paratextual and replacement-section questions.
4. **Typed responsibility layer.** Four unreviewed bibliographic claims now
   keep Nietzsche's `authored_by` relation on the Work separate from Raoul
   Richter's `edited_by` and `afterword_by` relations and Henry van de Velde's
   `designed_by` relation on the exact 1908 Edition. All three Agents resolve
   to current GND identities; the claims close over their actual subjects and
   are digest-bound to the recorded discovery evidence. This makes the
   responsibility boundary queryable without converting it into generic
   creator metadata, an author-finality claim, editorial collation, rights
   clearance, or accepted bibliography.

Two exact identifier conflicts are preserved. Groddeck's fresh article figure
captions use `GSA 71/31` for D 25, while current ORES, Kalliope, and HAdW agree
on `71/32`; current
[GSA 71/31](https://ores.klassik-stiftung.de/ords/f?p=401:2:::::P2_ID:75104)
is D 24, *Dionysos-Dithyramben*. ORES metadata for GSA 71/232 names
`Mp-XVI-2a`, its outgoing link names `Mp-XVI-2b`, and the current DFGA API
exposes both as distinct books.

The GSA IIIF manifests declare their exact digital objects `Public Domain`,
while the general GSA regulations require prior approval for reproduction use
beyond personal use. Nietzsche Source separately states CC BY-NC-ND 4.0 and
describes a DFGA non-commercial-derivative agreement. ToS preserves these as
layered, currently unreconciled evidence. The route creates no source payload,
accepted German, publication permission, semantic record, access request, or
human backlog.

### *Jenseits von Gut und Böse* authorial-witness follow-up — 2026-07-30

The first transfer work now has an exact upstream route recorded in
`JENSEITS_AUTHORIAL_WITNESS_ROUTE.md`.

1. **Official archive and correction layer.** Current GSA ORES and Kalliope
   resolve D 18 as `GSA 71/26`, 108 leaves, with 232 ORES canvases and nine
   archival child groups. W I 3-W I 8 are explicitly only partly used in the
   work. HAAB resolves the complete Nietzsche correction copy C 4619 and the
   partial proof C 4615 as distinct objects. Their open viewers do not supply
   usable publication permission.
2. **Digital and independent print layer.** A bounded DFGA observation
   reported 116 folio sheets, 224 image records, and 240 logical identifiers;
   a later same-day refresh timed out. Two bounded eKGWB observations exposed
   302 stable critical addresses without retaining the body. ZB Zürich
   e-rara supplies an independent official-library 1886 first-print route
   with Public Domain Mark evidence; no duplicate was acquired.
3. **Established layer.** Röllin's 2013 genetic study and Sommer's 2016 HAdW
   commentary establish D 18 as a late, physically reworked stage, preserve
   the recoverable 308-aphorism arrangement, and distinguish later C 4619
   corrections and Köselitz's 1894 numbering intervention.
4. **Fresh layer.** Röllin's 2024 chronology remains the current dating
   control. Pichler's 2025 §22 study supplies the exact W I 7 -> D 18 ->
   publication comparison route and demonstrates why semantic change must be
   asserted between versions rather than collapsed into one graph node.

The freshness search through 2026-07-30 found no 2026 philological
publication changing D 18 identity. This dated negative result, the current
Nietzsche Source timeout, and all rights ambiguities remain explicit. The
route creates no manuscript or correction payload, accepted German,
author-final synthesis, semantic claim, duplicate Item, access request, or
human backlog.

### *Zur Genealogie der Moral* authorial-witness follow-up — 2026-07-30

The second work in the frozen transfer frame now has an exact upstream route
recorded in `GENEALOGIE_AUTHORIAL_WITNESS_ROUTE.md`.

1. **Official archive layer.** Current GSA ORES and Kalliope split D 20 into
   `GSA 71/27,1` / DFGA `D-20a` for title, preface, and Treatises I-II, and
   `GSA 71/27,2` / `D-20b` for Treatise III and contents. W II 1
   (`GSA 71/157`) remains a mixed 1887-1888 notebook from which only exact
   source-backed regions may be attributed.
2. **Correction and documentary-edition layer.** HAAB C 4616 preserves only
   sheets 1 and 10 plus one handwritten leaf; its IIIF placeholder conflicts
   with the METS `InC 1.0` / all-rights-reserved statement. The University of
   Basel edition, first freely viewable in December 2025, co-edits D 20, K 11,
   and E 40 as distinct objects and exposes source-surface-to-section
   navigation without supplying an explicit reusable-content license.
3. **Established layer.** Sommer's 2019 HAdW commentary controls print history,
   responsibility, C 4616, and the unresolved C 4620/C 4621/unnamed hand-copy
   layer. Parkhurst's 2022 preface study demonstrates a bounded genetic
   comparison without overriding official object identities.
4. **Fresh layer.** The newly published 3 August 1887 Nietzsche letter in
   Jung/Carbone 2024 strengthens responsibility and chronology; Röllin 2024
   controls mixed manuscript dating; the Basel December 2025 edition is the
   freshest direct philological result. Current 2025-2026 philosophical-method
   work was classified separately because it does not change D 20/K 11/E 40
   identity.

The current Nietzsche Source include request timed out after 30 seconds. The
stable DFGA/eKGWB routes remain registered, but no response body, critical
address census, XML, manuscript image, OCR, or accepted reading was retained.
The existing 1892 second-edition Item remains a later historical witness, not
E 40, a critical text, or a transfer unit. The route creates no semantic
claim, translation judgment, publication permission, access request, or human
backlog.

### *Der Antichrist* authorial-witness follow-up — 2026-07-30

The third work in the frozen transfer frame now has a responsibility-aware
route recorded in `ANTICHRIST_AUTHORIAL_WITNESS_ROUTE.md`.

1. **Official archive layer.** Current GSA ORES and Kalliope identify D 22
   (`GSA 71/29`) as the 51-leaf print manuscript and preserve its nontrivial
   leaf order, original title leaf, and an earlier *Ecce homo* /
   *Der Fall Wagner* state on leaf 51v. W II 1-W II 6 are mixed
   revaluation-period notebooks: only exact source-backed regions, including
   later W II 3/W II 4 preliminary states, may be connected to the work.
   GSA 71/32 fol. 47 is a physically separate, two-sided leaf in the D 25
   container whose proposed *Antichrist* association is strong but contested;
   neither container nor later editorial placement makes it `AC 63`.
2. **Independent-copy and first-print layer.** University Library Basel
   `NL 53 : A 311` is Overbeck's early-1889, IV + 113-page copy. It is
   independent comparison evidence, not Nietzsche's autograph or an
   author-approved print, and it omits the associated `Gesetz` leaf. The
   Koegel/Naumann 1895 first print restored an older subtitle, silently
   incorporated Köselitz corrections, and suppressed or altered four
   passages. The exact Cornell/Google Books lead is no-view and remains
   deferred; no unprovenanced apparent 1895 scan was accepted.
3. **Established layer.** Sommer's 2013 HAdW commentary controls the two D 22
   title states, correction responsibility, A 311, the 1895 interventions,
   their propagation through archive editions, Schlechta's 1956
   de-suppression, Colli/Montinari KGW VI 3 (1969), and the material
   complication of the associated leaf. Basel KGW IX method requires spatial
   topology, deletions, overwriting, and revision to survive instead of being
   smoothed into detached fragments.
4. **Fresh layer.** Brobjer's chapter, first online 2025-11-04, supplies the
   freshest located direct genesis hypothesis and motivates exact
   note-region-to-section comparison without proving whole-notebook work
   membership. Di Sarno 2025 supplies a method counterweight against hidden
   intention or detached Nachlass becoming semantic authority. Zittel's
   in-press 2026 intertextual work is retained as a future semantic lead; it
   changes no witness identity.

The freshness search through 2026-07-30 found no 2026 philological result
changing D 22, A 311, the 1895 intervention history, or GSA 71/32 fol. 47.
Four bounded Nietzsche Source requests timed out after 30 seconds and retained
no body. The existing 1906 Item remains a later archive-edition witness, not
proof that 1895 interventions were repaired. No manuscript image, source
text, critical body, accepted German, author-final synthesis, translation
alignment, semantic claim, transfer unit, publication permission, access
request, or human backlog was created.

### *Der Fall Wagner* authorial-witness follow-up — 2026-07-30

The exact unlabelled 1888 Naumann/Bamberg Item now has a documentary route
recorded in `FALL_WAGNER_AUTHORIAL_WITNESS_ROUTE.md`.

1. **Official archive layer.** Current GSA ORES identifies W II 6
   (`GSA 71/162`, ORES `75271`) and W II 7 (`GSA 71/163`, ORES `75272`)
   as mixed notebooks naming several projects and other hands. Only exact
   source-backed *Fall Wagner* regions and inscription layers are admitted.
   The official `D 21 / GSA 71/28 / ORES 75099` record is
   *Götzen-Dämmerung* and is explicitly rejected as a *Fall Wagner*
   manuscript identity.
2. **Surviving-fragment layer.** University Library Basel/e-manuscripta
   resolves `NL 200 : II, 1`, DOI `10.7891/e-manuscripta-80734`, as a
   one-leaf, two-canvas mid-August 1888 object and the only surviving
   manuscript fragment. Nietzsche's two changes and the later
   Köselitz/Lauterbach notes are separate responsibility and provenance
   layers; the leaf is not a complete print manuscript.
3. **Established production layer.** Sommer's 2012 HAdW commentary records a
   first full print manuscript sent on 26 June and returned on 6 July, a new
   fair copy sent on 16 July, later Nachschriften/Epilog/proof additions,
   completion in September, and publication on 22 September. Both complete
   manuscript states are lost. The 1000-copy run's latter half was nominally
   labelled `Zweite Auflage`; the existing Item is from the unlabelled half,
   and no textual identity or difference is inferred.
4. **Topological, critical, and fresh layer.** KGW IX 9 preserves W II 6/W II
   7 topology. Röllin 2024 supplies the freshest direct layer chronology.
   Current Basel records reconfirm the sole fragment in 2025. Nietzsche Source
   documentation and established citation support the `WA` address family,
   but current requests timed out and no body or complete address census was
   retained. Current 2025 TEI and interpretive leads plus a directly titled
   2026 chapter remain downstream challengers; none changes witness identity.
5. **Typed publication-claim layer.** The Edition now owns six sibling
   `publication-claims.jsonl` packets. They separate the observed title-page
   year from reported author-supervised publication role, month-precision
   printing completion, official publication date, print-run extent, and the
   nominal later issue state. The last claim says neither “same text” nor
   “different text”: both statuses remain `unresolved`, the current Item is
   explicitly unlabelled, and no second Item exists. A digest-bearing
   annotation event closes provenance. This contrasting second prototype
   proves that the existing claim contract can preserve issue uncertainty; it
   does not create accepted bibliography, collation, or a bulk-population
   mandate.

The freshness search through 2026-07-30 found no newer direct philological
result changing W II 6/W II 7, the two lost complete manuscript states,
`NL 200 : II, 1`, or the 1888 production split. No remote image, manuscript
text, critical body, new Item, accepted German, reconstructed source,
translation relation, semantic claim, transfer unit, publication permission,
access request, or human backlog was created.

### *Götzen-Dämmerung* authorial-witness follow-up — 2026-07-30

The exact unlabelled 1889-dated Naumann/BSB Item now has a documentary route
recorded in `GOETZEN_DAEMMERUNG_AUTHORIAL_WITNESS_ROUTE.md`.

1. **Official archive layer.** Current GSA ORES/Kalliope positively identifies
   `D 21 / GSA 71/28 / ORES 75099` as Nietzsche's autograph print manuscript
   for E 42, currently described as 62 leaves and 124 IIIF canvases. W II 6
   through W II 9 are named physical preparatory soil, but they contain
   several projects and other hands. Earlier W II 1/W II 3/W II 5 material
   and all work membership require exact HAdW/KGW/Röllin-backed regions and
   inscription layers; no whole notebook is promoted.
2. **Established production layer.** Sommer's 2012 HAdW commentary records
   the 7 September clean manuscript, the 18 September addition, Köselitz's
   title suggestion and Nietzsche's adoption, proof-stage addition of
   *Streifzüge* §§32-44, late migration of *Was ich den Alten verdanke* from
   W II 9c, printing completed 13 November, authorial copies received
   24 November, the 1889 title-page date, and delayed late-January 1889 sale.
   These dates, changes, and responsibility layers remain distinct.
3. **Edition layer.** The existing MDZ/BSB Item remains the exact unlabelled
   first-publication witness. The Köselitz-edited 1893 second edition added
   headings and textual changes without Nietzsche's approval and is a
   separate, unacquired state. KSA/KGW, Montinari, and Pichler control reading,
   edition law, manuscript topology, and textual form without supplying a
   complete genetic dossier.
4. **Fresh and critical layer.** Röllin 2024 supplies the freshest direct
   inscription-layer chronology; D'Iorio 2024/2025 supplies current Nietzsche
   Source method. The *Magnum in parvo* 2024 reconstruction and its 2025
   review remain an editorial challenger, not a Nietzsche-authored completed
   Work or author-final text. The seven-article 2025
   *Nietzscheforschung* cluster remains downstream interpretation. Stable
   `GD` address examples are documented, but the bounded current Nietzsche
   Source request timed out with status `000` and no body.
5. **Typed publication-claim layer.** The existing Edition contract now owns
   six sibling `publication-claims.jsonl` packets. They preserve the observed
   title-page year, four reported publication-history assertions, and the
   distinct changed 1893 editorial state as separately evidenced, model-made,
   unreviewed claims. A digest-bearing annotation event closes provenance.
   This does not type remote notebooks or manuscripts as local objects, and
   it does not create an accepted bibliography or canonical graph.

The freshness search through 2026-07-30 found no newer direct genetic or
philological result changing D 21, the region-only notebook law, the
production chronology, or the separate 1893 editorial state. General web was
used last; Gutenberg-derived text and unprovenanced reprints or summaries
were rejected, while TextGrid remains a future challenger only. No remote
image, manuscript text, critical body, new Item, accepted German, synthetic
author-final reconstruction, translation relation, semantic claim, transfer
unit, publication permission, access request, or human backlog was created.

---

## Foundation model proposed for contract work

### Corpus identity classes

| Class | Purpose |
| --- | --- |
| `work` | intellectual work independent of one language or carrier |
| `expression` | language/textual state, including original or translation |
| `edition` | edited/published manifestation with editor, publisher, date, and apparatus |
| `item` | one physical or digital copy/container |
| `file` | immutable acquired byte sequence with digest and media type |
| `source-event` | discovery, access decision, acquisition, verification, or preservation event |
| `passage` | citable structural node in a declared witness version |
| `region` | visual page/canvas region supporting or recovering a passage |

### Text-bearing layers

| Layer | Mutation rule | Authority posture |
| --- | --- | --- |
| source bytes | immutable | forensic item evidence |
| rendered page | reproducible derivative | visual source-return surface |
| diplomatic transcription/OCR | append new revision | source-near proposal until reviewed |
| normalized text | append new revision; NFC allowed | declared editorial derivative |
| structural segmentation | versioned | address proposal, reviewable |
| translation | versioned by translator/method | independent expression/proposal |
| annotation | append/supersede | observation or interpretation by typed layer |
| claim | append/supersede/reject | evidence-bearing assertion |
| search/graph export | replaceable rebuild | non-authoritative projection |

### Sign and semantic layers

| Layer | Example | Review posture |
| --- | --- | --- |
| glyph/token occurrence | visible `Übermensch` at an anchor | source-near observation |
| surface form | exact spelling/case/punctuation | source-near observation |
| lemma/morphology | `Übermensch`, noun, singular | linguistic proposal/review |
| recurrence set | occurrences grouped by declared rule | reproducible analysis plus review |
| lexical sense | one contextual sense among alternatives | interpretive claim |
| etymology | component/history/source-language account | sourced scholarly claim |
| translation relation | source segment rendered by target segment | alignment plus translation claim |
| motif/sign | recurring image or sign across passages | interpretive proposal |
| concept | abstracted philosophical object | reviewed, contestable claim family |
| relation | supports, contrasts, transforms, inherits, echoes | typed claim with evidence |

Stable IDs apply at every layer. Stability of ID means continuity of the
record, not infallibility of its content.

### Claim packet minimum

Every semantic or relational assertion should contain:

- assertion ID and type;
- subject, predicate, object or literal body;
- source witness and anchor set;
- exact supporting quote or visual region reference;
- assertion layer: observation, linguistic analysis, bibliographic claim,
  translation judgment, interpretation, or canon judgment;
- maker: human, model, software, imported source, or mixed workflow;
- method, version, prompt/configuration, and timestamp where applicable;
- confidence as the maker's declared uncertainty, never truth probability;
- review state, reviewer, rationale, alternatives, and counterevidence;
- supersedes/superseded-by lineage;
- rights and visibility constraints inherited from supporting material.

---

## Translation as an independent research branch

AI, human, and AI+human translation can become one of ToS's highest-value
organs only if their differences remain visible.

### Required blind stages

1. establish and verify the original-language segment;
2. prepare morphology, literal gloss, etymological leads, and ambiguity notes
   without exposing a recognized translation to the first translator;
3. create an independent human, AI, or AI-assisted draft;
4. reveal one or more recognized translations and compare them by aligned
   segment;
5. perform an independent revision that did not author the draft;
6. adjudicate disagreements and record accepted, rejected, and unresolved
   alternatives;
7. publish a packet, not a single cleaned sentence.

### Evaluation axes

- source fidelity and omission/addition;
- semantic force and logical relation;
- key-term consistency without mechanical flattening;
- etymological and morphological sensitivity;
- ambiguity preserved, narrowed, or invented;
- voice, register, rhythm, imagery, syntax, and punctuation;
- intratextual recurrence and cross-passage resonance;
- explicitation, domestication, interpretation, and other translator
  interventions;
- fluency separately from faithfulness;
- reviewer confidence and unresolved risk.

Automatic metrics may be recorded as diagnostics. They cannot promote a
translation or settle an etymology.

---

## Experimental implications

### A/B/C principle

Every experiment uses the same frozen source sample and evaluation packet.
`A`, `B`, and `C` are independently reproducible routes, not three prompts to
one hidden service. Order is randomized or blinded when output style could
reveal the method to a reviewer.

### Minimum gold set

The first gold set should be deliberately small and difficult:

- pages with native OCR, no OCR, and corrupt OCR;
- headings, prose, verse-like layout, quotation, italics, line numbers,
  hyphenation, and page furniture;
- German/Russian segment pairs containing recurrence, ambiguity, compound
  words, metaphor, syntactic inversion, and recognized translation tension;
- lexical queries and implicit semantic queries;
- positive, negative, ambiguous, and counter-reading semantic examples;
- relations that should be accepted, rejected, split, or left unresolved.

### Manual review record

For each item, reviewers see the rendered source region and all necessary
context. They record error spans, severity, rationale, and whether the failure
blocks downstream use. A second manual pass checks the first review on a
sample. The validator only verifies that this evidence exists and is
well-formed.

---

## Decisions supported now

1. Build the corpus catalog and witness spine before mass semantic markup.
2. Store source files locally under a narrow gitignored item route; track
   catalogs, manifests, checksums, forensic reports, rights, and receipts.
3. Treat all extracted text as a versioned derivative, even when a PDF already
   contains OCR.
4. Use structural + quote + digest + visual anchors; never offsets alone.
5. Keep signs as a layered family from occurrence to interpretation.
6. Keep claims explicit; keep graph and vector stores disposable.
7. Start with resident models and runtimes; earn every download. The bounded
   Granite R2 Retrieval C artifact is now earned by the already-executed
   resident A/B comparison and the frozen independent-challenger requirement.
8. Compare lexical, resident dense+rerank, and independent multilingual dense
   retrieval on one ToS-authored query set; keep later hybrid fusion separate.
9. Run a dedicated blind translation lab with interlinear and recognized
   translation comparison.
10. Preserve rejected and unresolved outputs as training and architectural
    evidence.

## Questions intentionally left for laboratory evidence

- Can the vector-outline 2007 PDF be recovered more faithfully through direct
  rasterization + Tesseract, PaddleOCR, or a layout/VLM route?
- Is its visible typography sufficiently clean that OCR confidence predicts
  real error, or does a manual sample expose systematic semantic corruption?
- Does the 1996 ABBYY layer outperform fresh OCR after alignment to page
  images?
- Can the EPUB page sequence and sections be reconstructed from layout and
  recurring headers without importing false structure?
- Which multilingual embedding best retrieves German/Russian conceptual
  parallels without erasing lexical evidence?
- Do current small local models help with etymological and translation
  proposals, or merely produce persuasive ungrounded prose?
- Which annotation interface supports real human review without making its
  internal database a new hidden authority?
- Does RDF or property-graph projection improve inspection enough to justify
  its operational cost over tracked claim packets plus a simple graph view?

Those are experimental questions, not gaps to fill with confidence.
