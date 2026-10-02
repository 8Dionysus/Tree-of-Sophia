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

## Native whole producer

The installed native route is `tos technical-markup --source-root ABS` with
exactly one of `--build`, `--check`, or `--validate-tracked`.
`--build --issue-identities` additionally requests initial issuance; an existing
issuance is never replaced. Writing requires an explicitly admitted
`--scratch-bytes N` budget. The default whole deadline is 180 seconds;
`--max-seconds` accepts 1 through 600. The external Poppler child also owns a
finite one-second failure cleanup reserve.

The native v1 producer independently creates the original page/panel/block
packet, citation spine, region and heading candidates, both screening views,
summaries, provenance, and the two private layers. It shares only the retained
root/source custody, exact Poppler extraction, XML observation, resource budgets
and Python-compatible JSON primitives with v2. V2 paragraph/verse output is not
a replacement for these v1 units.

The historical Python builder bytes remain the provenance binding and parity
oracle; the native route never executes them. Historical event authorship,
dates and method labels are retained artifact data, not a claim of native
execution. A changed packet-schema digest requires explicit review of the
bounded large-array schema adapter before it can run.

The current maintained recipe is builder digest
`4c80683124592bc969e0db6ffe0d5696ec28d4079bc522725788aeff44abd8eb`.
The retained historical capture binds
`82452bce0c1599925e18ffd08c5d963d1a872da532d9a7aef72b155087aefcb9`;
five provenance warning strings and the method digest evolved, while extraction
and unit algorithms did not. Build/check require the current source binding.
Compare against current Python oracle outputs in separate admitted private
scratch on the same genuine primitives; keep the historical capture unchanged
and report its known provenance differences separately. Tracked validation can
validate the historical packet without extraction or producer execution.

Installed `360c7e344e8c9a2a83291d96c8ab5ac0c19ac382` passed the current
`4c806831` recipe's eleven-output byte/mode/count comparison and installed checks.
The first controlled run completed those assertions before a scratch census
race interrupted initial issuance; its terminal RED remains recorded. A fresh
bounded continuation passed initial issuance, tracked validation and root/private
symlink refusals. This composite mechanical acceptance preserves the original
capture and never reclassifies outputs from interrupted issuance as parity data.

The Python executable entry now delegates every supported v1 mode to installed
`tos technical-markup`, selected by `TOS_NATIVE_PREPARED_CONSUMER_BIN` or `tos`
on PATH. There is no Python producer fallback. `--repo-root` remains the
compatibility spelling for `--source-root`; writes additionally require an
explicit admitted `--scratch-bytes` budget. The maintained test caller consumes
the native command directly. The original 4c source bytes are retained in
`ToS/research-packets/retained-builder-inputs/build_antonovsky_1911_technical_markup/`;
Build/Check resolve only those exact recipe bytes, not the changed facade digest.
The source reference/digest identify the recipe; the installed image and runtime
receipt identify current execution. No retained archive is executed.

Installed `2402bd87760cde0e55ea86a74adbaaa0714f4ca9` subsequently passed
the focused new facade/archive route: read-only Check matched the existing
current-recipe outputs, facade and maintained native tracked validation passed,
and dirty archive plus missing active-original source controls were refused.
The original archive remained nonexecuted and all selected input/output bytes
were restored and checked. This closes the standalone v1 caller cutover in the
same technical scope as the earlier composite eleven-output acceptance.

The v2 standalone producer and maintained caller have also passed their scoped
native checks. Four research producers still import v2 read-only reconstruction
helpers, which in turn import extraction helpers from this active v1 module.
Those helper definitions remain retained until each research producer's native
parity and downstream caller acceptance closes; their native replacements are
already owned by the research routes. The whole Python helper family is not
retired. These mechanics accept no text, segmentation, semantic claim, rights
clearance or publication.
