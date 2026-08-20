# Retrieval C Admission — Granite Embedding R2

Status: method frozen before installation or Retrieval C output
Decision time: 2026-07-23T03:55:50Z
Authority: research and experiment-method selection only; no quality verdict

## Frozen decision

Retrieval C will test exactly one independent multilingual text-embedding
challenger:

- model: `ibm-granite/granite-embedding-311m-multilingual-r2`;
- repository revision: `44399559930365213510b1ee2eb15ded83374f0e`;
- release: 2026-04-29; repository last modified 2026-05-18;
- license: Apache 2.0;
- architecture/output: ModernBERT, 311M parameters, 768 dimensions;
- documented language posture: 200+ pretrained languages, with German and
  Russian among the 52 explicitly retrieval-trained languages;
- documented inference contract: shared query/passage bi-encoder, CLS pooling,
  L2 normalization, cosine similarity;
- execution route: official quantized OpenVINO IR on host CPU;
- isolation: host cache and versioned host runtime only; no system or project
  Python mutation; no remote model code.

Primary owner sources:

- [model card](https://huggingface.co/ibm-granite/granite-embedding-311m-multilingual-r2);
- [model repository](https://github.com/ibm-granite/granite-embedding-models);
- [R2 paper](https://arxiv.org/abs/2605.13521).

## Admitted artifact subset

The pre-download Hub dry run at the exact revision admits only:

```text
README.md
config.json
config_sentence_transformers.json
modules.json
1_Pooling/config.json
sentence_bert_config.json
special_tokens_map.json
tokenizer.json
tokenizer_config.json
openvino/openvino_model_qint8_quantized.bin
openvino/openvino_model_qint8_quantized.xml
```

The selected set is approximately 350 MB. The approximately 4.1 GB complete
repository is explicitly excluded because it duplicates weights across
PyTorch, ONNX, and several OpenVINO forms. Download receipts must replace these
remote estimates with locally computed SHA-256 and byte counts.

## Why this candidate enters C

The choice was made without seeing any Retrieval C scores or ranked results.
It follows the frozen method rather than optimizing toward an observed answer:

1. it is an independent IBM family, not another Qwen variant;
2. it is fresher than the 2024/2025 gte and BGE repository surfaces checked on
   2026-07-22;
3. its license and German/Russian coverage fit the corpus;
4. IBM publishes an exact INT8 OpenVINO artifact for this Intel host;
5. the bounded subset is substantially smaller than BGE-M3 and Qwen3-VL;
6. its static tokenizer and IR can be loaded without remote-code execution.

## Alternatives retained without substitution

| Candidate | Decision for this run | Reason |
| --- | --- | --- |
| `gte-multilingual-base` | retain as comparator | July 2025 repository state; documented SentenceTransformers path enables remote model code |
| BGE-M3 | retain as literature/reference candidate | older, roughly 2.27 GB, and dense+sparse+multi-vector behavior would change more than one variable |
| Qwen3-VL-Embedding-2B | defer from this text run to a separate visual-search experiment | this frozen text lane has passages rather than page images; a later experiment must independently freeze its image corpus and relevance boundary |
| lexical+dense fusion | defer to derived experiment | combines A/B components and is not the independent C required by the goal |

## Post-freeze visual-route status

This historical decision remains unchanged for text Retrieval C. A distinct
experiment was subsequently frozen on 2026-07-29 with the same 20 query bytes,
36 digest-bound page images, immutable completed text controls, and
`Qwen/Qwen3-VL-Embedding-2B` revision
`9f2f7e710d6d81056aa5c0a4f04764fec6bb7bda`.

Its exact acquisition and isolated offline CPU runtime are now mechanically
admitted. The 2026-07-29 memory-gated preflight remains preserved negative
evidence, but a fresh 2026-07-30 preflight later returned `ready` without
changing the 17,179,869,184-byte floor or using force. Owner-routed r9 then
completed over all 36 images and 20 queries with a persisted normalization
audit and text-free Tree result receipt. Its rankings and scores match the
preserved audit-incomplete r8 execution 20/20. A later exact five-query,
source-visible advisory inspection closes the immediate engineering-routing
question: four rank-1 language/witness-policy failures reject the current
unfiltered route as default, while one A/B-missed image-only page recovery
retains it as an opt-in post-text-coverage fallback with predeclared
item/witness/language/content-class constraints. Human relevance remains
unjudged, and no transcription, quotation, source acceptance, accepted
alignment, winner, promotion, or production/public adoption follows. This
later route does not retroactively alter the output-blind Granite selection
above.

## Comparability lock

Retrieval C must consume the same frozen inputs already used by A and B:

- 20 query records and their unchanged local-only query content;
- 24 non-empty passage units from Structure A;
- the same source anchors and rights boundary;
- top-10 output without generated answers;
- quality fields left null until human judgments;
- identical source-visible and blind-review protocol;
- preserved failed setup or execution attempts;
- isolated index deletion and rebuild proof.

No model benchmark, model-card score, or automated target match may promote C.
This file freezes the candidate and method; it does not declare the model fit,
correctness, or superiority.

## Post-freeze execution record

This addendum records what happened after the decision above; it does not
rewrite the selection rationale.

- The exact 11-file subset was installed at the frozen revision and measured
  at 348,082,051 bytes.
- An isolated Tokenizers 0.23.1 runtime uses the pre-existing OpenVINO 2026.2
  package without mutating system Python or a project environment.
- The first smoke preserved a real failure: OpenVINO 2026.2 rejected the
  generic `AFFINITY` property. The corrected smoke uses supported thread
  control while `abyss-machine` owns CPU placement.
- Retrieval C ran twice over the same 24 nonempty passages and 20 query
  records. Both packets verify; the second captured the complete owner
  resource receipt.
- Delete/rebuild and separate-run checks produced the same 237,643-byte index
  SHA-256
  `90fb3aaa0d9e0005af2c12e37b28d7379143901d50a184293967b5b4733eb41c`
  and identical ranked-anchor lists for all 20 queries.
- Five source-visible inspections were made by `model:codex`; they are
  explicitly advisory and non-human. Two inspected exact/noisy cases failed,
  and both cross-language cases exposed target-language policy failures.
- Human relevance receipts remain zero; nDCG, hard-negative error rate, and
  human cost remain null. No winner or promotion is declared.

The comparison and exact run paths are recorded in
`RETRIEVAL_COMPARISON_REPORT.md`.
