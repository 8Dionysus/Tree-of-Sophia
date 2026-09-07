# Semantic interchange registry

This directory owns the stable machine vocabulary used when ToS material is
composed into read-only knowledge lenses. It is an interchange layer over
source-owned meaning, not a universal ontology and not a route into canon.

## Identity and vocabulary

`entity-types.v1.json` gives durable `tos.entity.*` IDs to reusable entity
families. `relation-types.v1.json` gives durable `tos.relation.*` IDs to
relation families and declares their domain, range, directionality,
cardinality posture, evidence posture, and review requirement. Every mapping
also retains the exact source-native `kind_id` or `predicate_id`; the stable
family never erases the authored subtype.

Source mappings may carry localized `labels` for the exact native kind or
predicate. These labels override the family label for display only; explicit
instance display remains stronger. Registry version 3 carries the Russian
predicate vocabulary from `ToS/canon/registries/predicates.csv` verbatim into
both canon and candidate-intake mappings, and distinguishes the eight atlas
metadata kinds. This does not change source status, domain/range, identity or
review requirements. The source CSV remains the owner of its wording; the
access contract test checks crosswalk parity rather than accepting new meaning.

Unknown source vocabulary is represented by the explicit
`tos.entity.unmapped` or `tos.relation.unmapped` fallback. It must never be
silently coerced into the nearest familiar type. A new stable type is added by
extending the registry, declaring its owner and lifecycle, validating the
hierarchy and crosswalk, and bumping `registry_version` when a released
registry changes. Incompatible meaning receives a successor ID and an
explicit `supersedes_*` link rather than reusing an old ID.

## Semantic boundaries

- Agent is a persistent responsibility bearer. Author, translator, editor,
  designer, and other responsibilities are typed relations, not Agent
  subclasses or mutable role fields.
- Work, Expression, Edition, Item, File, and Link remain distinct. A Link is
  an observed access identity, not the object at its URI.
- Place is a persistent geographic identity. A source-navigation Region is a
  browsing partition and is explicitly not a Place.
- Event identity is separate from TemporalAssertion. Dates, intervals,
  precision, calendars, and publication stages remain source-returnable
  assertion values.
- Bibliographic and other evidence-bearing assertions use reified Claim
  topology: subject, predicate family, object, evidence, maker, provenance,
  review, version, and supersession stay inspectable.

Cross-layer predicates are intentionally narrow. `projects` connects carrier
representations that already declare the same persistent ToS ID. `grounded-in`
requires an exact declared source reference. `about` requires a source-owned
topical assertion. `represents` requires an explicit representation claim.
`same-as` is never inferred from names, paths, links, or similar text; it
requires evidence and accepted identity review.

## Projection law

The access backend may normalize, index, filter, traverse, and display these
types, but it must preserve complete public source payloads under the
normalized envelope, keep `source_refs`, expose mapping status, validate
domain and range, and report synthesized display prose as synthesis. Generated
graphs, D1 tables, catalogs, and LensResults are disposable read models. They
cannot accept source, rights, translation, semantic, identity, or canon
claims.

## Executable boundary and text spine

Registry version 2 rejects abstract instances, missing or cyclic supersession
targets, incompatible endpoint types and missing supporting Claim references.
A bibliographic Claim has exactly one subject and one object. Its literal
object does not inherit `claim_ref` as its identity. `same-as` requires resolved
exact-version review and evidence nodes, not an `accepted` string and a URL.
Multiple representations of one persistent ID remain separate; `projects` does
not discard a second representation in one source.

Property descriptors publish type, applicability, inheritance and operators.
`semantics.type_ancestors contains <type-id>` selects a type and descendants.
Comparable time bounds retain precision and declare calendar and year numbering.
Unknown dates/calendars do not gain invented order keys.

Public text-unit and semantic-annotation-v2 packets expose an addressable route
from Work through TextLayer, TextUnit/Anchor, Occurrence/Sign/Concept and Claim
to Evidence/Review. Competing interpretations remain separate. The projector
does not read private text; metadata-only packets omit lexical hashes and
declare content availability. A private lexical workbench is not automatically
a public or accepted annotation layer.

Missing reviews, unresolved source endpoints and synthesized descriptions remain
visible gaps. Broad legacy relation families retain native predicates; mapping
coverage does not prove their philosophical endpoint semantics. Tightening such
source assertions requires source-visible review, not inference from labels.

## Physical artifacts: existing-source adapter

Entity registry version 6 maps the existing `tos.artifact.*` identity to
`tos.entity.artifact`, a physical identity rather than a Work, transcription,
catalog record, reconstruction or digital representation. The native
`artifact-witness.json` v1/v2 records remain authoritative and unchanged; the
adapter does not rewrite them into `tos_corpus_record_v1`.

The catalog's optional `artifacts.jsonl` binds `artifact_id` to `record_id`
without changing its value, and records the exact source schema and canonical
source digest. The first declared custody inventory number is an attributed
navigation label (`label_source_pointer`), not a newly assessed preferred name.
`identity_status=null` explicitly means that these source schemas have no
Corpus identity-assessment field; the artifact's separate native
`authority.review_status` remains intact. Null is not admitted for ordinary
Corpus catalog entries.

The existing graph and access focus/inspection routes expose the full native
record, exact copies of its identity-boundary note and review/visibility
metadata, and the pointers behind those display fields. They do not infer
edges from custody, dates, inventory schemes, reported joins, visual links,
genre or planting refs. Metadata display is not assessed human-form admission;
source language and script of that display remain unknown when undeclared.
Adjacent metadata form sets currently require a Corpus-shaped subject and are
not yet an artifact form-production route.

The corpus-index source-navigation reader uses that same adapter, replacing
its previous ID-only planting placeholder while retaining the exact navigation
node ID and authored planting edges. Both navigation and claim-graph carriers
map to the same artifact type and persistent entity ID. Their existing
`projects` relation records representation, not historical `same_as`; no focus
priority or UI-side label heuristic is changed.
Relation registry version 6 explicitly includes Artifact in the range of
`grounds-source-backlog-anchor`, matching the existing artifact alternative in
`philosophy-source-planting.schema.json`; unrelated types remain outside that
range. This structural planting link is not a carrier/text or historical claim.
The same registry version maps the three existing historical families in
source-navigation as well as source-claims. The navigation reader validates
their exact source schema, ID, family, digest and visibility before using the
same adjacent-form materializer. Default focus therefore does not lose the
historical type or source-bound forms when both carriers become available.

Unknown schema versions, nonpublic metadata, source/catalog mapping drift,
duplicate IDs, symlink paths and records above 1 MiB are refused. Refusal does
not delete or silently normalize source. The catalog and graph builders can
regenerate this disposable adapter; rolling back the reader leaves the
physical-source records and any newer research untouched. The native source
validator still owns artifact/rights/provenance reference closure. This adapter
neither downloads media nor admits its use, and adds no write command for
artifacts. Evidence-bearing document/carrier relationships, artifact growth,
and fully assessed multilingual forms are still required for the complete
profile.

## Historical situations: source profile

Registry version 4 introduces `historical-event`, `historical-process`, and
`historical-state` under the abstract `historical-situation`. These are
source-described historical identities, not replacements for the authored
semantic `event`/`state`, the claim-scoped `provision-activity`, or a provenance
event. Duration alone never changes an event into a process or state.

`tos_historical_record_v1` reuses the source metadata properties of the
unchanged corpus-record contract and carries these kinds in adjacent
`historical-event.json`, `historical-process.json`, or `historical-state.json`
records under the source-witness home, normally `history/<subject>/`. Each
uses its own `tos.historical-<kind>.*` prefix and explicit visibility. Correcting
wording changes `record_version`, not the referent ID. Changing the kind to
another historical family requires an explicit identity transition; no
automatic reclassification, merge, split, or causal inference is provided.

`historical-claims.jsonl` uses `tos_historical_claim_v1`, reusing Claim fields with
`claim_type=relation`, `assertion_layer=scholarly_report`, separate evidence,
maker and provenance. Current decisions bind the exact record through the
existing assessment journal and optional `assessment_refs`; a reference does
not itself grant admission. There is no embedded human-signature requirement.
The compatibility `review_status=unreviewed` describes the initial source
record, not the current scoped admission, which the historical graph adapter
does not yet materialize. Four registered predicates are executable (relation
registry version 5 adds historical dating):

| Predicate | Domain → range | Qualification |
| --- | --- | --- |
| `historical_participant` | historical situation → Agent or Organization | `qualifiers.participation_role` is required source wording, not a new Agent type or a reviewed role-registry ID |
| `historical_place` | historical situation → Place | location does not imply political belonging, residence or influence |
| `historical_work` | historical situation → Work | topical association does not imply creation, reading, publication, reception or influence |
| `historical_dating` | historical situation → TemporalAssertion value | historical time, not witness creation, data capture or record revision |

A dating value has an explicit `kind` (`date-assertion`, `interval-assertion`,
`relative-order`, or `unknown-date`), `role=historical-time`, `calendar`,
`year_numbering`, `certainty`, and exact `source_wording` with its language
(which may be unknown). A date uses `value`; an interval uses `interval.start`
and/or `interval.end`; a relative date uses `relative.relation` (before, after,
during, overlaps) and a resolved historical `anchor_ref`. Unknown dates have
none of those absolute/relative values. Open bounds are absence, not infinity.
Additional uninterpreted fields live in `extensions` and survive inspection.

For example, the **synthetic, not historical evidence** value
`{"kind":"date-assertion","role":"historical-time","calendar":null,"year_numbering":null,"certainty":"approximate","value":"1883","source_wording":{"text":"Около 1883 года — тест","language":"ru"}}`
remains searchable and inspectable, but has no invented numerical bounds.
The enclosing Claim still supplies the subject, evidence, maker, exact version,
provenance, epistemic state, alternatives and assessment route. An exact date
value can belong to a disputed or negated Claim; comparison never accepts it.
Competing datings use separate Claim IDs, not overwritten event metadata.

Relative anchor edges run from the dating Claim to the referenced historical
situation as `has_historical_date_anchor`. They allow forward and reverse focus
through the ordinary Claim topology, retain the exact relative qualifier, and
never infer an absolute date, transitive closure, causality or true chronology.
The source wording becomes the temporal value's readable title and description;
assertion contexts remain mandatory in compact delivery. This is source-copy
display, not translation, assessment, or a complete seven-role Forms adapter.

Comparable structured values require an explicit Gregorian/proleptic-Gregorian
calendar, astronomical year numbering, exact precision/certainty, valid date
parts and two ordered interval bounds. Approximate, uncertain, unknown,
unsupported-calendar/numbering, conflicting nested contexts and open intervals
retain their complete raw values and issue codes without comparison keys.
The reader performs no calendar conversion or uncertainty expansion. Bare
legacy `YYYY[-MM[-DD]]` string values keep their documented proleptic-Gregorian,
astronomical shorthand; this compatibility rule is not applied to structured
objects with missing calendar or numbering. This narrows old numeric search
results where the previous reader invented such context, without changing
source bytes. Filter the ordinary `semantics.time.sort_start/sort_end` fields
for comparable proposed date ranges; inspect excluded raw values and their
issues instead of interpreting exclusion as absence in history.

The catalog emits additional family files only when source records exist and
names each new entry's `source_schema_ref` plus `extension_schema_refs` in the
manifest. Its old singular schema refs still describe the legacy families.
The earlier corpus and Claim schema bytes remain unchanged so historical
provenance input digests are not rewritten to describe a later contract.
The existing bibliographic Claim carrier includes the `historical` layer for
this bounded extension, keeps every historical identity addressable even with
no claims, and retains complete source records and assertion qualifiers. It
validates the source schema and the current registry's inherited domain/range;
it does not establish that an event happened. Private historical metadata is
refused, not silently projected or dropped. Other historical predicates are
not admitted by this carrier until their owner contract is implemented.

The ordinary access catalog, focus, type-ancestry filters, Claim inspection,
and adjacent metadata-form reader consume these records without a new UI.
The source-form command adapter can prepare/create/revise those adjacent forms;
it still cannot create or change the historical subject itself. The profile
adds no automatic dates: historical dating Claims, witness dating, capture
provenance and Claim revisions remain separate. As-of knowledge reconstruction,
calendar conversion, uncertainty-aware interval algebra, the full role,
biography, causal and reception grammar,
source-visible historical assessment and actual UI acceptance remain open.

Validation: `python -m unittest discover -s tests -p test_source_witness_bibliographic_graph.py`, then the existing knowledge
contract test and source-witness catalog/graph `--check` commands. The temporary
historical associations in these tests are synthetic, even where their
Agent/Place/Work endpoints are unchanged real corpus records. They are never
historical evidence. No corpus migration is implied.

This is additive for existing records, not transparent to an old closed-schema
reader presented with a new kind or schema family. Upgrade the catalog/schema/graph consumer
together before publishing historical records. Optional catalog files are
selected by the current manifest, so a leftover file from an earlier snapshot
does not restore a removed family. A reader rollback does not erase source
records, historical judgments, or their separate assessment history.
