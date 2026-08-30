# A02 — Laws of Hammurapi open-work acquisition

Date of research cut: 2026-08-30
Candidate: `open-work-candidate.laws-of-hammurapi`
Frozen queue snapshot SHA-256: `800afddd7a210cc400e809f761b5033991926e41ebbc39f1dcd0dc9ceb6cbb76`
Candidate SHA-256: `d59a8c13aa74b069480a154ea9c48d9fe940464f17088f7eb2c6f6dfb7f7bc18`
Authority: acquisition and rights evidence only; not source-text, translation, legal-history, semantic, graph, canon, or publication authority.

## Result first

The existing provider-independent Work, critical composite, and Louvre SB 8 artifact identities were reused. Two exact useful digital objects were acquired:

| layer | provider object | local File | bytes | pages or dimensions | SHA-256 | rights |
|---|---|---|---:|---:|---|---|
| modern English translation | Martha T. Roth, *Laws of Hammurabi*, UCF STARS item 134, latest revision May 2026 | `representations/roth-laws-of-hammurabi-revised-2026-pdf/payload/roth-laws-of-hammurabi-revised-2026.pdf` | 342,577 | 35 pages | `ddc38f90b258f47acd4bb42fa5e9319288bf7930f82e6e222c53ebed91947980` | CC BY-NC 4.0; attribution and noncommercial conditions |
| artifact photograph | Mbzt, rear or inscription-bearing face of Louvre SB 8, 2011; Commons filename says `recto` | `representations/commons-mbzt-rear-photograph-2011/payload/P1050551_Louvre_code_d_Hammurabi_recto_rwk.JPG` | 1,538,719 | 1153 × 2000 px | `e357d0faad60c5fb5b8b6c4ba6282a34fdd2d6c48e81d8d8ae3a837bc831f8b1` | CC BY 3.0; attribution and license notice |

The Roth PDF remains a local-only, unreviewed source witness. The Commons JPEG is a separately identified public visual representation and is tracked because its exact File has an explicit standard open license. Neither File is the ancient Work, the provider-independent composite, the physical stela, an accepted source text, an accepted translation, a semantic claim, or canon.

## Identity route

- ToS Work: `tos.work.akkadian-law.laws-of-hammurapi`.
- ToS provider-independent composite: `tos.composite.akkadian.laws-of-hammurapi`.
- ToS physical artifact: `tos.artifact.old-babylonian.susa.hammurabi-stele-sb-8`.
- Louvre holding: SB 8, former number AS 6064, ARK `ark:/53355/cl010174436`.
- CDLI physical-witness route: `P249253`; current composite route: `Q006387` / `P464358`, inscription revision `2337705`, with 27 displayed witnesses at this research cut.
- UCF publication: Martha T. Roth, *Laws of Hammurabi*, Open Educational Resources for the Ancient Near East, repository item 134; record citation 2022, posted 2023-02-27, exact PDF revised May 2026.
- Commons photograph: file page ID `16931477`, creator Mbzt, source SHA-1 `54c88f49034fc86a48b4b4d8bed7b07841b6b5d1`.

The local Roth PDF is a File-backed representation of a modern authored translation. Formal Expression, Edition, Item, translation alignment, and source-text admission remain later review work. The local photograph is a File-backed visual representation of the artifact. Its filename's `recto` wording and its description as the rear face are retained as a source-visible conflict rather than silently normalized.

## Ordered discovery and measured time

The discovery record is `ToS/source-witnesses/discovery/runs/laws-of-hammurapi-open-work-route.2026-08-30.v1.json`. Its immutable external timing receipt is `ToS/source-witnesses/discovery/timings/laws-of-hammurapi-open-work-route.2026-08-30.v1.json`.

Nine channels were executed in protocol order, each with a positive `time.perf_counter_ns` measurement:

| sequence | channel | elapsed seconds | outcome |
|---:|---|---:|---|
| 1 | Library of Congress | 0.240912 | HTTP 403 retained as a measured null result |
| 2 | BnF | 0.998114 | exact Scheil bibliographic control selected |
| 3 | Louvre | 1.067576 | artifact identity selected; collection images rejected; 3D route deferred |
| 4 | UCF STARS | 0.389302 | exact Roth PDF selected and acquired |
| 5 | CDLI | 1.242662 | current composite and witness membership selected as metadata evidence |
| 6 | Wikimedia Commons | 0.330975 | exact Mbzt JPEG selected and acquired |
| 7 | Crossref | 1.400270 | bibliographic leads retained only |
| 8 | Internet Archive and Project Gutenberg | 0.400482 | historical digital objects deferred for rights reasons |
| 9 | general web | 0.822368 | result routes retained; no weaker copy substituted |

These positive values close the earlier unknown-sentinel timing debt for the active acquisition loop. Historical receipts remain immutable and are superseded rather than rewritten.

## Source findings

### Originating and holding sources

- The BnF catalogue identifies Jean-Vincent Scheil's *Textes élamites-sémitiques* and its 1902 second series as the initial publication route. It establishes bibliography, not digital-object reuse rights.
- The Louvre record identifies SB 8 and exposes 105 credited collection images. Current Louvre terms provide enumerated custom uses rather than a standard open license. No Louvre-originating photograph was acquired.
- The Louvre's exact `skfb.ly` route resolves to an official Rmn-GrandPalais Sketchfab model. The page offers a light web representation, names Rmn-GrandPalais and the Louvre, marks the model `NoAI`, provides no standard open license or download authority, and directs high-quality requests to `agence.photo@rmngp.fr`. No 3D payload was acquired.
- CDLI provides the strongest current composite and member route. The exact response was fingerprinted, but its modern annotated text body was not acquired because no exact standard license was established for those bytes.

### Positively licensed Files

- UCF STARS is the originating record and download route for Roth's modern English translation. Both the record and exact PDF identify the work and author; the repository exposes CC BY-NC 4.0. The exact PDF was downloaded through the ordinary browser route without a technical access bypass.
- Wikimedia Commons supplies the exact Mbzt JPEG with CC BY 3.0, author, date, dimensions, byte size, source SHA-1, and originating File page. The File was downloaded from the API-exposed original-media URL without a technical access bypass.

### Historical editions and translations

- Internet Archive exposes scans of Scheil's 1902 volume, including text and grayscale PDFs and DjVu XML, but its metadata contains no `licenseurl` or rights statement for the exact digital objects. No bytes were acquired.
- Project Gutenberg item 17150 preserves the 1903 Johns translation and states public-domain status in the United States only. That jurisdiction-limited posture was retained as a lead rather than treated as a three-jurisdiction open acquisition.

## Acquisition and storage boundary

Both exact destination preflights reported sufficient host capacity and denied only machine-owned automation writes into the owner-controlled `/srv/AbyssOS` project tree. Under the operator's explicit download authorization and the ToS source-witness owner route, the Files were stored in their exact representation payload homes rather than diverted into the host artifact cache.

The Roth PDF is ignored as local-only payload while its representation, rights, digest, provenance, and catalog membership remain tracked. The Commons JPEG is a tracked public payload because its exact File carries a positive standard open license. Fixity was checked after placement.

## Rights layers

- Ancient Work: not used as a rights shortcut for modern bytes.
- Roth translation PDF: exact File licensed CC BY-NC 4.0; other translations and editions do not inherit that license.
- Commons photograph: exact File licensed CC BY 3.0; Louvre and CDLI photographs do not inherit that license.
- Louvre photographs: rejected from acquisition because custom enumerated-use terms are not the Goal's standard-open-license threshold.
- Official Rmn-GrandPalais 3D model: deferred as a permission route; no payload acquired.
- CDLI annotated text: metadata and revision route retained; text body not acquired.
- Scheil and Johns historical digital objects: bibliographic leads retained; exact digital-object or jurisdiction evidence is insufficient for this iteration.
- Public redistribution and server processing: allowed with the exact Commons photograph's CC BY conditions; the Roth PDF remains local-only pending human review despite its positive noncommercial license.

## Operational relations

The frozen candidate, discovery record, positive timing receipt, selected, rejected, and deferred results, two acquisition events, two representations, two Files, per-File rights records, provider-independent composite, physical artifact, source plantings, and this packet are connected by explicit refs. Formal Expression, Edition, Item, source-text, translation-alignment, semantic, graph, legal-historical, canon, and publication relations remain review work and are not silently created.

## Terminal posture

Recommended terminal status: `held_source_witness`.

The acquisition is mechanically strong but human-unreviewed. A later reviewer may verify attribution wording, compare the Roth translation with a frozen Akkadian layer, resolve the Commons face-label conflict, decide whether to admit formal Expression/Edition/Item identities, and separately evaluate CDLI text, historical scans, or the official 3D permission route.
