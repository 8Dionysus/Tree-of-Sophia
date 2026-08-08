# Local LLM Translation Qwen3 8B CPU Refresh — 2026-08-08

Status: research and pre-output admission frozen; source-free smoke required;
no model download, source run, candidate, accepted translation, human task,
semantics, graph truth, publication, or canon effect

Owner boundary: Tree of Sophia owns the admitted source route, the question,
the sealed-comparator law, and every later content decision. `abyss-stack`
owns the isolated runner, exact runtime profile, private output, and execution
receipt. `abyss-machine` owns current storage, memory, load, process, and
thermal admission.

## Decision first

The next useful machine-only translation challenger is the exact already-local
`OpenVINO/Qwen3-8B-int4-ov` artifact on CPU. It may enter one successor episode
only after an offline source-free smoke proves that the exact container,
model, CPU route, chat settings, output extraction, timeout, and cleanup path
work without touching the admitted German source.

If the smoke passes, one private candidate may use the same admitted
`Za-I-Vorrede-1` input, direct independent prompt family, sealed comparator,
deterministic decoding, and post-generation contract validation as the
rejected E2B and E4B candidates. The new question is deliberately narrow:
does an already-local, larger, different-family CPU route clear the obvious
Russian-surface defects that made both Gemma candidates unworthy of operator
attention?

This is not a fourth historical A/B/C lane, not a clean model-size benchmark,
and not a rerun of the failed Qwen3-4B/OVMS GPU route. Model family, capacity,
quantization, runtime, and device differ; the result can screen one candidate
but cannot establish a causal winner.

## 1. Classical and official documentation

The exact official OpenVINO model card identifies the artifact as Qwen3-8B
converted to OpenVINO IR with NNCF `INT4_ASYM`, ratio `1.0`, group size `128`,
scale estimation enabled, and `wikitext2` calibration. It declares Apache-2.0,
OpenVINO 2026.0 or later, and both OpenVINO GenAI CPU inference and OVMS
serving routes.

- [official OpenVINO Qwen3-8B INT4 model card](https://huggingface.co/OpenVINO/Qwen3-8B-int4-ov)
- [official Qwen3-8B model card](https://huggingface.co/Qwen/Qwen3-8B)
- [OpenVINO GenAI inference documentation](https://docs.openvino.ai/2026/openvino-workflow-generative/inference-with-genai.html)

The local artifact resolves to the official OpenVINO repository revision
`5c47abf4b8e12ebe8e99745bb0c1ec17e0c0abcc`. Excluding repository-control
metadata, its exact 19-file tree contains 4,882,866,534 bytes. The canonical
newline stream `relative_path<TAB>size<TAB>sha256`, sorted bytewise by relative
path, has SHA-256
`c75dc7bb87fe010aa8d50fccdf2ab58803e2aba37bea61996fc6c3eb8857f00a`.
The 4,855,348,053-byte weight file has SHA-256
`f2e6960fde61d24a83c477c46f9ee7158748762089f2018fc2bb6a6d6c7f2a7e`.
An executable profile must verify the whole declared tree, not only the model
directory name.

The already-local OVMS image is pinned by repository digest
`sha256:fdc34725b35d79cde23cff1967f90efb40237b7cf5545a70460209056f3da733`
and local image ID
`sha256:f2f53396b27d6efcf76f0184df0d10f0eb55ee09fa78b49e4277e23c585b6968`.
Its presence is runtime availability, not proof that this exact CPU task works.

## 2. Established evaluation basis

The challenge-set, literary-translation, expert-context, and human-assistance
research already ordered in `LOCAL_LLM_ADMISSION.md` remains the evaluation
basis. The current episode applies that method rather than restating it:

1. freeze one source and one question;
2. keep the recognized translation hidden;
3. preserve invalid, failed, and rejected outputs;
4. separate Russian-surface screening from German fidelity;
5. spend operator attention only after a candidate clears the inexpensive
   surface floor;
6. never use an LLM judge as translation authority.

The earlier Qwen3-8B four-case runtime packet proves only short structured
contract execution on the former GPU lane. It does not predict long generation,
CPU behavior, translation quality, or safety. The later Qwen3-4B translation
request produced an i915 hang and context reset; therefore the GPU lane remains
closed and cannot be generalized to this CPU challenger.

## 3. Fresh official and live-host edge

OpenVINO 2026.2 documentation current at this snapshot explicitly supports CPU
LLM pipelines and reports continued Qwen-family CPU optimization. This makes a
CPU-only source-free smoke a supported route, but not a guaranteed result on
the present artifact and container.

- [OpenVINO 2026 release notes](https://docs.openvino.ai/2026/about-openvino/release-notes-openvino.html)
- [OpenVINO `LLMPipeline` API](https://docs.openvino.ai/2026/api/genai_api/_autosummary/openvino_genai.LLMPipeline.html)

The host currently has the exact model tree and pinned OVMS image already
available. The ordinary live OVMS service serves a different embedding model
and must not be stopped, reconfigured, or reused for this trial. The challenger
must run in a separately named, network-disabled, disposable container with
the model mounted read-only and `target_device=CPU`. No GPU device, external
network, source-tree write, model download, or live-service mutation is
admitted.

Qwen3.5-4B remains a fresher Apache-2.0 model-family candidate, but it is absent
locally and would add acquisition, conversion or community-quant provenance,
and a new llama.cpp/runtime variable. TranslateGemma 4B remains a specialist
candidate, but the artifact is absent, access is gated, and the accessible
official card still does not by itself prove the exact German-to-Russian route
for this laboratory. Neither displaces the zero-model-download CPU control.

## Frozen successor method

The `abyss-stack` profile must:

1. bind this research packet, the current citation-witness decision, source
   return overlay, and both rejected Gemma episodes by exact digest;
2. verify the complete 19-file OpenVINO model inventory and exact image digest;
3. use a separately named container with no external network, no GPU device,
   a read-only model mount, and CPU target;
4. preserve the direct independent prompt family, sealed comparator, exact
   source echo, temperature `0`, top-p `1`, seed `42`, and bounded completion;
5. run one source-free synthetic smoke before any source-bearing request;
6. stop on startup failure, timeout, schema failure, container escape, source
   echo drift, or any resource-owner block;
7. keep request, response, source, and candidate text only in the declared
   owner-only artifact root;
8. remove the disposable container after success or failure while retaining
   stderr, response, state, measurements, and digests;
9. screen only Russian surface, output completeness, visible uncertainty, and
   obvious failure before any operator task;
10. create no accepted German, translation, etymology, semantic, graph,
    publication, or canon claim.

## Stop line

Do not alter the live OVMS service. Do not expose a host port. Do not attach a
GPU device. Do not download Qwen3.5, TranslateGemma, another quant, or another
runtime for this episode. Do not reveal Antonovsky or another recognized
translation. Do not rerun the rejected E2B/E4B candidates. A passing
source-free smoke opens one private candidate only; a passing candidate still
does not open human correction until it clears the AI-only Russian-surface
floor.
