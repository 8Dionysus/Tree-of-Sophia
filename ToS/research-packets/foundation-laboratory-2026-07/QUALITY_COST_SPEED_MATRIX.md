# Foundation Laboratory Quality / Cost / Speed Matrix

Status: mechanical measurements consolidated; human quality and correction
cost remain open
Snapshot: 2026-07-23

## Reading rule

This matrix keeps quality, speed, machine cost, storage cost, and human cost
separate. It does not normalize unlike tasks into one score and does not rank
a fast literal index against an OCR engine, translation interface, or graph
store.

`Not measured` is an evidence state. It must not be converted to zero. A
successful process, schema, model inspection, advisory expected hit, or
deterministic rebuild is mechanical evidence—not source-visible human quality.

## Consolidated matrix

| Family / variant | Executed scope | Quality evidence | Speed evidence | Machine / storage evidence | Human correction cost | Current decision |
| --- | --- | --- | --- | --- | --- | --- |
| native structure A, first | 36 units | real model-visible defect: XHTML head title added to body; human receipts 0 | 1.148 s | 32,759,808 peak RSS; 172,027 derived bytes | not measured | rejected revision; negative retained |
| native structure A, corrected | 36 units | all anchors resolved; 12 outline-PDF pages falsely empty; inherited OCR defects; human receipts 0 | 1.228 s | 32,632,832 peak RSS; 172,489 derived bytes | not measured | retain with limits; no accepted text |
| structure B / Docling hybrid R2 | 36 units; 12 per exact provenance-routed branch | six advisory inspections: 1 accept-with-limits, 5 reject; mechanical anchor resolution 1.0; structural quality not computable; human receipts 0 | 73.305 s runner; 29.466 units/min; 90.459 s owner wall | 1,582,149,632-byte child peak RSS; 3,250.324 MiB owner peak, zero swap; 1,267,021 artifact bytes | not measured | fast hybrid comparator; outputs unaccepted |
| structure C / PaddleOCR-VL full R1 | frozen 12-page visual subset | six advisory inspections: 5 accept-with-limits, 1 reject; 164 blocks, zero empty pages; structural F1/reading-order accuracy absent; human receipts 0 | 5,672.930 s runner; 0.1269 pages/min; 5,687.198 s owner wall; page predictions 22.523–834.815 s | 9,674,465,280-byte child peak RSS; owner peak 10,694.297 MiB RAM + 1,005.258 MiB swap; 515,122 artifact bytes | not measured | high-cost challenger awaiting human gold; no winner |
| OCR A / Tesseract R1 | 36 pages | six advisory inspections found substitutions, additions, marginal-number and reading-order defects; CER/WER absent | 40.15 s; 53.80 pages/min | about 126.7 MB child peak RSS | not measured | deterministic comparator only |
| OCR A / Tesseract R2 | 36 pages | recognition set identical to R1; repeatability is not accuracy | 31.74 s; 68.04 pages/min | about 126.6 MB child peak RSS | not measured | deterministic comparator only |
| OCR B / Kraken Party R1 | 27/36 pages; stopped | three advisory rejects and four accept-with-limits; omissions, duplication, invention, decoder-limit failure; no German conclusion | 3 h 50 m owner wall; 13 h 29 m CPU; 12,246.84 recognition s across completed pages | 3.7 GB memory peak; 809 MB swap peak | not measured | unchanged configuration rejected |
| OCR C / Paddle R1 | first page; OOM | no quality output | stopped during first page | 16.7 GB RAM plus 1.3 GB swap; omitted explicit resize | not measured | configuration rejected; negative retained |
| OCR C / dispatcher-invalidated R2 | intended one page, scope expanded; stopped after four bridge pages | outputs excluded from comparison | invalid scope | 3.6 GB RAM; 1.6 GB swap | not measured | runner-scope defect retained |
| OCR C / bounded R3-R4 | same one page twice | region/text output identical; no accuracy conclusion | 27.151 s and 26.106 s service wall | 3.7 GB and 3.6 GB RAM; zero swap | not measured | repeatability diagnostic only |
| OCR C / corrected R5 | 36 pages | six advisory accept-with-limits checks found line-number insertion, joined words, lost hyphens, punctuation/case loss, handwritten-note invention, substitution, and furniture inclusion; CER/WER absent | 605.56 s runner; 3.567 pages/min; 608.973 s service wall; 1,816.868 s CPU | 4 GB owner RAM peak; 136.8 MB swap; 1,029 regions | not measured | viable challenger awaiting human gold |
| retrieval A / SQLite FTS5 | 24 nonempty passages, 20 queries | model-proposed target top-10 17/19; literal OCR-noise and title retrieval; human judgments 0 | 0.04199 s build; 0.06026 ms warm; 0.36597 ms connection-cold | 200,704-byte database | not measured | exact-form fallback; not semantic/cross-language solution |
| retrieval B / Qwen3 + Qdrant + reranker | same passages and queries | advisory target top-10 18/19; cross-language recall plus target-language and scan-furniture failures; human judgments 0 | 2.561 s embedding + 0.611 s materialization; 66.27 ms warm dense; 2,424.38 ms warm reranked; 11,546.82 ms cold reranker route | 300,032-byte snapshot; shared resident memory not isolatable | not measured | serious candidate; no winner |
| retrieval C / Granite R2 CPU | same passages and queries; exact repeat | advisory target top-10 17/19; five rank-1 title/front pages; cross-language policy failures; rankings repeat exactly; human judgments 0 | 0.038 s model read + 0.646 s compile + 7.733 s passage embedding + 0.004 s write; 16.18 ms warm query embedding/rank; 19.213 s repeat runner | 663.5 MB owner peak, zero swap; 237,643-byte vector projection | not measured | independent comparator; no winner |
| graph A, first | 13 claims, 10 questions | field presence falsely called traceability | 0.000039 s build; 0.002685 ms median warm query | 42,169 packet bytes; 32,030,720 runner peak RSS | not measured | rejected revision; negative retained |
| graph A, corrected | same claim/query set | 10/10 advisory expectations and real owner-reference closure; claims still unreviewed | 0.023353 s build; 0.002290 ms median warm query | 79,827 packet bytes; 32,071,680 peak RSS | not measured | reference semantics only |
| graph B, first Neo4j | same claim/query set | literal status leaked into identity; 8/10 advisory expectations | 1.527820 s materialization; 4.549903 ms median warm query | 156,038 packet bytes; 32,301,056 runner peak RSS; shared service store not isolated | not measured | rejected revision; negative retained |
| graph B, corrected Neo4j | same claim/query set | 10/10 advisory expectations; human graph receipts 0 | 0.065963 s materialization; 2.826839 ms median warm query; 0.216974 s deletion; 0.058067 s rebuild | 156,132 packet bytes; 32,284,672 runner peak RSS | not measured | projection awaiting human review |
| graph C, first Oxigraph launch | no materialization | runtime-ownership check followed symlink into system Python | not run | launch receipt only | not measured | launch path rejected; negative retained |
| graph C, corrected Oxigraph | 13 claims, 10 questions, 537 quads | 10/10 advisory expectations; deterministic N-Quads; human graph receipts 0 | 0.074591 s build; 0.179564 ms median warm query; 0.000359 s deletion; 0.078919 s rebuild | 306,207-byte store; 470,377 packet bytes; 33,464,320 peak RSS | not measured | portable projection awaiting human review |
| translation v2 interface | 30 blind review units, 89 pages | exact interface closure; human passes and accepted German 0 | 40.706 s service wall | 142.961 MiB peak, zero swap; 45,034,156 bytes / 94 files; learned estimate 178.701 MiB | not measured | method prepared; all drafts blocked |
| translation A/B/C | no accepted source; no drafts | no translation-quality evidence | not measured | no model/runtime acquisition attributable to a translation run | not measured | not run; no winner |
| semantic annotation A/B/C | source-gated plan, zero tasks | no sign, relation, concept, or annotation-quality evidence | not measured | no experiment runtime | not measured | blocked-not-materialized |
| LLM assistance A/B/C | source-gated plan, zero tasks | no source-grounded accuracy, hallucination, or abstention result | not measured | no experiment runtime or challenger download | not measured | blocked-not-materialized |
| golden-kernel transfer A/B/C | three title-page scouts, zero semantic tasks | no benefit, harm, sign-reuse, or ontology-imposition evidence | not measured | no experiment runtime | not measured | blocked-not-run |

## What can be compared now

Only measurements inside a frozen family and method boundary are directly
comparable:

- OCR A R1/R2 supports deterministic recognition-set repeatability;
- OCR C R3/R4 supports one-page repeatability under the corrected bounded
  method;
- structure B has one corrected 36-unit run and structure C one corrected
  12-page run; they establish execution and resource profiles, not
  full-scope repeatability or an equal-scope winner;
- retrieval C's two runs support exact ranking/index reproducibility;
- corrected graph projections support lifecycle and trace-carriage comparison;
- first and corrected revisions expose specific implementation defects.

Even within a family, latency boundaries differ. Retrieval A is in-process,
B reuses resident services, and C starts a fresh OpenVINO process. Graph A has
no database, B reuses a shared service, and C owns an isolated disk store.
Their numbers must not be presented as a universal backend benchmark.

## Quality fields deliberately absent

The following remain absent rather than zero:

- OCR CER, WER, reading-order score, punctuation fidelity, and correction
  minutes;
- structure F1, text accuracy, and correction minutes;
- retrieval nDCG@10, hard-negative error rate, and human relevance;
- graph answer correctness and human trace-review minutes;
- translation fidelity on all 19 axes, comparator influence, and human effort;
- semantic annotation agreement, unsupported-promotion rate, and automation
  bias;
- LLM source-grounded accuracy, hallucinated-link rate, evidence completeness,
  and correction time;
- golden-kernel transfer accuracy, speed, correction, traceability,
  hallucinated relations, reusable-sign utility, and ontology imposition.

## Manual verification gates

The numbered list below records questions that require future human evidence,
not a standing backlog. The 2026-07-28 assurance schedule supersedes the
original v1 work implication without rewriting the historical packet: one
Russian calibration observation is preserved, fourteen rows are unscheduled,
same-human delayed recheck measures only intra-annotator stability, and German
textual claims remain language-competence-blocked.

Automatic and advisory values have not been recomputed against real-human
gold. If the corresponding research questions are opened, their evidence
requirements are:

1. two source-visible human passes on 15 OCR/structure gold candidates;
2. two source-visible human passes on 30 German source units;
3. 10 random and 10 hard blind human judgments for retrieval and graph;
4. real correction/adjudication timers rather than estimated labor;
5. manual recomputation of at least one aggregate metric from raw artifacts
   before trusting metric code;
6. 20 double-checked content-bearing target passages before semantic, LLM, or
   transfer evaluation.

Until a question-specific gate is closed, the corresponding metric remains
unmeasured. This does not create human debt for every prepared row. The
laboratory has mechanical speed and resource evidence plus real negative
failures, but no overall quality winner and no complete human-inclusive cost.
