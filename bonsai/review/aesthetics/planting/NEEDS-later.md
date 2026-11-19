# Needs from the later direction

The earlier line (`kit.py` at 6c78ee512) admitted only an original public-domain
witness, rejected additional languages and accepted publication through 1929
and deaths through 1942 only. The owner's 12 November brief calls for:

- jurisdiction-specific public-domain and short-quotation bases with evidence
  for each participant, printing and lawful source;
- original plus the reader's language (ru, en, es, de, zh-Hans), including
  source-bound translated places and provenance for modernized Russian;
- a way to record actual, bounded searches without treating an unsearched
  translation as absent, and to admit lawfully sourced whole-sentence quotes.

PR #432 landed these mechanisms on 19 November. Complete source research,
territorial evidence and independent review remain necessary for each packet;
the legacy `owes` result described below is not evidence of that completion.

## Concrete evidence for the implementation

- `akane-ohashi.beyond-the-baby-schema`, DOI 10.1371/journal.pone.0340903:
  publisher JATS carries CC BY 4.0 and the actual target is Abstract / Discussion.
  Model coauthors individually and bind each place to this exact licensed version.
- `hiroshi-nittono.the-power-of-kawaii`, DOI 10.1371/journal.pone.0046362:
  publisher JATS says CC Attribution without a version; Crossref gives BY 4.0
  for an unspecified content version. Preserve the discrepancy for review.
- `howard-phillips-lovecraft.supernatural-horror-in-literature`: the 1927
  *Recluse* pp. 24–25 and Suhrkamp's licensed-access 2017 sample p. 12 provide
  corresponding source places. Its German translation needs short-quotation
  assessment and a page-image transcription, not a claimed open licence.
- `charles-sanders-peirce.what-is-a-sign`: MS 404 (1894) includes sections
  published at different times; the author death year and manuscript date
  alone cannot establish a single US rights basis. Navarra's Rivas translation
  is an institutional web text with section numbers but no printed pages;
  allow explicit structured locators where the source has no pagination.
- `george-santayana.the-sense-of-beauty`: local source E027/E028 point at §11
  in the 1896 edition, pp. 49–50; EU, Russian and Mexican transition rules
  need evidence-bound handling for each version rather than one global basis.

The detailed candidates and remaining searches are in
`bonsai/review/aesthetics/availability/later.json`; `research-pending` is an
unsearched state, never an assertion that a text does not exist.

- `heinrich-woelfflin.kunstgeschichtliche-grundbegriffe`: on 19 November the
  existing maker actually stopped at the 1942 death cutoff (Wölfflin died in
  1945), without preparing places. The owner assigns this work as available;
  assess the actual Russian, EU, US and Mexican rules and participants instead
  of treating the earlier first-wave cutoff as an exhaustive rights judgment.
  Heidelberg's 1915 edition and p. 57 are available at
  https://doi.org/10.11588/diglit.27250#0075; the translation search remains
  incomplete. Its blocked packet is a work-in-progress, not an absence record.

## Legacy packets after #432

On 19 November the new offline gate reports `gate: open, owes 7` for the
unaccepted Pope and Knight packets: older single-basis rights, undocumented
Russian absence/searches and missing edition locator, plus no es/de/zh-Hans
search record. The owner's direction requires real investigation and original
plus each reader language, so these `owes` are not evidence of readiness.
Please provide a maker route to complete legacy-packet rights and witnesses
before second-reader acceptance; `drive.py line --retake` otherwise advances
an already open gate directly to the second reader. Full search evidence is
in the later availability reports; no absence should be inferred.
