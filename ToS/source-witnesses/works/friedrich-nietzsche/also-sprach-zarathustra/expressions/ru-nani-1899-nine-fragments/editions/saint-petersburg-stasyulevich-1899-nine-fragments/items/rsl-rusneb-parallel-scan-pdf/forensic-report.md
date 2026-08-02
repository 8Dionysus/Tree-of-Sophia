# Forensic Intake — Nani 1899 Parallel Fragments, RSL/RuNEB Scan

Status: AI-assisted technical and source-visible inspection; bibliographic and
rights judgments remain unreviewed

Inspected: 2026-08-01
Payload visibility: local only

## Source identity

- RuNEB record: `000199_000009_003689091`.
- RSL record: `01003689091`.
- RSL call number: `FB M 53/102`.
- Holding: Russian State Library.
- Cataloged extent: `XIV, [2], 105 с.; 24`.
- Cataloged languages: Russian and German; parallel text and title page.

The current institutional records and exact title pages identify a Saint
Petersburg 1899 edition of nine selected Zarathustra fragments translated by
S. P. Nani and printed by M. M. Stasyulevich. Catalog identity does not accept
the source text, expand the translator's name, identify the underlying German
textual state, or establish equivalence.

## Fixity and acquisition

- Source basename: `nani-1899-rusneb.pdf`.
- Canonical local basename: `nani-1899-rsl-rusneb.pdf`.
- Size: 16,228,737 bytes.
- SHA-256: `044be8c36536c751d1f846109b4e99534323138b7ad829e6266ef3150d1e5704`.
- SHA-1: `7c8aae149fd05c4304ee420e191ac563c45278f9`.
- MD5: `50f490a41a97cc52f6a1cf2dd7bb65a8`.
- Media type: `application/pdf`.

The file was acquired once from the official unauthenticated RuNEB PDF route
after storage preflight. The downloaded object and ignored canonical copy were
verified against the same size and SHA-256. No access control was bypassed.
The payload is not tracked by Git and is not authorized for publication.

## PDF inspection

Poppler 26.01.0 reports:

- PDF 1.4;
- 60 PDF pages, predominantly landscape spreads;
- first-page size 440 × 327.2 points;
- no encryption, JavaScript, or forms;
- tagged and optimized container with a metadata stream;
- 11 font rows;
- 96 image/mask rows;
- 38 distinct page geometries;
- embedded text extraction of 134,887 bytes and 2,370 lines, SHA-256
  `9e8f1dc229b8cb787cb29b1c2047e09c2bed1e4d2990636a710c5b94dce7be23`.

Poppler repeatedly emitted `Invalid Font Weight`, while rendering and text
extraction completed. `qpdf` is absent, so no qpdf validation is claimed. The
embedded text layer is a machine candidate only; it is not tracked and neither
its orthography nor content is accepted as source text.

## Source-visible review

PDF pages 1, 2, 59, and 60 were inspected directly.

- Page 1 exposes parallel German and Russian title pages. Both credit S. P.
  Nani as translator, identify nine fragments, name Saint Petersburg and the
  M. M. Stasyulevich printing house, and show 1899.
- Page 2 exposes `Дозволено цензурою. С.-Петербургъ. 18 Октября 1898 года.`
  and a dedication to Vera Fedorovna Komissarzhevskaya. The censor date is not
  modeled as publication date; the dedication creates no identity or
  responsibility relation in this slice.
- Nani's preface visibly presents the edition as an aid to reading the original
  and says the original is supplied beside the translation. This does not
  establish textual correctness or translation equivalence.
- Page 59 exposes printed pages 102 and 103 and the terminal title.
- Page 60 exposes handwritten notes and an unnumbered contents page.

Three proposed whole-page anchors retain source-return addresses for these
observations. They are not diplomatic transcriptions and have not been
independently reviewed by a human.

## Extent boundary

The institutional records report `XIV, [2], 105` pages. The acquired PDF has 60
digital surfaces, last visibly numbered printed page 103, and unnumbered final
matter. A 2025 physical-copy description also reports 105 pages, while another
commercial description reports 103. The exact digital return is verified, but
its completeness against the bibliographic extent is unresolved.

## Rights posture

RSL marks the document freely accessible and RuNEB provides an official PDF
route. Those facts support bounded local research access, not redistribution.
No exact-object reusable-content license or jurisdiction-specific human rights
review was found. The assessment remains `copyright_undetermined`;
redistribution is `not_authorized`, and derivatives are `local_research_only`.
The PDF must not be uploaded to a future ToS site.

## Foundation consequence

This parallel source is valuable soil for later German–Russian alignment,
etymology, translation, and semantic experiments. Intake does not perform
those experiments. It creates a typed Work→Expression→Edition→Item route, one
printed translator responsibility, and one printer manufacture claim. No OCR,
accepted text, German Expression identity, collation, derivation, equivalence,
translation-quality judgment, semantics, human acceptance, or canon promotion
is created.
