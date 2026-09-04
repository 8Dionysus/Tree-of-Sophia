# DTA first editions to Antonovsky 1911 paragraph alignment v1

This route materializes four source-bound, text-free translation-alignment
packets: one per published German part. It covers all 81 witness-local reading
pairs and every paragraph in the bounded technical layers exactly once:
3,447 German and 3,569 Russian paragraphs across 3,423 alignment records.

## Machine posture

- 3,303 mappings are `proposed`, 111 are `ambiguous`, and 9 source-only units
  outside the bounded 81-reading scope are `deferred`.
- The independent mixed prose-and-verse pass agrees with 3,311 primary groups
  and disagrees with 112, affecting 263 paragraph units. All disagreements are
  preserved as `ambiguous` or `deferred` with no machine resolution.
- The primary profile comparison contributes 20 risks: 12 overlap the
  independent disagreements and 8 agree with the independent grouping but
  remain `ambiguous`; the full conflict-ledger union is therefore 120.
- 39 German verse groups with 368 lines and 13 Russian source-visible
  continuous verse groups with 359 lines are explicit scope exclusions and
  structural-risk evidence, not paragraph mappings or inferred omissions.

The 12 internal null-side transitions are structural-role or cross-kind
candidates, not evidence of translator omission. The 9 German paragraphs
outside the 81-reading boundary are likewise scope residue, not an established
translation judgment.

## Files and private layers

`part-*.translation-alignment-packet.v1.json` are governed by
`ToS/contracts/translation-alignment-packet-v1.schema.json`.
`alignment-spine.v1.jsonl`, `coverage-receipt.v1.json`,
`per-reading-diagnostics.v1.jsonl`, `conflict-ledger.v1.jsonl`,
`scope-exclusions.v1.jsonl`, and `independent-challenger-audit.v1.jsonl` expose
text-free closure and risk receipts. `identity-issuance.v1.json` is a one-time
opaque issuance and is never reminted by a normal rebuild. `manifest.v1.json`
binds static inputs, generated outputs, and four ignored mode-`0600` Russian
paragraph layers used by exact text-position selectors.

## Rebuild and verify

- rebuild: `python scripts/build_zarathustra_de_ru_paragraph_alignment_v1.py --build`
- full local parity: `python scripts/build_zarathustra_de_ru_paragraph_alignment_v1.py --check`
- tracked-only validation: `python scripts/build_zarathustra_de_ru_paragraph_alignment_v1.py --validate-tracked`
- focused tests: `python -m unittest tests.test_zarathustra_de_ru_paragraph_alignment_v1`

Import and `--issue-identities` are historical one-time operations. The
builder refuses to replace either imported machine input or remint identities.

## Authority boundary

This is complete technical proposal coverage, not accepted translation truth.
All packet reviews and projections remain empty. No lexical or semantic
equivalence, translation omission, graph relation, public transfer, or canon
effect is asserted. Source-visible review may later accept, reject, or create
competing claims without rewriting either witness-local segmentation.
