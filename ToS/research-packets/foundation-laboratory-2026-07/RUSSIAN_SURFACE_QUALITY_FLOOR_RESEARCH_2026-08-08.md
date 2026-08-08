# Russian-Surface Quality Floor Research

Status: ordered research completed; deterministic A and pinned offline
LanguageTool B executed source-free; C not selected or acquired; no German,
translation, semantic, graph, publication, canon, or human-review authority

Snapshot: 2026-08-08

Implementation snapshot: `abyss-stack`
`f0adef34d774d58e495b86592ace303336a6bfa8`

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

## Next C gate

The next comparison, only when a measured uncovered question justifies it,
should preserve the completed A/B evidence and add:

1. A — deterministic floor alone;
2. B — A plus a pinned offline LanguageTool snapshot;
3. C — A plus a pinned Russian acceptability/GEC candidate selected after
   license, hardware, and benchmark-fit review;
4. optional D — an independent checklist LLM only if A/B/C leave a measured
   gap worth its runtime cost.

Measure false rejection, missed high-confidence defects, per-rule coverage,
abstention, latency, memory, storage, and human-attention avoided separately.
Do not optimize or promote from an aggregate score. Do not ask the solo
operator to review another translation candidate merely to test this method.
