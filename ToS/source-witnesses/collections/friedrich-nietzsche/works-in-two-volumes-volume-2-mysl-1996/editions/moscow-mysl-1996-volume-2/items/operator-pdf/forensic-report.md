# Forensic Intake — Mysl 1996, Works in Two Volumes, Volume 2 PDF

Status: AI-assisted technical inspection; work boundaries and translator
credits are model-observed and unreviewed; textual, rights, and human
bibliographic judgments remain open
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

Source-visible title pages and the table of contents agree on seven contained
works and their container starts:

- *Так говорил Заратустра* — page 5, translator Ю. М. Антоновский;
- *По ту сторону добра и зла* — page 238, translator Н. Полилов;
- *К генеалогии морали* — page 407, translator К. А. Свасьян;
- *Казус Вагнер* — page 525, translator Н. Полилов;
- *Сумерки идолов, или Как философствуют молотом* — page 556, translator
  Н. Полилов;
- *Антихрист* — page 631, translator В. А. Флёрова;
- *Ecce Homo. Как становятся сами собою* — page 693, translator
  Ю. М. Антоновский.

The notes begin on container page 770. The tracked text-free boundary map
therefore records proposed contiguous member ranges ending immediately before
the next visible boundary. It cites only page anchors, IDs, digests, and
bibliographic metadata; it does not copy the embedded OCR or source text into
Git. Every boundary and responsibility claim remains model-made and
unreviewed, and no textual equivalence or translation-quality conclusion is
drawn.

## External catalog reconciliation — 2026-07-28

The Vernadsky National Library catalog independently supports volume 2,
publisher `Мысль`, year 1996, K. A. Svasyan's responsibility, the translator
list, series volume 126, and ISBN `5-244-00854-2`. CiNii Books record
`BA35025034` independently supports the two-volume set, the 1996 publication,
and series volumes 125–126.

The NBUV record reports 830 pages while Poppler reports 831 pages in the local
PDF container. Both observations are retained. The discrepancy may reflect
bibliographic pagination versus scan/container pages, but that explanation is
not promoted as fact. Neither catalog record identifies the exact
operator-provided PDF or reconstructs its acquisition history, and neither is
treated as redistribution-rights evidence.

## Laboratory posture

This item is the embedded-OCR comparison case:

- route A preserves the existing ABBYY layer exactly;
- fresh OCR routes use the same frozen page renders;
- page-image comparison decides whether a changed character is a correction;
- the tracked boundary map distinguishes the aggregate volume from seven
  contained works, while deeper internal structure remains unreviewed;
- translator and expression equivalence require bibliographic plus textual
  review, not string similarity alone.

No new OCR, correction, or semantic extraction is claimed by this report.
