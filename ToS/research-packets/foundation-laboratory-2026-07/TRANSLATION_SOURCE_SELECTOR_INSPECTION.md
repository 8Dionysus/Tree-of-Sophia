# Translation Source Selector Inspection

Status: exhaustive source-visible model inspection; selector method rejected;
no German transcription, translation, or gold unit accepted

Inspection date: 2026-07-23

## Question

Can the frozen v1 translation plan safely obtain one German prose fragment
from each selected EPUB page by taking the first punctuation-terminated unit
inside the first body paragraph?

The answer from this inspection is **no**. The method remains useful as a
negative baseline, but it is not admissible as the source selector for the
translation laboratory.

## Evidence closure

The inspected private packet is
zarathustra-translation-source-v1-20260723.

| Evidence | SHA-256 or count |
| --- | --- |
| source packet manifest | 380f9df2266ee2812684bbdbcf266bcc9936860f374454fe5aa502d40d60b8d5 |
| frozen candidate set | dbc2b40dcb7ca2ca71c364cf17fd08fd68cddcdbeb26a0b48069c9984b232d72 |
| frozen translation sample plan | 0beecf2714b0d01bd8ba386e25a797977e298c93d37a0ab63d6c06488305e6aa |
| exact image-container PDF | 61c947e5aff76a64d82600cc52dcb25ff1b5862530d3a99c96824da885c1e6cf |
| source-visible pages rendered and inspected | 30 / 30 |
| tracked inspection receipt | translation-source-selector-inspection.v1.json |

Every selected PDF page was rendered independently at 180 DPI as a full-page
RGB PNG and visually compared with the matching candidate. The model maker is
recorded as model:codex; the receipt is advisory and has
promotion_authorized: false.

The recognized Russian comparator remained sealed. No comparator content was
consulted or emitted, and no translation lane was started.

## Result

| Decision | Count | Meaning |
| --- | ---: | --- |
| accept with limits | 7 | sentence unit looks structurally usable, but transcription is not accepted |
| reject | 22 | candidate is the wrong unit or contains visible automatic-text contamination |
| uncertain | 1 | previous-page context is required before the boundary can be decided |
| abstain | 0 | every frozen candidate was inspected |

Observed failure modes overlap:

| Failure mode | Count |
| --- | ---: |
| chapter or section heading selected as a sentence | 19 |
| terminal tail of a sentence begun on the previous page | 2 |
| visible OCR contamination in the candidate | 5 |
| page-start boundary unresolved without previous-page context | 1 |

Only 7 of 30 proposals are even structurally usable with limits. This is not a
70% or 23% quality score: there is no human diplomatic transcription against
which source accuracy can be measured. It is a method-rejection observation.

## Fragment ledger

| Fragment | PDF page | Decision | Principal observation |
| --- | ---: | --- | --- |
| 001 | 56 | accept with limits | complete prose unit is visually plausible |
| 002 | 60 | accept with limits | complete prose unit is visually plausible |
| 003 | 63 | accept with limits | unit plausible; exact historical glyph reading remains uncertain |
| 004 | 75 | reject | page-start sentence tail |
| 005 | 80 | reject | heading selected |
| 006 | 103 | reject | heading selected |
| 007 | 106 | reject | heading selected |
| 008 | 113 | reject | heading selected plus leading OCR contamination |
| 009 | 116 | reject | heading selected |
| 010 | 120 | reject | heading selected |
| 011 | 127 | reject | heading selected plus punctuation contamination |
| 012 | 129 | uncertain | previous-page boundary context absent |
| 013 | 133 | accept with limits | complete prose unit is visually plausible |
| 014 | 136 | accept with limits | complete prose unit is visually plausible |
| 015 | 139 | reject | page-start sentence tail |
| 016 | 144 | reject | plausible prose unit contaminated by a leading OCR character |
| 017 | 147 | accept with limits | complete prose unit is visually plausible |
| 018 | 151 | accept with limits | complete prose unit is visually plausible |
| 019 | 156 | reject | heading selected |
| 020 | 166 | reject | heading selected |
| 021 | 170 | reject | heading selected |
| 022 | 178 | reject | heading selected |
| 023 | 187 | reject | heading selected |
| 024 | 200 | reject | heading selected plus leading OCR contamination |
| 025 | 203 | reject | heading selected |
| 026 | 207 | reject | heading selected |
| 027 | 212 | reject | heading selected plus leading OCR contamination |
| 028 | 250 | reject | heading selected |
| 029 | 381 | reject | heading selected |
| 030 | 519 | reject | heading selected |

The detailed receipt binds each row to the exact source anchor, PDF page,
candidate artifact reference, and candidate digest without committing the
private German candidate text.

## Why the method failed

The automatic EPUB derivative commonly represents an entire printed page as
one HTML paragraph. That paragraph can begin with:

- a chapter heading;
- a section number or page furniture;
- the continuation of a sentence from the preceding page;
- OCR debris attached to a heading or prose line.

Punctuation segmentation cannot infer layout role or cross-page continuity.
The first period therefore often closes the wrong textual object. A correct
hash and a reproducible segmentation function only prove that the same wrong
choice can be recreated.

## Manual check caught a false green

The first attempt to render the remaining review pages passed an output name
through an invalid environment-variable expansion. The wrapper reported
success while the intended review directory remained at 6 of 30 pages and
systemd reported an invalid environment variable. A hidden .png was also
misrouted into the abyss-stack worktree.

The retry used an explicit prefix. Manual file counting then proved 30 of 30
pages, and the stray file's SHA-256 matched the corrected page-381 render
exactly before the duplicate was removed.

This is a concrete example of the laboratory law: process exit and schema
green are not sufficient. Expected artifacts, placement, count, content, and
digests require independent checks.

## Required v2 route

The v1 plan and packet must remain immutable negative evidence. A new plan
must explicitly supersede it and freeze a different selector before any
translation output:

1. classify visible layout role before sentence selection;
2. distinguish heading, section marker, prose, quotation, and page furniture;
3. inspect previous, current, and next page context for interior-page units;
4. on chapter openings, select the first prose sentence after the heading;
5. retain the visible page or region anchor beside the automatic text
   proposal;
6. perform diplomatic transcription as its own versioned layer;
7. require real source-visible human passes for glyphs, punctuation, and
   sentence boundary;
8. create new fragment identities or explicit supersession links instead of
   silently rewriting the rejected v1 rows;
9. keep the recognized translation sealed until independent human, AI, and
   AI+human drafts are frozen.

The pre-existing two-pass human source-review template remains the next
authority surface. The user's read-aloud layer remains separate from
philological acceptance.

## Stop line

No AI-only translation, human-only simulation, morphology, etymology,
semantic sign packet, or transfer A/B/C run is authorized from these v1
candidates. The next admissible movement is source selector v2 preparation
followed by real human source acceptance.
