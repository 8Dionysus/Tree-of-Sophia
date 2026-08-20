# Antonovsky 1913 Provision-Identity Research

Date of evidence refresh: 2026-08-01
Scope: one exact Russian Edition, its publication imprint, place/year statement,
and printer line
Authority posture: model-made bibliographic research; no human bibliographic,
textual, translation, or rights review

## Question

Can the exact 1913 Antonovsky Edition gain source-returnable publication and
manufacture claims without deriving an imprint from the directory name,
collapsing publisher and printer, inventing an edition number, accepting the
Russian text, or publishing the local PDF?

## Ordered method

The pass followed the repository discovery order:

1. the exact fixity-verified manifestation and its originating Commons record;
2. national authority and library catalogs;
3. established historical reference and peer-reviewed Nietzsche scholarship;
4. fresh current official-service checks;
5. general web search last, used only to locate a stronger record or a
   contradiction.

No catalog label was allowed to replace what the exact item displays. No
source payload, page image, or OCR string is copied into Git.

## 1. Exact manifestation and originating record

The local research copy remains the exact 12,244,179-byte Commons object with
SHA-256
`687716bc25ebf2281b967ebb0c6cf16b043c2d40bd16833d57d6dcf260d3476b`.
The current Commons API response was fetched on 2026-08-01 and had SHA-256
`c56f43429503b03cbaef034d892cfe8f9ff1c1fed09ea66c724a6b3dea459f29`.
It still resolves Commons page ID `69390926`, provider SHA-1
`93b088e496dd6e60ff1558022c402ef94807da6b`, 402 PDF pages, and the Penza
library holding statement. That digital-object identity does not itself supply
the book's publisher.

Direct visual inspection separates three surfaces:

- PDF page 5, the printed cover, displays
  `ИЗДАНІЕ «ЖИЗНЬ ДЛЯ ВСѢХЪ».`;
- PDF page 7, the title page, displays `С. ПЕТЕРБУРГЪ.` and `1913.` but no
  publisher;
- PDF page 8 displays
  `Типографія Бр. В. и И. Линникъ, Гончарная, 7.`.

The present preferred Edition label was therefore directionally right about
the imprint but imprecise in wording: the source says *издание «Жизнь для
всех»*, not *издательство «Жизни для всех»*. More importantly, the three
surfaces prove that the publishing imprint and printer must be represented as
different participants in different provision activities.

Three new proposed whole-page anchors bind those observations to PDF pages 5,
7, and 8 and to the exact file digest. They are bibliographic source-return
addresses, not accepted transcription or Russian text.

## 2. National authority and library catalogs

### Place

The current DNB GND linked-data record
[`4267026-3`](https://d-nb.info/gnd/4267026-3) names `Sankt Petersburg`, gives
`Petrograd` among the temporary or variant names, and dates the historical
name boundary to 18 August 1914. The fetched linked-data response had SHA-256
`c659c8ca5ea0650e4305ef29d46aaa1c75f5fe32b6931911834a4d1d6ff050c9`.
It supports one provisional Place identity for the 1913 literal
`С. ПЕТЕРБУРГЪ`; it does not normalize every historical spelling,
administrative boundary, or jurisdictional role.

### `Жизнь для всех` as publication imprint

The exact book was not found as a stronger current RSL/NEL catalog record in
the bounded official search. Independent RSL records nevertheless establish
that `Жизнь для всех` was cataloged as a Saint Petersburg publisher/imprint in
the same period:

- [a 1913 publication](https://search.rsl.ru/ru/record/01003805754) records
  `Санкт-Петербург : Журн. "Жизнь для всех", 1913`; current response SHA-256
  `6bb09812b8fa4423ee1dc74bcdfc72a6e2cd36b311feef63aa40480a5b273bd0`;
- [a 1914 publication](https://search.rsl.ru/ru/record/01004181437) records
  `Санкт-Петербург : Жизнь для всех, 1914`; current response SHA-256
  `d51277656d97415d1abde78d9cf072b52a281a18a3e2b01256ba88f18d39feae`.

These records corroborate the role carried by the exact cover statement. They
do not prove that the imprint was one particular modern legal entity, that the
journal and every book imprint are identical corporate bodies, or that
Vladimir Posse should replace the displayed imprint as the claim object.

### Brothers Linnik as printer

Two NEL records supplied by RSL independently normalize contemporary 1913
imprints to `тип. бр. В. и И. Линник`:

- [record 004016550](https://rusneb.ru/catalog/000199_000009_004016550/),
  response SHA-256
  `60cce0bacf20ab71ef88259cf82165c1d47cefdb042991e709d25a810d9562ba`;
- [record 004016557](https://rusneb.ru/catalog/000199_000009_004016557/),
  response SHA-256
  `20cce0721075cab572b7676055c0be42cde50e80886bbcb57afc3bf6af5c3b8e`.

They corroborate the exact page-8 printer string and its Saint Petersburg
context. They do not make the printer the publisher of this Edition.

## 3. Established reference and scholarship

The 1911 *New Encyclopedic Dictionary* entry and the peer-reviewed 2020
Institute of Philosophy RAS study already establish the Antonovsky
translation lineage and the translator's identity. The latter response still
has SHA-256
`340a0f02e483e6b2158273529bca58bc5198fbfcabf735a7e4c63de4d135b6dc`.
Neither source owns this Edition's imprint statement. Their value here is a
boundary: translation history must not be used to infer publisher, printer,
edition number, or textual identity among the 1900, 1903, 1907, 1911, and 1913
states.

The current [Presidential Library record for the 1913 journal](https://www.prlib.ru/item/964713)
reports *Жизнь для всех* under Vladimir A. Posse's general editorship and says
the periodical's publisher was V. A. Posse during 1909–1917. The response had
SHA-256
`c59f7af32ec6d3240b266c881cae6c999f07cafb4ae7f6f9f28368732ba4500f`.
This is useful historical context, but the book's cover names the imprint, not
Posse personally. No Person-level publisher claim is admitted.

## 4. Fresh/current check and general web last

The official responses above were re-fetched through
`2026-08-01T04:40:31-06:00`. They preserve the expected place, imprint, and
printer forms. The bounded official search found no exact national-library
record stronger than the fixity-verified item for this 1913 manifestation.
That absence is recorded rather than filled by a marketplace or copied
bibliography.

General web searches were run only after the exact, official, and established
routes. They returned Wikisource, Nietzsche-text mirrors, retail/reuse pages,
and secondary edition lists. None supplied a stronger exact-item authority,
an external identifier for either historical organization, or evidence that
publisher and printer were the same participant. None is selected for
normalization.

## Admission decision

Admit exactly:

- one provisional Saint Petersburg Place with GND `4267026-3` as external
  authority evidence;
- one provisional `Жизнь для всех` publishing-imprint Organization and one
  separate provisional brothers-Linnik printing Organization;
- three proposed source-return anchors for pages 5, 7, and 8;
- one Edition-owned publication claim joining the exact cover imprint to the
  exact title-page place and year;
- one separate Edition-owned manufacture claim joining the exact printer line
  to the same manifestation while warning that its city/year are reconciled
  across the item rather than repeated in the printer line;
- one ordered discovery record and digest-bound provenance events.

Do not admit:

- an edition number, release date, print-run count, sale event, or textual
  equivalence with 1911 or any later reprint;
- Vladimir Posse as the displayed publisher Person, or the printer as
  publisher;
- legal-entity or same-as equivalence for either provisional Organization;
- OCR, source text, page images, accepted Russian, translation quality,
  semantics, signs, concepts, human review, publication rights, site payloads,
  or canon.

The two claims record what the exact item says and keep the roles apart. They
do not convert bibliographic provision into accepted text or publication
authority.
