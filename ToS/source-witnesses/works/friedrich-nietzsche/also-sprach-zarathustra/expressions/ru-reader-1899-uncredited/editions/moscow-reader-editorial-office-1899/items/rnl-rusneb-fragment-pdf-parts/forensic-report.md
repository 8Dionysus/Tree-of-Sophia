# Forensic Intake — Reader 1899, RNL/RuNEB Fragment

Status: AI-assisted technical and source-visible inspection; bibliographic,
textual, and rights judgments remain unreviewed
Inspected: 2026-08-01
Payload visibility: local only

## Source identity

- RuNEB record: `000200_000018_v19_rc_1709542`.
- Provider object basename: `ob000000411`.
- Holding named by RuNEB: Russian National Library.
- Cataloged extent: 236 pages, 18 cm.
- Cataloged language: Russian.
- Catalog note: offprint from *Reader* 1899, books 16, 18–20, 26–28, and
  30–35.

The current RuNEB card and exact title page establish one Moscow 1899 Edition
issued by the editorial office of the journal *Reader*. The title page says
only `Переводъ съ нѣмецкаго`; it does not name Antonovsky or another
translator. Contemporary evidence associates the uncredited book with the
earlier *New Journal* route, but that report is not a collation and does not
turn this Item into a verified Antonovsky Expression.

## Fixity and acquisition

One official RuNEB ZIP download produced three stored PDF members:

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `ob000000411_0001.pdf` | 155,778,671 | `f27a4c1bc95d6452ebe8f13358ded0e0a3012031481285e0239bccb7bfb86b4d` |
| `ob000000411_0002.pdf` | 167,408,539 | `3b760f94356ecd938e7356b6115c055a8ba38dee029c379166bf9867afc8f818` |
| `ob000000411_0003.pdf` | 206,863,786 | `d3ce8e66b1f11da99a74362ae2014d73871fe7bb37189b0ea3bf7ff0ed4cef3d` |

The 530,051,690-byte transport ZIP has SHA-256
`d1da6726dac40fa1fe2c7c9ed14d07d22e46ade0975113f868bf0594c5cba516`.
`unzip -t` verified all three members. The extracted source members and their
copies in ignored canonical storage have matching sizes and SHA-256 values.
The transport container remains a recorded delivery checksum; the three
byte-exact PDF members are the canonical local payloads.

The object was downloaded once through the official unauthenticated ZIP
route with curl 8.18.0. No access control was bypassed and no mass download
was performed. The payloads are not tracked by Git and are not authorized for
publication.

## PDF inspection

Poppler 26.01.0 reports three PDF 1.6 files with 7, 7, and 8 pages:

- total PDF surfaces: 22;
- one raster image per PDF surface;
- no encryption, JavaScript, forms, or tagging;
- optimized containers;
- 19 distinct per-file page geometries across the three inventories;
- no usable embedded text layer: `pdftotext` emitted only page separators.

`qpdf`, ExifTool, and OCR tools are not installed on the current host surface,
so no qpdf validation, ExifTool report, OCR, transcription, language
acceptance, or text-layer quality claim is made.

## Source-visible review

All 22 PDF surfaces were rendered into a contact sheet to determine coverage.
The title page and four representative content surfaces were also inspected
individually: printed pages 8–9, 54–55, the part-II opening with printed page
87, and printed pages 138–139.

The first PDF surface visibly contains:

- `Фр. Ницше`;
- `Такъ говорилъ Заратустра`;
- `(Also sprach Sarathustra)` as printed;
- `Символическая поэма`;
- `Переводъ съ нѣмецкаго` without a translator name;
- `Изданіе редакціи журнала „Читатель“`;
- `Складъ изд. у Д. П. Ефимова, Москва, Тверская, д. Мухина, кв. 17`;
- year `1899`.

The photographed fragment then exposes selected pairs rather than a sequence:
8–9, 32–33, 44–45, 48–49, 50–51, 54–55, 60–61, 64–65, 70–71, 72–73,
the part-II opening and 87, 94–95, 96–97, 102–103, 106–107, 108–109,
110–111, 112–113, 120–121, 126–127, and 138–139. These printed-page labels
are model-read observations, not a human-reviewed collation. The digital
object does not return the complete cataloged 236-page extent.

One proposed whole-page anchor retains the exact source-return address for the
title, language-of-translation, publication, distribution, and year
statements. It is not an accepted diplomatic transcription.

## Rights posture

The RuNEB card exposes free viewing and an official ZIP route. The current
user agreement permits bounded personal, informational, scientific,
educational, and cultural use under applicable law and prohibits mass
automated downloading or bypass. Those terms support this single local
research acquisition, not redistribution.

No object-specific reusable-content license or exact-file redistribution
permission was found. The assessment remains `copyright_undetermined`;
redistribution is `not_authorized`, and derivatives remain local-research-only
until a separate rights and jurisdiction review. The three PDFs must not be
uploaded to a future ToS site.

## Foundation consequence

The acquired fragment closes the Edition and digital-Item identity that the
prior catalog-only research deliberately withheld. It also proves why
availability, cataloged extent, and actual source return are different facts.
The topology now permits a separate uncredited Russian Expression and a
fragmentary Item, while withholding translator responsibility, completeness,
1898/1900 derivation, textual equivalence, accepted Russian, OCR, semantics,
human review, and canon promotion.
