# A01 CDLB 2021:6 quantitative sign-use source planting research

Research snapshot: 2026-08-12. No article body, dataset, code archive,
artifact image, transliteration, sign list, or semantic assertion entered ToS.

## Decision

Plant one verified bibliographic Work:

> Logan Born and Kathryn Erin Kelley, “A Quantitative Analysis of
> Proto-Cuneiform Sign Use in Archaic Tribute,” *Cuneiform Digital Library
> Bulletin* 2021:6, published 10 December 2021.

The same Work fulfills two distinct A01 backlog roles through two planting
records:

1. line 16, the corpus/edition route for updated sign statistics, corpus size,
   and the Tribute case study;
2. line 22, the control/review route that keeps corpus-scale frequencies and
   the proposed proto-literary status of Tribute bounded.

This is one Work used twice, not two duplicate Works. The risk-control need at
line 5 is informed by the same research but is not itself a planting target:
that row has no `source_label`, and the current planting contract addresses
exact labeled source anchors.

An older P000015 discovery result and artifact bibliography called the article
“An Updated Sign List for Proto-cuneiform.” That phrase is not the title shown
by the publication. This packet records the correction append-only and the new
Work uses the exact title; the prior digest-bound discovery event and artifact
witness are not silently rewritten.

## 1. Official and classical documentation

### Exact publication identity and rights

The [CDLI article page](https://cdli.earth/articles/cdlb/2021-6) supplies:

- exact title, both authors, journal, issue identifier `CDLB 2021:6`, ISSN
  `1540-8760`, and publication date `2021-12-10`;
- author affiliations reported on the article page: Simon Fraser University
  for Logan Born and University of Toronto for Kathryn Erin Kelley;
- article text under Creative Commons Attribution 4.0, except where noted and
  excluding artifact images, which remain under the separate CDLI terms;
- canonical citation key `Born2021Quantitative` and the stable article URL.

The PDF is an open manifestation of the article, but this planting stops at
Work metadata. It does not create an Expression, Edition, Item, or local file;
that avoids pretending that journal publication structure has already been
reconciled at those levels.

CDLI author profile `1466` reports Logan Born but no ORCID. CDLI author profile
`1177` reports Kathryn Erin Kelley and ORCID `0000-0003-2886-1606`. Born
therefore remains a provisional Agent; Kelley is verified only to that exact
CDLI/ORCID identity. Neither identity record generalizes article authorship
into authority over every corpus claim.

### Classical substrate and what the article supersedes

The article explicitly starts from three historically separate layers:

- Green and Nissen 1987 / ATU 2 supplies the sign-list and transliteration
  convention substrate;
- Englund 1998 supplies the earlier top-sixty frequency table;
- the CDLI corpus and transliterations as known in 2020 supply the article’s
  computational source snapshot.

The article says it updates and supersedes Englund’s frequency list. It does
not supersede ATU 2 as a historical sign identity system, the physical tablets,
or the evolving CDLI editions. “Supersedes” is therefore scoped to the reported
frequency list and method, not to source authority in general.

### Exact 2020 corpus snapshot

For the retrieval performed on 19 May 2020, the article reports:

- 6,726 Uruk-period tablet records;
- 6,267 with a transliteration;
- 5,274 with at least one readable sign;
- 52,943 readable tokens in non-numerical entries before compound splitting;
- 1,990 sign names before variant merging;
- 1,299 after variant merging;
- 56,504 tokens and 1,261 names after compound splitting;
- 705 names after both compound splitting and variant merging;
- within that last count, 42 numerical and four proto-Elamite signs, leaving
  659 non-numerical Uruk IV–III signs under those parameters.

These are source-reported values for a dated retrieval, not current ToS corpus
facts. They must never be detached from the period, object-type, genre,
provenience, variant, compound, readability, and counting settings.

## 2. Established top scholarship and methodological controls

### Frequency is a measurement event, not a sign property

Born and Kelley make the variables explicit. A reusable ToS observation must
bind at least:

- corpus provider and retrieval date or immutable snapshot;
- period and object-type filters;
- genre and provenience filters;
- treatment of unreadable/damaged signs;
- whether graphic variants are merged;
- whether compound graphemes are kept or split;
- whether modern transliteration order is preserved or ignored;
- unit of counting: token, case/line, tablet, or another declared unit;
- software/data version and exact output provenance.

Without those fields, a bare `sign -> frequency` edge is misleading. The
authors also decline to attach one-word “translations” to the frequency table:
proto-cuneiform signs have multiple functions, and a familiar gloss can hide
administrative use. ToS must keep sign identity, occurrence, reading, lexical
value, translation, administrative function, concept, and interpretation as
different claim layers.

### Spatial organization and co-occurrence

Signs in a proto-cuneiform case do not form a securely understood linear
sequence. Transliteration order is a modern scholarly representation. The
article therefore permits ordered or unordered co-occurrence and distinguishes
raw combination counts from the number of cases containing a combination.

Consequences for the future semantic stack:

- a transliteration sequence is not automatically a source-visible word;
- co-occurrence is not syntactic dependency;
- unordered co-occurrence is not semantic equivalence;
- repeated signs can inflate combination counts unless the counting unit is
  explicit;
- spatial geometry needs its own source-anchored layer rather than being
  reconstructed from a linear string.

### Tribute as a frontier case

The case study compares the lexical composition *Tribute* with administrative
usage. It tests competing interpretations rather than classifying the work as
the first literary or philosophical text. The article reports, under its exact
settings, 11,611 distinct two-sign case-level combinations in Uruk III/IV
administrative texts and a long tail of rare combinations. It finds that none
of the 412 most common administrative bigrams occurs in Tribute and that many
Tribute pairs lack administrative parallels, while other analyses show the
ending is more administratively familiar than the beginning.

The proper A01 status is therefore `frontier`: computational distance from
known administrative practice can generate testable questions, but cannot by
itself establish genre, narrative, cultic meaning, literary priority, or
philosophical content. Civil, Veldhuis, Westenholz, Kelley, and the article’s
own alternative hypotheses remain interpretation controls.

## 3. Fresh current relevance check

### PCSL current frequency surface

The current [PCSL frequency page](https://oracc.museum.upenn.edu/pcsl/signlist/Frequencies/index.html)
exposes a live sign list with counts, including numerical signs and `X` among
the leading entries. Its values differ materially from the 2020 article table.
That is evidence of a changed representation/corpus surface, not evidence that
one table is “wrong.” PCSL does not expose enough parameter metadata on the
frequency page to treat the figures as directly comparable.

ToS should therefore retain both as dated observations and require a deliberate
reproduction experiment before calculating drift. The experiment must freeze
identical inclusion rules and a concordance between historical ATU/ZATU names,
article normalization, PCSL OIDs, and any Unicode identity.

### Unicode and PCSL identity changes

The 2025 Unicode proto-cuneiform proposal and the 2026 allocation pipeline use
PCSL as a current technical identity layer. Numerical and non-numerical signs
follow distinct encoding routes, and provisional allocations can still move.
Unicode characters and PCSL identifiers are valuable coordinates, but neither
is a reading, gloss, corpus occurrence, contextual quantity, or stable
semantic entity.

### Currentness of the 2020 code route

The versioned software deposit is:

- Logan Born, *MrLogarithm/pe-pc-datasets-interface: Proto-cuneiform sign
  count utilities*, v1.0.2, Zenodo DOI `10.5281/zenodo.4062226`, published
  2020-10-01;
- concept DOI `10.5281/zenodo.3858116`, which still resolved to v1.0.2 when
  checked on 2026-08-12;
- GitHub default branch `master`, last pushed 2022-08-27; the latest commit only
  clears old notebook output, while the substantive static-data update remains
  dated 2020-10-01.

Zenodo describes the license only as `other-open`, and GitHub exposes no
recognized repository license. Access is not converted into reuse permission.
No code or dataset is imported in this planting. A later laboratory
reproduction must first resolve exact dependency versions, dataset identity,
license, and compatibility with current CDLI/PCSL data.

### Fresh interpretive context

The 2025 open-access Antiquity article “Seals and signs: tracing the origins of
writing in ancient South-west Asia” is retained as a contextual control for
precursor and semantic inference. It does not replace sign-frequency evidence
or authorize shape-to-meaning inference. Generic 2025–2026 cuneiform OCR and
sign-detection work is excluded from this planting because later-period visual
classification does not answer the proto-cuneiform counting and semantics
problem without an exact domain bridge.

## Source, rights, and authority boundaries

- The article’s text is openly licensed CC BY 4.0; artifact images are not
  covered by that blanket statement.
- The linked software deposit and repository remain separately governed; their
  public availability does not establish a redistribution license.
- No source content is required for this metadata planting, so no access
  request is prepared or sent.
- No article claim becomes accepted ToS truth merely because it is
  peer-reviewed.
- No model-made sign, occurrence, frequency, co-occurrence, lexical, semantic,
  graph, or canon object is created.
- No human review task is opened. Later human work should adjudicate only
  consequential boundary cases, not retype or redundantly approve metadata.

## Laboratory consequences

When ToS later tests this route, the smallest honest A/B/C design is:

- **A — frozen reproduction:** run the versioned 2020 code and data exactly,
  preserving environment, parameters, output, cost, and failures;
- **B — harmonized current rerun:** apply the same declared counting policy to
  a dated current corpus snapshot and an explicit identity concordance;
- **C — adversarial parameter sweep:** vary one dimension at a time (period,
  genre, variant merging, compound splitting, order, count unit) and measure
  rank and conclusion sensitivity.

Manual verification samples must inspect source-returnable tablets/cases and
the actual outputs. A green schema or deterministic rerun is not evidence that
the counts express the intended corpus or that a semantic conclusion is true.

## Planting boundary

The present slice creates only:

- one Work;
- two author Agents;
- two unreviewed `authored_by` claims;
- one ordered discovery run;
- two role-specific A01 source plantings over the same Work;
- one digest-bound provenance event and regenerated navigation projections.

Everything textual, computational, visual, sign-level, semantic, graph-level,
canonical, public-serving, and human-reviewed remains absent.
