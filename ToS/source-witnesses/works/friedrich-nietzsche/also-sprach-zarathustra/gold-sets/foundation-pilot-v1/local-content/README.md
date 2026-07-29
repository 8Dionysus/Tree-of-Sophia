# Local Content Boundary

This directory is the source-near route for restricted transcriptions,
render crops, blind drafts, comparator text, and review forms. Its contents
other than this route card are ignored by Git.

Tracked files outside this directory may carry IDs, selectors, digests,
methods, rights posture, and review status. They must not carry sampled source
text while the relevant rights record remains unreviewed.

Model-produced transcription belongs under `model-drafts/`. Human pass-one
and pass-two receipts belong under distinct reviewer directories. Moving a
file between those directories does not promote it: `gold-status.json` and a
source-visible review receipt must record the transition.

OCR page renders and their local manifest belong under `ocr/renders/`; raw
variant outputs belong under separate `ocr/variants/<variant-id>/` routes.
The render manifest must freeze page-image digests before any variant runs,
and all A/B/C variants must consume those identical bytes. Reference text
layers stay sealed elsewhere until independent outputs are frozen.

Translation work uses this speaking local topology:

```text
translation/
├── source-review/
│   ├── pass-1/
│   ├── pass-2/
│   └── accepted-source/
├── pre-draft-analysis/
│   ├── human-only/
│   ├── ai-only/
│   └── ai-alternatives/<alternative-id>/
├── blind-drafts/
│   ├── human-only/
│   ├── ai-only/
│   ├── ai-alternatives/<alternative-id>/
│   └── ai-human/
├── comparator/
│   ├── sealed/
│   └── revealed-after-freeze/
├── comparison/
│   ├── alignments/
│   ├── post-reveal-changes/
│   └── preserved-alternatives/
├── adjudication/
└── personal-read-aloud/
```

The directories describe evidence lanes, not promotion states. Human-only and
model analysis must never share a working file before both freeze. Comparator
bytes stay under `sealed/` until the packet records the complete independent
draft freeze set. Personal read-aloud evidence remains separate from
philological adjudication.

Cross-work transfer preparation uses:

```text
transfer-targets/
└── v1/
    └── tos-target-candidate-<work>-p<page>-<stratum>.txt
```

These files are exact page segments emitted by the frozen `pdftotext` route
with the page separator removed. They are private automatic-extraction
candidates, not diplomatic transcription, target gold, or public material.
The tracked transfer plan carries only refs, digests, byte counts, mechanical
selection metrics, proposed anchors, and an explicit ineligible state. This
route opens no routine human work; source-visible target review begins only
after a deliberately activated transfer question.
