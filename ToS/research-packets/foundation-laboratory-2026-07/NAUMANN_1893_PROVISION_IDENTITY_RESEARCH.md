# Naumann 1893 Edition and Provision Identity Research

Status: ordered, source-returnable research and bounded model-authored
materialization; not accepted bibliography, German text, rights clearance,
release chronology, human review, semantics, or canon

Research snapshot: 2026-08-01

Discovery receipt:
`ToS/source-witnesses/discovery/runs/naumann-1893-provision-identity.2026-08-01.v1.json`

## Question

What may Tree of Sophia assert from the exact 1893 Naumann title page without
flattening an Edition statement, publication, manufacture, publisher, printer,
place, statement year, release, textual state, and current rights into one
label-derived fact?

## Result

The fixity-verified local image PDF supplies one exact source surface for three
different evidence roles:

- PDF page 46 prints the Edition statement
  `Zweite Auflage, mit Portrait und Brieffacsimile des Autors.`;
- the same page prints
  `LEIPZIG / Druck und Verlag von C. G. Naumann. / 1893.`;
- PDF page 47 prints `Uebersetzungsrecht vorbehalten.`

The first string is an Edition statement. The second visibly assigns both
printing and publishing functions and therefore opens two separate
Edition-owned provision claims over the same literal source statement. The
third is a historical rights statement and remains evidence inside the
unreviewed Item rights record; it is neither a current legal prohibition nor a
current permission.

The two provision claims normalize their roles separately:

- publication -> `tos.organization.c-g-naumann-verlag-leipzig`;
- manufacture -> `tos.organization.druckerei-c-g-naumann-leipzig`.

The current GND records distinguish the publisher and printer Corporate
Bodies. ToS preserves that authority split without asserting that the two
records describe legally unrelated companies, the same legal entity, a
corporate succession, or every historical use of the bare name `C. G.
Naumann`.

## I. Classical standards and official documentation

The already adopted
[`BIBLIOGRAPHIC_TIME_PLACE_RESPONSIBILITY_RESEARCH.md`](BIBLIOGRAPHIC_TIME_PLACE_RESPONSIBILITY_RESEARCH.md)
profile applies the current IFLA LRM/LRMoo, BIBFRAME ProvisionActivity,
PROV-O, GND, and source-return disciplines. It requires an Edition-owned n-ary
claim rather than flat `publisher`, `printer`, `place`, and `date` fields.

That profile matters especially here because the source joins two functions in
one line. BIBFRAME distinguishes Publication and Manufacture even when one
statement supplies both. LRMoo distinguishes Manifestation creation from Item
production. PROV-O keeps the 2026 annotation event separate from the 1893
activity described. GND identity remains a normalized assertion beside, not in
place of, the exact title-page wording.

### Exact source surface

The local visual witness is:

- Item:
  `tos.item.friedrich-nietzsche.also-sprach-zarathustra.de-naumann-1893.internet-archive-image-container-pdf`;
- file SHA-256:
  `61c947e5aff76a64d82600cc52dcb25ff1b5862530d3a99c96824da885c1e6cf`;
- source page: PDF page 46 for the Edition and provision statements;
- rights page: PDF page 47 for the historical translation-right reservation.

The exact local payload was re-hashed on 2026-08-01 and still matched its
tracked manifest. Pages 44-47 were rendered from those bytes and inspected.
No rendered image or payload is added to Git.

The current Internet Archive metadata API response retained the same exact
parent identifier, ARK, PDF and EPUB filenames, sizes, MD5 values, and SHA-1
values recorded during intake. The 2026-08-01 response was 9,020 bytes with
SHA-256
`d18e7d125ec6a7ae74029eafaa7b529df56b5e34c9bb10524cd2e08be7b7d05c`.
This is current parent-object corroboration, not title-page transcription.

### Current authority records

The exact DNB GND linked-data records were fetched from their official HTTPS
endpoints on 2026-08-01:

- [C. G. Naumann Verlag (Leipzig), GND 1072998033](https://d-nb.info/gnd/1072998033)
  is a Company established 26 January 1871, located in Leipzig, with variants
  including `Verlag von C. G. Naumann`; response SHA-256
  `bd80376b3617c2ba761976097b844bdb0877c06e288982c6d89110bac6fcb972`;
- [Druckerei C. G. Naumann, GND 16034133-4](https://d-nb.info/gnd/16034133-4)
  is a distinct Company record located in Leipzig; response SHA-256
  `212820db62cf1db8a4517328c8a48cb2377ed008ef901c7dbd232572eb3c255c`;
- [Leipzig, GND 4035206-7](https://d-nb.info/gnd/4035206-7) remains the
  existing Place authority; response SHA-256
  `e4007667fe7377b9e87b7c64af8d51636c1fb80269fd80b875f747169a4042d6`.

Both Company records name the same founder authority and Leipzig place. That
commonality does not license `same_as`, merger, successor, or legal-entity
claims. It does support role-specific normalized references while the local
Organization identities remain provisional and the claims remain unreviewed.

## II. Established bibliography and publication history

David Marc Hoffmann's 1991 source-based study
[*Zur Geschichte des Nietzsche-Archivs*](https://doi.org/10.1515/9783110870206)
records that the autumn 1892/1893 publication brought all four parts together
for the first time with Heinrich Köselitz's introduction, was labelled
`zweite Auflage, 1893`, and was followed by a `dritte, unveränderte Auflage
1894` already in August 1893. The inspected publisher preview had SHA-256
`7a955f86d3d19e9eaa85018b7814ad1e3e6c383e745613854866710cae1e3de7`.

This evidence is important because it prevents a simple ordinal collapse.
`Zweite Auflage` is exact manifestation wording, but it is not automatically
“the second state of every part”, “the second Work”, or one universal
bibliographic sequence independent of the four-part publication history.

The current
[DLA Marbach Work record](https://www.dla-marbach.de/find/opac/id/AK00003291/)
keeps the 1883 parts I-III, the 1885 private part IV, and the 1891 first
publisher issue of part IV distinct. Its 2026-08-01 response had SHA-256
`bede361b80d70ebac11f8487d622851f9536ca3efcf5acc2a0c699a376fd29b7`.
That Work-level history corroborates the staged route but does not replace the
exact 1893 title page or establish this Item's release day.

## III. Fresh current reconciliation and general web last

After the exact Item, official authorities, and established bibliography, a
fresh search checked current library, scholarly, and general-web results for a
2025-2026 correction to the 1893 imprint or Edition statement. No stronger
exact record or current contradiction was found. Marketplace descriptions
were rejected as authority; they add no source fact stronger than the local
title page and no identity evidence stronger than GND.

The same pass refreshed the separate Nietzsche Source source-admission block.
Its documentation is currently indexed under the official
`doc.nietzschesource.org` surface, but direct HTTPS to the documentation and
the exact eKGWB passage still failed from this host while the exact content
route remained available only over HTTP. This does not affect the 1893
provision claims and does not admit German text.

## Admission decision

Admit:

1. one proposed page-region anchor for the exact Edition statement on PDF
   page 46;
2. one proposed page-region anchor for the exact imprint on PDF page 46;
3. one proposed page-region anchor for the historical rights statement on PDF
   page 47;
4. the exact Edition statement in the existing Edition record;
5. one `publication` provision claim using the publisher Organization;
6. one `manufacture` provision claim using the printer Organization;
7. one new provisional printer Organization and refreshed evidence on the
   existing provisional publisher Organization;
8. the historical rights statement as additional evidence while retaining
   `copyright_undetermined`, `local_only`, and `not_authorized`.

Do not admit:

- an exact printing-completion or public-release date;
- one generic `C. G. Naumann` identity for both provision roles;
- a `same_as`, legal-unity, split, or successor relation between the two GND
  Corporate Bodies;
- a universal Work-level edition ordinal derived from `Zweite Auflage`;
- accepted German, critical equivalence, translation fidelity, semantics,
  signs, concepts, graph truth, canon, or publication permission;
- source payload, page images, OCR, or tracked source text.

All new anchors and claims are model-made and unreviewed. The graph may expose
the two role-specific normalized identities only through their claim nodes.
