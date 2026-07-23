# Structure Recovery Report: Foundation Pilot v1

Status: executed baseline, awaiting real human review
Experiment: `tos-structure-recovery-v1`, variant A
Frozen sample: 36 units, 12 per source item
Execution date: 2026-07-22

## Result first

The deterministic native route is useful as a fast container and text-layer
baseline, but it is not a general transcription method.

- It resolved all 36 tracked anchors mechanically.
- It produced nonempty text for the 12 ABBYY-PDF units and 12 auto-EPUB
  members.
- It produced no text for all 12 Antonovsky outline-PDF units even though the
  inspected pages visibly contain text.
- It inherited severe OCR defects from difficult ABBYY and auto-EPUB source
  layers.
- The first implementation also copied XHTML `<title>` text into the body.
  Source-visible inspection found this defect; that run was retained, the
  parser was corrected, and a separate run confirmed its removal.

No structural F1, text accuracy, or human correction cost is reported because
the required double-source-visible human gold does not yet exist. The result is
not promoted into ToS text, semantics, or canon.

## Preserved runs

Runtime root:
`/srv/abyss-machine/storage/artifacts/tree-of-sophia-foundation-lab/`

| Run ID | Runner digest | Wall time | Peak RSS | Derived bytes at execution | Outcome |
| --- | --- | ---: | ---: | ---: | --- |
| `foundation-pilot-v1-structure-a-20260722` | `3517f8f79214069538acfb28640e31a6da4136de906dc8827de5fbe1bc8bb34a` | 1.148 s | 32,759,808 | 172,027 | retained negative run; XHTML head title contamination |
| `foundation-pilot-v1-structure-a-headfix-20260722` | `1a2b6d07ed0340f9c60e4c7cf7665d1e12bcf93b28f816a04455e29215b3f6cb` | 1.228 s | 32,632,832 | 172,489 | corrected candidate, still awaiting human review |

The small timing difference is not interpreted as a performance change. Both
runs processed the same 36 frozen units and remained under the current
`abyss-machine` medium/indexing route. The host owner admitted the work in its
normal observed thermal policy; no local 90 °C stop rule was invented.

## Source-visible checks actually performed

The following checks compared the actual output to a rendered page or source
XHTML member. Their receipts identify the maker as `model:codex`, carry
`advisory-nonhuman`, and explicitly forbid promotion.

| Source | Inspected units | Concrete finding |
| --- | --- | --- |
| Antonovsky outline PDF | pages 11 and 50 in the first run; page 11 again after the fix | catastrophic total omission from visibly nonempty pages |
| Mysl 1996 ABBYY PDF | pages 4, 8, and 237; pages 8 and 237 again after the fix | some pages preserve layout well; page 8 contains severe inherited OCR corruption |
| Naumann 1893 auto-EPUB | members 55, 59, 380, and 522; members 59 and 380 after the fix | first run added head titles; corrected run preserves body order but inherits OCR/furniture and flattened structure |

Examples of defects observed in the real ABBYY page-8 output include
`эаратустра`, `выттте^сейя`, `челов'ек`, `выше,чшШысл`, and
`и^сіИокость`. Examples already present in the auto-EPUB witness include
`vop`, `weiche`, `gTösste`, stray separators, and Google/Cornell scan
furniture. These examples diagnose the source and method boundary; they are
not a corrected transcription.

Human manual-review receipts: **0**. The laboratory contract now requires
`reviewer_type: human` and explicit human-presence attestation, so a model
inspection cannot be mislabeled as the required manual pass.

## Decisions

1. Reject the first runner revision as a structure candidate because it
   introduced body additions from XHTML `<head>`.
2. Retain that rejected run as negative evidence rather than overwriting it.
3. Accept the corrected native route only with limits for container inventory,
   mechanical anchor maps, EPUB body extraction, and selected readable ABBYY
   pages.
4. Reject native extraction as text recovery for the Antonovsky outline PDF.
5. Do not treat ABBYY or auto-EPUB output as clean German or Russian text merely
   because it is nonempty.
6. Keep all 15 gold candidates unaccepted until two real source-visible human
   passes are recorded.
7. Route the Antonovsky pages into the OCR A/B/C comparison; route structure B
   and C only after their setup, license, storage, and host-resource gates pass.

## What remains unknown

- exact CER/WER and structural F1;
- exhaustive punctuation, glyph, and line-boundary accuracy;
- correction minutes and therefore real human cost;
- whether Docling or a document VLM improves structure without introducing
  less visible errors;
- whether a clean scan-derived German witness will disagree materially with
  the provided auto-EPUB text.

Those unknowns are blockers to content acceptance, not reasons to erase the
executed baseline.
