# Jenseits 1886 Internet Archive, Google, and Harvard Layered Rights Assessment

Status: model-authored rights research; exact upstream object and three local
files reconciled; no legal advice, human rights review, source-text acceptance,
operator transfer approval, or publication decision

Research snapshot: 2026-08-02

## Question

What may Tree of Sophia say about the exact Internet Archive package derived
from Google's scan of Harvard's physical copy of the 1886 Naumann edition of
*Jenseits von Gut und Böse*?

The package cannot be treated as one legally homogeneous object. At minimum it
contains or represents:

1. Nietzsche's underlying Work;
2. the historical 1886 Edition presentation;
3. the faithfully photographed historical book pages;
4. a modern Google-generated cover and digitization marks;
5. Harvard bookplate, stamps, shelf and holding furniture;
6. automatically recognized historical text embedded in PDF, DjVu XML, and
   ABBYY XML;
7. word, line, page-coordinate, XML, PDF, and derivative-package structure;
8. provider and bibliographic metadata;
9. the operator's local custody and the future site's separate admission
   policy.

## Result

The historical Work and 1886 Edition presentation are
`public_domain_reviewed` for the bounded German and United States routes. The
faithful scan of the old book pages receives the same result: the exact Item
carries Public Domain Mark 1.0, current German book-scan guidance treats a
faithful two-dimensional reproduction as creating no new copyright, and the
current United States Copyright Office Compendium excludes mere scanning and
digitization from copyrightable authorship.

The automatically recognized historical Nietzsche text is also
`public_domain_reviewed` as a mechanical rendering of public-domain material.
This is a rights result only. It says nothing about OCR accuracy, language,
reading order, page alignment, textual authority, or acceptance into ToS.

The exact three-file package cannot receive a positive aggregate result. Page
1 is a modern generated Google cover with its own design. Pages 2 and 4 carry
Harvard holding furniture and marks whose exact authorship and non-copyright
constraints have not been resolved. The DjVu and ABBYY files add coordinates,
hierarchy, and machine arrangement; the PDF and derivative chain add package
structure. Copyright originality may be weak for several of these machine
layers, but the exact provider surface supplies no operative license for them,
and possible database, contract, mark, attribution, and jurisdictional
constraints remain distinct. The Item-level PDM cannot silently flatten those
layers.

The aggregate record therefore becomes `conflicting_evidence`, not
`public_domain_reviewed`, `licensed`, or `in_copyright`. All three operator-held
files remain `local_only`, `not_authorized`, and `local_research_only`. A future
public candidate must be independently reacquired from an exact then-current
upstream route and pass fresh fixity, layer, rights, human-review, and operator
approval gates. It must not be copied from the local ToS payload.

## Exact objects and current response evidence

| Object or response | Fixity / observation |
| --- | --- |
| local and upstream text PDF | 13,944,828 bytes; SHA-256 `6ae316c90f958d09045fea27b2430b86623ebb85f8a27146099d028775cdc80a`; Internet Archive derivative of DjVu XML |
| local and upstream DjVu XML | 3,581,142 bytes; SHA-256 `6227d4a797fb27608386733a9d71fd06c049e5458c9e0687cb582f0c31177be0`; derivative of ABBYY XML |
| local and upstream ABBYY XML gzip | 6,445,855 bytes; SHA-256 `ba8f4c91a317a3de03ab1f318860aaba6837d979e1ec99365e6d13def7db5a34`; derivative of the processed page images |
| exact Internet Archive metadata response | 5,374 bytes; SHA-256 `dc7fddd19b97bcf08b2d29d48252cd788da87625bee08e726bb4d614693f57a3`; captured 2026-08-02 |
| current Internet Archive terms route | the fetched 1,872-byte response, SHA-256 `7c6422fe4878f289e99b18090bacfa91dfbe8eb2d62a147934fcb9de05a98615`, was only a JavaScript application shell and supplied no exact object license |
| Institutional Books metadata Parquet | 306,251,508 bytes; SHA-256 `55861c3e735c71bdcd78ee3d4eeec59a0a547aa37a4f62e9df712869d44c8dfb`; 983,004 rows; queried transiently and removed |

The exact upstream Item is
[`bub_gb_YIURAAAAYAAJ`](https://archive.org/details/bub_gb_YIURAAAAYAAJ),
ARK `ark:/13960/t6n048588`. Current metadata identifies Google Books ID
`YIURAAAAYAAJ`, OCLC `247426798`, Harvard University as contributor, Google as
sponsor and scanner, 274 pages, and ABBYY FineReader 9.0. The upstream sizes,
MD5 values, and SHA-1 values reconcile with all three local files. This proves
identity and fixity, not rights or text quality by itself.

Source-visible inspection separates the mixed layers:

- PDF page 1 is a modern designed cover with `Digitized by Google`;
- page 2 carries a Harvard College Library bookplate and holding marks;
- page 3 is the historical 1886 Naumann title page;
- page 4 carries Harvard marks and the historical line `Alle Rechte
  vorbehalten.`.

The 1886 reservation is preserved as historical evidence. It is not erased,
but it is not treated as a current prohibition or permission after the
reviewed historical terms expired.

## Classical and official documentation

### Exact provider surface and Public Domain Mark

The current [Internet Archive metadata
API](https://archive.org/metadata/bub_gb_YIURAAAAYAAJ) is the exact object
surface. It still exposes [Public Domain Mark
1.0](https://creativecommons.org/publicdomain/mark/1.0/) and the complete
derivative lineage. It does not expose a file-by-file license.

Creative Commons' current [public-domain tools
overview](https://creativecommons.org/public-domain/) distinguishes PDM from
CC0. PDM is an informational label, not a legal tool, license, waiver, or
warranty. Creative Commons recommends it only where the work is believed to be
free of known copyright restrictions worldwide and warns that other rights and
jurisdictional differences may remain. The exact PDM is strong source evidence
for the old-book object; it is not permission from every contributor to every
file layer.

The current Internet Archive [Terms route](https://archive.org/about/terms)
was checked separately. Its live response did not produce an exact object
license and cannot be used to upgrade the PDM into one. Provider access terms,
an object-level rights label, copyright, database rights, and ToS server
admission remain separate decisions.

### Germany

The current German Copyright Act supplies the bounded legal route:

- [UrhG §64](https://www.gesetze-im-internet.de/urhg/__64.html) provides the
  ordinary life-plus-seventy term. Nietzsche died in 1900, so that route ended
  at 1970-12-31.
- [§66](https://www.gesetze-im-internet.de/urhg/__66.html) supplies the
  anonymous and pseudonymous publication route. Even a conservative 1886
  historical-presentation calculation ended long before 2026.
- [§70](https://www.gesetze-im-internet.de/urhg/__70.html) protects qualifying
  scientific editions for twenty-five years, while
  [§72](https://www.gesetze-im-internet.de/urhg/__72.html) provides the special
  term for photographs. Neither establishes a current term for the
  source-visible 1886 Edition presentation.
- [§68](https://www.gesetze-im-internet.de/urhg/__68.html) concerns
  reproductions of public-domain visual works. Its visual-art scope is not
  stretched into a blanket rule for every book, OCR, package, or database
  layer.
- [§87a](https://www.gesetze-im-internet.de/urhg/__87a.html) keeps substantial
  database investment separate from copyright in individual text and facts.

Deutsche Digitale Bibliothek's current
[license guidance](https://pro.deutsche-digitale-bibliothek.de/daten-liefern/teilnahmekriterien/rechtliches/lizenzen-und-rechtehinweise-der-lizenzkorb-der-deutschen-digitalen-bibliothek)
says that a faithful two-dimensional reproduction of a public-domain source
generally creates no new copyright and the digital object remains in the
public domain. Its separate
[reproduction guidance](https://pro.deutsche-digitale-bibliothek.de/daten-liefern/teilnahmekriterien/rechtliches/das-urheberrecht-digitalen-abbildungen)
also shows why §68 must be applied narrowly. ToS uses this current,
book-specific institutional guidance together with the exact PDM; it does not
infer a worldwide provider warranty.

Machine OCR does not become a new authored German literary work merely because
ABBYY or a conversion pipeline recognized the historical text. This conclusion
does not reach the coordinate database, human correction, selection,
arrangement, provider prose, or contracts.

### United States

The United States Copyright Office's current
[copyright overview](https://copyright.gov/what-is-copyright/) says that works
published **in the United States** before 1 January 1931 are in the public
domain in 2026. The exact 1886 Edition was first published in Germany, so ToS
does not use that domestic-publication sentence as its sole United States
basis.

The current Copyright Office
[Circular 38B](https://www.copyright.gov/circs/circ38b.pdf) and
[17 U.S.C. §104A](https://www.copyright.gov/title17/92chap1.html#104a) provide
the foreign-work restoration route. A foreign work could be restored only if,
among the cumulative requirements, it was **not** in the public domain in its
eligible source country through expiration of term on the restoration date.
Nietzsche's ordinary German term ended at 1970-12-31, and the conservative
1886 historical-presentation route ended even earlier. Both were already
public domain through expiration in Germany on 1 January 1996, so they fail
that URAA restoration condition. The April 2026
[Circular 15A](https://www.copyright.gov/circs/circ15a.pdf) supplies the
current general duration table but is not substituted for this foreign-work
analysis.

The Copyright Office's [Compendium, Third
Edition](https://www.copyright.gov/comp3/) remains the governing administrative
manual. Chapter 300 requires human authorship and explains that mere copies
made by scanning or digitizing a literary work lack copyrightable authorship.
It also separates unprotected facts and ordinary layout from original human
expression. That supports the faithful-scan and automatic historical-text
results. It does not clear modern cover design, independent prose, marks,
creative enhancement, or a database as a whole.

### Harvard's exact institutional boundary

Harvard Library's current [Policy on Access to Digital Reproductions of Works
in the Public Domain](https://library.harvard.edu/about/policies/policy-access-digital-reproductions-works-public-domain)
states that Harvard asserts no copyright over openly available digital
reproductions of public-domain collection works on Harvard Library websites
and relinquishes corresponding foreign reproduction claims. This is strong
institutional policy evidence, but the exact package here is hosted by
Internet Archive, was scanned by Google, and follows a third-party derivative
chain. ToS therefore does not silently apply the Harvard-site policy as the
exact file license.

Harvard separately offers the [Harvard Library Public Domain
Corpus](https://library.harvard.edu/services-tools/harvard-library-public-domain-corpus).
Its images, text, and metadata are reached through a request route with
nonprofit, educational, research, and presented-Terms constraints. That corpus
is a possible source family, not evidence that this Internet Archive package
inherits its contract or rights statement.

## Established scholarship, cases, and institutional practice

The established sources support layer separation but not blanket permission:

- [*Bridgeman Art Library v.
  Corel*](https://law.justia.com/cases/federal/district-courts/FSupp2/36/191/2413183/)
  found no United States originality in exact photographic copies of
  public-domain two-dimensional works. It is a district-court decision, not a
  universal global rule.
- The Copyright Office's [fair-use
  index](https://www.copyright.gov/fair-use/fair-index.html) summarizes
  *Authors Guild v. Google*: scanning for indexing, search, and snippets was
  fair use in that bounded service. The holding is not a license for ToS to
  redistribute public full text or exact Google files.
- The CJEU's [*Football Dataco*
  judgment](https://curia.europa.eu/jcms/upload/docs/application/pdf/2012-03/cp120016en.pdf)
  requires original selection or arrangement for database copyright; skill
  and labor alone are insufficient.
- The CJEU's [*British Horseracing Board v. William
  Hill*](https://curia.europa.eu/juris/showPdf.jsf?docid=64559&doclang=EN)
  distinguishes investment in creating data from investment in obtaining,
  verifying, or presenting existing material. It does not justify treating a
  full OCR-coordinate corpus or provider database as automatically free.
- Burk's analysis of
  [positional word indexes](https://papers.ssrn.com/sol3/papers.cfm?abstract_id=949937)
  remains relevant to the difference between recognized old text and a
  structured word-position database.

Europeana's updated 2025 [Public Domain
Charter](https://pro.europeana.eu/post/the-europeana-public-domain-charter)
argues that digitization should not reconstitute control over public-domain
works and that rights should be communicated accurately. Europeana's current
[rights-statement selection
guidance](https://pro.europeana.eu/page/selecting-a-rights-statement) likewise
requires the institution to distinguish copyright, other legal restrictions,
and reuse policy. These are mature policy guides, not the exact package's
contract.

## Fresh and currently relevant checks

The August 2026 snapshot adds current evidence instead of relying on the age of
the book or an older metadata capture:

1. The exact Internet Archive metadata still exposes PDM, Google ID, Harvard
   contribution, Google scanning and sponsorship, ABBYY 9.0, 274 pages, and
   exact derivative lineage.
2. The exact PDF, DjVu XML, and ABBYY XML upstream fixity still reconciles with
   the three local gitignored payloads.
3. The current Internet Archive terms route did not expose an exact file or
   package license.
4. The current Copyright Office material now separates the domestic
   pre-1931 publication shortcut from the URAA foreign-work route. This German
   Work and historical Edition were already public domain through expiration
   in Germany by the 1996 restoration date and fail a cumulative restoration
   condition.
5. Harvard's 2025 Public Domain Corpus and the 2025
   [Institutional Books technical report](https://arxiv.org/abs/2506.08300)
   establish a separate, current, research-oriented route for roughly one
   million Google-derived Harvard books. They do not establish identity with
   this package.
6. The current [Institutional Books metadata dataset
   card](https://huggingface.co/datasets/institutional/institutional-books-1.0-metadata/blob/main/README.md)
   prohibits redistribution of the Early Access dataset in whole or in part
   and permits only non-substitute transformed outputs with attribution. ToS
   therefore records the query result below, not the restricted row.
7. HathiTrust's current [rights database
   documentation](https://www.hathitrust.org/the-collection/preservation/rights-database/)
   defines `pd` as public domain and `bib` as an automatically derived
   bibliographic determination. A `pd/bib` result is low-precedence evidence,
   not manual legal clearance.

The complete Institutional Books Parquet was queried locally and read-only.
Exact OCLC `247426798` produced **zero matches**. A title-and-author search
found one 1886 Harvard/Hathi volume, but its pagination and identifiers show
that it is a **different physical volume**. Its Hathi record reports `pd/bib`.
This is a promising independent acquisition route for the same historical
edition, not an identity bridge or rights decision for the Internet Archive
package. No restricted row, source image, source text, or dataset substitute is
copied into ToS.

The Google Books API exact-ID check again returned HTTP 429. The failure is
preserved as an availability result; no technical bypass or alternative
identity inference was used.

## General web search, last

Only after exact provider metadata, current law, current institutional policy,
established cases, and the freshest corpus route were checked, general
exact-identifier and title searches were run. They found mirrors, catalog
references, and ordinary records for the 1886 edition, but no stronger exact
file-level license and no evidence that the separate Harvard/Hathi volume is
the same physical copy.

General Google Books statements say public-domain books may be shown in full
and partner libraries receive copies for preservation and lawful use. They do
not establish an exact redistributable license for this Internet Archive PDF,
DjVu XML, or ABBYY XML. No general-web result overrode the current exact-object
and layer-specific evidence.

## Layer decisions

| Layer | Assessment | ToS consequence |
| --- | --- | --- |
| Nietzsche Work | `public_domain_reviewed` in DE/US | historical Work is reusable; no accepted German text or critical reading is selected |
| 1886 Edition presentation | `public_domain_reviewed` in DE/US | historical typography and arrangement are outside reviewed terms |
| faithful historical page scan | `public_domain_reviewed` in DE/US | exact PDM plus German book-scan guidance and the U.S. mere-copy rule converge; modern cover and holding furniture are excluded |
| modern Google-generated cover | `copyright_undetermined` | design and authorship are unresolved; PDM is not silently extended |
| Harvard bookplate, stamps, and holding furniture | `copyright_undetermined` | the exact creation and non-copyright rights are unresolved; Harvard's own-site policy is not treated as the IA file license |
| automatic historical OCR text | `public_domain_reviewed` in DE/US | machine rendering adds no authored text right; correctness, structure, and acceptance remain unreviewed |
| OCR coordinates and XML hierarchy | `copyright_undetermined` | individual coordinates may be factual or mechanical, but exact arrangement, database, and provider-contract posture are unresolved |
| PDF and derivative package | `copyright_undetermined` | wrapper, derivative choices, provider furniture, and mixed-layer composition have no exact license |
| metadata | `copyright_undetermined` | bounded attributed facts may remain public-safe; no blanket provider or dataset extraction is authorized |

## Server consequence

The server-import plan remains `rights-unknown`, `metadata-only`,
`blocked-rights`, and operator-unapproved. The positive DE/US findings for the
historical and faithful mechanical layers do not authorize payload transfer,
server OCR, transcription, page images, snippets, embeddings, alignments,
translations, source-bearing annotations, or content-derived search.

A future exact public candidate must be reacquired from its then-current
upstream route, compared by new fixity, and inspected by layer. One defensible
route may construct a new ToS package from positively reviewed historical page
images while excluding or separately licensing modern provider furniture. It
must have its own provenance and must not masquerade as the untouched exact
package.

The different Harvard/Hathi volume may be evaluated as a new Item only after
the operator chooses to enter the request route and accepts its current access
terms. It must never be collapsed into the Internet Archive Item merely because
author, title, publisher, and year agree.

## What this does not prove

This assessment does not prove:

- German orthographic, grammatical, textual, OCR, or reading-order correctness;
- agreement among PDF embedded text, DjVu XML, ABBYY XML, and page images;
- a critical, author-final, complete, or accepted Nietzsche text;
- that PDM is a license, warranty, or worldwide legal conclusion;
- unrestricted rights in Google or Harvard cover design, bookplates, seals,
  stamps, names, trademarks, attribution interests, or other marks;
- unrestricted rights in the XML coordinate database, provider package, or
  metadata collection;
- that Harvard's own-site public-domain policy governs an Internet Archive
  derivative;
- that the separate Institutional Books volume is the same physical Item;
- translation quality, etymology, semantics, signs, graph truth, or canon;
- permission to publish any operator-local file.

The durable result is a mixed-package boundary: reusable historical and
faithful mechanical layers are identified without laundering unresolved modern
layers, local custody, or future-site admission into one misleading `open`
flag.
