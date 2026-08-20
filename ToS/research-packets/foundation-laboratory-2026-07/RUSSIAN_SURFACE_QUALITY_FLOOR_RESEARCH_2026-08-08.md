# Russian-Surface Quality Floor Research

Status: ordered research completed; deterministic A, pinned offline
LanguageTool B, and pinned local Qwen GEC C executed source-free; C rejected as
a required gate or autonomous corrector; no German, translation, semantic,
graph, publication, canon, or human-review authority

Snapshot: 2026-08-08

Implementation snapshot: `abyss-stack`
`125466415f20b9303a1342eed2d71dff6dc39a59`

## Result first

Tree of Sophia now has a defensible first filter between schema-valid local
translation output and scarce human attention. It does not score translation
quality. It rejects only a small set of exact output-contract and obvious
Russian-surface failures; if none fires, the result is `uncertain`, not
`review-worthy` or accepted.

The method is intentionally per-rule and asymmetric. It emits no fused score,
cannot let a good aggregate conceal one broken rule, never creates a human task,
and returns no source or candidate text. Two already frozen private successor
candidates were screened without rerunning either model. Both retained their
earlier rejection and gained a separate digest-bound floor receipt. This is
method evidence only.

The first independent rule-engine challenger is now executed as B on 36
invented controls. It adds useful rule-specific signals but does not change a
single A disposition. LanguageTool missed all five packet/surface boundaries
owned by A, three explicit grammar or punctuation defect controls, and one
unbalanced-quote boundary; it also marked one deliberately ambiguous
lowercase literary fragment. B is therefore retained only as an advisory
layer above A. No private candidate was rerun to make this comparison.

The evidence-gated C has now been selected and executed on those same 36
source-free controls. It pins the smallest published Qwen3.5 SyntErr-to-
LoRuGEC adapter and runs it locally in an offline user/network namespace. The
model exactly corrected only 3 of 15 correction targets, changed 2 of 12
already-correct literary controls, and removed punctuation from one
intentional repetition. Its only visible gain over frozen B was the intrusive
subject-comma case. C is retained as quality-negative method evidence and is
rejected as a mandatory stage or autonomous corrector. It changes no A/B
disposition and creates no human task.

## Research question and boundary

The question is narrower than grammatical error correction:

> Can a cheap, reproducible, source-blind method find enough high-confidence
> Russian-surface or candidate-contract defects to prevent obviously poor local
> translation candidates from consuming human attention?

It cannot answer whether the Russian is excellent, whether a marked
construction is deliberate literary language, what the German means, whether
the translation is faithful, or whether a term supports an etymological,
semantic, sign, relation, graph, or canon claim.

## I. Classical and official foundation

### 1. Normative sources are evidence, not a binary software oracle

The Vinogradov Russian Language Institute describes Lopatin's *Complete
Academic Reference Book* as a modernized, fuller edition of the operative
Russian orthography and punctuation rules. The Institute's current Akademos
resource says its academic rules are grounded in established norms and the
academic orthographic dictionary. These are the strongest starting points for
rule identity, but a general literary fragment still requires contextual
judgment; a software match against one rule is not itself a rejection.

Sources:

- [Vinogradov Institute: Complete Academic Reference Book](https://ruslang.ru/book/pravila-russkoy-orfografii-i-punktuacii-polnyy-akademicheskiy-spravochnik-0)
- [Akademos academic rules](https://orfo.ruslang.ru/rules)
- [Akademos bibliographic foundation](https://orfo.ruslang.ru/bibliogr)

### 2. Corpus and dependency annotation describe structure

The Russian National Corpus exposes syntactic annotation in SynTagRus and
Universal Dependencies forms. UD Russian records morphology and dependency
relations, while the current SynTagRus treebank carries a CC BY-NC-SA 4.0
license. These tools can support later morphology and syntax signals, but a
parse is an analysis candidate, not proof that a sentence is acceptable or
that a translation is faithful.

Sources:

- [Russian National Corpus: syntactic annotation](https://ruscorpora.ru/page/instruction-syntax/)
- [Universal Dependencies: Russian](https://universaldependencies.org/ru/)
- [UD Russian SynTagRus](https://universaldependencies.org/treebanks/ru_syntagrus/index.html)

### 3. LanguageTool is locally usable but must be pinned

LanguageTool's open-source core is LGPL 2.1-or-later and can run as an embedded
local HTTP server. Its documentation explicitly says that the self-hosted
server does not include cloud-only AI rules. The project also switched to a
snapshot release model in March 2025, so “latest” behavior is not a stable
experiment identity. The public API is unsuitable for private ToS source work:
it transfers text externally, disallows automated use at scale, and may change
without warning.

LanguageTool therefore entered the laboratory only after an exact offline
snapshot, license, archive and full-runtime-tree digests, rule inventory,
false-positive controls, and a per-control no-network runtime were frozen.
The exact 2026-08-08 snapshot is now a prepared local experimental artifact,
not a release-admitted OS Abyss artifact: upstream supplied no adjacent
checksum or signature, and the owner trust gate remains fail-closed at
`unknown` because no durable registry record, SBOM, SLSA/in-toto evidence, or
Sigstore/Cosign evidence exists.

Sources:

- [LanguageTool source and license](https://github.com/languagetool-org/languagetool)
- [LanguageTool embedded HTTP server](https://dev.languagetool.org/http-server.html)
- [LanguageTool snapshot release roadmap](https://github.com/languagetool-org/languagetool-org.github.io/blob/master/roadmap.md)
- [LanguageTool public API limits and change warning](https://dev.languagetool.org/public-http-api)

## II. Established methods and top work

### 1. GEC must preserve precision and error categories

RULEC-GEC established a Russian GEC corpus for a morphologically rich
language. GECToR showed that edit tagging can be materially faster than full
sequence rewriting. ERRANT established the value of extracting edits and
evaluating error types instead of relying only on one corpus score. These are
important method patterns, but their original data and categories do not
directly define a literary Russian acceptance gate.

Sources:

- [Grammar Error Correction in Morphologically Rich Languages / RULEC-GEC](https://aclanthology.org/Q19-1001/)
- [GECToR: Tag, Not Rewrite](https://aclanthology.org/2020.bea-1.16/)
- [ERRANT: error-type annotation and evaluation](https://aclanthology.org/P17-1074/)

### 2. Acceptability is useful but not human-equivalent

RuCoLA was built specifically for binary Russian linguistic acceptability and
includes generated out-of-domain text. Its authors report a large remaining
gap between models and humans, especially for morphological and semantic
errors. A RuCoLA-style classifier may become a comparative advisory signal;
it cannot be the sole reject or promotion authority.

Source: [RuCoLA: Russian Corpus of Linguistic Acceptability](https://aclanthology.org/2022.emnlp-main.348/)

## III. Freshest relevant evidence

### 1. Russian GEC is moving toward rule-specific diagnostics

LORuGEC (2025) introduced a Russian rule-annotated diagnostic corpus with 348
validation and 612 test sentences selected for complex writing rules. A 2025
Russian GECToR adaptation introduced Russian-specific operations and reported
strong results on RU-Lang8 and GERA while remaining competitive on RULEC-GEC.

Sources:

- [LORuGEC and rule-guided example selection](https://aclanthology.org/2025.bea-1.38/)
- [Grammatical Error Correction via Sequence Tagging for Russian](https://aclanthology.org/2025.acl-srw.82/)

### 2. July 2026 evidence rejects aggregate-green admission

The freshest directly relevant paper demonstrates that aggregate Russian GEC
improvement can hide catastrophic rule-level regression: one reported
subordinate-clause comma result fell from 14% to 1% while the overall metric
improved. The paper introduces a 98-category taxonomy grounded in Rozental and
a 48-rule diagnostic benchmark. Its SyntErr generator exposes the per-rule
distribution and publishes source, data, and training artifacts; the archived
software release is 20.8 MB, so it is a plausible later machine-local
challenger rather than a heavyweight runtime.

This evidence directly determines the ToS method: retain per-rule outcomes and
negative controls; never admit a method because one total score is green.

Sources:

- [What Aggregate Scores Hide: Per-Rule Evaluation of Russian GEC](https://aclanthology.org/2026.bea-1.32/)
- [SyntErr v1.0.1 archive and provenance](https://zenodo.org/records/20182862)

### 3. LLM judgment must be decomposed and non-authoritative

CheckEval (2025) reports improved inter-model agreement when subjective rating
scales are replaced by traceable binary checklist questions. Broader 2025
judge studies still find substantial variability by task and evaluated
property, and difficulty identifying rare low-quality free-form outputs.
Consequently, any future local LLM judge belongs after deterministic and rule
signals, uses explicit binary questions, preserves `uncertain`, and cannot
promote a candidate.

Sources:

- [CheckEval: checklist-based LLM judgment](https://aclanthology.org/2025.emnlp-main.796/)
- [JUDGE-BENCH across 20 NLP evaluation tasks](https://aclanthology.org/2025.acl-short.20/)
- [LLM-as-a-Judge failures on poor-quality free-form text](https://aclanthology.org/2025.newsum-main.1/)

### 4. The selected C minimizes machine burden, not epistemic risk

The SyntErr release exposes code, a v1.0.1 archive, a 39,209-example synthetic
training file, adapters, and per-rule results. Its current implementation
covers 48 of the paper's 98 categories. The published Qwen3.5-0.8B results
rise from 13.7 zero-shot F0.5 to 45.2 after SyntErr training and 54.0 after
SyntErr-to-LoRuGEC continuation. Those aggregates justified a bounded trial,
not adoption: the paper's own per-rule table retains zero or very low results
for lexical compatibility, quantitative-numeral declension, government, and
agreeing participles, while some punctuation categories vary sharply.

The 0.8B route was chosen before output because it fit this machine without a
multi-gigabyte 4B challenger and could run direct BF16 CPU inference without a
quantization confound. The exact base revision is Apache-2.0. The adapter
repository has no formal license declaration, so its two selected files remain
local-research-only and non-redistributable. The trained JSONL was
byte-verified, but the published mixed-source list is not a bit-reproducible
training provenance surface: it contains about 47,000 duplicate lines,
including one line repeated 4,694 times. This limits lineage confidence even
before quality is considered.

Sources:

- [SyntErr project and per-rule tables](https://synterr-nlp.github.io/papers/bea-2026/)
- [SyntErr source](https://github.com/synterr-nlp/synterr)
- [SyntErr v1.0.1 archive](https://doi.org/10.5281/zenodo.20182862)
- [Published GEC adapters](https://huggingface.co/synterr-nlp/bea2026-gec-adapters)
- [Published SyntErr v4 training dataset](https://huggingface.co/datasets/synterr-nlp/synterr-v4-sft)
- [Qwen3.5-0.8B base model](https://huggingface.co/Qwen/Qwen3.5-0.8B)

## Method selected for the first executable floor

The stack-owned implementation is
`mechanics/inference-pilots/parts/tos-foundation-lab/russian_surface_quality_floor.py`.
Its receipt is private and contains only digests, provider fixity, field paths,
counts, rule IDs, posture, disposition, and zero-effect authority fields.

High-precision rejection is limited to:

- replacement characters and forbidden control characters;
- a candidate that is not predominantly Cyrillic;
- an explicit model wrapper such as a `translation:` label inside the
  candidate field;
- normalized exact collapse of candidate and literal-gloss roles;
- normalized exact duplication across main and alternative roles.

Mixed-script tokens, paired-punctuation imbalance, one narrow possible age
construction, generic unsupported quality claims, and Hunspell unknown words
are advisory. They cannot reject alone. Absence of all findings still means
`uncertain`.

## Executed controls and manual inspection

Nine invented source-free controls were read and compared one by one with the
observed per-rule output:

| Control class | Units | Observed result |
| --- | ---: | --- |
| clean literary prose and legitimate proper-name/em-dash prose | 2 | no finding; both remained `uncertain` |
| exact role collapse, duplicate main alternative, model wrapper, non-Russian output, Unicode corruption | 5 | each fired only its declared high-precision rule and rejected |
| possible age construction and generic quality claim | 2 | each fired only its advisory rule and remained `uncertain` |
| expectation/observation disagreements | 0 | none in the manual control walk |

The focused tests passed, but the table above was also inspected from the
actual control-by-control output rather than inferred from a green test count.

One warm in-process local microbenchmark over the nine controls observed:

- deterministic core: 900 evaluations in 6.936292 seconds, about 7.706991 ms
  per candidate;
- deterministic core plus required resident Russian Hunspell: 9 evaluations
  in 2.601451 seconds, about 289.05009 ms per candidate.

These are machine-local method measurements, not stable product benchmarks.
They exclude process startup, disk persistence, model generation, energy, and
human correction cost. No download, network request, model invocation, or new
runtime was used.

## Private candidate result without content disclosure

The floor was applied to the already frozen E4B and Qwen3-8B CPU successor
runs. Both run verifiers passed before screening. Both candidates were rejected
by one exact role-collapse rule; E4B also triggered the advisory possible-age
pattern. The Qwen receipt did not mechanically recover the earlier AI-noted
name-case defect, and neither receipt claims comprehensive grammar coverage.

Both receipts:

- are mode `0600` under the owner-only artifact roots;
- bind the exact run and candidate digests;
- contain no full source, main candidate, literal gloss, or alternative text;
- create zero human tasks and zero accepted German, translation, semantic,
  graph, or canon objects.

This is useful negative evidence: v1 reproduces enough of the earlier rejection
to save attention, while its missed issue categories remain visible rather than
being hidden by a successful disposition.

## Executed A/B comparison

The frozen source-free set contains 12 acceptable, 20 defect, and 4 ambiguous
literary controls. All 36 expected A and LanguageTool observations matched the
actual per-control output. LanguageTool exposed 13 distinct rule IDs. It left
nine defect controls without any LanguageTool signal:

- five candidate-packet/surface failures already rejected by A;
- number agreement, an intrusive subject comma, and a missing relative-clause
  comma;
- one unbalanced-quote provider boundary.

One ambiguous lowercase literary fragment triggered
`UPPERCASE_SENTENCE_START`. Because every LanguageTool finding remains
advisory-only, A and B both reject exactly the same five controls. This is not
an accuracy score or a production benchmark; it is a manually inspectable
rule-stratified method comparison with zero expectation disagreements.

The isolated cold-start comparison measured:

- 78.004763012 seconds total wall time for 36 controls;
- 1.528671645--2.553019610 seconds per LanguageTool call, with a
  2.054580681-second median and 74.734542889-second sum;
- 600,440 KiB peak child RSS;
- a 259,264,698-byte archive and 414,841,492-byte unpacked runtime payload;
- zero network content transfer, human tasks, accepted German units,
  translation packets, or semantic tasks.

The archive SHA-256 is
`07c0bd839258bfb3314f8e7b04bdf578dd5815f82b9d314a5ba4190545b96363`;
the 1,812-file runtime-tree SHA-256 is
`2f82cb558599790cf154f01456406cf280a62099e197800bac77e3201e6e65a9`.
The CLI identifies LanguageTool `6.9-SNAPSHOT`, build
`2026-08-08 18:49:10 +0200`, from source commit
`384f4ba1b6c5f80e8c444b3449fec37795b67d80`. The Russian package inventory
contains 488 explicit `grammar.xml` rule IDs, 22 top-level compiled rule
classes, and 5 top-level resource files; those counts identify the packaged
surface but do not prove that every rule is enabled, correct, or exercised.

## Executed C comparison and manual inspection

The pinned local C used Qwen3.5-0.8B revision
`2fc06364715b967f1860aea9cf38778875588b17`, adapter revision
`95aa4d62ba258194593511256215a186962ad5ed`, direct BF16 CPU inference,
greedy decoding, batches of four, and one offline namespace for the complete
control run. Base and adapter selected-file bytes, Python, packages, prompt,
decoding, `unshare`, and frozen A/B receipt were verified before inference.
No private source or translation candidate entered the run.

The comparison receipt reports:

- 3 exact corrections among 15 targets: preposition/case, an intrusive subject
  comma, and a missing subordinate-clause comma;
- 12 missed or wrong corrections, including unchanged spelling, agreement,
  lexical-government, repetition, typography, casing, and relative-clause
  controls;
- wrong non-minimal rewrites for the double-comparative and unbalanced-quote
  controls;
- 10 exact preservations among 12 already-correct controls, with both clean
  literary em-dash controls altered;
- one observed-only change that removed punctuation from deliberate literary
  repetition;
- no protocol-wrapper failure, but pass-through of meta, non-Russian, and
  corrupt-string boundary observations, which demonstrates that C cannot
  replace deterministic A.

Every actual source-free input, output, and explicit target was then read in
the private manual view. The all-row AI inspection receipt records that this
was content review rather than inference from schema or green tests; it is not
a human review. The result rejects this profile as a default surface gate,
autonomous corrector, or mandatory pipeline stage. Its narrow subject-comma
gain does not compensate for twelve correction misses and literary damage.

The owner-routed run measured 111.711071750 seconds comparison wall, nine
batches, 101.711699502 seconds summed shared-batch generation wall, a 3.5 GiB
cgroup memory peak, and zero process swap. Post-run temperature was 63 °C. The
comparison receipt SHA-256 is
`1ec8cb48e9bce2e630fbc7876eb996b7a43ccf59a1b2edcc343da269b2f30d21`;
the actual-output view SHA-256 is
`ed778dff7f32b1ec5b74a50327d8b4cff6ed7b7c33845a6759575fcb24a9cdbf`;
the all-row manual inspection receipt SHA-256 is
`d8a4ace39c68fb3c9e0d3e78dd030b8aa9959fe8d60e5eb3990e2bfe6fa03a0b`.

Artifact fixity is sufficient for this local experiment but not for
production admission. Formal ABI, SBOM, ML-BOM, SLSA/in-toto,
Sigstore/Cosign, and durable registry evidence are absent, and the adapter has
no formal license. The base, adapter, runtime, actual outputs, and receipts
therefore remain private and local. The test neither publishes nor
redistributes them.

## Next evidence gate

Preserve completed A/B/C as separate methods. Do not rerun this C profile on a
private candidate and do not ask the solo operator to review another
translation merely to exercise it. A future C successor or optional checklist
LLM D requires a new measured gap, per-rule hypothesis, frozen preservation
controls, license and artifact review, and current machine admission. It must
beat the relevant A/B rule evidence without literary damage; an aggregate
score, schema-valid output, or fluent rewrite is insufficient.
