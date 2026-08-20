# Source Discovery Protocol

## Purpose

Find the correct material and preserve why it was selected without confusing
a search result, repository badge, filename, or download button with
bibliographic identity or permission.

The required chain is:

```text
search lead
  -> originating bibliographic or authority record
    -> digital-object record
      -> work/expression/edition/item reconciliation
        -> jurisdiction-aware rights assessment
          -> lawful access or written request
            -> immutable acquisition receipt
```

## 1. Freeze the target

Before searching, record:

- requested work, expression, language, translator/editor, edition, date, and
  format, including every uncertainty;
- why this object is needed and which experiment or corpus gap it serves;
- acceptable substitutes and properties that may not be substituted;
- expected rights and access questions without turning them into conclusions.

## 2. Search in source order

Use the first applicable channels in this order:

1. national bibliographies, authority files, and originating library records;
2. university and holding-library catalogs;
3. publisher and critical-edition catalogs;
4. Nietzsche Source and other domain scholarly projects;
5. Europeana, IIIF collection catalogs, and repository object records;
6. Crossref, DataCite, OpenAlex, and other identifier indexes;
7. Internet Archive, Wikisource, Gutenberg, and other open-library catalogs;
8. KVK, WorldCat, Google Books, and similar aggregators as leads whose
   originating records must still be captured;
9. ordinary web search only to close a named unresolved gap.

Zotero translators may extract metadata, but the exact translator version and
the originating record remain part of the receipt. A search engine result page
is never the bibliographic authority.

## 3. Preserve each query

For every channel record:

- exact query string and parameters;
- execution date and timezone;
- channel name, endpoint URL, interface type, and API/version when available;
- result order, including zero-result queries;
- each result URL and the originating-record URL when different;
- bibliographic identifiers and the format actually offered;
- declared rights statement, its scope, evidence URL, and uncertainty;
- access state and machine-interface availability;
- selection, rejection, or deferral with a reason;
- elapsed human and machine time.

If a result is downloaded, preserve byte size, SHA-256, acquisition time,
source URL, and provenance event. If a public page is volatile and its terms
permit preservation, store a digest-addressed snapshot or WARC/WACZ receipt;
otherwise record `not-captured` and why. Do not bypass access controls.

## 4. Reconcile identity

Resolve the candidate through `work -> expression -> edition -> item -> file`.
Check responsibility, language, title variants, publisher, date, place,
edition statement, container membership, pagination, identifiers, and source
provenance separately. Conflicts remain explicit claims; similarity does not
authorize merging.

## 5. Assess rights separately

Record metadata rights, source-text rights, scan or digital-object rights,
translation rights, derivative rights, server-processing permission, and
redistribution permission separately for the jurisdictions actually reviewed.

Availability, age, a public-domain badge, and an open API are evidence, not a
global legal conclusion. Unknown or conflicting rights route to local-only
handling and, where useful, an access request.

Do not turn that caution into a blanket closed-corpus assumption. Many works,
editions, transcriptions, scans, and repository objects are public domain or
licensed for free, noncommercial, attribution-bound, or broader
redistribution. Capture the exact statement and terms, the object and layer
they cover, jurisdiction, commercial/noncommercial boundary, attribution and
share-alike duties, modification limits, source URL, access date, and review
status. When the evidence affirmatively permits the intended use, route that
exact material toward the matching public access class instead of leaving it
closed by inertia.

## 6. Compare channels

After the run, compare channels without hiding zero results:

- completeness for the frozen target;
- metadata precision and originating-record traceability;
- rights clarity;
- human and machine time;
- machine-interface stability;
- ability to preserve a reproducible receipt.

The best discovery channel is the one that most reliably closes identity and
rights questions for the task, not the one that returns the most links.

## 7. Refresh unstable evidence

Refresh live entitlement, price, availability, API version, title list,
license, terms, and contact routes immediately before acquisition or request.
The previous receipt remains; the refresh supersedes it rather than rewriting
history.

## Current limitation

The July 2026 foundation research identified and triaged source channels, but
the earliest web searches predated this run-receipt contract. Their exact query
order is therefore not reconstructed after the fact. The research packet and
translation reference register remain valid source summaries, while future
searches must use this protocol from their first query.
