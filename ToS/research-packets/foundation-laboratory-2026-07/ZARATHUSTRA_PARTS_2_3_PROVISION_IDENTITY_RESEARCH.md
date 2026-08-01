# Zarathustra Parts II–III Provision Identity Research

Status: ordered, non-authoritative post-experiment extension research; two
bounded model-made Edition claims are admitted, no bibliography is accepted

Research date: 2026-08-01

## Purpose

The first-part provision extension deliberately did not copy repeated
Schmeitzner and Chemnitz labels into parts II and III. This follow-up asks the
narrower question separately for each Edition: does its own exact authority
record support a provision statement? Official records are checked first,
established bibliographic and scholarly routes second, and fresh/general web
results last.

## I. Exact official records

The two tracked source snapshots already bind distinct DTA/SBB-PK objects:

- part II: DTA `nietzsche_zarathustra02_1883`, DTA ID `16841`, URN
  `urn:nbn:de:kobv:b4-200905194308`, snapshot SHA-256
  `28c91ee4f2b825f54713853fb3aed7c4fa7f2193e6149ac277c7e396d615ea85`;
- part III: DTA `nietzsche_zarathustra03_1884`, DTA ID `16842`, URN
  `urn:nbn:de:kobv:b4-200905194314`, snapshot SHA-256
  `76531c78e097f34cdef2f96f67c968f1abd0a30ae26fe3d3cdcd3eddda8aff45`.

The 2026-08-01 live
[part-II TEI header](https://www.deutschestextarchiv.de/api/tei_header/nietzsche_zarathustra02_1883)
had SHA-256
`2383716ff66e314677bfb67c81172730f4b610729faf0ac79ce420d48ea24232`.
Its own `sourceDesc/biblFull` identifies volume 2, first edition, 103 pages,
publisher `Schmeitzner`, place `Chemnitz`, and year `1883`. Its live Dublin
Core response remained byte-identical to the captured response at SHA-256
`1dc8d5699284e3b3d9b731210119e2d9e8f3b241c7c16ed0d03af13548d5e993`.

The live
[part-III TEI header](https://www.deutschestextarchiv.de/api/tei_header/nietzsche_zarathustra03_1884)
had SHA-256
`d4e22b66428804ac0718b1877dd953f757ab7ecf68e9aad2f141e3a8a872889e`.
Its separate `sourceDesc/biblFull` identifies volume 3, first edition, 119
pages, publisher `Schmeitzner`, place `Chemnitz`, and year `1884`. Its live
Dublin Core response also remained byte-identical to the captured response at
SHA-256
`7b064c9e8818f2c336ae2d48f3e0488c7afec18e50dcefb4616e82e731665afa`.

The current DTA HTML records independently expose the same volume, publisher,
place, year, first-edition, URN, holding library, and shelfmark fields for
[part II](https://www.deutschestextarchiv.de/nietzsche_zarathustra02_1883)
and [part III](https://www.deutschestextarchiv.de/book/show/nietzsche_zarathustra03_1884).
The shared SBB-PK shelfmark does not collapse the volumes into one Edition or
make their digital Items equivalent.

The normalized identities are reused from the preceding exact identity pass:
[Ernst Schmeitzner, Verlagsbuchhandlung, GND
1063670306](https://d-nb.info/gnd/1063670306) remains a Corporate Body and
[Chemnitz, GND 4029702-0](https://d-nb.info/gnd/4029702-0) remains a Place. A
fresh Corporate-Body linked-data response reproduced the already recorded
SHA-256
`9a40f3cf923e4d63bbb44876d64cdc5f944cac833b3978147f41ebbf19515e68`;
the subsequent Place request was rate-limited, so the same-day part-I
authority receipt is retained rather than replaced by an empty response.
Neither local identity is upgraded from provisional, and the Person Ernst
Schmeitzner, GND `118823698`, remains an explicit negative control.

## II. Established bibliographic and scholarly routes

The current ZB Zürich
[e-rara structure record](https://www.e-rara.ch/zuz/content/structure/22865485)
describes the three-part Chemnitz publication as `Verlag von Ernst
Schmeitzner, 1883-1884` and exposes separate title-page nodes for Band 2 and
Band 3. This is an independent holding and digital-object route, not the
SBB-PK Item behind DTA and not a basis for Item equivalence.

The Heidelberger Akademie der Wissenschaften historical-critical commentary
keeps the relevant work stages separate in its 2024 volumes for
[parts I–II](https://doi.org/10.1515/9783110293319) and
[parts III–IV](https://doi.org/10.1515/9783110293333). It is retained as the
established production-history control: composition, manuscript completion,
proofing, printing, receipt, distribution, and sale must not be compressed
into the publication-statement year. The two ToS claims therefore report only
what the exact authority records state.

## III. Fresh and general web last

Only after the official and established passes, a current general search
looked for 2025–2026 corrections or contradictions. A current rare-book
catalog corroborates that parts I–II appeared under Schmeitzner in 1883 and
part III in 1884, but it is not selected over DTA or e-rara. Recent articles,
translations, teaching pages, and corpus citations either repeat coarse
whole-work dates or concern interpretation and language use. None supplies a
stronger exact Edition identity, a contradictory imprint, or evidence for a
printing or public-release date.

## Admission decision

Admit two independent provision claims under the existing contract:

- exact part-II Edition: authority-record statement `Schmeitzner; Chemnitz;
  1883`;
- exact part-III Edition: authority-record statement `Schmeitzner; Chemnitz;
  1884`.

Both claims reuse the provisional Chemnitz Place and historical Schmeitzner
Organization, preserve year precision as `statement_date`, remain model-made
and `unreviewed`, and expose public metadata only. They are admitted from two
separate DTA source descriptions—not from label propagation or the part-I
claim.

No physical-title-page transcription, printing date, release date, copy or
Edition equivalence, publisher–Person equivalence, accepted bibliography,
source text, image, human review, semantic assertion, publication right, or
canon promotion is created.
