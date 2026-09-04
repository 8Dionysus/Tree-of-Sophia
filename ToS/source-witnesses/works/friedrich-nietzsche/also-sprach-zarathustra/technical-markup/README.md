# Zarathustra technical markup routes

This directory holds witness-local, non-semantic technical spines. Each route
keeps its own Work/Expression/Edition/Item/File binding, text-layer digest,
unit identities, citation display, boundary method, review status, rights
posture, and authority ceiling.

## Current routes

| Route | Witness-local basis | Current technical surface | Status |
| --- | --- | --- | --- |
| `dta-first-editions-parts-1-4-v1/` | four exact German DTA TEI first-edition items | 5,037 units, 3,447 source-marked paragraphs, 81 reading units, 116 nested sections, 39 verse groups, and 368 verse lines | `agent_verified_complete_technical`; source markup observed, editorial hierarchy unaccepted |
| `antonovsky-1911-pdf-layout-v1/` | exact Antonovsky 1911 RSL/RuNEB PDF | 153 PDF pages, 297 page-side panels, 7,225 layout blocks, 13,071 physical line fragments, and the earlier split-only screening | fixed layout-observation predecessor; paragraph and heading candidate sets are incomplete |
| `antonovsky-1911-structural-paragraph-v2/` | the same exact Antonovsky PDF and frozen bbox layer | 13,071 physical lines, 10,686 logical rows, 81 reading units, 112 numbered subparts, 3,569 prose paragraphs, 13 continuous verse groups, and 359 verse lines | `agent_verified_complete`; full coverage and independent challenger comparison, semantic authority false |

## Parallel posture

The German and Russian spines are now complete at the declared technical
layer and can be addressed independently. The joint closure receipt is
`parallel-technical-readiness.v1.json`. Both witnesses contain four parts and
81 witness-local reading units (`23 / 22 / 16 / 20`), but that equality is a
quality-control observation, not a source-target map. No German unit ID,
ordinal, chapter label, paragraph count, or citation is copied into Russian,
and no Russian unit is asserted to correspond to a German unit.

A later alignment stage must bind exact source and target unit sets, keep
one-to-many, many-to-one, omission, addition, reorder, and competing maps first
class, and enter through the translation-alignment claim and review route. It
must not rewrite either witness-local spine.
