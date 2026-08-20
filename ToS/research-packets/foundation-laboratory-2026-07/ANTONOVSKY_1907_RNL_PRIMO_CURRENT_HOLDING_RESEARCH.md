# Antonovsky 1907 RNL Primo Current-Holding Research

Date of evidence refresh: 2026-08-10
Scope: the current Russian National Library (RNL) electronic-catalogue record,
physical-copy coordinates, request surfaces, and digitization-recommendation
route for the exact Saint Petersburg 1907 third edition of *Thus Spoke
Zarathustra* translated by Yu. M. Antonovsky
Authority posture: model-made bibliographic and access research over current
official RNL documentation and exact institutional records, following the
already completed exact-source, established-work, fresh-work, and
general-web-last pass; no source book, accepted transcription, human
bibliography, rights clearance, semantics, or canon

## Result first

The current RNL electronic catalogue resolves the principal bibliographic and
holding gap left by GAK divider `67133`, card `82`. The exact modern record is:

- Primo record `07NLR_LMS004843723`;
- RNL system number `004843723` (`NLR01 004843723` in the display layer);
- title *Так говорил Заратустра : Книга для всех и ни для кого*;
- responsibility `Фридрих Ницше ; Пер. с нем. Ю.М. Антоновского`;
- `3-е изд.`;
- `Санкт-Петербург : тип. Ф. Вайсберга и П. Гершунина, 1907`;
- extent `VIII, 363 с.` and catalogue dimension `16 см`;
- three current RNL item rows in the Русский книжный фонд at
  `пл. Островского`, under shelfmarks `17.145.5.1`, `17.145.5.1а`, and
  `17.145.5.1 Б`;
- each returned item row reports `В хранении`, material `Книги`, and the
  status code `Web заказ`.

The modern record independently corroborates the GAK Edition endpoint and
supplies current physical-copy coordinates. It does **not** supply the book's
pages, a digital object, an acquired ToS Item, accepted text, or reuse
permission. The three remote library copies are holdings, not ToS Items.

The current order state also needs a deliberate warning. The record exposes a
`Заказать` surface and the item rows carry `Web заказ`, but the official RNL
catalogue guide says online ordering is limited to Russian-book-fund books
after 1957 and that other editions use paper request slips. The unauthenticated
request tab only asks the reader to sign in. Therefore ToS records the
catalogue status literally and does not claim that a 1907 copy can actually be
ordered online, delivered remotely, copied, or digitized.

## 1. Classical and current official documentation

The current official
[RNL electronic-catalogue guide](https://primo.nlr.ru/primo_library/libweb/locale/ru_RU/help/help_for_priomo_nui.pdf)
states that the catalogue describes materials held in RNL funds, that detailed
records expose shelfmarks and system numbers, and that document access follows
RNL service rules. Its current 52-page PDF response was inspected transiently:
3,167,456 bytes, SHA-256
`19555aaecae6b19731fef9faead663022a2426cb379f0c1a0e2640e6c5d84496`.
The guide is method and service evidence; it is not record-specific rights
permission.

The guide also limits electronic ordering of Russian-book-fund material to
post-1957 editions. That instruction outranks a convenient reading of the
record's `Web заказ` code. Whether a paper requirement, reading-room request,
or another current procedure applies to this exact 1907 copy remains an
institutional service question.

## 2. Exact current institutional record

### 2.1 Reproducible identity route

An exact basic search in the current RNL Primo catalogue returned one result.
The stable record identity is exposed through the
[RNL permalink](https://primo.nlr.ru/primo_library/libweb/action/dlDisplay.do?vid=07NLR_VU1&docId=07NLR_LMS004843723).
The classic full-record surface and official record-format service expose the
same system number and bibliographic description.

The official tagged RUSMARC display is:

`https://webservices.nlr.ru/util/?method=recordFormat&vid=07NLR_VU1&sysid=004843723&format=001&base=NLR01`

It reports, among other fields:

- `200`: exact title and Antonovsky responsibility;
- `205`: third-edition statement;
- `210`: Saint Petersburg, typography string, and 1907;
- `215`: `VIII, 363 с.` and `16`;
- `702`: `Антоновский, Ю. М., Юлий Михайлович (1857-1913), пер.`;
- `852`: RNL shelfmark `17.145.5.1`;
- `899-M`: RNL shelfmarks `17.145.5.1 Б`, `17.145.5.1`, and
  `17.145.5.1а`;
- `SYS`: `004843723`.

The transient tagged-record response had SHA-256
`b80e9ac98527f520e9b6ea4ca726fff06a0dc218e68f4b22b8d73b1cf3b8b4ce`.
The official MARC download returned 1,302 bytes with SHA-256
`866f8bb03f416d99e8e1a32b960225b9086691f89d8039de1a145ec8502bd986`.
Neither response is retained in Git; the tracked layer preserves only
public-safe metadata, routes, and fixity observations.

The record also exposes `V 106/216` as field `899` nonconventional data and as
a browse-shelf parameter. It is retained here as a literal catalogue
observation only. It is not equated to the overwritten GAK handwriting, not
treated as one of the three current RNL shelfmarks, and not promoted as an
Item identifier.

### 2.2 Current holding rows

The exact RNL `Места хранения` tab and the underlying public ALEPH
`item-global` response independently expose the same three item rows:

| Shelfmark | Barcode | Catalogue status | Due/holding state |
| --- | --- | --- | --- |
| `17.145.5.1` | `4843723-10` | `Web заказ` | `В хранении` |
| `17.145.5.1а` | `4843723-20` | `Web заказ` | `В хранении` |
| `17.145.5.1 Б` | `4843723-40` | `Web заказ` | `В хранении` |

The location tab names `Русский книжный фонд (пл. Островского)`. Its
transient response had SHA-256
`67a5796dd1b09b3ef131a8cfb5fd7359577716438f5657aa76528dfbf8bb3f35`.
The public ALEPH response had SHA-256
`dbc7f1553878af5335fc13834ac5b2ba0c03a2280c61d00b1afb41bac0276115`.

These rows establish current catalogue holdings and coordinates as observed
on 2026-08-10. They do not prove physical inspection, completeness, condition,
immediate availability at a service desk, successful ordering, or permission
to reproduce. Barcodes remain discovery evidence and are not made public ToS
Item identities.

## 3. Exact source, established work, and freshest work

This follow-up does not restart or replace the evidence order completed in
`ANTONOVSKY_1907_RNL_GAK_HOLDING_RESEARCH.md`:

- Antonovsky's exact 1911 preface remains the source-visible witness for the
  reported 1907 third-edition revision state;
- the established 1991 Blok study remains independent evidence for the 1907
  Edition and one historical copy context;
- Ermakova's 2025 direct translation comparison remains the freshest retained
  methodological control for keeping witnesses separate;
- the earlier general-web-last pass produced no source object that outranks
  the current RNL record.

The new current holding metadata strengthens only bibliographic identity and
custody. It does not turn reported revision lineage into collation,
equivalence, accepted text, or translation quality.

## 4. Access and digitization routes

The exact record exposes three distinct routes that must not be flattened:

1. `Места хранения` reports current holding metadata.
2. `Заказать` requires reader authentication; official documentation leaves
   the exact 1907 fulfillment procedure unresolved and likely outside normal
   online ordering.
3. [Рекомендовать к оцифровке](https://webservices.nlr.ru/forms/scanrequest/nui/004843723)
   is a record-specific public form with system number `004843723`. Its text
   says that a recommendation will be considered for a future digitization
   plan. It is not a copy order, permission request, rights statement, or
   promise of digitization.

The recommendation form was inspected by GET only. The 4,845-byte transient
response had SHA-256
`77e92ca648597d4a120294cf1f85868de0ac6f272b29c18dca2944382618ace6`.
No form, request, order, login, payment, message, or personal data was
submitted.

The existing RNL access-request ledger is therefore revised, not sent. It now
asks about copying conditions, current source-page or digital-object access,
and each research transformation separately. The digitization-recommendation
form is preserved as an optional distinct route, not misrepresented as the
rights or research-copy channel.

## Materialized boundary

| Layer | Result now | Deliberately absent |
| --- | --- | --- |
| Expression | existing provisional Antonovsky 1907 Expression gains a current exact institutional record route | accepted text, responsibility change, collation, equivalence, semantics |
| Edition | current RNL record independently confirms third Edition, extent, dimension, system number, and three shelfmarks | source-visible book pages, publisher/printer claim, human bibliography |
| remote holdings | three current RNL catalogue rows in the Russian Book Fund | ToS Item identity, physical inspection, completeness, condition, successful fulfillment |
| Item/File | none | book bytes, manifest, fixity, page map, OCR, transcription, anchors |
| Rights/access | exact record, service surfaces, recommendation-only digitization form, and revised unsent request | permission, redistribution, publication, server processing, successful order |
| Relations | existing topology and unreviewed revision relation unchanged | exemplar, derivation, equivalence, semantic, sign, concept, or canon edge |

## Rights and publication boundary

Current custody, shelfmarks, `В хранении`, a request UI, and a digitization
recommendation route do not establish rights for the physical copy, a future
scan, OCR, transcription, embeddings, quotation, derivative publication, or
source redistribution. Historical Work, Antonovsky translation, Edition,
physical copy, reproduction, and derived data remain separate layers.

Any acquired reproduction remains access-controlled, local, and gitignored
unless exact written terms permit more. Public ToS may expose permitted
bibliographic metadata and provenance without exposing source bytes. No local
operator source file is selected for publication or future-site upload.

## Next source-grounded move

The current holding question is now closed enough to stop guessing and start
from exact coordinates. The remaining lawful route is narrow:

1. use system number `004843723` and the three shelfmarks when communicating
   with RNL;
2. distinguish a digitization recommendation from a copy/service request and
   from rights permission;
3. ask first for exact access/copying terms and any existing digital-object
   successor;
4. if lawful, obtain bibliographic pages before requesting a complete local
   research copy;
5. keep every received source file local and every permission scope explicit.

Until source pages or an exact reproduction are acquired under known terms,
the route remains **current-holding verified and source-incomplete**.
