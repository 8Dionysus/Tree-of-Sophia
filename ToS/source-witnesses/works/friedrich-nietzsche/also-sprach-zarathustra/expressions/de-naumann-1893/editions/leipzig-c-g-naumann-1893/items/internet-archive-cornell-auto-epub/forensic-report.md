# Forensic Intake — Naumann 1893, Internet Archive/Cornell OCR EPUB

Status: AI-assisted technical inspection; bibliographic and rights judgments
await human review
Inspected: 2026-07-22
Payload visibility: local only

## Fixity

- Original basename: `Nietzsche_Also_sprach_Zarathustra_1893.epub`
- Size: 2,017,690 bytes
- SHA-256: `adc7eeae2d5a5cf2b225ef81170b595c77664167a539f0cb1a478184aca8e9de`
- Media type: `application/epub+zip`
- Source-local modification time observed before intake:
  `2026-01-28T15:08:56.502461363-06:00`

The earlier download URL and original acquisition date were not supplied at
intake. A later source-discovery pass resolved the exact parent item:

- Internet Archive identifier: `NietzscheAlsoSprachZarathustra1893`;
- ARK: `ark:/13960/t9574r74x`;
- item page: `https://archive.org/details/NietzscheAlsoSprachZarathustra1893`;
- IA-reported EPUB size: 2,017,690 bytes;
- IA-reported EPUB MD5: `0f56595d01d5e6a4388a81bae8cbf340`;
- IA-reported EPUB SHA-1: `bdd8b3333eae2aa1a214723070a14a1b1fbb2132`.

The reported size, MD5, and SHA-1 all match the local EPUB. This converts the
parent-item identification from inference to checksum-backed evidence, while
leaving the historical acquisition date unresolved.

## Container inspection

Info-ZIP 6.00 reports no compressed-data errors. The EPUB 3 container has:

- 541 entries;
- 529 `page_N.html` XHTML files;
- five JPEG images;
- package UUID `7d909f64-0412-446d-b744-63994ab97356`;
- generator `Ebook-lib 0.17.1`;
- package modified timestamp `2023-11-22T11:38:26Z`.

The OPF omits useful `dc:title`, `dc:creator`, language, publisher, date, and
source identifiers. `nav.xhtml` has an empty ordered list. The accessibility
summary warns that automated character recognition may be inaccurate and that
reading sequence may be wrong.

`notice.html` states that Internet Archive generated the EPUB automatically
from scanned pages with `hocr-to-epub` 1.0.0 and warns of OCR, structure,
header/footer, page-number, and image-detection errors.

## Visible and textual evidence

- `page_0.html` says `Google Original from CORNELL UNIVERSITY`.
- `image_0001_00.jpeg` visibly carries the work title *Also sprach
  Zarathustra*.
- `image_0045_00.jpeg` visibly carries `LEIPZIG`, `Druck und Verlag von
  C. G. Naumann`, and `1893`.
- `image_0527_00.jpeg` shows the Cornell University library capture surface.
- OCR page files contain recurring Google/Cornell furniture, malformed words,
  and low-confidence pages; the package language is incorrectly `None` on page
  documents.

The images and the newly reacquired full scan support the Naumann 1893 edition
identification: the source-visible title page states *Zweite Auflage, mit
Portrait und Brieffacsimile des Autors*, Leipzig, C. G. Naumann, 1893. The
EPUB itself is a 2023 automatic derivative and must not be described as a
born-digital 1893 publication.

The same Internet Archive item exposes a 51,456,288-byte image-container PDF
with MD5 `ca4fc412680cfcc98213f69761132ac6` and SHA-1
`31322ef982e0b17594c57008b53843aedbcfb859`. That PDF is now preserved as the
distinct sibling item `internet-archive-image-container-pdf`; it must not be
collapsed into this EPUB item merely because both descend from the same scan
family.

## Laboratory posture

This item is the container/structure recovery case:

- native XHTML extraction is the zero-cost baseline but retains OCR noise;
- the empty navigation and page-per-XHTML layout require section recovery;
- the five image resources preserve only selected visual evidence, not a full
  image of every page; the sibling PDF is the honest visual input for OCR;
- German text corrections require comparison to a stronger source or scan,
  not language-model plausibility;
- edition, OCR derivative, and source text identities remain separate.

Internet Archive reports `ppi=600` at item level, while `pdfimages -list`
reports the embedded PDF page images at 856 by 1482 pixels and 144 ppi. ToS
retains the disagreement rather than promoting either number as an
unqualified scan-resolution fact.

No corrected German text, reconstructed navigation, human bibliographic
acceptance, rights clearance, or semantic extraction is claimed by this
report.
