# Specialized DE→RU MT challenger admission — 2026-08-10

Status: **research-selected and executed once; exact artifact acquisition,
locked offline runtime, source-free smoke, one private candidate, independent
surface floor, and bounded AI inspection are fixed; result retained as
uncertain method evidence, not accepted translation**.

This packet asks one bounded question created by preserved negative evidence:
after the resident Gemma E2B/E4B and Qwen3 8B general-model candidates all
failed the independent Russian-surface reject floor, is a direct multilingual
MT model a materially better private AI-only proposal mechanism for the same
sealed German fragment?

It does not reopen or replace any prior candidate. It creates neither a fourth
human/AI/AI+human lane nor a translation winner. The candidate remains the
AI-only lane, specialized by method.

## Decision first

Admit **MADLAD-400 3B MT q4 GGUF through a pinned local-only Candle 0.11.0 CPU
runner** as the first specialized challenger, subject to all acquisition,
runtime, source-free smoke, resource, and private-content gates below.

Retain **NLLB-200 distilled 600M** only as a restricted research reserve. The
official 600M checkpoint evidence does not expose an exact German→Russian
score, while MADLAD's primary paper explicitly lists both `de-ru` and `ru-de`
among direct pairs with at least 500,000 examples. This corrects the earlier
generic “NLLB first because it is smaller” hypothesis.

The chosen experiment may emit exactly one Russian string plus token/time
measurements. It must not fabricate explanations, alternatives, lexical
decisions, etymology, uncertainty analysis, or semantic claims that the MT
model did not produce.

## 1. Current primary documentation and artifact evidence

### 1.1 MADLAD-400 training and direction evidence

The [MADLAD-400 paper](https://arxiv.org/abs/2309.04662) reports a multilingual
corpus spanning 419 languages and a translation corpus of 4.1 billion sentence
pairs across 4,124 language pairs. Its multiway corpus contains 11.9 billion
pairs across 20,742 directions. The model uses target-language prefixes of the
form `<2xx>`.

For this admission, the important evidence is narrower than the headline
scale:

- Figure 3 lists both `de-ru` and `ru-de` among direct translation pairs with
  at least 500,000 examples;
- German and Russian have substantial target-language data in the reported
  corpus;
- the 3B architecture is a full 32-layer encoder plus 32-layer decoder, not an
  instruction model pretending to be a translator.

The paper's aggregate non-English MT-3B score must not be presented as an
exact DE→RU result. Its evaluated non-English subset does not provide that
specific pair score.

### 1.2 Exact selected model revision

The selected repository is the official Google namespace
[`google/madlad400-3b-mt`](https://huggingface.co/google/madlad400-3b-mt), exact
revision `fa184c675da0b5c9e1c8694fccd4e12e2d422094`, last modified
2023-11-27, declared Apache-2.0, and not access-gated.

The first bounded artifact set is:

| Role | Exact file | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| weights | `model-q4k.gguf` | 1,654,597,280 | `ea6e5531a3e95213c7f0635988d119e078a655c09306e47851e15d4c0c3f9c37` |
| config | `config.json` | 749 | `cad399cab799b99409a6ec2d90d72552257c2bb752861261d2016691e0643e7c` |
| tokenizer | `tokenizer.json` | 16,629,031 | `a2799ccc696b752ba00c34f58726bfe253a04921ceb6cfc620400f560474790b` |

The path-independent artifact-set digest over role, bytes, and SHA-256 is
`17fd01d2da5bfe4b94886434b05975ff15a4e8f04c9b6e8df324ee0ca93b31b7`.
The [exact revision tree](https://huggingface.co/api/models/google/madlad400-3b-mt/tree/fa184c675da0b5c9e1c8694fccd4e12e2d422094?recursive=true&expand=true)
supplies LFS identities and byte counts; the small config digest was
independently streamed and recomputed.

### 1.3 Provenance ceiling

This is not a paper-team-authored native GGUF release. The repository history
states that it was duplicated from `jbochi/madlad400-3b-mt`; the full
Safetensors artifact is also described as a T5X→Transformers conversion. The
Google-namespace model card explicitly exposes the q2/q3/q4 GGUF files for
Candle, which is stronger evidence than an unrelated third-party quant, but it
does not erase the conversion lineage.

Required posture:

`upstream-namespace-community-conversion-disclosed`

Therefore the artifact may enter a private, reversible research experiment
after exact byte verification. It must not be called paper-authored weights,
a release-trusted production artifact, or a redistribution-ready ToS model.

### 1.4 Runtime evidence and the local-only correction

The official [Candle 0.11.0 release](https://github.com/huggingface/candle/releases/tag/0.11.0)
maps to signed tag commit
`31f35b147389700ed2a178ee66a91c3cc25cc80d` and includes quantized T5/MADLAD
support. The upstream
[`quantized-t5` example](https://github.com/huggingface/candle/blob/31f35b147389700ed2a178ee66a91c3cc25cc80d/candle-examples/examples/quantized-t5/main.rs)
loads CPU GGUF weights and documents the MADLAD prompt route.

The example itself is not admissible unchanged:

- it always creates a Hugging Face API client;
- even with local weights and config it fetches `tokenizer.json` from the Hub;
- it defaults to temperature 0.8 and repeat penalty 1.1;
- it streams token spellings through a replacement heuristic;
- it offers no exact local runtime/output receipt.

The stack route must therefore use a small pinned derivative runner that:

- contains no Hub client;
- takes only exact local GGUF/config/tokenizer/source/output paths;
- runs CPU-only inside a new user+network namespace;
- requires `de`, `ru`, and `<2ru>`;
- rejects input beyond 512 tokens rather than silently truncating;
- freezes temperature `0`, repeat penalty `1`, and at most 512 output tokens;
- decodes the final token sequence with the tokenizer;
- writes one structured candidate plus token and duration measurements;
- writes no rationale, alternatives, or etymology.

The repository source contract is not a runtime. The resolved `Cargo.lock`,
compiler identity, dependency closure, licenses, source digest, binary digest,
complete host runtime manifest, and source-free smoke must all exist before
private source execution.

### 1.5 Why CTranslate2 is not the first MADLAD runtime

[CTranslate2 4.8.1](https://github.com/OpenNMT/CTranslate2/releases/tag/v4.8.1)
remains relevant for NLLB and supported Transformers models. However its
current converter passes `revision` when locating model weights while config,
tokenizer, and general model-file resolution omit that same revision in the
reviewed source. A remote `model-name + revision` conversion could therefore
assemble mixed revisions.

If CTranslate2 is tested later, ToS must first acquire a complete exact local
snapshot, verify every file, and convert from that local directory. It is not
used for the selected MADLAD q4 route because the exact quantized artifact and
Candle implementation already form a smaller one-variable CPU experiment.

## 2. Established work that shapes evaluation

The selected model is not admitted on average BLEU or a generic leaderboard.

- [MADLAD-400](https://arxiv.org/abs/2309.04662) establishes the multilingual
  data and model family, but not literary Nietzsche quality.
- [NLLB](https://arxiv.org/abs/2207.04672) establishes broad many-to-many MT,
  but family-level direction count cannot be transferred to an unpublished
  exact checkpoint-pair score.
- [Experts, Errors, and Context](https://aclanthology.org/2021.tacl-1.87/)
  shows why professional, context-bearing error analysis can change system
  rankings compared with crowd evaluation.
- [A Challenge Set Approach to Evaluating Machine Translation](https://aclanthology.org/D17-1263/)
  supports small predeclared probes tied to concrete divergences rather than
  a post-hoc convenient sample.

For ToS, recognized Russian translations remain sealed witnesses, not automatic
ground truth. The first specialized candidate faces only the already-frozen
source identity and then the asymmetric Russian-surface reject floor. German
fidelity still requires a competence-bearing later route.

## 3. Freshest relevant evidence checked at 2026-08-10

Fresh literary-MT work strengthens restraint rather than selecting a model:

- [LITEVAL / NAACL 2025](https://aclanthology.org/2025.naacl-long.548/)
  evaluates more than 2,000 literary translations and 13,000 sentences. Its
  results caution that complex MQM assigned to nonexpert reviewers can
  mis-rank published human translation, while simpler expert-facing relative
  judgment is more useful. Human translations remain less literal and more
  diverse; smaller M2M/NLLB systems rank consistently low in the reported
  setting.
- [Tian et al., ACL 2026](https://aclanthology.org/2026.acl-long.205/)
  report 7,530 human scores and show that both traditional metrics and LLM
  judges remain unreliable for nonliteral translation and thresholded
  decisions.
- [Tan et al., ACL 2026](https://aclanthology.org/2026.acl-long.268/)
  find document translation plus segment refinement strongest in their
  setting, but the gains are concentrated in fluency and can project output
  toward the refiner's distribution rather than guarantee adequacy.
- [Zhang et al., Findings ACL 2026](https://aclanthology.org/2026.findings-acl.2030/)
  separate comprehension from creative translation potential; strong
  comprehension does not establish human literary creativity.
- [Griebel and Underwood, NLP4DH 2026](https://aclanthology.org/2026.nlp4dh-1.17/)
  analyze 130,486 paragraphs from 106 novels and report a persistent tension
  between fluency and faithfulness metrics, with segment length affecting the
  result.

Consequences for this episode:

1. no scalar winner;
2. no automatic metric as acceptance authority;
3. Russian fluency is allowed to reject obvious damage but cannot prove
   fidelity;
4. no “LLM judge” substitutes for German competence;
5. no human task is created unless the specialized candidate first survives
   the cheap floor and leaves a real comparison question;
6. one bounded sentence cannot justify family-wide claims.

## Shortlist correction

| Candidate | Direct DE→RU evidence | License/artifact posture | Machine cost | Decision |
| --- | --- | --- | ---: | --- |
| MADLAD-400 3B MT q4 | primary paper lists `de-ru` and `ru-de` direct pairs with ≥500k examples; no exact pair score | Apache-2.0; official Google namespace but disclosed community conversion/quant lineage | 1.67 GB selected files; CPU quantized | **first specialized challenger** |
| NLLB-200 distilled 600M | model supports both language codes, but official 600M metrics contain `de↔en` and `ru↔en`, not `de↔ru` | CC-BY-NC-4.0; research/noncommercial constraints; PyTorch pickle weights | 2.48 GB selected snapshot before conversion/runtime | restricted reserve/control |
| TranslateGemma 4B | German and Russian supported generally; reviewed report does not expose an exact 4B DE→RU literary result | gated Gemma license; about 8.6 GB BF16; no selected official text-only quant | materially larger | conditional watch, not first |
| EuroLLM 1.7B | multilingual European family, principally English-centric parallel route; no exact direct-pair result found | Apache-2.0; 3.31 GB | larger than MADLAD q4 and less direct evidence | defer |
| EuroMoE 2.6B-A0.6B | efficient sparse multilingual instruction family; no exact direct-pair evidence | Apache-2.0; about 10.45 GB F32 | high disk despite low active params | defer |
| resident Gemma/Qwen general models | no specialist direct-pair claim required | already local | already measured | preserve three negative candidates; do not rerun |

## Exact stop-lines before acquisition

Acquisition is permitted only after all of the following are current:

1. `/etc/abyss-machine/AGENTS.md`, storage policy, and nearest cache/runtime
   owner instructions have been reread;
2. an `abyss-machine changes preflight` permits the exact cache/runtime
   surfaces;
3. storage preflight covers model bytes, Rust dependency/build cache, runtime,
   and preserved evidence separately;
4. a non-forced resource plan uses an explicit measured/estimated memory
   demand for q4 CPU loading;
5. only the three exact selected model files are fetched;
6. every byte and size matches the table above;
7. provenance remains explicitly conversion-derived;
8. no model, runtime, or cache enters Git, ToS payload storage, `$HOME` root
   caches, or an unrelated service tree.

## Exact stop-lines before private source execution

Execution is permitted only after:

- the pinned local runner builds into the host runtime tree from an exact
  source and `Cargo.lock`;
- the complete runtime manifest verifies every file and command;
- the exact model artifact-set digest verifies;
- an invented, source-free DE→RU smoke passes with the same model and runtime;
- the smoke receipt proves no private source/candidate use and no network;
- the source-return and citation-witness digests still match the exact ToS
  commit;
- both prior E4B and Qwen3 8B negative episode receipts remain fixed and all
  acceptance/promotion gates remain zero;
- fresh storage, process, load, memory, swap, and owner resource gates allow;
- the recognized translation and prior authored Russian surfaces remain
  sealed.

Any denial is a valid retained result. No `force` is authorized.

## Frozen evaluation route

```text
exact ignored German research input
  -> exact MADLAD q4 + exact local Candle runtime
    -> one private candidate, no synthetic explanation
      -> existing deterministic Russian-surface reject floor
        -> reject and stop, or retain as uncertain method evidence
          -> only a real surviving comparison question may open human attention
```

Even a finding-free Russian surface remains `uncertain`. No output from this
episode can establish accepted German, fidelity, style, rhythm, etymology,
semantic meaning, signhood, graph truth, publication rights, or canon.

## Executed setup outcome — 2026-08-10/11 UTC

The selected setup route crossed its mechanical gates and then admitted one
separately commit-bound private source episode.

### Acquisition

- The first acquisition attempt stopped before network or model bytes because
  a redundant storage scan inside the already owner-gated launch timed out at
  180 seconds. Its observed footprint was 8,080.625 MiB. This failure is
  retained; it is not model evidence.
- Route v2 made the owner resource launch the single atomic storage/resource
  authority. It completed in 431.423 service seconds without force and
  preserved 1,653.359 MiB RAM plus 0.523 MiB swap peak.
- All three selected files match the predeclared sizes and digests. The
  acquisition receipt SHA-256 is
  `83bc5deeda07088b80f45e59cc92b18eef8f554323b7cd0bf015985cc82050e3`;
  its model artifact-set digest remains
  `17fd01d2da5bfe4b94886434b05975ff15a4e8f04c9b6e8df324ee0ca93b31b7`.
- The receipt preserves Apache-2.0 and
  `upstream-namespace-community-conversion-disclosed`; it explicitly sets
  private source use, publication authorization, and runtime admission false.

### Locked offline runtime

- The exact source tree and generated `Cargo.lock` were built with Cargo and
  Rust 1.97.1 from the already fetched closure, one build job, and no network.
  The initial clean compile exposed a real mutable-tokenizer API error and was
  retained; the narrow source correction then passed.
- The fresh owner-launched build completed in 432.966 service seconds. It
  observed 1,852.047 MiB RAM plus 29.023 MiB swap, and produced the
  9,501,904-byte binary with SHA-256
  `a18b4b23f95a55f414d6d1ea48e2a5371af7332d73a045f67a9db676e0fca93e`.
- Mechanical build success did not bypass admission. The first runtime
  manifest was rejected because `TOKENIZERS_PARALLELISM` intersects the
  runtime secret-key marker `TOKEN`; three offline flags were unnecessary as
  well. The failed runtime remains retained.
- The exact retained binary, lock, dependency metadata, license, source-tree,
  acquisition, and owner-launch evidence were reverified and materialized
  without recompilation under a schema-compatible empty runtime environment.
  One intermediate repair attempt with a non-schema authority string is also
  retained. The admitted runtime manifest SHA-256 is
  `35544710075f2e1b76b7300879c8e69bdc6cb4a1221fa2bd077c4d7de1a7bd9a`;
  its complete artifact-set digest is
  `b0407f32ca9735953fccb0235725c8ca0697a19302503523b56877fd5a9b7e09`.
- Dependency licenses remain recorded package by package. The runtime makes
  no false aggregate SPDX assertion over the closure: the runner is
  Apache-2.0 and the locked dependency aggregate is `NOASSERTION`.

### Source-free smoke

The invented `Das Fenster ist offen.` fixture ran through the exact model and
runtime in a new user+network namespace. The owner launch returned `allow`,
`forced=false`, peak 3,486.566 MiB, and zero swap. The service itself ran for
13.377 seconds; model load was 6.293957121 seconds and generation
4.477793941 seconds. The worker emitted five tokens and stopped on EOS.

The output `Окно открыто.` is recorded only as manual source-free mechanical
sanity evidence. It is not a DE→RU quality result and does not establish that
the model can translate Nietzsche.

The smoke receipt SHA-256 is
`9ab976b3813646b98c615244cb6641ac0635544ec922651d080d64ffb6ca7023`.
An independent closure audit verified schema, receipt and artifact digests,
the exact non-forced owner launch, the `unshare -Urn` command prefix, zero
network authority, and every declared file. That audit also caught one
worker-created `0644` output inside the enclosing `0700` tree. Its bytes did
not change; the mode was corrected to `0600`, the live closure reverified with
zero issues, and the future runner now enforces owner-only output. This is a
manual evidence-control finding that the original schema alone did not prove.

The measured source-free peak raises the next private execution declaration to
4,608 MiB. The owner resource gate remains authoritative and may still deny.
No setup success by itself authorizes source use, model publication,
translation acceptance, or promotion.

### One private source candidate

After two unrelated host workloads ended naturally, profile
`tos-zarathustra-specialized-translation-madlad400-3b-q4-candle-cpu-v1` at
abyss-stack revision `0afa0517aa847a713681135321ff6a405158bbbf` passed its
fresh exact-source, ToS commit, model, runtime, smoke, namespace, memory, load,
and non-forced resource checks. The separate atomic owner launch then proved
`resource=allow`, `storage=allow`, exact 10,485,760 requested output bytes, and
`force=false` before starting the worker.

The complete call took 979.291729 seconds. The pre-worker owner gate accounted
for 901.152729 seconds; the atomic launch after that decision took 78.139
seconds. The worker took 71.241677 seconds, including 2.651449428 seconds model
load and 61.390189241 seconds generation for 36 source tokens and 33 generated
tokens. The owner cgroup measured a 3,330,088,960-byte memory peak and a
667,648-byte swap peak. This makes centralized storage admission, rather than
decoding, the dominant observed wall cost of the episode.

The private candidate is nonempty and schema-valid. Its run receipt SHA-256 is
`4ccf62e1be5924cd4cb519c2261fd696b110491aee4783a6a5b150dcba65da14`;
its candidate artifact SHA-256 is
`1e76c0d0d3044fa373c5cdabdaffd1e1afdbc43931bcca8845df5707b08e8ff9`.
Both artifacts and the source remain owner-only local files. The recognized
translation was not revealed.

The independent deterministic Russian-surface floor emitted zero findings and
therefore returned `uncertain / retain-for-method-comparison`, never “passed”
or “accepted.” A direct AI reading found the Russian surface fluent enough to
avoid an obvious-defect rejection, while preserving three source-aware risks:
explicit source repetition is compressed, historical source surface is
normalized, and source syntax/cadence is smoothed. These are bounded AI risk
signals, not human German competence or a translation-fidelity verdict. No
human task was created.

## State at this research snapshot

- selected model files downloaded: **3 exact files**;
- new model bytes retained: **1,671,227,060**;
- Candle runtime built: **yes, locked/offline and manifest-verified after two
  retained manifest-admission negatives**;
- source-free smoke executed: **yes, passed mechanically and independently
  closure/mode-checked**;
- private source consumed by this method: **yes, exactly one local-only admitted
  episode**;
- specialized candidate created: **1, private and retained as uncertain method
  evidence**;
- human task created: **no**;
- accepted German units: **0**;
- accepted translation packets: **0**;
- semantic/graph/canon effects: **0**.

This packet now records exact local model/runtime/smoke setup evidence, its
failures, and one text-free private candidate outcome. It is not an accepted
translation, literary-quality verdict, publication decision, or promotion
decision.
