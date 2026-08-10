# Antichrist 1906 Wikimedia Commons and Stanford Layered Rights Assessment

Status: model-authored rights research; exact acquired Commons revision and
current provider state reconciled; no legal advice, human rights review,
source-text acceptance, operator transfer approval, or publication decision

Research snapshot: 2026-08-10

## Question

What may Tree of Sophia say about the exact Wikimedia Commons DjVu descended
from the Google/Internet Archive scan of the Stanford copy of *Nietzsche's
Werke*, Erste Abtheilung, Band VIII, C. G. Naumann, Leipzig 1906, when ToS
currently represents only the *Der Antichrist* member on scan pages 228-329?

The file is not one legally or textually homogeneous object. It contains or
represents at least:

1. Nietzsche's underlying *Der Antichrist* Work and other historical Nietzsche
   material not yet cataloged by the partial member map;
2. the historical 1906 Edition typography and presentation;
3. archive editorial selection, arrangement, headings, prefaces, and added
   matter whose exact contributors and scopes are not completely identified;
4. faithful images of historical printed pages;
5. the physical Stanford binding, bookplate, barcode, circulation furniture,
   marks, and other holding-copy matter;
6. automatic OCR stored in 490 compressed `TXTz` chunks inside the exact DjVu;
7. OCR positions, page relationships, and layout structures;
8. the Commons DjVu wrapper and derivative-package choices;
9. the Commons structured file metadata;
10. the Commons unstructured description and revision prose;
11. the contaminated Internet Archive lineage metadata; and
12. the operator's local custody and the future site's independent admission
    policy.

## Result

Nietzsche's represented Work and the source-visible 1906 Edition presentation
are `public_domain_reviewed` for the bounded German and United States routes.
The faithful scan of historical printed pages receives the same result.
Current German institutional guidance and United States Copyright Office
practice converge on the absence of a new copyright in a merely faithful
mechanical reproduction, while the exact Commons object is marked public
domain, `Copyrighted=False`, and attribution-free.

The automatic historical OCR text is also `public_domain_reviewed` as a
mechanical rendering of public-domain material. This decision is restricted
to recognized historical Nietzsche text. The `TXTz` chunks were counted but
not decoded, copied, or accepted. The result proves neither OCR accuracy nor
German correctness, reading order, source authority, member identity, or ToS
text admission.

The archive editorial and added-matter layer remains `conflicting_evidence`.
Any qualifying restored United States term for a contribution published in
1906 would have ended at 2001-12-31, and several German publication-based
related-right routes would also be expired. But the exact volume does not
identify every editor or contributor on the currently reviewed surfaces, and
ToS has not inventoried which portions are Nietzsche text, editorial
arrangement, historical preface, posthumous publication, or another
contribution. A published-1906 United States term result is not converted into
a complete German contributor-term conclusion or applied to unidentified
unpublished or later contributions.

The exact DjVu cannot receive a positive aggregate result. Its source-visible
Stanford binding and holding furniture remain unresolved. No exact operative
license was found for OCR coordinates and arrangement, the mixed DjVu package,
or the Internet Archive database surface. Commons separately applies CC0 to
structured file-namespace data and CC BY-SA 4.0 to unstructured page prose;
those licenses do not clear the file's other layers. Stanford's open-metadata
policy is not applied to marks embedded in page images or to the Commons/IA
package.

The aggregate record therefore becomes `conflicting_evidence`, not
`public_domain_reviewed`, `licensed`, or `in_copyright`. The operator-held DjVu
remains `local_only`, redistribution `not_authorized`, and derivatives
`local_research_only`. A future public object must be independently reacquired
from an exact then-current upstream route and pass fresh fixity, layer, rights,
human-review, and operator-approval gates. It must not be copied from the local
ToS payload.

## Exact objects and current response evidence

| Object or response | Fixity / observation |
| --- | --- |
| exact locally held Commons revision | 24,324,176 bytes; SHA-256 `8f61aaecd55339fc3ba11eca24fbeef85e953d1a262200b36e59d5fbc545ca9d`; Commons SHA-1 `decddcd95661f368fbf725ee855350b9cb3701d2`; 523 DjVu pages |
| exact current Commons API response | 3,474 bytes; SHA-256 `4a779e362406af34d253c12f3f1243c1b42dce3d80bb90acd24106ad7367a63d`; captured 2026-08-10 without a dynamic server-time field; response bytes changed while exact file identity and rights fields remained stable |
| exact Commons file revision | timestamp `2011-09-08T21:13:30Z`; 24,324,176 bytes; SHA-1 `decddcd95661f368fbf725ee855350b9cb3701d2`; uploader `DeirdreAnne` |
| current Commons description revision | revision `904592562`; timestamp `2024-07-29T09:37:56Z`; wikitext uses `{{PD-old-auto-1923|deathyear=1900}}`; exact oldid HTML response 133,891 bytes, SHA-256 `6ffb4b2c6bfae5294d28bb3941368703a6b5011fb30ad35477ebcb7887500fdc` |
| current Internet Archive metadata response | 98,412 bytes; response-specific SHA-256 `b74ba9f8e2b19172f1e0620c97f4efeedd028ecc6880d02d0f6ac2db7544b7c6`; the response includes a dynamic creation field |
| current Internet Archive original PDF | 13,784,419 bytes; SHA-1 `8f4a12d5c97e370c4d2bad7d2fbc6b2d1a47042e`; not the exact acquired Commons DjVu |
| current Internet Archive DjVu XML | 8,882,082 bytes; SHA-1 `4f4efaafa1a5f849aa97a9282805dd335098ed93`; not locally acquired and not the Commons DjVu package |
| exact local DjVu structural check | 523 `Sjbz`, 2,092 `BG44`, and 490 `TXTz` chunk signatures; no OCR body decoded or admitted |

The exact Commons API route is:

`https://commons.wikimedia.org/w/api.php?action=query&format=json&formatversion=2&prop=imageinfo%7Crevisions&titles=File%3ANietzsche%27s_Werke%2C_VIII.djvu&iiprop=url%7Csize%7Csha1%7Cmime%7Ctimestamp%7Cuser%7Cextmetadata&rvprop=ids%7Ctimestamp%7Ccontent&rvslots=main`

The API and current file page report `LicenseShortName=Public domain`,
`UsageTerms=Public domain`, `Copyrighted=False`, and
`AttributionRequired=false`. The rendered provider statement says that the
author died in 1900, so the work is public domain in its country of origin and
life-plus-100-or-shorter jurisdictions, and separately says that the work is
public domain in the United States because it was published before 1931. The
Public Domain Mark records that identification but supplies no warranty and is
not substituted for ToS's layer analysis. In particular, the provider's
pre-1931 sentence is retained as a provider assertion, not adopted as ToS's
independent United States foreign-work term conclusion.

The current Internet Archive record still describes Gustav Siewerth,
Düsseldorf 1971, even though the source-visible object is the Nietzsche/Naumann
1906 Stanford scan. The conflict remains provider metadata contamination. The
record exposes `possible-copyright-status=NOT_IN_COPYRIGHT` and
`copyright-region=US`, but no `rights` or `licenseurl` field. None of its 22
current file entries matches the Commons DjVu by SHA-1 or byte size. No current
IA object is silently substituted for the exact Commons revision.

The 2026-07-30 source-visible inspection remains the bounded physical evidence:

- pages 1-2 show the Stanford binding, bookplate, and barcode
  `36105025673729`;
- page 5 states *Nietzsche's Werke*, Erste Abtheilung, Band VIII, Leipzig,
  C. G. Naumann Verlag, 1906;
- page 6 states `10. und 11. Tausend des Antichrist` separately from print-run
  statements for other volume contents;
- pages 10-12 carry the volume contents;
- scan page 228 begins the *Der Antichrist* preface, page 230 is its internal
  title, page 232 begins numbered section 1, page 329 carries the terminal
  declaration, and page 330 begins different archive-edition matter; and
- pages 517-523 contain publisher advertisements, binding, and Stanford
  circulation furniture.

The partial boundary map remains model-made and unreviewed. Rights analysis
does not upgrade its member identity, expand the unrepresented ranges, or
accept any text.

## Classical and official documentation

### Exact provider surface

The current [Commons file
page](https://commons.wikimedia.org/wiki/File:Nietzsche%27s_Werke,_VIII.djvu),
its permanent description revision, and its API response are the exact object
surfaces. The current wikitext uses
[`PD-old-auto-1923`](https://commons.wikimedia.org/wiki/Template:PD-old-auto-1923)
with Nietzsche's 1900 death year. The rendered statement supplies both a
source-country term route and a United States pre-1931 route. ToS preserves the
provider expression as evidence, not as a worldwide warranty or a blanket
license for separable modern and provider layers.

Commons' current [PD-scan
guidance](https://commons.wikimedia.org/wiki/Commons:When_to_use_the_PD-scan_tag)
distinguishes faithful mechanical scanning and automatic enhancement from
creative selective enhancement. It supports the faithful historical page
layer, not binding design, holding furniture, OCR arrangement, package, or
metadata prose.

The current Commons file-page footer separates provider contributions.
[CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/) governs
structured file-namespace data, while [CC BY-SA
4.0](https://creativecommons.org/licenses/by-sa/4.0/) governs unstructured
text under the stated page terms. These licenses are recorded as separate
metadata layers and are not treated as licenses for the DjVu. Wikimedia's
current [Terms of
Use](https://foundation.wikimedia.org/wiki/Policy:Terms_of_Use) and the
[Public Domain Mark](https://creativecommons.org/publicdomain/mark/1.0/)
provide no exact-object warranty.

### Germany

The current German Copyright Act supplies the bounded route:

- [UrhG §64](https://www.gesetze-im-internet.de/urhg/__64.html) provides the
  ordinary life-plus-seventy term. Nietzsche died in 1900, so that route ended
  at 1970-12-31.
- [§66](https://www.gesetze-im-internet.de/urhg/__66.html) supplies the
  anonymous and pseudonymous publication route. A conservative 1906
  publication calculation for unidentified presentation authorship ended at
  1976-12-31; this calculation does not assert that every editorial
  contribution was anonymous.
- [§70](https://www.gesetze-im-internet.de/urhg/__70.html) provides a
  twenty-five-year related right for a qualifying scientific edition, and
  [§71](https://www.gesetze-im-internet.de/urhg/__71.html) provides a
  twenty-five-year route for first publication of certain posthumous works.
  Those publication-based routes would have ended by 1931 for a 1906 object,
  but they do not identify every contribution in this aggregate.
- [§68](https://www.gesetze-im-internet.de/urhg/__68.html) is kept to its
  statutory scope for reproductions of public-domain visual works and is not
  expanded into a blanket rule for a mixed book, OCR, metadata, or package.
- [§87a](https://www.gesetze-im-internet.de/urhg/__87a.html) keeps database
  investment separate from rights in individual facts and old text.

Deutsche Digitale Bibliothek's current [digital-images
guidance](https://pro.deutsche-digitale-bibliothek.de/daten-liefern/teilnahmekriterien/rechtliches/das-urheberrecht-digitalen-abbildungen)
explains that a faithful reproduction of a public-domain source generally
does not create a new reproduction right. ToS applies that principle only to
faithful images of historical printed pages.

### United States

The United States Copyright Office's April 2026 revision of [Circular
15A](https://www.copyright.gov/circs/circ15a.pdf) states that works published
in the United States before 1 January 1931 are public domain. That domestic
publication boundary cannot by itself decide a German publication whose
United States notice, renewal, nationality, and simultaneous-publication facts
have not been established.

The current [Circular
38B](https://www.copyright.gov/circs/circ38b.pdf) and [17 U.S.C.
§104A](https://www.copyright.gov/title17/92chap1.html#104a) preserve the
foreign-work restoration question. Nietzsche's Work was already public domain
through expiration in Germany on 1 January 1996 and therefore fails the
cumulative source-country-protection condition in §104A(g)(6)(B). The same is
true for the source-visible historical presentation under the assessed German
anonymous-publication and publication-based related-right alternatives, which
ended by 1976-12-31. If a distinct 1906 contribution nevertheless satisfied
all restoration conditions, §104A(a)(1)(B), [17 U.S.C.
§304](https://www.copyright.gov/title17/92chap3.html#304), and Circular 38B
would preserve only the remainder of the ordinary ninety-five-year publication
term, ending at 2001-12-31. That conditional calculation neither invents the
missing restoration facts nor settles an unidentified contributor's German
life-based term, an unpublished contribution, or later added matter.

[17 U.S.C. §§102-103](https://www.copyright.gov/title17/92chap1.html) separate
original derivative or compilation contributions from preexisting material.
The Copyright Office's [Compendium, chapter
300](https://www.copyright.gov/comp3/chap300/ch300-copyrightable-authorship.pdf)
states that merely photocopying, scanning, or digitizing a literary work is a
noncopyrightable mere copy. That supports faithful page images and automatic
historical recognition, not modern furniture, creative correction, prose,
selection, structured arrangement, marks, contracts, or the package as a
whole.

## Established scholarship, cases, and institutional practice

Established sources reinforce separation rather than blanket permission:

- [*Golan v. Holder*, 565 U.S. 302
  (2012)](https://tile.loc.gov/storage-services/service/ll/usrep/usrep565/usrep565302/usrep565302.pdf)
  upheld Congress's foreign-work restoration regime. It confirms why prior
  United States public-domain appearance is not enough: the exact statutory
  restoration eligibility and remaining term still have to be applied.
- [*Bridgeman Art Library v.
  Corel*](https://law.justia.com/cases/federal/district-courts/FSupp2/36/191/2413183/)
  found no United States originality in exact photographic copies of
  public-domain two-dimensional works. It is a district-court decision, not a
  worldwide rule.
- [*Authors Guild v.
  Google*](https://law.justia.com/cases/federal/appellate-courts/ca2/13-4829/13-4829-2015-10-16.html)
  upheld bounded scanning, indexing, search, and snippets as fair use. It is
  not a full-file redistribution license and is not needed to make the
  historical public-domain term calculation.
- Europeana's original public-domain stewardship principle and its current
  [Public Domain
  Charter](https://pro.europeana.eu/post/the-europeana-public-domain-charter)
  state that digitization should not recreate exclusive rights over
  public-domain material. The charter is policy, not this file's contract.
- Stanford's current [copyright
  notice](https://library.stanford.edu/copyright-notice) warns that materials
  can carry third-party copyright, privacy, publicity, or contract limits and
  puts permission analysis on the reuser. Its separate [open-metadata
  policy](https://library.stanford.edu/stanford-university-libraries-open-metadata-policy)
  applies CC0 to Stanford library metadata subject to stated exceptions. ToS
  does not project that metadata policy onto binding images, bookplates,
  barcodes, marks, Commons prose, IA metadata, or the DjVu wrapper.

## Fresh and currently relevant checks

The August 2026 result does not rely only on the 1906 date or the 2011 upload:

1. The exact Commons file remains available at the same SHA-1 and byte size as
   the local fixity-verified DjVu.
2. The current Commons description revision remains `904592562` and uses the
   source-country-plus-US `PD-old-auto-1923` route with death year 1900.
3. Current extmetadata still says public domain, `Copyrighted=False`, and
   `AttributionRequired=false`.
4. Current Commons policy still separates CC0 structured metadata, CC BY-SA
   unstructured prose, and the file's rights statement.
5. The April 2026 Copyright Office duration circular has moved the boundary
   for United States publications to before 1 January 1931; it is not an
   independent foreign-work conclusion for this German 1906 Edition. The
   positive historical-layer route instead applies §104A's source-country
   condition and, where needed, the expired 95-year remainder.
6. Europeana's 2025 charter explicitly carries public-domain stewardship into
   the age of artificial intelligence while preserving other rights and harms.
7. Stanford's November 2025 copyright notice remains narrower than its April
   2025 open-metadata policy; neither is an exact DjVu package license.
8. The current Internet Archive record remains bibliographically contaminated,
   exposes no `rights` or `licenseurl`, and has no file matching the Commons
   DjVu by size or SHA-1.
9. A fresh exact-byte structural check sees 490 `TXTz` chunks but does not
   decode or accept provider OCR.
10. The current German, United States, DDB, Commons, Creative Commons,
    Europeana, Stanford, and exact provider surfaces were checked before the
    general web search.

The 2026-08-10 exact official and institutional checks retained no response
body. Circulars 15A and 38B were respectively 273,867 and 484,230 bytes with
SHA-256 `6ff3c5612e1cd56469b3eb7d35eba7fd3374843d82744c63ab909225f53b7aef`
and `8fa4bea09add40392b090a5816ca328fd5e1d232c4d49bf73df6230e2dbe9947`.
The current Title 17 chapter 1 and chapter 3 HTML responses had SHA-256
`97999d678e2877c8142da81a0dce819b210e9f92aa5a405d130bf2f2e50c7d75`
and `121ed92e20b6fc9325e22a2f0f33cadabc56fda923b06902e8f8372292d0db6d`.
The official *Golan* United States Reports PDF had SHA-256
`5a03252861ef6b7d3bc893e458733fcfeb45ca5d3638723e32b2b06cdab63d69`.
Current DDB, Commons PD-scan, Stanford copyright-notice, and Stanford
open-metadata responses had SHA-256
`05b7a623e9ba59a01a541331eb9089d2189b47931c64d179744591a80d3e9f26`,
`d7c8a97cc6236ad33e7953f56f9a3af83ca8cf58ae4ec316835308664708163e`,
`b6c9887797ba4c0d417166239c8409894cc9a1a727fec28830f5bdc2ff9b2ac8`,
and `f99a5f08275ed7205797b8f36efc18f6adadcf8494fbb2d7a34bbcf2d430dd4f`.

The exact official German responses checked for §§64, 66, 68, 70, 71, and 87a
had SHA-256 values respectively
`41febd024baed1009a7ff5593a80378aa37ed91c74d236f9425c1ed3b415eeff`,
`895acb177f0809f636cb6bc8f326d561ea92c3e5a2f45db86b2a064917ca1062`,
`90c39a74fa158bf561fdf744db542085ad82b81cc00e62d1a8268760191e6b5f`,
`b2f37cc3dd30f94a61df2388620ded4723ea92f757a30c4fa9dd61576b3f430f`,
`c2333e8fa7eeb892f33628b59c13212060cb0d92933e126e00b16ad7c33aacef`,
and `15f33364dbfeb118e20f9a7a22cb3c938b87f992ee929eeee27a3fce12403afd`.
No response body is promoted as a new owner source.

## General web search, last

Only after the exact provider surfaces, current law, institutional practice,
established cases, and fresh checks were complete, general searches were run
for the exact filename, Internet Archive identifier, Google Books identifier,
title, volume, publisher, and year.

The searches returned the same Commons object, its German Wikisource index and
proofread pages for later volume material, ordinary citations, catalogs, and
mirrors. No result supplied a stronger exact-package license, a clean
Internet Archive identity, an editor-term resolution, or an accepted
*Antichrist* text route. Wikisource transcription status is independent of
the embedded automatic OCR and of ToS text acceptance; no source text was
copied.

## Layer decisions

| Layer | Assessment | ToS consequence |
| --- | --- | --- |
| Nietzsche *Der Antichrist* Work | `public_domain_reviewed` in DE/US | historical Work is reusable; no accepted German text is selected |
| 1906 Edition presentation | `public_domain_reviewed` in DE/US | historical typography and presentation are outside the reviewed terms |
| archive editorial and added matter | `conflicting_evidence` | any qualifying restored term for a contribution published in 1906 ended at 2001-12-31, but exact contributor identity, publication state, and German term coverage remain incomplete |
| faithful historical page scan | `public_domain_reviewed` in DE/US | provider statement, German institutional guidance, and the U.S. mere-copy rule converge |
| Stanford binding and holding furniture | `copyright_undetermined` | physical and institutional layers are not flattened into the old printed pages |
| automatic historical OCR text | `public_domain_reviewed` in DE/US | machine recognition adds no authored literary right; coverage, quality, and acceptance remain open |
| OCR coordinates and layout | `copyright_undetermined` | factual or mechanical elements do not settle arrangement, database, or contract posture |
| DjVu derivative package | `copyright_undetermined` | wrapper and derivative choices have no exact package license |
| Commons structured metadata | `licensed` under CC0 1.0 | structured file-namespace data may be reused under its exact route |
| Commons unstructured description | `licensed` under CC BY-SA 4.0 | reuse must follow attribution and share-alike conditions; no DjVu rights are inferred |
| Internet Archive lineage metadata | `copyright_undetermined` | attributed public-safe facts may be recorded; no blanket database extraction is authorized |

## Server consequence

The server-import plan remains `rights-unknown`, `metadata-only`,
`blocked-rights`, and operator-unapproved. Positive DE/US findings for the
historical and faithful mechanical layers do not authorize payload transfer,
server OCR, transcription, page images, snippets, embeddings, alignments,
translations, source-bearing annotations, or content-derived search.

Only public-safe ToS bibliographic, provenance, source-route, layer-status, and
proposed-boundary metadata plus source-attributed licensed Commons metadata may
remain conditional for lexical, search, and graph projections. The proposed
pages 228-329 membership stays visibly unreviewed. No provider row, source
text, page image, OCR coordinate, or local payload is included.

A future exact public candidate must be independently reacquired from the
then-current Commons route, compared by new fixity, and inspected again by
layer. A later ToS-built object might crop unresolved holding furniture and use
only independently cleared historical pages, but that would be a new Item with
new provenance, accepted text and quality decisions, human rights review, and
a separate operator publication decision.

## What this does not prove

This assessment does not prove:

- German orthographic, grammatical, textual, OCR, or reading-order correctness;
- that pages 228-329 are human-accepted membership or an accepted text;
- that the 1906 archive edition is author-final, critical, or preferable to
  the 1888 manuscripts, Overbeck copy, 1895 first print, or a critical edition;
- that every historical editorial contribution has an expired German term;
- that Commons' provider statement is a worldwide warranty or package license;
- that Internet Archive's contaminated record identifies the visible object;
- that Stanford's metadata policy licenses physical furniture or page images;
- that public-domain historical text is an accepted ToS transcription;
- that local possession authorizes publication or server transfer; or
- that a green validator can replace source-visible or human review.

The narrow durable result is a reproducible, eleven-layer rights model whose
positive findings are retained without converting unresolved mixed layers into
permission.
