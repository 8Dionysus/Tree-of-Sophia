# Antonovsky 1911 PDF layout markup v1

This route materializes a witness-local, non-semantic technical spine for the
exact Antonovsky 1911 RSL/RuNEB PDF already held under the source-witness
storage boundary.

It records:

- all 153 PDF pages from the tracked resource inventory;
- the left/right half-page panels observed in the two-up scan layout;
- every Poppler `bbox-layout` block as a paragraph candidate;
- line and word counts inside each block without promoting either to accepted
  linguistic units;
- six source-visible region candidates: front matter, Parts I-IV, and back
  matter, bounded at the relevant page side rather than copied from German;
- a deliberately high-precision but incomplete set of typographic
  section-heading candidates;
- a source-visible model screening of all 86 heading candidates into technical
  roles, without creating a human review or accepting section boundaries;
- a provisional paragraph-span overlay that splits 692 internally merged
  Poppler blocks at 1,372 repeated first-line indent boundaries into 2,064
  spans;
- opaque unit identities issued independently of OCR text, labels, ordinals,
  offsets, and display citations;
- a text-free citation spine such as `Za-RU-Ant1911.pdf070.R.b001`.

The PDF is a two-up scan. A single PDF page usually contains two printed book
pages, so PDF-page counting and printed-page interpretation remain distinct.
The source-visible part openings occur on right panels while the left panels
still belong to the preceding region. Region boundaries therefore use ordered
page-side panels and remain unreviewed candidates.

## Authority ceiling

The exact PDF page boundary is source-attested. Panel assignment, Poppler
blocks, inserted extraction whitespace, region spans, and heading detection
are method results. A block counted here is a `layout-block paragraph
candidate`, not an accepted Russian paragraph. A heading candidate is not an
accepted chapter title or chapter span.

The screening layer is explicitly `model_source_visible`, not the
`real_human` review required by the text-unit contract. Its 42-block
source-visible split sample exposed and excluded front-matter reference
material and one verse-line indentation pattern. The sample does not accept
the remaining boundaries. The overlay leaves every v1 block unit and anchor
unchanged, issues no successor unit identities, and remains fully removable.

The embedded text contains OCR and reading-order defects. No correction,
normalization, hyphen repair, historical-spelling repair, sentence boundary,
token, lemma, translation alignment, sign, concept, relation, graph edge, or
canon node is created.

The tracked artifacts contain only identities, locators, geometry, digests,
counts, and status. The sequential embedded-text layer and full Poppler bbox
observation remain mode-0600 below the existing ignored local-content
boundary. The exact scan and embedded text remain local-research-only and are
not authorized for publication or transfer.

## Files

- `plan.v1.json` is the authored source, extraction, region, heading-rule,
  count, storage, rights, and authority plan.
- `identity-issuance.v1.json` binds exact layout locators to already issued
  opaque IDs; normal rebuilds never remint them.
- `source-text-unit.v1.json` is the source-bound technical packet governed by
  `ToS/contracts/source-text-unit-packet-v1.schema.json`.
- `citation-spine.v1.jsonl` is a text-free page/panel/block navigation view.
- `region-candidates.v1.jsonl` reports part-local block counts without making
  a German/Russian correspondence.
- `heading-candidates.v1.jsonl` reports text-free typographic start candidates.
- `technical-screening-plan.v1.json` is the authored, text-free classification,
  split-rule, sample, guard, and authority plan.
- `heading-screening.v1.jsonl` retains all 86 candidates while assigning a
  witness-local technical role to each one.
- `paragraph-segment-overlay.v1.jsonl` records 2,064 exact text-position spans
  over 692 existing block units; the 1,372 added boundaries remain proposed.
- `screening-summary.v1.json` closes screening counts and authority flags.
- `summary.v1.json` and `provenance.jsonl` are deterministic receipts.

## Build and check

Initial identity issuance is an explicit one-time action:

- `python scripts/build_antonovsky_1911_technical_markup.py --build --issue-identities`

Normal operations:

- rebuild: `python scripts/build_antonovsky_1911_technical_markup.py --build`
- full local parity: `python scripts/build_antonovsky_1911_technical_markup.py --check`
- tracked-only validation: `python scripts/build_antonovsky_1911_technical_markup.py --validate-tracked`
- focused tests: `python -m unittest tests.test_antonovsky_1911_technical_markup`

Any source-locator set, source digest, Poppler version, bbox digest, warning
surface, region boundary, or expected count drift fails closed. A real-human
reviewed successor may accept, reject, or compete with these boundaries and
then issue successor text-unit identities. Until that happens, the packet
stays `proposed`, `reviews[]` stays empty, and DE↔RU alignment claims remain
unopened. The 86-row heading set also remains deliberately recall-incomplete;
screening every enumerated row does not establish that every Russian heading
has been found.
