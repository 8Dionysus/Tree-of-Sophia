# Genealogie 1892 Edition and Provision Identity Research

Status: ordered, source-returnable research and bounded model-authored
materialization; not accepted bibliography, German text, rights clearance,
printing or release chronology, human review, semantics, or canon

Research snapshot: 2026-08-01

Discovery receipt:
`ToS/source-witnesses/discovery/runs/genealogie-1892-provision-identity.2026-08-01.v1.json`

## Question

What may Tree of Sophia assert from the exact 1892 second-edition Item when
the title page names the publisher and year, while a separate terminal page
names the printer without a date?

## Result

The fixity-verified local Wikimedia Commons/UNC scan supplies two distinct
source surfaces:

- PDF page 5 prints `LEIPZIG / Verlag von C. G. Naumann. / 1892.`;
- PDF page 204 prints `LEIPZIG / Druck von C. G. Naumann.`.

These statements support two separate Edition-owned provision claims:

- publication -> `tos.organization.c-g-naumann-verlag-leipzig`;
- manufacture -> `tos.organization.druckerei-c-g-naumann-leipzig`.

The publication claim carries the transcribed 1892 statement date. The
manufacture claim also carries 1892 only as the source-visible Edition year
from the title page; the printer line itself is undated. It is therefore not
an exact printing-completion date, a public-release date, or proof that every
copy was manufactured in one dated event.

## I. Classical standards and official documentation

The current
[`BIBLIOGRAPHIC_TIME_PLACE_RESPONSIBILITY_RESEARCH.md`](BIBLIOGRAPHIC_TIME_PLACE_RESPONSIBILITY_RESEARCH.md)
profile remains the controlling standards layer. IFLA LRM/LRMoo preserves
Edition-like manifestation identity separately from Item production;
BIBFRAME ProvisionActivity separates Publication from Manufacture while
grouping place, agent, and date; PROV-O keeps the 2026 annotation event apart
from the historical activity described by the claim.

That distinction matters more here than in the 1886 and 1893 Naumann cases.
The publisher and printer do not appear in one combined formula. They occur
on different pages and must remain independently returnable.

### Exact local Item and current provider records

The exact source is:

- Item:
  `tos.item.friedrich-nietzsche.zur-genealogie-der-moral.de-naumann-1892-second.wikimedia-commons-unc-scan-pdf`;
- PDF SHA-256:
  `5705dbc4f32faa924919fd533962c931e92462d72dab5183610eb68adeecac03`;
- publisher/year source: PDF page 5;
- printer source: PDF page 204.

The local PDF was re-hashed on 2026-08-01 and still matched the tracked
manifest. Pages 5 and 204 were rendered from those exact bytes and inspected.
No rendered image, OCR, transcription corpus, or payload is added to Git.

The refreshed Wikimedia Commons API response still reported page ID
`95720779`, the 2020-11-01 revision, 13,227,300 bytes, SHA-1
`babdcddf6d519d4084f6705333e467fd1a63ba25`, `Public domain`, and
`Copyrighted=False`. Its response SHA-256 was
`cd5274f2dc9e9a0d6c0d754ac675d9449999fed03ac731a513101e07b87cd861`.

The refreshed Internet Archive metadata still reported the 1892 Leipzig
C. G. Naumann object from the University of North Carolina at Chapel Hill.
Its current PDF remains a different byte revision: 13,256,897 bytes and
SHA-1 `2d2a6bd18995cd52d1ca79e2e8ff4af10735a075`. The response SHA-256 was
`19d0491ef71a41d7c80cae94e310f8a540e443d2381dbbb18d8e5b616bf58473`.
The IA record is lineage and institutional provenance, not byte equivalence
with the exact Commons Item.

### Current identities and an independent exact copy

The same-day GND evidence already captured for the Naumann foundation slices
remains the normalization layer:

- [C. G. Naumann Verlag, GND 1072998033](https://d-nb.info/gnd/1072998033)
  is the publisher authority;
- [Druckerei C. G. Naumann, GND 16034133-4](https://d-nb.info/gnd/16034133-4)
  is the distinct printer authority;
- [Leipzig, GND 4035206-7](https://d-nb.info/gnd/4035206-7) is the Place
  authority;
- [Zur Genealogie der Moral, GND 4342502-1](https://d-nb.info/gnd/4342502-1)
  is the Work authority.

All four linked-data retries failed to connect on this pass. No access control
was bypassed, no failed response was promoted, and the role-specific local
Organizations remain provisional rather than `same_as` assertions.

The Nietzsche Documentation Center Naumburg's collection bibliography,
current to 2026-03-27, independently records another exact 1892 copy as
*Zweite Auflage*, Leipzig, C. G. Naumann, XIV + 182 pages, shelfmark
`1892SW001`. This corroborates the Edition identity and title-page imprint,
not the UNC Item or its terminal printer page.

## II. Established bibliography and textual history

Andreas Urs Sommer's 2019 historical-critical commentary is the established
control. It distinguishes the 1887 first print, the 1892 second edition, and
the 1894 third edition; it also records the later inserted series half-title
for the 1892 copy. That prevents `Zweite Auflage` from being flattened into a
Work-wide ordinal or the local Item from being equated with the 1887 first
print, a correction copy, or the critical text.

William A. B. Parkhurst's 2022 genetic study of the preface separates
preparatory stages, print manuscript, correction sheets, first edition, and
later development. The established evidence constrains textual-stage and
chronology interpretation. It does not replace the two source-visible
provision statements.

The fuller current route through D 20a, D 20b, partial K 11/C 4616, E 40,
later-copy uncertainty, this 1892 Item, and critical addresses remains in
[`GENEALOGIE_AUTHORIAL_WITNESS_ROUTE.md`](GENEALOGIE_AUTHORIAL_WITNESS_ROUTE.md).

## III. Fresh current work, then general web last

The University of Basel documentary edition, freely viewable since December
2025, is the freshest directly relevant philological result found. It gives
exact source-surface routes for the print manuscript, correction sheets, and
1887 first print. This sharpens stage separation but does not alter the 1892
title-page or printer-line transcription.

A current 2026 Hackett publisher record describes a new English translation
edition. It is relevant to a future translation-comparison branch, but it is
not an identity, provision, or textual authority for this German 1892
Edition. No 2026 philological correction to the two inspected imprint
statements was found.

General web search ran last. Marketplace listings repeated `Zweite Auflage`,
Leipzig, Naumann, and 1892 but supplied no source fact stronger than the exact
Item, the independent institutional copy, or the established commentary.
They were rejected as authority.

## Admission decision

Admit:

1. one proposed page-region anchor for the publisher/year imprint on PDF
   page 5;
2. one proposed page-region anchor for the printer line on PDF page 204;
3. one `publication` provision claim using the existing provisional publisher
   Organization;
4. one `manufacture` provision claim using the existing provisional printer
   Organization;
5. the source-visible 1892 Edition year with an explicit warning that the
   printer line is undated.

Do not admit:

- an exact printing-completion or public-release date;
- one generic `C. G. Naumann` identity for both roles;
- a `same_as`, legal-unity, split, or successor relation between the GND
  Corporate Bodies;
- equivalence to the 1887 first print, correction sheets, an author-final
  state, or a critical text;
- accepted German, OCR, translation fidelity, semantics, signs, concepts,
  graph truth, canon, or publication permission;
- source payload, rendered pages, OCR, or tracked source text.

All new anchors and claims are model-made and unreviewed. The graph may expose
the two role-specific normalized identities only through their claim nodes.
