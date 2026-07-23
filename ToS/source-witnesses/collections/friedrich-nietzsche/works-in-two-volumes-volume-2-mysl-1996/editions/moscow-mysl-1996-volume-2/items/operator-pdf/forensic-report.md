# Forensic Intake — Mysl 1996, Works in Two Volumes, Volume 2 PDF

Status: AI-assisted technical inspection; bibliographic, translator, and
rights judgments await human review
Inspected: 2026-07-22
Payload visibility: local only

## Fixity

- Original basename: `Ницше собрание сочинений.pdf`
- Size: 24,126,572 bytes
- SHA-256: `ce16b68089dee0fc53a31d9e97723991b292dd8835c43bbbb40a9373c9a436aa`
- Media type: `application/pdf`
- Source-local modification time observed before intake:
  `2026-01-26T07:16:41.706573096-06:00`

The earlier download source and original acquisition date were not supplied.

## Container inspection

This item demonstrates why a shallow green probe is unsafe:

- `file` reports PDF 1.3 and `4 page(s)`;
- Poppler 26.01.0 `pdfinfo` reports 831 pages at 317.28 × 507.36 points.

The PDF-aware result matches the actual container. Embedded metadata names
`ABBYY FineReader 10` and `iTextSharp 5.0.6`. The file is unencrypted,
untagged, and not optimized.

`pdfimages -list` reports 835 image/mask rows. `pdffonts` reports 17 Type 1
font rows with Unicode mappings, alongside many invalid-font-weight warnings.
`pdftotext -layout` yields approximately 3,573,309 bytes and 38,826 lines.
The first pages already expose spacing and character confusions such as
`С О Ч И Н Е Н И Я`, `С О Ч М Н Е Н И Я`, and mixed Greek/Cyrillic-looking
glyphs. The text layer is useful evidence but is not a reviewed transcription.

## Visible bibliographic evidence

The opening pages identify:

- Friedrich Nietzsche, *Сочинения в двух томах*, volume 2;
- series `Философское наследие`, volume 126;
- Moscow, publisher `Мысль`, 1996;
- compiler, editor, and notes author K. A. Svasyan;
- translators Yu. M. Antonovsky, N. Polilov, K. A. Svasyan, and
  V. A. Flerova;
- ISBN `5-244-00854-2` for the volume;
- a 1996 publisher copyright notice.

Extracted text contains the heading `ТАК ГОВОРИЛ ЗАРАТУСТРА` and repeated work
text. This supports an unreviewed membership claim that the collection
contains the work. The opening translator list does not, by itself, prove
which translator owns each contained work. The current Antonovsky expression
handle remains provisional until work-specific evidence or text comparison is
reviewed.

## Laboratory posture

This item is the embedded-OCR comparison case:

- route A preserves the existing ABBYY layer exactly;
- fresh OCR routes use the same frozen page renders;
- page-image comparison decides whether a changed character is a correction;
- structure recovery must distinguish the aggregate volume from the contained
  work;
- translator and expression equivalence require bibliographic plus textual
  review, not string similarity alone.

No new OCR, correction, or semantic extraction is claimed by this report.
