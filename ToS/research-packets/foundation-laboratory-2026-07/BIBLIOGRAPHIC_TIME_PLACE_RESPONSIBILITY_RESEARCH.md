# Bibliographic Time, Place, and Provision Responsibility Research

Status: ordered non-authoritative research, bounded contract recommendation,
and one exercised two-Edition model-run experiment; not accepted bibliography,
human review, graph truth, or authorization to bulk-populate the corpus

Research snapshot: 2026-07-31

## Result

The next literature-trunk layer should not add flat `publisher`, `place`, and
`date` fields to Edition records, and it should not immediately import the
whole LRMoo/CIDOC event ontology. The smallest defensible next object is a
source-returnable **provision-activity claim** owned by one Edition.

That claim should keep together the n-ary bibliographic statement that a
particular provision activity involved:

- one exact Edition;
- one typed activity such as publication, production, distribution, or
  manufacture;
- one transcribed or reported statement;
- zero or more role-specific place assertions;
- zero or more role-specific agent assertions;
- a bounded date or interval with declared precision;
- evidence, maker, method, provenance event, review state, and source return.

The claim must preserve both a literal form and a normalized identity when
both are known. It must not make normalization a prerequisite for retaining
what the source actually says.

The recommendation was exercised on 2026-08-01 through the two positive cases
and explicit negative controls below. The source-return, role, identity, date,
graph-edge, size, and timing evidence is preserved in
[`PROVISION_ACTIVITY_ABC_EXPERIMENT.md`](PROVISION_ACTIVITY_ABC_EXPERIMENT.md).
That result retains the bounded claim shape as foundation mechanics while all
new claims and identities remain model-made, provisional or `unreviewed`.

```text
Edition
  -> provision-activity claim
       |-> kind: publication | production | distribution | manufacture
       |-> statement: exact transcription or exact reported wording
       |-> place assertion
       |    |-> literal form
       |    `-> optional normalized Place ref
       |-> agent assertion
       |    |-> literal form
       |    |-> role
       |    `-> optional normalized Agent or Organization ref
       |-> bounded date / interval / precision
       `-> evidence + provenance + review + source return
```

This profile is intentionally narrower than BIBFRAME, LRMoo, CIDOC CRM, RDA,
or GND. Those standards guide crosswalks and distinctions; they do not become
the hidden owner of ToS source meaning.

## Why this is the next foundation gap

The current source-witness tree has fourteen Edition records. Place,
publisher, and year are commonly visible only inside `preferred_label`,
`notes`, source metadata, or the current two Edition-owned publication-claim
prototypes. There are no tracked Place or corporate-organization record
families. Only the *Götzen-Dämmerung* 1889 and *Der Fall Wagner* 1888 Editions
currently carry structured `publication_claim_refs`.

The newly completed Work chronology answers a different question: it gives
each current Nietzsche Work one bounded first-publication profile. It does not
state which organization performed an Edition-level provision activity, what
form the source transcribed, which place the statement names, or whether the
date belongs to printing, title-page dating, public sale, private issue, or a
later catalog report.

Consequently:

- a Work first-publication date cannot stand in for an Edition imprint;
- an Edition label cannot stand in for a reviewed place or publisher claim;
- an imprint cannot stand in for the actual date a work became available;
- a source transcription cannot silently become an authority-file identity;
- a publisher cannot silently become a printer, manufacturer, distributor,
  editor, designer, rights holder, or current corporate successor.

## Research method and freshness rule

Evidence was checked in the required order:

1. current standards and official documentation;
2. established knowledge-graph and cultural-heritage design work;
3. the freshest directly relevant 2025-2026 work discoverable at the
   snapshot date.

The freshness pass was repeated on 2026-07-31. A newer version number was not
treated as implementation authority by itself. This matters here: CIDOC CRM
7.3.2 is newer than 7.1.3, but the official version table marks 7.3.2 as a
draft and 7.1.3 as the current Official ISO Correspondence release. In the
opposite direction, LRMoo 1.1.1 has moved beyond 1.0 and is itself listed as
Official (IFLA).

## I. Current standards and official documentation

### IFLA LRM 2024: identity before event detail

The [2024 IFLA Library Reference Model](https://repository.ifla.org/items/214c74cb-c075-4428-a138-39f8d06c55aa)
remains the high-level identity foundation. Work, Expression, Manifestation,
and Item are distinct; Agent, Place, and Time-span are also distinct entities.
For ToS, an Edition is the local manifestation-like owner and an acquired
file remains item/file evidence, not a replacement for that identity.

LRM is deliberately conceptual. It justifies the separation of identity
layers but does not prescribe the first local JSON shape for an imprint,
provision event, or source-returnable claim.

### LRMoo 1.1.1: manifestation creation is not item production

The current [LRMoo version table](https://cidoc-crm.org/lrmoo/fm_releases)
lists version 1.1.1, released in November 2025, as Official (IFLA). The
[1.1.1 definition](https://cidoc-crm.org/sites/default/files/LRMoo_V1.1.1.pdf)
keeps several distinctions that directly protect ToS:

- `F30 Manifestation Creation` creates an `F3 Manifestation`;
- `F32 Item Production Event` materializes a Manifestation and produces
  individual `F5 Item` objects;
- Work creation and Expression creation remain separate event classes;
- a later reproduction event is not the same activity as the original
  publication or item production.

Therefore a title-page imprint, creation of a publication specification,
printing of copies, release for sale, and acquisition of one digital surrogate
must not be collapsed into one generic `published_at` edge.

LRMoo is an interoperability and reasoning guide here. The first ToS slice
does not need to instantiate every event class or reproduce the entire
ontology locally.

### CIDOC CRM: use the official implementation line

The current [CIDOC CRM version table](https://cidoc-crm.org/versions-of-the-cidoc-crm)
lists 7.3.2, released March 2026, as Draft. The same page and the
[last-official-release page](https://cidoc-crm.org/get-last-official-release)
identify 7.1.3, released February 2024, as Official (ISO Correspondence) and
provide its normative serializations.

ToS should therefore:

- map to the official 7.1.3 line when claiming implementable CRM
  compatibility;
- observe 7.3.2 only for future model changes;
- never advertise draft-version use as current official conformance;
- keep local claim identifiers and source provenance stronger than a
  replaceable ontology projection.

### BIBFRAME: preserve both statement and linked identities

The official [BIBFRAME 2.0 model overview](https://www.loc.gov/bibframe/docs/bibframe2-model.html)
places publisher, place, and date information at the Instance layer. Its
official
[publication, production, distribution, and manufacture examples](https://www.loc.gov/aba/pcc/bibframe/TaskGroups/CSR-PDF/PublicationProductionDistributionManufacture.pdf)
provide the most directly useful local pattern:

- `ProvisionActivity` groups place, agent, and date;
- Publication, Production, Distribution, and Manufacture remain distinguishable;
- a `provisionActivityStatement` may preserve transcribed data;
- linked Place and Agent values can coexist with that transcription.

ToS should adapt this grouping, not copy the BIBFRAME storage model. The
source statement and normalized relations answer different questions and
must be independently inspectable.

### GND: authority identity is a separate assertion

The current [GND Ontology](https://d-nb.info/standards/elementset/gnd) is
version 1.5.0, modified 2025-12-16 and published under CC0. It provides
separate authority-resource classes for corporate bodies and places or
geographic names, and distinguishes associated places such as place of
business and place of manufacture.

GND is especially relevant for historical German publishers and places, but
an external identifier must remain evidence-bearing:

- an imprint string may be retained without a GND match;
- a candidate GND match is not `same_as` until identity evidence is reviewed;
- a present-day successor organization is not automatically identical to the
  historical publishing entity named in an 1880s or 1900s source;
- a historical place name may normalize to a Place while its exact spelling
  remains part of the witness statement.

### EDTF: adopt only the subset actually enforced

The Library of Congress
[Extended Date/Time Format specification](https://www.loc.gov/standards/datetime/)
supports year, month, and day precision plus intervals at Level 0, and adds
uncertain, approximate, unknown, and open endpoints at higher levels.

The current ToS chronology contract enforces a bounded Level-0-like subset.
It must not claim full EDTF conformance. A provision-activity contract should
reuse the existing exact Gregorian date and declared-precision rules first.
Uncertainty or approximation belongs in explicit posture fields until a
larger EDTF subset is deliberately implemented and tested.

### PROV-O: the claim is not the event it describes

The existing ToS provenance route already follows the central distinction in
[W3C PROV-O](https://www.w3.org/TR/prov-o/): an entity or assertion can be
generated by an activity and attributed to an agent. The annotation or
cataloging event that generates a provision claim is not the historical
publication or production event described by the claim.

This boundary must remain machine-checkable. Otherwise a 2026 model-made
record can be mistaken for evidence that the model participated in an 1889
publication event.

## II. Established work that changes the local design

### Beyond 2022: versioned fact groups and provenance

The
[Beyond 2022 knowledge-graph design](https://doi.org/10.1145/3474829)
uses named graphs and separate provenance graphs so that manually and
automatically produced fact groups can be revised without erasing their
history. It models people, organizations, and places as identities rather
than printable strings alone.

For ToS, the important lesson is not that RDF named graphs are mandatory. It
is that one grouped provision assertion needs its own identity, origin,
revision path, and source return. Replacing a literal inside `edition.json`
would lose that history.

### Pattern-based cultural-heritage KG design: n-ary relations are real

The established
[pattern-based ArCo study](https://doi.org/10.3233/SW-200422)
shows why time-indexed roles and typed locations become n-ary situations and
why catalog records themselves need version history. Place, agent, role, and
time cannot always be represented honestly as independent timeless edges.

For ToS this supports one bounded provision-activity claim object. It does
not justify a universal event ontology before the local corpus has enough
real contrasting cases.

### LRMoo development: conceptual and physical production remain distinct

The official LRMoo release history and its event-centered definition codify a
long-established library/museum integration pressure: creation of the
publication specification and production of physical carriers are related
but not identical. This distinction is directly applicable to Nietzsche
editions whose title-page year, printing completion, authorial receipt, and
public release fall on different dates.

## III. Freshest directly relevant work

### CHAD-KG, 2025 preprint / 2026 publication

[CHAD-KG](https://arxiv.org/abs/2505.13276) combines CIDOC CRM, LRMoo,
CRMdig, and controlled vocabularies in an application profile, then
materializes a graph through a reproducible pipeline. Its 2026 publication
does not make RDF the ToS owner format. It strengthens four requirements:

- the source application profile must be explicit;
- digitization paradata is separate from bibliographic identity;
- graph materialization must be reproducible;
- the graph is a consumer of source data, not the only place the facts exist.

### Connecting manuscript knowledge graphs, 2026

The 2026 open-access case study
[Connecting cultural heritage through knowledge graphs](https://doi.org/10.1007/978-981-95-5009-8_36)
reports that two KGs can both use CIDOC CRM and still require difficult
identity resolution, semantic approximation, metadata enrichment, and
provenance preservation.

This is a direct warning against premature `same_as` for Leipzig, C. G.
Naumann, Insel-Verlag, or historical corporate successors. Shared ontology
terms do not prove shared entity identity.

### Freshness conclusion

The 2025-2026 work does not overturn the classical direction. It reinforces
source-owned profiles, explicit identity alignment, event/role distinction,
reproducible projections, and provenance-preserving revision. No fresh paper
found in this pass provides evidence that LLM extraction, ontology alignment,
or a green shape validator may automatically promote a bibliographic fact.

## Architecture comparison

| Variant | Shape | Strength | Failure or cost | Decision |
| --- | --- | --- | --- | --- |
| A. Flat Edition fields | add `publisher`, `place`, `date` literals to `edition.json` | cheap and readable | cannot group roles and date, loses claim identity and revision, conflates transcription with normalized identity | reject as the authority model |
| B. Full local LRMoo/CIDOC event layer | add general Place, Organization, Time-span, Manifestation Creation, Item Production, release, and reproduction records now | expressive and interoperable | large ontology and identity surface before sufficient cases; easy to manufacture false event precision and orphan identities | defer until repeated consumers prove the pressure |
| C. Bounded Edition-owned provision-activity claim | one reified claim grouping typed kind, statement, place, agent, date, evidence, provenance, and review | source-returnable, reversible, BIBFRAME-shaped, graphable, narrow enough for a real experiment | needs a new contract, exact role vocabulary, and guarded identity references | recommend for the first slice |

Variant C is not a compromise placeholder. It is the smallest object that
preserves the real relation while allowing later projection to BIBFRAME,
LRMoo/CIDOC CRM, RDF, or a property graph.

## Recommended contract boundary

The first `provision_activity` object should require:

- `provision_kind`: one of `publication`, `production`, `distribution`, or
  `manufacture`;
- `statement_basis`: `manifestation_transcription`, `item_observation`,
  `authority_record`, or `scholarly_report`;
- `transcribed_statement`: required only when the evidence is a visible or
  encoded source statement; exact spelling and punctuation are preserved;
- `places`: zero or more assertions, each with `role`, `literal_form`, and an
  optional `normalized_place_ref`;
- `agents`: zero or more assertions, each with `role`, `literal_form`, and an
  optional `normalized_agent_ref`;
- `date` or `interval` using the existing Gregorian precision discipline;
- `event_posture`: whether the object records a source statement, reported
  historical activity, or both without claiming their equivalence;
- the normal claim packet fields for evidence, maker, provenance, review,
  visibility, revision, and source-line return.

The first contract should reject:

- `publisher` as an untyped generic role;
- one agent carrying mutually incompatible provision roles in the same
  assertion without separate evidence;
- normalized refs to missing or wrong-type records;
- a transcribed-statement basis without exact source evidence;
- day precision with an invalid Gregorian date;
- a publication-place assertion used as composition place;
- a title-page year silently treated as public-sale date;
- an annotation provenance event used as the historical event identity;
- a direct graph edge that bypasses the claim node.

## Identity admission

Place and organization identities should be added only for exact normalized
refs used by the experiment. They need the same local discipline as current
Agent records:

- persistent ToS record ID;
- preferred and variant labels;
- identity status;
- evidence-bearing external identifiers;
- no-equivalence posture by default;
- source refs and revision history.

A missing authority match must not block the provision claim. In that case,
the literal stays source-returnable and the normalized ref remains absent.

## First bounded experiment

The recommended laboratory slice is two positive Editions plus one negative
control:

1. *Götzen-Dämmerung*, Leipzig, C. G. Naumann, 1889:
   preserve the visible imprint separately from the reported November 1888
   printing, authorial receipt, and January 1889 public-sale chronology.
2. *Ecce homo*, Leipzig, Insel-Verlag, 1908:
   preserve publication provision separately from Nietzsche authorship,
   Raoul Richter's editing/afterword responsibility, Henry van de Velde's
   design responsibility, and the posthumous publication posture.
3. Negative control: attempt to use the shared literal `Leipzig` or an
   Edition label as sufficient evidence for a normalized Place, publisher
   identity, public-release date, or organization successor. Every such
   shortcut must fail.

These cases deliberately reuse one place while changing publisher,
responsibility, chronology, and authorial posture. If the model cannot share
one reviewed Place identity without merging the two provision activities, it
is not ready for wider use.

An optional third positive case, *Also sprach Zarathustra* part I, Chemnitz,
Ernst Schmeitzner, 1883, should be admitted only after the two-case contract
passes. It tests a different place/publisher pair and a staged Work whose
parts must not be collapsed into one manifestation event.

## A/B/C evaluation of the contract, not of philosophical truth

The first experiment should compare three representations over the exact same
two source cases:

- A: current label/notes retrieval;
- B: flat candidate Edition fields in an isolated fixture;
- C: the bounded provision-activity claim object.

The comparison should measure:

- source-return completeness;
- number of unsupported inferences required to answer a query;
- ability to distinguish transcription, normalization, and report;
- ability to distinguish provision roles and date facets;
- exact negative-query behavior;
- human time to inspect the source and understand the answer;
- generated catalog/graph size, rebuild time, and query time.

No prepared validator score is a quality verdict. The final test must include
manual inspection of the exact source statement, raw claim row, catalog row,
graph trace, and negative query output.

## Decision gates before implementation

Implementation is admitted only when all of the following are explicit:

1. the predicate and activity-kind vocabulary cannot collapse publication,
   production, distribution, manufacture, or public release;
2. transcribed literal and normalized identity are separate fields;
3. Place and Organization records have a bounded schema and cannot be
   manufactured from Edition labels alone;
4. date syntax, precision, and Gregorian validity reuse the existing tested
   chronology discipline;
5. provenance distinguishes the 2026 annotation event from the historical
   activity described;
6. Edition refs close exactly over source claims with no orphans;
7. catalog and graph projections return to the exact source line and create no
   direct subject-object truth edge;
8. isolated negative fixtures fail for missing identity, wrong role, false
   date equivalence, stale provenance, and stale generated output;
9. no source payload, transcription, or restricted work is published;
10. the experiment remains model-made and `unreviewed` until an actual
    bibliographic review event occurs.

## Deferred questions

This research does not yet choose:

- a universal Place hierarchy or coordinate model;
- corporate succession and historical organization time spans;
- complete composition chronology;
- all printing, binding, issue, distribution, and sale events;
- RDA or BIBFRAME conformance;
- full EDTF Level 1 or Level 2 support;
- RDF-star, named-graph, or property-graph storage as source authority;
- automatic authority alignment;
- bulk population of all fourteen current Editions.

Those questions should open only after the two-case experiment produces a
real consumer pressure or a concrete failure that the bounded claim cannot
represent.
