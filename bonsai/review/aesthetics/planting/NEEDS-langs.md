# Langs: translation route needed

The direction needs the PR1 translation route for the existing accepted works:
add a separately sourced witness for a specified language, cut and align each
existing original place to whole translated sentences, retain the edition,
translator dates, source, page, rights and provenance, compare against the
source, and make a separate second-reader gate possible. Support partial
coverage without presenting it as complete. A recorded exhaustive search must
remain distinguishable from an actual witness and from work not yet searched.

The current kit gate requires `entry.witnesses` to contain only the original
and exposes no translation command (`kit.py --help`; `kit.py` around lines
688–707, at ea79117d3). The line's report already lists this route for PR1.
The langs direction is preparing source-checked language files via direct HTTP
while the route lands; genuinely browser-only lookups are queued for Claude.

## Scan corrections in translation witnesses

PR #432 at `4a045a07` requires `witness-cut` and `witness-cut-candidate`
to find the final translated text verbatim in the fetched transcription. Some
legal transcriptions differ from the printed page we have checked. For example,
Halsted’s Poincaré has “guiding; And” in Gutenberg #39713 but the 1921 scan
reads “guiding. And” on p. 391, so the existing sentence ends after “guiding.”
Please provide an auditable page-image correction route for witnesses (record
the original cut, exact correction, justification and source page, and bind
second review to the corrected text), analogous to the original packet’s
correction route. Do not silently treat a corrected local file as unmodified
remote source. Existing candidate files record such discrepancies explicitly.
