# A01 — Nissen, *Uruk and I*: documentation and find-context control

Date: 2026-08-12
Status: metadata-only source planting; model-authored; unreviewed
Decision: plant one Work and one `authored_by` claim, but no article body,
archaeological fact, semantic relation, or graph promotion.

## Why this source matters

The A01 dossier names Hans J. Nissen's *Uruk and I* for a precise reason: it
does not merely add another interpretation of proto-cuneiform. It exposes how
the evidential foundation itself was made, damaged, reorganized, published,
and later interpreted. That makes the article a control on every downstream
claim that tries to connect a tablet, seal impression, vessel, building,
stratigraphic layer, date, or institutional interpretation.

The article must not be flattened into a neutral reference work. Nissen
describes it as a retrospective account of his own relations and experiences
with Uruk, including candid criticism of the excavation. This testimony is
valuable precisely because of its position and limits. It may identify records
and contradictions that require investigation, but it does not automatically
settle every disputed episode or turn recollection into an excavation-time
observation.

## 1. Official and classical documentation

### Exact Work and rights boundary

The [CDLI originating page](https://cdli.earth/articles/cdlj/2024-1) resolves:

- title: *Uruk and I*;
- author: Hans J. Nissen;
- serial identity: CDLJ 2024:1, ISSN 1540-8779;
- publication date: 2024-05-30;
- direct publisher PDF route;
- article-text license: CC BY 4.0 except where noted;
- separate treatment for artifact images under CDLI terms.

The present slice does not download or admit the HTML or PDF. Open access is
recorded at the exact article layer and is not generalized to artifact images,
linked excavation records, photographs, plans, or third-party material. No
permission request is necessary for the metadata-only planting.

### What Nissen actually establishes as a research problem

The article reports an asymmetric archive, not a uniformly undocumented dig.
Architecture could be recorded with great precision: plans, brick bonds, and
heights received sustained attention. Finds and find situations often did not.
Nissen reports cases with only a 20 by 20 metre square as provenience, almost
no find elevation, discarded pottery, inconsistent personnel and terminology,
category-based final publications detached from building contexts, and
identifier loss during tablet firing. He also questions direct dating of
rubbish and its contents from an underlying building and describes difficulties
aligning the older level system with later architectural sequences.

Those are source-reported episodes. Their immediate consequence is not a set
of accepted Uruk facts, but a set of explicit questions:

1. What was observed during excavation?
2. What record captured the observation, at what resolution and by whom?
3. What physical thing, volume, interface, or event did the record concern?
4. What identifier and custody transformations occurred afterward?
5. What was later inferred, reconstructed, normalized, or published?
6. Which original and later records support or contradict that operation?

### CRMarchaeo 2.0

[CRMarchaeo 2.0](https://cidoc-crm.org/extensions/crmarchaeo/html/CRMarchaeo_v2.0.html)
is the strongest official modeling control found. Its `A1 Excavation Processing
Unit` treats excavation as a documented sequence of observing and removing
matter and explicitly separates the material state at excavation time from
later causal interpretation. It also provides distinct routes for matter
produced or discarded, excavation surfaces, removed stratigraphic units, and
material destroyed by excavation.

For ToS, the especially important separation is:

- `AP11`: a physical relation;
- `AP13`: an assigned stratigraphic relation;
- `AP13.2`: the physical relation used to justify that stratigraphic relation.

Therefore a graph edge such as “earlier than” must not replace the observed
physical relation or the act that inferred the sequence. The justification is
part of the graph, not a comment that can be lost after promotion.

### ADS lifecycle guidance

The [Archaeology Data Service Guides to Good
Practice](https://archaeologydataservice.ac.uk/help-guidance/guides-to-good-practice/)
place documentation and metadata alongside data creation, naming, versioning,
formats, preservation, access, reuse, and intellectual-property decisions.
This supports a lifecycle model: provenance is created during fieldwork and
every later transformation, not appended only after an object reaches a
repository.

### Harris stratigraphy

Edward C. Harris's [*Principles of Archaeological
Stratigraphy*](https://harrismatrix.com/) remains the classical control for
representing stratigraphic sequences. Its proper role here is bounded. A
sequence representation can organize documented relations and expose
contradictions; it cannot recreate a height never measured, a findspot never
recorded, or an identifier lost without a surviving bridge.

## 2. Established top scholarship

Penelope Allison's [2008 Internet Archaeology legacy-data
issue](https://intarch.ac.uk/journal/issue24/) established legacy records as an
archaeological interpretation and reuse problem rather than a simple file
conversion task.

Faniel, Kansa, Kansa, Barrera-Gomez, and Yakel's [2013
study](https://www.oclc.org/research/publications/2013/context-archaeological-data-reuse.html)
shows why reusable data needs more than compliant fields. Reusers rely on
collection procedures, research design, field-document wording and structure,
the original team, and repository context to understand, verify, and trust a
dataset. An ontology can preserve many of these dimensions, but it cannot
silently invent the ones the source does not contain.

Together these works supply a durable rule for ToS: semantic normalization is
useful only when the original record, its production context, and the mapping
decision remain returnable.

## 3. Fresh current relevance check

The current literature sharpens four different aspects of the problem.

### Provenience and provenance are related, not interchangeable

Bethany G. Anderson's [2024
article](https://link.springer.com/article/10.1007/s10502-024-09459-5)
distinguishes archaeological provenience or findspot from archival provenance,
while showing how both depend on relationships and documented circumstances.
Excavation removes material from its original context and archival processing
recontextualizes its records. ToS therefore needs both the archaeological
encounter relation and the later documentary and custody chain.

### Legacy data still requires semantic judgment

Andrea D'Andrea's [2024
article](https://www.archcalc.cnr.it/journal/articles/1360) rejects the idea that
decades of heterogeneous, weakly semantic spatial and documentary records can
be made analytically coherent by automation alone. AI may help aggregate and
route evidence, but a human-accountable semantic mapping remains necessary.
The source should be treated as archaeological data with a history, not as an
inferior residue to be overwritten by a cleaner derived representation.

### Provenience crosses recording formats

Buchanan, Stephenson, Nesti, and Mogetta's [2025
study](https://www.mdpi.com/2076-0787/14/11/210) directly addresses
provenience across plans, sections, photographs, finds, spatial fields, and
changing recording systems. It supports relation-level links between records
instead of one flattened “context” field. The exact reuse license was not
resolved from the publisher response inspected here, so this planting records
metadata and relevance only.

### Reconstruction can help without becoming observation

De Weirdt, Nys, and Recke's [2026
article](https://www.cambridge.org/core/journals/advances-in-archaeological-practice/article/old-data-new-horizons-3d-modeling-as-a-catalyst-for-recontextualizing-fragmentary-legacy-data/A260C71CA9977ED28310F00250C0ACBD)
is the freshest direct control. It demonstrates that 3D-GIS can align and
recontextualize fragmented legacy records when physical overlap, modern
sondages, drawings, and bounded interpolation provide anchors. It also retains
critical limits: similarly named layers may not be equivalent, documentation
may conflict or be absent, and transparent manual manipulation can be more
honest than automation whose setup and validation costs exceed the small,
heterogeneous corpus.

The architectural rule is strict: every reconstructed elevation, boundary,
alignment, equivalence, or date remains a derived object with its anchors,
method, uncertainty, and alternatives. It never overwrites the field record or
inherits `observed` status.

## 4. Foundation model for ToS

The minimum useful topology should distinguish at least:

1. excavation project, season, trench, and processing unit or encounter event;
2. stratigraphic volume and interface;
3. physical find, sample, or amount of matter;
4. field record, register entry, plan, section, photograph, and label;
5. identifier assignment, replacement, loss, and reconciliation event;
6. conservation, firing, storage, movement, and custody event;
7. publication, selection, category split, normalization, and terminology
   crosswalk;
8. later reconstruction, spatial alignment, stratigraphic assignment, and date
   assignment;
9. testimony, memoir, criticism, and retrospective claim.

These are not merely columns. They are entities and events with source-bearing
relations. A tablet can survive while its excavation identifier does not. A
photograph can retain an identifier absent from the fired object. A publication
can correctly describe an object category while severing the wider find
context. A newer architectural sequence can be stronger for buildings yet
harder to connect to finds recorded under older level names.

### Relation-level completeness vector

A single confidence number would hide the actual failure mode. Every
consequential find-context relation should expose independent states for:

- horizontal location and grid precision;
- elevation and vertical precision;
- stratigraphic unit or level;
- depositional state: in situ, secondary, redeposited, or unknown;
- association with building, feature, deposit, or neighboring finds;
- recording method and responsible actor;
- identifier continuity;
- custody and conservation transformations;
- publication transformation and selection;
- terminology and crosswalk state;
- contradiction and alternative-reading state.

The value vocabulary must distinguish at least `recorded-exact`,
`recorded-coarse`, `reconstructed`, `conflicting`, `not-recorded`,
`record-lost`, `inaccessible`, `source-silent`, and `not-applicable`. A database
null cannot safely represent all of them.

### Negative invariants

- “Not found in surviving records” is not “absent from excavation”.
- A shared level label is not proof of a shared layer.
- Rubbish above a building is not automatically contemporary with or produced
  by that building.
- Digitization is not semantic recovery.
- Interpolation is not observation.
- Eyewitness memoir is not a neutral field register.
- Architectural precision does not transfer automatically to find-context
  precision.

## 5. Laboratory design

No experiment is run in this slice. The prepared design is:

### A — testimony extraction

Extract Nissen's claims with exact section anchors and classify each as
recollection, reported observation, judgment, criticism, methodological rule,
or proposed interpretation. Preserve hedges and scope. The output is not
accepted archaeology; it is a source-addressable claim set.

### B — record triangulation

For each consequential episode, seek the named excavation register, plan,
photograph, inventory, publication, object record, and later revision. Build
the completeness vector per relation, not per article. Preserve contradictions
and explicitly record when the relevant source is lost, inaccessible, silent,
or never created.

### C — machine reconstruction proposal

Permit GIS, rule-based, statistical, or LLM-assisted methods to propose
crosswalks, object matches, spatial alignments, or dates. Every proposal must
cite its anchors, declare interpolation and transformations, preserve
alternatives, and remain derived until adjudicated. Include negative controls
where labels match but physical evidence does not, or where an attractive
context link depends on a missing elevation.

Compare the arms by:

- false recovered links and false equivalences;
- provenance-completeness vector coverage;
- uncertainty calibration and contradiction preservation;
- ability to return to the exact supporting source;
- time, compute, financial cost, and human attention per adjudicated relation;
- error visibility when a source, identifier, or terminology mapping changes.

Local software, LLM, OCR, GIS, and 3D runtime trials belong in the
`abyss-stack` laboratory. ToS owns the source identity, provenance-bearing
research design, input and output references, and eventual reviewed result.
Human review is reserved for consequential unresolved relations and sampled
control points, not repeated transcription or ritual approval of every machine
step.

## Planting boundary

This slice creates one Work, reuses the existing provisional Hans J. Nissen
Agent, creates one unreviewed `authored_by` claim, one ordered discovery run,
one A01 source planting, and one provenance event. It creates no source text,
artifact, excavation fact, semantic assertion, graph relation, canon state,
external request, or human task.
