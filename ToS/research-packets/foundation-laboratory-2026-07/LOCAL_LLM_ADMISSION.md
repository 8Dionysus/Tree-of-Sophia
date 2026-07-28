# Local Software and LLM Admission for the ToS Foundation Laboratory

Status: freshness gate complete; no new model download and no content-quality
run admitted

Research snapshot: 2026-07-26

Owner boundary: Tree of Sophia owns the task material, source posture, review
meaning, and promotion decision. `abyss-stack` owns the experiment mechanics
and receipts. `abyss-machine` owns the live device, model, runtime, storage,
memory, and thermal facts.

## Outcome first

The next local LLM comparison should use only already resident artifacts:

| Candidate identity | Exact first-wave role | Admission |
| --- | --- | --- |
| A — Gemma 4 E2B, resident GGUF | lowest-cost proposal baseline | admitted after task-material gate |
| B — Gemma 4 E4B, resident GGUF | stronger same-family proposal | admitted after task-material gate |
| C — Qwen3 4B, resident OpenVINO INT4 | independent family and runtime control | admitted after task-material gate |

This is an admission decision, not a model-quality result. No prompt was run,
no output was inspected, and no model was downloaded during this refresh.

The three artifacts are locally present, small enough for sequential trials,
and sufficient to answer the next question: whether a larger resident model or
a different family reduces human correction without weakening source return.
A new model cannot answer that question until the existing three have been
tested on one fixed, manually reviewable packet.

Qwen3 8B remains a resident escalation control. Qwen3.5-4B and
TranslateGemma 4B remain conditional second wave candidates. Qwen3.6 27B and
35B-A3B are current but excluded from the first wave because total loaded
weights, runtime novelty, and acquisition cost are not justified by present
evidence.

## Question and stop line

The question is not “Which July 2026 model is newest?” It is:

> Which already available local route produces the most source-grounded,
> reviewable help per human minute on the next accepted ToS task?

The answer cannot yet be measured. The 15-page human-gold session remains
unaccepted, German textual review remains language-competence-blocked, and the
completed private OCR candidate pass is one human, one pass, ten Russian
pages, and thirty method-choice judgments. That pass may inform an error
taxonomy; it is not diplomatic gold and does not unlock translation,
semantic promotion, CER/WER, or a general model ranking.

The next admissible action is therefore to freeze the exact LLM evaluation
packet and its rubric. Model execution remains closed until the corresponding
source and human-evidence gate is satisfied.

## Evidence order

The evidence was rechecked in the required order. Model freshness was treated
as a compatibility and watchlist fact, not as an automatic promotion signal.

## 1. Current official documentation and model cards

### Gemma 4 and the resident GGUF route

Google's official release page dates Gemma 4 to 2026-03-31, the MTP variants
to 2026-04-16, and the 12B Unified model to 2026-06-03. The page was last
updated 2026-07-02. The current overview describes E2B and E4B as the small
members, permits responsible commercial use of the open weights, gives them a
128K advertised context, and warns that context memory is additional to
weight-loading memory.

Sources:

- [Gemma releases](https://ai.google.dev/gemma/docs/releases)
- [Gemma 4 model overview](https://ai.google.dev/gemma/docs/core)
- [official Gemma 4 E2B model card](https://huggingface.co/google/gemma-4-E2B-it)
- [official Gemma 4 E4B model card](https://huggingface.co/google/gemma-4-E4B-it)

The host does not contain the official Google GGUF files. It contains pinned
Unsloth conversions:

- E2B `UD-Q4_K_XL`, revision
  `90f9618340396838ee7ff5b0ba2da27da62953d3`, 3,184,494,720 bytes;
- E4B `UD-Q5_K_XL`, revision
  `653803f092503c04a65164346f3208a36e707693`, 6,656,152,736 bytes.

They run through a host-managed Vulkan+CPU `llama.cpp` build at commit
`05ff59cb57860cc992fc6dcede32c696efea711c`. These exact community
quantizations must be named in every receipt; official base-model properties
do not prove quantized local behavior.

The first ToS trial uses the host profile's bounded 8,192-token context or a
smaller task-specific value, not the advertised 128K maximum. The experiment
must measure the actual KV-cache and shared-memory footprint.

### Qwen3, Qwen3.5, and Qwen3.6

The resident Qwen3 4B artifact is an OpenVINO INT4 model. Its official
OpenVINO card declares symmetric INT4, group size 128, AWQ and scale
estimation, and compatibility with OpenVINO 2026.0 or later. The host has
OpenVINO `2026.2.0-21903-52ddc073857-releases/2026/2`, so version
compatibility is plausible; the exact ToS prompt/output receipt and resolved
device still have to be proved by execution.

Sources:

- [Qwen3 release overview](https://qwenlm.github.io/blog/qwen3/)
- [OpenVINO Qwen3-4B INT4 model card](https://huggingface.co/OpenVINO/Qwen3-4B-int4-ov)

Qwen3.5-4B is the freshest small open Qwen candidate at this snapshot. Its
official card declares Apache 2.0, 201 languages and dialects, a multimodal
causal architecture, and 262,144 native context. The same card recommends
current framework branches and at least 128K context for full thinking
behavior. Those defaults are not a safe or necessary ToS assumption on this
30-GiB shared-memory host. A reduced-context local conversion would be a new
artifact and a new compatibility experiment, not a drop-in upgrade.

Source:

- [official Qwen3.5-4B model card](https://huggingface.co/Qwen/Qwen3.5-4B)

The current official Qwen3.6 repository lists open 35B-A3B and 27B releases,
not a new small first-wave model. Qwen3.6 is therefore fresh but not
machine-admissible for this question. “Three billion active parameters” does
not mean that only three billion parameters need to be loaded.

Source:

- [official Qwen3.6 repository](https://github.com/QwenLM/Qwen3.6)

### TranslateGemma

TranslateGemma 4B is a relevant specialist control, not the next default.
Google's official card declares 55 translation languages and a total input
context of 2K tokens. Access is license-gated, the current source artifact is
not resident, and explicit German/Russian language-code and direction support
must be verified before acquisition. The short context may be an advantage for
bounded fragments but a limitation for document and recurrence context.

Source:

- [official TranslateGemma 4B model card](https://huggingface.co/google/translategemma-4b-it)

Translation remains closed until the exact German source text is accepted by
a competence-bearing route. A specialized model cannot supply that missing
human competence.

### Intel runtime paths

OpenVINO's current NPU guide pins a particular conversion stack for 2026.2,
recommends group size 128 for models up to roughly 4B–5B, and warns that
greater than 7B models with prompts longer than 1,024 tokens may require more
than 16 GB on Core Ultra Series 2. NPU is therefore a bounded experimental
subroute for the resident Qwen3 4B, not the default C device and not a reason
to add a fourth contestant.

Sources:

- [OpenVINO 2026 release notes](https://docs.openvino.ai/2026/about-openvino/release-notes-openvino.html)
- [OpenVINO GenAI on NPU](https://docs.openvino.ai/2026/openvino-workflow-generative/inference-with-genai/inference-with-genai-on-npu.html)

The official `llama.cpp` build documentation supports Vulkan, and its SYCL
documentation lists built-in Arc GPUs in Meteor Lake, Arrow Lake, and Lunar
Lake as supported. The host already has a pinned, working Vulkan build. A
SYCL build would add a backend and dependency variable without yet answering a
content-quality question, so SYCL stays a later runtime diagnostic.

Sources:

- [`llama.cpp` build documentation](https://github.com/ggml-org/llama.cpp/blob/master/docs/build.md)
- [`llama.cpp` SYCL backend documentation](https://github.com/ggml-org/llama.cpp/blob/master/docs/backend/SYCL.md)
- [exact resident `llama.cpp` commit](https://github.com/ggml-org/llama.cpp/commit/05ff59cb57860cc992fc6dcede32c696efea711c)

## 2. Established evaluation work

Model selection for ToS needs a small, predeclared challenge set rather than a
general leaderboard. Isabelle, Cherry, and Foster define challenge sets as
small hand-designed probes for specific structural divergences. For
translation, Freitag et al. show that professional evaluators with full
document context and explicit error analysis can produce substantially
different system rankings from crowd evaluation.

Sources:

- [A Challenge Set Approach to Evaluating Machine Translation](https://aclanthology.org/D17-1263/)
- [Experts, Errors, and Context](https://aclanthology.org/2021.tacl-1.87/)

Literary translation needs a deliberately simpler human-facing comparison in
addition to an error ledger. Zhang, Zhao, and Eger found that complex MQM used
by nonexpert evaluators can misjudge literary translations, while simpler
best-worst judgments discriminated more effectively in their study.

Source:

- [How Good Are LLMs for Literary Translation, Really?](https://aclanthology.org/2025.naacl-long.548/)

Model suggestions also change the evidence-generating process. A
preregistered 2025 study found that suggestions did not speed annotators in
its subjective tasks but increased confidence and shifted label
distributions. Same-human repetition measures temporal stability, not
inter-reviewer agreement.

Sources:

- [Just Put a Human in the Loop?](https://aclanthology.org/2025.findings-acl.1323/)
- [Consistency is Key](https://aclanthology.org/2025.nlperspectives-1.6/)

Consequently, ToS must collect an unassisted human baseline before exposing
model suggestions on subjective semantic or translation tasks. A later
AI-assisted decision remains useful but has different provenance.

## 3. Fresh primary work checked at the snapshot date

The July 2026 ACL record strengthens rather than relaxes the human boundary.
Gu et al. report a task-specific 25% reduction in expert extraction time with
AI assistance, while still finding the AI route unsuitable as an independent
expert replacement.

Zhang et al. separate source comprehension from translational creativity.
Across their paired literary tasks, stronger comprehension did not imply
human-level creative translation; literal and contextually inappropriate
renderings remained common. Tian et al. find that both traditional metrics and
LLM-as-a-judge are unreliable on non-literal translation, including score
inconsistency and knowledge-cutoff failures.

Sources:

- [Large Language Models Are Effective Human Annotation Assistants, But Not Good Independent Annotators](https://aclanthology.org/2026.findings-acl.4/)
- [Beyond Reproduction](https://aclanthology.org/2026.findings-acl.2030/)
- [Beyond Literal Mapping](https://aclanthology.org/2026.acl-long.205/)

For ToS this means:

- evaluate comprehension and creative/interpretive adequacy separately;
- keep human-only, AI-only, and AI+human lanes distinct;
- never use another LLM's score as the promotion authority;
- preserve metaphor, wordplay, ambiguity, recurrence, and deliberate
  strangeness as explicit review axes;
- treat productivity gains as task-local measurements, not universal claims.

## Live host evidence

The local owner facts were re-read after the web research:

| Owner fact | Snapshot evidence | Consequence |
| --- | --- | --- |
| devices | `2026-07-26T16:41:58-06:00`; OpenVINO CPU, Arc iGPU, and Intel AI Boost NPU all visible | CPU/GPU/NPU capability exists, but per-model execution remains unproved |
| OpenVINO | `2026.2.0-21903-52ddc073857-releases/2026/2` | compatible with the official resident Qwen3 4B card's minimum |
| Gemma registry | E2B and E4B profiles ready; exact revisions and quantizations recorded above | no acquisition cost for A/B |
| Qwen deployment | Qwen3 4B INT4 about 2.29 GB and Qwen3 8B INT4 about 4.88 GB already present | C and one escalation control need no download |
| storage pressure | `2026-07-26T16:46:26-06:00`; `/srv` 72.11% used, 123,556,466,688 bytes free, owner class `green` | capacity exists, but this is not permission to download |
| thermal law | stable 100–105 °C is monitored active range; above 105 °C requires trend and context; emergency is roughly 109–110 °C | no repository-local 90 °C stop may be invented |

Authoritative live locations:

```text
/var/lib/abyss-machine/ai/devices/latest.json
/var/lib/abyss-machine/ai/models/latest.json
/var/lib/abyss-machine/ai/llm/registry/latest.json
/var/lib/abyss-machine/storage/pressure/latest.json
/etc/abyss-machine/AGENTS.md
/etc/abyss-machine/storage-policy.json
```

These are machine-local facts, not portable ToS contracts. They must be read
again immediately before a real run.

## Admission matrix

| Candidate | Artifact state | First-wave decision | Evidence needed to change decision |
| --- | --- | --- | --- |
| Gemma 4 E2B GGUF | resident, pinned | A after content gate | fail source grounding or lose badly on human correction |
| Gemma 4 E4B GGUF | resident, pinned | B after content gate | no meaningful gain over E2B or disproportionate cost |
| Qwen3 4B INT4 OpenVINO | resident | C after content gate | device/prompt receipt fails or source grounding is worse |
| Qwen3 8B INT4 OpenVINO | resident | escalation only | shared A/B/C error taxonomy shows a capacity-limited failure |
| Phi-3.5 mini INT4 | resident | older control, not selected | a distinct diagnostic question requires an age/family control |
| Qwen3.5-4B | absent | watchlist / conditional second wave | resident A/B/C failure plus storage, conversion, runtime, and quantization admission |
| TranslateGemma 4B | absent and license-gated | translation-only second wave | accepted German source, license acceptance, DE/RU direction proof, and resident-route failure |
| Gemma 4 12B Unified | absent | defer | a multimodal/content question justifies acquisition beyond resident 8B |
| Qwen3.6 27B / 35B-A3B | absent and large | exclude first wave | materially different hardware or compelling small-model failure evidence |
| remote proprietary API | no private-witness route | exclude | explicit rights, data-egress, privacy, cost, and owner decision |

No new download is admitted by this matrix.

## Exact first-wave experiment contract

### Scope

The first run is one bounded proposal task, not a general intelligence
benchmark. Eligible task families are:

1. anchored OCR correction suggestions on accepted Russian text;
2. source-grounded structure or lexical observations;
3. semantic-candidate assistance after an unassisted human baseline;
4. translation analysis only after accepted German source text.

The task packet must declare which one question it asks. Mixing translation,
OCR correction, semantic annotation, and free commentary in one score is
invalid.

### Frozen inputs

Before any output:

- freeze exact source IDs, anchor IDs, source digests, context, task
  instructions, output schema, and stop conditions;
- freeze random and hard cases, including omissions, repeated forms,
  negation, metaphor, ambiguity, page boundaries, and false semantic friends;
- freeze a human-only baseline where interpretation is subjective;
- keep recognized translations and contestant identities sealed when they
  would create anchoring;
- commit or otherwise hash-close the public-safe specification before copying
  candidate outputs into the review packet.

### Candidate execution

- Run A, B, and C sequentially under the same task and context contract.
- Use deterministic or minimally stochastic settings where supported; record
  seed, temperature, sampling, thinking mode, and all defaults.
- Record exact model ID, upstream revision, local file digest, quantization,
  quantizer/provider, runtime commit, backend, resolved device, context,
  prompt digest, input digest, output digest, and elapsed/resource evidence.
- Keep thinking/reasoning residue separate from the proposed answer. Do not
  treat hidden reasoning as source evidence.
- Require source spans, `uncertain`, and `abstain`; reject unanchored claims.
- Preserve malformed, hallucinated, refused, empty, and failed outputs.

### Human review

The workbench presents one source-visible task at a time. Candidate identity
is hidden where technically possible. Assignment follows the predeclared
balanced A/B/C schedule so every method occupies every display position as
evenly as the source count permits.

Review is two-stage:

1. judge the source and record the human-only baseline without model
   suggestions for subjective tasks;
2. compare anonymized suggestions using simple choice plus an explicit error
   ledger.

The reviewer records:

- source grounding and source-return success;
- omission, addition, unsupported inference, and contradiction;
- lexical and recurring-sign consistency;
- morphology, syntax, historical-language, and etymology posture where
  competence permits;
- ambiguity preservation;
- metaphor, wordplay, rhythm, and read-aloud posture for translation;
- usefulness of explanation;
- correction minutes and accepted interventions;
- `accept`, `accept-with-limits`, `reject`, `uncertain`, or `defer`.

For German, a reviewer without declared competence may judge only page
identity, legibility, visible structure, and interface behavior. No model
agreement upgrades that visual review into German textual acceptance.

### Measurements

Keep three axes separate:

| Axis | Required evidence |
| --- | --- |
| quality | source-visible error counts by class and severity, best/worst or preference choice, correction outcome, rejected and uncertain cases |
| speed | cold load, prompt processing, generation, total wall time, and human correction minutes |
| cost | peak memory, swap, device use, cache/output bytes, setup and operator time, runtime complexity, and license burden |

Do not collapse these into one weighted score before seeing the error
distribution. A smaller model may win routine extraction while a slower route
is retained only for a narrow hard case.

### Stop and escalation

Stop the run when:

- the task-specific source-evidence or unassisted-human-baseline gate is absent;
- the owner resource route denies the launch;
- the backend silently changes device;
- output cannot resolve to supplied anchors;
- repeated hallucination persists without a new hypothesis;
- active work becomes unstable, memory/swap behavior becomes unsafe, or the
  thermal owner enters a guard condition;
- license, rights, or data-egress posture is unresolved.

Escalate to resident Qwen3 8B only when the shared A/B/C error ledger supports
a specific capacity hypothesis. Admit Qwen3.5-4B, TranslateGemma, a new
backend, or another model only after the resident comparison leaves an
important error class unresolved and the new candidate is likely to isolate
that class.

## Decision

The freshness gate is complete for the next LLM choice:

- resident E2B/E4B/Qwen3-4B is the admitted first-wave set;
- Qwen3 8B is a resident escalation control;
- no model download, new runtime build, NPU conversion, or heavy generation
  run is currently justified;
- model suggestions remain downstream of accepted source text and, for
  subjective work, an anchoring-independent human baseline;
- no model or validator can create human gold, German competence, an accepted
  translation, a sign, a relation, or canon status.

The exact content-free evaluation contract is now frozen in ToS and its
resident execution profile is frozen in `abyss-stack`. The next owner route is
to admit twenty task-specific source units, materialize the exact task packet,
and only then collect the anchoring-independent baseline required by those
subjective tasks. The older 30/15 packet snapshot schedules no work and cannot
authorize A/B/C.
