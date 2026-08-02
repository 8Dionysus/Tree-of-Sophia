# Ecce Homo 1908 Commons and Getty Layered Rights Assessment

Status: model-authored rights research; exact acquired Commons revision,
current provider state, contributor terms, and source-visible design checked;
no legal advice, human legal review, source-text acceptance, operator transfer
approval, or publication decision

Research snapshot: 2026-08-02

## Question

What may Tree of Sophia say about the exact Wikimedia Commons PDF derived from
the Internet Archive scan of Getty Research Institute copy 965 of the 1908
Insel first edition of Friedrich Nietzsche's *Ecce homo*?

The file is not one legally homogeneous object. The review first separates
twelve analytical concerns:

1. Nietzsche's underlying Work;
2. Raoul Richter's editorial contribution;
3. Richter's separately titled editor's afterword;
4. Henry van de Velde's binding, double title, lettering, and ornaments;
5. faithful captures of historical book pages;
6. Getty holding furniture, including a barcode and label;
7. automatically recognized text embedded in the PDF;
8. OCR positions, page layout, and an Internet Archive PDF derivative package;
9. the exact Commons file revision and provider statement;
10. Commons structured file metadata;
11. Commons unstructured description prose; and
12. the current Internet Archive lineage metadata and byte-divergent PDF.

They are normalized below into eleven machine-readable decisions: OCR
positions, page layout, and the Internet Archive derivative package remain one
unresolved lineage-package decision, while the exact Commons PDF has its own
aggregate decision so that a positive sublayer cannot silently authorize the
mixed object.

## Result

Nietzsche's Work is `public_domain_reviewed` in the bounded German and United
States routes. Richter died in 1912, so both his independently authored
afterword and any copyright-capable creative editorial contribution are also
outside the reviewed German and United States terms. A qualifying German
scientific-edition right would have expired still earlier. These positive
results do not make the 1908 text author-final or philologically accepted.

The exact Edition also contains a different and still active German layer.
The source-visible colophon credits the title, binding, and ornaments to Henry
van de Velde. The front binding and two-leaf title are visibly designed rather
than mechanically typeset, and the colophon extends that credit to ornaments
through the Edition. Van de Velde died on 25 October 1957. Under the current
German ordinary life-plus-seventy rule and calendar-year calculation, the
conservative ToS term for those applied-art contributions remains active
through 2027-12-31. The same 1908 publication is public domain under the
current United States pre-1931 boundary.

Current European Union law does not permit ToS to dismiss the design merely
because it is applied art. The Court of Justice's December 2025 joined-cases
judgment says that applied art uses the same originality requirements as
other works and that protection depends on identifiable expressions of free
and creative choices. It also says creativity cannot simply be presumed. ToS
does not adjudicate originality, but the named, source-visible ornamental
composition and current active term require fail-closed `in_copyright`
treatment in Germany in the absence of an exact license or permission.

The current Commons wikitext still uses only `PD-US-expired`. That template is
positive United States evidence and explicitly warns that a non-United-States
work needs a source-country basis. It does not clear the active German design
layer. Commons' current extmetadata reports `Public domain` and
`Copyrighted=False`, but those provider fields do not erase the missing German
basis or create a warranty.

Getty's current Open Content Program is exact-record gated: its CC0 route
applies when a Getty image is actually marked as Open Content. No exact Getty
record marking this scanned book or PDF CC0 was found. Getty's contribution
and sponsorship in Internet Archive therefore cannot be converted into a CC0
grant for this object. Internet Archive currently supplies neither a `rights`
nor a `licenseurl` value for the Item.

The aggregate exact PDF therefore becomes `in_copyright`, not globally public
domain or licensed. It remains `local_only`, `not_authorized`, and
`local_research_only`. The decisive active layer is van de Velde's design;
unresolved holding furniture, OCR arrangement, and derivative-package layers
add independent ceilings. A future public candidate must be independently
reacquired from a then-current upstream route after 2027-12-31 or under an
exact permission, and must still pass fresh fixity, layer, rights, human-review,
and operator-approval gates. The operator-held PDF is never the upload source.

## Exact objects and current response evidence

| Object or response | Fixity / observation |
| --- | --- |
| exact locally held Commons file revision | 9,703,400 bytes; SHA-256 `f3058ff02611cc961de1f7c4fbf0ebc0e4427a913da718189b33a0dde11339bc`; Commons SHA-1 `c8d7cd1891e1d32a878e93694db5fff0d473bdf6`; 166 PDF pages |
| exact current Commons API response | 3,832 bytes; SHA-256 `c37fabf775540d03f1f9449fe9ed64e237d6b31d2adb60ae76a5df9b7bb88b37`; captured 2026-08-02 |
| exact Commons file revision | timestamp `2020-10-12T21:31:51Z`; 9,703,400 bytes; SHA-1 `c8d7cd1891e1d32a878e93694db5fff0d473bdf6`; uploader `Fae` |
| current Commons description revision | revision `1074063624`; timestamp `2025-08-18T05:32:29Z`; SHA-1 `e6520bef12b22bff17c2f14806be5c29c30cd4f3`; wikitext uses only `{{PD-US-expired}}` |
| current Internet Archive metadata response | 38,038 bytes; response-specific SHA-256 `2404e57d21ac408a74cf1bc4c2d4c7aa68e5033f5871191b8bafa435ed0818f4` |
| current Internet Archive PDF | 9,726,827 bytes; SHA-1 `1cf12032bdcba84d8cddd1da8074cb9c62cd9c4c`; this is not the acquired Commons byte revision |
| current Raoul Richter GND response | GND `116512857`; born `1871-01-16`; died `1912-05-14`; response SHA-256 `a65f029971d974a19cfbd344983a2752e964db1fc5f10b014fd3f9ec606fe301` |
| current Henry van de Velde GND response | GND `118626442`; born `1863-04-03`; died `1957-10-25`; response SHA-256 `ffa439008fbfb3829118fa2c1f2e7252b87517e010760b9f6d35cfe0e8769002` |

The exact Commons API route is:

`https://commons.wikimedia.org/w/api.php?action=query&format=json&formatversion=2&prop=imageinfo%7Crevisions&titles=File%3AEcce+homo+%28IA+eccehomo00niet_0%29.pdf&iiprop=timestamp%7Cuser%7Curl%7Csize%7Csha1%7Cmime%7Cmediatype%7Cextmetadata&rvprop=ids%7Ctimestamp%7Csha1%7Ccontent&rvslots=main`

The current API reports `LicenseShortName=Public domain`, `UsageTerms=Public
domain`, `Copyrighted=False`, and no restriction or license URL. Those values
record the provider's current assertion; they do not supply the absent German
source-country analysis.

Current Internet Archive metadata identifies Item
[`eccehomo00niet_0`](https://archive.org/details/eccehomo00niet_0),
Getty Research Institute as contributor and sponsor, scanner
`scribe2.valencia.archive.org`, and scan date `20140520161239`. Its `rights`
and `licenseurl` values are absent. The current PDF differs from the exact
Commons revision and was not substituted or downloaded.

## Source-visible review

The exact operator-held PDF was opened again during this assessment. Temporary
page renders were used only for review and are not corpus or publication
artifacts.

- PDF page 1 shows the front binding and a composed gold *ecce homo* emblem.
- PDF pages 8-9 form a continuous ornamental double title with custom
  lettering and mirrored line compositions.
- PDF page 161 states that the title, binding, and ornaments were drawn by
  Henry van de Velde and identifies copy 965 of 1,250.
- PDF page 165 shows the Getty Research Institute barcode and holding label.
- PDF page 166 returns to the rear binding.

This verifies the named contribution and its presence in the exact Item. It
does not decide the legal originality of every line, identify a current rights
holder, or establish that every ordinary text page carries a protected
ornament. Because the colophon credits ornaments generally, no text-page-only
public derivative is assumed without a complete page-level design inventory.

## Classical and official documentation

### Exact provider surfaces

The current [Commons file
page](https://commons.wikimedia.org/wiki/File:Ecce_homo_(IA_eccehomo00niet_0).pdf)
and API are the exact provider surfaces. The description links the Internet
Archive lineage and invokes
[`PD-US-expired`](https://commons.wikimedia.org/wiki/Template:PD-US-expired).
The template states that pre-1931 publication supplies the United States basis
and that a foreign work also needs a source-country tag. The exact file page
contains no German tag.

The Commons footer separates provider contributions. Structured data in the
file namespace is under
[CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/), while
unstructured page text is under
[CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/). Those
licenses are modeled only for their exact metadata and prose layers; neither
is transferred to the PDF or historical design.

Wikimedia's current [Terms of
Use](https://foundation.wikimedia.org/wiki/Policy:Terms_of_Use) provide no
exact-object warranty and keep responsibility for lawful reuse with the
reuser.

Getty's current [Terms of
Use](https://www.getty.edu/legal/terms-of-use/) and [Open Content Program
FAQ](https://www.getty.edu/projects/open-content-program/faqs/) require an
exact record to carry the Open Content or CC0 indication. The general program
does not license every object ever contributed by Getty to Internet Archive.
No exact marked Getty record for this book was located.

### Germany

The current German Copyright Act supplies the bounded legal route:

- [UrhG Section 2](https://www.gesetze-im-internet.de/urhg/__2.html) includes
  works of applied art and requires a personal intellectual creation.
- [Section 64](https://www.gesetze-im-internet.de/urhg/__64.html) supplies the
  ordinary life-plus-seventy term.
- [Section 69](https://www.gesetze-im-internet.de/urhg/__69.html) calculates
  that term from the end of the relevant calendar year. Nietzsche's layer
  ended at 1970-12-31, Richter's at 1982-12-31, and van de Velde's design
  layer remains active through 2027-12-31.
- [Section 70](https://www.gesetze-im-internet.de/urhg/__70.html) grants a
  twenty-five-year related right to qualifying scientific editions that
  materially differ from known editions. Any such 1908 Edition right ended at
  1933-12-31; this does not decide whether Richter's work qualified.
- [Section 71](https://www.gesetze-im-internet.de/urhg/__71.html) protects a
  first lawful publication only after the underlying copyright has expired.
  Nietzsche had been dead only eight years in 1908, so this is not the basis
  of the first Edition and creates no live modern term.
- [Section 68](https://www.gesetze-im-internet.de/urhg/__68.html) removes
  neighboring-right protection from reproductions of public-domain visual
  works. It cannot make van de Velde's still-protected design public domain.
- [Section 72](https://www.gesetze-im-internet.de/urhg/__72.html) addresses
  simple photographs. It does not change the rights in what a scan depicts.

The exact official responses for Sections 2, 64, 68, 69, 70, 71, and 72 were
rechecked on 2026-08-02. Their SHA-256 values were respectively
`934c52dbc789f46a65d1ee8ab5f01c23389bb3a16674d5671d72cbe98336a513`,
`41febd024baed1009a7ff5593a80378aa37ed91c74d236f9425c1ed3b415eeff`,
`90c39a74fa158bf561fdf744db542085ad82b81cc00e62d1a8268760191e6b5f`,
`fbfcc6d0bb112580a7cd4e82930dd6855b1fd2efbdd15388ad5837a0e4281461`,
`b2f37cc3dd30f94a61df2388620ded4723ea92f757a30c4fa9dd61576b3f430f`,
`c2333e8fa7eeb892f33628b59c13212060cb0d92933e126e00b16ad7c33aacef`,
and `916ddfc255ad6e9b64479e79e45e50540591ecf998040e49b61c0582b9ad4987`.
The response bodies are evidence, not new owner sources.

Deutsche Digitale Bibliothek's current [digital-images
guidance](https://pro.deutsche-digitale-bibliothek.de/daten-liefern/teilnahmekriterien/rechtliches/das-urheberrecht-digitalen-abbildungen)
states both sides of the boundary directly: faithful capture generally adds no
new right to a public-domain flat source, but a scan of a still-protected book
remains protected through the depicted work even when the scanning process
adds no right. That is the correct model for this mixed Item.

### United States

The United States Copyright Office's current [Circular
15A](https://www.copyright.gov/circs/circ15a.pdf) states that works published
before 1 January 1931 are in the United States public domain. The 1908 Work,
Edition, afterword, and design fall inside that current boundary.

The Copyright Office's [Compendium, Third
Edition](https://www.copyright.gov/comp3/) requires human authorship and does
not treat mere scanning or digitization as new authorship. That supports only
the absence of a new faithful-scan copyright. It does not validate the OCR,
clear provider prose or furniture, or erase the independent German design
term.

## Established scholarship, cases, and institutional practice

The established authorities reject a single blanket status:

- The German Federal Court of Justice's *Geburtstagszug*, I ZR 143/12,
  abandoned a higher copyright threshold for applied art than for autonomous
  art. It remains important history but is now read with harmonized Court of
  Justice originality doctrine.
- [*Cofemel*, C-683/17](https://curia.europa.eu/site/upload/docs/application/pdf/2019-09/cp190109en.pdf)
  requires original expression rather than a merely aesthetic effect.
- [*Brompton Bicycle*, C-833/18](https://infocuria.curia.europa.eu/tabs/redirect/juris/liste.jsf?language=en&num=C-833%2F18)
  preserves protection where technical constraints still leave free and
  creative choices, while excluding expression dictated by function.
- [*Bridgeman Art Library v.
  Corel*](https://law.justia.com/cases/federal/district-courts/FSupp2/36/191/2413183/)
  found no United States originality in exact photographic copies of
  public-domain two-dimensional works. It does not clear an image of a work
  that remains protected in Germany.
- DDB, Commons, Getty, and Europeana all use exact-layer or exact-record
  practice. Their policies do not allow one object's status to be inherited
  from an institution's general openness.

## Fresh and currently relevant checks

The August 2026 snapshot does not rely on the age of the book, the 2020 file
timestamp, or an early-2026 rights assumption:

1. The exact Commons file bytes remain available with the same size and SHA-1
   as the local payload.
2. The current description revision is from 18 August 2025 and still invokes
   only `PD-US-expired`, with no German source-country tag.
3. Current Commons extmetadata still says public domain and
   `Copyrighted=False`; no exact license URL is supplied.
4. The 4 December 2025 Court of Justice judgment in joined Cases C-580/23 and
   C-795/23, ECLI:EU:C:2025:941, is the current controlling applied-art
   clarification: no stricter originality test applies, creative choices
   cannot be presumed, and the protected expression must be identifiable.
5. The 19 March 2026 Court of Justice judgment in Case C-649/23,
   ECLI:EU:C:2026:213, confirms that a critical edition may be a separately
   protected work when it expresses free and creative choices. That does not
   revive Richter's expired layer or change Nietzsche's underlying text.
6. The April 2026 Copyright Office Circular 15A still places the 1908
   publication inside the United States public-domain boundary.
7. Current DNB authority responses still identify Richter's death in 1912 and
   van de Velde's death in 1957.
8. The current Internet Archive PDF remains byte-different and its Item still
   exposes no rights or license URL.
9. Getty's current Open Content policy remains exact-record gated; no exact
   CC0 record was found for this scanned book.
10. Europeana's current [2025 Public Domain
    Charter](https://pro.europeana.eu/post/the-europeana-public-domain-charter)
    remains an explicitly non-binding policy statement. It supports accurate
    public-domain labeling but cannot override van de Velde's active term.
11. A fresh source-visible review confirmed the ornamental binding, double
    title, colophon credit, and Getty holding furniture in the exact payload.

## General web search, last

Only after exact provider surfaces, current law, established authority, and
fresh checks were completed, general searches were run for the exact filename,
Internet Archive identifier, Getty control number, OCLC, title, publisher,
editor, designer, and year.

The search found the same Commons and Open Library identities, current
WorldCat attribution of Nietzsche as author, Richter as editor, and van de
Velde as book designer, plus rare-book records describing the double title,
binding, typography, and ornaments. These sources corroborate identity and
visible responsibility but supply no stronger exact-file permission. Auction
and bookseller pages were not treated as rights authority. No result supplied
an exact Getty CC0 record, German permission, or a contradiction of the
source-visible colophon.

## Layer decisions

| Layer | Assessment | ToS consequence |
| --- | --- | --- |
| Nietzsche Work | `public_domain_reviewed` in DE/US | historical Work is reusable; no accepted German text is selected |
| Richter creative editing | `public_domain_reviewed` in DE/US | any ordinary author term ended at 1982-12-31; a qualifying Section 70 term ended earlier |
| Richter afterword | `public_domain_reviewed` in DE/US | independently authored prose is outside the reviewed terms but remains a distinct responsibility layer |
| van de Velde binding, double title, lettering, and ornaments | `in_copyright` in DE and public domain in US | fail closed through 2027-12-31 in Germany absent exact permission; do not flatten into Edition typography |
| faithful mechanical capture | `public_domain_reviewed` as no-new-right process in DE/US | scanning adds no separate right, but it transmits the active German design layer |
| embedded historical OCR text | `public_domain_reviewed` in DE/US | mechanical text recognition adds no literary authorship; correctness and acceptance remain open |
| exact Commons PDF digital object | `in_copyright` aggregate | current US provider statement does not clear the German design or unresolved package layers |
| Getty holding furniture | `copyright_undetermined` | barcode, label, marks, and physical-copy surfaces are not flattened into the historical book |
| Commons structured metadata | `licensed` under CC0 1.0 | may be reused only within its exact structured-data scope |
| Commons unstructured description | `licensed` under CC BY-SA 4.0 | attribution and ShareAlike apply to reused prose, not to the PDF |
| Internet Archive metadata and current derivative | `copyright_undetermined` | public-safe facts remain source attributed; no Item license or byte equivalence is inferred |

## Server consequence

The Item rights record becomes `in_copyright`; the server-import policy maps
that result to `restricted` and remains `metadata-only`, `blocked-rights`, and
operator-unapproved. Positive DE/US results for
Nietzsche, Richter, scanning, and OCR text do not authorize payload transfer,
server OCR, transcription, page images, snippets, embeddings, alignments,
translations, source-bearing annotations, or content-derived search.

Only public-safe ToS bibliographic, provenance, contributor, source-route,
layer-status, and expiry metadata plus source-attributed licensed Commons
metadata may remain conditional for lexical, search, and graph projections.
No source page, OCR token, design image, provider prose, or local payload is
included.

After 2027-12-31, a new candidate may be independently reacquired from a
then-current route and reassessed. Expiry of the ordinary German design term
will not automatically clear provider contracts, marks, furniture, package
structure, metadata prose, other jurisdictions, or operator approval. The new
bytes must receive a new Item or revision identity and provenance; the local
payload remains private research custody.

## What this does not prove

This assessment does not prove:

- German orthographic, grammatical, textual, OCR, or reading-order
  correctness;
- that the 1908 Edition is author-final, critical, complete, or preferable to
  manuscript, proof, Köselitz-copy, or later critical reconstruction routes;
- that every editorial choice by Richter was creative or that the Edition
  qualified under Section 70;
- that every line or ornament attributed to van de Velde independently meets
  the originality threshold;
- that Commons' `PD-US-expired` statement is a German or worldwide warranty;
- that Getty's general Open Content Program covers this exact book or scan;
- that the current Internet Archive PDF is the exact acquired Commons file;
- that Getty, Internet Archive, or Commons granted a package-wide license;
- that public-domain historical text is an accepted ToS transcription;
- that term expiry at the end of 2027 will itself authorize future-site
  publication; or
- that local possession or a green validator can replace source-visible,
  human, legal, and operator review.

The narrow durable result is an eleven-layer rights model that preserves the
free Nietzsche and Richter strata, identifies the active van de Velde design
ceiling, and keeps the exact mixed PDF local without losing a future 2028
reacquisition route.
