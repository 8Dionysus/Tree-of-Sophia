# OCR A/B/C Candidate Admission

Status: A retained as deterministic comparator; B R1 stopped and rejected; C full-packet mechanics executed and awaiting human review

Snapshot: 2026-07-28

Experiment: `tos-ocr-foundation-v1`

## Decision

The visual OCR comparison admits three recognition families, sequentially:

1. **A — Tesseract 5.5.2**, the deterministic classical baseline;
2. **B — Kraken 7.0.2 + Party v4**, the current trainable historical-document
   challenger;
3. **C — PaddleOCR 3.7.0 + multilingual PP-OCRv5**, the independent pipeline
   challenger.

The gate was applied sequentially. A was materialized first and retained after
two mechanically identical runs. That decision admitted B. B was then stopped
under the experiment law and its unchanged configuration was rejected. Only
then was C acquired and tested: one failed configuration, one invalidated
runner scope, two exact one-page repeats, and one complete 36-page run are all
retained as distinct evidence. This ordering prevented eager downloads, cache
pressure, and retrospective candidate selection.

All routes consume exactly the same 36 full-page PNG files rendered once by
Poppler at 300 DPI. The render manifest and image SHA-256 digests freeze before
the first OCR output. No route may crop, deskew, denoise, threshold, enhance,
or rerender its private input in the primary comparison.

## Frozen source packet

The projection is
`ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/ocr-visual-samples.json`:

- 12 pages from the exact 1893 Naumann German image-container PDF;
- 12 pages from the Antonovsky Russian PDF;
- 12 pages from the 1996 *Mysl* Russian collected-works PDF;
- five human-gold candidates per source, selected before output;
- the same-edition German EPUB and the 1996 embedded ABBYY layer sealed until
  all independent contestant outputs and digests are frozen.

Formal CER, WER, correction time, reading-order quality, and winner claims are
blocked until five diplomatic transcriptions per source have been
double-checked by a human against visible page regions. Exploratory runs may
measure mechanics, speed, resource use, output shape, omissions, and obvious
source-visible failures. A green schema or successful process exit is not an
OCR-quality result.

## Executed evidence through 2026-07-23

### A — retained mechanical comparator

Runs `zarathustra-ocr-a-tesseract-20260722-r1` and
`zarathustra-ocr-a-tesseract-20260722-r2` each resolved all 36 frozen anchors.
Their method fields, per-sample recognition outputs, and recognition-set digest
were identical. R1 took 40.15 seconds and R2 31.74 seconds (53.80 and 68.04
pages/minute respectively); their observed child peak RSS values were about
126.7 MB and 126.6 MB. This earns retention as the deterministic classical
comparator only.

Six source-visible model inspections found substitutions, marginal-number and
reading-order defects, additions, and uncertainty. Five were
`accept-with-limits` and one was `uncertain`. These are advisory nonhuman
observations. No human diplomatic gold, CER, WER, correction-time result, or
accepted transcription exists.

### Independent raw-artifact recomputation

On 2026-07-28, a one-off shell inspection independently recomputed the
mechanical aggregates for A R2 from its immutable private artifacts. It did
not call the runner, its metric-building functions, or a repository validator.
The inspected inputs were:

- `run.receipt.json`, SHA-256
  `a6eb5ed985870b2b67b5d16b152db7c25f5075961020affd8582a879aaecc517`;
- `raw-output/ocr-output-manifest.json`, SHA-256
  `91e1f9574d9c88343c9ca3b6b31ca9731e9fab93cc464e21a4bea88871d8f8b8`;
- `metrics/tesseract-ocr-summary.json`, SHA-256
  `8ef969c23712ee5ad106e743f1ffab6af5464ce97a8d5ebf554693d546aed13d`.

The inspection recalculated file digests and byte sizes directly for all 144
text/TSV/hOCR/ALTO outputs; Unicode and non-whitespace character counts for
all 36 text files; word-row counts, confidence-row counts, per-page mean and
median confidence directly from all 36 TSV files; unique source and sample
counts; empty-output count; mean page confidence; and pages per minute from
the recorded wall interval. Results:

| Recomputed field | Raw result | Recorded result | Mismatch |
| --- | ---: | ---: | ---: |
| output files checked | 144 | 144 declared | 0 |
| text count records checked | 36 | 36 declared | 0 |
| TSV diagnostic records checked | 36 | 36 declared | 0 |
| samples / unique sources / empty texts | 36 / 3 / 0 | 36 / 3 / 0 | 0 |
| mean of page confidences | 88.343545980493 | 88.34354598049339 | below 1e-12 |
| pages per minute | 68.044794448293 | 68.04479444829315 | below 1e-12 |

This closes one independent recomputation gate for **mechanical metrics only**.
It is stronger than a green schema check because it reads the output bytes and
TSV rows again through a separate one-off route. It still cannot produce CER,
WER, reading-order accuracy, correction cost, source acceptance, or an OCR
winner: those require independent accepted gold rather than engine output.

### B — stopped negative result

Run `zarathustra-ocr-b-kraken-party-20260723-r1` completed 27 of the 36 frozen
pages. Naumann `p046` stopped after segmentation and language conditioning and
has no recognition output. The owner scope ran for 3 hours 50 minutes, consumed
13 hours 29 minutes of CPU, reached a 3.7 GB memory peak and an 809 MB swap
peak. The 27 completed pages accumulated 12,246.84 recognition seconds; the
slowest completed page, Antonovsky `p007`, took 1,452.60 recognition seconds.

Source-visible inspection exposed repeated method-introduced failures:

- Antonovsky `p002` duplicated a prior line while omitting the following
  editorial-name line;
- Antonovsky `p005` and `p007` drove multiple table-of-contents lines toward
  the 384-token decoder cap, with repeated or invented neighboring strings;
- *Mysl* `p008` omitted a philosophically material clause and duplicated a
  preceding line.

Seven advisory model inspections are retained: three `reject` and four
`accept-with-limits`. The Naumann `p044` check was a post-stop diagnostic, not a
preselected inspection; the preselected German `p060` and `p381` checks were
not reached. Therefore no German transfer conclusion is permitted.

The run was deliberately stopped under `MACHINE_FIT.md#stop-conditions`, not
lost or relabeled as success. Its receipt is `stopped`, its unchanged full-page
Party configuration is `rejected-candidate`, and the complete negative evidence
is retained. It receives no R2: a repeat of the same unsafe method would add
cost without a new hypothesis. Any future Party route must declare a revised
method and fresh experiment identity. This rejection is the explicit gate that
admits C; it is not an OCR-family-wide claim against Kraken or Party.

### C — bounded independent challenger

The isolated runtime is
`/srv/abyss-machine/runtimes/tree-of-sophia-foundation-lab/paddleocr-3.7.0-paddlex-3.7.2-paddle-3.3.1-cpu`.
Its manifest SHA-256 is
`6ade62666178becc448e412421ce5a6f511c84ed45856bb07bbb68eb71bb51e9`
and its complete software/model artifact-set digest is
`b8d239f91b99d1b8dd427ba453c71d638bd204792a00697d250f08742de191ee`.
The detector and two recognizer archives remain individually fixed in that
manifest. The runtime changed neither the system Python nor a project
environment.

C preserves its failures instead of rewriting them into the successful route:

- `zarathustra-ocr-c-paddle-20260723-r1` completed zero pages and was OOM-killed
  at 16.7 GB RAM plus 1.3 GB swap. The bridge had omitted an explicit detector
  resize, so the installed `64/min` pipeline behavior expanded the 2480 x 3509
  page to approximately 2496 x 3520 for detection. This rejects that
  configuration path; it is not evidence that the detector family cannot fit.
- `zarathustra-ocr-c-paddle-explicit-960-max-20260723-r2` was invalidated when a
  one-page CLI request expanded to all 36 pages because the dispatcher failed
  to forward the selected sample IDs. The operator stopped the exact service
  after four bridge-stage pages. Its 3.6 GB RAM and 1.6 GB swap observations are
  machine-fit evidence only; none of its output enters the quality comparison.
- After the dispatcher regression was fixed, R3 and R4 each processed exactly
  Antonovsky `p002` with `960/max`. Their recognized text and ordered region
  records are byte-for-byte and semantically identical. The owner measured
  27.151 and 26.106 seconds of service wall time, 3.7 and 3.6 GB RAM peaks, and
  zero swap. This establishes repeatability for the bounded diagnostic, not
  accuracy or full-packet repetition.
- `zarathustra-ocr-c-paddle-explicit-960-max-full-20260723-r5` then resolved all
  36 frozen anchors with no empty page: 605.56 seconds runner wall time,
  3.567 pages/minute, 1,029 regions, 4 GB owner-reported RAM peak, and 136.8 MB
  swap peak. The service used 10 minutes 8.973 seconds wall time and 30 minutes
  16.868 seconds CPU time. Its status remains `awaiting-manual-review`.

Six source-visible model inspections of R5 are advisory and
`accept-with-limits`. They exposed errors a successful exit cannot detect:
marginal line numbers inserted into Russian prose and verse, joined words and
lost hyphens, case and punctuation loss, a handwritten German annotation
invented as body text, a printed-word substitution, and footer/page material
included in the body. The alternating clock labels and verse on *Mysl* `p166`
were recovered in the intended sequence, while Naumann `p381` was mostly
strong; neither observation supplies a human acceptance decision.

C is therefore retained as a mechanically viable independent challenger, not
as the winner. R3/R4 prove an exact one-page repeat and R5 proves one complete
packet execution; a second full-packet repeat is not claimed. CER, WER,
reading-order scoring, correction time, final retention, and any A/B/C quality
ranking remain blocked on the real double-checked human diplomatic gold and
source-visible human review.

## A — classical baseline

| Field | Frozen value |
| --- | --- |
| software | Tesseract `5.5.2-1.fc44` |
| language data | Fedora `tesseract-langpack-deu` and `tesseract-langpack-rus` `4.1.0-12.fc44` |
| execution | isolated host-managed CPU lane; direct recognition of shared PNGs |
| outputs | text, TSV, hOCR, ALTO where supported, timing, confidence, exact runtime/data digests |
| optional packaging | OCRmyPDF 17.8.1 only after recognition retention |
| principal risks | old typography, page segmentation, punctuation loss, misleading confidence |

The Tesseract and language-data RPMs were downloaded and extracted into the
versioned `tesseract-5.5.2-fc44` host runtime; they were not installed into the
Fedora root. OCRmyPDF does not render or recognize in the primary A route and
cannot hide a failed page behind a searchable derivative.

## B — historical-document challenger

| Field | Frozen value |
| --- | --- |
| Kraken | 7.0.2; PyPI wheel SHA-256 `5882392e9e4ffdb69bf582483ce0d238017d0e8ad61b0ada38b97d2504e9bb21` |
| Party code | commit `c2589b1b515ed690f883c6afaef6c01ce29bf72d` |
| Party model | v4, DOI `10.5281/zenodo.20642057`, 518,329,816 bytes, source MD5 `cf165e67061d492b72f600a6a72b7c61` |
| license | Apache-2.0 for Kraken, Party, and the frozen model record |
| execution | isolated Python 3.12 CPU lane; Kraken segmentation plus language-conditioned Party recognition |
| principal risks | segmentation sensitivity, inconsistent transcription normalization, mixed public/restricted/private training provenance |

Kraken 7.0.2 is deliberately frozen with Lightning 2.6.1. Its release warns
that Lightning 2.6.2 and 2.6.3 were compromised; dependency resolution must
not upgrade through that range. Party's published held-out CER values are not
portable evidence for Nietzsche, these scans, or ToS punctuation fidelity.
The model is useful as an already multilingual historical-print prior, but a
future fine-tune requires a dataset-level provenance, license, normalization,
and transfer-risk audit.

Primary sources:

- [Kraken 7.0.2 release](https://github.com/mittagessen/kraken/releases/tag/7.0.2)
- [Party repository](https://github.com/mittagessen/party)
- [Party v4 model record](https://zenodo.org/records/20642057)

## C — independent multilingual pipeline

| Field | Frozen value |
| --- | --- |
| PaddleOCR | 3.7.0; wheel SHA-256 `c0f0a81ad4112727f30c6fcf986ac0ef6a120d31ee0991a01fae0357ee32d338` |
| PaddleX | latest compatible 3.7.x at admission: 3.7.2; wheel SHA-256 `f1678bf650bbaccfd8f0d4e49d0ae631b4685c829fdae6e802ccd90d4fcb9a7f` |
| PaddlePaddle | CPU 3.3.1, CPython 3.12 wheel SHA-256 `9016fc497213e1101261684321fbb31ef5960019ef39cb07ded27bc70e2a9858` |
| detector | `PP-OCRv5_server_det` |
| German recognizer | `latin_PP-OCRv5_mobile_rec` |
| Russian recognizer | `eslav_PP-OCRv5_mobile_rec` |
| license | Apache-2.0 for the exact software wheels and acquired official model archives, recorded in the runtime manifest |
| execution | isolated Python 3.12 CPU lane; `paddle_static`, MKLDNN off, batch 1, four threads, detector resize `960/max` |
| principal risks | mixed Latin/Cyrillic glyph substitution, marginalia and footer intrusion, reading-order errors, detector/recognizer normalization |

PP-OCRv6 is current inside the 3.7.0 release but does not cover Russian. It is
therefore excluded from the primary C route rather than being given a German
advantage and a different Russian fallback. The documented PP-OCRv5 Latin and
East-Slavic recognizers provide the necessary two-language coverage. Although
the early candidate landscape included PaddleOCR-VL, the admitted C route is
the general multilingual OCR pipeline: it is independent from Tesseract and
Party, covers both frozen languages with one detector family, and fit the host
after the resize path was bounded. A document VLM remains a separate hard-page
structure hypothesis because its generative error surface is not comparable to
this page-grounded recognition test.

Primary sources:

- [PaddleOCR 3.7.0 release](https://github.com/PaddlePaddle/PaddleOCR/releases/tag/v3.7.0)
- [PP-OCRv5 multilingual documentation](https://paddlepaddle.github.io/PaddleOCR/latest/en/version3.x/algorithm/PP-OCRv5/PP-OCRv5_multi_languages.html)

## Excluded or conditional alternatives

- Native PDF text and EPUB XHTML are reference witnesses, not contestants.
- The German-only Kraken `german_print` model remains a literature/control
  candidate; it cannot cover the Russian pages without changing families.
- PP-OCRv6 is excluded from primary C because it lacks Russian coverage.
- PaddleOCR-VL, Docling VLM, and other visual-language models remain separate
  hard-page structure experiments. Their hallucination surface would confound
  a recognition-family comparison.
- eScriptorium and OCR4all remain second-phase workflow/training candidates if
  no admitted printed-OCR route reaches an acceptable human correction cost.
- LLM transcription cannot silently replace page-grounded OCR. It may only
  propose reversible corrections after source-aligned OCR exists.

## Materialization and stop law

Before each candidate is downloaded or run:

- refresh live `/srv` pressure and pass `abyss-machine storage write-preflight`;
- record a host change preflight, exact expected download/output bytes,
  licenses, digests, runtime root, cache root, and removal route;
- verify no conflicting heavy workload, then execute through
  `abyss-machine resource launch --class heavy --kind ai`;
- treat the machine owner as the thermal authority: 100–105 °C is the normal
  monitored active range; above 105 °C is watch, 106 °C hot, and 109 °C
  critical. No local 90 °C rule may be invented;
- stop on digest mismatch, owner denial/revocation, material swap growth,
  silent device/backend change, missing pages, rights ambiguity, or loss of
  page-to-output traceability;
- stop when repeated hallucinated transcription makes further batching unsafe
  before method revision, or when the same error class persists without a new
  hypothesis. A mechanical near-decoder-cap guard may pause a route for review,
  but it cannot itself decide source-visible accuracy.

Each completed route needs two mechanically identical full-packet runs before
its final retain/reject decision. A meets that requirement. C currently has two
identical one-page diagnostics and one full-packet execution, so only that
narrow repeatability claim is made; full-packet repetition and final retention
remain open. Reproducibility proves only that the mechanism repeats, while
quality remains a source-visible human judgment. A route stopped by the law is
not a completed route and must not be forced through a second run merely to
make the matrix look complete.
