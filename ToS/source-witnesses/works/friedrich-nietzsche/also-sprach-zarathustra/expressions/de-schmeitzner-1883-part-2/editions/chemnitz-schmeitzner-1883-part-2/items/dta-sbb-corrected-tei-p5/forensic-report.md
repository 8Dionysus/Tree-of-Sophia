# Forensic Intake — DTA Schmeitzner 1883 Part 2 TEI

Status: exact structured payload acquired and mechanically inspected; external
quality statement preserved; ToS textual and rights acceptance open

## Identity

- DTA object: `nietzsche_zarathustra02_1883`
- DTA ID: `16841`
- URN: `urn:nbn:de:kobv:b4-200905194308`
- printed source: *Also sprach Zarathustra*, part 2, Chemnitz,
  Schmeitzner, 1883
- holding copy: SBB-PK, 19 ZZ 10200-1/3
- DTA-reported extent: `103 S.`

The acquired object is DTA's digital transcription. It is not the physical
holding copy, the 117 DTA facsimile images, a critical edition, or a
ToS-accepted German text.

## Fixity and custody

- filename: `nietzsche_zarathustra02_1883.tei.xml`
- media type: `application/xml`
- byte size: `158936`
- SHA-256: `b78a09d74bcc1dd9610b81ee348387942a5df1f9e6c8a4340fb7f2221d8372cb`
- MD5 diagnostic: `be919304e93789b51403c65c9b7a5823`

The exact file is stored only in the canonical checkout's ignored item
`payload/`. Tracked records retain identity, fixity, provenance, rights, and
inspection evidence. The payload is absent from Git and remains outside the
future-site upload route.

## Mechanical structure

Independent parsing established:

- XML 1.0 is well formed;
- `117` `<pb>` boundaries and `24` `<div>` elements;
- first page anchor `#f0001`;
- `115156` body descendant characters;
- `114885` characters after whitespace collapse;
- collapsed-body SHA-256 `ba418a68ea7964e3ad8b67de86a750d90eba8650936a84a0e4cbeb9bbf9f8128`.

The live DTA header separately reports `18788` tokens, `4134`
types, `118965` characters, and corpus labels `core`, `ready`, and
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

The TEI carries `117` page-break anchors that can return later claims
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
