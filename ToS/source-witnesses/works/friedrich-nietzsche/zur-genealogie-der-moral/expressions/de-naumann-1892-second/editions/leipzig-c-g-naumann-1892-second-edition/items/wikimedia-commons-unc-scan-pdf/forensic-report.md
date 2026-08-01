# Forensic Report — *Zur Genealogie der Moral*, Naumann 1892

Status: exact local payload identified and mechanically inspected; source-visible
edition identity verified by a model; text and rights remain unaccepted

## Item

- item:
  `tos.item.friedrich-nietzsche.zur-genealogie-der-moral.de-naumann-1892-second.wikimedia-commons-unc-scan-pdf`
- local payload:
  `payload/zur-genealogie-der-moral-naumann-1892-second-edition.pdf`
- size: 13,227,300 bytes
- SHA-256:
  `5705dbc4f32faa924919fd533962c931e92462d72dab5183610eb68adeecac03`
- source SHA-1:
  `babdcddf6d519d4084f6705333e467fd1a63ba25`
- source SHA-1 and byte-size match: yes
- Git posture: ignored by the narrow item-payload rule

## Container inspection

Poppler 26.01.0 reports:

- PDF 1.5;
- 208 pages;
- not encrypted;
- no JavaScript;
- no forms;
- no embedded files detected by the available Poppler route;
- one unembedded Courier Type 1 OCR font;
- 624 page image/mask resources, three per PDF page;
- document metadata names Nietzsche and *Zur Genealogie der Moral* and links
  the Internet Archive lineage identifier.

`qpdf` and ExifTool were unavailable. Their absence is recorded rather than
silently replaced by a broader claim.

## Source-visible review

A model opened the actual page images, not only embedded OCR.

- PDF pages 1–3 show UNC binding and circulation evidence, call number
  `B3313 .27 1892`, and a barcode.
- PDF page 5 is the historical title page. It visibly states *Zur Genealogie
  der Moral*, *Eine Streitschrift*, Friedrich Nietzsche, *Zweite Auflage*,
  `LEIPZIG`, `Verlag von C. G. Naumann.`, and `1892.`
- PDF page 6 states the printed relationship to *Jenseits von Gut und Böse*.
- PDF page 7 begins *Vorrede*; page 8 continues it.
- PDF pages 195–202 visibly carry printed pages 175–182.
- PDF page 203 carries the printed contents; page 204 separately states
  `LEIPZIG` and `Druck von C. G. Naumann.`; pages 205–208 are end leaves and
  covers.

The two pages support separate publication and manufacture statements. The
printer line is undated: the title-page year may identify the Edition's
source-visible year but is not promoted to exact printing completion or
public release.

This checks edition identity and visible beginning/end continuity. It is not a
German-language acceptance, collation against 1887, or proof that every leaf is
complete.

## Revision boundary

The acquired file is the exact Wikimedia Commons revision uploaded on
2020-11-01. The current Internet Archive PDF under the source-lineage
identifier has a different size and SHA-1. ToS therefore records the IA object
as lineage and institutional provenance but does not claim byte identity
between the current IA derivative and the acquired Commons revision.

## OCR boundary

The PDF contains an embedded OCR layer. It helped locate the title page and
terminal matter, but visible errors occur even on the title page. The OCR is
not tracked separately, accepted as German, or used as a critical text.

## Rights and publication boundary

Wikimedia Commons declares the exact file public domain and not copyrighted.
ToS preserves this as strong positive route evidence while leaving the model
assessment `copyright_undetermined`, jurisdiction review empty, redistribution
unknown, and the local payload `local_only`.

The operator-held file will not be uploaded to a future site. A later public
route must independently select or reacquire an exact authorized object and
review each relevant layer.
