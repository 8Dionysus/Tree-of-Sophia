# Jenseits 1886 Edition and Provision Identity Research

Status: ordered, source-returnable research and bounded model-authored
materialization; not accepted bibliography, German text, rights clearance,
printing or release chronology, human review, semantics, or canon

Research snapshot: 2026-08-01

Discovery receipt:
`ToS/source-witnesses/discovery/runs/jenseits-1886-provision-identity.2026-08-01.v1.json`

## Question

What may Tree of Sophia assert from the exact 1886 *Jenseits von Gut und
Böse* title page without collapsing publication, manufacture, publisher,
printer, place, statement year, printing completion, public release, textual
state, and current rights into one label-derived fact?

## Result

The fixity-verified local Google/Harvard scan supplies two distinct source
surfaces:

- PDF page 3 prints `Leipzig / Druck und Verlag von C. G. Naumann. / 1886.`;
- PDF page 4 prints `Alle Rechte vorbehalten.`

The title-page statement visibly assigns both printing and publishing
functions. It therefore supports two separate Edition-owned provision claims
over the same literal statement:

- publication -> `tos.organization.c-g-naumann-verlag-leipzig`;
- manufacture -> `tos.organization.druckerei-c-g-naumann-leipzig`.

The second page is historical rights evidence only. It is not a current legal
prohibition, permission, license, or jurisdictional conclusion.

## I. Classical standards and official documentation

The existing
[`BIBLIOGRAPHIC_TIME_PLACE_RESPONSIBILITY_RESEARCH.md`](BIBLIOGRAPHIC_TIME_PLACE_RESPONSIBILITY_RESEARCH.md)
profile remains the controlling standards layer. Its current IFLA LRM/LRMoo,
BIBFRAME ProvisionActivity, PROV-O, and GND reading requires an Edition-owned
n-ary claim, separate literal and normalized forms, role-specific agents and
places, bounded temporal precision, and source return.

That separation is decisive here. One title-page line supplies two functions,
but BIBFRAME still distinguishes Publication from Manufacture; LRMoo does not
turn manifestation creation, item production, public release, and later
digitization into one event; PROV-O keeps the 2026 annotation event separate
from the 1886 activity described.

### Exact local Item

The exact source is:

- Item:
  `tos.item.friedrich-nietzsche.jenseits-von-gut-und-boese.de-naumann-1886.internet-archive-google-harvard-scan-pdf`;
- PDF SHA-256:
  `6ae316c90f958d09045fea27b2430b86623ebb85f8a27146099d028775cdc80a`;
- title-page source: PDF page 3;
- historical-rights source: PDF page 4.

The local PDF was re-hashed on 2026-08-01 and still matched the tracked
manifest. Pages 3 and 4 were rendered from those exact bytes and inspected.
No rendered image, OCR, transcription corpus, or payload is added to Git.

The current Internet Archive metadata API response retained the exact
identifier `bub_gb_YIURAAAAYAAJ`, title, creator, 1886 date, Public Domain
Mark URI, and the recorded PDF/DjVu XML/ABBYY filenames, byte sizes, MD5, and
SHA-1 values. The response SHA-256 was
`aef60fcb3320d48bbf88b7e6f30fd4ed416a14b2a2056814b6bfa0f0f7bd4adb`.
This corroborates digital-object identity, not the title-page transcription.

### Current official identities and independent exact record

The same-day official DNB/GND evidence already refreshed for the immediately
preceding Naumann 1893 slice remains applicable:

- [C. G. Naumann Verlag, GND 1072998033](https://d-nb.info/gnd/1072998033)
  is the role-specific publisher authority;
- [Druckerei C. G. Naumann, GND 16034133-4](https://d-nb.info/gnd/16034133-4)
  is a distinct role-specific printer authority;
- [Leipzig, GND 4035206-7](https://d-nb.info/gnd/4035206-7) is the Place
  authority.

The current
[DNB Work authority, GND 4211811-6](https://d-nb.info/gnd/4211811-6)
identifies Nietzsche's Work, the fuller title, German language, and appearance
in 1886. It does not identify this physical copy, transcribe this title page,
or establish a release day.

The independent current
[Zentralbibliothek Zürich e-rara record](https://www.e-rara.ch/zuz/content/titleinfo/20295083)
reports shelfmark `43.273`, DOI `10.3931/e-rara-73194`, VI + 266 pages, and
the exact imprint `Leipzig : Druck und Verlag von C. G. Naumann, 1886`. It
also exposes a Public Domain Mark and citation guidance. This independently
corroborates the manifestation statement but is not turned into a duplicate
ToS Item and was not downloaded.

A later same-turn attempt to refresh the GND linked-data endpoints met HTTP
429 and connection failures, and the e-rara IIIF route returned HTTP 403.
No access control was bypassed. The already captured same-day GND responses,
the accessible DNB Work page, the accessible e-rara title record, and the
exact local Item remain the bounded evidence set.

## II. Established bibliography and textual genesis

Beat Röllin's 2013 study
[*Ein Fädchen um's Druckmanuskript und fertig?*](https://doi.org/10.1515/9783110298901.47)
is the established genetic baseline. It treats D 18 as an internally layered
print manuscript whose sheets, order, numbering, late additions, and final
composition were still changing before the 1886 print. The current University
of Basel record response had SHA-256
`a07dc6d73180c0aa9ddda9194386de8a7aced0ed11c0180b6bc795d24e5882d3`.

Andreas Urs Sommer's 2016 historical-critical commentary preserves the June
and July 1886 typesetting and correction sequence, Köselitz's participation,
later numbering intervention, and Nietzsche's later correction copy. These
stages prevent a title-page year from becoming an exact printing-completion
or public-release date and prevent one first-print Item from becoming the
author-final or critical text.

The established evidence constrains interpretation of the imprint; it does
not replace the source-visible statement.

## III. Fresh current work, then general web last

Axel Pichler's 2025 chapter
[*Genese – Kontext – These*](https://doi.org/10.24894/978-3-7965-5300-4)
is the freshest directly relevant methodological work found. It applies a
contrastive textual-genetic reading to *Jenseits* §22 and separates genesis
from the validity of a philosophical thesis. The current University of Vienna
record response had SHA-256
`02a3b8da143833594e3819548d09d76ffdd5b931721c8e2bbb45271c4f024eb8`.
That method reinforces stage separation but supplies no stronger provision
statement than the exact title page.

A 2025-2026 freshness search found no directly relevant 2026 correction to
the 1886 imprint, GND identities, or first-print identity. Current Open
Library, WorldCat, Google Books, and critical-edition results repeated the
basic 1886 Naumann metadata but did not outrank the exact local Item or the
independent e-rara record.

General web search ran last. Marketplace records were rejected as authority;
they supplied no source fact stronger than the exact title page and no
identity evidence stronger than GND.

## Admission decision

Admit:

1. one proposed page-region anchor for the exact imprint on PDF page 3;
2. one proposed page-region anchor for the historical rights statement on PDF
   page 4;
3. one `publication` provision claim using the existing provisional publisher
   Organization;
4. one `manufacture` provision claim using the existing provisional printer
   Organization;
5. the historical rights statement as additional evidence while preserving
   `copyright_undetermined`, `local_only`, unknown redistribution and
   derivative postures, and the metadata-only server plan.

Do not admit:

- an Edition statement not printed on the inspected title page;
- an exact printing-completion or public-release date;
- one generic `C. G. Naumann` identity for both roles;
- a `same_as`, legal-unity, split, or successor relation between the GND
  Corporate Bodies;
- first-print, author-final, correction-copy, or critical-text equivalence;
- accepted German, OCR, translation fidelity, semantics, signs, concepts,
  graph truth, canon, or publication permission;
- source payload, page images, OCR, or tracked source text.

All new anchors and claims are model-made and unreviewed. The graph may expose
the two role-specific normalized identities only through their claim nodes.
