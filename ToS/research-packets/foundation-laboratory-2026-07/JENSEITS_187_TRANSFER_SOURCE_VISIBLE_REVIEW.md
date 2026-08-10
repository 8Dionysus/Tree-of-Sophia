# Jenseits §187 transfer source-visible review

**Review date:** 2026-08-10

**State:** one private model-source-visible candidate; not admitted

**Route:** `tos-transfer-route-readiness-jenseits-p0308-random-unit-187`

## Question

Can one of the thirty-five mechanically ready transfer routes be made
substantially more trustworthy without pretending that a solo operator who
does not read German can accept German, without retyping work for its own sake,
and without publishing local source text?

Yes, but only to the posture `machine-triangulated-candidate-not-admitted`.
The useful result is a reproducible discrepancy packet and a narrow future
human checkpoint, not an accepted bilingual passage or an A/B/C input.

## Evidence order

### 1. Exact Items and classical/official documentation

The pass begins with the exact local Items, not a search index or normalized
corpus:

- the Naumann 1886 first-print PDF, SHA-256
  `6ae316c90f958d09045fea27b2430b86623ebb85f8a27146099d028775cdc80a`,
  pages 111–112;
- the Polilov/Mysl 1996 target PDF, SHA-256
  `ce16b68089dee0fc53a31d9e97723991b292dd8835c43bbbb40a9373c9a436aa`,
  pages 307–308;
- their separate tracked Item manifests, inventories, and rights records;
- the official [eKGWB documentation](https://doc.nietzschesource.org/en/ekgwb),
  [rights statement](https://doc.nietzschesource.org/en/rights), and stable
  section locator `eKGWB/JGB-187`.

The historical print, automatic derivatives, Russian translation, and
critical edition remain distinct witnesses. A stable critical address does
not silently replace the 1886 Item or authenticate a local HTTP response.

### 2. Established editorial and comparison baseline

`JENSEITS_AUTHORIAL_WITNESS_ROUTE.md` already places the 1886 print and eKGWB
inside the longer manuscript, correction, print, and critical-edition route.
`EKGWB_RIGHTS_AND_TRANSPORT_REFRESH.md` separately establishes the private
non-commercial comparison posture and the public-derivative block. Those
established layers control the interpretation here: exact agreement with the
critical layer may corroborate a candidate reading, but it is not proof of
historical-critical identity, author-final text, German competence, or rights
to publish a derivative.

The comparison therefore uses source-aware printed-line dehyphenation and
Unicode alphabetic token sequences only. It reports aggregate edit topology,
not CER/WER, linguistic correctness, or translation quality.

### 3. Freshest exact checks

On 2026-08-10 the current HTTPS route failed before an HTTP response. Two
bounded reads of the exact Nietzsche Source static JGB include over HTTP both
returned 624,405 bytes with SHA-256
`143d6330c94bf00b135f887eeed5f20f6caee6a98174746227af255583ae8b20`.
The responses were byte-identical and contained the exact `eKGWB/JGB-187`
anchor. The payload remains mode 0600 and local-only. Repeatability improves
fixity; unencrypted transport still supplies no origin authentication.

The four page images were then regenerated from the exact local PDFs with
Poppler `pdftoppm` 26.01.0 at 180 DPI. All four PNG hashes reproduced exactly,
and the temporary render directory was deleted.

## Bounded method

1. Select one short route before looking at any future A/B/C outputs.
2. Read the source and target page pairs directly and record private
   diplomatic line candidates with page/line coordinates.
3. Keep the maker explicit as `model`; do not relabel the result human.
4. Re-render all pages from the exact Items and require the declared hashes.
5. Resolve the eKGWB section from its exact HTML anchor and compare it only as
   a separate critical layer.
6. Compare each automatic candidate against its private source-visible
   candidate after line-aware normalization.
7. Track only identities, hashes, aggregate token diagnostics, finding classes,
   and closed gates. Keep all source-bearing strings ignored and local.

The deterministic recorder is
`scripts/record_golden_kernel_transfer_source_visible_review.py`; its tracked
result is
`transfer-source-visible-review.jenseits-187.v1.json`.

## Findings

### German source side

The printed §187→§188 boundary is visible across pages 111–112. The prior
automatic ABBYY candidate is materially non-diplomatic: fused words, glyph
substitutions, and layout/OCR contamination survive despite the mechanically
valid boundary. Against the private model transcript it has 144 alphabetic
tokens versus 156, with 123 equal tokens, 33 reference-side missing tokens,
21 candidate-side extra tokens, and 18 replacement blocks.

The independently selected critical section and the private source-visible
candidate have the same 156-token alphabetic sequence. This is strong machine
triangulation and a useful check on the visual reading. It does not establish
punctuation identity, historical-critical equivalence, German orthography,
grammar, semantics, or a human-accepted source passage.

### Russian target side

The printed §187→§188 boundary is visible across pages 307–308. Direct page
inspection found a defect that the existing green structural validators did
not: the automatic target candidate omits one complete visible line at the
bottom boundary of page 307.

Against the private model transcript, the automatic target has 144 alphabetic
tokens versus 148, with 136 equal tokens, 12 reference-side missing tokens,
8 candidate-side extra tokens, four replacement blocks, and one deletion
block. The aggregate includes both the whole-line omission and smaller
automatic-layer discrepancies; it must not be read as a human gold score.

## Decision and next gate

Retain this route as the first source-visible, machine-triangulated transfer
candidate and retain the automatic-layer failures as negative evidence. Do
not mutate the frozen thirty-five-candidate sets: the receipt is an additive
overlay over their immutable history.

Current effects remain exactly zero:

- accepted German passage: 0;
- accepted Russian passage: 0;
- source-to-target alignment: 0;
- eligible transfer unit: 0;
- target gold: 0;
- human review or human debt: 0;
- semantic, sign, concept, claim, relation, graph, canon, and publication
  effects: 0.

No routine human checkpoint is scheduled. If a future declared transfer
question selects §187, the useful human work is a short criteria-based review
of the already prepared visible source/target pair and the explicit
discrepancies—not another transcription exercise and not a UI test.
