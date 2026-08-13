# A02 ETCSL digital corpus: ordered source planting research

Status: source-planting research complete; no ancient composition, source text,
translation, curriculum, semantic, graph, or canon claim admitted
Cut-off: 2026-08-12
Backlog anchor: A02 source table 13, row 1, tracked line 11
Original dossier label: `ETCSL`
Selected bibliographic Work: Black, Cunningham, Ebeling,
Flückiger-Hawker, Robson, Taylor, and Zólyomi, *The Electronic Text Corpus
of Sumerian Literature* (Oxford, 1998–2006)
Purpose: plant the dossier's first exact corpus route while keeping project,
scholarly corpus Work, archived release, contained ancient compositions,
editorial composites, physical witnesses, transliterations, translations,
lemmata, and current successor projects distinct.

## Outcome first

ETCSL is indispensable historical digital scholarship, but its authority has a
precise boundary:

```text
ETCSL project and scholarly corpus Work (1998–2006)
  != one ancient corpus or manuscript
  != the approximately 394 contained composition identities
  != an exact Oxford Text Archive release or acquired Item
  != current critical editions of every composition
  != current physical-witness metadata
  != an open redistribution licence
  != accepted school, curriculum, translation, semantic, or graph truth
```

The University of Oxford identifies ETCSL as a completed 1997–2006 project.
Its live site describes a selection of nearly 400 literary compositions from
the late third and early second millennia BCE, with Sumerian transliterations,
English prose translations, bibliographies, browsing, and search. The site
also states that funding ended in 2006 and that no content work is currently
being done. Backend maintenance in 2017 kept the interface alive; it did not
turn the archived editorial state into a continuously revised critical
edition.

The present planting therefore creates one modern collaborative Work for the
digital scholarly corpus. It creates no ancient Work, Collection membership,
Expression, Edition, Item, source payload, text layer, or accepted reading.
The Work identity is the authored digital scholarly resource, not a container
identity for silently absorbing its ancient contents. A future exact
composition route must model the ancient Work and its witnesses separately;
a future corpus-release question may justify a Collection and/or Edition only
with exact membership and release evidence.

## 1. Classical and official documentation

### Oxford project identity and completion state

The current Oxford Digital Humanities record for
[*Electronic Text Corpus of Sumerian Literature*](https://dh.web.ox.ac.uk/project/electronic-text-corpus-sumerian-literature)
gives start date 1997, end date 2006, status `Completed`, and describes the
searchable transliterations, translations, lemmata, and TEI XML. This record is
the strongest current institutional control for project lifecycle.

The official [ETCSL homepage](https://etcsl.orinst.ox.ac.uk/index.html)
defines the corpus scope and explicitly says that funding ended in summer
2006 and no work is currently being done to the site's contents. The
[project overview](https://etcsl.orinst.ox.ac.uk/about.htm) explains why the
corpus was built: the classical Sumerian literary corpus had to be
reconstructed from thousands of fragmentary tablets, relatively few
compositions had satisfactory modern editions, and older translations could
become unusable as knowledge advanced. ETCSL's own rationale therefore
forbids treating its composites and translations as timeless final text.

### Responsibility is collaborative and role-bearing

The official second-edition
[credits and copyright](https://etcsl.orinst.ox.ac.uk/edition2/credits.php)
page instructs users to cite:

> Black, Cunningham, Ebeling, Flückiger-Hawker, Robson, Taylor, and Zólyomi,
> *The Electronic Text Corpus of Sumerian Literature*, Oxford 1998–2006.

It distinguishes editor, senior editor, technical developer, project
associate, and director roles. The current ToS responsibility contract admits
only Work-level `authored_by` and `contributed_by`, not those finer historical
roles. The planting therefore records the seven named citation contributors
as source-reported `contributed_by` claims. It does not flatten their exact
project roles into false sole authorship, nor does it create an Organization
identity for a temporally changing project team.

### Corpus encoding is an editorial representation

The official [SGML/XML account](https://etcsl.orinst.ox.ac.uk/project/sgml-xml.htm)
and Oxford project record identify project-specific DTDs, later XML, and TEI
compatibility. The public corpus includes reconstructed composite texts,
translations, bibliography, and linguistic markup. These are modern editorial
representations over fragmented witnesses. XML validity and TEI compatibility
are structural evidence only; they do not verify a transliteration, lemma,
translation, witness relation, or ancient textual boundary.

### The archived revised edition is a separate future object

The Oxford Text Archive currently lists
[*The Electronic Text Corpus of Sumerian Literature. Revised edition.*](https://llds.phon.ox.ac.uk/llds/xmlui/discover?filter=Sumerian&filter_0=BCE&filter_1=corpus&filter_relational_operator=equals&filter_relational_operator_0=equals&filter_relational_operator_1=equals&filtertype=language&filtertype_0=date_range&filtertype_1=type&order=desc&rpp=80)
as an OTA Core Collection record with 10 files, 4.9 MB, and `Publicly
Available`. The item page was protected by an automated-access challenge
during this run, and the search record did not expose an item-specific reuse
licence. No corpus file was downloaded.

`Publicly Available` is an access statement, not an inferred redistribution,
AI-processing, or publication licence. ETCSL's credits page asserts copyright
and moral rights. A later release-level question must reconcile the exact OTA
metadata, file set, licence, and ETCSL copyright before creating an Edition,
Item, or payload.

### Current bibliographic and witness routes

The current CDLI publication record
[*Electronic Text Corpus of Sumerian Literature*](https://cdli.earth/publications/1691588)
assigns publication identifier `1691588`, dates the resource 1998–2006, and
links it to many `CDLI Literary` composite records. This is valuable
bibliographic and cross-provider evidence, but a `CDLI Literary` composite is
not a physical tablet and the publication link does not prove complete or
current witness membership.

## 2. Established and field-shaping scholarship

### Ebeling 2007: the corpus as a bounded computational object

Jarle Ebeling,
[*The Electronic Text Corpus of Sumerian Literature*](https://doi.org/10.3366/cor.2007.2.1.111),
*Corpora* 2(1) (2007), begins the mature corpus-linguistic assessment of
ETCSL. Ebeling and Graham Cunningham's edited volume
[*Analysing Literary Sumerian: Corpus-based Approaches*](https://www.equinoxpub.com/home/analysing-literary-sumerian/)
(Equinox, 2007) includes Ebeling's “Corpora, corpus linguistics, and the
Electronic Text Corpus of Sumerian Literature”, pp. 33–50, alongside studies
that demonstrate what the corpus enables and what its selection and composite
architecture condition.

For ToS, searchability and consistent lemmatization are properties of one
editorial corpus state. They do not erase manuscript variation or convert a
composite coordinate into a physical source anchor.

### Ebeling 2010: all-in-one utility remains a question

Ebeling's “The ETCSL: an all-in-one-corpus?”, in *Your Praise Is Sweet*
(Griffith Institute, 2010), pp. 53–67, is recorded on ETCSL's official
[project-publications page](https://etcsl.orinst.ox.ac.uk/edition2/etcslpublications.php).
The question mark is methodologically important: one digital corpus can
support reading, search, bibliography, and linguistic analysis while still
not replacing manuscript-level textual criticism, newer editions, or
material provenance.

### Delnero 2012: composites cannot erase variation

Paul Delnero,
[*The Textual Criticism of Sumerian Literature*](https://www.asor.org/asor-publications/book-series-monographs/jcs-supplemental-series/),
JCS Supplemental Series 3 (ASOR, 2012), ISBN `978-0-89757-088-6`, develops a
method for evaluating mechanical, local, regional, diachronic, scribal,
idiosyncratic, and interpretive variants. Its pressure on ToS is direct:

```text
composition identity
  -> individual manuscript witnesses
  -> readings and variants
  -> argued editorial reconstruction
```

The editorial composite remains useful, but it must not become the hidden
owner of all witness readings.

### Semantic-web and linked-data work: relation requires explicit limits

Terhi Nurmikko-Fuller's 2018 chapter
[*Publishing Sumerian Literature on the Semantic Web*](https://doi.org/10.1163/9789004375086_013)
uses ETCSL to examine ontology and publication questions. The ANU repository
records the chapter as restricted until 2037-12-31. Its central caution is
applicable to ToS: heterogeneous and incomplete ancient-world data, modern
interpretation, and ontology-designer bias must be explicit rather than
bridged by silent certainty.

Christian Chiarcos and colleagues,
[*Towards a Linked Open Data Edition of Sumerian Corpora*](https://aclanthology.org/L18-1387/)
(LREC 2018), show a linked-data route for Sumerian corpora and linguistic
annotation. The result supports status-preserving derived projections. It
does not authorize RDF or a graph database to replace source-owned corpus,
claim, or review records.

## 3. Fresh current relevance check

### ORACC/ePSD2: active complementary route, not identity replacement

The current ORACC/ePSD2 literary interface reuses and lemmatizes ETCSL
material alongside newer project corpora. The preliminary ComPass chapter
[*Data Acquisition: ETCSL*](https://oracc.museum.upenn.edu/compass/downloads/2_4_Data_Acquisition_ETCSL.html)
states that ETCSL is extensive but incomplete, that ORACC can host new or
updated editions, and that conversion changes character conventions and
lemmatization while omitting ETCSL translations from the ePSD2 dataset.

That route is a transformation, not equivalence:

```text
ETCSL XML/transliteration state
  -> declared conversion rules
  -> ORACC/ePSD2 representation
  != unchanged text, lemma, translation, or release identity
```

The ORACC [DSSt](https://oracc.museum.upenn.edu/dsst/) project supplies score
texts, composites, translations, and exact project scope for disputations,
Edubba texts, and diatribes. It is a current composition-specific alternative
where applicable, not a blanket replacement for all ETCSL contents.

### CDLI 2025–2026: current object and publication infrastructure

Rattenborg and Pagé-Perron's
[*The Cuneiform Digital Library Initiative: A Primer*](https://doi.org/10.5281/zenodo.15309738)
(2025) describes CDLI as an open-access research platform for nearly 400,000
cuneiform artifacts. The live CDLI surface shows 2026 updates and retains
ETCSL transliteration and translation links as external resources. CDLI is the
stronger current route for artifact metadata, provenience, collections, and
representation links. It does not automatically supply the preferred
critical reading of a literary composition.

### SumTablets 2026: fresh machine dataset, not philological successor

Simmons, Diehl Martinez, and Jurafsky,
[*SumTablets: A Transliteration Dataset of Sumerian Tablets*](https://arxiv.org/abs/2602.22200)
(arXiv preprint, 25 February 2026), derives 91,606 glyph–transliteration pairs
primarily from current Oracc/ePSD2 and OSL data and publishes a CC BY 4.0
dataset and code. It preserves some surfaces, line breaks, damage, columns,
and rulings, but converts transliterations to Unicode glyph sequences and
reports unknown mappings. Its authors report strong automatic chrF while also
finding genre imbalance, convention differences, name errors, missing
Unicode signs, and weaker literary/liturgical performance.

This is a valuable future A/B/C challenger for machine transliteration and
error discovery. It is not an ETCSL successor, critical edition, source-image
corpus, translation authority, or human philological review. The paper remains
a preprint at this cut-off.

### DCSL: named successor pressure, currentness unresolved

ETCSL's older project pages point to the *Diachronic Corpus of Sumerian
Literature* (DCSL). A current Oxford Digital Humanities catalog page still
labels DCSL `Current` and `Ongoing`, but the named project endpoint did not
respond during this run and no current release, repository, update history, or
working corpus interface was established. ToS must not convert an undated
institutional catalog label into a live-successor claim. DCSL remains a
deferred discovery lead until independently current project evidence exists.

## 4. General-web last check

Only after the official, established, and current scholarly routes were
resolved was the general web checked. Wikipedia, Reddit reading lists,
community guides, and newly published convenience interfaces repeatedly call
ETCSL useful, old, archival, or the best introductory corpus. These results
are retained only as discoverability and user-pressure evidence.

They cannot decide:

- which composition edition is current;
- whether a composite reading is defensible;
- which physical tablets support a line;
- whether a corpus file may be redistributed;
- whether DCSL, ORACC, CDLI, or another project is a legal or scholarly
  successor;
- whether a translation or lemmatization should be accepted.

Recency, votes, interface convenience, and search rank do not outrank the
Oxford lifecycle record, exact project credits, manuscript scholarship, or
current source-project evidence.

## Identity and authority fence for ToS

| Layer | Admit now | Defer or prohibit |
| --- | --- | --- |
| Project history | Oxford project, 1997–2006, completed | present-day active-team or institutional-succession claim |
| Scholarly corpus Work | collaborative ETCSL digital resource, 1998–2006 | ancient authorship or one universal Sumerian corpus identity |
| Corpus release | OTA revised-edition lead | Edition/Item without exact files, licence, and fixity |
| Ancient composition | later exact Work identity | silent creation of approximately 394 Works from one catalog label |
| Editorial composite | later source-owned composite with member evidence | ancient manuscript or physical artifact equivalence |
| Text and translation | later exact immutable layer and rights record | copying ETCSL content into tracked data now |
| Lemma and morphology | versioned project assertion | timeless linguistic fact |
| ORACC/ePSD2 conversion | explicit derived representation | unchanged identity or automatic supersession |
| CDLI route | artifact/publication metadata and provider links | complete witness list or preferred critical text by default |
| SumTablets | future machine challenger | critical edition, gold transliteration, or human review |
| Graph/index | disposable source-returning projection | source, review, or canon authority |

## Laboratory route: A/B/C, prepared but not run

| Condition | Input and method | Main question | Failure to measure |
| --- | --- | --- | --- |
| A — ETCSL-only | one predeclared composition's ETCSL composite, translation, bibliography, and lemmata | What can the historical corpus support cheaply and reproducibly? | stale reading, hidden witness variation, obsolete translation, unreturnable composite line |
| B — current-project reconciliation | A plus ORACC/ePSD2, DSSt when in scope, CDLI artifact records, and current critical bibliography | Which readings, lemmata, witness links, and translations changed or remain unresolved? | false equivalence, dropped variants, project-version drift, provider conflation |
| C — manuscript-first review | B plus exact tablet images/catalog records, manuscript transcriptions, provenience, collation, and human philological review | Which editorial assertions survive return to physical witnesses? | correction time, abstention, competing readings, source visibility, rights limits |

The sample must be fixed before comparing systems and include complete-looking
composites, damaged witnesses, extracts, additions, alternative recensions,
genre minorities, names, ambiguous signs, and missing Unicode mappings.
Quality, elapsed time, compute, retained bytes, rights effort, and human
attention are measured separately. A green XML or graph validator cannot
accept a reading.

## Decision matrix

| Candidate | Layer | Access/rights | Decision | Reason |
| --- | --- | --- | --- | --- |
| Oxford ETCSL homepage/about | official project documentation | open view; copyright retained | select | exact scope, content, and archival boundary |
| Oxford DH ETCSL record | current institutional metadata | open metadata | select | completed 1997–2006 project lifecycle |
| ETCSL second-edition credits | official responsibility and rights | open view; copyright and moral rights asserted | select | exact collaborative citation and roles |
| Oxford Text Archive revised edition | release lead | publicly available; item licence not resolved | defer acquisition | release identity and rights require exact item access |
| CDLI publication `1691588` | current bibliographic/provider route | open metadata; linked content rights separate | select | stable publication ID and composite links |
| Ebeling 2007/2010 | established corpus scholarship | publisher/official metadata; bodies term-specific | select | corpus-linguistic utility and boundary |
| Delnero 2012 | established textual criticism | commercial monograph metadata | select | manuscript variation and edition method |
| Nurmikko-Fuller 2018 | semantic-web scholarship | repository says restricted until 2037 | select metadata only | ontology and incompleteness control |
| Chiarcos et al. 2018 | linked-data method | ACL open paper; repository copy CC BY-NC 4.0 | select | derived LOD route, not authority transfer |
| ORACC/ePSD2/ComPass | current transformation route | open view; project-specific rights | select with draft status | current lemmatized representation and explicit conversion |
| CDLI Primer/current site | current artifact infrastructure | open-access metadata; media item-specific | select | material-provenance complement |
| SumTablets 2026 | fresh ML dataset/preprint | dataset reported CC BY 4.0 | watchlist | useful challenger, not critical or translation authority |
| DCSL | named successor lead | currentness and endpoint unresolved | defer | no verified live release or interface |
| general-web summaries/tools | popularity/discovery | mixed | reject as authority | convenience and recency cannot settle identity or reading |

## Planting decision

Create:

- seven provisional Agents named in ETCSL's prescribed citation;
- one verified modern Work
  `tos.work.digital-sumerian-corpora.electronic-text-corpus-of-sumerian-literature`;
- seven unreviewed `contributed_by` claims preserving collaborative
  responsibility without inventing finer unsupported ToS roles;
- one ordered discovery record;
- one A02 source planting fulfilling tracked source-backlog line 11.

Do not create:

- an ETCSL project Organization or institutional-successor claim;
- a Collection of ancient Works or approximately 394 membership claims;
- an ancient composition Work;
- an OTA Edition or Item;
- a source payload, corpus snapshot, XML, transliteration, translation,
  bibliography row, lemma, sign, anchor, or semantic object;
- a current-edition, school, curriculum, transmission, graph, or canon claim;
- an access request, external message, server transfer, or human task.

The later A02 backlog label `ETCSL project publications` must reuse this exact
Work and discovery evidence where the role is the same. It must not create a
duplicate Work merely because the dossier repeats the project in a control
table. Separate publications arising from ETCSL remain separate Works only
when an exact later source need selects them.

## Remaining uncertainty

- The precise legal reuse terms of the OTA revised-edition files remain
  unresolved; public access alone is insufficient.
- ETCSL's Work identity is verified, but all responsibility claims remain
  source-reported and human-unreviewed.
- DCSL's current institutional catalog status conflicts with the absence of a
  verified live endpoint or current release.
- ORACC/ePSD2 reuses and transforms ETCSL-derived data, but exact per-text
  derivation and replacement status require composition-level evidence.
- CDLI composite links support navigation; complete witness membership and
  current readings require exact composition-level review.
- SumTablets is fresh and reproducible-looking but still a preprint and its
  reported aggregate metric cannot establish literary-text correctness.
