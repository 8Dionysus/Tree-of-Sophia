# Antonovsky 1900 LNB Holding Research

Date of evidence refresh: 2026-08-01
Scope: the exact Saint Petersburg 1900 edition of *Thus Spoke Zarathustra*
translated by Yu. M. Antonovsky, its current Lithuanian National Library
catalog identity and physical holding, the absence of a digital source, and a
lawful route for requesting a research reproduction
Authority posture: model-made bibliographic research over current official
records and supporting scholarship; no source-page, human bibliography,
collation, translation-quality, semantic, rights, or canon review

## Result first

The missing 1900 bibliographic endpoint is now institutionally identified.
The Lithuanian National Library's current iBiblioteka record identifies an
exact printed book:

- *Так говорил Заратустра: книга для всех и ни для кого*;
- Friedrich Nietzsche, translated from German by Yu. M. Antonovsky;
- Saint Petersburg, 1900;
- publisher recorded as `[s.n.]`;
- physical extent `[3], XI, 624 p.`;
- iBiblioteka code `C10000469160`, internal record `4970734`;
- one copy held by the Martynas Mažvydas National Library of Lithuania,
  General Collections (old), call number `1.40686`.

This closes the exact Edition identity that the previous pass could only infer
from Antonovsky's 1911 preface. It does **not** close the source-witness route.
The public record carries no attachments, page images, electronic-resource
link, content licence, or downloadable media. Its exemplar endpoint reports a
physical holding, not an Item acquired by ToS. Consequently this pass creates
a provisional Edition and one unreviewed `Expression -> Edition`
bibliographic-topology claim, but no ToS Item, File, title-page anchor, source
text, publisher claim, derivation edge, equivalence claim, or rights clearance.

A public-safe access-request card is prepared for the exact call number. It is
`draft-not-sent`; no contact, order, payment, permission acceptance, or
external upload has occurred.

## 1. Classical and official documentation

### 1.1 Exact national-library record

The [iBiblioteka public record](https://ibiblioteka.lt/metis/publication/C10000469160)
and its current machine-readable public API endpoint identify record
`4970734` / `C10000469160`. The API exposes the following UNIMARC evidence:

- field `200`: title, subtitle, Nietzsche responsibility, and the literal
  statement `перевод с немецкого Ю.М. Антоновского`;
- field `210`: `Санкт-Петербург`, `[s.n.]`, `1900`;
- field `215`: `[3] XI, 624 p.`;
- field `101`: Russian, translated from German;
- field `005`: record transaction timestamp `20250416091534.4`;
- field `801`: Lithuanian catalog agency `C1`, record date `20060123`;
- field `990`: holding code `C1/BFS` and call number `1.40686`.

The library's own 2024 [Nietzsche bibliography](https://lnb.lt/media/public/naujienos/F._Ny%C4%8D%C4%97._Literat%C5%ABros_s%C4%85ra%C5%A1as_2024_10.pdf)
independently prints the same title, Antonovsky responsibility, Saint
Petersburg, 1900, unknown publisher, and extent. Agreement between the public
catalog, its UNIMARC rendering, the holdings endpoint, and the library's
curated bibliography is enough to materialize a bounded bibliographic
Edition identity. It is not a substitute for inspection of the book.

`[s.n.]` is retained as an explicit unknown publisher. It is not a publisher
name, and it does not authorize filling the field from a later bibliography or
general-web assertion.

### 1.2 Exact physical holding without a ToS Item

The current public exemplar query returns exactly one holding:

- institution: `Lietuvos nacionalinė Martyno Mažvydo biblioteka`;
- department: `Bendrieji fondai (senasis)`;
- call number: `1.40686`;
- issue condition: `IN_PLACE`;
- total copies in that fund: one;
- currently available copies: one;
- reader-visible/orderable through this endpoint: false.

This is evidence that the institution describes a concrete physical copy. The
ToS `Item` contract, however, describes a copy or container **as acquired** and
requires payload bytes, a manifest, fixity, provenance, rights, and a resource
inventory. Creating an empty Item from remote holdings metadata would make the
identity ladder look complete while removing its evidence floor. The call
number therefore remains a discovery and request identifier until ToS lawfully
receives an exact reproduction or records a separately designed external-
holding class.

### 1.3 Current lawful-copy route

The National Library's 2026 [rules for use](https://www.lnb.lt/media/public/english/pdf/rules_for_the_use_of_national_library_of_lithuania_2026.pdf)
state that good-quality copies from specified historic collections can be
ordered through a prescribed request and supplied for a fee when the document
condition and rights allow it. The current [request form](https://www.lnb.lt/media/public/english/pdf/Annex_4_2024.pdf)
asks for the call number, title, page range, and desired reproduction format.
The library's [ordering guidance](https://lnb.lt/en/services/for-visitor/ordering-and-collecting-orders)
routes collection questions through its institutional service.

The prepared request is deliberately tiered: first confirm the exact
department, call number, physical condition, copying eligibility, terms, and
price; then request title and provision pages plus preliminaries; finally seek
a complete research reproduction if lawful and practicable. Local OCR,
indexing, embeddings, limited quotation, and public bibliographic metadata are
asked separately. Source redistribution and derivative publication are not
requested.

### 1.4 Official negative controls

An exact DNB SRU query for `Antonovskij` and *Tak govoril Zaratustra* returned
nine records. Direct MARC21 extraction showed publication years from 1996 to
2019; no 1900 record was present. DNB therefore corroborates the translator
authority route for later editions but does not identify the target Edition.

The [RuNEB 1900 record](https://rusneb.ru/catalog/000201_000010_REDKOSTJ34077/)
is an important false positive: it is a Moscow, D. P. Efimov, 339-page volume
translated by A. V. Perelygina. Matching author, title, and year are not enough.
It is rejected as evidence for the Antonovsky 1900 Edition. The result becomes
a durable identity control for future automated discovery.

## 2. Established scholarship

Pertsev's 2015 peer-reviewed [study of Russian *Zarathustra*
translations](https://doi.org/10.17072/2078-7898/2015-3-24-31) remains the
methodological control: philosophical meaning, style, rhythm, and translation
strategy must be compared between exact witnesses. A translator name and
matching title do not establish that two editions carry the same text.

Antonovsky's exact 1911 preface remains the primary lineage report. It counts
the 1900, 1903, 1907, and 1911 editions and says the first three successors
were reworkings rather than simple reprints. The LNB record now supplies the
missing bibliographic endpoint for the first of those dates; it does not add a
textual relationship to 1898 or 1899.

## 3. Fresh and current evidence

Ermakova's 2025 [variant-aware translation study](https://publications.hse.ru/en/articles/1061726305)
confirms the present need for exact witness comparison rather than
translator-level text collapse. The current iBiblioteka record exposes a 2025
transaction timestamp, and the National Library's 2026 use rules were checked
on the actual execution date. Freshness here strengthens the live catalog and
request route; it does not make the historical metadata source-visible.

The public API currently reports no attachments, electronic media, download,
or content licence for record `4970734`. Searches of ePaveldas/Europeana,
Internet Archive, Google-indexed books, DNB, and other open routes found no
complete exact digital copy. That negative result is time-sensitive and must
be rerun before sending or acquiring.

## 4. General web last

General web search was used only after the official, established, and current
routes. One circulating bibliography-like PDF attributes the 624-page 1900
book to the B. M. Wolf printing house. The claim is plausible but its source
chain is not exposed, while the current national-library record explicitly
keeps the publisher unknown. No `B. M. Wolf` publisher or printer claim is
therefore admitted.

A source-visible title page or a stronger originating catalog statement may
later distinguish publisher, printer, and distributor. Until then, even the
role is unresolved; a printing house must not be silently normalized into a
publisher.

## Materialized boundary

| Layer | Result now | Deliberately absent |
| --- | --- | --- |
| Work | existing *Also sprach Zarathustra* Work | no new Work identity |
| Expression | existing provisional Antonovsky 1900 Expression receives current source return and one Edition link | no accepted text, responsibility claim, derivation from 1898/1899, or equivalence |
| Edition | provisional Saint Petersburg 1900 Edition, `[s.n.]`, `[3], XI, 624 p.`, external record IDs | no source-visible title page, edition-number statement, publisher/printer claim, or human verification |
| external holding | LNB call number `1.40686`, one physical copy | no ToS Item and no claim that the holding has been acquired |
| File/source | none | no bytes, fixity, page map, OCR, transcription, or source anchors |
| Rights | metadata used as discovery evidence; request route identified | no payload rights, redistribution permission, or publication permission |
| Relation | one unreviewed `embodied_by` bibliographic-topology claim | no Edition-to-external-holding edge and no new Expression derivation/equivalence edge |

## Rights and publication boundary

The historical Work and translation age do not settle the reproduction rights,
institutional terms, editorial state, or future-site publication of a copy.
The exact source file, if supplied, will remain local and gitignored unless an
affirmative object- and layer-specific permission or public-domain/open-license
route is demonstrated. The public site may expose permitted bibliographic
metadata and provenance without exposing source bytes.

The request asks what the institution permits; it does not pre-accept unknown
terms. Any reply, price, licence, personal data, or attachment belongs in the
ignored private correspondence lane. A redacted public receipt may be created
only after an actual response.

## Next source-grounded move

Before any send, a human must inspect the exact draft, desired budget, and
scope. If approved, refresh the record, call number, 2026 rules, request form,
and institutional route, then send once. On receipt:

1. preserve the original bytes locally and compute fixity;
2. create the exact Item/File/rights/provenance route;
3. visually inspect title, verso, provision, contents, and terminal pages;
4. resolve whether `B. M. Wolf` appears and in which role;
5. build page-level return before OCR;
6. collate bounded samples against 1898, 1899, 1903, 1907, and 1911 only when
   those exact witnesses exist;
7. keep every new textual relation and rights conclusion reviewable and
   reversible.

No further chronology or semantic graph edge should be inferred merely because
the 1900 Edition now has a stable catalog identity.
