# A05 Pyramid Texts Open-Work Discovery and Rights Route

Status: non-authoritative source-linked research packet. It is not source text, legal advice, semantic review, graph authority, canon, or publication approval.

## Frozen target

The reviewed loop candidate was `open-work-candidate.pyramid-texts`. The iteration resolved four layers without collapsing them:

1. **Work-like corpus pressure** — “Pyramid Texts” is a modern corpus name, not a newly minted unitary ancient Work.
2. **Physical witness** — the in-situ Pyramid of Unas is one reported early inscription witness.
3. **Modern scholarly composite** — Kurt Sethe’s 1908 and 1910 volumes are a modern critical reconstruction based on paper impressions and photographs.
4. **Digital Files** — BSB objects `bsb00068075` and `bsb00068079` are two exact PDF representations, each with its own digest and rights record.

## Ordered discovery result

The protocol-native run is `ToS/source-witnesses/discovery/runs/pyramid-texts-open-work-route.2026-08-30.v1.json`; its external timing receipt is `ToS/source-witnesses/discovery/timings/pyramid-texts-open-work-route.2026-08-30.v1.json`.

Ten channels were inspected in order: BnF authority; BSB holding and IIIF; Deutsche Digitale Bibliothek; University of Chicago EOS; UCL Digital Egypt; TLA; Library of Congress; Internet Archive; Open Library/Google Books; general web last. The run preserves exact queries, result order, the empty LOC result, all useful URLs, selected/deferred/rejected decisions, and positive monotonic transport time for every channel.

Key sources:

- [UCL Pyramid Texts overview](https://www.ucl.ac.uk/museums-static/digitalegypt/literature/religious/pyramid.html) — modern corpus label, earliest Unas example, named royal witnesses, transmission notes, and Sethe bibliography; page content remains all rights reserved.
- [TLA PT 217](https://thesaurus-linguae-aegyptiae.de/text/FZQ6IM7ENFHHROXB2W6PTYMXNE?lang=en) and [TLA licenses](https://thesaurus-linguae-aegyptiae.de/info/licenses) — one persistent Unas text identity and bibliography; current terms do not authorize acquisition of the whole subcorpus.
- [BSB volume 1 IIIF](https://api.digitale-sammlungen.de/iiif/presentation/v2/bsb00068075/manifest) and [volume 2 IIIF](https://api.digitale-sammlungen.de/iiif/presentation/v2/bsb00068079/manifest) — exact bibliographic identity, attribution, sequence, and CC BY-NC-SA 4.0 declaration.
- [DDB volume 1](https://www.deutsche-digitale-bibliothek.de/item/Z62NPYXUF26JTYSGIQPYHJWB32VTRBLT) and [volume 2](https://www.deutsche-digitale-bibliothek.de/item/LMFRYBVJA5HYWFPYWM3NDJQCNCRSQA5A) — independent BSB object, call-number, URN, and license corroboration.
- [Chicago EOS](https://www.lib.uchicago.edu/cgi-bin/eos/eos_title.pl?callnum=PJ1553.A1_1908_cop3) — exact edition and section route; no sufficient object-level reuse license found.
- [Internet Archive item](https://archive.org/details/diealtaegyptisch03sethuoft) — later Sethe volumes and PDF derivative; the page’s possible-status display is not mirrored by rights fields in the metadata API, so acquisition was deferred.

## Acquisition and fixity

The BSB download form required explicit agreement with CC BY-NC-SA 4.0. The operator had explicitly prioritized downloading permitted works. No technical-access bypass was used.

| Part | File | Bytes | PDF pages | SHA-256 |
| --- | --- | ---: | ---: | --- |
| Band 1 | `bsb00068075.pdf` | 564,510,348 | 535 | `a67fd170e8cff617d34f5c03759a394f083f312ac25f81e33c5ca96faeccac84` |
| Band 2 | `bsb00068079.pdf` | 608,709,027 | 565 | `6bfa1cefa25c388c5b3ca3daf55265abad809f18804af8e1ab0725be435269ca` |

The typed host preflight returned `deny` because `/srv/AbyssOS` is protected from machine-owned storage automation. Capacity was sufficient. The project-local write proceeded under explicit operator authority and the ToS source-evidence boundary; this does not grant general host-automation authority.

## Rights decision

Both exact BSB digital objects are positively licensed under [CC BY-NC-SA 4.0](https://creativecommons.org/licenses/by-nc-sa/4.0/). The license permits copying, retention, non-commercial redistribution with attribution, and adaptations under ShareAlike. The repository nevertheless keeps these source payloads `local_only` and Git-ignored.

The positive decision covers only the two SHA-256-bound BSB PDFs. It does not license or admit UCL prose, TLA content, Chicago or Internet Archive scans, ancient inscription text, Sethe readings as truth, OCR, transcription, translation, or any derivative.

## Operational relations created

- A05 backlog row 11 → Sethe composite planting.
- Sethe composite → reported member artifact Pyramid of Unas.
- Composite → two provider-independent File representations.
- Each representation → its exact BSB/DDB provider records, payload digest, rights record, discovery run, and acquisition event.

These are source/provenance relations. No semantic relation, graph fact, or canon promotion was made.
