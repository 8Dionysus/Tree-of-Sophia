# Antonovsky 1900 RSL–RNL–LNB Current-Holdings Research

Date of evidence refresh: 2026-08-10
Scope: current Russian State Library (RSL), Russian National Library (RNL),
and Martynas Mažvydas National Library of Lithuania (LNB) records and
holdings for the Saint Petersburg 1900 *Thus Spoke Zarathustra* translated by
Yu. M. Antonovsky; their bibliographic agreements and unresolved differences;
digital-object state; and lawful research-copy routes
Authority posture: model-made bibliographic and access research ordered from
current official documentation and exact institutional records through
established scholarship, the freshest relevant evidence, open-library
controls, and general web last; no source book, accepted transcription, human
bibliography, collation, rights clearance, semantics, or canon

## Result first

The first counted Antonovsky Edition is no longer bound to one Lithuanian
catalogue record. The current RSL record exposes an RNL control identifier;
that identifier resolves an exact RNL RUSMARC record and one current RNL item
row:

- RSL public record `01003693380`, MARC 001 `003693380`, and field 035
  `(NLR)rc\1717107`;
- RNL Primo record `07NLR_LMS004843721`, system number `004843721`, and
  RUSMARC 001 `v19\rc\1717107`;
- the same transaction value `20061002170739.0` and the same title,
  Antonovsky responsibility, Saint Petersburg place, 1900 year, and main
  extent in the RSL and RNL records;
- RSL holding coordinates `FB Рб 18/414` and
  `OMF 810-83/174-7 (1900, ч. 1-4)`;
- RNL shelfmark `128/318`, barcode `4843721-10`, fund
  `РКФ-Главное здан`, catalogue status `Web заказ`, and state `В хранении`.

The current LNB record and exemplar response remain live and unchanged in
substance: iBiblioteka `C10000469160` / record `4970734`, one in-place and
available holding under call number `1.40686`, and no attachment or electronic
media. Its exact title, responsibility, place, year, and 624-page main body
agree with the Russian record cluster.

The records do **not** agree completely. RSL/RNL report
`тип. Б.М. Вольфа` and `[4], XII, 624 с.; 16`; LNB reports `[s.n.]` and
`[3] XI, 624 p.` The difference may reflect catalogue practice, a copy-level
defect or binding state, or a distinct issue, but metadata alone cannot choose
among those explanations. ToS therefore keeps one provisional 1900 Edition
route while explicitly withholding exact cross-copy equivalence, exact
preliminary-page collation, and any source-visible printer or publisher claim.
The stable Edition identifier remains `saint-petersburg-unknown-publisher-1900`:
`[s.n.]` is not a publisher, while the Russian literal names a typography and
does not by itself identify a publisher.

None of the four remote holding coordinates is a ToS Item. No institution
exposes a record-specific open digital object. The RSL record has no file and
no populated online-reading link; the generic hidden “Читать онлайн” modal is
page furniture and is rejected. RNL exposes a recommendation-to-digitize form,
not a copy, order, permission, or guarantee. LNB exposes a lawful request route
but no payload. No form, request, order, vote, payment, login, message, or
external upload was submitted.

## 1. Classical and current official documentation

The current RSL help distinguishes an actual interactive electronic copy from
ordinary record controls and separately documents reading-room and paid
fragment ordering. The current RNL electronic-catalogue guide explains record
identifiers, shelfmarks, and holding rows, and states that ordinary online
ordering for Russian Book Fund material is limited to post-1957 publications.
Consequently the literal `Web заказ` value is retained without claiming that
the 1900 item can actually be fulfilled online.

The current LNB record API, public exemplar API, 2026 use rules, and official
copy-request form distinguish bibliographic metadata, physical availability,
copying eligibility, fees, and permission. The form route can ask the holding
institution about this exact copy; it cannot predetermine condition, price,
rights, or delivery.

These documents define service semantics. They do not prove that any exact
copy is complete, textually identical to another, digitized, or reusable.

## 2. Exact RSL–RNL control crosswalk

### 2.1 RSL endpoint

An exact RSL title query with Antonovsky and a 1900 year bound returns one
[record](https://search.rsl.ru/ru/record/01003693380). Its current MARC21
display reports:

- public record `01003693380`;
- MARC 001 `003693380`;
- field 005 `20061002170739.0`;
- field 035 `(NLR)rc\1717107`;
- Russian translated from German;
- title and subtitle, Antonovsky translator responsibility, Saint Petersburg,
  `тип. Б.М. Вольфа`, 1900, and `[4], XII, 624 с.; 16`;
- the two RSL holding coordinates.

The exact AJAX result had 18,287 bytes and SHA-256
`73c834707fe168e17359de63fcf8852c7ecd11507dfa595ce83fef97a65ee3d6`.
The current record response had 63,226 bytes and SHA-256
`943cad477088e31e7ff950acf6a068f2960219901503ad52020f86dffebd9f17`.
Neither mutable response is retained in Git.

### 2.2 RNL endpoint

The RSL field-035 control core leads to RNL system number `004843721`. The
official tagged
[RUSMARC display](https://webservices.nlr.ru/util/?method=recordFormat&vid=07NLR_VU1&sysid=004843721&format=001&base=NLR01)
reports:

- field 001 `v19\rc\1717107`;
- field 005 `20061002170739.0`;
- the same title, responsibility, imprint string, year, extent, and dimension;
- RSL fields 899 with both RSL coordinates;
- RNL fields 852 and 899-M with shelfmark `128/318`;
- system number `004843721`.

The tagged response had 7,930 bytes and SHA-256
`f970f44376717a71d9da5ff5e90be02f37d37962a99ddf71aadb981f8b837596`;
the RUSMARC download had 2,035 bytes and SHA-256
`f06ecdd697004e707a58cb4cb8ec32041fcdd857e80da92d43511ce4ea824037`.
The stable Primo route is
`https://primo.nlr.ru/primo_library/libweb/action/dlDisplay.do?vid=07NLR_VU1&docId=07NLR_LMS004843721`.

The wrapper syntaxes `(NLR)rc\1717107` and `v19\rc\1717107` remain distinct
literals. Their shared control core, transaction value, and full
bibliographic agreement reconcile the RSL and RNL records; they do not equate
the institutions' physical holdings or their text layers.

### 2.3 Search-method finding

A broad RNL query for title, Antonovsky, and `1900` did not surface the target
on its first page because `1900` also matched Nietzsche's death year across
many later records. Searches for `624 Антоновского Ницше` and
`Так говорил Заратустра 624` returned no rows. Those results are not evidence
of catalogue absence. Following the originating record's cross-agency control
identifier resolved the exact record immediately. The durable method is:
prefer exact institutional identifiers and record crosswalks over free-text
relevance when old-book year tokens collide with authority dates.

## 3. LNB agreement and unresolved discrepancy

The current
[iBiblioteka record](https://ibiblioteka.lt/metis/publication/C10000469160)
and API still report:

- code `C10000469160`, record `4970734`, transaction
  `20250416091534.4`;
- the exact title and Antonovsky responsibility;
- Saint Petersburg, `[s.n.]`, 1900;
- `[3] XI, 624 p.`;
- no attachments, cover, electronic resource, media licence, excerpt,
  download, or reader.

The current record API response had 15,244 bytes and SHA-256
`372cf6a5fd4a786720f55657866d792b2ace306c2488fe832eb645c714b733a0`.
The current exemplar API returns one `IN_PLACE` and available physical holding
under call number `1.40686`; that 1,525-byte response had SHA-256
`64e00f72755b43a8f58da18a53b36efb2e2dbbeccc03fb38c099afeaa2ad6a24`.

The agreement is strong enough to keep the three catalogue routes attached to
one explicitly provisional Edition candidate. It is not strong enough to
normalize away the preliminary-page discrepancy, replace `[s.n.]`, create a
Wolf Organization, assert a provision activity, or claim that the LNB and
Russian copies instantiate an identical issue. Title, verso, provision,
contents, first numbered page, and terminal-page images are the minimum
source-visible evidence needed to adjudicate the discrepancy.

## 4. Current holdings are not ToS Items

| Institution | Current coordinate | Public state | Boundary |
| --- | --- | --- | --- |
| RSL | `FB Рб 18/414` | catalogue holding; reading-room order control | no local custody or physical inspection |
| RSL | `OMF 810-83/174-7 (1900, ч. 1-4)` | catalogue holding coordinate | not assumed to be the same carrier or a digital object |
| RNL | `128/318`, barcode `4843721-10` | `Web заказ`; `В хранении`; `РКФ-Главное здан` | actual 1900 online eligibility unresolved |
| LNB | `1.40686` | one in-place and currently available copy | not reader-visible/orderable through the public exemplar endpoint |

The transient RNL item response had 19,460 bytes and SHA-256
`4fe30c652ba2b9c506e3987ecb2350bc08470bd401f698824018a684c4590610`.
No Item, File, manifest, fixity record, page map, OCR, or source anchor follows
from any shelfmark.

## 5. Established and freshest relevant work

Pertsev's 2015 peer-reviewed study remains the established methodological
control: translator identity and Edition chronology cannot replace exact
witness comparison of philosophical meaning, style, rhythm, and translation
strategy.

Ermakova's 2025 variant-aware comparison remains the freshest inspectable
Antonovsky-relevant study found. A 2026 exact-topic search surfaced a new
commercial Antonovsky reprint, a forthcoming English translation with a
translation afterword, and new general philosophical studies of *Zarathustra*.
It found no newer inspectable study of the exact 1900 witness or its
preliminary-page state. Freshness therefore changes live catalogue, holding,
service, and reprint evidence, not the historical witness conclusion.

## 6. Record-specific access and digital-object state

The RSL record exposes reading-room ordering and a paid
[fragment-copy route](https://search.rsl.ru/ru/fragment-eorder/rsl01003693380),
but no populated record-specific online-reading route. Its transient fragment
page had SHA-256
`2d99b2c9806735c1d7acca50b7ff97099de8d624ae54ec13c3a134e0501f55a6`.
The generic hidden free-access modal has an empty target and is rejected as a
digital-object false positive.

The RNL record exposes the exact
[digitization recommendation](https://webservices.nlr.ru/forms/scanrequest/nui/004843721).
Its page says a suggestion may be considered for a future digitization plan;
it is not a copy request, rights request, permission, or guarantee. The
4,845-byte response had SHA-256
`6c37074e3b95049899fc492e23ba554555c7ff314239bda5efead1da9a2fec5c`.

The existing LNB request card remains the most useful tiered research-copy
route because it targets one exact physical copy and can ask about the
cross-catalog discrepancy. It is updated, not sent. Russian fragment,
digitization-recommendation, Lithuanian reproduction, source access, and reuse
rights remain separate channels.

## 7. Open-library and general-web-last controls

Fresh exact Internet Archive queries in Russian and transliteration returned
zero items. Their response digests were
`19c9726b4dd332a70f5e3a494e6515eed9ec49786c7c057225d46d239e431920`
and
`895d07b17cc322259c13f1044e0b16ea010021b02376cf540f7e02f3c10df53c`.
An exact Open Library query also returned zero records; its response digest was
`961a1b43bf4b5af8572af4d05182896bd2a48ef7ee39574aeff3d8f8c196ed60`.
These are time-bounded negatives, not universal absence claims.

General web was used last. A search result that appeared to describe an old
Antonovsky witness resolved to CiNii NCID `BA91913156`: a 1981 New York
Chalidze publication of 292 pages, held at one Japanese university. It is a
later publication and is rejected as the 1900 target. Current commercial 2026
and other modern records likewise do not supply an exact historical source.

## Materialized boundary

| Layer | Result now | Deliberately absent |
| --- | --- | --- |
| Expression | existing provisional Antonovsky 1900 Expression gains current RSL/RNL/LNB evidence | accepted text, responsibility change, collation, equivalence, semantics |
| Edition | one provisional three-catalogue record cluster; exact Russian control crosswalk; LNB discrepancy explicit | settled issue identity, source-visible provision, normalized publisher/printer, human bibliography |
| remote holdings | two RSL coordinates, one RNL item, one LNB exemplar | ToS Items, same-copy relation, physical inspection, completeness, condition |
| Item/File | none | bytes, manifest, fixity, page map, OCR, transcription, anchors |
| Rights/access | current RSL fragment, RNL recommendation, and LNB request routes; LNB draft updated and unsent | permission, redistribution, publication, server processing, successful order |
| Relations | existing provisional Expression-to-Edition topology unchanged | new exemplar, derivation, equivalence, semantic, sign, concept, or canon edge |

## Rights and publication boundary

The age of the Work, translation, and Edition does not automatically govern a
particular library reproduction, provider metadata, OCR, embeddings,
quotation, publication, or redistribution. Any acquired source remains local,
access-controlled, and gitignored under its exact written terms. Public ToS
may expose permitted bibliography and provenance without exposing source
bytes.

## Next source-grounded move

The current catalogue problem is resolved far enough to stop free-text
guessing. If external acquisition becomes the chosen experiment:

1. refresh all three exact records and the chosen institution's terms;
2. keep RSL, RNL, and LNB service routes institution-specific;
3. acquire title, verso, all preliminaries, first numbered page, contents, and
   terminal provision pages before a complete reproduction;
4. compare the literal sequence and count behind `[3] XI` versus `[4], XII`;
5. determine from source pages whether B. M. Wolf appears, where, and in which
   role;
6. create no ToS Item until exact bytes, custody, fixity, provenance, rights,
   and resource inventory exist;
7. collate text only after exact witness identity and page topology are
   source-visible.

Until then the route remains **cross-institutionally identified,
bibliographically discrepant, and source-incomplete**.
