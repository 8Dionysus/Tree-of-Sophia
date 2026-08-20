# Stanford Loeb-Tinsley Post-Street Metadata Check

Status: ordered post-street metadata check complete 2026-08-11 at 00:16
America/Los_Angeles; publisher-authenticated release remains unresolved;
distribution metadata changed but still conflicts; zero book payloads acquired;
zero requests sent

## Question

After 2026-08-11 began at Stanford, did the Loeb-Tinsley *Thus Spoke
Zarathustra* become an exact, lawfully held, rights-assessed, and suitable
reference manifestation for the ToS translation laboratory?

## Result

No. The calendar trigger fired, but the evidence gates did not collapse into
one release claim.

- Direct requests to the current Stanford title page and copy-request page
  still returned HTTP 429 Vercel security checkpoints. No checkpoint was
  solved, proxied, or bypassed.
- Stanford's exact 5,446,888-byte Spring/Summer 2026 catalog remains
  byte-identical to the 2026-08-10 check, SHA-256
  `3b2f5d08cd48a92a01486ebf529c9ba1b6407c73a4e7d561324efc679614075e`.
  It still says August 2026 and 424 pages for the announced cloth, paper, and
  eBook formats.
- The currently indexed Stanford title record still says August 2026 and 424
  pages, but the index reports an old crawl. It is not a current publisher
  response and cannot prove post-street release.
- Harvard Book Store's exact paper and cloth records now expose an add-to-cart
  form, the date 2026-08-11, and 552 pages. This is a real distribution-state
  signal, but it is not publisher authentication, physical possession, or an
  inspected manifestation.
- Other current print channels remain contradictory. The exact iMusic cloth
  route still says preorder, 424 pages, and expected delivery on August
  19-24. Its US paper route exposes `out of stock`, 552 pages, and the
  incompatible date July 10. Shakespeare & Company and UK bookseller records
  still describe print as preorder or coming soon on the date itself.
- The Google Books eBook metadata record gives 2026-08-11 and 552 pages.
  Crossref and Open Library still return zero exact records for the three
  ISBNs, while the unauthenticated Google Books API remains quota-blocked.

The correct state is therefore **post-street metadata-only, unresolved**. It
is stronger than the pre-street state because at least one exact retailer now
accepts cart addition and the eBook metadata date has arrived. It is weaker
than an exact manifestation claim because no publisher-authenticated release,
custody, title page, copyright page, pagination, byte fixity, lawful research
permission, or passage-level suitability has been established.

## Research order

### I. Official publisher documentation

The exact [Stanford title
page](https://www.sup.org/books/theory-and-philosophy/thus-spoke-zarathustra)
and [copy-request
page](https://www.sup.org/books/theory-and-philosophy/thus-spoke-zarathustra/desk-examination-copy-requests)
both returned 429 responses at the post-street check. The response proves
transport denial from this client, not release or nonrelease.

The exact [Spring/Summer 2026
catalog](https://supress.sites-pro.stanford.edu/sites/supress/files/media/file/spring26_catalog.pdf)
returned HTTP 200, the same byte size, and the same SHA-256 as on 2026-08-10.
Its dated announcement remains authoritative for what that catalog says, not
for whether production metadata later changed.

Search-index access to the title record remains only cached evidence. No
publisher description, endorsement, excerpt, review-copy content, or book
content was copied into ToS.

### II. Established scholarly lineage

The established lineage remains unchanged. Loeb and Tinsley's earlier
Complete Works volume 14 has a specialist review, but that review is neither
reception nor a transferable quality verdict for volume 7. The new volume's
announced notes, notebook cross-references, afterword, and key-term glossary
remain reasons to examine it later, not proof of accuracy or superiority.

### III. Identifier registries

The three exact ISBNs remain distinct:

| Format | ISBN | Post-street registry state |
| --- | --- | --- |
| cloth | `9780804728799` | Google/provider records disagree at 424 and 552 pages; no Crossref or Open Library record |
| paper | `9781503647282` | distribution records disagree at 424 and 552 pages; no Crossref or Open Library record |
| eBook | `9781503647299` | Google Books metadata gives 2026-08-11 and 552 pages; no Crossref or Open Library record |

No Google preview page was deliberately opened. The final general search
response nevertheless emitted an unsolicited metadata-adjacent contents and
common-terms snippet for the eBook record. That material is not quoted,
retained, admitted, or used. This research context must not adjudicate a later
blind translation candidate. The already frozen translation candidates
precede this exposure.

### IV. Fresh distribution state

The exact [Harvard paper
record](https://www.harvard.com/book/9781503647282) and its cloth selector now
show 2026-08-11, 552 pages, and an add-to-cart form. That is a stronger current
availability signal than the prior preorder label.

It is not a clean release proof. The exact [iMusic paper
record](https://imusic.co/books/9781503647282/friedrich-nietzsche-2026-thus-spoke-zarathustra-volume-7-the-complete-works-of-friedrich-nietzsche-paperback-book)
still exposes internally incompatible stock and date metadata. The exact
[iMusic cloth
record](https://imusic.co/books/9780804728799/friedrich-nietzsche-2026-thus-spoke-zarathustra-volume-7-the-complete-works-of-friedrich-nietzsche-hardcover-book)
still says preorder, 424 pages, and expected delivery after the nominal date.
Other current print routes likewise remain preorder or coming soon.

These are vendor and distributor facts about their current surfaces. They do
not establish that one physical copy exists, that the eBook bytes correspond
to the print pagination, or that any exact format has been lawfully acquired.

### V. Scholarly index and general web last

The exact title-plus-translators review search still returned the publisher
record, prior-volume lineage, announcements, stores, and third-party leads,
but no independently authenticated scholarly review of volume 7. This is a
dated negative result within the tested channels, not a claim that no review
exists anywhere.

No third-party upload was opened. No book preview, excerpt route, table of
contents page, copyright page, or passage was deliberately requested.

## Rights and suitability boundary

The translation, editorial notes, afterword, glossary, and digital
presentation are in-copyright surfaces unless exact evidence proves a narrower
permission. A sale, cart button, preview capability, review-copy route, public
metadata page, or exact publication date does not authorize redistribution,
model ingestion, derivative publication, or server upload.

The volume is promising for later comparison because its announced method
addresses wording, wordplay, notebook variants, and key terminology. That is
publisher-supplied suitability metadata only. Suitability for ToS requires an
exact lawfully held manifestation, separate rights review, content-isolated
inspection after independent lanes freeze, and passage-level comparison.

## Decision and next trigger

Keep the stable historical reference ID
`tos-ref.en.loeb-tinsley-stanford-2026-forthcoming`, but change its dated state
to `post-street-metadata-only-unresolved`. The suffix is continuity, not a
current release assertion.

Retain:

- `access_state = metadata-only`;
- `acquisition_state = not-acquired`;
- `content_ingested_for_translation_lab = false`;
- `accepted_as_truth = false`;
- zero human bibliographic or rights reviews;
- zero translation-lane reveals;
- zero requests sent.

The next trigger is evidence-based, not another date loop: a current
publisher response or an exact lawfully held manifestation. Only then inspect
title and copyright pages, manifestation-specific extent, fixity, custody,
permitted research uses, and passage-level suitability. A request route may be
prepared later, but sending remains a separate operator action.
