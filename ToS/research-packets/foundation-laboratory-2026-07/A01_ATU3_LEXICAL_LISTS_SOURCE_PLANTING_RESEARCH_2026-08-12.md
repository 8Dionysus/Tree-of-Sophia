# A01: ATU 3 Lexical-Lists Source Planting Research

Date: 2026-08-12
Authoring posture: model-authored, review-ready, no human or Assyriological
judgment claimed
Scope: bibliographic identity, responsibility, list/index coordinates,
provenance, rights, access route, and future laboratory method; no source text
copied into Git

## Result

The A01 backlog label `ATU 3 — Die Lexikalischen Listen der archaischen
Texte aus Uruk [24]` resolves to a standalone scholarly Work rather than an
aggregate volume like ATU 2. Current institutional records identify Robert K.
Englund and Hans J. Nissen as authors and Peter Damerow as collaborator. CDLI
flattens all three into its author field; ToS retains the more specific
responsibility statement reported by the publisher, Freie Universität Berlin,
Heidelberg, and CiNii.

The 1993 publication is *Archaische Texte aus Uruk* 3 and *Ausgrabungen der
Deutschen Forschungsgemeinschaft in Uruk-Warka* 13. The current publisher
marks it out of print. Heidelberg holds an exact loanable copy and an
Assyriology reading-room copy, but no lawful complete digital Item was found.
The first planting therefore creates one Work identity and three unreviewed
responsibility claims, not a complete Expression/Edition/Item ladder.

PCSL's current `LLATU` abbreviation identifies pages 177-327 as ATU 3's sign
index. Its coordinate is a page number plus the index of a sign on that page,
for example `177_01`. It is not a stable ToS sign ID. No LLATU row, list line,
sign form, reading, gloss, translation, etymology, ontology, or graph relation
is imported by this planting.

## 1. Official and classical documentation

### Exact Work and publication identity

The current [CDLI publication record](https://cdli.earth/publications/1785922)
identifies the book, BibTeX key `Englund1993-V33RHF3H`, designation `ADFU 13`,
Berlin, Gebr. Mann, 1993. CDLI reports Englund, Nissen, and Damerow in one
author list.

Freie Universität Berlin's official
[ATU project page](https://www.geschkult.fu-berlin.de/en/e/vaa/forschung/projekte_abgeschlossen/archaische_texte/index.html)
gives the more articulated statement: Robert K. Englund and Hans J. Nissen,
`unter Mitarbeit von Peter Damerow`. ToS normalizes this to a
`contributed_by` claim while preserving the exact source wording as a claim
qualifier; it does not silently promote Damerow to co-author or infer the
scope of his contribution. The project page also places ATU 3 in the project
that worked to make more than 5,000 late-fourth-millennium tablets and fragments
available to scholarship. This establishes project lineage, not completeness
of any current digital representation.

The current [Gebr. Mann publisher page](https://www.reimer-mann-verlag.de/controller.php?cmd=detail&titelnummer=301687&verlag=3)
reports ATU Band 3 / ADFU Band 13, ISBN `978-3-7861-1687-5`, 1993, 330 pages,
22 figures, 1,712 signs, 99 plates with 808 illustrations, and ten additional
plates with 32 illustrations. It labels the title `Vergriffen`. A generic
manual-order banner on the same page is not treated as proof that stock exists
or that a source copy may be redistributed.

Heidelberg's exact
[HEIDI record](https://katalog.ub.uni-heidelberg.de/titel/3639871) reports the
responsibility statement, `327, 99, X S.`, German and Sumerian language
content, ISBNs, K10plus PPN `1104598086`, loanable shelfmark `93 B 726::13`,
and an Assyriology reading-room copy. The difference between the publisher's
marketing extent and the catalog's segmented extent remains a description
difference, not a page-count correction made by ToS.

[WorldCat](https://search.worldcat.org/de/title/lexikalischen-listen-der-archaischen-texte-aus-uruk/oclc/489728713)
supplies OCLC `489728713`; [CiNii](https://cir.nii.ac.jp/crid/1971430859736190362)
corroborates the responsibility statement, series, place, year, and ISBN. These
records identify a publication. None supplies source bytes or a content-use
license.

### LLATU is an edition coordinate, not truth

The current PCSL
[abbreviation register](https://oracc.museum.upenn.edu/pcsl/signlist/Abbreviations/index.html)
defines `LLATU` as pages 177-327 of ATU 3. It describes the book as the
definitive edition of lexical texts from Uruk and says its sign index often
distinguishes forms beyond ZATU by lowercase letters. The entries are listed
alphabetically with references rather than numbered as a separate sign list.

The current [LLATU concordance](https://oracc.museum.upenn.edu/pcsl/signlist/LLATUConcordance/index.html)
states that an LLATU number is the page number in ATU 3 plus the index of the
sign on the page. Its mapping to a PCSL name is a current concordance surface.
It does not prove that an LLATU label, ZATU number, PCSL OID, visual form,
reading, or meaning is invariant.

The identity rule for later laboratory work is therefore:

```text
ATU 3 Work / 1993 publication identity
  != LLATU page-entry coordinate
  != ZATU sign identity
  != PCSL OID or current PCSL name
  != proposed or accepted Unicode character
  != one reading, gloss, lexeme, concept, or ontology
```

Every crosswalk must be a dated, evidence-bearing assertion over the exact
source state it compared.

## 2. Established top scholarship

### Direct reviews remain correction surfaces

Piotr Steinkeller's review in *Archiv für Orientforschung* 42/43 (1995/1996),
pages 211-214, and Niek Veldhuis's 1995 review in *Bibliotheca Orientalis* 52,
pages 433-440, are retained as direct review routes. Their existence prevents
the 1993 organization from being treated as an unreviewed timeless ontology.
No review text is copied into the source spine, and no reported correction is
materialized until it can return to an exact source-visible claim.

### Veldhuis 2014: tradition, use, and transmission

Niek Veldhuis's peer-reviewed monograph
[*History of the Cuneiform Lexical Tradition*](https://researchportal.helsinki.fi/en/publications/history-of-the-cuneiform-lexical-tradition/)
is the established long-range control. It treats lexical texts as lists of
words and signs developed to describe, transmit, and investigate knowledge of
writing across a roughly three-thousand-year tradition. That historical and
institutional frame is directly relevant to ToS: recurrence and ordering may
be observed, but their function and semantic interpretation must remain
versioned rather than inherited from the modern label “dictionary”.

### Gabriel 2020: the proto-cuneiform frontier

Gösta Gabriel's
[*Die archaischen Listen aus Uruk und die proto-keilschriftliche frontier*](https://www.degruyterbrill.com/document/doi/10.1515/janeh-2020-0002/html)
starts from the unresolved function of the archaic lists. It proposes that the
corpus explored the frontier of what a growing administrative notation system
could express, with potential beyond bureaucracy. This is a useful hypothesis
for ToS, not a fact to encode as one universal purpose. It argues against
equating a later category label with a settled ancient semantic system.

### DCCLT/ORACC as a modern documentary layer

The Digital Corpus of Cuneiform Lexical Texts remains the current operational
route for scholarly composites and exemplar comparison. Existing ToS witness
`tos.composite.proto-cuneiform.archaic-vessels-and-garments` already proves
that a DCCLT composition, its physical members, editorial lines, provider
coverage, and ATU 3 lineage are different layers. ATU 3 is the classical
publication lineage; a current DCCLT Q-page is not an electronic facsimile of
the book and cannot replace it silently.

## 3. Fresh current relevance check

### Lecompte and Naccaro 2025: colophons and social setting

Camille Lecompte and Hugo Naccaro's open-access 2025 article
[*The Colophons of the Archaic Lists and their Social Setting*](https://www.degruyterbrill.com/document/doi/10.1515/za-2025-0001/html)
re-edits and analyzes archaic-list colophons, their archaeological clusters,
onomasticon, vocabulary, and possible institutional setting. It reports that
the lack of an unambiguous term for “scribe” and the presence of several
professional designations complicate simple models of scribal organization.
The authors advance an educational-tool explanation, but ToS retains it as a
current scholarly claim rather than settled ancient purpose. The article is
CC BY 4.0; its license does not change the rights of ATU 3 plates or pages.

### Scribal Worlds 2026: classification requires cultural context

UCL Press published the open-access volume
[*Scribal Worlds: Scholarship and classification in cuneiform cultures*](https://uclpress.co.uk/book/scribal-worlds/)
on 29 June 2026 under CC BY-NC-ND 4.0. Its explicit scope joins language,
ontology, classification, scribal learning, material culture, and philosophy
of science, and asks how fragmentary texts, objects, and practices can be
interpreted in cultural context. Chapters titled “The Order-of-the-World” and
“Is there a Mesopotamian ontology?” make the question itself current; they do
not license a shortcut from list order to an ancient ontology graph.

For ToS the fresh control is methodological:

```text
observed list order or recurrence
  -> source-bound observation
  -> competing functional/classificatory interpretations
  -> explicit evidence and review
  != automatic ontology or canonical concept hierarchy
```

### Current PCSL and Unicode state

The live PCSL LLATU concordance is useful because it exposes splits, merges,
renamings, and working OIDs instead of hiding them. Unicode proposal
[`L2/25-211`](https://www.unicode.org/L2/L2025/25211-proto-cuneiform.pdf)
uses ATU 3/LLATU as a major source, but the official
[Unicode pipeline](https://www.unicode.org/alloc/Pipeline.html), last updated
4 August 2026 when checked, still marks the Proto-Cuneiform block as
provisional. UTR #56 continues to describe working-data risks. Proposed code
points are current technical coordinates, not permanent ToS identity or
semantic truth.

## Rights and access boundary

| Layer | Current access | ToS conclusion |
| --- | --- | --- |
| CDLI, FU Berlin, publisher, HEIDI, WorldCat, CiNii metadata | public web metadata | public-safe citation and identity evidence; no book-content grant |
| Complete ATU 3 volume | no lawful complete digital Item located | do not acquire from an unverified mirror or publish |
| Heidelberg physical copies | one loanable and one reading-room holding | access candidate only; not a ToS Item and not a rights grant |
| Heidelberg reproduction service | official order route; copyright material may be refused or copied only in part | prepare a request; no order, payment, or permission is inferred |
| PCSL LLATU pages | public working web concordance | inspect as a dated coordinate layer; do not import it as source truth or assume reuse terms |
| Lecompte-Naccaro 2025 | CC BY 4.0 article | reusable with attribution at its own layer; no rights transfer to source tablets or ATU 3 |
| Scribal Worlds 2026 | CC BY-NC-ND 4.0 | cite and study; do not create a modified public derivative from its content |

A public-safe draft asks Heidelberg first for front matter, contents, and
terms, then for lawful local research access to the complete volume or a
bounded representative packet. It asks separately about local OCR,
transcription, indexing, embeddings, and quotation. It asks for no source
redistribution, public derivative, or server processing. No form is submitted.

## A/B/C laboratory route

| Condition | Material and method | State | Quality question | Cost/speed observation |
| --- | --- | --- | --- | --- |
| A | exact institutional metadata, reviews, DCCLT/PCSL documentation, and current standards | executed, metadata only | Can Work identity, responsibility, LLATU coordinate semantics, access, and false equivalences be resolved without source bytes? | seconds per source; negligible retained storage; cannot assess page or list fidelity |
| B | lawful local, fixity-bound ATU 3 Item; native extraction plus page rendering and OCR/layout challengers | prepared, not acquired | Can list boundaries, columns, entry order, signs, references, plates, and page-index coordinates be recovered with exact source return? | reproduction cost and terms unknown; OCR/layout variants must be timed and manually sampled |
| C | B plus DCCLT composites, LLATU-to-PCSL concordance, ZATU controls, and current Unicode data | prepared, not run | Which coordinates remain one-to-one, split, merge, rename, or conflict, and which semantic inferences fail under cross-source comparison? | highest review cost; machine joins are cheap only after identity and source-visible anchors are correct |

Condition A proves metadata and method only. B and C require exact source
digests, versioned transformations, representative manual checks, recorded
error classes, elapsed time, storage, energy where measurable, and explicit
cost. A green validator cannot prove a cuneiform form, list membership,
reading, translation, historical function, or ontology.

## Rejected shortcuts

- Do not model the standalone monograph as a one-member Collection.
- Do not treat metadata as a complete Expression, Edition, Item, or text.
- Do not infer a lawful copy or redistribution right from an out-of-print page,
  physical holding, generic order banner, search result, or public concordance.
- Do not call LLATU a separate ancient sign list or use its page-entry value as
  a permanent ToS sign ID.
- Do not replace LLATU with PCSL OIDs or provisional Unicode values.
- Do not infer one reading, gloss, lexeme, or concept from a sign label.
- Do not turn modern list names or order into an ancient ontology graph.
- Do not ask a human to transcribe or review material before a source-visible
  disagreement or promotion decision exists.

## Remaining gates

- Complete ATU 3 source Item: absent.
- Exact source-visible front matter and contents: absent.
- Expression and Edition identity rungs: intentionally open.
- Rights-holder and full local-processing scope: unresolved.
- List, line, sign-index, plate, and page-region anchors: absent.
- LLATU-to-ZATU/PCSL/Unicode concordance claims: not materialized.
- Readings, glosses, translations, etymologies, concepts, and historical
  function claims: absent.
- Assyriological and human source-visible review: not performed or scheduled.
- Graph and canon: unchanged.

The durable result is a direct bibliographic Work planting that fixes what ATU
3 is, how its responsibility statement differs across records, what LLATU
coordinates mean, how current scholarship limits semantic overreach, and what
lawful source access is still required before laboratory work can begin.
