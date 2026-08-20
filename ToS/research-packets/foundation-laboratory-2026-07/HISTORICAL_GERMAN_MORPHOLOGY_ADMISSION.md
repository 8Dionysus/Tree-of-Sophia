# Historical German Morphology and Lemmatization Admission

Status: ordered research, source-gated A census executed, one output-blind
three-context B packet materialized but B not acquired or executed; C remains
unacquired and question-inapplicable; no German linguistic analysis accepted

Research snapshot: 2026-08-10

## Decision

Admit a future, source-gated comparison with one shared identity control and
three deliberately different routes:

1. **identity control** — retain the exact historical surface and make no
   lexical claim;
2. **A — DWDSmor Open 0.18.0** — deterministic finite-state candidate
   generation directly from the historical surface;
3. **B — ZDL `de_zdl_lg` 4.0.0** — contextual modern-German
   POS/morphology/lemma proposal directly from the same surface;
4. **C — DTAEC type normalizer plus the B route** — a research-only
   normalization-assisted proposal, with the unmodified surface and every
   normalization edit retained.

The routes are admitted as methods to test, not future winners. The exhaustive
A census now has an exact `abyss-stack` experiment profile, acquisition and
runtime receipts, a current host preflight, a frozen source-local input packet,
and one retained private run. Those receipts admit no contextual B/C run.
Every later run still requires its own exact packet, rights gate, artifact
closure, and fresh host admission.

No route may create an accepted lemma, `lexeme_id`, sign, concept, translation
decision, or source correction. German linguistic acceptance remains blocked
because the present solo+AI operator has not claimed competence to judge
German orthography or grammar. Agreement among A/B/C is machine
triangulation, not independent gold.

## Question and boundary

The whole-work lexical index proves that ToS can return from an exact form to
an exact DTA witness location. It does not yet answer:

- which inflected occurrences may belong to one lexeme;
- whether an old spelling should be normalized at all;
- which lemma is intended in an ambiguous context;
- which POS and morphosyntactic features apply to the occurrence;
- whether a compound should be decomposed;
- whether an unusual form is historical spelling, authorial style, a coined
  word, an editorial intervention, or an extraction error.

This packet researches the next linguistic layer only. It does not reopen
source acceptance, perform translation, infer etymology, generate signs, or
promote semantic claims.

## Research method

The research was conducted in the required order:

1. standards, official guidelines, official corpus documentation, and
   originating software/model surfaces;
2. established comparative studies and field-defining methods;
3. current releases, exact package/model metadata, and the freshest relevant
   primary work discoverable through 2026-07-29.

The freshness pass searched ACL Anthology and arXiv for 2025–2026 historical
German lemmatization, morphology, POS tagging, and normalization; checked
official GitHub, PyPI, Hugging Face, Zenodo, Universal Dependencies, LMU,
BBAW/DTA, and project pages; and read exact release/package metadata through
their public APIs. It found no 2026 historical-German morphology or
lemmatization system that supersedes the candidates below. The current 2026
historical-German work returned by the search concerns other tasks such as
historical relation extraction and OCR correction. That negative result is
recorded rather than padded with adjacent work.

No package, model, corpus, or runtime was downloaded or installed during this
research.

## 1. Classical and official foundation

### 1.1 Universal Dependencies is an interchange layer, not ToS truth

Universal Dependencies (UD) separates morphology into lemma, POS, and
features. Its own guidance says that lemma choice follows language-specific
convention, may collapse orthographic variation, and may even normalize
authorial style aggressively in some treebanks. UD also distinguishes tokens
from syntactic words and permits a persistent lemma identifier in `MISC/LId`
rather than polluting the lemma string.

Consequences for ToS:

- UD `UPOS` and features are useful portable projections;
- German `XPOS` should remain available beside the UD mapping;
- a provider's `LEMMA` is a versioned proposal with provider and convention;
- historical variation must not be labeled `Typo=Yes` merely because it
  differs from current spelling;
- fused and split forms require an explicit tokenization/word-segmentation
  assertion before morphology;
- ToS identity cannot be derived from a lemma string.

Sources:

- [UD morphology principles](https://universaldependencies.org/u/overview/morphology.html)
- [CoNLL-U format](https://universaldependencies.org/format.html)
- [UD German HDT](https://universaldependencies.org/treebanks/de_hdt/index.html)
- [UD German GSD](https://universaldependencies.org/treebanks/de_gsd/index.html)

The current UD 2.18 pages expose an important quality distinction. HDT's
lemmas, UPOS, and features were manually annotated in a non-UD style and then
converted; the conversion reports 97% accuracy on a manually converted
subset. Its source is nevertheless 1996–2001 `heise.de` news/nonfiction/web,
not nineteenth-century literary philosophy. GSD's lemmas and features were
assigned by a program and not manually checked. Neither is Nietzsche gold.
HDT is the more defensible modern comparator and training reference.

### 1.2 STTS and HiTS preserve German-specific distinctions

The Stuttgart-Tübingen Tagset (STTS) remains the conventional German XPOS
inventory. HiTS extends the STTS family for historical German and, more
importantly for ToS, distinguishes the lexical category associated with the
lemma from the category of the concrete occurrence. Its treatment of
tokenization, univerbation, multi-word material, and historical variation
shows why a single `lemma + POS` pair is too narrow.

HiTS was developed for much earlier historical stages than Nietzsche's
1883–1885 text. ToS should adopt its **separation principle**, not force its
entire tag inventory onto late nineteenth-century German.

Sources:

- [STTS documentation](https://hinrichs.sfs.uni-tuebingen.de/projects/Elwis/stts/stts.html)
- [HiTS: ein Tagset für historische Sprachstufen des Deutschen](https://jlcl.org/article/view/170/168)

### 1.3 Normalization and modernization are different assertions

The Anselm guidelines explicitly separate a normalization layer from a
modernization layer. This is the decisive model for ToS:

- a conservative normalization may regularize a historical representation
  while remaining close to its language state;
- a modernization maps it toward current orthography or usage;
- neither operation replaces the diplomatic surface;
- the output of either operation is not automatically the lemma.

Source:

- [Anselm normalization guidelines](https://www.linguistics.ruhr-uni-bochum.de/comphist/projects/anselm/normalization.html)

### 1.4 DTA/DTABf supplies source structure, not accepted analysis

The Deutsches Textarchiv (DTA) and its DTABf TEI P5 profile provide the
strongest existing structured historical witness route in this laboratory.
The originating project documents automatic sentence segmentation,
tokenization, lemmatization, modernization, and POS processing plus DTAQ
quality controls. Those layers are valuable comparison evidence. They do not
become ToS-accepted German or accepted lemmatization merely because the DTA
created them.

Sources:

- [DTABf introduction](https://deutschestextarchiv.github.io/dtabf/einfuehrung.html)
- [DTA project record](https://gepris.dfg.de/gepris/projekt/37149321?language=en)
- [DTA project results](https://gepris.dfg.de/gepris/projekt/37149321/ergebnisse)
- [BBAW CLARIN corpus services](https://clarin.bbaw.de/en/corpus/)

### 1.5 Official historical processing routes

Two established originating tools remain relevant but do not lead the first
trial:

- **DTA-CAB 1.115**, dated 2021-03-04 on MetaCPAN, performs historical
  canonicalization and linguistic analysis. Its current MetaCPAN metadata
  reports the license as unknown. It remains a service/reference lead until
  exact endpoint behavior, license, and response provenance are frozen.
- **RNNTagger 1.5.0** supplies POS and lemma models for more than fifty modern
  and historical languages. Its official package is 3.6 GB and is freely
  available for research, education, and evaluation; other use requires a
  separate license. The historical German models target Early New High German
  and Middle High German, not late nineteenth-century German.

Sources:

- [DTA-CAB distribution](https://metacpan.org/dist/DTA-CAB)
- [DTA-CAB Text+ registry record](https://registry.text-plus.org/admin/entity/68686b1fdfe34a43998c5ce5/)
- [RNNTagger official page](https://www.cis.uni-muenchen.de/~schmid/tools/RNNTagger/)
- [RNNTagger historical morphology paper](https://www.cis.uni-muenchen.de/~schmid/papers/Datech2019.pdf)

## 2. Established comparative evidence

### 2.1 Normalization has no universal winner

Bollmann's large-scale eight-language comparison found no single historical
normalization method that dominates across datasets, training sizes, and
evaluation choices. Earlier rule-based work and later few-/zero-shot and
delayed-reward systems reinforce the need for an identity baseline, explicit
data regimes, and corpus-local evaluation.

Sources:

- [A Large-Scale Comparison of Historical Text Normalization Systems](https://aclanthology.org/N19-1389/)
- [Rule-Based Normalization of Historical Texts](https://aclanthology.org/W11-4106/)
- [Few-Shot and Zero-Shot Learning for Historical Text Normalization](https://aclanthology.org/D19-6112/)
- [Historical Text Normalization with Delayed Rewards](https://aclanthology.org/P19-1157/)

For ToS, identity is not a weak placeholder. It is the control that measures
how often a method changes something that should have remained historically
visible.

### 2.2 Domain shift must be measured directly

Eger, Gleim, and Mehler compare German and Latin morphology/lemmatization
in-domain and out-of-domain and include processing/training cost rather than
accuracy alone. The result is directly applicable: modern news-treebank
scores cannot be carried over to Nietzsche.

Source:

- [Lemmatization and Morphological Tagging in German and Latin](https://aclanthology.org/L16-1239/)

### 2.3 Direct historical models still expose gold and unseen-form limits

Schmid's RNNTagger study reports direct historical spelling experiments on
GerManC and ReM rather than normalizing first. It also records that manual
inspection found errors in the supposed gold data itself. GATEtoGerManC
reported 89.44% POS and 83.16% lemma accuracy on unseen Early Modern German
and found improvement from normalization.

These values describe different centuries, genres, tagsets, and gold. They
justify preserving disagreement and auditing reference quality; they are not
expected ToS scores.

Sources:

- [Deep Learning-Based Morphological Taggers and Lemmatizers for Annotating Historical Texts](https://www.cis.uni-muenchen.de/~schmid/papers/Datech2019.pdf)
- [GATEtoGerManC](https://aclanthology.org/L12-1584/)
- [Learning from Within?](https://aclanthology.org/L16-1684/)

## 3. Freshest relevant evidence and exact current artifacts

### 3.1 DWDSmor Open 0.18.0

DWDSmor is an explainable finite-state morphology system for written German.
It can return multiple analyses, lemmas, morphosyntactic categories,
paradigms, and root/word-formation analyses rather than silently choosing one
answer.

Exact current open artifact evidence:

| Field | Frozen research value |
| --- | --- |
| release | `v0.18.0`, published 2026-04-28 |
| source commit | `f97b92ce2a5d6db8750afbdb222eb39470e57cf6` |
| Python | `>=3.12` |
| PyPI wheel | 4,803,531 bytes |
| wheel SHA-256 | `395a15e15286b0c191b42355b6e3c2a43c8959621ccf3563336c2e30399a2973` |
| license | GPL-2.0 for code, grammar, sample lexicon, and Open automata |
| latest located HF model-card revision | `bb9f775bb8d29b51cac45ea3a5a49b22beca236e`, last modified 2025-08-01; identity with the later PyPI 0.18.0 wheel is not proven |
| HF snapshot's self-reported HDT coverage | 0.8577 overall; 0.7508 NN; 0.0632 NE; context only until recomputed from the acquired A artifact |

The older HF snapshot's low proper-name and reduced open-class coverage are a
useful diagnostic, not a defect to hide or a metric to assign to the later
wheel. Nietzschean compounds, coinages, foreign forms, names, and pre-reform
spellings are expected hard cases. The future A receipt must inspect the wheel
and measure its own coverage rather than assume that the two release surfaces
are byte- or result-identical.

The full DWDS Edition reports typical 95–100% coverage for most word classes
but is all-rights-reserved and available only on request for research. It is a
future access-request route, not the present A artifact.

Sources:

- [DWDSmor repository](https://github.com/zentrum-lexikographie/DWDSmor)
- [DWDSmor 0.18.0 on PyPI](https://pypi.org/project/dwdsmor/0.18.0/)
- [DWDSmor Open model card](https://huggingface.co/zentrum-lexikographie/dwdsmor-open)

### 3.2 ZDL spaCy models 4.0.0

The ZDL project trains German tagger, morphologizer, lemmatizer, parser, and
NER pipelines. It is a stronger contextual modern-German comparator than
generic GSD-based models because it uses UD-HDT and trains its lemmatizer on a
large DWDS example set annotated through deterministic DWDSmor analysis.

The freshness check corrected an initially stale choice:

- Zenodo 2.3.0, published 2025-09-25, remains a citable six-megabyte source
  archive with published model-method context;
- GitHub later released 3.1.0 on 2025-11-09;
- the official package index currently serves 4.0.0, built on 2025-12-11;
- tag `v4.0.0` resolves to
  `7eabc17097a3ea39f5cc9c030a605ff7edc20ae4`;
- `de_zdl_lg-4.0.0` is 627,548,130 bytes with SHA-256
  `9d35263ac80e80e9730ee21830ffdbe96cf256b72c71e30326ae5865456ade9a`;
- the alternate `de_zdl_dist-4.0.0` is 508,569,684 bytes with SHA-256
  `72ffdd5f8e9db0fa5b197c6be68e8d160ce1b6a3953f17f19f64d30bcf7e0081`.

Version 4.0.0's build binds UD German HDT `r2.16` and a DWDS example dataset
versioned `2025.12.05`. The repository is GPL-3.0, but its own license notice
warns that training datasets have heterogeneous terms. There is no matching
GitHub release note or Zenodo record for 4.0.0 and the tag README still shows
the older install example. Therefore B is admitted **conditionally**: its
wheel metadata, model `meta.json`, data/license inventory, and CPU-only
dependency closure must be inspected after acquisition and before execution.
No published 4.0.0 metric is treated as known.

Sources:

- [ZDL spaCy models repository](https://github.com/zentrum-lexikographie/spacy-models)
- [ZDL v4.0.0 tag](https://github.com/zentrum-lexikographie/spacy-models/tree/v4.0.0)
- [ZDL 2.3.0 archival record](https://zenodo.org/records/17201960)
- [official package index](https://gitup.uni-potsdam.de/api/v4/projects/21461/packages/pypi/simple/de-zdl-lg/)

### 3.3 Historical DTAEC normalization

Ehrmanntraut's report targets German literary texts from approximately
1700–1900 and combines type-level normalization with contextual language-model
reranking. This is temporally and generically closer to Nietzsche than modern
news models. The paper still reports limited generalization and scarcity of
high-quality parallel data.

The small type model is the narrowest useful C component:

| Field | Frozen research value |
| --- | --- |
| paper | arXiv v2, revised 2025-02-25; not peer-reviewed evidence |
| code commit | `8619fd8961caac5d5f961df0e689f6a9ad3948cd` |
| code license | MIT; weights and lexicon explicitly excluded from that grant |
| model revision | `bf13bc367ff08a91983eeb44002f48a2e713b28b` |
| model weights | 31,741,808 bytes |
| weights SHA-256 | `002146d5baf5a9676c0748993d7ec275f0e991ca6bd4664ff8c7644d902f77fd` |
| model shape | 7.93M parameters, F32, single-token input only |
| self-reported score | 0.9546 word accuracy; 0.9096 OOV on DTAEC lexicon evaluation |

The model card labels the weights CC0, then explicitly warns that model
inferences or derivatives may count as adaptations of CC BY-NC 3.0 DTA
EvalCorpus/source material, CC BY 3.0 TextGrid material, and Project Gutenberg
material. That ambiguity is a real rights gate. C may be acquired only for a
local research-only experiment after layer-specific rights review; no C text
or derivative is public by default.

The full hybrid route also requires a DTAEC lexicon and
`dbmdz/german-gpt2`. It is deferred from the first trial so that the experiment
can isolate the effect of type normalization without adding a second
contextual language model and a larger rights/dependency surface.

Sources:

- [Historical German Text Normalization Using Type- and Token-Based Language Modeling](https://arxiv.org/abs/2409.02841)
- [DTAEC type-normalizer model card](https://huggingface.co/aehrm/dtaec-type-normalizer)
- [hybrid_textnorm repository](https://github.com/aehrm/hybrid_textnorm)

### 3.4 Current modern controls and why they are not B

| Candidate | Current official state | Decision |
| --- | --- | --- |
| spaCy | 3.8.14, released 2026-03-29 | runtime/library context; generic German pipelines remain modern GSD/news controls |
| Stanza | 1.14.0, released 2026-07-15 | watchlist sanity comparator; German packages exist, but public performance documentation still describes Stanza 1.5.1 on UD 2.12 and model licensing inherits unclear upstream terms |
| RNNTagger | 1.5.0, 3.6 GB official bundle | defer; historical models target earlier language stages and package/license cost adds no first-wave distinction |
| DTA-CAB | 1.115, 2021-03-04 | service/reference watchlist; exact license and current service behavior unresolved |
| SMOR | mature noncommercial research route | superseded in first wave by the maintained and smaller DWDSmor Open route |
| GermaLemma | last release lineage from 2019; requires POS input and TIGER-derived resources | defer; old and less cleanly self-contained |
| LLpro | 2023 literary-language pipeline using RNNTagger | reject for this task; its paper expects substantial degradation on older text and its Python/CUDA dependency stack is disproportionate |

Sources:

- [spaCy German models](https://spacy.io/models/de/)
- [spaCy releases](https://github.com/explosion/spaCy/releases)
- [Stanza 1.14.0 release](https://github.com/stanfordnlp/stanza/releases/tag/v1.14.0)
- [Stanza models](https://stanfordnlp.github.io/stanza/available_models.html)
- [Stanza performance and licensing note](https://stanfordnlp.github.io/stanza/performance.html)
- [LLpro](https://aclanthology.org/2023.konvens-main.3/)

### 3.5 Fresh methodological warnings

Two newer results affect the protocol without becoming candidates:

- *Lemma Dilemma* (2025) shows that very large LLMs can produce strong
  few-shot contextual lemmatization on modern multilingual data, but it also
  records missing and hallucinated tokens under weak prompting. The evaluated
  models are too large, externally hosted, and not historical-German
  validation for this machine.
- the 2025 historical-Italian comparison of low- versus high-level
  lemmatization demonstrates that conservative and modernized lemma
  conventions are different research choices. It supports multiple explicit
  lemma-convention fields; it does not validate German outputs.

Sources:

- [Lemma Dilemma](https://aclanthology.org/2025.findings-emnlp.988/)
- [Low- vs High-level Lemmatization for Historical Languages](https://aclanthology.org/2025.clicit-1.4.pdf)

## 4. Required ToS layer contract

Every future morphology row must preserve the following layers independently:

| Layer | Required content | Authority posture |
| --- | --- | --- |
| source anchor | exact item/file digest, page/division/XML locator | source identity |
| diplomatic surface | exact addressed form as represented in the selected witness layer | observation |
| tokenization assertion | token and syntactic-word boundaries, join/split rationale | versioned textual assertion |
| conservative normalization candidate | minimally regularized form and exact edit operations | proposal |
| modernization candidate | current-orthography form and exact edit operations | proposal |
| lemma candidate set | zero or more lemmas, not a forced single answer | linguistic proposal |
| lemma POS | lexical category associated with each lemma candidate | linguistic proposal |
| occurrence POS | category/function selected for this exact context | contextual linguistic proposal |
| morph features | provider-native labels plus mapped STTS/UD fields | contextual linguistic proposal |
| compound/root analysis | optional segmentation and root candidates | analytical proposal |
| provider evidence | software/model revision, artifact digest, config, candidate rank/score | provenance |
| review event | maker, competence scope, alternatives, status, rationale | immutable decision event |

Additional laws:

- the surface is never overwritten by normalization or lemma;
- absence of an analysis is a valid result, not a reason to invent one;
- every provider's raw label is retained before mapping;
- a normalized form is not silently used as the source anchor;
- compound decomposition does not establish lexeme, sign, or concept identity;
- `lexeme_id` is not minted merely because tools agree on a lemma string;
- exact form, historical form family, normalized form, lemma, sense, sign, and
  concept remain separately queryable;
- source-bearing local outputs inherit the current local-only/public-blocked
  posture until their exact derivative rights are decided.

## 5. Frozen A/B/C method

### Shared control

The identity control returns the exact input form and no lemma. It measures:

- how often each method changes a historical form;
- whether the change was necessary for the evaluated task;
- whether token boundaries or punctuation were altered;
- whether a downstream gain came from normalization or from contextual
  analysis.

### A — deterministic candidate generation

Run DWDSmor Open 0.18.0 directly on each historical surface token:

- retain every returned analysis, not only the first;
- retain no-analysis/unknown explicitly;
- request lemma/morph and, as a separate diagnostic, root analysis;
- map provider POS/features to STTS and UD only in a derived field;
- do not use context to choose a winner.

A asks: how much can an explainable current lexicon/grammar recover without
historical modernization or contextual inference?

### B — contextual direct analysis

Run the exact `de_zdl_lg 4.0.0` CPU model on the unchanged historical
sentence/context:

- preserve its tokenizer boundaries and compare them with the source token
  plan;
- retain lemma, POS, morph, and confidence/score where the package exposes it;
- retain the exact context window and sentence boundary;
- do not repair the input before B.

B asks: does a modern contextual model disambiguate A usefully, and where
does late-nineteenth-century literary domain shift defeat it?

### C — normalization-assisted contextual analysis

Run the pinned DTAEC type normalizer only on eligible alphabetic surface
tokens, then pass its candidate normalized sentence through the exact B
route:

- the identity input and B-direct output remain frozen first;
- every character edit is recorded;
- punctuation, whitespace, and source token boundaries are not regenerated by
  the normalizer;
- unchanged, changed, split-like, invalid, and uncertain outputs are distinct;
- the normalizer cannot silently modernize an authorial coinage;
- an auxiliary `C→A` lookup may show whether normalization changes
  deterministic coverage, but it is a diagnostic, not a fourth contestant.

C asks: does historical normalization improve contextual morphology enough to
justify its information loss and rights/runtime cost?

## 6. Experimental admission gates

### 6.1 Before acquisition

1. Re-read `/etc/abyss-machine/AGENTS.md` and
   `/etc/abyss-machine/storage-policy.json`.
2. Refresh all release, license, package-size, and security evidence.
3. Create an `abyss-stack` experiment specification with A/B/C identities,
   exact source revision, output schema, stop law, and removal posture.
4. Run the owner storage/resource/thermal preflight.
5. Acquire A first. Acquire B only after A mechanics and artifact audit close.
   Acquire C only after the rights gate and an actual A/B disagreement
   question justify it.

This research does not authorize installation into system Python, root caches,
the Tree repository, or a project virtual environment. Future runtimes belong
under `/srv/abyss-machine/runtimes`; regenerable caches and run artifacts
belong under their host-owned `/srv/abyss-machine` routes.

### 6.2 Before corpus output

Freeze, before seeing candidate output:

- exact source item/file and local-content digest;
- exact passage and occurrence anchors;
- tokenization method/version;
- one source-local input packet;
- strata and selection seed;
- rights/visibility posture;
- expected output fields and error taxonomy;
- blinded A/B/C display assignment;
- the exact question whose answer could change a decision.

Packet existence creates no human backlog. A first content episode should open
only when a concrete source, translation, sign, or retrieval question needs
morphology. Its bounded calibration set is thirty occurrences, five from each
of six strata:

1. common inflection with expected direct coverage;
2. pre-1901 or otherwise historical spelling;
3. contextually ambiguous surface;
4. compound, truncation, or Nietzschean coinage;
5. name, foreign-language form, or citation;
6. punctuation, hyphenation, page boundary, or tokenization hazard.

The thirty-unit shape is a maximum first calibration, not standing debt.
One-to-three source-visible units may be opened for a narrower concrete
question. Expansion occurs only after a reviewed error pattern justifies it.

### 6.3 Quality evidence

Mechanically computable:

- exact input preservation;
- provider coverage and no-analysis rate;
- number of analyses per occurrence;
- token split/merge disagreement;
- normalization change rate and edit distance;
- A/B/C agreement and disagreement matrix;
- exact-repeat determinism where the method promises it;
- anchor and provenance closure.

Computable only against real competent gold:

- top-1 and top-k lemma accuracy;
- lemma-candidate recall;
- lemma-POS and occurrence-POS accuracy;
- morph-feature micro/macro F1;
- normalization precision/recall;
- compound-boundary/root accuracy;
- error rates by the six declared strata.

No score from HDT, DTAEC, GerManC, or a model card is copied into a ToS quality
cell. External gold can validate wrappers and mappings; only source-matched
competent review can support Nietzsche quality.

### 6.4 Human and solo+AI review

A future review view must show:

- the exact source page/region and addressed diplomatic form;
- enough source context to judge the occurrence;
- separate, blinded candidate analyses;
- the exact normalization edits;
- dictionaries/corpora/critical-witness links used as evidence;
- `accept`, `accept-with-limits`, `reject`, `ambiguous`, and `defer`;
- a correction timer only when correction is actually requested.

The current operator may verify source/page identity, visible correspondence,
candidate provenance, and whether a Russian explanation is useful. The
operator may not certify German orthography, grammar, lemma, morphology,
etymology, or meaning. AI explanations and agreement among tools remain
AI-assisted evidence. If no German-competent authority is supplied, linguistic
results remain `machine_triangulated_candidate` or unresolved.

### 6.5 Cost and speed evidence

Measure each route separately:

- exact download and installed bytes;
- cold runtime load and warm reuse;
- sentence and occurrence throughput;
- peak RSS, zram/swap, CPU/GPU utilization, and wall/CPU time;
- private output and tracked public-safe derivative bytes;
- install/maintenance complexity;
- manual review and correction minutes;
- number of unresolved cases requiring outside competence.

Do not collapse coverage, accuracy, speed, disk, and human effort into one
score. A fast method that forces confident wrong single lemmas may cost more
than an abstaining candidate generator.

## 7. Error ledger

Every run must retain at least these error classes:

- source/OCR error mistaken for a historical form;
- legitimate historical spelling overwritten;
- authorial style or deliberate oddity modernized;
- token split, merge, hyphen, apostrophe, or punctuation loss;
- wrong lemma among homographs;
- right lemma with wrong occurrence POS;
- right POS with wrong inflectional features;
- missed separable verb or fused form;
- compound over-segmentation or under-segmentation;
- false root analysis;
- name/foreign word treated as common German;
- Nietzschean coinage forced into a familiar lexeme;
- capitalization-driven error;
- provider unknown/abstention;
- provider output not mappable without loss;
- reference-gold error or unresolved convention mismatch.

Failed and ambiguous rows stay in the experiment. A metric is invalid if its
denominator silently excludes them.

## 8. Stop and promotion law

Stop or reject a configuration when it:

- overwrites or cannot return to the exact historical surface;
- changes token boundaries without an explicit alignment;
- emits only one analysis while hiding available ambiguity;
- cannot bind output to exact artifact/config/input digests;
- sends source text to an external service;
- requires an unreviewed or incompatible license;
- exceeds the current owner resource gate;
- converts unknown forms into plausible-looking lemmas without an abstention
  trace;
- makes source-bearing output public without a layer-specific rights decision.

Promotion is deliberately narrow:

1. exact occurrence and context may already be source-near observations;
2. A/B/C outputs remain linguistic proposals;
3. a reviewed morphology decision may become an accepted linguistic
   annotation with declared convention and competence;
4. only then may a separate reviewed claim propose `lexeme_id` membership;
5. sign, concept, translation, etymology, and graph relations remain later
   independent decisions.

The private A output occupies step 2 and no current artifact crosses beyond
it.

## 9. Rejected shortcuts and watchlist

- Do not normalize the whole DTA corpus first and then discard the historical
  spellings.
- Do not treat DTA automatic lemma/POS fields as ToS gold.
- Do not use GSD's program-assigned lemmas as a manual reference.
- Do not use a giant LLM as the first lemmatizer merely because it can explain
  an answer fluently.
- Do not infer a unique lexeme or sign from DWDSmor root decomposition.
- Do not call pre-reform spelling a typo by default.
- Do not acquire the full 3.6 GB RNNTagger bundle before a unique comparative
  question exists.
- Do not acquire the full DWDS Edition without a recorded access decision.
- Keep OntoLex-Morph as an interoperability watchlist for later lexical export;
  do not let an RDF vocabulary become the source owner of linguistic
  decisions.

## 10. Current evidence state

| Requirement | State |
| --- | --- |
| ordered research | complete through 2026-07-29; targeted B/C artifact and trigger refresh completed 2026-08-10 |
| exact A/B/C identities | frozen as admission candidates |
| exact current package/model revisions | frozen for A, B, and C research |
| license review | A clear for local GPL experiment; B dataset/model terms need artifact-level review; C research-only rights ambiguity open |
| source-local input packet | exact ignored 11,352-row JSONL; 86,287 token weight; 4,213,466 bytes; mode 0600; SHA-256 `1c734e6f8371e28b863d58e216b7a649aa6d603192f1f7310056e8caa022eb9a` |
| `abyss-stack` experiment profile | present for the source-gated whole-vocabulary A census and later gated B/C challengers |
| host preflight | exact A plan, input, runtime, storage, memory, load, thermal, CPU, license, and offline gates returned `ready` |
| package/model acquisition | A acquired as a seven-wheel exact closure; principal DWDSmor wheel 4,803,531 bytes; B/C zero |
| runtime | exact offline Python 3.12 runtime; 23,451,336 bytes; 1,043 artifacts; artifact-set SHA-256 `6fa42ae80572f582a2dbcab49dbcaa42012279458a106b0ba6717262fa0c260d` |
| morphology tasks | one exhaustive private A census over all exact form types; one three-occurrence output-blind contextual packet materialized from raw TEI with exact target return |
| machine outputs | A raw 54,495,917 bytes, mode 0600, retained owner-locally; two byte-identical passes; tracked receipt contains no source or provider-lemma strings |
| A mechanical coverage | 6,610 / 11,352 form types (0.5822762508809021); 75,872 / 86,287 token weight (0.8792981561533023); 4,742 mechanically unknown types with weight 10,415 |
| A measured execution | pass 1: 3.137308178 s; pass 2: 3.111671021 s; 3,618.3886 forms/s on pass 1; process peak RSS 44,339,200 bytes; host cgroup peak 118,292,480 bytes; swap peak 0 |
| A accuracy / residue review | unmeasured; coverage is not accuracy; unknown and ambiguity aggregates remain unreviewed |
| contextual B/C | one concrete A ambiguity now has a three-context B-only packet; B remains unacquired and unexecuted; C remains question-inapplicable and unacquired |
| human work | zero scheduled; no routine backlog created |
| competent German gold | zero |
| accepted lemmas / lexemes | zero |
| sign, semantic, translation, graph effects | zero |

### 10.1 Targeted freshness and trigger refresh — 2026-08-10

The trigger state changed without changing any accepted linguistic state.
The exact form already selected independently by the initial source-observation
route and returned across the whole Work has one direct-A row with two
competing provider categories: `ADV` and a separable verbal component
(`V`/`SEP`). The A row is preserved in the private run, while the tracked
episode may retain only its digest, category shape, and source-free counts.
This is a concrete contextual POS-disambiguation question rather than an
invitation to review the whole mechanical residue.

The existing complete source-return bundle contains 145 occurrences in frozen
part/token order. Before any B output, the first episode selects exactly three
rows by recurrence rank `1`, `73`, and `145`: first, inclusive median, and
last. The rule uses only the already frozen source order and A ambiguity; it
does not inspect B, C, a translation, a dictionary, a semantic label, or
philosophical importance. The private packet retains the exact raw-TEI context,
target offsets, and composite source selectors. Git receives only digests,
counts, rank roles, part distribution, and the no-authority boundary.

A live primary-source refresh found no pin drift:

- the official ZDL GitLab package registry still exposes only
  `de_zdl_lg` 4.0.0, created 2025-12-11, at 627,548,130 bytes with SHA-256
  `9d35263ac80e80e9730ee21830ffdbe96cf256b72c71e30326ae5865456ade9a`;
  tag `v4.0.0` still resolves to
  `7eabc17097a3ea39f5cc9c030a605ff7edc20ae4`;
- the official DTAEC model API still resolves the ungated public repository to
  revision `bf13bc367ff08a91983eeb44002f48a2e713b28b`, last modified
  2024-08-08; its 31,741,808-byte weight retains SHA-256
  `002146d5baf5a9676c0748993d7ec275f0e991ca6bd4664ff8c7644d902f77fd`;
- `hybrid_textnorm` `main` still resolves to
  `8619fd8961caac5d5f961df0e689f6a9ad3948cd`.

The refresh also narrows the route. The selected exact-form and normalized-form
hashes are identical, while the actual question is occurrence-level POS and
separable-verb disambiguation. Type normalization therefore supplies no
decision-relevant distinction for this episode. B is the only justified new
challenger. C stays blocked until a separate historical-normalization question
or an observed A/B disagreement makes its information-loss, rights, runtime,
and storage cost relevant.

Primary refresh surfaces:

- [ZDL GitLab project API](https://gitup.uni-potsdam.de/api/v4/projects/21461)
- [ZDL official package index](https://gitup.uni-potsdam.de/api/v4/projects/21461/packages/pypi/simple/de-zdl-lg/)
- [DTAEC model card](https://huggingface.co/aehrm/dtaec-type-normalizer)
- [DTAEC model API](https://huggingface.co/api/models/aehrm/dtaec-type-normalizer)
- [hybrid_textnorm repository](https://github.com/aehrm/hybrid_textnorm)

The three-row private context packet is now materialized and independently
reverified: ranks `1`, `73`, and `145` return to parts 1, 3, and 4; two context
units are TEI paragraphs and one is a verse group; all three target slices
match the selected exact-form hash; exact strings and positions remain ignored
mode-`0600`; the tracked receipt exposes no context or occurrence locator.

The next legitimate action is B's artifact-level license/dependency audit,
fresh storage/resource/thermal admission, and only then acquisition and a
bounded exact-B run over this frozen packet. No C acquisition, routine human
review, accepted lemma, lexeme, sign, semantic effect, or publication route is
opened by the packet.
