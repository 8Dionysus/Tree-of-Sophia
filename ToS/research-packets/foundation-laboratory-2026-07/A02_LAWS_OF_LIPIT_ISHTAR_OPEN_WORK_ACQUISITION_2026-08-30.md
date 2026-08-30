# A02 — Laws of Lipit-Ištar open-work acquisition

Date of research cut: 2026-08-30
Candidate: `open-work-candidate.laws-of-lipit-ishtar`
Frozen queue snapshot SHA-256: `c800e6fb95ef7502fc1f15fb836bdb47cd338ddf521ce4064941b70d74cd49b6`
Candidate SHA-256: `9bb253a164e6489fbc73722c8f81dfb4f850ed4a4fec3253735452ae88e1e643`
Authority: acquisition and rights evidence only; not source-text, translation, legal-history, semantic, graph, canon, or publication authority.

## Result first

The existing provider-independent Work, critical composite, and Louvre artifact identities were reused. Two exact useful digital objects were acquired:

| layer | provider object | local File | bytes | pages | SHA-256 | rights |
|---|---|---|---:|---:|---|---|
| annotated Sumerian composite representation | ETCSRI/Oracc `Q000613`, static `html` view | `representations/oracc-etcsri-q000613-static-html/payload/Q000613.html` | 56,318 | — | `d5fea8eab41c2570a32d69cff978dada64a905c13a6f3bd35f67ba272501f5f6` | CC BY-SA 3.0; attribution and ShareAlike |
| modern Polish translation and study | Tytus Mikołajczak, *Prawa Lipit-Esztara*, DOI `10.14746/cph.2022.2.2` | `representations/mikolajczak-prawa-lipit-esztara-2022-pdf/payload/cph-2022-2-2.pdf` | 236,010 | 21 | `61c2539463b95c791d0a41d004c5b901f6d145f15d25355041f73b364e47aadc` | CC BY-SA 4.0; attribution and ShareAlike |

Both Files remain local-only, unreviewed source witnesses. The ETCSRI File is a modern annotated reconstruction; the Mikołajczak File is a modern authored translation and study. Their presence does not accept text, witness membership, segmentation, translation quality, legal interpretation, meaning, or philosophical relevance.

## Identity route

- ToS Work: `tos.work.sumerian-law.laws-of-lipit-ishtar`.
- ToS provider-independent composite: `tos.composite.sumerian.laws-of-lipit-ishtar`.
- Existing physical member witness: `tos.artifact.sumerian.uncertain.louvre-ao-05473`.
- Louvre holding identity: AO 5473, ARK `ark:/53355/cl010166109`, publication coordinate TCL 15,34.
- ETCSRI object: `Q000613`, displayed as *Laws of Lipit-Eštar*, coordinated with RIME `4.01.05.add10`.
- CDLI current composite route: `Q000613` / `P464355`, inscription revision `2269336`, with thirteen displayed witnesses at the earlier verified research cut.
- Modern translation publication: Tytus Mikołajczak, *Prawa Lipit-Esztara*, *Czasopismo Prawno-Historyczne* 74(2), 37–57, DOI `10.14746/cph.2022.2.2`.

Shared Q-numbers and titles corroborate routes; they do not make the ancient Work, critical reconstruction, physical fragments, Oracc response, article, translation, and local Files one identity. The modern article is recorded as a File-backed composite representation with a complete bibliographic part, while formal Expression/Edition/Item admission remains for later review.

## Ordered discovery

The measured discovery record is `ToS/source-witnesses/discovery/runs/laws-of-lipit-ishtar-open-work-route.2026-08-30.v1.json`. Its external timing receipt is `ToS/source-witnesses/discovery/timings/laws-of-lipit-ishtar-open-work-route.2026-08-30.v1.json`.

Ten channels were executed in protocol order. Every channel has a positive `time.perf_counter_ns` measurement. HTTP 403 and Oracc's command-line TLS transport failure remain timed outcomes rather than zero or unknown sentinels.

### National and holding records

- The Library of Congress books endpoint returned HTTP 403 to the bounded probe; no result was invented.
- The Louvre record exactly identifies AO 5473 and exposes JSON plus nine photographs.
- Every photograph has a named Louvre, GrandPalaisRmn, distribution, or photographer credit. Louvre reuse terms permit only an enumerated non-collective set of private, museographic, scientific, educational, and limited EU-publication uses; other uses require permission or payment. This is useful rights evidence but not an open license for the Goal. No photograph was downloaded.

### Publisher and scholarly projects

- The originating Adam Mickiewicz University journal record identifies the Mikołajczak article, author, ORCID, DOI, issue, pages, publication date, PDF, and CC BY-SA 4.0 license.
- The exact PDF's first page independently repeats the CC BY-SA 4.0 statement. It is therefore stronger than a third-party ResearchGate label and was acquired from the publisher.
- ETCSL catalogue entry `3.4.2` is unlinked and exposes no exact composition content object.
- ETCSRI/Oracc exposes the exact annotated representation at `Q000613` and a static `html` route. The ETCSRI project footer releases project content under CC BY-SA 3.0. General Oracc guidance requires attribution and ShareAlike and keeps independently supplied images, PDFs, and downloads separate.
- CDLI supplies the strongest current composite, witness, and bibliography route. Its custom academic-reuse language was not treated as the standard open-license basis for either acquisition.

### Aggregators, identifiers, open libraries, and general web

- Europeana returned HTTP 403; no object or rights conclusion was inferred.
- The first five Crossref results were preserved in order: a commentary dataset, the Roth chapter, Steele's 1948 article, Kramer's 1947 article, and a review. They remain bibliographic leads because no exact open-license File was established through those results.
- Internet Archive exact-title search returned `numFound=0`.
- General web search was last. It found the Polish title variant, which was then resolved back to the originating UAM journal before rights assessment and acquisition. A ResearchGate copy was rejected as the acquisition source because its displayed license metadata conflicted, while the originating record and PDF agreed on CC BY-SA 4.0.

## Acquisition and transport boundary

The command-line client rejected the Oracc host because the server did not present a locally complete certificate chain. Certificate verification was not disabled. The exact HTTPS object loaded normally in the application's validated browser, and the canonical static response body was captured from that session. `technical_access_bypass_used` remains `false`.

The UAM PDF was downloaded directly over verified HTTPS from the originating journal. PDF inspection found 21 pages, no encryption, and an explicit CC BY-SA 4.0 statement on the first page.

For each destination, the machine storage preflight reported sufficient capacity and denied only machine-owned automation writes into the owner-controlled `/srv/AbyssOS` tree. Under the operator's explicit acquisition authorization and the ToS source-witness route, both Files were stored in candidate representation payloads rather than diverted into the host artifact cache.

## Rights layers

- Ancient Work: not treated as the rights-bearing identity for modern bytes.
- ETCSRI annotated HTML: positively licensed under CC BY-SA 3.0.
- Mikołajczak Polish translation/study PDF: positively licensed under CC BY-SA 4.0 at both the journal record and File surface.
- CDLI text and metadata: governed by separate custom terms; not inherited from ETCSRI.
- Louvre artifact metadata: retained as an operational identity route.
- Louvre photographs: rejected from acquisition because their custom limited-use terms are not an open license.
- Roth, Steele, Kramer, Wilcke, and other translations or editions: remain separate unresolved rights routes.
- Server processing and redistribution: licensed with attribution and ShareAlike for the two exact acquired Files; ToS nevertheless keeps both payloads local-only until human review.

## Operational relations

The candidate, discovery, timing receipt, selected and rejected results, acquisition events, representations, Files, per-File rights records, provider-independent composite, physical artifact metadata, source planting, and this packet are connected by explicit refs. Formal Expression/Edition/Item, translation-alignment, semantic, graph, legal-historical, canon, and publication relations remain review work and are not silently created.

## Terminal posture

Recommended terminal status: `held_source_witness`.

The outcome is mechanically strong but human-unreviewed. A later reviewer may compare ETCSRI, CDLI, Louvre, and Mikołajczak layer boundaries; assess exact attribution wording; review the Polish translation against a frozen Sumerian layer; and decide whether formal Expression, Edition, Item, source-text, or translation-alignment objects should be admitted.
