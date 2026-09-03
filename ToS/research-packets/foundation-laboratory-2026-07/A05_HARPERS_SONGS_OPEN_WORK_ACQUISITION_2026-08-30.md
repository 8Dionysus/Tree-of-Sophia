# A05 — Harper's Songs / Papyrus Harris 500: open-work acquisition

Date: 2026-08-30
Status: one frozen open-work candidate resolved to held source witnesses; no human philological or legal acceptance
Candidate: `open-work-candidate.harpers-songs`
Candidate digest: `e229f3fff8c580172d738372b30e078ff080d03e5bbebc5384845e138eee637f`

## Outcome

The candidate label is not admitted as one closed ancient Work. The current evidence supports three separate, linkable layers:

1. physical Papyrus Harris 500, British Museum EA10060 / 1872,1101.2 / TM 380901;
2. a provisional synoptic composite for Harper's Songs witnesses and publications, with one reported member observation for the Recto 6–7 Antef Song;
3. Goodwin's distinct 1874 modern scholarly Work, English expression, London edition, NLS/Internet Archive Item, and exact PDF File.

This separation preserves the alleged mansion-of-Intef source, the New Kingdom papyrus copy, surrounding love songs and tales, modern historical translation, current TLA text record, photographs, and exact files as different entities.

## Ordered discovery

The reconciled discovery run is `ToS/source-witnesses/discovery/runs/harpers-songs-open-work-route.2026-08-30.v1.json`.

| # | Channel | Useful result | Decision |
|---:|---|---|---|
| 1 | K10plus | PPN 1894019164, exact Goodwin 1874 article and IA binding | select |
| 2 | British Museum | EA10060, recto 6.2–7.3 description, 74-image object, four exact licensed image-use pages | select and acquire four JPEGs |
| 3 | TLA | object `AZQUE…`, text `CRNQ…`, collective caption `WLWU…`, exact scholarly hierarchy and bibliography | select metadata; do not acquire TLA text |
| 4 | Europeana / IIIF | no exact object or manifest; formal endpoint returned 403 | zero result preserved |
| 5 | Trismegistos | TM 380901 corroborated by stronger records; direct TLS transport failed | identifier lead only; no bypass |
| 6 | Open Library | Work OL15247338W, Edition OL24821288M, IA binding | select metadata |
| 7 | Internet Archive | exact NLS PDF, license, size, SHA-1, MD5, scan provenance | select and acquire |
| 8 | National Library of Scotland | current faithful-copy/CC0 digitisation policy and byte-distinct optimized PDF | retain rights evidence; reject duplicate payload |
| 9 | British Library | Goodwin 1817–1878 authority evidence | select metadata |
| 10 | general web | repeated stronger sources; no new reusable object | final gap check only |

General web search was last. No access control, paid access, authentication, or technical restriction was bypassed.

## Timing debt closed

The active run has a generated monotonic timing receipt at `ToS/source-witnesses/discovery/timings/harpers-songs-open-work-route.2026-08-30.v1.json`. Every active channel has a positive measurement; the historical zero-sentinel debt is not carried into this terminal iteration.

| Channel | Seconds | Outcome |
|---|---:|---|
| K10plus | 0.809766 | HTTP 200 |
| British Museum | 0.661798 | HTTP 200 |
| TLA | 1.210829 | HTTP 200 |
| Europeana | 0.271739 | HTTP 403 |
| Trismegistos | 0.323431 | transport error |
| Open Library | 0.384341 | HTTP 200 |
| Internet Archive | 0.568386 | HTTP 200 |
| NLS | 0.949572 | HTTP 200 |
| British Library | 0.990734 | HTTP 200 |
| final web check | 0.202208 | HTTP 200 |

These values measure request-through-first-16,384-response-bytes only. They are not human research time, interpretation time, rights-review time, or proof of source quality.

## Acquired exact objects

| Layer | File | Bytes | SHA-256 | Rights |
|---|---|---:|---|---|
| BM photograph 20450001 | `mid_00020450_001.jpg` | 408,248 | `73c05fb29c3491d5fccac36f2fb2c5b2cb403f4591a394aa49b2cd148ccb3a56` | CC BY-NC-SA 4.0; credit © The Trustees of the British Museum |
| BM photograph 20451001 | `mid_00020451_001.jpg` | 416,568 | `a93fc3da7e1778f9d170b2e918f7d863e6be8b0ef7920433841abd5f69eba9ad` | CC BY-NC-SA 4.0; same credit and conditions |
| BM photograph 20452001 | `mid_00020452_001.jpg` | 413,518 | `25319705c549e625151159102045465976f8cc5f69ee9b72eecf21ad9cdd73e5` | CC BY-NC-SA 4.0; same credit and conditions |
| BM photograph 20453001 | `mid_00020453_001.jpg` | 405,220 | `646f4c505dcbc43691f81c5551cc6793be537579a0407b9c1da0bafa3ff278ba` | CC BY-NC-SA 4.0; same credit and conditions |
| Goodwin 1874 NLS/IA scan | `onfoursongsconta00good.pdf` | 672,231 | `71ca30507a61a791b503102b43c034aad08b45c5d78bbedf16043632add33675` | CC BY-NC-SA 2.5 Scotland; underlying historical layers separately term-reviewed |

The Goodwin PDF SHA-1 `1014a4a0c4965a71a57d63b985ee53cc5859b38b` matches Internet Archive metadata. All four JPEGs are 1000 × 1000 and have separate source SHA-1 values and object-specific image-use records.

The host storage preflight denied machine-owned automation writes to the protected project root while capacity remained sufficient. The operator's explicit Goal authorization governed these exact repository acquisitions; the denial scope, authorization reference, and absence of bypass are recorded on every representation and Item.

## Rights by layer

- Artifact and composite packets retain bounded factual metadata only.
- Each exact BM JPEG is separately licensed CC BY-NC-SA 4.0; attribution, non-commercial, share-alike, license-link, and change-notice conditions remain attached.
- Goodwin's identified 1874 Work, English expression, and edition are outside the reviewed GB, MX, and US terms; no human legal review has occurred.
- The exact NLS/IA PDF is conservatively governed by the explicit CC BY-NC-SA 2.5 Scotland item license even though current NLS policy supplies a CC0 waiver for digitisation rights to the extent they subsist.
- TLA permits bounded academic quotation of individual data sets but does not currently supply a standard free license for acquiring the relevant subcorpus as a reusable ToS payload; no TLA text bytes were stored.
- Museum prose, other photographs, provider databases, inscription text, transcription, transliteration, modern translations, and later scholarship remain separate.

## Operational relation chain

The candidate, discovery run, selected results, acquisition event, artifact, composite, visual representations, Goodwin bibliographic ladder, Files, rights records, resource inventory, planting, and terminal receipt are linked by stable IDs or repository refs. Work–Expression, Expression–Edition, Edition–Item, authorship, and translation-responsibility claims are explicit evidence-bearing claim packets.

The composite member relation is only `reported_witness_of`; it is not projected as an accepted ancient Work identity, fixed passage, semantic fact, graph edge, or canon relation.

## Remaining review

- determine whether “Harper's Songs” should stay a genre/corpus grouping or be split into one or more durable ancient Work identities;
- review the mansion-of-Intef attribution and transmission relation;
- manually resolve exact columns and passage coverage in the four retained photographs;
- review historical Goodwin transcription and translation only as reception evidence, not as current philology;
- obtain human legal/editorial approval before any publication decision.

The candidate can terminate as `held_source_witness`: useful exact open objects are held, while Work identity, ancient text, semantics, graph, canon, and publication remain unaccepted.
