# Forensic Intake — DTA Schmeitzner 1883 Part 1 TEI

Status: exact structured payload acquired and mechanically inspected; external
quality statement preserved; ToS textual and rights acceptance open

Inspection date: 2026-07-28

## Identity

- ToS item:
  `tos.item.friedrich-nietzsche.also-sprach-zarathustra.de-schmeitzner-1883-part-1.dta-sbb-corrected-tei-p5`
- source object:
  `https://www.deutschestextarchiv.de/book/show/nietzsche_zarathustra01_1883`
- DTA ID: `16840`
- URN: `urn:nbn:de:kobv:b4-200905194290`
- printed source: Friedrich Nietzsche, *Also sprach Zarathustra*, part 1,
  Chemnitz, Schmeitzner, 1883
- holding copy: Staatsbibliothek zu Berlin – Preußischer Kulturbesitz,
  `SBB-PK, 19 ZZ 10200-1/3`

The acquired object is DTA's digital transcription. It is not the physical
copy, a scan package, the later 1893 Naumann edition, or eKGWB.

## File and fixity

| Field | Observed value |
| --- | --- |
| local basename | `nietzsche_zarathustra01_1883.tei.xml` |
| media type | `application/xml` |
| bytes | `159265` |
| SHA-256 | `d3fa5f8af39d87c8f0ede7a0b52b26da0c57e73125d3b0bd2a836b80eae24a4b` |
| MD5 | `5aa5680600f53531239eb5a04bc59408` |
| XML declaration | XML 1.0, UTF-8 |
| well-formedness | `xmllint --noout` passed |
| source download completed | `2026-07-28T19:25:32-06:00` |

The file is stored only in the canonical checkout's ignored item `payload/`.
The worktree carries its manifest, fixity, provenance, rights, and inspection
records, not its text bytes.

## Structural inspection

The current TEI header reports:

- 123 source images;
- 19,669 tokens;
- 3,819 types;
- 121,387 characters;
- DTABf / TEI P5 encoding;
- DTA corpus labels `core`, `ready`, and `mts`.

A separate local XML traversal observed:

- 123 `<pb>` elements;
- 39 `<div>` elements;
- 117,948 descendant text characters in `<body>`;
- 117,639 characters after whitespace collapsing;
- collapsed-body SHA-256
  `57a9b99e682a9d36be5e6c64f2983e9df5313db703b27b7f61f46724fdfcb39e`.

`Zarathustra's Vorrede` begins after facsimile anchor `#f0009`. Printed section
1 begins after `#f0011` and is structurally compatible with the critical
locator `Za-I-Vorrede-1`. Compatibility is not exact eKGWB alignment.

## External quality evidence

The live DTA TEI header says that the work was recognized automatically by OCR
and then checked by native speakers under the DTA transcription guidelines.
This is materially stronger evidence than an unreviewed OCR dump. It is still
an imported institutional quality statement:

- ToS did not observe or reproduce the historical correction process;
- no ToS reviewer has accepted German fidelity, orthography, grammar, or
  semantics;
- a `ready` corpus label is not diplomatic gold;
- later normalization, encoding, or transcription decisions remain
  editorial assertions.

The TextGrid repository exposes an earlier DTA-derived TEI alternate with the
same 123 page breaks and 39 divisions but different serialized and
whitespace-collapsed body digests. No equality or superiority claim is made.

## Source-visible boundary

DTA exposes all 123 facsimile pages and directly links them from the TEI page
breaks. One page image was fetched into host temporary storage for route
inspection and was not retained as a ToS payload. No scan package was
acquired. The DTA terms assign image publication questions to the holding
library, so image rights are not inferred from the text license.

## Rights evidence

The current TEI header and current DTA terms identify CC BY-SA 4.0 for the
annotated DTA text and require attribution to Deutsches Textarchiv. The terms
separately describe pure text as unrestricted. The live Dublin Core endpoint
still reports the former CC BY-NC 3.0 field. This conflict is preserved in
`rights.json`; it is not silently resolved by choosing the broadest statement.

## Admission boundary

Admitted:

- exact immutable TEI payload and fixity;
- first-part expression/edition/item identity;
- page-addressable structured text;
- holding-copy and facsimile route;
- external native-speaker correction statement;
- positive and conflicting rights evidence.

Not admitted:

- ToS-accepted German;
- critical-edition equivalence;
- exact eKGWB passage alignment;
- source gold;
- translation, etymology, semantic sign, or graph claim;
- facsimile redistribution;
- future-site source upload.
