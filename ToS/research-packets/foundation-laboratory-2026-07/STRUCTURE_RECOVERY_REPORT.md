# Structure Recovery Report: Foundation Pilot v1

Status: variants A, B, and C executed; awaiting real-human review
Experiment: `tos-structure-recovery-v1`
Frozen scopes: A/B 36 units; C 12 output-blind pages selected before execution
Execution dates: 2026-07-22 through 2026-07-23

## Result first

Three materially different structure routes now have real machine evidence:

- A is a deterministic native container/text-layer baseline. It is fast and
  useful for inventory, but it returns no Antonovsky outline-PDF text and
  inherits ABBYY/auto-EPUB corruption.
- B is a Docling 2.72.0 hybrid. It routes each tracked item through exact
  corpus provenance: Tesseract plus Heron layout for the outline PDF,
  programmatic text plus Heron layout for the ABBYY PDF, and exact EPUB XHTML
  through SimplePipeline. It completed all 36 units without partial success.
- C is PaddleOCR-VL 1.6 plus PP-DocLayoutV3 over a frozen 12-page visual
  subset. It completed every page, produced no empty output, and recovered
  useful roles and order on several difficult pages, but it was extremely
  slow and memory-intensive on this CPU host.

No variant has a structural F1, reading-order score, accepted transcription,
or measured human correction cost. Human manual-review receipts remain
**0**. Six source-visible advisory model inspections were recorded for B and
six for C, but every one is `advisory-nonhuman` and forbids promotion.

The result therefore retains A/B/C as distinct laboratory evidence without
declaring a quality winner or moving any output into ToS text, semantics, or
canon.

Separately from the A/B/C extraction outputs, the source-owned witness route
now has a deterministic text-free structure map across the four DTA part
witnesses and the Naumann 1893 EPUB/PDF pair. It yields 82 named
division-start correspondences and 246 stable proposed addresses: one TEI
structural path, one exact EPUB member, and one whole PDF page per
correspondence. This closes an addressing gap; it does not improve or accept
any A/B/C transcription.

## Preserved runs

Runtime root:
`/srv/abyss-machine/storage/artifacts/tree-of-sophia-foundation-lab/`

| Run ID | Scope | Runner wall | Machine observation | Outcome |
| --- | ---: | ---: | --- | --- |
| `foundation-pilot-v1-structure-a-20260722` | 36 | 1.148 s | 32,759,808-byte runner peak RSS; 172,027 derived bytes | rejected native revision; XHTML head-title contamination retained |
| `foundation-pilot-v1-structure-a-headfix-20260722` | 36 | 1.228 s | 32,632,832-byte runner peak RSS; 172,489 derived bytes | corrected native candidate; awaiting human review |
| `structure-b-full-36-20260723-r1` | pre-execution failure | not run | language could not be resolved from a brittle sample-name heuristic | failed receipt retained; no content result |
| `structure-b-full-36-20260723-r2` | 36 | 73.305 s | 1,582,149,632-byte child peak RSS; 3,408,211,968-byte owner peak; zero swap; 1,267,021 artifact bytes | 36/36; retained awaiting human review |
| `structure-c-full-12-20260723-r1` | 12 | 5,672.930 s | 9,674,465,280-byte child peak RSS; owner peak 10,694.297 MiB RAM plus 1,005.258 MiB swap; 515,122 artifact bytes | 12/12; retained awaiting human review |

Variant B's owner service took 90.459 seconds and learned a 4,062.905 MiB
future admission estimate. Variant C's owner service took 5,687.198 seconds
(about 94.8 minutes); its observed owner footprint was 11,699.555 MiB and
the runtime owner learned a 14,624.444 MiB future estimate.

C's page prediction times ranged from 22.523 seconds to 834.815 seconds.
That range is material: a single average hides strong dependence on page
density and output length. B and C also ran different frozen scopes, so their
timings are machine-cost evidence within their own boundaries, not a universal
backend benchmark.

## Variant A: native baseline

The corrected A route resolved all 36 tracked anchors mechanically. It
produced nonempty text for the 12 ABBYY-PDF units and 12 auto-EPUB members,
but no text for any of the 12 Antonovsky outline-PDF units even though the
rendered pages visibly contain text.

The first implementation copied XHTML `<title>` text into the body.
Source-visible inspection found the addition; the failed revision was
retained, the parser was corrected, and a separate run confirmed removal.
The remaining output still inherits severe OCR defects and scan furniture.

## Variant B: Docling hybrid

The first full-B attempt failed before useful execution because OCR language
was inferred from sample-name substrings. The method was corrected to resolve:

`item.manifest.json embodiment_ref -> edition embodies_expression_refs ->
expression language`.

Missing, mixed, unresolved, or unsupported language now fails closed, and a
preparation failure closes the run receipt as `failed` instead of leaving a
misleading `prepared` state.

The corrected run completed exactly 12 units in each branch:

- `tesseract-full-page-fallback-plus-heron-layout`;
- `programmatic-text-plus-heron-layout`;
- `exact-epub-xhtml-simple-pipeline`.

It had zero partial successes and mechanically resolved all 36 anchors.
Quality remained `not-computable` and human cost `not-measured`.

Six actual source-visible model inspections produced one
`accept-with-limits` and five `reject` decisions:

- Antonovsky page 300: reject; page number and marginal numbers were
  misrecognized and verse lineation/order was damaged.
- Antonovsky page 330: accept with limits; broad prose order survived, but
  substitutions, marginal numbers, italics, and structural roles did not.
- Mysl page 4: reject; normalized output omitted identity-bearing shelfmark,
  ISBN/copyright structure, and credits were flattened.
- Mysl page 8: reject; inherited ABBYY corruption included a visible reversal
  of the “Я учу...” and “Все существа...” sequence.
- Naumann pages 59 and 380: reject; structure was flattened into single
  blocks and source OCR/furniture was inherited.

These decisions reject inspected outputs, not the entire Docling family.
B remains useful as a fast, provenance-routed hybrid comparator.

## Variant C: document VLM

C used exact host-owned runtime artifacts:

- PaddleOCR 3.7.0 / PaddleX 3.7.2 / PaddlePaddle 3.3.1;
- `PaddlePaddle/PaddleOCR-VL-1.6` at revision
  `66317acc4c9fc17bd154591ce650735cd2855f3e`;
- `PaddlePaddle/PP-DocLayoutV3` at revision
  `7b48a7566925fa464281f930c58eee04fe2c862a`;
- runtime artifact-set digest
  `2e5df640dea5e6dd9aa50b341cbe60bb2655ce12e69fc2060a654400395ba6f1`.

The frozen selection contained two hard and two deterministic-random pages
per source. Initialization took 31.429 seconds; predictions took 5,631.494
seconds; 164 blocks were emitted with zero empty pages. One warning records
that this local model does not support the requested temperature control and
ignored it. This warning is preserved and is not confused with host thermal
policy.

Six source-visible advisory inspections produced five
`accept-with-limits` and one `reject`:

- Antonovsky page 300: main verse-line order and marginal-number separation
  were useful, but stanza and continuation relationships remained absent.
- Antonovsky page 330: reject; marginal numbers were moved before prose in
  the incorrect order `5, 20, 10, 15, 25, 30`, with further substitutions
  and word mergers.
- Mysl page 4: the title, credits, ISBNs, publisher, and year were present,
  but `Г` became Greek `Γ` and `©` became `С`.
- Mysl page 8: visual prose order was recovered rather than inheriting B's
  reversal, but substitutions such as `сдетали`, `поместь`, and `чить`
  prevent clean-text use.
- Naumann page 59: printed prose, page number, and scan-provider furniture
  were separated, but handwritten annotations and frame relations were
  omitted.
- Naumann page 380: title, section, prose-song sequence, refrain, page number,
  and furniture were usefully separated; ornament and explicit
  stanza/refrain relationships were absent.

C is therefore a serious structure challenger, not an accepted
transcription. Its improved visible structure on several pages must be
weighed against page-level errors, omitted witness marks, approximately
95-minute wall time for 12 pages, and an 11.4 GiB observed owner footprint.

## Exact Naumann 1886 numbered-unit source trial

A later bounded A/B/C trial targeted a different question: whether the exact
1886 *Jenseits von Gut und Böse* scan could support stable numbered-unit
start-page navigation without first accepting any OCR text.

- A, the PDF embedded text layer, lost too many printed numerals and was
  rejected as a complete map source.
- B, the provider DjVu XML, exposed all 274 pages and word coordinates. It was
  retained as an independent secondary navigation layer.
- C, the compressed provider ABBYY XML, exposed the same 274-page order plus
  paragraph, line, word, and character geometry. A strict ordered alignment
  produced 272 of the 299 expected machine candidates.
- The remaining 27 algorithmic gaps were not hidden behind looser matching.
  Explicit scan-visible review supplied their start pages; additional bounded
  review retained OCR disambiguations as such.

The exact source page also revealed a structural correction. PDF page 189
prints `237.` a second time after “Sieben Weibs-Sprüchlein” and before 238.
The source-only map therefore preserves 299 units: integers 1–296 plus 65a,
73a, and 237a. It emits 299 proposed whole-page anchors and no OCR strings.

The Russian Mysl witness does not repeat 237/237a at the corresponding prose:
the prose follows the Seven Sayings before 238. The parallel map records this
asymmetry and stays at division granularity. No exact translation alignment
was inferred.

The focused numbered-unit builder completed in about 2.5 seconds on the
resident CPU after the local companions were present. The broader resource
inventory rebuild, which re-read all ten locally available source items and
decompressed the 88 MB ABBYY working stream, took about 65 seconds. These are
observed route costs, not quality scores.

## Exact Polilov/Mysl 1996 target-label trial

A separate bounded A/B/C trial then asked a narrower target-only question:
whether the exact local Polilov/Mysl 1996 PDF could expose stable Russian
numbered-label start pages without transcribing the translation or aligning it
to the German.

- A, plain embedded PDF text, preserved substantial readable material but
  mixed reading order, running heads, and damaged numerals. It was rejected as
  a complete deterministic label map.
- B, bbox-filtered numerals without an order constraint, narrowed the search
  but still admitted unrelated page numbers. It was rejected on its own.
- C, the same bbox geometry constrained by the expected target label order,
  supplied 265 machine candidates. All 33 gaps were checked against visible
  pages instead of being hidden behind a looser OCR rule.

The selected route yields 298 monotonic proposed whole-page starts for
integers 1–296 plus visibly printed `65a` and `73a`. One embedded OCR glyph for
§6 was preserved as a source-visible OCR disambiguation rather than silently
treated as a clean machine match.

The target does not print a second 237/237a label. The separate source map's
`237a` is therefore recorded as a source-only, nonmaterialized asymmetry.
Nothing in the target map pairs a Russian unit with a German unit. It
establishes neither accepted Russian text nor source-to-target alignment,
translation equivalence, translation quality, or semantics.

The local builder reproduced the final map from the fixity-bound PDF in under
one second on the resident host during this run. That is route-cost evidence
for this exact embedded layer, not a quality score or a general OCR benchmark.

A separate release-safe transform now intersects the 298 labels materialized
on both sides and resolves each pair to its two proposed page anchors. It reads
neither payload nor text, and it leaves source-only `237a` unpaired. This makes
future review returnable at shared-label granularity; it does not perform the
source-to-target passage alignment that remains an open content question.

## Hierarchical Mysl target-only trial

The remaining twelve frozen transfer pages belong to two works whose printed
numbering cannot honestly reuse the flat *Jenseits* label map. *Zur Genealogie
der Moral* resets its numerals across four independent series: preface and
essays 1, 2, and 3. *Der Antichrist* has one 1–62 series. A later bounded
target-only trial therefore preserves series-qualified identities instead of
collapsing repeated labels into one false unit namespace.

Over the exact local Mysl PDF, ordered embedded-PDF bbox candidates supplied
123 of 140 proposed starts. Seventeen gaps were resolved through model-only
source-visible page inspection: seven *Genealogie* starts across seven pages
and ten *Antichrist* starts across nine pages. The resulting maps preserve:

| Target expression | Series | Proposed starts | Ordered bbox | Visible model overrides |
| --- | ---: | ---: | ---: | ---: |
| *Genealogie*, Svasyan | 4 | 78 | 71 | 7 |
| *Antichrist*, Flerova | 1 | 62 | 52 | 10 |
| **Total** | **5** | **140** | **123** | **17** |

The builder emits no Russian prose. It binds the exact local Item inventory
and collection work-boundary map, emits proposed whole-page anchors, and keeps
all starts unreviewed by a real human. A separate target-only crosswalk narrows
the six frozen *Genealogie* pages to eight possible series-qualified routes
and the six *Antichrist* pages to twelve. No German parallel numbered-unit map
exists for either work in this slice, so the source-route count remains zero.

This closes target-side navigation mechanics only. It supplies no exact line
or passage-end boundary, accepted Russian, German source route,
source-to-target alignment, translation relation, semantics, rights
clearance, eligible transfer unit, target gold, or human work.

## Decisions

1. Preserve the contaminated A revision, failed B language route, and all
   earlier C setup/scope failures as negative evidence.
2. Retain corrected A for container inventory, anchors, EPUB body extraction,
   and selected readable ABBYY mechanics only.
3. Retain corrected B as a fast, provenance-routed hybrid comparator; reject
   the five inspected output examples and make no family-wide rejection.
4. Retain C as a high-cost independent challenger; reject the inspected
   Antonovsky page 330 result and accept five inspected packets only with
   nonhuman advisory limits.
5. Do not compare B's 36-unit full scope to C's 12-page subset as if they were
   an equal benchmark.
6. Do not report structural F1, reading-order accuracy, correction minutes,
   or a winner until real-human gold exists and at least one aggregate is
   manually recomputed from raw artifacts.
7. Keep all 15 gold candidates unaccepted until two real source-visible human
   passes are recorded.
8. Reuse the 246 Zarathustra structural addresses, 299 Jenseits source starts,
   298 target-only Polilov/Mysl starts, and 140 series-qualified
   Genealogie/Antichrist target starts for future bounded review and comparison,
   but require a narrower reviewed anchor before treating a whole member or
   page as an exact passage boundary.
9. Keep source and target numbered-unit maps independent. The derived
   shared-label pairs may seed later explicit passage-alignment review, but
   they do not themselves prove textual, translational, or semantic
   correspondence.
10. Keep the Genealogie and Antichrist crosswalks target-only until an exact
    source-side question justifies a separately evidenced German unit map; do
    not manufacture parallel routes from matching numerals or work identity.

## What remains unknown

- exact CER/WER, punctuation fidelity, and structural F1;
- whether C's apparent structural gains survive blind human review across the
  full 36-page packet;
- whether B can improve under a new, predeclared branch hypothesis;
- correction minutes and therefore real human-inclusive cost;
- whether a clean scan-derived German witness disagrees materially with the
  provided auto-EPUB text;
- whether independently reviewed narrow source and target anchors support any
  unit-level translation alignment beyond shared visible numbering;
- whether a question-specific German source map can be admitted for either
  *Genealogie* or *Antichrist* without flattening documentary or numbering
  structure;
- whether any structure output is stable enough to seed translation,
  semantic, sign, relation, concept, or graph work.

Those unknowns are blockers to content acceptance, not reasons to erase the
executed machine evidence. Stable addresses make the evidence returnable; they
do not answer the unknowns.
