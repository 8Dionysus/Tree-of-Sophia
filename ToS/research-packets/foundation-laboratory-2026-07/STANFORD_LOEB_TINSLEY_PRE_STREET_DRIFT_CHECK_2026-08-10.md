# Stanford Loeb-Tinsley Pre-Street Metadata Drift Check

Status: ordered currentness check complete 2026-08-10 at 17:59
America/Los_Angeles; anticipated 2026-08-11 street date has not yet begun at
the publisher; metadata conflict has increased; zero book content acquired,
opened, retained, or admitted; zero requests sent

## Question

Did the Loeb-Tinsley *Thus Spoke Zarathustra* advance from a forthcoming
metadata candidate into a released, inspectable reference witness between the
2026-08-08 refresh and the end of 2026-08-10 at Stanford?

## Result

No. The current evidence does not establish a completed release or an exact
inspectable manifestation. It does establish a real metadata drift that the
2026-08-08 record did not contain:

- Stanford's downloadable Spring/Summer 2026 catalog still says **August
  2026**, **424 pages**, and lists cloth, paper, and eBook ISBNs;
- direct access to the current Stanford title and request pages returned a
  Vercel security checkpoint with HTTP 429, so no live publisher-page value
  was inferred from that response;
- the current search-index snippet for the Stanford title page says **552
  Pages**, while the official PDF catalog still says 424;
- Google Books now reports **552 pages** for all three ISBN routes and gives
  `2026 M08 11` for the eBook record;
- Harvard Book Store still labels both print formats **Preorder**, gives
  **August 11, 2026**, and reports **552** pages;
- iMusic now reports **552 pages**, `out of stock`, and the incompatible phrase
  **Released Jul 10**. That combination is treated as vendor-feed conflict,
  not proof of release.

The prior packet's statement that iMusic reported 424 pages is therefore a
dated 2026-08-08 observation, not a current value. The stable reference record
must preserve both observations and refuse silent normalization.

## Research order

### I. Official publisher documentation

The current official [Spring/Summer 2026
catalog](https://supress.sites-pro.stanford.edu/sites/supress/files/media/file/spring26_catalog.pdf)
was retrieved transiently as a 5,446,888-byte PDF with SHA-256
`3b2f5d08cd48a92a01486ebf529c9ba1b6407c73a4e7d561324efc679614075e`.
Its page 3 still gives August 2026, 424 pages, paper ISBN
`9781503647282`, cloth ISBN `9780804728799`, and eBook ISBN
`9781503647299`. The transient PDF was used only to verify current metadata
and was not retained in ToS.

Direct requests to the [official title
page](https://www.sup.org/books/theory-and-philosophy/thus-spoke-zarathustra)
and its [copy-request
route](https://www.sup.org/books/theory-and-philosophy/thus-spoke-zarathustra/desk-examination-copy-requests)
returned HTTP 429 Vercel security checkpoints. ToS did not solve, proxy, or
bypass the checkpoint. The response proves current transport denial from this
client, not absence or any title-page value.

A general-web search performed only after the official, established, registry,
and distribution passes returned a current indexed snippet for the Stanford
title page with `August 2026` and `552 Pages`. That is useful drift evidence,
but it is search-index evidence rather than a publisher response. It therefore
cannot supersede the exact official catalog, prove a production change, or
resolve the extent per manifestation.

The search pass also exposed an official `Excerpts & More` route. It was not
opened: the independent translation lanes remain blind, and existence of a
publisher excerpt surface is not permission for corpus ingestion, model input,
or redistribution.

### II. Established scholarly lineage

The established baseline is unchanged. Loeb and Tinsley's Stanford Complete
Works volumes 14 and 15 cover the notebook period around *Zarathustra*, and
Robin Small's specialist [review of volume
14](https://doi.org/10.5325/jnietstud.51.1.0133) remains direct evidence that
their earlier work entered Nietzsche scholarship. The review is neither a
review of volume 7 nor a transferable quality verdict.

### III. Identifier and catalog registries

Exact Crossref queries for all three ISBNs returned no matching registered
work, and exact Open Library ISBN queries returned empty objects. Absence from
those registries on this date does not prove nonpublication.

The current Google Books HTML records resolve all three ISBNs and report 552
pages. The eBook route identifies `2026 M08 11`; the print routes identify
2026 and link the eBook as another version. The unauthenticated Google Books
API was separately unavailable with an explicit daily-quota HTTP 429, so the
HTML record rather than a fabricated API result is the dated evidence.

Google marks at least one route as limited preview and exposes internal
content-capability flags. No preview page, table of contents, copyright page,
or text was opened or retained. A provider preview signal is not a ToS source
admission or a rights grant.

### IV. Fresh distribution and availability metadata

The current [Harvard Book Store paperback
record](https://www.harvard.com/book/9781503647282) says 552 pages, carries
the exact date 2026-08-11, labels the item `Preorder`, and says publication
dates are subject to change. Its format selector exposes the cloth ISBN with
the same date and preorder state.

The current [iMusic paperback
record](https://imusic.co/books/9781503647282/friedrich-nietzsche-2026-thus-spoke-zarathustra-volume-7-the-complete-works-of-friedrich-nietzsche-paperback-book)
says 552 pages and exposes `out of stock`, while its description says
`Released Jul 10`. Because these values conflict with each other, with the
Stanford catalog, and with Harvard's live preorder state, they are retained as
one vendor-feed anomaly. They do not establish publication, possession, or an
exact July release.

### V. Scholarly index and general web last

An exact OpenAlex search for the title plus both translators returned eleven
loosely related works, including the established Small review, but no record
for a review of the new volume 7. The later exact general-web search surfaced:

- the current Stanford title-page snippet and official excerpts route;
- a 2026-07-30 Progressive Geographies announcement that reproduces the
  publisher description but offers no passage-level or translation review;
- retail/preorder records;
- an Academia.edu result apparently carrying title and book material.

The Academia.edu result was not opened or downloaded. Its authorization,
version, completeness, and relation to the final manifestations are
unresolved, and opening it would contaminate the still-blind translation
lanes. It is rejected as a ToS acquisition route unless an exact authorized
deposit and its use terms are independently established.

No independent post-publication scholarly review was found. This is a dated
negative search result before the anticipated street date, not evidence that
no advance assessment exists.

## Manifestation and rights boundary

| Format | ISBN | Current evidence | ToS state |
| --- | --- | --- | --- |
| cloth | `9780804728799` | official catalog 424; Google Books 552; Harvard preorder for 2026-08-11 | metadata conflict; not acquired |
| paper | `9781503647282` | official catalog 424; Google/Harvard/iMusic 552; Harvard preorder; iMusic self-conflicting release/out-of-stock fields | metadata conflict; not acquired |
| eBook | `9781503647299` | official catalog 424; Google Books 552 and 2026-08-11; exact content not inspected | metadata conflict; not acquired |

The translation and editorial matter remain in copyright. Public metadata,
publisher excerpts, retailer availability, search snippets, previews, and a
third-party upload are separate evidence surfaces. None supplies permission
for ToS to redistribute, publish a derivative, ingest the book into a model,
or reveal it to the blind comparison lanes.

## Decision and next trigger

Keep the stable reference ID
`tos-ref.en.loeb-tinsley-stanford-2026-forthcoming`, update its dated metadata
state, and retain:

- `access_state = metadata-only`;
- `acquisition_state = not-acquired`;
- `content_ingested_for_translation_lab = false`;
- `accepted_as_truth = false`;
- zero human bibliographic and rights reviews;
- zero translation-lane reveals.

The next check remains event-based, not a loop: after 2026-08-11 has actually
begun at Stanford, recheck the publisher state and each ISBN independently.
Even a changed availability label will not create an Item. Exact title and
copyright pages, extent, fixity, lawful custody, permitted research uses, and
passage-level suitability remain separate later gates.
