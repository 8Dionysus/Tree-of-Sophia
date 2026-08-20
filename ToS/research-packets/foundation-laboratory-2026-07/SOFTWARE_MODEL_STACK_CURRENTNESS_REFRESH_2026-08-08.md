# Software and Model Stack Currentness Refresh — 2026-08-08

Status: ordered currentness research complete; no download, installation,
runtime mutation, model execution, result supersession, or candidate promotion

Checked through: 2026-08-08, America/Mexico_City

Owner boundary: Tree of Sophia owns the source question, evidence meaning,
review criteria, and promotion decision. `abyss-stack` owns future isolated
experiment mechanics and receipts. `abyss-machine` owns live devices,
runtimes, resident artifacts, storage, memory, and thermal evidence.

## Outcome first

The remaining foundation software/model surface is current enough to continue
without rebuilding the laboratory and without acquiring another model.
Several version facts have changed since the 2026-07-23/26 snapshots, but no
change supplies content quality, source authority, or a winner:

- OCRmyPDF moved from 17.8.1 to 17.10.0 and Docling from 2.114.0 to 2.118.1;
- OpenVINO 2026.3 is current upstream while the host still owns 2026.2;
- Stanza 1.14.0 introduces security fixes and an incompatible lemmatizer-model
  boundary that matters if a future route is opened;
- Qdrant 1.19.0 adds useful memory and sparse-search controls but remains an
  optional retrieval projection, not a knowledge graph or authority store;
- Neo4j now publishes 2026.06 under calendar versioning, while the executed
  resident 5.26.26 LTS comparison remains valid evidence at its exact version;
- `llama.cpp` publishes rolling build tags many times per day, so a current tag
  is a watch fact and an exact commit remains the reproducibility identity;
- no newly verified small open-weight model displaces the already resident
  task controls or reopens a failed translation/GEC result.

The next task-specific experiment, when an actual source or quality question
opens it, must resolve and freeze its own binaries, models, model-data files,
configuration, licenses, digests, and device route. This refresh itself opens
no task and creates no human backlog.

## Research law and scope

Evidence was checked in the required order:

1. standards, current official documentation, owner release notes, and model
   cards;
2. established methods and leading evaluation work;
3. the freshest primary papers that materially change ToS method or risk.

Release recency was never used as a quality rank. Marketing benchmarks,
download counts, secondary roundups, and generic leaderboards were excluded
from version authority and from admission decisions.

This pass covers source/container forensics, OCR and document intelligence,
language and translation runtimes, embeddings and retrieval, graph stores,
local inference backends, and small/conditional model candidates. Semantic
annotation standards and tools have their separate 2026-08-08 refresh in
`RESEARCH.md`; exact translation successors and their negative results retain
their separate, task-bounded records.

## I. Current official documentation and releases

### Source and container forensics

| Surface | Current official observation | ToS consequence |
| --- | --- | --- |
| Poppler | [26.08.0](https://poppler.freedesktop.org/), released 2026-08-02; signed tarball; `pdfimages` adds minimum-height/width filters | remains the PDF-aware baseline; a future run records the actual binary version and output digests rather than treating extraction as validation |
| qpdf | [12.3.2](https://github.com/qpdf/qpdf/releases/tag/v12.3.2), released 2026-01-24; signed release and published asset hashes | useful structural/encryption diagnostic; not a renderer, OCR engine, or content validator |
| EPUBCheck | [5.3.0](https://github.com/w3c/epubcheck/releases/tag/v5.3.0), the current production-ready EPUB 3.3 checker | container/specification conformance remains separate from text accuracy, provenance, and rights |
| ExifTool | the official `ver.txt` and history endpoints did not yield a stable current value in this pass | exact current version remains unresolved; any future acquisition must re-read the official endpoint and pin the artifact instead of copying an unverified search value |

The unresolved ExifTool version is negative evidence, not a reason to infer a
number from a mirror. Existing source fixity and format-specific inspection do
not depend on resolving it now.

### OCR and document intelligence

| Surface | Current official observation | Delta and decision |
| --- | --- | --- |
| Tesseract | [5.5.2](https://github.com/tesseract-ocr/tesseract/releases/tag/5.5.2) | unchanged; the executed classical A result stays bound to its exact binary and `traineddata` |
| Kraken | [7.0.2](https://github.com/mittagessen/kraken/releases/tag/7.0.2) | unchanged; the stopped B run and its dependency freeze remain negative evidence, not an upgrade request |
| PaddleOCR | [3.7.0](https://github.com/PaddlePaddle/PaddleOCR/releases/tag/v3.7.0) | unchanged; PP-OCRv6 still does not make a fair Russian recognition route, so the executed v5-language C identity is preserved |
| OCR-D core | [3.13.2](https://github.com/OCR-D/core/releases/tag/v3.13.2), released 2026-07-20 | adds a CUDA+ONNX base image; this does not improve ToS evidence until one exact workflow needs and measures it |
| OCRmyPDF | [17.10.0](https://github.com/ocrmypdf/OCRmyPDF/releases/tag/v17.10.0), released 2026-08-05 | supersedes the 17.8.1 watch fact; watcher hardening separates data/code, rejects symlinks and non-regular input, and prevents recursive output; it remains secondary packaging only |
| Docling | [2.118.1](https://github.com/docling-project/docling/releases/tag/v2.118.1), released 2026-08-07 | supersedes 2.114.0; fixes pictures in table cells, threaded-parser shapes, implicit HTML file requests, and label normalization; no old structure result is silently reinterpreted |

Docling 2.118.0 also added all PP-OCR languages through RapidOCR and
reading-order dehyphenation, while 2.116.0 added a layout-driven OCR pipeline.
Those are future challenger features. They are not reasons to rerun the
already closed structure packet or to substitute generated layout for source
coordinates.

### Language and translation runtimes

| Surface | Current official observation | ToS consequence |
| --- | --- | --- |
| Stanza | [1.14.0](https://github.com/stanfordnlp/stanza/releases/tag/v1.14.0), released 2026-07-15 | fixes zip-slip, restricts document unpickling, removes shell calls, and changes the lemmatizer model format; future use requires exact code/model-data digests and no implicit download |
| CTranslate2 | [4.8.1](https://github.com/OpenNMT/CTranslate2/releases/tag/v4.8.1), released 2026-07-03 | adds Gemma 4 12B support and hardens legacy checkpoint loading while fixing a model-load heap overflow; it remains a conditional runtime, not a new translation candidate |

Stanza 1.13 lemmatizer data is not compatible with 1.14. A green import with
untracked replacement models would therefore be false reproducibility. No
Stanza package or model was acquired during this pass.

### Local inference backends and model families

| Surface | Current official observation | ToS consequence |
| --- | --- | --- |
| OpenVINO | [2026.3](https://docs.openvino.ai/2026/about-openvino/release-notes-openvino.html), released 2026-08-04 | upstream changed; the host remains on 2026.2 and is not upgraded by this research |
| `llama.cpp` | rolling [build b10330](https://github.com/ggml-org/llama.cpp/releases/tag/b10330) was current when checked on 2026-08-08 | daily build recency has no content meaning; future receipts keep the exact resident commit and backend |
| Gemma 4 | official [release history](https://ai.google.dev/gemma/docs/releases) still leaves E2B/E4B as the small resident family used by the first-wave controls | no new acquisition or rerun; community quantizations remain distinct artifacts from official model claims |
| Qwen 3.5/3.6 | the official [Qwen3.6 repository](https://github.com/QwenLM/Qwen3.6) still lists 27B and 35B-A3B releases, while 4B belongs to Qwen3.5 | no new small 3.6 first-wave model; total loaded weights, not active parameters alone, control machine fit |
| Qwen text embedding | [Qwen3-Embedding-0.6B](https://huggingface.co/Qwen/Qwen3-Embedding-0.6B) remains the resident multilingual baseline | no model identity change and no replacement of the completed retrieval evidence |
| Qwen visual embedding | [Qwen3-VL-Embedding-2B](https://huggingface.co/Qwen/Qwen3-VL-Embedding-2B) remains the exact visual challenger family | audit-complete mechanical r9 remains separate from human relevance and from text A/B/C |
| Granite multilingual embedding | [311M Multilingual R2](https://github.com/ibm-granite/granite-embedding-models) remains the selected independent text family | no family change; exact frozen revision and observed ToS output outrank later repository activity |

OpenVINO 2026.3's own known-issues section reports that Qwen3 can fail to load
on NPU and can fail compilation above 8K context without a compiler-mode
workaround. It also reports a Gemma 4 chat-template incompatibility in the
GenAI Minja path. These facts strengthen device- and template-specific
preflight; they do not authorize a host upgrade or a workaround hidden inside
a content run.

The digest-bound `LOCAL_LLM_TRANSLATION_QWEN3_8B_CPU_REFRESH_2026-08-08.md`
describes 2026.2 as current inside its frozen admission episode. That wording
is preserved because the executed episode closes over the file digest. This
later record supplies the superseding upstream-currentness fact without
rewriting the provenance of the run.

The official `llama.cpp` surface published several signed build tags on the
same day. This confirms that “latest `llama.cpp`” is not an adequate artifact
identity. The exact commit, build flags, backend, binary digest, and model
digest remain mandatory.

### Retrieval and graph stores

| Surface | Current official observation | Delta and decision |
| --- | --- | --- |
| Qdrant | [1.19.0](https://github.com/qdrant/qdrant/releases/tag/v1.19.0), released 2026-08-05 | new TurboQuant primary storage, per-component cold/cached/pinned memory, prefix filtering, and per-query sparse IDF are relevant future controls; the release also carries substantial consistency/concurrency repair, so no adoption follows from features alone |
| Oxigraph / PyOxigraph | [0.5.9](https://github.com/oxigraph/oxigraph/releases/tag/v0.5.9), released 2026-06-18 | unchanged; exact lightweight RDF pilot remains valid and still does not prove large-corpus maturity |
| Neo4j | the official [current operations manual](https://neo4j.com/docs/operations-manual/current/) identifies 2026.06.0 and calendar versioning; 5.26 remains an LTS documentation line | preserve resident 5.26.26 evidence; a future migration is a separate compatibility/store-format task, not a currentness cleanup |

The old Neo4j GitHub releases page is not current server-version authority.
The official operations manual and exact resident service facts are used
instead. Qdrant stays a retrieval adjunct: vector storage features do not make
it the owner of claims, relations, source identity, or canon.

## II. Established work that still governs the method

The release changes above do not replace the stable methodological floor:

- [OCR-D's end-to-end historical OCR framework](https://dl.acm.org/doi/10.1145/3322905.3322917),
  PAGE/ALTO interchange, and exact page/workflow identities keep recognition,
  coordinates, model data, and processing steps separable;
- [DocLayNet](https://arxiv.org/abs/2206.01062) and the
  [Docling technical report](https://research.ibm.com/publications/docling-technical-report)
  keep layout/reading-order evaluation distinct from transcription quality;
- [BEIR](https://arxiv.org/abs/2104.08663),
  [MTEB](https://arxiv.org/abs/2210.07316), and
  [MIRACL](https://aclanthology.org/2023.tacl-1.63/) show why a benchmark rank
  cannot substitute for a German/Russian ToS query set with hard negatives;
- [ColPali](https://arxiv.org/abs/2407.01449) makes direct visual retrieval a
  legitimate independent modality, not an OCR-quality verdict;
- [Experts, Errors, and Context](https://aclanthology.org/2021.tacl-1.87/)
  and [literary translation evaluation with humans and LLMs](https://aclanthology.org/2025.naacl-long.548/)
  preserve full context, expert judgment, simple comparative questions, and
  separate quality axes above a single score;
- [model cards](https://arxiv.org/abs/1810.03993) remain the minimum reporting
  pattern for intended use and limitations, but a card does not prove a
  conversion, quantization, runtime, or ToS result.

The durable experiment pattern therefore remains: freeze one question and
one source packet; preserve A/B/C identities and outputs; inspect source-visible
failures; measure quality, operator time, machine cost, and speed separately;
and allow no validator, benchmark, or model judge to promote content.

## III. Freshest primary evidence that changes the next preflight

### Historical OCR correction needs two success definitions

The July 2026 [HIPE-OCRepair competition](https://arxiv.org/abs/2607.08143)
finds that LLM-assisted correction can improve historical OCR but varies by
dataset, language, and noise, and systematically over-corrects low-noise input.
Its retrieval-oriented metric is deliberately not diplomatic fidelity.

For ToS, post-correction must therefore declare whether it optimizes faithful
transcription, search access, or both. A search-improving form is a derived
normalization/correction layer and may never silently overwrite diplomatic
text.

The July 2026 ACL study [When Good OCR Is Not Enough](https://aclanthology.org/2026.acl-industry.60/)
finds that low CER/WER can coexist with structural and semantic errors that
damage downstream retrieval. This requires two independent result families:
source-visible transcription/structure review and question-level retrieval
tests. Neither green family proves the other.

### Text and image retrieval remain complementary, not substitutable

[IRPAPERS](https://arxiv.org/abs/2602.17687) reports complementary failures
between text and page-image retrieval and stronger hybrid recall in its
scientific-paper benchmark, while text RAG had better answer alignment than
image RAG. That evidence supports keeping ToS visual r9 independent and
retaining lexical/dense text retrieval; it does not justify fusion until the
same ToS questions have human relevance judgments.

The July 2026 study on [graded relevance thresholds in multilingual dense retrieval](https://aclanthology.org/2026.findings-acl.382/)
shows that an effective binary threshold varies across languages and tasks.
ToS must retain graded, reason-bearing judgments and must not tune one global
German/Russian threshold from model-annotated labels.

### Advertised context and literary quality remain weak admission evidence

[MLRBench](https://aclanthology.org/2026.eacl-long.290/) finds large gaps
across languages and effective use of less than 30 percent of advertised
context in its multilingual setting. The laboratory therefore continues to
use bounded source-returnable packets and measured context, not a model-card
maximum.

[Better Literary Translation](https://aclanthology.org/2026.acl-industry.22/)
supports multi-aspect literary evaluation but also reports that DPO degraded
its own setting. Its English-to-Chinese training result is neither evidence
for German-to-Russian Nietzsche nor authority to create synthetic gold. It
reinforces separate fluency, fidelity, imagery, rhythm, ambiguity, metaphor,
terminology, etymology, and intervention review.

## Decision delta

| Question | 2026-08-08 decision |
| --- | --- |
| Replace completed OCR A/B/C evidence? | no; retain exact executed artifacts and negative runs |
| Rerun structure recovery under Docling 2.118.1? | no; only a new source/structure question may open a separately versioned challenger |
| Upgrade host OpenVINO from 2026.2 to 2026.3? | not admitted; first prove an exact task need and compatibility/rollback plan in the machine owner |
| Track the newest `llama.cpp` build tag? | no; pin an exact commit/build/backend for each future receipt |
| Acquire Qwen3.6 or another new LLM? | no; the current small-task evidence and resident routes do not justify it |
| Replace Qwen/Granite embeddings? | no; current owner families remain adequate controls and their ToS evidence is already bounded |
| Adopt Qdrant 1.19.0? | no; retain as conditional projection until an exact filtering/memory/retrieval question exists |
| Migrate resident Neo4j 5.26.26? | no; migration would be a separate owner task and cannot improve the already frozen comparison retroactively |
| Use Stanza 1.14.0 now? | no; if a linguistic question opens it, freeze package and compatible model-data artifacts with security-safe acquisition |
| Open human review? | no; release currentness alone is not a content question or rare human checkpoint |

## Current machine fit, without execution

The 2026-08-08 host owner snapshots were read after the web research:

- OpenVINO 2026.2 sees CPU, integrated Arc GPU, and Intel AI Boost NPU;
- the exact Gemma 4 E2B and E4B community GGUF profiles remain locally present
  and `ready`, bound to resident `llama.cpp` commit `05ff59c`;
- the registered Qwen3.6 27B routes remain `model-missing` and therefore supply
  no acquisition or execution fact;
- both `/` and `/srv` remain in the owner's `green` pressure class;
- no inference, benchmark, model load, download, compilation, or thermal load
  was initiated for this refresh.

These are capability and custody facts only. They do not prove runtime quality
on a ToS packet. The machine law remains unchanged: stable temperatures through
105 degrees Celsius are within the monitored normal range; values above 105
require trend and context, while emergency behavior is around 109–110. A
future experiment must use the machine owner's current preflight rather than
copy these time-bound observations.

## Next-use gate

Before any one of these surfaces executes again, the exact task packet must
name:

1. the source/question and why existing evidence is insufficient;
2. exact binary, model, model-data, configuration, image/container, and
   revision identities with digests;
3. license, rights, data-egress, cache/storage, and rollback posture;
4. CPU/GPU/NPU route, context, memory, swap, time, and thermal stop conditions;
5. diplomatic, structure, retrieval, translation, or semantic success axes
   without collapsing them;
6. the source-visible manual checks that can falsify a green machine result;
7. the explicit stop line and the authority that any output still lacks.

## Stop line

This refresh closes the broad software/model currentness debt through
2026-08-08. It changes version/watch facts and strengthens future preflight,
but changes no source, text, translation, sign, concept, graph, rights,
promotion, or human-review state. Historical experiment reports retain their
exact versions and are not rewritten as if newer software had produced them.
