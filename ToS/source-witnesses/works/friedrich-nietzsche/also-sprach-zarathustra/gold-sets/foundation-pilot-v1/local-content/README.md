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

Whole-work lexical observation uses:

```text
lexical-search/
└── zarathustra-dta-first-editions-parts-1-4-v1.sqlite3

morphology/
└── zarathustra-dta-exact-form-census-v1.jsonl

semantic-source-observation/
└── initial-v1/
    ├── diplomatic-form.txt
    ├── normalized-form.txt
    ├── context-bundle.json
    └── work-recurrence-bundle.json
```

This private SQLite/FTS5 file may contain exact forms, page-local token order,
source context, structural filters, and opaque occurrence locators. The
tracked companion must remain non-sequential and source-string-free. Neither
the local database nor its tracked receipt accepts German, clears rights, or
creates a lexeme, lemma, translation correspondence, sign candidate, or sign.

The morphology packet is a deterministic one-row-per-exact-form projection of
that database. It keeps exact strings local, preserves the historical surface,
and supplies only the admitted DWDSmor A census. It contains no provider
output, accepted lemma, lexeme, German judgment, context sample, or scheduled
human work. Its tracked companion carries only digests and aggregate counts.

The semantic source-observation branch contains the first packet-local
exact-form selection and its complete context census inside the admitted
Edition section. It is narrower than the whole-work lexical database and does
not reuse the Work-identity context control as a sign. Exact source values stay
mode `0600`; the tracked packet carries only hashes, counts, opaque occurrence
IDs and text-free TEI source-return selectors. This branch creates no accepted
German, morphology, lemma, translation, sign candidate, semantic claim or
human backlog. Its later work-recurrence bundle returns the same selected form
to all 145 exact occurrences across the four admitted DTA Items. Every raw TEI
offset is checked, but occurrence positions and source values remain private;
the tracked receipt carries only fixity, aggregate distribution, part counts,
and an unchanged-packet/no-promotion boundary.

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
