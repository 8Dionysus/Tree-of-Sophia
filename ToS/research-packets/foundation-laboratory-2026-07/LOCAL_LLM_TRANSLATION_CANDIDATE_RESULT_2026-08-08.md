# Resident E4B Translation Candidate Result

Status: executed, frozen, and rejected before human review
Snapshot: 2026-08-08
Authority: non-authoritative laboratory result; no source or candidate text

## Question

Can the exact already-resident Gemma 4 E4B route produce one structurally valid
private Russian translation candidate for the admitted `Za-I-Vorrede-1`
opening sentence, with the recognized comparator sealed, and does the result
clear a cheap AI-only Russian-surface floor before operator attention?

This was a single successor capacity probe selected by
`LOCAL_LLM_TRANSLATION_CANDIDATE_REFRESH_2026-08-08.md`. It was not a fourth
lane in the closed historical A/B/C experiment, not a recognized-translation
comparison, and not an accepted translation workflow.

## Fixed method

- source admission: the existing 20-token local-only DTA-derived input;
- critical citation route: the human-admitted-with-limits eKGWB decision;
- prompt: `direct-independent-translation-json-v1`;
- model: `unsloth/gemma-4-E4B-it-GGUF` at upstream revision
  `653803f092503c04a65164346f3208a36e707693`;
- artifact: 6,656,152,736-byte `UD-Q5_K_XL` GGUF with SHA-256
  `5fb55145c335edd0c2e0cdd7505ed1557cd346787449479be65094c6f5b016c6`;
- runtime: exact resident llama.cpp revision
  `05ff59cb57860cc992fc6dcede32c696efea711c`, Vulkan plus CPU;
- decoding: temperature 0, top-p 1, seed 42, maximum 768;
- resource authority: `abyss-machine resource launch`, no force;
- output authority: private candidate only, then AI Russian-surface triage;
- comparator, pre-existing authored translations, German correctness,
  etymology, and semantic judgment: sealed or explicitly unassessed.

## Episode 1: retained infrastructure failure

The first launch loaded the model but produced no candidate. The private
runtime evidence identified two runner-contract incompatibilities: the strict
grammar exceeded llama.cpp's sane repetition limit, and the model's custom
chat template required Jinja mode. The process ended before generation with a
50.736652-second owner wall time.

This is infrastructure evidence, not a translation-quality result. A bounded
runner correction was permitted because there was no model output to preserve
as an immutable candidate. The failed run, stderr, resource evidence, profile,
model, and runtime remain fixity-bound; the episode was not deleted or painted
green by the successful follow-up.

## Episode 2: successful generation, failed surface floor

The corrected route used Jinja and a relaxed structural generation schema,
then enforced the complete candidate contract and exact source echo after
generation. It returned one schema-valid private candidate. The candidate and
source remain private and are represented in Git only by SHA-256.

AI-only Russian-surface triage found:

- an ungrammatical age construction in the main candidate;
- a defective literal gloss that merely duplicated the candidate;
- analysis notes that remained in English rather than Russian;
- unjustified low uncertainty despite the visible defect.

One grammatical alternative was useful enough to preserve as uncertainty
evidence, but it did not rescue the main candidate. The disposition is
`reject-before-human-review`. No operator correction task, comparison against a
recognized translation, German-language judgment, or semantic task was
created.

## Quality, speed, cost, and resource evidence

| Dimension | Infrastructure failure | Frozen E4B candidate |
| --- | ---: | ---: |
| owner wall | 50.736652 s | 288.737642 s |
| model wall | 21.856117 s | 256.656317 s |
| service runtime | 22.040 s | 4 min 16.886 s |
| CPU time | 18.582 s | 1 min 52.338 s |
| memory peak | 11,060,039,680 bytes | 11,505,102,848 bytes |
| swap peak | 144,154,624 bytes | 166,227,968 bytes |
| prompt | unmeasured before failure | 544 tokens at 20.20 tokens/s |
| completion | absent | 542 llama.cpp evaluation runs at 2.61 runs/s |
| observed temperature | 101→102 °C sensor max | 101→102 °C sensor max |
| monetary cost | unmeasured local execution | unmeasured local execution |
| energy cost | unmeasured | unmeasured |
| human correction | not incurred | not incurred |

`completion_eval_runs` is retained under its runtime meaning; it is not silently
renamed to an exact completion-token count. Temperatures remained within the
operator-declared normal range through 105 °C. Successful execution, speed,
memory, and thermal evidence do not imply translation quality.

The historical Gemma E2B A run and this E4B successor are not a clean model
benchmark: runner contracts and generation handling differ, and the current
runtime metric is evaluation runs rather than an asserted token count. The
only defensible quality conclusion is narrower: both main candidates failed
the inexpensive Russian-surface floor, so neither deserved human correction.

## Method result

Retain:

- the single-candidate owner-routed execution route;
- immutable negative infrastructure episodes;
- runner/profile/model/runtime fixity;
- structural generation followed by full post-validation;
- exact source-echo validation;
- cheap AI Russian-surface triage before human attention;
- separate quality, speed, resource, monetary, energy, and human-cost fields.

Reject or defer:

- this exact E4B candidate;
- rerunning it for a cosmetically green output;
- comparison against a recognized translation;
- claims of German correctness, fidelity, etymology, or semantics;
- treating a larger resident model as automatically better;
- creating a human task merely because a candidate exists.

## Tracked evidence

- `experimental-translation-episode.e4b-direct.infrastructure-failure.v1.json`
  records the first no-candidate failure;
- `experimental-translation-episode.e4b-direct.russian-surface-rejection.v1.json`
  records the successful run and AI-only rejection;
- `provenance.experimental-translation-episodes.jsonl` binds both events;
- `experimental-translation-episode.schema.json` is reusable for future
  no-candidate failures and candidate-bearing AI triage without inheriting the
  historical A-specific container assumptions.

The tracked route contains no source text, candidate text, recognized
translation, or private filesystem path. Accepted German, accepted
translation, human review, etymology, semantic signs, relations, graph truth,
publication, and canon effects remain zero.
