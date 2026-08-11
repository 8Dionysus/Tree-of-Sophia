# Source Anchor Identity and Selector Research

Status: ordered research plus public-synthetic A/B/C mechanics; not source-text
acceptance, human review, translation alignment, semantic evidence, rights
clearance, migration authority, or canon

Research and experiment date: 2026-08-11

## Question

What is the smallest durable source-anchor contract that lets Tree of Sophia
return from an occurrence, sign proposal, translation alignment, claim, or graph
edge to the exact selected evidence without confusing:

- a persistent anchor identity with mutable location syntax;
- source-file identity with the representation actually selected;
- Unicode character positions with UTF-8 bytes or UTF-16 code units;
- independent alternative selectors with ordered refinement;
- mechanical resolution with textual or scholarly acceptance;
- a tracked public locator with a private source-bearing selector?

The current `tos_source_anchor_v1` remains valid for its existing consumers. It
is not rewritten or bulk-migrated by this research. The additive
`tos_source_anchor_v2` is admitted only as a contract and source-free laboratory
surface until a concrete source route justifies one bounded migration.

## Research order

### 1. Classical standards and official documentation

#### W3C Web Annotation

The normative foundation is the
[Web Annotation Data Model](https://www.w3.org/TR/annotation-model/). It keeps
the source resource, a selector for a segment, and a state for the desired
representation distinct. Its consequences for ToS are exact:

- `TextQuoteSelector` copies the selected text and recommends prefix/suffix
  context;
- text selection is counted in Unicode code points, not code units, in logical
  order, and should not split a grapheme cluster;
- `TextPositionSelector` uses the half-open interval `[start,end)` and is brittle
  if the representation changes, so a state should accompany it;
- `DataPositionSelector` addresses bytes and is not interchangeable with text
  position;
- multiple selectors describe the same segment as alternatives;
- `refinedBy` applies a narrower selector to the result of a broader selector;
- copied quotes can disclose restricted content, while a position selector can
  avoid copying that text into a shared annotation.

[Selectors and States](https://www.w3.org/TR/selectors-states/) is an informative
W3C Working Group Note rather than a Recommendation. It is useful syntax and
implementation guidance, but the Web Annotation Recommendation remains the
stronger authority.

ToS deliberately tightens one Web Annotation allowance: W3C permits consumers
to pick one selector if alternatives differ. A v2 anchor may be marked
`mechanically_resolved` only when its alternatives resolve to the same result.
Different results remain unresolved evidence, not equivalent addresses.

#### TEI, IIIF, EPUB, and format-local address systems

- [TEI P5 4.12.0](https://www.tei-c.org/release/doc/tei-p5-doc/en/html/index.html),
  updated 2026-07-28, remains the current official text-encoding guidance. Its
  `xml:id`, reference-system declaration, linking, segmentation, stand-off, and
  revision surfaces support structural selectors and versioned layers. A TEI
  canonical reference is corpus-specific and must declare its scheme; it is not
  a universal passage identity.
- The [IIIF API status page](https://iiif.io/api/) still lists Presentation API
  3.0.0 and Content State API 1.0 as current specifications. Presentation API
  4.0.0 is a Release Candidate, not stable authority. IIIF Canvas and `xywh`
  routes are strong viewer/page-region coordinates, but not file fixity or
  textual truth.
- [EPUB 3.3](https://www.w3.org/TR/epub-33/) became a W3C Recommendation on
  2026-01-13. [EPUB 3.4](https://www.w3.org/TR/epub-34/) is a Candidate
  Recommendation Draft dated 2026-08-03. EPUB CFI or another fragment syntax
  can therefore enter only as a format-specific selector with an explicit
  `conforms_to`; it cannot replace the ToS Item/File identity ladder.

#### IETF identity and fragment standards

- [RFC 5147](https://www.rfc-editor.org/rfc/rfc5147) defines fragment
  identifiers for `text/plain` and makes the position unit part of the
  addressing contract.
- [RFC 6901](https://www.rfc-editor.org/rfc/rfc6901) supplies the JSON Pointer
  syntax used by synthetic variant A.
- [RFC 6920](https://www.rfc-editor.org/rfc/rfc6920) standardizes naming digital
  objects with hashes. A digest identifies exact bytes; it does not prove
  location, custody, edition identity, provenance, or meaning.
- [RFC 7089](https://www.rfc-editor.org/rfc/rfc7089) supplies the stable Memento
  protocol behind temporal representation access.
- [RFC 8141](https://www.rfc-editor.org/rfc/rfc8141) defines URN syntax. A
  persistent name still needs a separate resolution and evidence route.

### 2. Established and leading work

- Phelps and Wilensky,
  [“Robust Hyperlinks Cost Just Five Words Each”](https://www2.eecs.berkeley.edu/Pubs/TechRpts/2000/5442.html)
  (2000), demonstrates that a small lexical signature can rediscover moved or
  modified material. That is useful recovery evidence, not source identity or
  acceptance.
- Sanderson, Ciccarese, and Van de Sompel,
  [“Designing the W3C Open Annotation Data Model”](https://doi.org/10.1145/2464464.2464474)
  (WebSci 2013), explains why interoperable annotation needs explicit resource,
  segment, provenance, and multiplicity semantics rather than one opaque
  locator string.
- Van de Sompel et al.,
  [“Memento: Time Travel for the Web”](https://arxiv.org/abs/0911.1112), grounds
  temporal representation negotiation. Time is helpful state evidence but does
  not replace exact byte fixity.
- The
  [2014 W3C Web Annotation workshop report](https://www.w3.org/2014/04/annotation/report.html)
  records real scholarly OCR/text anchoring with XPath-range, normalized text
  positions, text quotes, and context together, while also surfacing copyright
  risk. This supports bundled selectors and an explicit public/private split.

The established lesson is not “find one perfect locator.” It is: bind several
independently meaningful locators to the exact representation state, preserve
their semantics, and retain failure rather than silently falling back.

### 3. Freshest relevant work and current surfaces

- Guo and Wei,
  [“From OCR to Analysis: Tracking Correction Provenance in Digital Humanities Pipelines”](https://aclanthology.org/2026.nlp4dh-1.1/)
  (NLP4DH 2026; arXiv
  [2603.00884](https://arxiv.org/abs/2603.00884)), treats provenance as a
  first-class analytical layer. Its span-edit contract uses a fixed base
  revision, Unicode-code-point offsets, half-open intervals, exact original
  substring checks, explicit revision steps, and correction-source metadata.
  This is the closest fresh confirmation of the v2 direction.
- Zhu et al.,
  [TROVE](https://aclanthology.org/2025.acl-long.577/) (ACL 2025), traces target
  sentences to source sentences and types quotation, compression, inference,
  and other relations. It reinforces that an evidence span and the relationship
  derived from it are distinct records.
- Hirsch et al.,
  [LAQuer](https://aclanthology.org/2025.acl-long.746/) (ACL 2025), localizes a
  user-selected generated span to corresponding source spans. Fine-grained
  attribution reduces verification burden, but model localization remains a
  candidate until it resolves against an exact source state.
- Xu et al.,
  [Q-Mask](https://arxiv.org/abs/2604.00161) (2026), reports that current VLMs
  still struggle with stable text-to-region grounding and introduces a dedicated
  benchmark. It is relevant to future visual page-region challengers, not a
  replacement for deterministic file, page, and region identity.
- [“Document Overlap Is Not Evidence Continuity”](https://aclanthology.org/2026.evaleval-1.35/)
  (2026) separates document identity from span-level evidence stability and
  warns that normalized span hashes are sensitive to chunk boundaries. This
  supports keeping a span digest as a strict receipt, never semantic
  equivalence.

No fresh paper or draft displaces the stable Web Annotation distinction among
source, state, selector, and refinement. The current work instead makes the
need for exact span provenance and evidence continuity more urgent. Draft and
RC surfaces remain watchlist inputs, not stable ToS authority.

## Gap in source-anchor v1

The v1 contract correctly binds an anchor to Item, File, digest, provenance,
and typed selector objects. It does not yet say:

1. whether `text_position` counts code points, UTF-16 code units, bytes, or
   tokens;
2. whether `[start,end]` or `[start,end)` is intended;
3. which exact derived text layer and normalization state the offset addresses;
4. whether an array of selectors means alternatives or ordered refinement;
5. whether a mechanically resolved address was textually reviewed;
6. whether a tracked digest-only public record is resolvable or merely a
   receipt for an ignored private selector;
7. when copied quote text is permitted to cross the publication boundary;
8. which format specification governs an external fragment;
9. whether pixel/point coordinates fit the exact declared representation
   extent.

Changing those meanings inside v1 would reinterpret existing anchors. That is
rejected. The safe route is an additive version.

## Additive v2 decision

`ToS/contracts/source-anchor-v2.schema.json` introduces these separations:

| Surface | v2 rule |
| --- | --- |
| immutable target | exact Item, File, SHA-256, and media type |
| typed identity | Anchor, Item, File, Passage, Event, and Review refs are namespace-constrained rather than generic ToS IDs |
| representation state | every selector names a representation ref, exact SHA-256, media type, and text normalization when relevant |
| selector composition | explicit `single`, `alternatives`, or ordered `refinement_chain` |
| text position | Unicode code points only, half-open interval only |
| bytes | distinct `byte_position` selector |
| regions | page/Canvas identity, coordinate space, representation dimensions where needed, and overflow rejection |
| external fragment | value plus mandatory `conforms_to` URI |
| public/private boundary | tracked versus ignored-local storage, source-content visibility, quote presence, and public-payload expectation |
| withheld selector | digest/count/mode receipt that cannot claim mechanical resolution |
| result posture | mechanical resolution and human review are separate fields; every non-`unreviewed` posture requires a typed review ref |
| method | maker, method version, configuration ref, and configuration digest |

The anchor ID remains persistent. A changed representation or materially
changed selector creates a new anchor version or successor rather than
rewriting the earlier evidence. Version 2 or later requires a typed predecessor,
and an anchor cannot supersede itself.

## Public-synthetic A/B/C

All fixtures are invented for this experiment and tracked under
`source-anchor-v2-abc/`. No Nietzsche text, private source payload, local path,
translation, screenshot, model output, or human judgment enters the packet.

| Variant | Representation | Expression | Direct result |
| --- | --- | --- | --- |
| A | compact structured JSON | JSON Pointer and TextQuote as alternatives | both independently returned `The source remains the authority.` |
| B | UTF-8 text containing an astral emoji and decomposed `e + U+0301` | single Unicode-code-point `[4,9)` | returned the complete `café` sequence |
| C | synthetic container manifest plus XHTML member | container member → XML `id=p2` → text-position `[14,23)` | returned `relations` |

The focused resolver reported the exact selection digests:

- A: `2a1627793a54328429a0bf73890698252c9fb1634556819e8d5b6c22fe9202dd`;
- B: `81ef060bcd98adc7824eb5c1ada83c32491b16018e11e79f00ab9d09e04b015a`;
- C: `e2ba41af03120f143f51748f319f1e7a0e647f1bac9c27d63eef38f856f9987e`.

### Negative controls

| Control | Observed result |
| --- | --- |
| UTF-16 offsets `5:10` mislabeled as Unicode code points | selected `afé.` rather than `café`; mismatch rejected |
| quote alternative changed to the other paragraph | alternatives diverged; anchor rejected |
| selector representation digest replaced | state-to-bytes closure failed |
| refinement steps reversed | chain failed rather than being treated as an unordered list |
| tracked `local_only` anchor carrying TextQuote | publication policy rejected copied source text |
| normalized page region with `x + width > 1` | semantic extent check rejected the region |

### Manual checks outside the validator

The actual resources were inspected with independent tools after the focused
run:

- `jq` JSON Pointer output and `rg` quote output were identical for A;
- direct Python slicing showed code-point `[4:9] == 'café'` and the simulated
  UTF-16 mistake `[5:10] == 'afé.'` for B;
- `xmllint` returned the complete `id=p2` element text, after which an
  independent Python slice `[14:23]` returned `relations` for C.

This matters more than a green schema alone: the selected material was observed
directly and the wrong alternatives produced visibly wrong results.

## Quality, cost, and speed

Five warm, separate focused CLI processes completed in `0.19–0.20 s` wall time
with `47,268–47,464 KiB` peak RSS on the current machine. The experiment used
only resident Python, `jsonschema`, `jq`, `rg`, and `xmllint`; no model, GPU,
network acquisition, paid API, or source payload was used. Direct cash cost is
therefore zero for this bounded run. Energy was not directly measured and must
not be inferred from wall time.

Quality is bounded to mechanics:

- 3/3 frozen selections matched their expected public synthetic strings and
  digests;
- 6/6 frozen negative controls were rejected;
- 3/3 results remain `unreviewed` despite mechanical resolution.

These numbers do not measure robustness against real OCR drift, edition
variation, rights complexity, German correctness, translation alignment, or
semantic use.

## What is deliberately not adopted

- quote-only or offset-only source identity;
- UTF-16 or token offsets hidden behind a generic `start/end` pair;
- fuzzy rediscovery as source truth;
- EPUB CFI, XPath, TEI cRef, IIIF Canvas, or a graph URI as a universal ToS ID;
- content hash equality as Work, passage, or semantic equivalence;
- a timestamp without byte state;
- a digest-only public receipt labeled resolvable;
- copied nonpublic quote text in a tracked anchor;
- automatic migration of any existing v1 anchor;
- mechanical resolution labeled human verification.

## Next bounded route

The next source-bearing use must begin from a real unresolved question and pick
one existing v1 anchor whose ambiguity matters. It may create one v2 successor
only after verifying the exact representation and publication posture. Until
then, v2 is proven contract soil and v1 remains the current corpus surface.

Tree for orientation. Graph for relation. Source for authority.
