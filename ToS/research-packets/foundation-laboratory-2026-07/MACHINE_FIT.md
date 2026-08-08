# ToS Foundation Laboratory — Machine-Fit Matrix

Status: live-host research snapshot plus bounded graph, retrieval, and OCR A/B/C evidence, not a general benchmark result

Observed: 2026-07-23; live model/device/storage facts refreshed 2026-07-26;
direct-visual runtime, admission, and execution refreshed 2026-07-30;
Russian-surface Qwen GEC admission and execution refreshed 2026-08-08

Host owner: `/etc/abyss-machine/AGENTS.md` and storage policy

## Outcome first

The host can run a meaningful ToS laboratory without downloading a new large
model. It is suited to sequential small-model, OCR, embedding, and graph
trials; it is not suited to casual concurrent model loading or first-wave
35B-class experiments.

The first wave reuses resident Gemma 4, Qwen3, Qwen3 Embedding, OpenVINO,
llama.cpp, Neo4j, and the now host-managed PyOxigraph 0.5.9 runtime. Retrieval
A/B earned one bounded independent-family C artifact: Granite Embedding 311M
Multilingual R2's official INT8 OpenVINO subset. The exact subset and isolated
tokenizer runtime are installed, smoke-tested, and have completed the frozen C
packet twice with identical indexes and rankings. No C output had been
inspected when the method was frozen. This is machine-fit and reproducibility
evidence, not a quality finding.

The sequential OCR lane is now materially executed. Tesseract A completed the
36-page packet twice, Kraken/Party B was stopped at 27/36 and rejected in its
unchanged configuration, and multilingual PaddleOCR C progressed through an
OOM configuration failure, an invalid runner-scope attempt, exact one-page
repeats, and one successful 36/36 run. These are execution, resource, and
advisory inspection facts. Human-gold metrics and an OCR winner remain absent.

One later source-free Russian-surface question admitted the smaller
Qwen3.5-0.8B base plus a pinned SyntErr-to-LoRuGEC adapter in a separate BF16
CPU runtime. The 36-control run completed in 111.711071750 seconds with a
3.5 GiB cgroup peak and zero process swap, proving fit. All outputs were then
read: only 3/15 correction targets were exact, two clean literary controls and
one intentional repetition were changed. Machine fit therefore does not imply
method fit; this profile is rejected as a required or autonomous correction
stage.

## Observed machine surface

| Surface | Observation | Laboratory implication |
| --- | --- | --- |
| OS | Fedora 44 | prefer current native packages or isolated host-managed runtimes |
| CPU | Intel Core Ultra 9 285H, 16 logical CPUs, AVX2 | strong deterministic OCR/extraction and small-model CPU fallback |
| RAM | 33,035,067,392 bytes total; roughly 15–20 GiB available during probe | run one heavy model route at a time |
| swap | zram about 20.6 GB, roughly 7.2 GB already used during probe | swap activity is a stop signal, not free model capacity |
| GPU | Intel Arrow Lake-P Arc Pro 130T/140T integrated GPU; Vulkan 1.4/OpenCL visible | shared-memory Vulkan/OpenVINO trials are viable; no dedicated-VRAM assumption |
| NPU | Intel `8086:7d1d`, `intel_vpu`, `/dev/accel/accel0` | only compatible fixed/INT4 OpenVINO routes; do not assume universal support |
| OpenVINO | `2026.2.0-21903-52ddc073857-releases/2026/2`; CPU/GPU/NPU visible at `2026-07-26T16:41:58-06:00` | preferred cross-device comparison surface; model/device proof still required |
| root filesystem | 97,383,477,248 bytes free in the owner pressure snapshot | large mutable AI artifacts still prohibited here |
| `/srv` | 123,556,466,688 bytes free, 72.11% used, owner class `green` at `2026-07-26T16:46:26-06:00` | capacity is not acquisition permission; every material download still requires preflight |
| existing AI cache | `/srv/abyss-machine/cache/ai` about 44 GB | reuse and account for cache; do not duplicate weights |
| llama.cpp runtimes | pinned Vulkan+CPU build at commit `05ff59cb57860cc992fc6dcede32c696efea711c` | preferred resident GGUF baseline |
| oneAPI | about 5.7 GB present | SYCL is possible but not first-route ready |

This is a capability snapshot. The latest owner benchmark had skipped all
three OpenVINO devices and was not an affirmative performance result.
See `LOCAL_LLM_ADMISSION.md` for the 2026-07-26 official-docs, established
work, and freshest-primary-work refresh and the exact no-download decision.

## Storage and runtime law

- Source repositories do not own host model/runtime installation.
- Regenerable caches route to `/srv/abyss-machine/cache`.
- Host-managed runtimes route to `/srv/abyss-machine/runtimes`.
- Model or runtime writes must first pass:

  ```bash
  abyss-machine storage write-preflight
  ```

- Heavy trials must run through the owner resource route, conceptually:

  ```bash
  abyss-machine resource launch --class heavy --kind ai -- <command>
  ```

- Actual commands, runtime digests, thermal preflight, and exit receipts belong
  in the abyss-stack experiment record. The graph A/B/C runs now provide that
  bounded evidence; this packet still does not own execution.
- Project witnesses remain under the narrowly gitignored ToS item store; model
  caches and benchmark scratch data do not.

## Resident model inventory relevant to ToS

| Model | Format/runtime | Approximate stored size | First-wave role | State |
| --- | --- | ---: | --- | --- |
| Gemma 4 E2B | GGUF UD-Q4_K_XL, llama.cpp Vulkan+CPU | 3,184,494,720 bytes | low-cost LLM A | ready after task-material gate |
| Gemma 4 E4B | GGUF UD-Q5_K_XL, llama.cpp Vulkan+CPU | 6,656,152,736 bytes | stronger LLM B | ready after task-material gate, sequential |
| Qwen3-Embedding-0.6B | OpenVINO INT8 | 612 MB | dense retrieval/alignment | ready |
| Granite Embedding 311M Multilingual R2 | official OpenVINO INT8 subset at exact revision | 348,082,051 bytes across 11 selected files | independent multilingual Retrieval C | installed in host cache; CPU smoke and two frozen packet runs pass mechanically |
| Qwen3 4B | OpenVINO INT4 | 2.29 GB plus existing repository objects | LLM C/family/runtime control | ready after task-material gate via owner/deployment route |
| Qwen3 8B | OpenVINO INT4 | 4.88 GB plus existing repository objects | conditional capacity challenger | present, not default |
| Phi-3.5 mini | OpenVINO INT4 | 2.09 GB plus existing repository objects | older control | present, conditional |
| Whisper tiny / large-v3-turbo | OpenVINO | present | future spoken/recitation branch, outside first text lab | out of current scope |

`/srv/AbyssOS/abyss-stack/Models` is a deployed project surface and is not to
be mutated or treated as a download target. Tests must reference assets
through the owner-approved route or copy only when a later explicit contract
requires it.

## Software readiness

| Component | Observed state | First use | Missing gate |
| --- | --- | --- | --- |
| Poppler utilities | resident | PDF forensics/render/native extraction | capture exact version in run receipt |
| Tesseract 5.5.2 | isolated `tesseract-5.5.2-fc44` runtime materialized; exact runtime-manifest SHA-256 `83b73ba91c49e53dbe75bd458e4ecc0c0558f2288ec43ff758fc6d51b22f5ca9` | OCR A completed twice on all 36 pages | human diplomatic gold and source-visible human review |
| OCRmyPDF 17.8.1 | packaging candidate, not installed for this lab | optional searchable derivative after OCR retention | isolated Python 3.12 runtime; never owns rendering or recognition |
| Kraken 7.0.2 + Party v4 | isolated `kraken-7.0.2-party-c2589b1` runtime materialized; manifest SHA-256 `304bd3c3aee20a83ae5f2c1a28ae3ea466d8d5d5808bfd8eb9b99d07c53ad110` | B stopped at 27/36 after decoder saturation and source-visible failures | unchanged configuration rejected; any new Party route needs a new hypothesis and experiment identity |
| OpenVINO 2026.2 | CPU/GPU/NPU visible | embeddings, Qwen3, OCR candidate | per-model device compatibility proof |
| llama.cpp Vulkan | host-managed build available | Gemma 4 trials | prompt and sampling receipt contract |
| llama.cpp SYCL | oneAPI present; command not ready on PATH | optional Intel GPU comparison | build/version/dependency justification |
| PaddleOCR 3.7.0 + PaddleX 3.7.2 + PaddlePaddle CPU 3.3.1 | isolated runtime and exact server detector plus Latin/East-Slavic recognizers materialized; manifest SHA-256 `6ade62666178becc448e412421ce5a6f511c84ed45856bb07bbb68eb71bb51e9` | C one-page exact repeat and 36/36 full-packet execution | second full-packet repeat, human diplomatic gold, correction cost, and source-visible human review |
| Docling | candidate, version must be >=2.91 | layout/structure challenger | isolated environment, exact 2.114.x or reviewed later version |
| INCEpTION | not installed for this lab | annotation challenger | Java/service storage and reversible trial route |
| CATMA | not installed for this lab | qualitative annotation challenger | deployment/export/reversibility review |
| PyOxigraph 0.5.9 | isolated host runtime installed under `/srv/abyss-machine/runtimes/tree-of-sophia-foundation-lab/pyoxigraph-0.5.9`; named-graph/SPARQL smoke and Graph C run passed | RDF projection challenger | exact wheel, source revision, runtime manifest, license, delete/rebuild, and Store bytes captured |
| Neo4j 5.26.26 Community | resident `abyss_neo4j_1` service used by Graph B through a credential-contained bridge | property-graph challenger | shared-store cost is not exactly isolatable; namespace cleanup and community authorization boundary remain required |
| Granite R2 tokenizer + OpenVINO IR | exact revision installed under host cache/runtime roots; Tokenizers 0.23.1 isolated; OpenVINO 2026.2 inherited read-only | independent Retrieval C | exact 11-file digests, failed `AFFINITY` attempt, corrected CPU smoke, owner-receipted repeat, and deletion route retained; human quality gate remains |

Installing all candidates up front is explicitly rejected. Each setup follows
the experiment that needs it and has a rollback receipt.

### OCR candidate footprint and comparability

| Route | Frozen software/model surface | Known acquisition floor | Admission state |
| --- | --- | ---: | --- |
| A | Fedora 44 Tesseract `5.5.2-1.fc44`; `deu`/`rus` language packs `4.1.0-12.fc44`; optional OCRmyPDF 17.8.1 | 8,927,653-byte isolated runtime tree | retained deterministic comparator after exact R1/R2 recognition repeat; about 126.6-126.7 MB child peak RSS |
| B | Kraken 7.0.2; Party commit `c2589b1b515ed690f883c6afaef6c01ce29bf72d`; Party v4 DOI `10.5281/zenodo.20642057`, 518,329,816-byte model | 1,816,411,644-byte isolated runtime tree | R1 stopped at 27/36; 3.7 GB RAM and 809 MB swap peaks; unchanged configuration rejected |
| C | PaddleOCR 3.7.0; PaddleX 3.7.2; PaddlePaddle CPU 3.3.1; `PP-OCRv5_server_det`; `latin_PP-OCRv5_mobile_rec`; `eslav_PP-OCRv5_mobile_rec` | 1,268,461,715-byte isolated runtime tree; artifact-set SHA-256 `b8d239f91b99d1b8dd427ba453c71d638bd204792a00697d250f08742de191ee` | one-page exact repeat at 3.6-3.7 GB RAM and zero swap; full 36-page run at 4 GB RAM plus 136.8 MB swap; awaits human review |

All three primary routes are CPU routes over one digest-frozen 300 DPI Poppler
render. This keeps backend/device and input pixels fixed while the recognizer
changes. Any later GPU/OpenVINO or VLM run is a new experiment, not a silent C
substitution. The embedded ABBYY layer and same-edition EPUB OCR are sealed
reference witnesses and add no contestant runtime cost.

The C evidence also defines a configuration boundary. Its first run inherited
Paddle's installed `64/min` detector preprocessing and was OOM-killed at
16.7 GB RAM plus 1.3 GB swap before completing a page. Explicit `960/max`
reduced the one-page owner peak to 3.6-3.7 GB with zero swap. After a separate
runner-scope defect was fixed and regression-tested, the unchanged bounded
method completed all 36 pages in 10 minutes 8.973 seconds of service wall time,
with 4 GB RAM and 136.8 MB swap peaks. This supports the bounded CPU method on
this host. Observed temperatures remained 76-80 C, below the machine owner's
100-105 C normal monitored active band; neither preserved C failure was
thermal. These facts do not convert engine confidence or successful completion
into OCR accuracy.

The PyOxigraph setup is the bounded exception now earned by Graph C. It added
about 31 MB in a versioned host runtime, mutated neither system Python nor a
project environment, and passed the `abyss-machine` storage and resource
owner gates. Its successful 13-claim run is evidence for this narrow RDF lane,
not proof of large-corpus query maturity.

The Granite setup is the bounded exception earned by Retrieval A/B and frozen
before C output. It downloaded only the official INT8 OpenVINO/tokenizer subset
at revision `44399559930365213510b1ee2eb15ded83374f0e` (348,082,051 bytes),
mutated neither system Python nor a project environment, and passed exact-file,
runtime, storage, resource, and AI checks. The first smoke failed because
OpenVINO 2026.2 does not accept the generic `AFFINITY` property; that negative
receipt is retained. The corrected CPU smoke produced three 768-dimensional
vectors, matched all three expected cross-lingual routes, used 392 MB peak by
the owner systemd receipt, and completed in 3.056 seconds. None of this is a
retrieval-quality result.

The owner-receipted Retrieval C repeat completed in 19.400 seconds with a
663.5 MB cgroup memory peak and zero swap. It embedded 24 nonempty passages,
served 20 queries, wrote a 237,643-byte run-local index, and reproduced the
first run's exact index and all 20 ranked-anchor lists. Median warm query
embedding plus exact ranking was 16.18 ms. The bridge's own peak-RSS field is
higher than the cgroup peak, so the discrepancy remains an instrumentation
finding; the owner receipt is the machine-cost authority. Quality metrics and
human cost remain null.

## Runtime A/B/C

| Route | Backend | Intended models | Why it enters | Stop condition |
| --- | --- | --- | --- | --- |
| A | llama.cpp Vulkan | Gemma 4 E2B/E4B GGUF | already host-managed and suited to Intel iGPU | shared-memory pressure, instability, or thermal guard |
| B | OpenVINO GenAI | Qwen3 4B/8B, embedding; CPU/GPU/NPU per model | current Intel owner stack and multiple devices | unsupported operation, conversion mismatch, or non-reproducible device fallback |
| C | llama.cpp SYCL | one selected GGUF only | independent Intel GPU backend | build/dependency cost exceeds expected diagnostic value |

NPU is a subroute of B, not an automatic third winner. Current OpenVINO NPU
generation requires compatible quantization, group size, shapes, and sometimes
pinned conversion dependencies. A model working on CPU/GPU is not proof it
works correctly on NPU.

## Capacity classes

| Class | Working-set intent | Examples | Concurrency |
| --- | --- | --- | --- |
| `tiny` | under about 1 GB model/data working set | embedding, lexical index, Tesseract page OCR | may coexist with normal work after owner check |
| `small` | roughly 2–4 GB weights or measured working set | E2B, Qwen3 4B, bounded PaddleOCR/Kraken page OCR | one model route at a time |
| `medium` | roughly 5–8 GB weights plus runtime/KV | E4B, Qwen3 8B | exclusive heavy reservation; short context |
| `large` | above about 10 GB weights or uncertain peak | Qwen3.6 35B total weights, large VLM | defer on this host |

These are planning bands, not promised peak RSS. Every pilot records measured
peak memory. Context length is a major part of the working set; advertised
maximum contexts are never the default local test setting.

## Model candidate fit

| Candidate | Download | Expected fit | Decision |
| --- | --- | --- | --- |
| resident Gemma 4 E2B | none | comfortable small-model baseline | first wave A |
| resident Gemma 4 E4B | none | feasible sequential workhorse | first wave B |
| resident Qwen3 4B INT4 | none | feasible OpenVINO family/runtime control | first wave C |
| resident Qwen3 8B INT4 | none | feasible but higher memory/cost | only after smaller-model failure taxonomy |
| Qwen3.5-0.8B + SyntErr-to-LoRuGEC adapter | acquired exact base plus two selected adapter files for local research | measured 3.5 GiB cgroup peak, zero process swap, 111.711071750 s for 36 controls | machine-fit proven; quality-negative profile rejected as required/autonomous Russian-surface stage; no private-candidate run, production admission, or redistribution |
| Qwen3.5-4B | new artifact | likely feasible with reduced context | second wave after storage preflight |
| TranslateGemma 4B | gated new artifact | runtime feasible; rights/context uncertain | license-gated second wave |
| Granite Embedding 311M Multilingual R2 | 348,082,051 bytes already present | strong CPU/OpenVINO fit; independent IBM family; 200+ languages | Retrieval C executed twice; deterministic mechanics proven; human quality unresolved |
| gte-multilingual-base | new compact artifact | feasible but older repository route requests remote model code | retain as comparator, not selected for C |
| BGE-M3 | roughly 2.27 GB model artifact | feasible but heavy and multi-mode | retain as literature reference; do not confound current C |
| Qwen3-VL-Embedding-2B | exact 4,549,670,315-byte acquisition and 1,182,279,019-byte isolated CPU runtime in host-owned storage | fresh unforced preflight passed with 19,101,630,464 available bytes against the unchanged 17,179,869,184-byte floor; r9 completed in 2,807.234 s with 4,808,843,264-byte memory and 380,157,952-byte swap peaks | retain audit-complete r9 mechanics and r8 negative; no rerun without a new question; human relevance and adoption remain open |
| PaddleOCR-VL 0.9B | new runtime/models | potentially feasible on Intel path | hard-page conditional only |
| Qwen3.6 35B-A3B | large new artifact | total-weight and memory/storage pressure | reject first wave |

## Experiment scheduling

The safe order is designed to gain evidence before resource cost:

1. format forensics and native extraction;
2. frozen visual projection and one shared digest-frozen 300 DPI page render;
3. bounded Tesseract A setup and two mechanically identical full-packet runs;
4. retain A as comparator, then admit Kraken/Party B and stop its unsafe
   unchanged configuration at 27/36;
5. admit multilingual PP-OCRv5 C, preserve its failed and invalid runs, prove
   an exact one-page repeat, and complete one 36-page run;
6. complete the five human gold pages per source, optionally repeat the full C
   packet if the final repeatability decision requires it, reveal sealed reference
   witnesses only after independent drafts freeze, and perform manual review;
7. resident E2B, E4B, and Qwen3 4B sequential LLM proposals;
8. preserve the separately frozen visual-retrieval decision and prepared
   Qwen3-VL CPU route; execute it only after fresh unforced resource admission;
9. only then decide whether fusion or another service/model install is
   justified.

OCR, embeddings, LLMs, graph services, and annotation services should not all
run concurrently. Experiments are small enough that isolation is more
valuable than throughput.

## Required measurement fields

Every run receipt should record:

- experiment/run ID and immutable input digests;
- git commit/worktree, package version, model artifact digest, runtime build;
- backend and resolved device, quantization, context, batch, threads, and seed;
- start/end timestamps and wall-clock duration;
- peak RSS, GPU/shared-memory observation when available, swap delta, disk
  bytes written, and cache delta;
- input/output unit counts: pages, pixels, characters, tokens, passages,
  queries, nodes, or edges;
- automatic mechanics metrics, clearly labeled;
- manual error counts by type/severity and reviewer time;
- crashes, fallbacks, warnings, skipped units, and retry history;
- estimated marginal cost: machine time + operator minutes + retained bytes;
- verdict: promote, retain as comparator, rerun, defer, or reject, with reason.

## Stop conditions

A run stops and is recorded as such when any of the following occurs:

- owner resource route denies or revokes the reservation;
- storage class remains `watch` and projected write is not explicitly allowed;
- free `/srv` capacity would cross the owner safety threshold;
- swap grows materially or the desktop/active work becomes unstable;
- thermal or power owner reports a guard condition;
- backend silently falls back to a different device;
- output loses input-to-source traceability;
- repeated hallucinated transcription or translation makes further batch work
  unsafe before method revision;
- license, data egress, or rights posture is unresolved;
- the same error class persists without a new hypothesis.

Stopping is a valid experimental result. It must not be hidden as a skipped
green check.

## Cost/speed/quality comparison shape

The final laboratory matrix must keep three axes separate:

| Axis | Measurements |
| --- | --- |
| quality | manually verified character/word/structure/translation/retrieval/claim errors, severity, source-return success |
| speed | wall time, throughput by page/segment/query, reviewer correction time |
| cost | peak resources, disk/cache growth, setup complexity, operator minutes, ongoing service burden |

No weighted total is defined in advance. A fast method that introduces
semantic drift may be rejected; a highly accurate method whose reviewer cost
cannot scale may remain a gold-production method rather than a corpus-wide
method.

## Current machine-fit decision

The host is sufficient for the first laboratory and for producing the first
gold packet. Reuse and measurement remain the default. The 2026-07-26
freshness gate admits resident Gemma 4 E2B, Gemma 4 E4B, and Qwen3 4B as the
next sequential model-proposal A/B/C set, but it does not admit execution
before the matching human/source packet exists. Qwen3 8B is escalation only;
Qwen3.5-4B, TranslateGemma, Qwen3.6, SYCL, and NPU generation remain closed
without a specific failure hypothesis.

The Qwen3.5-0.8B GEC exception does not reopen Qwen3.5-4B for translation. It
answered one source-free Russian-surface question with a different adapter,
scope, runtime, and preservation test, and it closed at a negative method
decision. Its local runtime and cache remain removable host artifacts; the
adapter's absent formal license and missing production trust evidence prohibit
publication, redistribution, and automatic consumer admission.

Granite R2 is the already earned bounded retrieval exception; the sequential
OCR gate has now earned and materialized all three isolated runtimes without
placing them in the repository or Fedora root. A is cheap and repeatable, B is
prohibitively slow and rejected in its unchanged configuration, and C is viable
at explicit `960/max` with a much larger memory footprint than A. None can be
called a winner without double-checked human transcription, correction-time
accounting, and source-visible human review. Visual embeddings and
service-heavy annotation tools likewise remain candidates whose admission
depends on new evidence from the smaller stack.
