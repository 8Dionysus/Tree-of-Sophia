# Retrieval Comparison Report: Foundation Pilot v1

Status: text variants A/B/C executed; separate direct-visual challenger
prepared but resource-blocked before execution; real human relevance review
not started
Experiment: `tos-retrieval-foundation-v1`
Frozen query set: 20 queries, sealed before variant output
Indexed sample: 36 source-addressable units, of which 24 contain extracted text
Execution date: 2026-07-22
Direct-visual admission snapshot: 2026-07-29

## Result first

All three variants are useful for different jobs. None is eligible for
promotion, and there is no quality winner before human grading.

- SQLite FTS5 remains the exact-form fallback: extremely fast, small,
  explainable, and strongest on literal text including inherited OCR errors.
- The resident Qwen3 embedding route establishes multilingual candidate
  recall, and the Qwen3 reranker materially improves candidate ordering.
- The independent Granite R2 CPU route is compact, exactly revisioned, fully
  local at inference time, and mechanically reproducible across two separate
  runs. Its 20 ranked-anchor lists and 237,643-byte vector index are identical
  between runs.
- A separate direct-visual experiment reuses the same 20 query bytes and all
  36 digest-frozen page PNGs. Its exact Qwen3-VL-Embedding-2B source revision,
  29-wheel CPU closure, 18-file model tree, isolated offline runtime, visual
  input-use audit, network audit, source-to-image crosswalk, and immutable
  text-control receipts are prepared. It is not a fourth result in the
  comparison below.
- The latest durable direct-visual preflight is `blocked`, not `ready`.
  Available memory was 11,545,407,488 bytes against the frozen
  17,179,869,184-byte minimum, and the host resource owner returned
  `force_required`. Storage, load, temperature, exact plan, runtime, license,
  CPU, and local-only network posture all passed. No force was used, the
  threshold was not weakened, and no run root, page embedding, index, ranking,
  quality metric, or winner exists.
- On a common advisory denominator of 19 non-coverage queries, A returns the
  model-proposed target in the first ten for 17 queries, B for 18, and C for
  17. This is not a quality score: the target declarations are model-proposed
  and no human relevance judgments exist.
- B finds both cross-language target anchors in its first ten, while A fails
  both. B nevertheless ranks the same-language translation parallel declared
  as a hard negative above the intended target in both directions. Hit@10
  therefore conceals a real task-policy failure.
- B misses the exact corrupted phrase `gTösste Frevel` because its source page
  does not enter the dense top ten. The reranker cannot recover a passage it
  never receives; A retrieves that literal corrupted form at rank 1.
- C also misses `gTösste Frevel`, and additionally misses the proposed exact
  Russian target for `великий полдень`. Five of C's rank-1 results are title or
  front-matter pages. This is a direct warning against treating embedding
  similarity as content classification or exact-form retrieval.
- C finds both cross-language target anchors, at ranks 3 and 5, but in both
  directions ranks the declared same-language hard negative first. Like B,
  the raw embedding lane discovers translation relation without obeying the
  intended target-language policy.
- The one-line Google/Cornell scan-furniture passage at Naumann page 000 is
  dense rank 1 for 13 of 20 B queries. Reranking reduces that to one query, but
  the contamination must still be typed or filtered rather than trusted as
  content.
- The Antonovsky coverage query remains impossible for both variants because
  all 12 sampled pages are empty in the upstream native extraction.

No nDCG@10, hard-negative error rate, or human correction cost is reported.
Those fields remain `null`. All successful run packets remain
`awaiting-manual-review`; no derived index is canonical.

The direct-visual non-run adds no row to the comparable A/B/C measurements.
Its one-time acquisition and runtime preparation cost is recorded separately
below; it supplies no retrieval speed, memory peak, relevance, or coverage
result.

## Comparable measurements

| Evidence | A: SQLite FTS5 | B: resident semantic + rerank | C: Granite R2 CPU |
| --- | ---: | ---: | ---: |
| frozen queries | 20 | 20 | 20 |
| source-addressable units | 36 | 36 | 36 |
| nonempty / empty units | 24 / 12 | 24 / 12 | 24 / 12 |
| first materialization | 0.04199 s | 2.561 s embedding + 0.611 s materialization | 0.038 s model read + 0.646 s compile + 7.733 s passage embedding + 0.004 s write |
| exact rebuild | 0.04199 s including text indexing | 0.189 s reusing captured vectors | 7.789 s repeated embedding + 0.004 s write |
| derived index measurement | 200,704-byte SQLite database | 300,032-byte Qdrant snapshot | 237,643-byte canonical JSON vector projection |
| warm retrieval latency | 0.06026 ms | 66.27 ms dense | 16.18 ms query embedding + exact ranking median |
| warm reranked end-to-end latency | not applicable | 2,424.38 ms | not applicable |
| cold / whole-run measurement | 0.36597 ms connection-cold | 11,546.82 ms genuine cold reranker route | 19.213 s whole repeat runner; 19.400 s owner service |
| isolated owner memory peak | not measured here | shared resident services, not exactly isolatable | 663.5 MB, swap 0 |
| model-proposed target in top 10, common 19-query denominator | 17 / 19 | 18 / 19 | 17 / 19 |
| model-proposed target at rank 1 | 17 / 19 | 14 / 19 after reranking | 8 / 19 |
| source-anchor resolution | 20 / 20 returned rows | 200 / 200 dense and 200 / 200 reranked rows | 200 / 200 ranked rows |
| real human relevance receipts | 0 | 0 | 0 |
| advisory model inspection receipts | 8 | 5 | 5 |

The latency rows are not equivalent service-level benchmarks. A is an
in-process literal lookup. B includes localhost HTTP, query embedding, exact
Qdrant vector scan, and optionally GPU reranking. C is a fresh process that
reads and compiles an INT8 OpenVINO model, then performs exact cosine ranking
over a run-local JSON projection. At only 24 points neither B nor C measures
corpus-scale ANN behavior. C's bridge self-reported 1,085,730,816 peak bytes,
which conflicts with the stronger owner cgroup peak of 663.5 MB; this
instrumentation discrepancy is retained rather than averaged away.

## Preserved runs

Variant A:

`/srv/abyss-machine/storage/artifacts/tree-of-sophia-foundation-lab/tos-retrieval-foundation-v1/foundation-pilot-v1-retrieval-a-20260722/variant-A`

Variant B:

`/srv/abyss-machine/storage/artifacts/tree-of-sophia-foundation-lab/tos-retrieval-foundation-v1/foundation-pilot-v1-retrieval-b-resident-20260722/variant-B`

Variant C first run:

`/srv/abyss-machine/storage/artifacts/tree-of-sophia-foundation-lab/tos-retrieval-foundation-v1/foundation-pilot-v1-retrieval-c-granite-r2-20260722/variant-C`

Variant C owner-receipted repeat:

`/srv/abyss-machine/storage/artifacts/tree-of-sophia-foundation-lab/tos-retrieval-foundation-v1/foundation-pilot-v1-retrieval-c-granite-r2-repeat-20260722/variant-C`

Variant B used the isolated collection
`tos_foundation_retrieval_b_20260722_facf7810`. The runner proved that the
collection was absent, materialized 24 points, deleted it, proved absence,
rebuilt the same 24 points, measured and deleted a temporary snapshot, and
retained only the rebuilt collection for manual review. Existing Qdrant
collections were not used or mutated.

The first C run completed and verifies, but the host's latest-only resource
receipt was displaced before capture. Its journal facts and the gap are
recorded without reconstructing a receipt. The second C run used identical
inputs and method, captured the complete owner receipt immediately, and
reproduced all 20 ranked-anchor lists plus the exact index digest. Both local
indexes remain isolated under their run roots.

## Direct visual challenger: exact route prepared, no execution

`tos-visual-retrieval-foundation-v1` is a distinct output-blind experiment.
It does not rename or supersede Granite as text-retrieval variant C. Its own
challenger C asks a different question: can a Russian or German text query
retrieve the relevant source page directly from its image when extracted or
OCR text is absent, corrupt, or contaminated?

The source-owned plan was frozen before model acquisition or challenger output:

- plan SHA-256
  `27e142883d4385b8b69857ab7ebcbee7facb16454e54649ef6ed1df15e3b7c25`;
- 20 unchanged private query strings and 36 unchanged page PNGs;
- render-manifest SHA-256
  `734e7474923ec2697a243b1e1b322c43c4e2a8b82b6336df57ab82f3bbc8fec7`;
- immutable text-control receipt SHA-256 values
  `d6d13bae304ff72afa126143921865415740b2342cb49d0c70e252de51e691fa`
  for A and
  `b879670826b2b5f512bceac56f3cc0b48eaeec72e4c7e375f72cc40c9901b3db`
  for B;
- `Qwen/Qwen3-VL-Embedding-2B` revision
  `9f2f7e710d6d81056aa5c0a4f04764fec6bb7bda`, 2,127,532,032 parameters,
  Apache-2.0, selected 2,048-dimensional output.

The host-owned setup then closed the exact local execution route without
placing model or runtime bytes in Tree of Sophia:

- frozen acquisition: 4,549,670,315 bytes, comprising 4,271,068,726 model
  bytes and 278,601,589 bytes across 29 CPU wheels;
- acquisition receipt SHA-256
  `c93062e4cd6395b30ab057be184b80afd41a1632203424b528fec14f63c43c73`;
- isolated runtime: 1,182,279,019 bytes and 27,587 inventoried artifacts;
- runtime-manifest SHA-256
  `ff730fe5780216fe84f26b7fbed6c57d0f0de58cabac5026d6af403e094632ba`;
- runtime artifact-set SHA-256
  `837f70def92d6808d975f1d2c4871aec24a184443307266531909105a23ebf38`.

Early setup failures remain evidence. The first runtime materialization
exposed an undeclared `torchvision` import dependency before model load. The
corrected closure makes that dependency explicit. A later code audit exposed
that the reviewed upstream helper can catch an image-processing exception and
fall back to a `NULL` text input. The admitted bridge now fails closed unless
all 36 requests demonstrably consume exactly one decoded page image. It also
records and requires zero network attempts. These controls prove execution
integrity only; they are not model-quality evidence.

The latest durable preflight is stored by the host owner as:

`/srv/abyss-machine/storage/artifacts/tree-of-sophia-foundation-lab/preflights/tos-visual-retrieval-foundation-v1-c-20260729t160500z.json`

Its SHA-256 is
`299cdf3ad98d17c4f3b98b2080eb21e20525c5bae869573b22633c7674195e16`.
At `2026-07-29T16:05:13.585670+00:00` it established:

| Admission fact | Observed | Frozen requirement / decision |
| --- | ---: | --- |
| source plan | exact digest admitted | pass |
| isolated runtime | exact manifest and artifact set admitted | pass |
| `/srv` free bytes | 51,723,489,280 | at least 21,474,836,480; pass |
| storage-owner request | 6,697,153,963 bytes | `allow` |
| load 1m | 5.7431640625 | at most 8; pass |
| temperature | 87 °C | below 105 °C watch boundary; pass |
| available memory | 11,545,407,488 bytes | at least 17,179,869,184; **block** |
| heavy/indexing owner admission | `force_required` | `allow` required; **block** |

The owner projected that admitting the additional heavy/indexing startup
demand alongside existing reservations would breach the host reserve. That is
the reason for the non-run. Temperature and storage are not the reason, and
the receipt supplies no model failure or retrieval result. The frozen
pre-output plan intentionally remains unchanged: mutating it after setup would
break the digest-bound comparison. A later attempt requires a fresh
`decision=ready` preflight under the same 16 GiB memory floor; it must not use
force merely to complete the matrix.

Even a successful future run can establish only page-image candidate
retrieval. A returned image is not a transcription, quotation, source-text
acceptance, semantic judgment, or philosophical claim. Human review remains
triggered only if adoption, a material route change, unresolved ambiguity, or
explicit drift control makes that decision necessary.

## Variant A: what remains proven

SQLite FTS5 indexes exact and normalized extracted text with source anchors.
Its query behavior remains narrow and explicit:

| Query class | Observed A behavior | Boundary |
| --- | --- | --- |
| exact Russian and German quotation | proposed target at rank 1 | requires exact words in the same language |
| lexical sign conjunction | proposed same-language page at rank 1 | no sign identity, sense, or cross-language alignment |
| OCR-noise | corrupted source form at rank 1 | indexes inherited corruption rather than repairing it |
| Russian to German | Russian parallel at rank 1 | cross-language target absent |
| German to Russian | German parallel at rank 1 | cross-language target absent |
| Antonovsky coverage | zero results | upstream native-text omission propagates into retrieval |
| other Nietzsche works | literal title pages at rank 1 | no work-membership or relation reasoning |

The `lemma`, canonical `phrase`, `section`, and `sign_candidate` fields remain
empty. A conjunction of words is not evidence of a stable sign.

## Variant B: live route and provenance

The run exercised the deployed localhost path rather than an invented
in-process approximation:

```text
passage text -> langchain-api -> OVMS Qwen3 embedding -> isolated Qdrant
query -> rag-api -> OVMS query embedding -> Qdrant -> rerank-api on GPU
```

Pinned live evidence includes:

- `langchain-api` image ID `efa6cecbb3ae...` and live source SHA-256
  `3af17a156c588291e348adf88c7f5dd605f76078f962ea4301102be20e8d5fe8`;
- `rag-api` image ID `aa142cafb78f...` and live source SHA-256
  `58f888c16bd46216defb11703a6af9e66951a06dbd5257cb1b7bb3b8366077e1`;
- `rerank-api` image ID `a40d363dcf89...` and live source SHA-256
  `1afd2b76342cb68a19dd786040c259a1f826c8442feaafe48ba7da01af2974f4`;
- Qdrant image digest
  `sha256:45f8e3ddc2570a4d029877e1b5ec1045c19b3852b4e22a55c7f43b05aea0ca89`;
- OVMS image digest
  `sha256:fdc34725b35d79cde23cff1967f90efb40237b7cf5545a70460209056f3da733`;
- embedding weight SHA-256
  `e5fa45cb772afbdfb360905241d24d11b62f80f2ab33a3b884d10a08e21224c4`;
- reranker source revision `d7dd424f08cbafe70657ca391c619820a99c1ca9`
  and weight SHA-256
  `c64cf6aca738b553f85914522a05fd9898d1f54acc26953a6737dfd2f0dc53f3`;
- runner SHA-256
  `facf781068dc601f42c3604ee985c918a3642100fac8713eb7ee329ac45624f1`.

The embedding copy has no recoverable upstream revision metadata. Its config,
XML, binary digests, and live OVMS config are fixed, but this is weaker than a
repository revision plus artifact digest and must be repaired before any
promotion decision.

## Variant C: independent runtime and provenance

C was selected and frozen before installation or ranked output as IBM
`ibm-granite/granite-embedding-311m-multilingual-r2`, repository revision
`44399559930365213510b1ee2eb15ded83374f0e`. It uses the official INT8
OpenVINO export on CPU, the same encoder for queries and passages, CLS pooling,
L2 normalization, and exact cosine ranking. It is not a hybrid with A or B.

Pinned evidence includes:

- 11 selected upstream files totaling 348,082,051 bytes, rather than the full
  repository;
- model license Apache-2.0 and no remote-code execution;
- OpenVINO `2026.2.0-21903-52ddc073857-releases/2026/2`;
- isolated Tokenizers 0.23.1 wheel SHA-256
  `5075b405006415ea148a992d093699c66eb01952bf59f4d5727089a98bda45a4`;
- runtime-manifest SHA-256
  `2a3da5f7795877bb76464d0614753d9bff40c751e40c3b4b2fde1665c329cb55`;
- runner SHA-256
  `2cfb31e58a3aaef5d61424d168b0640de014a65c0ba068c1ce7074c24d1eff06`;
- bridge SHA-256
  `586ac4fb7715c56f1ec1006776a3866103b5652a871eb9ff2d39da640398d826`;
- query-plan SHA-256
  `481e1f8ea32b9f7b40eb73a59e7bc3e614d08a3b8958bebd7b0c7fce94fed7a4`;
- private query-content SHA-256
  `5c9ce96fb362512a9ef775bd7d7212b5df6b00570226961a96d51478ba980a1e`;
- owner-repeat receipt SHA-256
  `24f0fdb536393da1bb39a17fff71b6120ef28d3f7acd7c5bbfd2280e243eb106`.

The first runtime smoke failed because OpenVINO 2026.2 rejects the generic
`AFFINITY` property suggested by the owner route. That failed receipt remains
preserved. The corrected runtime sets only supported inference-thread control;
CPU placement remains owned by the `abyss-machine` systemd route. System
Python and project environments were not mutated.

The repeat run proved two different reproducibility levels:

- delete/rebuild within a run produced the same vector-index digest;
- a separate owner-routed run produced the same index SHA-256
  `90fb3aaa0d9e0005af2c12e37b28d7379143901d50a184293967b5b4733eb41c`
  and the same all-query ranked-anchor projection SHA-256
  `ecdf14d993e99066be0fefdf051bdbe745911369c125219a36e9aa6c34784b7a`.

This proves deterministic mechanics for the frozen small packet, not semantic
correctness.

Observed C behavior is deliberately kept separate from relevance truth:

| Query class | Observed C behavior | Boundary |
| --- | --- | --- |
| exact quotation | most proposed targets present, but `tos-query-003` absent | embeddings are not a literal index |
| OCR-noise | both inspected noisy cases rank poorly; `gTösste Frevel` target absent | semantic normalization does not guarantee OCR-error tolerance |
| Russian to German | proposed German target rank 3, Russian hard negative rank 1 | multilingual relation found; target-language intent violated |
| German to Russian | proposed Russian target rank 5, German hard negative rank 1 | same policy failure in reverse |
| phrase with translation parallel | German original rank 1, requested Russian target rank 3 | potentially useful parallel discovery, but not compliant routing |
| content class | five rank-1 results are title/front-matter pages | no learned or explicit content-class control |
| Antonovsky coverage | no source passage can be indexed | upstream native-text omission still propagates |

## What reranking changed

Across queries with a proposed target already present in dense top ten,
reranking moved ten targets upward, left seven at the same rank, and moved one
downward. Proposed target-at-rank-1 changed from 5 to 14 queries.

Representative changes:

- `tos-query-003`, Russian `великий полдень`: rank 9 -> rank 1;
- `tos-query-004`, German exact quotation: rank 5 -> rank 1;
- `tos-query-007`, Russian OCR-noise case: rank 7 -> rank 1;
- `tos-query-012`, motif query: rank 10 -> rank 1;
- `tos-query-017`, German exact quotation: rank 4 -> rank 1;
- `tos-query-010`, intended Russian cross-language target: rank 3 -> rank 4.

This proves that the reranker changes ordering and often suppresses obvious
scan furniture. It does not prove philosophical relevance, correct target
language, safe thresholding, or hard-negative correctness.

## Source-visible non-human inspections

Five bounded B inspections and five bounded C inspections were recorded with
maker `model:codex`, authority `advisory-nonhuman`, and
`promotion_authorized: false`. They are manual source-visible checks performed
by the acting model, not human relevance labels.

For B:

- `tos-query-003`: rerank recovery of the proposed target;
- `tos-query-008`: exact OCR-noise omission and scan-furniture top result;
- `tos-query-009`: Russian-to-German target/parallel ordering conflict;
- `tos-query-010`: German-to-Russian target/parallel ordering conflict;
- `tos-query-019`: German original above the intended Russian target.

For C:

- `tos-query-003`: proposed Russian exact-quote target absent; title/front
  matter dominates;
- `tos-query-008`: the source-visible noisy occurrence is absent; unrelated
  front matter and scan furniture dominate;
- `tos-query-009`: intended German target rank 3, declared Russian hard
  negative rank 1;
- `tos-query-010`: intended Russian target rank 5, declared German hard
  negative rank 1;
- `tos-query-019`: intended Russian phrase target rank 3, German original rank
  1, unrelated Russian passage rank 2.

The cross-language inspections expose a shared contract issue: the frozen plan
records intended target language, but B's deployed `/retrieve` request and C's
unfiltered cosine route do not enforce it. Both systems can discover
multilingual relation without reliably obeying a language-intent policy.
Adding an unreviewed filter after seeing these outputs would invalidate the
frozen runs; it belongs in a separately declared method revision.

## Decisions

1. Retain A as the explainable exact-form fallback, including for known OCR
   strings, and reject it as the semantic or cross-language solution.
2. Retain B as a serious multilingual/reranking candidate, not as a winner.
3. Retain C as an exactly revisioned, reproducible, independent multilingual
   CPU comparator, not as a winner. Reject it as a replacement for literal
   retrieval or content-class control.
4. Require retrieval to preserve both relevance and task policy: target
   language, witness/work scope, content class, and source anchor must be
   explicit rather than inferred from rank score.
5. Add a candidate-generation depth experiment because reranking cannot
   recover `tos-query-008` from a dense top-10 omission.
6. Add scan-furniture/content-class evidence before embedding or as a declared
   filter; do not silently delete inherited contamination from source records.
7. Keep human labels, hard-negative decisions, nDCG, and error rates absent
   until the blind source-visible protocol occurs.
8. Repair upstream revision provenance for B's embedding artifact; C's exact
   revision does not repair B by proxy.
9. Treat lexical + semantic candidate union, language filtering, and content
   filtering as future declared variants. Do not retroactively call the
   observed A/B/C outputs a hybrid system.
10. Keep the isolated B collection and C run-local indexes only through manual
    review, then delete them
    unless an explicit retention decision says otherwise.
11. Preserve the C runtime/cache only while it supports the remaining frozen
    laboratory; its versioned removal route is already recorded.
12. Retain the direct-visual route as an admitted but unexecuted challenger.
    Do not force the host resource owner, lower the frozen 16 GiB floor, or
    infer a model result from setup/preflight evidence.

## Comparison still required

A genuine quality conclusion still requires:

- 10 random and 10 hard source-visible human judgments under blind labels;
- explicit adjudication of translation parallels versus hard negatives;
- manually recomputed nDCG@10 and hard-negative error rate for at least one
  query before trusting aggregate code;
- separate indexing, query, machine, storage, and human-time costs;
- candidate-depth and target-language-policy trials declared before output;
- a corpus-scale Qdrant run large enough to exercise its vector index;
- one owner-admitted offline direct-visual run, only after a fresh preflight
  returns `ready`, followed by triggered source-visible review if its output
  is actually considered for adoption or changes a source-return route;
- final deletion or retention receipts for all derived indexes.

Until those conditions hold, the defensible conclusion is narrower: lexical
and semantic retrieval are complementary foundation instruments. B adds
multilingual recall and strong reranking; C adds a compact, independently
revisioned and reproducible multilingual CPU comparison. Exact OCR noise,
source contamination, target-language policy, B provenance completeness, and
human relevance remain unresolved. No variant wins.
