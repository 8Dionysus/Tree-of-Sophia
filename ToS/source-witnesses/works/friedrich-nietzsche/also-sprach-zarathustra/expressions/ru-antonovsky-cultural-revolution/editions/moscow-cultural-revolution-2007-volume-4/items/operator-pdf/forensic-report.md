# Forensic Intake — Cultural Revolution 2007, Antonovsky PDF

Status: AI-assisted technical inspection; bibliographic and rights judgments
await human review
Inspected: 2026-07-22
Payload visibility: local only

## Fixity

- Original basename: `Так говорил Заратустра П Антоновского.pdf`
- Size: 162,778,307 bytes
- SHA-256: `49614015865f8c65a68746b1a6dc9a8b7f036d817b2e04b1f3b94fec8ef7b0c2`
- Media type: `application/pdf`
- Source-local modification time observed before intake:
  `2026-01-25T16:56:32.241616479-06:00`

The earlier download source and original acquisition date were not supplied.
This record describes the exact bytes registered into ToS, not their history
before operator possession.

## Container inspection

Poppler 26.01.0 reports:

- PDF 1.7;
- 432 A4 pages;
- unencrypted, untagged, not optimized;
- title metadata `Так говорил Заратустра.cdr`;
- author metadata `User`;
- creator `CorelDRAW 2018`;
- producer `Corel PDF Engine Version 20.0.0.633`;
- creation/modification timestamp `2021-01-12` as embedded metadata.

`pdffonts` reports zero fonts. `pdfimages -list` reports one image object.
`pdftotext -layout` yields exactly 432 form-feed bytes and zero text lines.
Visual sampling shows that the page text is rendered as vector outlines. The
file is therefore visually legible but has no usable native text layer.

## Visible bibliographic evidence

Rendered pages were inspected at pages 1, 2, 3, 4, 5, 6, 100, and 432.

- Page 2 identifies the Institute of Philosophy of the Russian Academy of
  Sciences, Friedrich Nietzsche, the complete works in thirteen volumes, and
  publisher `Культурная Революция`, Moscow.
- Page 3 identifies volume 4, *Так говорил Заратустра. Книга для всех и ни для
  кого*, translation by Yu. M. Antonovsky, Moscow, 2007.
- Page 4 states that Antonovsky's translation first appeared in 1898 and was
  checked and newly edited for this edition. It names general editor
  V. A. Podoroga, scientific editor E. V. Oznobkina, commentary translator
  A. G. Zhavoronkov, and designer I. Bernshtein.
- Page 4 carries explicit 2007 copyright notices for the publisher, translation
  editing, commentary translation, and design.
- Pages 5–6 show a structured table of contents; page 100 visibly carries the
  section `Об отребье`; page 432 carries production details.

These observations support a provisional/verified local catalog record.

## External catalog reconciliation — 2026-07-28

The Russian State Library record `01005395580` independently supports the
volume title, thirteen-volume series, publisher and year, Antonovsky
responsibility, scientific editor E. V. Oznobkina, 432-page extent, and ISBN
`978-5-250-06018-9`.

This reconciles the edition-level identity. The catalog record does not name,
hash, or otherwise identify the exact operator-provided PDF, does not recover
its earlier acquisition route, and does not establish redistribution rights.
Human bibliographic and rights review therefore remain open at their stated
scope.

## Laboratory posture

This item is the hard OCR case:

- native extraction is a documented null baseline;
- pages must be rendered deterministically before OCR;
- page regions must remain available beside any recognized text;
- OCR and correction results must cite this file digest and render receipt;
- plausible LLM transcription is not acceptable without visual comparison.

No OCR experiment or semantic extraction is claimed by this report.
