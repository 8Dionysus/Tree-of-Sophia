# Antonovsky 1911 technical-screening note — 2026-09-01

## Scope inspected

- Exact local Antonovsky 1911 RSL/RuNEB PDF bound to SHA-256
  `57518b50e24fee37b3e9c151e853c36ad34258f51a8b65b8233742d69017db69`.
- All 86 rows of the high-precision typographic heading-candidate projection,
  each inspected against a rendered source crop with surrounding layout.
- A 42-block region-stratified source-visible sample of repeated-indent split
  candidates, including single-split and multi-split pressure.

## Outcome

The source-visible model screening retained every heading observation and
assigned technical roles: 57 reading-unit heading candidates, 17 numbered
subsection markers, four work/part-title elements, three front-matter elements,
and five publisher-catalog headings.

The initial indent rule exposed two false-positive classes: front-matter
reference material and verse-line indentation. The bounded successor rule
excludes front/back matter and rejects a block when at least five lines are
present and more than 60 percent of possible internal boundaries match the
indent band. The resulting overlay records 1,372 proposed boundaries in 692
existing Poppler blocks, yielding 2,064 exact spans and an effective 8,597
paragraph candidates when inherited unsplit blocks are included.

## Authority and current home

This is `model_source_visible` screening evidence, not a `real_human` review.
It does not populate the source-text-unit packet `reviews[]`, accept headings
or paragraphs, replace existing block units, issue successor identities,
alter source bytes, or create DE↔RU alignment or semantics.
The 86-row high-precision set remains recall-incomplete: complete source
heading coverage has not been claimed.

Current technical authority remains under
`ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/technical-markup/antonovsky-1911-pdf-layout-v1/`.
The next owner is a competent real-human Russian source reviewer who can accept,
reject, or record competing boundaries before alignment claims are opened.
