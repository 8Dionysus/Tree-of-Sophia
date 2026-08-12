# Foundation Laboratory Decision Matrix

Status: research decision scaffold updated with bounded graph, retrieval, structure, and OCR A/B/C evidence; human decisions remain pending

Snapshot: 2026-07-23; local software/LLM admission refreshed 2026-07-26;
direct-visual retrieval and historical-German morphology routes refreshed
2026-07-30; broad software/model-stack currentness refreshed 2026-08-08;
specialized DE→RU MT setup evidence refreshed 2026-08-10/11 UTC; semantic
identity, translation-alignment identity, and source-text-unit contracts
refreshed 2026-08-11

The exact freshness decision and next experiment contract are recorded in
`LOCAL_LLM_ADMISSION.md` and the non-executing superseding currentness record
`SOFTWARE_MODEL_STACK_CURRENTNESS_REFRESH_2026-08-08.md`. The latter updates
watch and preflight facts without changing any exact historical result,
admitting a download, upgrade, run, winner, or human task.

## Decision vocabulary

| State | Meaning |
| --- | --- |
| `baseline` | must enter the first comparison; simple or already resident |
| `challenger` | enters the first comparison when dependencies are already available |
| `conditional` | enters only after a stated evidence, storage, license, or machine gate |
| `defer` | relevant later but not justified in the first bounded laboratory |
| `exclude` | conflicts with authority, rights, security, or machine boundaries |

Selection is provisional until manually reviewed A/B/C evidence exists.
Popularity, a release date, benchmark rank, and a green validator are not
promotion criteria.

## Cross-cutting gates

Every candidate must declare:

- exact version, license, source URL, artifact digest, and installation route;
- inputs and outputs, including whether source text leaves the machine;
- CPU/GPU/NPU/runtime path and peak memory/storage expectations;
- deterministic configuration or seed where possible;
- wall time, operator time, output bytes, and failure count;
- whether it can preserve page/segment coordinates and per-span provenance;
- rollback/removal route;
- manual sample and reviewer decision.

Network APIs are excluded from the first private-witness tests. They may be
evaluated later only with an explicit data-egress and rights decision.

## Source acquisition and forensics

| Candidate | Role | State | Why | Gate or risk |
| --- | --- | --- | --- | --- |
| `sha256sum`, `stat`, `file` | fixity and shallow type facts | baseline | resident and deterministic | `file` page counts are not authoritative |
| Poppler `pdfinfo`, `pdffonts`, `pdfimages`, `pdftotext`, `pdftoppm` | PDF-aware structure and render probes | baseline | exposed the real differences among both PDFs | record Poppler version; extraction is not validation |
| ZIP/EPUB container inspection | integrity, OPF, spine, navigation, resources | baseline | native and lossless for the EPUB container | XML/HTML parsing must remain untrusted-input safe |
| BagIt-style package manifests | future transfer inventory | conditional | useful for server handoff and preservation | implement after local item contract stabilizes |
| Apache Tika or broad magic extraction | generic metadata | defer | convenience does not beat format-specific first pass | extra dependency and shallow false confidence |

## OCR and document structure

| Candidate | Role | State | Machine fit | Principal risk | Intended comparison |
| --- | --- | --- | --- | --- | --- |
| existing ABBYY PDF text | sealed reference witness | baseline control | CPU, resident | hidden OCR may be badly wrong and would leak answers if inspected early | reveal only after independent A/B/C outputs are frozen |
| direct same-edition EPUB XHTML extraction | sealed reference witness | baseline control | CPU, resident | OCR residue, false page structure, and answer leakage | use for post-freeze diagnosis, never as an OCR contestant |
| Tesseract 5.5.2 + `deu`/`rus` 4.1.0 data | classical OCR | A executed twice; retained mechanical comparator | CPU; about 126.6-126.7 MB child peak RSS | page segmentation, Fraktur/old typography, punctuation, confidence miscalibration | A on the shared 36-page render manifest |
| Kraken 7.0.2 + Party v4 | trainable historical-document recognition | B stopped at 27/36; unchanged configuration rejected | Python 3.12 CPU; 3.7 GB RAM and 809 MB swap peaks | decoder saturation, omissions/repetition, segmentation sensitivity, mixed training provenance | preserved negative evidence on identical frozen PNG bytes |
| PaddleOCR 3.7 + PP-OCRv5 detector/language recognizers | independent multilingual pipeline | C exact one-page repeat plus one 36/36 run; awaits human review | Python 3.12 CPU; full run 4 GB RAM plus 136.8 MB swap peak | mixed-script substitutions, marginalia/furniture intrusion, reading-order defects | C on the identical frozen PNG bytes at explicit `960/max` |
| Docling standard pipeline | layout and reading-order recovery | challenger | CPU/GPU possible | output plausibility may hide wrong structure | A/B structure comparison |
| Docling hybrid with backend text forced | preserve native text while adding layout | challenger | CPU preferred | must be >=2.91 because of CVE-2026-44022 | 1996 PDF and EPUB-derived comparison |
| Docling VLM pipeline | hard-page structure/OCR proposal | conditional | Intel GPU, narrow pages only | hallucinated text and semantic drift | C on adversarial gold pages |
| PaddleOCR-VL 0.9B | compact VLM OCR/layout | conditional structure route | OpenVINO/Intel GPU feasibility first | historical-text hallucination and setup cost | hard-page structure test only, not OCR C |
| OCRmyPDF 17.10.0 | searchable-PDF packaging | conditional secondary route | CPU | derived PDF can be mistaken for source; watcher boundaries must separate code/input/output/archive | package only after an OCR route is retained |
| eScriptorium | trainable transcription/HTR and review | defer | service footprint not yet justified | operational/database ownership | use if printed OCR cannot reach gold threshold |
| OCR4all | historical OCR workflow | defer | CPU/service fit to be measured | training and workflow overhead | second-phase specialist comparison |
| LLM-only transcription | OCR replacement | exclude | technically runnable | plausible hallucination without visual fidelity | may only propose correction to anchored OCR |

### OCR A/B/C route

- `A`: Tesseract 5.5.2 with language-specific `deu` or `rus` 4.1.0 data,
  consuming the shared frozen page PNGs directly and emitting coordinates,
  confidence, and an untouched diplomatic draft.
- `B`: Kraken 7.0.2 baseline segmentation plus multilingual Party v4 at exact
  commit/model revision, over the same PNG bytes.
- `C`: PaddleOCR 3.7.0 with `PP-OCRv5_server_det` and the documented Latin or
  East-Slavic PP-OCRv5 mobile recognizer, over the same PNG bytes.

The 1893 German scan and both Russian PDFs contribute 12 full pages each. The
render specification and all 36 PNG digests were frozen once before any route
ran. Native/embedded text is neither A nor an input to A/B/C: ABBYY text and
the same-edition EPUB OCR remain sealed reference witnesses while independent
drafts and the human-gold lane are kept distinct. A has two complete frozen
output sets, B has one deliberately stopped and frozen negative set, and C has
an exact one-page repeat plus one complete frozen set. PP-OCRv6 lacks Russian
in the current release, and visual-language models are kept out of this
recognition comparison so that language coverage and hallucination are not
confounded with the primary OCR question.

## OCR correction and normalization

| Candidate | State | Allowed use | Promotion boundary |
| --- | --- | --- | --- |
| deterministic Unicode NFC | baseline | normalized layer only | never rewrite source bytes/diplomatic text |
| rule-based dehyphenation and furniture removal | baseline proposal | reversible span edits | manual review on gold sample |
| dictionary/language-model correction | challenger | suggestion with before/after span and score | accepted correction needs source-region review |
| local LLM correction | conditional | independent proposal with prompt/model digest | never overwrite OCR; preserve rejected proposal |
| silent cleanup before evaluation | exclude | none | destroys error and provenance evidence |

## Historical German morphology and lemmatization

`HISTORICAL_GERMAN_MORPHOLOGY_ADMISSION.md` owns the exact research,
artifact evidence, layer contract, and future experiment law. The shared
identity control makes no lexical claim. Its digest-bound pre-output state is
immutable; `HISTORICAL_GERMAN_MORPHOLOGY_B_ARTIFACT_ADMISSION_2026-08-10.md`
records the later private acquisition and denied runtime admission additively;
`HISTORICAL_GERMAN_MORPHOLOGY_B_EXECUTABLE_STOP_LINE_2026-08-12.md` records
the reviewed executable `agent` stop line without rewriting that event;
`HISTORICAL_GERMAN_MORPHOLOGY_B_EXECUTION_2026-08-12.md` records the later
admitted 42-wheel runtime and exact private B execution additively.

| Route | Exact candidate | State | First question | Principal boundary |
| --- | --- | --- | --- | --- |
| control | exact historical surface, unchanged | baseline | how much did a method change and was the change necessary? | no lemma or normalization claim |
| A | DWDSmor Open `0.18.0` | exhaustive source-gated census executed twice with byte-identical streams; retained private raw output and text-free aggregate receipt | which candidate analyses can an explainable FST recover directly? | 6,610 / 11,352 type coverage and 75,872 / 86,287 token-weighted coverage are mechanics, not accuracy; preserve all analyses and unknowns; no contextual winner |
| B | ZDL `de_zdl_lg 4.0.0` CPU | frozen three-context packet executed privately twice through the admitted 42-wheel runtime; byte-identical streams; retained text-free result receipt | does modern contextual analysis resolve ambiguity on unchanged historical text? | all three targets align to one token and receive an `ADV` proposal, but German-competent gold and accepted tokenization/morphology/lemma remain zero; local artifact integrity grants no redistribution or production authority |
| C | DTAEC type normalizer revision `bf13bc367ff08a91983eeb44002f48a2e713b28b` -> exact B | research-only conditional | does historical normalization improve B enough to justify information loss and cost? | weights/data derivative-rights ambiguity; every edit and original surface retained |
| DTA-CAB / RNNTagger | reference/watchlist | defer | later historical-system control if A/B/C expose a distinct gap | license, 3.6 GB package, and earlier-language-stage mismatch |
| LLM-only lemma | fluent-proposal watchlist | exclude first wave | none | missing/hallucinated tokens and persuasive forced answers |

Both A and the one frozen B episode have executed in isolated owner runtimes.
B preserves the earlier denied 41-wheel attempt, then binds the later
import-complete 42-wheel subject, five verified controls, exact `agent` allow
verdict, source-free smoke, two identical three-row streams, and measured
resource evidence. C remains unacquired and question-inapplicable to the
frozen B episode. No
German-competent gold, accepted morphology, lemma, `lexeme_id`, sign, semantic
claim, translation decision, or graph effect exists. Machine
coverage and agreement remain proposals, not linguistic acceptance.

## Text-unit and segmentation identity

| Candidate | Role | State | Why | Authority boundary |
| --- | --- | --- | --- | --- |
| source-observed layout and explicit line-break units | physical/layout floor | baseline | closest unit layer to the fixed representation and independently reconstructable | records layout, not a sentence, token, or linguistic intention |
| Unicode UAX #29 / ICU boundary candidates | deterministic orthographic proposal | baseline for a future concrete question | current, versioned, explainable, and language-tailorable | version and tailoring must be captured; a default boundary is not witness truth |
| language- and historical-text-tailored segmentation | linguistic challenger | conditional | may preserve abbreviations, historical orthography, verse, and editorial structure better than defaults | requires an exact task, declared competence, alternatives, and source-visible review before acceptance |
| tokenizer or model subword pieces | model-input projection | conditional projection only | needed to reproduce some model and embedding runs | never promotes to token, lexeme, sign, or semantic identity |
| interchange units such as CoNLL-U/TEI exports | transport projection | conditional | useful for downstream tools when generated from an owned packet | projection status and visibility cannot exceed their source unit packet |
| one forced canonical segmentation | universal owner | exclude | erases legitimate ambiguity and couples future work to one mutable method | no single algorithm, ordinal, tokenizer, or model may silently become source authority |

The additive `source-text-unit-packet-v1` is now the baseline owner contract.
Its public 42-byte/42-code-point A/B/C proves exact unit identity, coverage and
gap declaration, reciprocal alternatives, hierarchy closure, review and
projection gates, and deterministic reconstruction. The 25 rejected controls
and independent raw-byte inspection promote those mechanics only. No real
German/Russian boundary, linguistic unit, method winner, corpus migration, or
human task is admitted, and the existing `tos-local-sentence-segmentation-v1`
method remains unchanged.

## Alignment of editions and translations

| Candidate | Role | State | Why | Risk |
| --- | --- | --- | --- | --- |
| structural heading/paragraph alignment | coarse alignment | baseline | explainable and source-returnable | edition structure may differ |
| length/punctuation/number cues | deterministic feature baseline | baseline | cheap and interpretable | fails on paraphrase/reordering |
| Vecalign + multilingual embeddings | sentence proposal | challenger | established linear-time method | semantic similarity can hide non-equivalence |
| Qwen3-Embedding-0.6B similarity | local alignment feature | baseline model | already resident; German/Russian capable | ToS-domain quality unknown |
| adaptive/local alignment (Bertalign/AIlign family) | non-global monotonic challenger | conditional | may handle local omissions/reordering | dependency and reproducibility review first |
| LLM-generated segment mapping | explanatory challenger | conditional | may describe omissions and shifts | persuasive invented alignments |
| one-to-one forced alignment | exclude | none | erases omission, fusion, division, and reordering | architecturally false |

Alignment records must support `1:1`, `1:n`, `n:1`, `n:m`, `omitted`,
`added`, `reordered`, `uncertain`, and `rejected`.

The additive `translation-alignment-packet-v1` now makes that decision
executable without changing `translation-packet` v2. Both exact witness and
frozen text-layer sides, segmentation/tokenization artifacts, and ordered
anchors precede an opaque alignment plus claim identity. A and B use invented
private-use languages to validate one-to-one and reciprocal competing
one-to-one/one-to-many proposals. C and seventeen negative controls fail
closed on unreviewed acceptance, broken cardinality/reference closure,
competition, supersession, or visibility. Promote the contract mechanics with
limits; do not promote any real alignment, translation, lexical equivalence,
human evidence, export, graph, or canon object.

## Local LLM candidates

| Candidate | State | First role | License/storage posture | Stop condition |
| --- | --- | --- | --- | --- |
| Gemma 4 E2B GGUF | baseline A after content gate | low-cost source-grounded proposal | resident pinned community quantization, about 3.2 GB | cannot ground outputs on provided spans |
| Gemma 4 E4B GGUF | baseline B after content gate | stronger same-family proposal | resident pinned community quantization, about 6.7 GB | memory pressure or no manual quality gain |
| Qwen3 4B INT4 OpenVINO | challenger C after content gate | different family/runtime control | resident deployment model, about 2.3 GB plus existing cache | OpenVINO route cannot reproduce prompt/output/device receipt |
| Qwen3 8B INT4 OpenVINO | admitted successor only after source-free CPU smoke | larger different-family translation surface challenger outside historical A/B/C | exact official 4,882,866,534-byte 19-file artifact already local; Apache-2.0; pinned OVMS image; no model download | both Gemma candidates failed the Russian-surface floor and the 4B GPU route hung; use only an isolated network-disabled CPU container, preserve model/runtime/device confounding, and stop before human attention on visible Russian defects |
| Phi-3.5 mini INT4 | conditional control | older small control | resident | no unique evidence contribution |
| Qwen3.5-4B | watchlist / conditional download | fresh small multimodal challenger | Apache 2.0 model card; new conversion/runtime and reduced-context proof required | resident A/B/C has no reviewed failure taxonomy |
| TranslateGemma 4B | conditional, license- and source-gated | specialized MT challenger | gated license, absent artifact, 2K input context | German source unaccepted, DE/RU route unproved, or resident MT controls untested |
| MADLAD-400 3B q4 + Candle 0.11.0 CPU | exact specialized challenger; setup admitted and one private source candidate retained as uncertain | one-string DE→RU machine proposal after three general-model surface rejections | exact 1,671,227,060-byte Apache-2.0 artifact set from the Google namespace with disclosed community-conversion lineage; locked offline 9,501,904-byte runner; source-free smoke and one non-forced atomic owner run passed; zero deterministic surface findings, three bounded source-aware AI risks, human reviews 0 | retain for method comparison only; owner gate dominated wall time; keep recognized translations sealed; no German, fidelity, etymology, semantic, human-task, publication, model-winner, or canon authority |
| EuroLLM 1.7B | defer | compact European-language MT/control | new model artifact required | resident models fail and storage gate passes |
| NLLB | defer/restricted | research-only MT reference | non-commercial/research license posture | incompatible intended use or rights uncertainty |
| Qwen3.6 27B, 35B-A3B, and larger | exclude first wave | none | current but total weights are too large for the first justified local question | revisit only on different hardware or specific small-model failure evidence |

LLMs may propose transcriptions, structures, glosses, translations,
etymological leads, annotations, and relations. They never self-promote them.
The A/B/C identities in this table apply only to a model-proposal comparison;
they do not replace the separate human-only / AI-only / AI+human translation
lanes.

## Russian-surface triage candidates

| Route | State | Evidence | Decision |
| --- | --- | --- | --- |
| A / deterministic floor | retained asymmetric reject floor | nine invented controls and two digest-bound retrospective private receipts; about 7.707 ms/candidate without Hunspell | may reject only declared high-precision packet/surface failures; a clean result stays `uncertain` |
| B / pinned offline LanguageTool 6.9 | retained advisory challenger | 36 source-free controls, 13 observed rule IDs, nine declared defect misses, one ambiguous literary signal, 78.004763012 s cold-start comparison wall | independent rule evidence only; cannot change A disposition or admit content |
| C / Qwen3.5-0.8B SyntErr-to-LoRuGEC | executed and rejected as required/autonomous stage | all 36 actual source-free outputs read; 3/15 exact corrections, 12 missed or wrong, two clean literary false changes, one intentional-repetition change, 111.711071750 s wall and 3.5 GiB cgroup peak | retain negative quality/cost and one narrow B-complement; no private-candidate rerun, production use, publication, redistribution, automatic correction, or human task |
| D / independent checklist LLM | deferred | no frozen measured gap after C and no admitted artifact | open only for a new question whose expected benefit exceeds runtime and preservation risk |

## Translation methods

| Route | Method | State | Human visibility |
| --- | --- | --- | --- |
| A | independent human or literal/interlinear draft from verified German, recognized translation hidden | baseline | translator identity and source aids recorded |
| B | local LLM independent draft from the same verified German and context, recognized translation hidden | baseline | exact model/runtime/prompt recorded |
| C | AI+human collaborative revision, still before recognized comparator reveal | challenger | every accepted intervention attributed |
| comparator | Antоновsky and other recognized editions after drafts freeze | required | not scored as automatic truth |
| adjudication | independent reviewer with aligned original, drafts, comparator, and context | required | reasoned accept/reject/uncertain decision |

COMET or similar metrics may be added as diagnostics only after the manual
packet exists. A metric-only translation route is excluded.

## Embeddings

| Candidate | State | Role | Cost/fit | Risk |
| --- | --- | --- | --- | --- |
| Qwen3-Embedding-0.6B INT8 OpenVINO | baseline | dense DE/RU/EN passages and alignment | already resident, about 612 MB | no ToS-domain proof yet |
| Granite Embedding 311M Multilingual R2, revision `44399559930365213510b1ee2eb15ded83374f0e` | selected C, pre-output freeze | independent multilingual dense text challenger | Apache 2.0; official INT8 OpenVINO subset about 350 MB; 200+ languages with German/Russian emphasis; CPU route | no ToS-domain result yet; CLS/tokenization implementation must be proved exactly |
| gte-multilingual-base | retain comparator, not selected | compact 75-language dense reference | about 305M parameters but July 2025 repository state | documented integration enables remote model code; may add no gain over resident model |
| BGE-M3 | retain comparator, not selected | dense+sparse+multi-vector reference | roughly 2.27 GB model artifact | age, weight, and multiple retrieval modes would confound this single-method C run |
| Qwen3-VL-Embedding-2B, revision `9f2f7e710d6d81056aa5c0a4f04764fec6bb7bda` | selected challenger C in a separate visual experiment; audit-complete r9 executed without force; exact five-query advisory routing inspection complete | direct text-query-to-page-image retrieval without OCR text | exact 4,549,670,315-byte acquisition plus 1,182,279,019-byte isolated CPU runtime; 2,807.234 s owner wall; 4,808,843,264-byte memory and 380,157,952-byte swap peaks; Apache 2.0 model repository | 20/20 stable rankings and 200 resolved anchors are mechanical; four inspected rank-1 language/witness-policy failures reject unfiltered default use, while one image-only A/B-missed recovery retains an opt-in post-text-coverage fallback; human relevance, promotion, production/public adoption, and winner absent |
| generic English-only embedding | exclude | none | poor source-language fit | would structurally privilege translation |

## Retrieval and indexing

| Candidate | Route | State | Authority posture | Evaluation focus |
| --- | --- | --- | --- | --- |
| exact ID/anchor lookup | identity | baseline | direct tracked corpus | perfect source return |
| source-gated DTA whole-work SQLite/FTS5 plus tracked hash/resource companion | lexical observation floor | executed baseline | exact sequential source remains ignored; tracked rows are non-sequential, string-free, local-only, and public-blocked | exact/normalized/prefix/phrase and page/division return mechanics; no source, lemma, sign, or relevance-quality acceptance |
| complete page-bounded exact KWIC over one frozen Work-identity control | usage-context method control | executed baseline | all 527 exact contexts remain ignored mode-0600 local evidence; tracked plan/receipt/provenance contain only hashes, counts, selectors, rights posture, and explicit non-authority | deterministic context identity, full occurrence and selector closure, clipping behavior, source return, and byte/cost baseline; no sentence, sense, sign, public route, or human task |
| SQLite FTS5/BM25 or equivalent local lexical index | lexical A | baseline | rebuildable | names, compounds, citations, rare terms |
| Qwen3 dense index | semantic B | baseline | rebuildable | implicit parallels and cross-language queries |
| Granite R2 dense index | independent multilingual C | selected, method frozen before output | rebuildable isolated local projection | independent-family cross-language behavior on the identical query set |
| lexical+dense rank fusion | later derived experiment | conditional | rebuildable but not an independent A/B/C method | recall gain can only be tested after component results are preserved |
| Qdrant 1.19.0 | vector-store implementation | conditional | projection only | filters, reproducibility, consistency, quantization, and operational cost |
| Qwen3-VL direct page-image retrieval | visual challenger C in separate experiment | r9 executed audit-complete; r8 retained audit-incomplete; exact five-query advisory routing inspection complete | projection only | retain only as an opt-in post-text-coverage source-page fallback with predeclared item/witness/language/content-class constraints; reject the current unfiltered route as default; no human relevance or winner |
| LLM answer without retrieval trace | exclude | none | no source-return path | unacceptable for corpus truth |

The gold query set must contain exact lexical queries, paraphrases,
cross-language questions, implicit conceptual relations, hard distractors,
and questions with no relevant answer.

## Annotation interfaces

| Candidate | State | Strength | Limitation | ToS boundary |
| --- | --- | --- | --- | --- |
| tracked human-readable claim/review packets | baseline | diffable, durable, owner-visible | less ergonomic | authoritative record |
| small local packet editor generated by lab | baseline implementation target | exact contract and source-region view | must not invent a second schema | writes/export packets only |
| INCEpTION 41.x | challenger | spans, relations, agreement, recommenders, curation | service/database/version migration | export plus receipt; not canon owner |
| CATMA 7 | challenger | qualitative/literary, exploratory tags, TEI export | project representation and service workflow | export plus receipt; not canon owner |
| direct annotation inside graph database | exclude | visually tempting | makes projection hidden authority | graph remains downstream |

INCEpTION and CATMA should be compared on the same tiny annotation packet,
not installed permanently before workflow value is demonstrated.

## Claims, provenance, and validation

| Candidate | State | Role | Boundary |
| --- | --- | --- | --- |
| JSON/JSON-LD explicit claim resources | baseline | tracked canonical packet shape | human-readable fields remain primary |
| `source-text-unit-packet-v1` | baseline contract | exact frozen-text unit identity, explicit coverage/gaps, competing segmentation, review, and projection boundary | public synthetic A/B/C proves mechanics only; model pieces stay non-linguistic and no real boundary or bulk migration is admitted |
| `semantic-annotation-packet-v2` | baseline contract | stand-off occurrence→lexeme/sense→sign→concept entities plus first-class competing claims, human reviews, typed relations, and source-returnable graph admission | public synthetic A/B/C proves mechanics only; no bulk migration or accepted semantic object |
| PROV-O profile | baseline projection | entity/activity/agent provenance | local terms must be documented |
| PREMIS-inspired event/rights fields | baseline profile | preservation and fixity history | not full repository certification |
| JSON Schema | baseline | mechanics/shape validation | no semantic truth claim |
| SHACL 1.0 | challenger | graph-projection shape validation | stable version only |
| SHACL 1.2 draft-only features | defer | none in authority contract | standard not stable at snapshot |
| RDF-star quoted triples as sole claim storage | exclude | none | semantics/version risk and poor review ergonomics |

## Graph storage and visualization

| Candidate | Route | State | Strength | Risk |
| --- | --- | --- | --- | --- |
| tracked explicit claims + generated JSON graph | authority/source | baseline | transparent, portable, diffable | custom projection code |
| tracked explicit claims queried directly | graph A | baseline, executed | zero-database reference semantics; transparent and diffable | custom bounded query code; no multi-user service |
| Neo4j Community 5.26.26 LTS (resident; current upstream 2026.06) | graph B | challenger, executed | mature property graph; claim-first typed relationships; existing service | shared-store cost is not isolatable; community authorization and resident-service burden; no currentness-driven migration |
| PyOxigraph 0.5.9 | graph C | challenger, executed | compact RDF/SPARQL; claim-specific named graphs; canonical N-Quads; exact isolated Store bytes | active development; tiny pilot does not prove large-query maturity |
| Qdrant 1.19.0 | retrieval adjunct | conditional | vector filtering/search | not a knowledge graph; release recency is not adoption evidence |
| any graph DB as only copy of claims | exclude | none | deletes provenance and review authority | prohibited |

The graph comparison asks whether each projection makes source-return,
contradiction, lineage, ambiguity, and rejected alternatives more inspectable;
raw traversal speed alone does not win.

## Discovery and access

| Candidate | State | Use | Rights boundary |
| --- | --- | --- | --- |
| DNB SRU and authority records | baseline | bibliographic identity | metadata terms still recorded |
| DNB SPARQL beta | challenger | linked discovery | beta behavior versioned |
| KVK | baseline lead source | locate originating catalogs | never cite KVK as owning record |
| Nietzsche Source eKGWB | local research reference plus publisher-authentication/permission target | critical text, stable addresses, private non-commercial indexing and analysis | CC BY-NC-ND 4.0 permits private adaptations but prohibits sharing them; Arquivo.pt provides WARC-backed historical byte corroboration while explicitly not validating source origin; the current owner route remains HTTP-only, and ToS authorizes neither source admission nor payload publication |
| Nietzsche-Wörterbuch | access-request target | lexical/sense evidence | subscription/copyright restricted |
| Europeana | conditional | item discovery | content and metadata rights separate |
| Google Books / Internet Archive / HathiTrust | conditional | digital-item leads | repository availability is not permission |
| Wikisource dumps / Gutenberg mirrors | conditional | reproducible acquisition where licensed | jurisdiction and item license checked |
| scraping restricted viewer content | exclude | none | violates access and provenance boundary |

## First-wave experiment set

The first wave reuses resident models and runtimes where they answer the
question. The OCR sequential gate was applied: Tesseract A was isolated and
retained first, its evidence admitted Kraken/Party B, and B's preserved stop
and unchanged-method rejection admitted PaddleOCR C. Every setup records its
license, exact source and artifact digests, storage delta, runtime receipt, and
removal route. Execution does not satisfy the still-human promotion rule.

| Experiment | A | B | C |
| --- | --- | --- | --- |
| OCR | Tesseract 5.5.2 on shared render | Kraken 7.0.2 + Party v4 on same bytes | PaddleOCR 3.7 + multilingual PP-OCRv5 on same bytes |
| structure recovery | deterministic page/heading rules | Docling standard/hybrid | one selected VLM route on hard pages |
| correction | no correction | reversible rules | local LLM proposal |
| alignment | structure/length cues | Vecalign + local embedding | LLM explanation/proposal |
| translation | independent human/interlinear | resident E2B/E4B draft | AI+human revision |
| semantic annotation | manual packet | INCEpTION export | CATMA export |
| retrieval | lexical SQLite FTS5 | resident Qwen dense + Qdrant + reranker | independent Granite R2 multilingual dense |
| graph | canonical JSON/JSONL claim records | resident Neo4j claim-first projection | PyOxigraph explicit-claim/named-graph projection |

No experiment may change its sample after seeing results. Any necessary
change creates a new run and receipt.

## Promotion rule

A candidate advances only if:

1. its exact run is reproducible from a receipt;
2. manual review finds a meaningful improvement on declared error classes;
3. source-return and provenance remain intact;
4. resource and operator cost are proportionate;
5. license and rights posture are compatible;
6. failures, rejected outputs, and unresolved cases remain visible;
7. the gain survives a second manual check on a held-back sample.

Otherwise it is deferred or rejected even if its aggregate metric is higher.
