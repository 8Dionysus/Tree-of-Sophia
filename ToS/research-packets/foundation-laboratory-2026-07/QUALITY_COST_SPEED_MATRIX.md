# Foundation Laboratory Quality / Cost / Speed Matrix

Status: mechanical measurements consolidated and raw morphology/OCR aggregates
independently recomputed; human quality and correction cost remain open
Snapshot: 2026-08-10

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
| whole-work DTA lexical observation | four fixed TEIs; 86,287 occurrences; 506 body pages; 213 divisions | exact/normalized/prefix/phrase/section/page/language/edition probes pass; deterministic byte-identical rebuild; three form aggregates and 35 raw XML locators manually reconciled; no accepted German, lemma, sign, or retrieval relevance evidence | three source-identical rebuild-check observations: 5.50 s, 7.71 s, and 15.35 s wall; 4.47–7.00 s user plus 1.00–2.36 s system; host contention not controlled, so no cold/warm claim | 273,988–274,784 KiB observed max RSS; 119,533,568-byte private SQLite/FTS5 database; 12,919,723-byte tracked hash/count/resource projection | not measured; no human task opened | promote mechanical index foundation with rights/source/linguistic/semantic limits |
| historical-German morphology A / DWDSmor Open 0.18.0 | all 11,352 exact DTA form types representing 86,287 occurrences, analyzed unchanged in two passes | 6,610 types and 75,872 token weight receive at least one provider analysis; 4,742 types / 10,415 weight are mechanically unknown; both streams byte-identical; Tree recorder independently recomputes raw denominators, distributions, and residue shape; German gold, accuracy, accepted morphology, lemma, and lexeme remain 0 | pass 1 wall/CPU 3.1373/3.1226 s; pass 2 3.1117/3.0983 s; 3,618.39 forms/s pass 1; host service 7.069 s wall / 6.961 s CPU | seven-wheel cache closure; 4,803,531-byte principal wheel; 23,451,336-byte/1,043-artifact runtime; 4,213,466-byte private input; 54,495,917-byte private raw output; process peak RSS 44,339,200 bytes; host cgroup peak 118,292,480 bytes, swap 0 | not measured; no human task opened | promote exhaustive A mechanics and retained text-free receipt only; coverage is not accuracy and mechanical residue does not open B/C |
| historical-German contextual morphology B | one exact-form A ambiguity; output-blind recurrence ranks 1/73/145; two raw-TEI paragraphs and one verse group across parts 1/3/4; exact private contexts frozen before B output | packet and exact-return mechanics proven; exact artifact acquisition proven; B output, German gold, contextual POS quality, accepted morphology, lemma, and lexeme remain absent because runtime admission was denied before build or input consumption | acquisition 356.746 s under the owner `medium/indexing` plan; B execution not measured | 5,203-byte ignored mode-0600 context packet; 627,548,130-byte principal wheel plus 40 dependency wheels acquired for CPython 3.12; acquisition peak reported 2.1G RAM and 0B swap; ABI/SBOM/ML-BOM/SLSA sidecars present, Cosign absent, verified controls zero, candidate not latest/release/subject-store complete | acquisition quality/cost separated from nonexistent model execution; no human task opened | retain the exact negative admission receipt; do not build or run until license, signature, trust-root, lifecycle, verification, and subject closure pass without bypass |
| historical-German contextual morphology C | DTAEC-normalized exact-B method retained in the general method profile | question-inapplicable for this episode because exact and normalized selected-form hashes are identical; no packet or output | not measured | 31,741,808-byte weights identified but unacquired | not measured; no human task opened | keep blocked for a distinct normalization question; do not run a third method merely to complete an A/B/C shape |
| retrieval A / SQLite FTS5 | 24 nonempty passages, 20 queries | model-proposed target top-10 17/19; literal OCR-noise and title retrieval; human judgments 0 | 0.04199 s build; 0.06026 ms warm; 0.36597 ms connection-cold | 200,704-byte database | not measured | exact-form fallback; not semantic/cross-language solution |
| retrieval B / Qwen3 + Qdrant + reranker | same passages and queries | advisory target top-10 18/19; cross-language recall plus target-language and scan-furniture failures; human judgments 0 | 2.561 s embedding + 0.611 s materialization; 66.27 ms warm dense; 2,424.38 ms warm reranked; 11,546.82 ms cold reranker route | 300,032-byte snapshot; shared resident memory not isolatable | not measured | serious candidate; no winner |
| retrieval C / Granite R2 CPU | same passages and queries; exact repeat | advisory target top-10 17/19; five rank-1 title/front pages; cross-language policy failures; rankings repeat exactly; human judgments 0 | 0.038 s model read + 0.646 s compile + 7.733 s passage embedding + 0.004 s write; 16.18 ms warm query embedding/rank; 19.213 s repeat runner | 663.5 MB owner peak, zero swap; 237,643-byte vector projection | not measured | independent comparator; no winner |
| direct visual retrieval C / Qwen3-VL-Embedding-2B CPU | frozen 20 text queries × 36 page images; one audit-complete r9 run plus preserved audit-incomplete r8 | 20/20 repeat-stable rankings; 200/200 anchors resolve; advisory target top-10 19/19 evaluable; one route recovered where A/B miss; hard negatives present 9/10 and outrank proposed expected in 4 queries; cross-language proposed targets 2/2 but hard negatives outrank both; human judgments 0 | 1.042 s model load; 2,507.904 s page encoding; 6,800.709 ms first and 6,955.081 ms warm median query; 2,806.762 s runner / 2,807.234 s owner wall | 4,742,041,600-byte bridge peak RSS; 4,808,843,264-byte owner memory peak + 380,157,952-byte swap peak; 1,588,701-byte vector index; one-time 4,549,670,315-byte acquisition + 1,182,279,019-byte runtime; 76 persisted normalization records; r8/r9 rankings and scores identical 20/20 | not measured; narrow five-query criteria-only review open but unscheduled; human debt 0 | retain r9 mechanical and trigger evidence plus r8 negative; no relevance verdict, adoption, promotion, or winner |
| graph A, first | 13 claims, 10 questions | field presence falsely called traceability | 0.000039 s build; 0.002685 ms median warm query | 42,169 packet bytes; 32,030,720 runner peak RSS | not measured | rejected revision; negative retained |
| graph A, corrected | same claim/query set | 10/10 advisory expectations and real owner-reference closure; claims still unreviewed | 0.023353 s build; 0.002290 ms median warm query | 79,827 packet bytes; 32,071,680 peak RSS | not measured | reference semantics only |
| graph B, first Neo4j | same claim/query set | literal status leaked into identity; 8/10 advisory expectations | 1.527820 s materialization; 4.549903 ms median warm query | 156,038 packet bytes; 32,301,056 runner peak RSS; shared service store not isolated | not measured | rejected revision; negative retained |
| graph B, corrected Neo4j | same claim/query set | 10/10 advisory expectations; human graph receipts 0 | 0.065963 s materialization; 2.826839 ms median warm query; 0.216974 s deletion; 0.058067 s rebuild | 156,132 packet bytes; 32,284,672 runner peak RSS | not measured | projection awaiting human review |
| graph C, first Oxigraph launch | no materialization | runtime-ownership check followed symlink into system Python | not run | launch receipt only | not measured | launch path rejected; negative retained |
| graph C, corrected Oxigraph | 13 claims, 10 questions, 537 quads | 10/10 advisory expectations; deterministic N-Quads; human graph receipts 0 | 0.074591 s build; 0.179564 ms median warm query; 0.000359 s deletion; 0.078919 s rebuild | 306,207-byte store; 470,377 packet bytes; 33,464,320 peak RSS | not measured | portable projection awaiting human review |
| translation v2 interface | 30 blind review units, 89 pages | exact interface closure; human passes and accepted German 0 | 40.706 s service wall | 142.961 MiB peak, zero swap; 45,034,156 bytes / 94 files; learned estimate 178.701 MiB | not measured | method prepared; all drafts blocked |
| bounded translation A / Gemma E2B direct | one calibration-only 20-token source string; no accepted source or draft | schema-valid machine candidate; direct AI inspection finds obvious Russian grammar defects; German fidelity unjudged | 146.973579 s; 5.007703 completion tokens/s; 542 prompt + 736 completion tokens | resident 3,184,494,720-byte exact GGUF and existing llama.cpp route; no acquisition | not measured | frozen private method evidence; no translation-quality claim |
| bounded translation B / Gemma E2B source-aware | same exact source/model/runtime and decoding; prompt family changed | incomplete outer JSON at the 768-token ceiling; visible Russian repeats A defect; parser defect preserved and repaired without rerun | 169.745670 s; 4.524416 completion tokens/s; 574 prompt + 768 completion tokens | same resident artifact/runtime as A; no acquisition | not measured | frozen invalid-model-output evidence; no winner |
| bounded translation C / Qwen3 4B OVMS | same source-aware prompt/decoding; one exact request | i915 `GPU HANG` and context reset preceded OVMS `CL_OUT_OF_RESOURCES`; no HTTP response, completion count, candidate, or language-quality evidence | 35.420039 s to failure; kernel hang at +17.331189 s; throughput unavailable | existing 2,290,768,181-byte immutable 18-file payload and exact OVMS image; 8 GiB cold start and 2 GiB warm request admitted without force; cache last reported 2.6%; container exit 139, `OOMKilled=false`; contained compute runtime 26.09 versus host 26.22 is an unresolved mismatch, not a cause | not measured | frozen failed-and-retained runtime evidence; exact route blocked for translation-sized work; no rerun, tuning, or winner |
| translation successor E4B / first infrastructure episode | same admitted source and direct prompt; separate single-candidate capacity route, not historical D | no candidate and therefore no language-quality evidence; strict grammar repetition and missing Jinja mode identified from private runtime evidence | 50.736652 s owner wall; 21.856117 s model wall; 22.040 s service runtime | exact resident 6,656,152,736-byte E4B GGUF; 11,060,039,680-byte memory peak + 144,154,624-byte swap peak; 101→102 °C observed sensor max; no acquisition | not incurred | retain infrastructure failure; bounded correction allowed only because no candidate existed |
| translation successor E4B / corrected direct candidate | same admitted source, direct prompt, deterministic decoding, Jinja, relaxed structural generation plus full post-validation | schema valid and source echo exact; AI Russian-surface triage found obvious defects and rejected before human review; German fidelity unjudged | 288.737642 s owner wall; 256.656317 s model wall; 544 prompt tokens at 20.20 tokens/s; 542 completion evaluation runs at 2.61 runs/s | same resident E4B artifact/runtime; 11,505,102,848-byte memory peak + 166,227,968-byte swap peak; 101→102 °C observed sensor max; no acquisition | not incurred | candidate rejected; method retained; no recognized comparator, rerun, human task, or winner |
| translation successor Qwen3 8B INT4 / OVMS CPU | same admitted source and sealed comparator; separate family/capacity/runtime/device episode, not historical D and not a clean capacity comparison | source-free smoke passed; schema-valid private candidate and exact source echo; AI Russian-surface triage found obvious defects and rejected before human review; German fidelity unjudged | 206.546902 s owner wall; 177.709070 s model wall; 496 prompt + 860 completion tokens | exact resident 4,882,866,534-byte 19-file OpenVINO tree and pinned OVMS image; isolated CPU-only six-thread network-disabled container; no GPU or host port; model-process peak memory unmeasured because the owner wrapper did not contain the rootless model process | not incurred | candidate rejected; CPU execution and cleanup retained; no 4B GPU repair, family/capacity winner, rerun, human task, or quality claim |
| specialized MT / MADLAD-400 3B q4 + Candle CPU | exact three-file model set, locked offline runtime, invented source-free smoke, and one admitted private Nietzsche sentence; comparator sealed | private candidate schema-valid and fluent enough to avoid configured surface rejection; deterministic floor findings 0 => `uncertain`; bounded AI reading flags compressed source repetition, normalized historical surface, and smoothed syntax/cadence; German fidelity and human review remain unjudged | complete private call 979.291729 s; pre-worker owner gate 901.152729 s; atomic launch 78.139 s; worker 71.241677 s; model load 2.651449428 s; generation 61.390189241 s for 36 source and 33 generated tokens | 1,671,227,060 model bytes; 9,667,291 runtime bytes; private-run owner peak 3,330,088,960 bytes RAM + 667,648 bytes swap; exact atomic resource/storage `allow`, 10,485,760 requested output bytes, no force; setup and preflight negatives retained | zero; no human task opened | retain exact setup, private candidate, uncertainty, risks, and measured gate cost for method comparison; no winner, accepted translation, German, etymology, semantic, graph, publication, or canon claim |
| deterministic Russian-surface floor | nine invented controls plus retrospective source-blind screening of the two frozen successor candidates | invented controls matched exactly: five high-precision rejects, two advisory uncertain cases, two clean uncertain cases; both private candidates rejected for exact role collapse; E4B gained one advisory age-pattern finding; at least the independently observed Qwen name-case defect was not detected | 900 deterministic control evaluations in 6.936292 s, about 7.707 ms/candidate; nine evaluations with required resident Hunspell in 2.601451 s, about 289.050 ms/candidate; local in-process microbenchmark, not production | existing Python only for deterministic mode; existing `/usr/bin/hunspell` and resident `ru_RU` dictionary for optional advisory mode; no model, network, download, or new runtime | zero; no human task opened | promote asymmetric rejection floor with limits; a finding-free result remains `uncertain`, never review-worthy or accepted; the separately executed LanguageTool B is advisory-only and any GEC/LLM C remains distinct work |
| Russian-surface B / pinned offline LanguageTool 6.9 snapshot | 36 invented source-free controls: 12 acceptable, 20 defect, 4 ambiguous literary; no private candidate rerun | all frozen per-control expectations reproduced with zero disagreements; 13 rule IDs observed; 9 defect controls had no LanguageTool signal, including all 5 A-owned packet/surface boundaries, 3 declared grammar/punctuation misses, and 1 unbalanced-quote boundary; 1 ambiguous lowercase literary fragment was marked; B changed no A disposition | 78.004763012 s comparison wall; individual isolated cold starts 1.528671645--2.553019610 s, 2.054580681 s median, 74.734542889 s sum | 600,440 KiB peak child RSS; 259,264,698-byte archive; 414,841,492-byte / 1,812-file runtime; one new user+network namespace per control; local digests present, but no upstream checksum/signature, SBOM, SLSA/in-toto, Sigstore/Cosign, or durable registry record; trust gate `unknown` / fail-closed | zero; no human task opened | retain B as advisory-only independent rule signal above A; not release-admitted, not a reject/promotion authority, no aggregate score or winner; completed C remains a separate quality-negative generative method |
| Russian-surface C / Qwen3.5-0.8B SyntErr-to-LoRuGEC BF16 CPU | same 36 invented source-free controls; 15 explicit correction, 12 preservation, and 9 observed-only boundary/ambiguity targets; no private candidate rerun | all actual outputs manually read: 3/15 exact corrections, 12 missed or wrong corrections, 10/12 exact preservations, 2 false changes to clean literary em-dash controls, and 1 change to intentional repetition; only visible complement to B was the subject-comma control; pass-through of meta, non-Russian, and corrupt observations; no protocol wrapper; C changed no A/B disposition | 111.711071750 s comparison wall; 1.231214469 s model load; 101.711699502 s summed shared-batch generation wall across 9 batches | 3,810,525,184-byte cgroup memory peak and zero process swap; 1,213,808,640-byte isolated runtime; 1,770,037,248-byte base cache and 25,616,384-byte deliberately partial adapter cache; post-run 63 °C; base Apache-2.0, adapter formal license absent; ABI, SBOM, ML-BOM, SLSA/in-toto, Sigstore/Cosign, and durable registry evidence absent | zero; no human task opened; all-row review was AI manual source-free inspection, not human gold | retain fixity, actual-output, manual-inspection, resource, and negative-quality evidence; reject this profile as default gate, autonomous corrector, mandatory stage, production artifact, redistribution candidate, aggregate-score winner, or reason to spend operator attention |
| semantic annotation A/B/C | source-gated plan, zero tasks | no sign, relation, concept, or annotation-quality evidence | not measured | no experiment runtime | not measured | blocked-not-materialized |
| LLM assistance A/B/C | source-gated plan, zero tasks | no source-grounded accuracy, hallucination, or abstention result | not measured | no experiment runtime or challenger download | not measured | blocked-not-materialized |
| golden-kernel transfer A/B/C | three title-page scouts plus twenty private pre-output page candidates (10 digest-random, 10 mechanically hard); zero eligible semantic tasks | source-visible model check confirms content-bearing pages only; no target gold, benefit, harm, sign-reuse, or ontology-imposition evidence | preparation extraction measured only; experiment speed not measured | 93,282 local extracted bytes plus tracked metadata/anchors; no experiment runtime | human debt 0; correction not measured | candidate preparation reproducible; blocked-not-run |

## What can be compared now

Only measurements inside a frozen family and method boundary are directly
comparable:

- OCR A R1/R2 supports deterministic recognition-set repeatability;
- OCR A R2 now also has an independent one-off raw-artifact recomputation:
  144 output digests/sizes, 36 text counts, 36 TSV diagnostics, sample/source
  counts, empty-output count, mean page confidence, and pages/minute matched
  the frozen report without invoking the runner or repository validators;
- OCR C R3/R4 supports one-page repeatability under the corrected bounded
  method;
- structure B has one corrected 36-unit run and structure C one corrected
  12-page run; they establish execution and resource profiles, not
  full-scope repeatability or an equal-scope winner;
- retrieval C's two runs support exact ranking/index reproducibility;
- direct visual retrieval has one audit-complete execution and one preserved
  audit-incomplete execution with identical 20/20 rankings and scores; this
  supports internal repeat behavior and direct-page-image cost measurement,
  but the task and corpus differ from the text variants and human relevance
  remains absent;
- the full-work DTA lexical observation index proves a different source-layer
  capability from retrieval A: exact local navigation over four structured
  witnesses. It has no shared quality labels and is not a fourth retrieval
  variant or evidence that the small-corpus rank comparison generalizes;
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
- direct visual human relevance, nDCG@10, hard-negative error rate, and human
  correction time; machine ranking, advisory coverage, latency, and resource
  peaks are measured but are not quality labels;
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

The A R2 mechanical aggregate has been independently recomputed from raw
artifacts. Automatic quality values still cannot be recomputed against
real-human gold because no accepted gold exists. If the corresponding research
questions are opened, their remaining evidence requirements are:

1. two source-visible human passes on 15 OCR/structure gold candidates;
2. two source-visible human passes on 30 German source units;
3. 10 random and 10 hard blind human judgments for a text retrieval or graph
   adoption comparison; the direct-visual challenger currently opens only its
   five declared trigger queries, not a broad standing batch;
4. real correction/adjudication timers rather than estimated labor;
5. repeat the independent raw-artifact method for the exact content-quality
   metric once accepted gold exists;
6. 20 double-checked content-bearing target passages before transfer
   evaluation; the twenty prepared private pages are only a trigger-driven
   sampling frame and do not satisfy this gate.

Until a question-specific gate is closed, the corresponding metric remains
unmeasured. This does not create human debt for every prepared row. The
laboratory has mechanical speed and resource evidence plus real negative
failures, but no overall quality winner and no complete human-inclusive cost.
