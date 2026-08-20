# Antonovsky 1903 RSL–RNL Current-Holdings Research

Date of evidence refresh: 2026-08-10
Scope: the current Russian State Library (RSL) and Russian National Library
(RNL) records, their shared bibliographic control identity, current physical
holdings, digital-object state, and lawful access routes for the exact Saint
Petersburg 1903 second corrected edition of *Thus Spoke Zarathustra*
translated by Yu. M. Antonovsky
Authority posture: model-made bibliographic and access research over current
official documentation and exact institutional records, followed by
established scholarship, the freshest relevant evidence, and general web
last; no source book, accepted transcription, human bibliography, rights
clearance, semantics, or canon

## Result first

The current RNL catalogue resolves the source-control bridge that the RSL
record previously exposed only as \`(NLR)rc\1717108\`. An exact RNL search
returns one record:

- Primo record \`07NLR_LMS004843722\`;
- RNL system number \`004843722\`;
- RNL RUSMARC field 001 \`v19\rc\1717108\`;
- the same title, Antonovsky responsibility, \`2-е изд., испр.\`, Saint
  Petersburg typography string, 1903 year, extent, and 2006 transaction value
  as RSL record \`01003693381\` / MARC \`003693381\`;
- the four already known RSL holdings;
- two additional current RNL holdings, \`129/5943\` and \`38.35.6.32\`, in
  \`РКФ-Главное здан\`;
- both actionable RNL rows report \`Web заказ\` and \`В хранении\`.

The wrapper syntaxes \`(NLR)rc\1717108\` and \`v19\rc\1717108\` remain
distinct literals, but their shared control core and identical record-level
bibliography close the unresolved institutional crosswalk. They do not prove
that RSL and RNL hold the same physical copy, that any copy is complete, or
that any text layer is identical.

The public RNL item response also contains one \`TEMP\` row with barcode
\`4843722-70\`, undefined status and fund, and no shelfmark. It is retained as
a rejected catalogue observation, not counted as a current usable holding and
not promoted to an identifier or ToS Item.

Neither institution exposes a record-specific open digital object. The RSL
record's actual read-link section is empty; a generic hidden template modal
contains “Читать онлайн”, but its link has no record-specific target. Treating
that template string as digitization would be a false positive. RSL exposes a
paid fragment-order route and reading-room order; RNL exposes a
recommendation-only digitization form. No login, vote, recommendation,
request, order, payment, or message was submitted.

## 1. Classical and current official documentation

The current RSL catalogue help explains that a real electronic copy has an
interactive document route and that access labels describe its access
conditions. It separately describes reading-room ordering and a paid
page-range fragment order with electronic delivery. These rules define the
service vocabulary; they do not prove that this exact 1903 record has a
digital object or that a complete reproduction is available.

The current official RNL electronic-catalogue guide says the catalogue
describes held materials and exposes system numbers and shelfmarks. It also
states that ordinary online ordering for Russian Book Fund material is
limited to post-1957 editions. The two RNL item rows therefore preserve their
literal \`Web заказ\` status without turning it into a claim that a 1903 copy
is actually eligible for online fulfillment.

The previously inspected 52-page RNL guide response had 3,167,456 bytes and
SHA-256
\`19555aaecae6b19731fef9faead663022a2426cb379f0c1a0e2640e6c5d84496\`.
The current RSL FAQ response had SHA-256
\`544b07867f86994150a20280d8e54e6b2f2003f621c9ec1cdf2b4aa4c523b459\`.
These mutable documentation payloads are not retained in Git.

## 2. Exact current institutional crosswalk

### 2.1 RSL endpoint

The exact
[RSL record](https://search.rsl.ru/ru/record/01003693381) still reports:

- public record \`01003693381\`;
- MARC 001 \`003693381\`;
- field 035 \`(NLR)rc\1717108\`;
- transaction value \`20061002170739.2\`;
- four shelfmarks: \`FB L 31/57\`, \`FB Рб 33/442\`,
  \`FB T 97/44\`, and \`OMF 801-85/11097-8\`.

Its transient current response had 64,535 bytes and SHA-256
\`6c82a0ea9721690a4203c97897cd839c783f3d7cc559a7f86562ddb3a6dc8134\`.
The record has reading-room and fragment-copy controls but no populated
record-specific online-reading link.

### 2.2 RNL endpoint

An exact current RNL query for the title, Antonovsky, and 1903 returned one
result. The stable endpoint is the
[RNL Primo record](https://primo.nlr.ru/primo_library/libweb/action/dlDisplay.do?vid=07NLR_VU1&docId=07NLR_LMS004843722).
The 106,972-byte classic-search response had SHA-256
\`502ff0a183826253f8e2f2185f3e62853e056661ff48aff347e5bf314e36922e\`.

The official tagged RUSMARC display is:

\`https://webservices.nlr.ru/util/?method=recordFormat&vid=07NLR_VU1&sysid=004843722&format=001&base=NLR01\`

It reports:

- field 001 \`v19\rc\1717108\`;
- field 005 \`20061002170739.2\`;
- fields 200/205/210/215 with the same title, responsibility, second corrected
  Edition, typography string, year, extent, and dimension;
- fields 899 with all four RSL shelfmarks;
- fields 852 and 899-M with RNL shelfmarks \`129/5943\` and
  \`38.35.6.32\`;
- system number \`004843722\`.

The tagged response had 9,151 bytes and SHA-256
\`55e6f374cad0daed012233dfb37ff96bd03cc9f15f55f903f82df4e7ddd576a5\`.
The 1,405-byte MARC response had SHA-256
\`bb50a7852799a4f770bb8ca0d31005902694c73e7f7a96ffa83a7c5849025fec\`.
Only public-safe metadata, routes, and fixity observations are tracked.

## 3. Current holdings are not ToS Items

The RSL record continues to expose four institution-held copies. The public
RNL ALEPH item response separately exposes:

| RNL shelfmark | Barcode | Catalogue status | Holding state |
| --- | --- | --- | --- |
| \`129/5943\` | \`4843722-10\` | \`Web заказ\` | \`В хранении\` |
| \`38.35.6.32\` | \`4843722-20\` | \`Web заказ\` | \`В хранении\` |

The same response's undefined \`TEMP\` row has barcode \`4843722-70\`, no
shelfmark, and no identified fund. It is not silently repaired from adjacent
metadata. The full item response had SHA-256
\`c92a38360dd091750edbdd2e2cb0d851583afdd922d340fc83644638856d1def\`.

The six actionable remote shelfmarks establish catalogue custody only. No
copy was physically inspected or acquired, and no completeness, condition,
same-exemplar, successful-order, or reuse claim follows.

## 4. Established and freshest relevant work

Pertsev's 2015 peer-reviewed study of Russian *Zarathustra* translations
remains the established control: translator name and Edition sequence cannot
replace exact-witness comparison of philosophical meaning, style, rhythm, and
strategy.

Ermakova's 2025 direct comparison remains the freshest inspectable
Antonovsky-specific methodological control found. A 2026 search returned a
forthcoming English translation, a new commercial reprint of Antonovsky, and
unrelated translation studies, but no newer inspectable exact-witness study
that changes the 1903 bibliographic or source-access conclusion. Freshness
therefore changes the live catalogue and service evidence, not the historical
Edition assertion.

## 5. Access and digital-object routes

The exact RSL record exposes:

1. reading-room ordering;
2. a paid
   [fragment-copy route](https://search.rsl.ru/ru/fragment-eorder/rsl01003693381);
3. no populated record-specific online-reading route.

The transient fragment-order page had SHA-256
\`6392ea7076be12b080852257495e5b1b50abfaaecd8096af5d47e33510760ba9\`;
the current RSL copying-information page had SHA-256
\`11b25ab26272e266403a237d6aa8f59337dd3c54d6dd45ec7448db7a4a3183a6\`.

The exact RNL record exposes:

1. two current physical holding rows;
2. catalogue order controls whose actual 1903 eligibility remains unresolved;
3. a record-specific
   [digitization recommendation](https://webservices.nlr.ru/forms/scanrequest/nui/004843722).

The RNL form is a recommendation for possible inclusion in a future
digitization plan, not a copy order, rights request, permission, or guarantee.
Its transient 4,845-byte GET response had SHA-256
\`2dcbc25b73b578b1c550cbd282a8430d616c1c709e9911b55197c931a80e7b9d\`.

A new RNL-specific request card is prepared separately from the existing RSL
draft. Both remain \`draft-not-sent\`; neither institution's service route is
substituted for the other.

## 6. Open-library and general-web-last controls

Fresh exact Internet Archive queries for Russian/transliterated title,
Antonovsky variants, and year 1903 returned zero items. The two response
digests were
\`e2f477aa2dc6c575fc37c49370893f36b70feec97a3abcd8d55427e10c7913b0\`
and
\`cdf4b1e0bf81c245194c114891484d50d0e50113fcf457300b5e6c37648a5268\`.
The exact Open Library query also returned zero records; its response digest
was
\`a68a51a345fbaef76433e2dc9c3af3f7f73ee9f366a07aa6e9eec27944a0f069\`.
These are time-bounded negatives, not proof of universal absence.

Only after the official, exact, established, and fresh passes, general web
returned the now-readable
[Litfund copy description](https://www.litfund.ru/auction/445/52/) and current
Alib listings. They corroborate Edition-level description and copy-specific
condition but provide neither a complete digital source nor institutional
rights. A crowd-sourced typography page explicitly warns that the typography
was not necessarily a publisher; it is not authority for creating an
Organization or provision claim.

## Materialized boundary

| Layer | Result now | Deliberately absent |
| --- | --- | --- |
| Expression | existing provisional Antonovsky 1903 Expression gains exact RNL crosswalk evidence | accepted text, responsibility change, collation, equivalence, semantics |
| Edition | exact RSL/RNL metadata reconciles control identity and six actionable remote shelfmarks | source-visible book pages, publisher/printer normalization, human bibliography |
| remote holdings | four RSL and two RNL catalogue holdings; one undefined RNL TEMP row rejected | ToS Item identity, same-copy relation, physical inspection, completeness, condition |
| Item/File | none | bytes, manifest, fixity, page map, OCR, transcription, anchors |
| Rights/access | current RSL fragment route, RNL recommendation route, and two separate unsent drafts | permission, redistribution, publication, server processing, successful order |
| Relations | existing topology and unreviewed revision relation unchanged | exemplar, new derivation, equivalence, semantic, sign, concept, or canon edge |

## Rights and publication boundary

The age of the Work and historical Edition does not automatically govern a
library reproduction, provider metadata, OCR, embeddings, quotation,
publication, or redistribution. Any acquired file stays local,
access-controlled, and gitignored under its exact written terms. Public ToS
may expose permitted bibliography and provenance without exposing source
bytes.

## Next source-grounded move

The source-control and current-holding questions are now resolved enough to
stop guessing. If a concrete research question justifies external contact:

1. keep RSL and RNL requests separate;
2. ask each institution first for exact copy eligibility, condition, format,
   cost, and terms;
3. request bibliographic pages before a complete research reproduction;
4. treat digitization recommendation, paid fragment service, access, and
   rights permission as separate events;
5. create no ToS Item until exact bytes, custody, fixity, provenance, rights,
   and inventory exist.

Until a lawful source return exists, the route remains
**cross-institutionally reconciled and source-incomplete**.
