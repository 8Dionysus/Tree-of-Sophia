# A02 CDLI digital artifact registry: ordered source planting research

Status: source-planting research complete; no artifact, source text, image,
transliteration, translation, semantic, graph, or canon claim admitted
Cut-off: 2026-08-12
Backlog anchor: A02 source table 13, row 2, tracked line 12
Original dossier label: `CDLI`
Selected bibliographic Work: *Cuneiform Digital Library Initiative* (CDLI),
an evolving collaborative scholarly digital resource launched in 1998
Purpose: plant the dossier's material and archaeological source route while
keeping the project, its changing organization, the digital resource Work,
catalog releases, artifact records, physical objects, images, inscriptions,
publications, derived annotations, and rights layers distinct.

## Outcome first

CDLI is the strongest broad discovery and material-metadata route in this
part of A02. It is not a single immutable corpus release or a source of
automatically accepted readings:

```text
CDLI evolving scholarly resource Work
  != the changing people and institutions that operate it
  != one immutable database release
  != the physical cuneiform artifacts it describes
  != artifact ownership or present custody
  != every linked publication, image, transliteration, or translation
  != a critical edition of every inscription
  != one blanket open licence across all layers
  != accepted archaeological, textual, semantic, or graph truth
```

The current project describes an international collaboration of
Assyriologists, curators, historians of science, museums, and technical
partners. It documents the form and content of cuneiform inscriptions from
the beginning of writing into the late first millennium BCE. Its present
catalogue exposes more than 400,000 artifact records, while the project
estimates a recovered universe above 500,000 objects. Those are mutable
coverage statements, not stable identity facts or completeness guarantees.

The planting therefore creates one modern Work identity for CDLI as the
evolving scholarly resource. It creates no Organization because the current
operator and partner topology is distributed and changes over time; it also
creates no individual responsibility claims because the official pages do
not define one stable, exhaustive contributor statement for the Work across
its full history. Exact historical and current responsibility may be modeled
later as source-reported, time-bounded claims.

## 1. Classical and official documentation

### Founding scope and representation layers

Robert Englund's Max Planck preprint 183 (2001),
[*The Cuneiform Digital Library (CDLI)*](https://www.mpiwg-berlin.mpg.de/Preprints/P183.PDF),
records the early project as an international effort to publish the form and
content of early cuneiform tablets. It already separates the intended data
set into document transliterations, glossaries, digitized originals, and
photographic archives. It also presents digital documentation as a basis for
later paleographic, lexical, grammatical, administrative, and semiotic work,
not as a replacement for the tablets.

This classical statement establishes the durable source order:

```text
physical artifact and excavation/custody evidence
  -> catalog and publication metadata
    -> images, copies, inscription representations, and translations
      -> explicit scholarly or computational derivations
```

The current [CDLI About page](https://cdli.earth/about) expands the temporal
scope and reports more than 400,000 cataloged artifacts. It also preserves
historical narrative figures that differ from the current headline. ToS does
not freeze either number as an invariant: the page itself demonstrates why a
dated access receipt and release coordinate are required for quantitative
claims.

### The artifact model is relational, not a flat row

The official
[artifact metadata documentation](https://cdli.earth/docs/artifact-metadata-fields)
separates:

- CDLI artifact ID and designation;
- artifact type, material, condition, preservation, and dimensions;
- period and date statements;
- provenience, place of writing, excavation, findspot, and stratigraphy;
- present collection and museum number;
- archive membership and composite number;
- language, genre, seals, publications, and external resources;
- field-level uncertainty, including uncertain language, genre, material,
  date, provenience, and archive assignments.

Retired records are preserved with redirects rather than silently deleted.
That posture is directly compatible with ToS's supersession and claim-trace
discipline. It also prohibits flattening `provenience`, `written_in`, current
collection, archive, publication link, and composite membership into one
generic `origin` or `source` edge.

### Search, API, and export are access mechanisms

The official [search documentation](https://cdli.earth/docs/search) and
[live search surface](https://cdli.earth/search) support publication,
collection, provenience, period, transliteration, translation, and P/Q/S
identifier queries. The current exports include flat, expanded, and linked
catalogue forms; ATF/JTF inscription data; CoNLL and CoNLL-U annotations;
linked annotations; word and sign lists; and bibliography.

The official [API documentation](https://cdli.earth/docs/api) exposes JSON
metadata endpoints for artifacts, collections, composites, dates, genres,
languages, materials, publications, external resources, and update events.
Artifact and vocabulary routes can also emit JSON-LD, RDF, RDF/XML,
N-Triples, and Turtle. These interfaces make CDLI an excellent discovery and
interchange source. They do not make a JSON-LD graph, export row, or current
API response the owner of artifact, reading, or rights truth.

### Rights are layer-specific

The official [terms of use](https://cdli.earth/terms-of-use) distinguish
images from text:

- photographs and images of original artifacts generally remain owned by
  the holding institutions, except where stated otherwise;
- line art remains associated with the named publication or creators;
- image use is framed as noncommercial, and commercial publication requires
  written permission;
- transliterations and translations may be copied, aggregated, and reused
  according to academic practice, with acknowledgement for considerable
  reuse.

The statement is not one machine-actionable licence over every database
field, image, text, annotation, linked publication, and derivative. A future
Item must carry separate metadata, source-text, scan/image, derivative,
server-processing, training, and redistribution evidence. This planting
downloads and republishes none of those payloads.

The [contribution documentation](https://cdli.earth/contribute) further
separates community data contributions, reviewed update events, software
contributions, and image submission. The web framework and related code may
have permissive software licences; those licences do not transfer to museum
images, inscriptions, database exports, or third-party publications.

### Metadata and publication links remain claims

The official guide for
[publishing on CDLI](https://cdli.earth/docs/publishing-on-cdli-for-journal-editors)
makes metadata the first priority and recommends images, line art, and surface
notes when available. Artifact-publication association uses P identifiers,
publication keys, exact references, museum numbers, and genres. This is
evidence that catalog relations are curated assertions, not automatic proof
that a publication's reading is complete, current, or textually identical to
another representation.

## 2. Established and field-shaping scholarship

### The 2025 primer fixes the current project identity

Rune Rattenborg and Émilie Pagé-Perron,
[*The Cuneiform Digital Library Initiative: A Primer*](https://zenodo.org/records/15309738)
(version 1.0, 2025, DOI `10.5281/zenodo.15309738`), describe CDLI as an
open-access research platform launched in 1998 and maintaining close to
400,000 artifact records. The primer explains the catalogue, search,
contribution, bibliography, and future project routes. Zenodo identifies the
deposit as a lesson and displays copyright rather than a reusable licence for
the primer body. Its DOI identifies the primer, not the CDLI resource Work or
one CDLI database release.

### Digital editions require transformation receipts

Nele Homburg and colleagues,
[*From an Analog to a Digital Workflow: An Introductory Approach to Digital Editions in Assyriology*](https://cdli.earth/articles/cdlb/2023-4)
(CDLB 2023:4), show why machine-readable editions require explicit editorial
and transformation work. Correct syntax, linked identifiers, and enriched
data improve reuse, but do not dissolve the material and philological
complexity of source objects. The article is CC BY 4.0 except where artifact
images remain governed separately.

Christian Chiarcos and colleagues,
[*Towards a Linked Open Data Edition of Sumerian Corpora*](https://aclanthology.org/L18-1387/)
(LREC 2018), demonstrate a linked-open-data route from CDLI metadata and ATF.
For ToS this supports a rebuildable graph projection with typed provenance;
it does not justify treating the projection as the source record or
silently merging artifact, composite, text, sign, lemma, and concept IDs.

### Material provenance exceeds a database coordinate

[*Envisioning networked provenance data storytelling with American cuneiform collections*](https://doi.org/10.1007/s00799-022-00335-0)
shows how item- and collection-level histories include purchase, transfer,
loan, deaccession, theft, archival reconciliation, expert dating, imaging,
and later CDLI contribution. Present custody, archaeological provenience,
collector history, digitization provenance, and database availability are
different relations. ToS must preserve those paths rather than compress them
into one edge.

### Derived machine corpora reveal selection pressure

The CuneiML data paper,
[*CuneiML: A Machine Learning Dataset for Cuneiform Tablet Analysis*](https://doi.org/10.5334/johd.151)
(2023), derives a filtered vision corpus from CDLI records and images. Its
large scale demonstrates CDLI's value for downstream work, while its
selection, scraping, preprocessing, image availability, and reconstruction
choices demonstrate why a derived dataset is not a transparent copy of CDLI
or a representative sample of all cuneiform objects.

## 3. Fresh current relevance check

### The service and curation stream are active in 2026

The live [CDLI home page](https://cdli.earth/) exposes accepted metadata
updates dated May 2026. The current Max Planck Institute of Geoanthropology
[project page](https://www.gea.mpg.de/200475/the-cuneiform-digital-library-intitiative)
reports more than 400,000 records, 738 provenienced sites, 1,284 collections,
and more than 67,000 records added since the 2022 interface launch. These are
current institutional activity signals, not an immutable versioned release.

The current CDLI GitLab documentation and framework repositories, and the
CDLI GitHub organization, show active software work in 2026. The last visible
named Phoenix beta changelog entry is older than the current activity. ToS
therefore records a live evolving service and declines to invent a stable
database version from a software tag or website footer.

### 2026 evidence exposes the real metadata-production boundary

Alstola, Sahala, Valk, and Ong,
[*Semi-Automatic Annotation of Babylonian Cuneiform Texts*](https://doi.org/10.5334/johd.494)
(*Journal of Open Humanities Data* 12:41, published 6 March 2026), report a
workflow over 6,099 Babylonian archival texts. For CDLI metadata they used
existing records, augmented records, or created new entries primarily from
original publications and sometimes another project. They state that
metadata quantity and quality depend on available source material; the work
was mainly manual but assisted by scripts and Microsoft Copilot. Nearly one
thousand CDLI entries were created and roughly 300 updated.

This fresh peer-reviewed evidence is decisive for ToS's authority posture:

```text
CDLI metadata field
  = a versioned, curated, source-dependent assertion
  != direct observation merely because it appears in the database
```

The same paper reports that syntax validation can coexist with manual
correction, unresolved out-of-vocabulary material, occasional semantic
mislabeling, and automatic transcription that was not manually corrected.
Green structural validation is therefore necessary but never sufficient for
textual, linguistic, or semantic acceptance.

### Fresh ML remains a challenger, not an authority upgrade

Frederik Hagelskjær,
[*A novel network for classification of cuneiform tablet metadata*](https://arxiv.org/abs/2603.03892)
(arXiv v2, 15 July 2026), proposes point-cloud classification for tablet
metadata under limited annotated data. It is retained as a fresh future
laboratory challenger for material-shape inference. It is a preprint and does
not establish artifact identity, provenance, period, genre, reading, or
metadata correctness for any CDLI record.

The old Zenodo monthly CDLI deposit `2022.01` remains useful historical release
evidence, but it is rejected as a current 2026 snapshot. Before any bulk
laboratory import, ToS must resolve an exact current export, timestamp,
checksum, field scope, licence by layer, and update-event boundary.

## 4. General-web last check

Only after the official, classical, established, and fresh sources were read
was ordinary web search used. Wikipedia and general descriptions reproduce
the broad mission and historical names but cannot resolve current operators,
release identity, field provenance, layer-specific rights, or scholarly
quality. They are rejected as planting authority.

General search did expose one useful pressure: many downstream pages call
CDLI a corpus, library, database, project, platform, or collection. These
labels describe overlapping functions. ToS selects `Work` as the narrowest
current bibliographic identity available to the source-planting contract,
while explicitly refusing equivalence with an Organization, database
release, Collection of ancient Works, or set of physical artifacts.

## Identity and authority fence for ToS

| Layer | Admit now | Defer or prohibit |
| --- | --- | --- |
| Scholarly resource Work | evolving CDLI digital research resource, launched 1998 | exact stable organization or exhaustive contributor roster |
| Database release | current live service and export routes | Edition/Item without exact snapshot, checksum, scope, and rights |
| Artifact | later exact P-record plus physical-object reconciliation | bulk artifact creation from a project label |
| Composite | later exact Q-record and membership claims | physical tablet or critical text equivalence |
| Publication | later exact publication Work and link claim | reading acceptance from a P-link alone |
| Images and copies | later Item/File with holding-institution rights | blanket reuse from open viewing |
| Transliteration/translation | later exact representation and version | timeless or universally current text |
| Metadata fields | source-reported claims with update provenance | direct observation or semantic truth by default |
| Graph/API/export | rebuildable access and relation projection | owner truth or merge authority |
| Semantics | later source-returnable annotation and review | automatic promotion from genre, lemma, or co-occurrence |

## Planting decision

Create:

- one verified modern Work,
  `tos.work.digital-cuneiform-corpora.cuneiform-digital-library-initiative`;
- one ordered discovery run preserving official, established, fresh, and
  general-web-last evidence;
- one A02 source planting for backlog line 12;
- one provenance event for this metadata-only planting.

Do not create:

- a CDLI Organization or individual contributor roster without a stable,
  time-bounded responsibility source;
- ancient Works, Collections, artifacts, composites, Expressions, Editions,
  Items, Files, or payloads;
- accepted dates, proveniences, holdings, archive memberships, publications,
  signs, readings, transliterations, translations, lemmata, or semantic
  relations;
- redistribution, publication, training, or server-import permission;
- human review tasks, graph promotion, or canon state.

The later A02 backlog row `CDLI project` must reuse this Work and discovery
route unless it names an exact distinct release or publication. It must not
create a duplicate CDLI identity.

## Future A/B/C laboratory route

For one exact artifact family, compare:

- **A — live CDLI record:** exact P/Q/S identifiers, API response, update
  event, field uncertainties, linked publications, and layer-specific rights;
- **B — publication and holding-institution return:** catalog, excavation,
  museum, publication, image, and custody evidence reconciled field by field;
- **C — derived machine route:** export, linked data, NLP, vision, or LLM
  inference with exact input snapshot and transformation receipt.

Measure field coverage, contradiction rate, stale-link rate, source-return
cost, expert minutes, machine time, compute cost, uncertainty calibration,
and whether every material claim can be traced back to evidence. Manual
inspection of a small source-visible sample remains the reality check; no
validator, API success, or aggregate model score can substitute for it.
