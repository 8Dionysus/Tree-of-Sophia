# Antonovsky 1911 structural and paragraph spine v2

This route is the complete agent-verified **technical** reconstruction of the
Russian Antonovsky 1911 witness. It is built from the exact frozen PDF and v1
Poppler bbox observation; tracked outputs contain locators, geometry, digests,
counts, and issued opaque identities, but no source text.

## Closed technical census

- 13,071 physical source lines, each assigned exactly once;
- 10,686 reconstructed printed rows;
- 4 parts, 81 reading units, and 112 numbered subparts;
- 3,569 prose paragraphs (838 / 889 / 991 / 851 by part);
- 13 source-visible continuous verse groups and 359 verse lines;
- 110 embedded numbered markers plus 2 raster-visible markers absent from the
  embedded bbox layer;
- 49 rule-bound boundary conflicts, all resolved;
- all 13,070 adjacent physical-line boundaries compared with the primary
  challenger: 358 disagreements retained and resolved in favor of the stronger
  independent source-visible pass, 0 unresolved.

`structure-census.v2.jsonl`, `identity-issuance.v2.json`, and
`primary-challenger-input.v2.json` are fixed inputs. The remaining JSON/JSONL
files are generated spines and receipts covered by `manifest.v2.json`.

## Rebuild and verify

The native producer is called through the installed `tos` executable with an
explicit absolute source home. It requires the same exact local PDF, inventory,
v1 observation plan and Poppler 26.01.0 for reconstruction.

- rebuild: `tos structural-paragraph --source-root ABS_SOURCE_HOME --build`
- full parity: `tos structural-paragraph --source-root ABS_SOURCE_HOME --check`
- tracked-only validation: `tos structural-paragraph --source-root ABS_SOURCE_HOME --validate-tracked`
- native focused tests: `cargo test -p tos-compiler --lib antonovsky_structural::tests`

The separately requested `--private-model` emits exact private words, rows and
identity bindings for local maintained consumers. It is not a public transport.
Writing modes still require the source owner's authority and host storage
admission. `--issue-identities` and `--import-challenger ABS_DIRECTORY` retain
the existing refusal to replace their once-issued inputs.

The original Python builder and its unique tests remain the retained oracle
until the native implementation, all fourteen output bytes, installed caller
and dependent consumer routes have passed their actual acceptance checks:
`python scripts/build_antonovsky_1911_structural_paragraph_v2.py --check` and
`python -m unittest tests.test_antonovsky_1911_structural_paragraph_v2`.
A source port alone does not retire that oracle or admit source meaning.

Opaque IDs were issued once and are source-binding independent. The builder
refuses to remint them or replace the challenger input.

## Authority boundary

`agent_verified_complete` means complete and independently checked at this
technical layer. It does not accept the Russian text as a corrected edition,
declare editorial or linguistic paragraph truth, align the Russian and German
witnesses, promote semantics, create graph facts, or admit canon. Those remain
separate later layers.
