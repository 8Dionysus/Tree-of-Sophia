# Zarathustra Part-IV Provision Identity Research

Status: ordered, non-authoritative post-experiment extension research; one
bounded model-made Edition claim is admitted, no bibliography is accepted

Research date: 2026-08-01

## Purpose

The first three Zarathustra provision claims were established from their own
exact DTA source descriptions. Part IV requires a separate pass because its
private-print, printing, statement-year, and delivery history cannot be
represented by copying the earlier Schmeitzner pattern or by treating `1891`
as one undifferentiated publication date. This pass therefore asks only
whether the exact modeled Edition supports a reported provision statement.
Official records are checked first, established bibliographic and scholarly
routes second, and fresh/general-web results last.

## I. Exact official records

The tracked DTA/SUB Göttingen metadata snapshot has SHA-256
`c3699e9c9eddc2e0cb6be847710446ae42b8ac0a93ccec8671a73b00d7ba1dc2`.
It identifies the exact part-IV object as DTA ID `16843`, URN
`urn:nbn:de:kobv:b4-200905197250`, grounded in SUB Göttingen shelfmark
`8 PHIL I, 7352:4 RARA`. Its `sourceDesc/biblFull` reports:

- title `Also sprach Zarathustra`;
- volume 4, `Vierter und letzter Theil`;
- edition number 1;
- extent `134 S., [3] Bl., 21 S.`;
- publisher `Naumann`;
- place `Leipzig`;
- year `1891`.

The 2026-08-01 live
[DTA TEI header](https://www.deutschestextarchiv.de/api/tei_header/nietzsche_zarathustra04_1891)
had SHA-256
`3b74fd6d7a3c9a0ad885b5c3a3f62572fcd3cb59f36999ab5b74331fd90cc546`.
The header is regenerated, but the exact source description above remains
stable. The live Dublin Core response had SHA-256
`40d0796d4c03c8df38d8b5200b517cd31d24442c05b560b1ad5098dcaa6955f2`
and remained byte-identical to the captured response. The already recorded
CC BY-SA 4.0 versus stale CC BY-NC 3.0 metadata conflict remains a rights
review issue; it is not resolved by this bibliographic pass.

The current GND authorities preserve distinctions lost by the literal
`Naumann` label:

- [C. G. Naumann Verlag (Leipzig), GND
  1072998033](https://d-nb.info/gnd/1072998033) is a Company established in
  1871, with Leipzig as place of business; the live JSON-LD response had
  SHA-256
  `e7794e2d949d3561ba654aa5c51e010bd211a3a8239f232a83e289ea3d3e885f`;
- [Leipzig, GND 4035206-7](https://d-nb.info/gnd/4035206-7) is the Place;
  the live response had SHA-256
  `90ec7dafa7154eccc68306b23e46162cb74b9535259e3bda0c3b793f5e88bd46`;
- [Druckerei C. G. Naumann, GND
  16034133-4](https://d-nb.info/gnd/16034133-4), response SHA-256
  `a958f873911f96fc70ab8ed3eb01739b512b19e8a598f3a4f0479fd0b48e31cf`,
  is a related printing company, not the publisher Organization selected for
  this claim;
- the founder Person, GND `103754711X`, response SHA-256
  `22a0f4ff21cfbfe4ffbf26795d0edcf1cc2f5434bbdef7964f72046964bb3ae5`,
  is not substituted for the Organization.

The existing provisional ToS Place and Organization identities are reused
without upgrade or `same_as` assertion.

## II. Established bibliographic and production history

The current official
[DLA Marbach work record](https://www.dla-marbach.de/find/opac/id/AK00003291/)
distinguishes parts I–III, the 1885 private print of part IV, and the first
publisher edition of part IV at Naumann in Leipzig in 1891. It reports forty
private copies. This is useful independent chronology evidence, but it does
not turn the DTA catalog statement into a physical-title-page transcription
or an exact release event.

The current official
[HAAB record 741630192](https://portal.haab.klassik-stiftung.de/Record/741630192)
exposes the part-IV title, `Leipzig: Naumann, 1891`, 134 and 21 pages, and
multiple distinct holding copies. Direct automated retrieval returned HTTP
403, so no response body or digest is admitted. The open-view catalog result
is retained as an independent institutional route; no technical bypass was
attempted and no holding-copy equivalence is asserted.

The established HAdW historical-critical commentary, Andreas Urs Sommer,
*Nietzsche-Kommentar 6/2* (2013),
[DOI 10.11588/diglit.70914](https://doi.org/10.11588/diglit.70914), keeps the
production stages separate on
[pages 666](https://digi.hadw-bw.de/view/nietzsche_kom6_2/0666/text_ocr)
and [667](https://digi.hadw-bw.de/view/nietzsche_kom6_2/0667/text_ocr): the
1885 private small print, the 1890 arrangement involving Köselitz and
Naumann, printing completed in November 1890, the planned spring-1891 Leipzig
fair delivery, its suspension, and delivery of one thousand copies in March
1892. The two inspected page responses had SHA-256
`9acd1aeb6d210710ca4e2a498e96f550590d6a1f600a7fb013b77cf3f45819b8`
and
`ec92ac12ea6066530fa505d6f75477c569af1612dbf80b6433c122f2910e0691`.

The private-print copy count is not silently reconciled: DLA reports forty,
while the established commentary and other bibliographic routes report
forty-five. That conflict is outside this exact Edition provision claim and
must remain visible for a later chronology review.

## III. Fresh and general web last

Only after the official and established passes, a current general search
checked for 2025–2026 corrections or contradictions. A current
[ZVAB rare-book record](https://www.zvab.com/erstausgabe/sprach-Zarathustra-Vierter-letzter-Theil-NIETZSCHE/32470223821/bd)
corroborates C. G. Naumann, Leipzig, 1891 and repeats the printing-1890 /
delivery-March-1892 distinction. It remains a commercial lead, not authority.
Recent translation, teaching, and interpretation pages do not supply a
stronger exact Edition record or erase the production-stage distinctions.

## Admission decision

Admit one provision claim for the exact modeled part-IV Edition:

- basis: its DTA authority record, not a claimed physical-title-page
  transcription;
- reported statement: `Naumann; Leipzig; 1891`;
- normalized participants: the existing provisional Leipzig Place and
  C. G. Naumann Verlag Organization;
- temporal facet: year-precision `statement_date` with `catalog_supplied`
  posture;
- review posture: model-made and `unreviewed`;
- visibility: public metadata only.

The claim is not inferred from parts I–III. It does not assert composition,
the 1885 private print, November-1890 printing completion, spring-1891 planned
delivery, March-1892 actual delivery, public sale, a physical imprint
transcription, Edition or Item equivalence, accepted bibliography, source
text, rights clearance, human review, semantics, or canon promotion.
