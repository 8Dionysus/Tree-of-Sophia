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

## Translation-responsibility materialization — 2026-08-01

Ordered research rechecked the exact source pages, current national-library
records, classical Work/Expression modelling, established translation
scholarship, fresh 2025–2026 material, and general-web results in that order.
The resulting model-made annotation admits one unreviewed Expression-level
`translated_by` claim to the stable Antonovsky Agent. Proposed whole-page
anchors return to PDF pages 3 and 4.

The page-4 statements for general editing, scientific editing/checking of the
translation, commentary translation, and design remain exact research
evidence but are not flattened into the current responsibility predicates.
The annotation does not admit source text, OCR, Expression equivalence,
translation quality, semantics, rights clearance, or human review. The local
payload remains unchanged and local.

## Layered rights continuation — 2026-08-11

The exact registered bytes were reverified, and temporary renders of PDF
pages 3 and 4 were opened again. Ordered research then checked current official
RU/US law, established digitisation scholarship, fresh 2026 WIPO guidance,
and general web results last. The result is recorded in
`ANTONOVSKY_2007_CULTURAL_REVOLUTION_LAYERED_RIGHTS_ASSESSMENT.md` and in the
Item's version-2 `rights.json`.

The assessment now separates eight layers. Nietzsche's underlying Work and
Antonovsky's historical translation substrate are public-domain-reviewed for
the bounded RU/US route. That historical result does not clear the exact 2007
Expression: page 4 separately identifies and claims new translation editing,
commentary translation, and design in 2007. Those recent contributions and
the aggregate operator PDF are treated as in copyright; the underlying modern
commentary and the exact scope of the publisher contribution remain
copyright-undetermined.

The improved status is a more precise closed boundary, not permission. The
162,778,307-byte PDF remains local-only; redistribution, public derivatives,
server processing, and future-site upload remain unauthorized. Rights review
does not accept OCR, source text, translation quality, semantics, or canon,
and human legal review and operator transfer approval remain absent.
