# Forensic Intake — DTA Naumann 1891 Part 4 TEI

Status: exact structured payload acquired and mechanically inspected; external
quality statement preserved; ToS textual and rights acceptance open

## Identity

- DTA object: `nietzsche_zarathustra04_1891`
- DTA ID: `16843`
- URN: `urn:nbn:de:kobv:b4-200905197250`
- printed source: *Also sprach Zarathustra*, part 4, Leipzig,
  Naumann, 1891
- holding copy: SUB Göttingen, 8 PHIL I, 7352:4 RARA
- DTA-reported extent: `134 S., [3] Bl., 21 S.`

The acquired object is DTA's digital transcription. It is not the physical
holding copy, the 170 DTA facsimile images, a critical edition, or a
ToS-accepted German text.

## Fixity and custody

- filename: `nietzsche_zarathustra04_1891.tei.xml`
- media type: `application/xml`
- byte size: `237849`
- SHA-256: `ffc34ce6bea3f0b906c37f313bc66e5e58d9a8b0e69b53ea497a778c15dadd0d`
- MD5 diagnostic: `b6293af4be2ac56ad0353a2df049dec6`

The exact file is stored only in the canonical checkout's ignored item
`payload/`. Tracked records retain identity, fixity, provenance, rights, and
inspection evidence. The payload is absent from Git and remains outside the
future-site upload route.

## Mechanical structure

Independent parsing established:

- XML 1.0 is well formed;
- `170` `<pb>` boundaries and `85` `<div>` elements;
- first page anchor `#f0001`;
- `163693` body descendant characters;
- `162853` characters after whitespace collapse;
- collapsed-body SHA-256 `9927be34810cec94ce9dba2a400a6d253d7a906311162946836d7b2a154f23f3`.

The live DTA header separately reports `25933` tokens, `5274`
types, `168768` characters, and corpus labels `core`, `ready`, and
`mts`. These counts and labels are source assertions; the independent parse
checks container structure rather than German correctness.

## External quality evidence

DTA states that OCR output was subsequently checked by native speakers under
the DTA transcription guidelines and encoded in DTABf/TEI P5. ToS preserves
that institutional claim as meaningful quality evidence. It does not imply:

- a German-competent ToS reviewer accepted orthography, grammar, or meaning;
- exact agreement with eKGWB or another critical edition;
- a source-accepted translation lane;
- gold, semantic truth, or promotion.

## Facsimile and page return

The TEI carries `170` page-break anchors that can return later claims
to DTA page views. No facsimile package was acquired. DTA assigns image reuse
to a separate holding-library posture, so text-license evidence is not applied
to page images.

## Rights evidence

The current TEI header and current DTA terms report CC BY-SA 4.0 for the
annotated DTA text, while the live official Dublin Core field still reports
the former CC BY-NC 3.0. The terms also distinguish pure text from facsimile
images. `rights.json` preserves every layer and the conflict rather than
choosing whichever statement is most convenient.

## Admitted and blocked

Admitted as evidence:

- exact immutable TEI payload and fixity;
- expression, edition, item, and holding-copy identity;
- TEI/page structure and DTA-reported corpus statistics;
- external native-speaker-checking statement;
- positive and conflicting rights evidence.

Not admitted:

- ToS-accepted German;
- exact critical-text equivalence;
- translation, sign, concept, or claim authority;
- public payload redistribution;
- a human review task or debt.
