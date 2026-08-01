# Forensic Intake — Antonovsky 1911, RSL/RuNEB Scan

Status: AI-assisted technical and source-visible inspection; bibliographic and
rights judgments remain unreviewed
Inspected: 2026-08-01
Payload visibility: local only

## Source identity

- RuNEB record: `000199_000009_003693383`.
- RSL record: `01003693383`.
- Holding named by RuNEB: Russian State Library.
- Cataloged extent: VIII, 292 pages.
- Cataloged languages: Russian translated from German.

The current RSL MARC record identifies Friedrich Nietzsche, translator Юлий
Михайлович Антоновский (1857–1913), fourth edition, Saint Petersburg,
Prometey, 1911. The exact title page independently exposes the shorter source
credit `Ю. М. Антоновскаго`, the fourth-edition statement, imprint, place,
address, and year. Catalog identity does not establish text identity with any
other Antonovsky manifestation.

## Fixity and acquisition

- Local basename: `antonovsky-prometey-1911-rsl-neb.pdf`.
- Size: 62,952,283 bytes.
- SHA-256:
  `57518b50e24fee37b3e9c151e853c36ad34258f51a8b65b8233742d69017db69`.
- SHA-1: `3e3f79fd632d4a6cc1e37d1ce50d97a271b1fe7d`.
- MD5: `7ccfb876440404adcd0db6bd1b171b01`.
- Media type: `application/pdf`.
- Acquired once through the official RuNEB PDF route with curl 8.18.0 on
  2026-08-01, after a 62,952,283-byte storage preflight.

The downloaded object and the copy placed in ignored canonical storage were
both checked against the same size and SHA-256. No access control was bypassed.
The payload is not tracked by Git and is not authorized for publication.

## PDF inspection

Poppler 26.01.0 reports:

- PDF 1.4;
- 153 pages, predominantly two-page landscape spreads;
- no encryption or JavaScript;
- optimized and tagged container;
- first-page size 792 × 583.56 points;
- 27 reported font rows;
- 173 image/mask rows;
- an embedded text extraction of 925,039 bytes and 15,667 lines.

Poppler emitted repeated `Invalid Font Weight` warnings while inspecting the
container, but rendering and text extraction completed. `qpdf` is not
installed on the host, so no qpdf validation is claimed. The embedded text
layer is only a machine candidate; neither its orthography nor its content has
been accepted as source text.

## Source-visible review

PDF pages 1–6 were rendered. Pages 1, 2, 4, and 6 were inspected directly.

- PDF page 1 contains the title page on the right-hand half: Friedrich
  Nietzsche, *Так говорил Заратустра*, `Перевод с немецкого Ю. М.
  Антоновскаго`, `Четвертое издание`, `Книгоиздательство Прометей`, 1911,
  Saint Petersburg, and the Povarской 10 address.
- PDF page 2 contains the title-verso printer line on the left-hand half:
  `Типо-литография "ЭНЕРГИЯ", Спб., Загородный пр., д. 17.`
- PDF page 4 contains Antonovsky's preface. It says excerpts first appeared in
  1898, identifies edition years 1900, 1903, 1907, and the current 1911, and
  explicitly says the prior editions were not simple reprints but new
  reworkings of the translation.
- PDF page 6 begins Zarathustra's preface.

Three proposed whole-page anchors retain source-return addresses for these
observations. They are not diplomatic transcriptions and have not been
independently reviewed by a human.

## Rights posture

The exact RuNEB record exposes free viewing and an official PDF route. The
current RuNEB user agreement permits bounded personal, informational,
scientific, educational, and cultural use under applicable law and prohibits
mass automated downloading or bypass. These terms support this local research
acquisition, not redistribution.

No resource-specific reusable-content license or exact-file redistribution
permission was found. The assessment therefore remains
`copyright_undetermined`; redistribution is `not_authorized`, and derivative
use is limited to local research. Public bibliographic metadata, provenance,
and non-content-bearing locators may remain tracked. The PDF itself remains
local and must not be uploaded to a future ToS site.

## Foundation consequence

The 1911 witness is more than another scan. Antonovsky's own preface makes the
translation lineage edition-sensitive: a shared translator label cannot serve
as proof of invariant text. This Item therefore receives its own Expression,
Edition, fixity, rights boundary, and source-return anchors. No OCR, accepted
Russian, translation quality, semantic claim, equivalence relation, human
review, or canon promotion is created by this intake.
