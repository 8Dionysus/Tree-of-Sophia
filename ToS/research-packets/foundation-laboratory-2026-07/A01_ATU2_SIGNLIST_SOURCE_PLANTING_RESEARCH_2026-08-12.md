# A01: ATU 2 Sign-List Source Planting Research

Date: 2026-08-12
Authoring posture: model-authored, review-ready, no human or Assyriological
judgment claimed
Scope: bibliographic identity, contained-work boundaries, provenance, rights,
access route, and future comparison method; no source text copied into Git

## Result

The A01 backlog label `ATU 2 — Zeichenliste der archaischen Texte aus Uruk
[24]` does not resolve cleanly to one monolithic sign-list Work. It resolves to
a 377-page aggregate scholarly volume published in Berlin by Gebr. Mann in
1987 as *Archaische Texte aus Uruk*, Band 2. Current authoritative catalogs
report at least two distinct contained contributions relevant to ToS:

- Margaret W. Green's Sign List, pages 167-345;
- Peter Damerow and Robert K. Englund, *Die Zahlzeichensysteme der
  archaischen Texte aus Uruk*, pages 117-166.

The branch anchor asks for the basic sign list, so the planted witness is the
first contained Work, held inside an explicit ATU 2 Collection. The numerical
systems chapter receives a separate Work identity because its authorized 1985
preprint and its 1987 chapter are independently identifiable. The volume is
not forced into the scholarly-composite spine: it is a printed aggregate
publication already covered by the bibliographic Collection route.

No lawful complete digital copy of Green's pages or the full volume was found
and admitted. The planting is public metadata only. It creates no sign entry,
reading, gloss, allograph relation, translation, semantics, graph fact, or
canon state.

## 1. Official and classical documentation

### Exact volume identity

The current [CDLI publication record](https://cdli.earth/publications/1638433)
identifies a book with BibTeX key `Green1987ATU2`, designation `ATU 2`, authors
Margaret W. Green and Hans J. Nissen, series *Archaische Texte aus Uruk*, Band
2, Berlin, Gebr. Mann, 1987.

The University of Tübingen's exact
[KeiBi record](https://ocb.uni-tuebingen.de/Record/KEI00054231) supplies the
critical component boundary that the flat book citation does not expose. It
reports Green's Sign List at pages 167-345 and Damerow-Englund's numerical-
systems contribution at pages 117-166. It also preserves a recognized review
trail rather than treating the 1987 organization as timeless or final.

[LIBRIS](https://libris.kb.se/bib/5961231) and
[Google Books](https://books.google.com/books/about/Archaische_Texte_aus_Uruk.html?id=Rn-I0QEACAAJ)
corroborate 377 pages and ISBN `3-7861-1439-0` / `978-3-7861-1439-0`.
[WorldCat](https://search.worldcat.org/title/Zeichenliste-der-Archaischen-Texte-aus-Uruk/oclc/41328127)
provides the corresponding union-catalog route. These records establish the
Edition identity; they do not establish a reusable digital Item.

### The accessible 56-page file is not ATU 2

CDLI's author bibliography labels
[`englund1987a.pdf`](https://cdli.earth/files-up/publications/englund1987a.pdf)
as the Damerow-Englund contribution at pages 117-166. Its 56 PDF surfaces are
consistent with a chapter plus surrounding matter, not a 377-page book. It
must never be indexed or described as the full ATU 2 volume or as Green's
pages 167-345.

The Max Planck Society exposes an official
[1985 preprint](https://pure.mpg.de/rest/items/item_3377273_2/component/file_3377274/content)
of *Die Zahlzeichensysteme der archaischen Texte aus Uruk*. The
[KeiBi preprint record](https://vergil.uni-tuebingen.de/keibi/Record/KEI00058579?lng=de)
reports an authorized preprint with xii + 45 pages, plates 54-60, and two
tables. The preprint describes itself as an introductory technical chapter for
Green's sign list. Its independent access is valuable, but it does not turn
the preprint and published chapter into proven byte-identical Expressions or
grant redistribution rights.

### False full-text route

The American Academy in Rome catalog exposes a `Full Text Online` link for the
book record, but the destination is Heidelberg's PDF of Manfred Krebernik's
1994 review, not the book. This route is retained as a negative control:

```text
catalog label "Full Text Online"
  != resolved object identity
  != complete book
  != reusable source payload
```

ToS discovery must resolve the final object, title, extent, and rights before
admission. A convenient link label is never sufficient.

## 2. Established top scholarship

### Krebernik's 1994 review

Manfred Krebernik's
[review](https://archiv.ub.uni-heidelberg.de/propylaeumdok/1234/1/Krebernik_REZ_Green_Nissen_Zeichenliste_1994.pdf)
in *Orientalistische Literaturzeitung* 89 (1994), pages 380-385, is the most
direct source found for the sign list's established strengths and limits. It
reports 783 ordinary and 60 number signs, alphabetical organization by later
readings or descriptive names, and separation of Uruk IV and Uruk III forms.
It also identifies limits material to ToS:

- boundaries between sign groups and compound signs can remain fluid;
- notation can collapse distinctions such as `A+B` and `A×B`;
- names and citations require reconciliation with related lists;
- some identifications or form groupings are questionable;
- exemplar forms are not always tied to exact attestations.

The review treats ATU 2 as indispensable progress while making clear that a
sign-list number is not self-proving identity or interpretation. ToS must
therefore preserve the historical ZATU coordinate and each later concordance
claim separately.

### Born and Kelley 2021

Born and Kelley's
[A Quantitative Analysis of Proto-Cuneiform Sign Use in Archaic Tribute](https://cdli.earth/articles/cdlb/2021-6)
is the current established corpus-scale control selected for this planting.
It updates an earlier frequency snapshot from the CDLI corpus known in 2020,
inherits sign-name conventions from Green-Nissen while allowing reassessment,
and explicitly avoids simple sign translations because one sign can serve
multiple functions. Composite lexical texts are excluded from frequency
counts when their editorial assembly would distort the observed corpus.

The article's text is CC BY 4.0 except for separately governed artifact
images. Its frequencies and downloadable data are useful future challengers,
not replacements for the 1987 Work or universal sign meanings.

## 3. Fresh current relevance check

The freshest relevant movement is not another retrospective article. It is
the active technical standardization of Proto-Cuneiform sign identities.

Unicode proposal
[`L2/25-211`](https://www.unicode.org/L2/L2025/25211-proto-cuneiform.pdf),
dated 7 October 2025, cites ATU 2 while documenting later discontinuities in
sign identity. It deliberately uses opaque sign names where later readings
would hide fourth-millennium distinctions and separates forms proposed as
distinct encoded characters.

The official [UTC #185 minutes](https://www.unicode.org/L2/L2025/25226.htm)
record provisional assignment of 1,392 characters to a new Proto-Cuneiform
block. The current [Unicode Pipeline](https://www.unicode.org/alloc/Pipeline.html),
last updated 4 August 2026, still marks `U+12690..U+12BFF` as provisionally
assigned, not stable encoded identity. This must not be confused with the
separate Archaic Cuneiform Numerals block accepted for Unicode 18.0.

The current Proto-Cuneiform Sign List pages expose useful working coordinates:
for example [BAN~b](https://oracc.museum.upenn.edu/pcsl/signlist/l0066/o0980090/index.html)
has OID `o0980090`, ZATU 048, corpus instances, and a proposed Unicode value;
[ZATU693](https://oracc.museum.upenn.edu/pcsl/signlist/l0090/o0981626/index.html)
has OID `o0981626`. Unicode
[UTR #56](https://www.unicode.org/standard/reports/tr56/) characterizes the
associated Oracc sign-list data as working material in which misreadings,
duplicates, unencodable signs, and later deprecations remain possible.

The durable identity rule is therefore:

```text
historical ATU 2 / ZATU coordinate
  != current PCSL OID
  != proposed Unicode code point
  != accepted Unicode scalar value
  != one fixed reading or meaning
```

Concordances among these coordinates must be evidence-bearing, dated, and
reviewable. A proposed code point cannot serve as ToS's permanent sign ID.

## Rights and access boundary

| Layer | Current access | Redistribution conclusion |
| --- | --- | --- |
| CDLI, KeiBi, LIBRIS, WorldCat metadata | public web metadata | public-safe citation and identity evidence; no book-content grant |
| Full ATU 2 / Green pp. 167-345 | no lawful complete payload located | rights unknown; do not acquire from an unverified mirror or publish |
| CDLI 56-page Damerow-Englund PDF | openly reachable partial chapter | accessibility alone does not prove redistribution permission |
| Max Planck 1985 preprint | official open-view PDF | research access established; redistribution license not established |
| Krebernik review PDF | public repository access | Heidelberg's non-CC repository terms do not authorize further public redistribution |
| Born-Kelley article text | CC BY 4.0, images excepted | reusable later with attribution and layer-specific image handling |
| Unicode and PCSL pages | public technical documentation | cite and inspect; do not republish datasets beyond their exact terms |

No source-bearing file enters Git. No private/local Item is created because no
exact payload was admitted. A public-safe, unsent access-request card asks the
University Library Tübingen for a lawful research-access route or copy terms,
first for exact contents/boundaries and then, if permitted, Green's pages
167-345. The draft requests no source redistribution and requires a current
route refresh before any human-authorized send.

## A/B/C laboratory route

| Condition | Material and method | State | Quality question | Cost/speed observation |
| --- | --- | --- | --- | --- |
| A | catalogs, reviews, official technical standards, and exact resolved links | executed, metadata only | Can identity, component boundaries, false links, rights, and currentness be resolved without source payloads? | seconds per source; negligible retained storage; cannot assess sign-entry fidelity |
| B | lawful local copy of exact front matter and Green pp. 167-345, fixity-bound under an Item | prepared, not acquired | Can printed entries, page structure, sign forms, numbering, and errata be anchored without flattening them into text? | copy/access cost unknown; local OCR and layout work required |
| C | B plus current PCSL/OID, Born-Kelley data, and Unicode proposal concordance | prepared, not run | Which identities survive cross-source comparison, and where do later reassessments split or merge ATU 2 entries? | highest review cost; machine comparison is fast only after source-visible anchors exist |

Condition A proves only bibliographic and methodological foundations. B and C
must be evaluated by exact source return, sampled manual inspection, observed
error types, elapsed time, storage, and cost. A green validator cannot prove a
sign shape, reading, concordance, or semantic judgment.

## Rejected shortcuts

- Do not model the 377-page volume as one sign-list Work.
- Do not call `englund1987a.pdf` the full book or Green's sign list.
- Do not treat the AARome review link as a complete source Item.
- Do not infer redistribution permission from an open PDF endpoint.
- Do not replace ZATU coordinates with mutable PCSL OIDs.
- Do not persist provisional Unicode code points as durable ToS identity.
- Do not assign one translation or concept to a sign from its list label.
- Do not schedule human transcription or review before a real source-visible
  question exists.

## Remaining gates

- Full ATU 2 Item: absent.
- Green pp. 167-345 source bytes: absent.
- Exact complete contents and remaining Work boundaries: unresolved.
- Preprint/chapter textual equivalence: unasserted.
- Rights-holder and reuse permission: unresolved.
- Sign-entry anchors, forms, readings, glosses, translations, and etymologies:
  absent.
- ATU 2 to PCSL/OID/Unicode concordance: not materialized.
- Assyriological and human source-visible review: not performed or scheduled.
- Graph and canon: unchanged.

The durable result is a bibliographic source foundation that identifies what
ATU 2 is, what its sign-list contribution is, which accessible file is not the
book, where established criticism constrains interpretation, and why fresh
technical identifiers remain provisional coordinates rather than truth.
