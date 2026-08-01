# Zarathustra Part-I Provision Identity Research

Status: ordered, non-authoritative post-experiment admission research; one
bounded model-made Edition claim is admitted, no bibliography is accepted

Research date: 2026-08-01

## Purpose

The two-Edition provision-activity experiment passed its declared structural
and negative gates, so the optional third case was researched rather than
populated by analogy. The check again kept official records, established
scholarship, fresh live evidence, and general-web discovery in that order.

## I. Official records and exact source evidence

The fixity-verified local DTA TEI for part I has SHA-256
`d3fa5f8af39d87c8f0ede7a0b52b26da0c57e73125d3b0bd2a836b80eae24a4b`.
Its encoded `sourceDesc/biblFull/publicationStmt` reports publisher
`Schmeitzner`, publication place `Chemnitz`, and publication year `1883` for
the separately modeled first part. The 2026-08-01 live DTA header response had
SHA-256
`014298c4153aef9cc085a54187c4e00eb7659690106aca2bca41dced20f07b32`;
the regenerated header bytes differ from the captured 2026-07-28 response,
while the relevant source description remains unchanged. The live Dublin Core
response remained byte-identical to the captured response at SHA-256
`371d74b552cc457206b1f019312fd16515a53da6cc62f61917e70d3a3b7f7e88`.
The existing tracked source-metadata snapshot remains the stable source return.

The current GND linked-data records make an identity distinction that a flat
publisher string would lose:

- [Chemnitz, GND 4029702-0](https://d-nb.info/gnd/4029702-0) is the place
  authority; the 2026-08-01 JSON-LD response had SHA-256
  `393599a954676bd92e05aac2f61c56707ab34f6a0b7f1699932013e3cc308daf`;
- [Ernst Schmeitzner, Verlagsbuchhandlung, GND
  1063670306](https://d-nb.info/gnd/1063670306) is a Corporate Body,
  established in 1874, with Chemnitz as its place of business; the response
  had SHA-256
  `9a40f3cf923e4d63bbb44876d64cdc5f944cac833b3978147f41ebbf19515e68`;
- [Schmeitzner, Ernst, GND
  118823698](https://d-nb.info/gnd/118823698) is a differentiated Person,
  affiliated with that corporate body and active in Chemnitz; the response
  had SHA-256
  `be879bce6c339990a750302316c8b4efd5ceec02afb51eb2dd126cb8c18f9814`.

The provision participant should therefore normalize to the historical
publishing organization, not silently to the person. Both local identities
must remain provisional and `no_equivalence_claim`: the GND match is strong
authority evidence, but no human identity review occurred.

## II. Established bibliographic and production history

The HAdW historical-critical commentary preserves a different event layer: it
records Nietzsche sending the completed part-I manuscript to Schmeitzner in
Chemnitz on 14 February 1883 and separately discusses the later publication
and reception. This supports the existing rule that manuscript transmission,
publication statement, production, receipt, and sale must not be collapsed
into one date or event.

The live ZB Zürich
[e-rara record](https://www.e-rara.ch/zuz/content/structure/22865485) identifies
the 1883-1884 Schmeitzner publication and exposes `Band 1` with its own title
page. It is an independent holding/digital-object route, not the SBB-PK Item
behind the DTA TEI. It corroborates the Edition-level place/publisher boundary
but cannot be used to manufacture Item equivalence or a title-page
transcription for the local DTA witness.

## III. Fresh live check and general web last

The final search pass checked current 2025-2026 results only after the records
above. A current DNB record for the Friedrich-Nietzsche-Stiftung's 2022
part-I reprint explicitly reports that its source was the first part published
by Ernst Schmeitzner in Chemnitz in 1883. A Deutsche Digitale Bibliothek
record updated 2026-03-17 independently exposes the 1879 corporate form
`Ernst Schmeitzner, Verlagsbuchhandlung in Chemnitz`. Fresh 2025-2026
translation, interpretation, and publisher pages did not supply a stronger
or contradictory historical identity. They remain relevant to later
translation and reception work, not to this provision assertion.

No current result supports treating all three Schmeitzner parts as one
Edition, inferring a public-release day, or populating the other two Editions
from the shared publisher and place labels.

## Admission decision

Admit one third positive provision case under the already exercised contract:

- subject: the exact part-I Edition only;
- basis: the DTA authority record, not a claimed physical title-page
  transcription;
- reported statement: `Schmeitzner; Chemnitz; 1883`;
- normalized participants: provisional Chemnitz Place and provisional Ernst
  Schmeitzner Verlagsbuchhandlung Organization;
- temporal facet: year-precision statement date, not composition, printing,
  receipt, or public-release time;
- review posture: model-made and `unreviewed`;
- visibility: public metadata only;
- negative boundary: parts II and III receive no claim merely because their
  labels repeat Schmeitzner and Chemnitz.

This admits one contrasting case, not bulk population. The exact physical
imprint transcription, corporate succession, publisher-person equivalence,
other Schmeitzner Editions, and any accepted bibliography remain deferred.
