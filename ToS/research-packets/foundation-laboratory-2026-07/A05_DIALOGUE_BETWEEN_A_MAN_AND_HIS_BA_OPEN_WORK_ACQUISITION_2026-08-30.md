# A05 — Dialogue between a Man and His Ba / P. Berlin 3024: open-work acquisition

Date: 2026-08-30
Status: one frozen open-work candidate resolved to held source witnesses; no human philological or legal acceptance
Candidate: `open-work-candidate.dialogue-between-a-man-and-his-ba`
Candidate digest: `6ed989583f23f77cce0fb4014ad2d9e934f324bf32dad10f36a6d2fedd91b2fd`

## Outcome

The candidate label is not admitted as one closed ancient Work. The current evidence supports a provider-independent physical-artifact packet for P. Berlin 3024 and two exact, openly licensed historical line-art representations of that papyrus. It also preserves, without accepting, the reported joins to P. Amherst III and Mallorca fragments, the reconstructed beginning, and the several modern title translations.

The packet therefore keeps apart:

1. physical P. Berlin 3024, including the current SMB component P 3024/C;
2. P. Amherst III and the reported Mallorca fragments;
3. the TLA aggregate object and subordinate composition record;
4. Lepsius's historical plates 111–112;
5. modern editions, transcriptions, translations, reconstruction articles, and repository copies;
6. the provisional ancient Work boundary called *Dialogue between a Man and His Ba*.

No ancient Work, Expression, Edition, Item, accepted text, fragment join, translation, semantic claim, graph fact, canon state, or publication decision is created by this iteration.

## Ordered discovery

The reconciled discovery run is `ToS/source-witnesses/discovery/runs/dialogue-between-a-man-and-his-ba-open-work-route.2026-08-30.v1.json`.

| # | Channel | Useful result | Decision |
|---:|---|---|---|
| 1 | K10plus | PPN 1187984728, Erman's exact 1896 publication and life dates | select bibliography; no File-rights conclusion |
| 2 | Staatliche Museen zu Berlin | object `obj:1388678`, current P 3024/C mounting, material and dimensions | select physical-object metadata; no image acquisition |
| 3 | Morgan Library & Museum | Amherst Egyptian Papyrus 3, four fragments and custody | select holding metadata; direct request returned 403 and no exact open image license was observed |
| 4 | De Gruyter | DOI `10.1515/zaes-2017-0002` for the Mallorca reconstruction article | select originating publication metadata only |
| 5 | Thesaurus Linguae Aegyptiae | object `N3YS…`, text `XZIG…`, collective caption `S6UT…`, bibliography and reported joins | select scholarly hierarchy; do not acquire TLA text under limited terms |
| 6 | University of Tokyo IIIF | exact Lepsius plates 111 and 112, IIIF manifest, CC BY 4.0 | select and acquire two maximum-resolution JPEGs |
| 7 | Liverpool Repository | exact published article PDF and purpose-limited repository policy | reject public payload: not a standard open license and not commercial-purpose compatible |
| 8 | Europeana | three exact searches; no retained exact result | zero result preserved |
| 9 | Crossref / OpenAlex | exact DOI, green OA location, no named license | select identifier evidence; OA visibility is not rights clearance |
| 10 | BHL / Internet Archive | exact multi-work 1896 proceedings volume and provider fixity | reject public payload: Mexico term remains through 2037 and topology is multi-work |
| 11 | Google Books | duplicate historical-volume lead | defer to stronger originating and rights-bearing sources |
| 12 | general web | repeated stronger sources; no additional exact open object | final gap check only |

General web search was last. No access control, paid access, authentication, or technical restriction was bypassed.

## Timing debt closed

The active run has a generated monotonic timing receipt at `ToS/source-witnesses/discovery/timings/dialogue-between-a-man-and-his-ba-open-work-route.2026-08-30.v1.timing.json`. Every active channel has a positive measurement; zero is not used as an unknown sentinel.

The narrower physical-artifact route is `ToS/source-witnesses/discovery/runs/dialogue-man-ba-papyrus-berlin-p3024-artifact.2026-08-30.v1.json`, with its own receipt at `ToS/source-witnesses/discovery/timings/dialogue-man-ba-papyrus-berlin-p3024-artifact.2026-08-30.v1.json`. Its SMB, TLA, and University of Tokyo channels measured 0.785924, 15.310813, and 20.981907 seconds respectively, all strictly positive; the last bounded probe ended in a recorded transport error without invalidating the previously downloaded and fixed exact Files.

| Channel | Seconds | Outcome |
|---|---:|---|
| K10plus | 1.047534 | HTTP 200 |
| Staatliche Museen zu Berlin | 0.951716 | HTTP 200 |
| Morgan Library & Museum | 0.228464 | HTTP 403 |
| De Gruyter | 0.264447 | HTTP 202 |
| TLA | 20.020955 | transport error after bounded timeout |
| University of Tokyo IIIF | 0.944629 | HTTP 200 |
| Liverpool Repository | 0.906117 | HTTP 200 |
| Europeana | 0.569450 | HTTP 403 |
| Crossref / OpenAlex | 0.389705 | HTTP 200 |
| BHL / Internet Archive | 0.495678 | HTTP 200 |
| Google Books | 0.484383 | HTTP 200 |
| final web check | 0.246866 | HTTP 200 |

These values measure request-through-first-16,384-response-bytes only. They are not human research time, interpretation time, rights-review time, or proof of source quality.

## Acquired exact objects

| Layer | File | Pixels | Bytes | SHA-256 | Rights |
|---|---|---:|---:|---|---|
| Lepsius plate 111, P. Berlin 3024 lines 1–107 | `BG_222_6_12-0045.jpg` | 6132 × 8176 | 4,036,600 | `b68b6ffa4a40015834f98748ed408288546cee47373d802276df2179d5dca2bc` | CC BY 4.0; University of Tokyo Digital Archive Portal |
| Lepsius plate 112, P. Berlin 3024 lines 108–180 | `BG_222_6_12-0046.jpg` | 6132 × 8176 | 3,720,252 | `b7d8ad123d378d1611d20fb3ea7661c5f044dea5807bc10cdf907986b50e0f9b` | CC BY 4.0; University of Tokyo Digital Archive Portal |

The file-to-plate mapping is visible in the scan itself and independently supported by the TLA bibliography, which identifies Lepsius plates 111–112 for P. Berlin 3024. The representations are historical line art of the papyrus; they are not modeled as current physical layout, a full diplomatic transcription, or a modern reconstruction.

## Rights by layer

- The physical artifact and factual catalog metadata remain evidence layers; the packet does not claim copyright in the ancient papyrus.
- Each exact University of Tokyo JPEG is separately governed by CC BY 4.0. Attribution, license-link, and change-notice obligations remain attached to the File.
- The University of Tokyo repository record explicitly carries the CC BY 4.0 license URI, and its institutional terms allow download, copying, modification, and commercial or non-commercial reuse subject to attribution and indication of changes.
- The TLA record is retained as bounded scholarly identity and bibliography evidence. TLA text bytes were not acquired under its limited academic-use terms.
- The Liverpool article PDF is downloadable, but the observed repository permission is purpose-limited to personal research, education, and non-profit use; availability therefore did not become open-license clearance.
- The complete 1896 proceedings scan was not imported. Although a US provider marks the volume public domain, Adolf Erman's 1937 death leaves the reviewed Mexican life-plus-100 term unresolved through 2037, and the scan is a multi-work container.
- Museum prose, Morgan imagery, modern reconstruction, transcription, transliteration, translations, semantic interpretation, and later scholarship remain separate and unlicensed by the two positive JPEG results.

## Operational relation chain

The frozen candidate, queue snapshot, ordered discovery run, per-channel timing receipt, selected and rejected results, acquisition event, artifact, two representation packets, exact Files, per-File rights, source planting, research packet, and terminal receipt are connected through stable IDs and repository refs.

The following are retained only as reviewable assertions, not accepted relations: P. Amherst III as a physical join; the Mallorca fragments as the missing beginning; the TLA aggregate as one physical object; P 3024/C as equivalent to the complete historical papyrus; and any modern title containing “soul” as a literal ancient semantic identity.

## Remaining review

- inspect the current complete SMB mounting and identify all components beyond P 3024/C;
- review the physical and textual joins among Berlin, Amherst, and Mallorca witnesses;
- establish whether the two Lepsius plates cover the historical Berlin roll completely and how their layout relates to current conservation mounts;
- separate the ancient composition's durable Work identity from title translations and reconstructed ordering;
- review the Escolano-Poveda reconstruction and later critical editions without treating repository access as a license;
- obtain human legal, papyrological, philological, and editorial approval before any source-text or publication decision.

The candidate can terminate as `held_source_witness`: two exact CC BY 4.0 source representations are held with fixity, while ancient Work identity, source text, reconstruction, semantics, graph, canon, and publication remain unaccepted.
