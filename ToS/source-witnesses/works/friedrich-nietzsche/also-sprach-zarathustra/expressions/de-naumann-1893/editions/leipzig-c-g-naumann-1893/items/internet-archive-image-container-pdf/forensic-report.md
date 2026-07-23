# Forensic Intake — Naumann 1893, Internet Archive Image PDF

Status: AI-assisted technical inspection; bibliographic and rights judgments
await human review
Inspected: 2026-07-22
Payload visibility: local only

## Source identity

- Internet Archive identifier: `NietzscheAlsoSprachZarathustra1893`.
- ARK: `ark:/13960/t9574r74x`.
- Item page: `https://archive.org/details/NietzscheAlsoSprachZarathustra1893`.
- IA file role: `Image Container PDF`, source `original`.
- Source-visible title page: *Also sprach Zarathustra. Ein Buch für Alle und
  Keinen. Von Friedrich Nietzsche. Zweite Auflage, mit Portrait und
  Brieffacsimile des Autors.* Leipzig, C. G. Naumann, 1893.
- Capture furniture: `Digitized by Google` and `Original from Cornell
  University`.

The locally held OCR EPUB has the same basename family and matches the EPUB
derivative in this exact IA item by byte size, MD5, and SHA-1. That checksum
bridge, the IA metadata, and the visible title/holding evidence jointly ground
the parent-item and edition identification. They do not make the PDF and EPUB
the same corpus item.

## Fixity and acquisition

- Original basename: `Nietzsche_Also_sprach_Zarathustra_1893.pdf`.
- Size: 51,456,288 bytes.
- SHA-256: `61c947e5aff76a64d82600cc52dcb25ff1b5862530d3a99c96824da885c1e6cf`.
- MD5: `ca4fc412680cfcc98213f69761132ac6` — matches IA metadata.
- SHA-1: `31322ef982e0b17594c57008b53843aedbcfb859` — matches IA metadata.
- Media type: `application/pdf`.
- Acquired: 2026-07-22 through HTTPS using curl 8.18.0 under the
  `abyss-machine` resource owner.

The owner launch completed in 9.883 seconds with a 57 MiB cgroup memory peak
and zero swap peak. Its retained receipt is
`/srv/abyss-machine/storage/artifacts/tree-of-sophia-foundation-lab/source-acquisition/naumann-1893-internet-archive/abyss-machine-resource-launch.json`
with SHA-256
`fd63f8fd0afe06ae321f9af5cbc7566e324ce89f7deb8118fba5cc4e665b38c9`.
That machine-local receipt measures acquisition cost; this tracked report and
the metadata snapshot preserve the portable reacquisition route.

## PDF inspection

Poppler 26.01.0 reports:

- PDF 1.6;
- 529 pages;
- no encryption;
- no JavaScript;
- no embedded fonts or native text layer;
- page size 428.442 by 739.703 points;
- one 856 by 1482 pixel RGB image per inspected page, reported at 144 ppi.

Internet Archive item metadata separately reports `ppi=600` and
`pdf_degraded=invalid-jp2-headers`. These fields conflict with the effective
embedded-image resolution reported by Poppler. Both observations are retained;
the directory and item label intentionally avoid an unqualified `600 ppi`
claim.

`pdfinfo`, `pdffonts`, page rendering, and `pdfimages` reads succeeded. `qpdf`
was not installed, so `qpdf --check` was not run. This report therefore does
not claim a full independent PDF integrity audit.

## Source-visible review

Pages 1, 2, 3, 45, 46, 47, 527, 528, and 529 were rendered and inspected.
They expose, among other things:

- Nietzsche's portrait;
- the stylized work title;
- Cornell shelf marks and rights furniture;
- the title page with edition statement, publisher, place, and year;
- Cornell end-capture surfaces.

This review was performed by the model and has not yet received an independent
human repeat. The rendered contact sheet remains machine-local laboratory
evidence, not a source witness replacement.

## Rights posture

The IA item advertises Public Domain Mark 1.0. ToS records that statement but
does not convert it into an automatic legal or redistribution verdict. No
jurisdiction-specific human review has been performed. The payload therefore
remains local-only and is not promoted for redistribution.

## Laboratory posture

This item closes the missing German visual-input gap:

- OCR variants must operate on these source page images, not on text rendered
  back into synthetic pages from the EPUB;
- the existing OCR EPUB remains a separate historical derivative and a useful
  comparison witness;
- structure recovery may compare scan-first output against the EPUB's 529
  page XHTML files without granting either derivative hidden authority;
- CER/WER and promotion still remain unavailable until the frozen manual gold
  pages receive both required human checks.

No OCR quality, corrected German text, bibliographic human acceptance, rights
clearance, or semantic correctness is claimed by this intake.
