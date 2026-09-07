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

## Declared source-metadata profiles

Entity registry version 7 gives the three historical identities an executable
`source_record_profile`. This is a source-to-reader contract on the existing
type entry, not a second type registry and not a claim that all source material
has one shape. The first supported reader is `corpus-metadata-v1`; native
physical-artifact records continue through their separate adapter below.

A profile declares its revision, native record kind, persistent ID prefix,
source basename, catalog filename, graph layer, and an explicit list of source
schema versions with local schema refs and dependencies. Several schema
versions may describe the same kind without changing its referent identity.
Each record selects an understood route by its exact `schema_version`; no
nearest-version guess, remote schema retrieval, code selection or dynamic
execution is allowed. The current registry schema owns the descriptor shape.
Names, notes, language declarations, source refs, external identifiers,
identity posture and record version reuse the existing Corpus metadata
properties. The profile's own schema adds its source-specific constraints;
it cannot weaken those common metadata fields. No source is rewritten into a
new schema merely to make it readable.

The catalog, claim-graph identity reader, source-navigation reader and source
validator use `scripts/source_record_profiles.py`. A new metadata kind in this
reader is added as a source schema and a profile on its concrete identity type,
with explicit mappings for both `source-claims` and `source-navigation`.
It does not require another Python kind branch. The ordinary knowledge
catalog and `tos.knowledge.contracts` expose the declaration, including when
there are no instances. Catalog entry fields and their source digest are
checked against the exact source record; all public fields, including
uninterpreted `extensions`, survive inspection. Additional catalog families
are admitted only by a declared profile, never by a permissive catalog schema
alone. Missing or unrecognized profiles fail closed without deleting source.

Adjacent human-form sets reuse the existing bounded metadata materializer.
Complete source-copy names and notes retain language, script, context, exact
source version, provenance and visible quality state. This reader does not
create translations, assess wording, accept historical claims or infer edges
from a metadata field. An addressable source record may have no Claims.
General semantic predicates, roles, new production modes, growth permissions
and admission are not granted by a metadata profile; their own contracts must
be implemented. Source-write commands remain separately delegated: the
[source-owner command](../../../mechanics/growth-cycle/parts/branch-growth-cycle/README.md#declared-profile-subject-creation)
`source.create` now creates an initial public-metadata subject and its forms
from an independently selected profile configuration. It reuses the exact
reader/schema route, not a new per-type Python branch, and grants neither
claims nor admission. Historical creation with initial claims retains its
narrower contract. The separately delegated `record.revise` profile
configuration corrects the same declared metadata kinds through the existing
source-revision transaction. It preserves stable identity, the exact prior
flat source package and forms, and validates the current registry/schema
dependencies before publication. It cannot change type, source schema version,
identity status, rights or admission. Native non-profile records and Claims
retain separate write routes.

The reader rejects duplicate kind/prefix/basename/catalog ownership, mappings
owned by another type, abstract identity instances, reserved native-adapter
collisions, unknown reader modes and schema versions, nonpublic visibility,
duplicate JSON keys, nonfinite numbers, symlink paths, metadata above 1 MiB,
undeclared schema dependencies and catalog/source drift. Schema resources are
local and exact; consumed registry/schema bytes supply the graph dependency
digests. Neither supported metadata nor a schema-valid record is accepted
knowledge or permission to publish source contents.

When a previous registry is supplied to the semantic validator, a changed
profile must advance `profile_version`, preserve all earlier schema routes,
and not repurpose its kind or ID prefix. Registry changes also advance the
registry version. Compatible source evolution adds a schema route; an
incompatible identity meaning needs an explicit successor and reference
migration. A catalog path move likewise needs coordinated reference migration;
it never changes the identity merely because a path changed.

The first migration moves the three historical readers' hard-coded schema and
catalog choices into their owner type entries. Existing historical and
artifact source bytes are unchanged. An old reader must be upgraded together
with its registry/catalog schema before consuming a new profile. Reader
rollback does not erase source records, judgments, creation receipts, retained
record history or human-form predecessors.

This closes a metadata extension seam, not the complete Foundation profile
grammar. The synthetic `fixture-document` in the reader test is not an actual
letter, historical evidence, a material artifact or source admission.

## Reasoning objects, contextual roles and addressed objections

Entity registry version 10 adds Thesis, Argument, InferenceStep and Objection
as specific SemanticObjects, without reclassifying existing canon nodes.
They use the same `semantic-metadata-v1` source/create/read/revise/form route.
`thought-description-record.schema.json` composes the shared source metadata
and semantic scope constraints; it adds a required `semantic_content` with
its own language/script and the following substantive fields:

| Kind | Required account | Boundary |
| --- | --- | --- |
| Thesis / Тезис | `proposition`, `assertion_force` | a proposition under examination, not its research Claim, truth or endorsement |
| Argument / Аргумент | `reconstruction_note`, `coverage` | a reasoning reconstruction, possibly partial; not merely a label or a proof of soundness |
| InferenceStep / Шаг вывода | `transition_account`, `reasoning_mode` | a described inferential transition, not executable inference code or historical chronology |
| Objection / Возражение | `challenge_account` | a reasoned challenge, not automatic negation of its target or proof that it succeeds |

`coverage` is `partial`, `claimed_complete` or `unknown`: even claimed
completeness remains a research posture, not a serializer verdict. Force and
reasoning mode preserve the source's wording without requiring one logical
school's taxonomy. Unknown nested fields remain source data, never commands.
Descriptions need a source-visible assessment for substantive adequacy.

Names and notes carry the complete semantic scope **and content** as mandatory
reading context. A hypothetical premise must not appear as an unconditional
fact after a short label is selected. The seven named content properties in
the registry support semantic-ID queries through the ordinary catalog and
snapshot-bound property filter. They do not require a kind-specific reader.
The separately delegated correction route may update `semantic_content` only
where the source schema and grant allow it, retaining exact previous bytes
and rebuilding forms; it cannot change the referent criterion or admission.
A changed account of the *same* referent is distinct from historical thought
change or a new referent, which need their own subject and grounded relations.

Relation registry version 9 adds the following reified source Claims:

| Predicate | Subject → object | Meaning and limit |
| --- | --- | --- |
| `conception_has_thesis` | Conception → Thesis | scoped membership, not universal endorsement or truth |
| `argument_for_thesis` | Argument → Thesis | offered support, not a successful proof |
| `argument_has_step` | Argument → InferenceStep | `qualifiers.step_position` is a required nonnegative integer position in this reconstruction, not an absolute date |
| `step_has_premise` | InferenceStep → Thesis | premise role in this transition, possibly granted only hypothetically |
| `step_has_conclusion` | InferenceStep → Thesis | proposed conclusion role here, not established logical consequence |
| `objection_to_thesis` | Objection → Thesis | challenge to this proposition; use this to challenge a premise as content |
| `objection_to_step` | Objection → InferenceStep | challenge to the transition, distinct from challenging its premise |
| `objection_to_argument` | Objection → Argument | challenge to the specified reasoning structure as a whole |
| `objection_to_conception` | Objection → Conception | challenge to specified commitments of a situated account |
| `objection_developed_by_argument` | Objection → Argument | reasoning develops the challenge; the referents stay distinct |
| `thought_expressed_in` | Thesis/Argument/InferenceStep/Objection → Work/Expression/Document | interpreted expression, not an exact occurrence anchor, authorship or literal quotation |
| `thought_attributed_to` | Thesis/Argument/InferenceStep/Objection → Agent/Organization | scoped attribution, not the research Claim maker or exclusive authorship |

Premise and conclusion are relational roles, not new universal entity classes.
One thesis may fill different roles in different steps. No global acyclicity,
one-author rule or universal cardinality is imposed. A partially reconstructed
argument may lack known steps or premises; unknown content is not silently
invented to complete a graph. Rival step positions live in separate qualified
Claims rather than overwriting one another. All twelve predicates have
specific endpoints, both reading directions, mandatory statement/basis and
ordinary evidence, provenance, uncertainty and separate assessment. They add
no unconditional edges and infer neither logical validity nor historical truth.

`claims.create` applies a bounded reasoning batch atomically through the
existing source-owner writer. A malformed or unauthorized member prevents the
whole batch; replay returns the same receipt. Endpoint versions and source
contracts are bound at preparation. The read-only access boundary stays intact.
Synthetic tests cover these boundaries and the connected argument/objection
route. The [JGB freedom research](../../review-ledger/2026-09-07-jgb-freedom-reader-review.md)
also traverses the actual corpus reader; exact occurrence links and substantive
assessment remain required Foundation work.

## Inquiry, stances and conceptual differentiation

Entity registry version 11 adds eight source-described profiles through the
same semantic metadata route. `thought-topic-record.schema.json` reuses the
common identity, source, language, substantive-notes and continuity rules.
Each profile adds the following mandatory semantic content, with its own
language/script; all of that content accompanies short human forms.

| Profile | Required content | Distinction retained |
| --- | --- | --- |
| Aspect / Аспект | `perspective_account` | a dimension of examination, not a duplicate concept or conception |
| PhilosophicalCategory / Философская категория | `category_account` | an organizing philosophical category, not a technical datatype or a commitment imposed on the core |
| Problem / Проблема | `problem_statement`, `inquiry_stakes` | an inquiry's difficulty and stakes, not its question wording or answer |
| ProblemFamily / Семейство проблем | `grouping_basis` | an explicit grouping of distinguishable problems, not one problem under many names |
| Question / Вопрос | `question_text`, `presupposition_account` | an interrogative formulation whose presuppositions need not be accepted |
| Position / Позиция | `stance_account` | a stance with commitments and limits, not its holder or a record status |
| Distinction / Различение | `differentiation_criterion` | a differentiation in a stated respect, not necessarily an exhaustive partition |
| Opposition / Оппозиция | `differentiation_criterion`, `opposition_basis` | a subtype of Distinction; opposition is not automatically contradiction, succession or social conflict |

The ten content properties are discoverable by `tos.property.*` IDs in
the ordinary catalog and execute in the same snapshot-bound node/path filter.
Opposition inherits the Distinction differentiation property; it does not need
a second ID for the same criterion merely because it is a subtype.
This introduces no new reader branch, UI screen or kind-specific writer.
`source.create` and separately authorized `record.revise` use the selected
profile schema, exact dependencies, predecessor retention and the existing
transaction boundaries. A revision can correct the account but cannot change
the referent criterion, type, identity, source schema or admission. Unknown
nested semantic fields survive as uninterpreted source data, not executable
instructions. Nonempty wording is only a structural requirement, not a
substantive adequacy assessment.

Relation registry version 10 adds ten specific, nontransitive reified Claims:

| Predicate | Subject → object | Scope |
| --- | --- | --- |
| `problem_family_member` | ProblemFamily → Problem | explicit grouping basis, not identity or exhaustive coverage |
| `problem_has_question` | Problem → Question | articulation, not acceptance of presuppositions |
| `question_proposed_answer` | Question → Thesis | proposed answer, not established truth |
| `position_has_thesis` | Position → Thesis | specified commitment, with separate holder attribution |
| `position_addresses_problem` | Position → Problem | engagement, not successful resolution |
| `conception_has_aspect` | Conception → Aspect | perspective on this situated account |
| `aspect_of_concept` | Aspect → CrosscuttingConcept | the perspective's crosscutting subject |
| `category_organizes_concept` | PhilosophicalCategory → CrosscuttingConcept | philosophical organization, not runtime classification |
| `distinction_first_term` | Distinction (including Opposition) → CrosscuttingConcept/Conception/PhilosophicalCategory/Thesis/Position/Aspect | first term is a contextual role, not temporal priority or superiority |
| `distinction_second_term` | same domain/range | second term is a contextual role, not temporal succession or inferiority |

`thought_expressed_in` and `thought_attributed_to` also admit these eight
profiles with the same limited meanings described above. Attribution to an
agent is still distinct from the maker of the researcher Claim and does not
establish endorsement or exclusive authorship. All relationships retain source,
statement language, explicit relation basis, uncertainty, provenance, review
posture and both reading directions. Partial and competing term/membership
Claims can coexist. No global two-term completeness, exhaustive family tree,
one-holder cardinality or graph-wide acyclicity is inferred. The type hierarchy
itself remains acyclic. Substantive comparison must examine the term Claims
and differentiation criterion together, not treat any drawn line as a proof.

Existing canonical and atlas Concept, Method, institution and category-like
records are not silently retyped into these source profiles. Their source
identity and existing mappings remain authoritative until an explicit mapping
or revision is reviewed. This section defines an executable extension, not a
claim that the whole Foundation thought domain or its real assessment is done.

## Methods, hypothetical inquiry, imagery and valuation

Entity registry version 12 adds ten source-described profiles through the
same semantic metadata reader and source-owner commands. The shared
`semanticContentFields` contract owns the language/script declaration for
reasoning, inquiry and practice content. Existing record shapes are unchanged;
`thought-practice-record.schema.json` adds only the following requirements.

| Profile | Required content | Boundary |
| --- | --- | --- |
| ThoughtMethod / Метод мышления | `method_account`, `applicability_conditions` | a described inquiry method, not executable code or the existing atlas Method category |
| ThoughtOperation / Операция мышления | `operation_account`, `prerequisites` | what is done conceptually, not a callable source operation or authority grant |
| ThoughtMove / Ход мысли | `movement_account`, `context_requirement` | a described reframing or movement, not a record correction or automatic historical succession |
| ThoughtExperiment / Мысленный эксперимент | `scenario_account`, `assumptions`, `assumption_coverage`, `examined_consequence` | a hypothetical inquiry, not an actual event or accepted consequence |
| ThoughtImage / Образ мысли | `image_account`, `image_mode` | an imaginative presentation, not a raster file, physical artifact or eyewitness observation |
| RhetoricalFigure / Риторическая фигура | `figure_account` | a described expressive arrangement, not an atlas historical figure |
| Metaphor / Метафора | inherited `figure_account`, `source_domain`, `target_domain`, `mapping_basis` | a subtype of RhetoricalFigure; proposed transfer, not literal identity |
| Value / Ценность | `value_account`, `valuation_context` | a source-described evaluative criterion, not a scalar value or an adopted ToS norm |
| Ideal / Идеал | `ideal_account`, `realization_posture` | a normative or proposed model, not proof of an actual bearer |
| OntologicalCommitment / Онтологическое обязательство | `commitment_account`, `commitment_force` | an account's scoped commitment, possibly conditional, not an ontology imposed on the core |

Conditions, prerequisites and assumptions are string arrays: empty means none
recorded, not proof that none exist. Assumption coverage is `explicit_only`,
`reconstructed_partial`, `claimed_complete` or `unknown`. Realization posture
is `normative_model`, `proposed_realization`, `claimed_realized` or `unknown`.
Neither a completeness claim nor a realization claim verifies itself. The
remaining account fields retain source-described wording without mandating one
philosophical or aesthetic taxonomy. Values need not become separate entities;
these profiles are for referents whose independent identity is useful.

All 22 added content properties are discoverable and executable by semantic
property ID, including array membership filters. Metaphor inherits the same
RhetoricalFigure account property, not a renamed duplicate. Short human forms
carry the complete scope and content; conditions and normative posture are not
optional technical details. The ordinary create/correct/read/form route retains
unknown nested fields, exact predecessor bytes and one subject ID. Source
content cannot supply a command, choose a reader implementation or grant itself
assessment/admission powers. Schema validity does not assess descriptive quality.

Relation registry version 11 adds thirteen specific, nontransitive reified
Claims with language, statement, grounds, uncertainty and separate assessment:

| Predicate | Subject → object | Scope |
| --- | --- | --- |
| `method_uses_operation` | ThoughtMethod → ThoughtOperation | conceptual use, not execution |
| `move_uses_operation` | ThoughtMove → ThoughtOperation | operation within this movement, not identity |
| `experiment_uses_method` | ThoughtExperiment → ThoughtMethod | method used by this hypothetical inquiry |
| `experiment_assumes_thesis` | ThoughtExperiment → Thesis | granted for the trial, not asserted true |
| `experiment_tests_thesis` | ThoughtExperiment → Thesis | proposition under examination, not a successful result |
| `experiment_adopts_commitment` | ThoughtExperiment → OntologicalCommitment | adoption within the scenario and its conditions, not actuality or core law |
| `conception_has_commitment` | Conception → OntologicalCommitment | commitment with the account's scope and force |
| `thought_uses_image` | Conception/Argument/Thesis/ThoughtExperiment/Position/Ideal → ThoughtImage | expressive use, not evidential proof |
| `thought_uses_figure` | same domain → RhetoricalFigure, including Metaphor | interpreted expressive arrangement, not literal identity |
| `ideal_exemplifies_value` | Ideal → Value | normative exemplification, not an actual person |
| `position_affirms_value` | Position → Value | scoped valuation, not ToS endorsement |
| `method_guided_by_value` | ThoughtMethod → Value | a methodological norm, not a runtime budget |
| `image_presents_conception` | ThoughtImage → Conception | may present a critical target; endorsement is not inferred |

Each has an explicit inverse reading. `thought_expressed_in` and
`thought_attributed_to` include the ten new kinds without strengthening their
existing meanings. Partial reconstructions need not fabricate every assumption,
operation or bearer to validate. Competing interpretation Claims remain possible.
No global experiment completeness, one-figure taxonomy or historical order is
imposed. Existing atlas Method and Figure entries and canon Analogy/Principle
nodes retain their original identities, types and owner routes. A new source
profile adds data and contracts, not a per-kind Python branch or a UI screen.

## Concepts, situated conceptions and transformations

Entity registry version 9 adds `CrosscuttingConcept` as a subtype of the
existing broad Concept family, and Conception as a distinct SemanticObject.
Existing canon and philosophy Concept nodes keep their IDs, mappings and
scoped meanings. For example, `tos.concept.becoming` remains the authored
Zarathustra-prologue interpretation; it does not become a universal account
of becoming. No projection infers equivalence with a new crosscutting subject.

`semantic-metadata-v1` explicitly reads concrete semantic-family descriptions
through the same metadata pipeline. It does not coerce them into the identity
family used for bibliographic or historical subjects. The exact
`semantic-description-record` schema requires substantive notes, declared
wording languages and `semantic_scope`: a scope note and a referent continuity
criterion with their own language/script. Those fields are source-described
research commitments, not accepted universal definitions. Names alone are not
a complete description. The mechanical check rejects blank descriptions but
cannot establish the adequacy of a criterion or its philosophical content.

Source-near descriptions live in
`ToS/source-witnesses/semantic-descriptions/<stable-subject>/`. They describe
the source-visible subject; interpretation, membership and comparison are
separate source Claims, not a new canon or a replacement for philosophy and
candidate-intake authoring. This route cannot ingest arbitrary research prose
as accepted witness. Canon, admission and source interpretation retain their
existing review and assessment owners. Initial records are provisional under
the separately delegated `source.create`; mere reading grants no permission.

Human-form names and notes carry the complete `semantic_scope` as mandatory
context, alongside identity posture and wording language. Changing that scope
stales exact-version forms and prepared dependent Claims. Source correction
uses `record.revise` and retains prior bytes and form history. It changes the
description version, not the conception's identity. The ordinary correction
route cannot change the scope/identity criterion, kind, ID or admission. A
different referent needs a separately created subject and an explicit grounded
transition; it must not be smuggled in as a corrected note. Correctness of a
same-referent prose correction remains a content-assessment question.

Relation registry version 8 declares these grounded semantic predicates:

| Predicate | Subject → object | Required distinction |
| --- | --- | --- |
| `conception_of` | Conception → CrosscuttingConcept | explain membership through the declared continuity criterion, not a shared name |
| `conception_attributed_to` | Conception → Agent/Organization | attributed thinker or collective is not the researcher making the attribution |
| `conception_expressed_in` | Conception → Work/Expression/Document | interpreted expression is not authorship, endorsement or complete textual coverage |
| `conception_redefines` | Conception → Conception | changed definition, with retained and changed features |
| `conception_rejects` | Conception → Conception | rejection of specified commitments, not necessarily the entire concept |
| `conception_narrows` / `conception_expands` | Conception → Conception | a specified comparison dimension and restricted or extended scope |
| `conception_secularizes` | Conception → Conception | specified theological commitments reworked in a non-theological register, not an automatic progress claim |
| `conception_psychologizes` | Conception → Conception | mental-process explanation, not diagnosis of a person |
| `conception_politicizes` | Conception → Conception | specified political reworking, not mere historical context |
| `conception_inverts` | Conception → Conception | a specified ordering, valuation or explanatory direction reversed |

The transforming conception is the subject; the conception it reworks is the
object. Each predicate has forward/inverse Russian and English labels, a
concrete domain/range and an explicit definition. None is transitive. There
is no graph-wide acyclicity rule or one-conception/one-author restriction.
These relations do not imply reading, direct influence, historical priority
or a change in the general ontology's own commitments.

`semantic-relation-v1` requires at least one specific semantic endpoint and
allows the relation's declared specific identity endpoints. It cannot replace
the old identity reader silently or use Thing/Identity/SemanticObject fallback
roots. Its exact source schema requires a full statement, explicit wording
language/script and `relation_basis`, as well as ordinary evidence, maker,
provenance, uncertainty, counterevidence and separate assessment. Basis quality
is not proved by a nonempty string. Negated, disputed and competing Claims
remain distinct and source-returnable through the ordinary catalog, both
graph readers, focus, inspection, compact Claim paths and `claims.create`.
All source qualifications remain available; no unconditional shortcut edge
or new assessment is synthesized. Unknown extension fields retain their bytes
and do not select executable behavior.

The common pipeline supports new concrete semantic metadata and relation
profiles as registry/schema data, within these reader modes and independent
write/assessment permissions. Source-mode changes require an explicit
successor, not a higher version that silently retypes old records.
The [decision rationale](../../../docs/decisions/TOS-D-0053-source-described-conceptions.md)
explains the additive subtype and source/review boundary. These contracts and
synthetic tests do not alone complete the real concept-history route, precise
occurrence/argument/objection grammar, or substantive assessment of examples.

## Social bodies and source-attributed relationships

Entity registry version 13 adds `SocialGroup` and `InstitutionalBody` under
the existing collective `Organization` identity family, and `Community`
under `SocialGroup`. Group membership is not mere classification by a shared
attribute. Community adds continuing shared practice or belonging; an
institutional body has organized roles and continuity and is not its building.
These are historical research referents, not canon nodes or atlas categories.
Existing `Tradition`, `SchoolTradition` and `Institution` navigation types and
their IDs retain their meanings. Intellectual school, tradition and movement
profiles use the distinct formation contract below; none is implemented by
renaming a social group.

`social-body-record` composes the common metadata and description fields.
Its `semantic_scope` field names the scope and continuity criterion of the
research description; reuse of that field does not make the subject a
`SemanticObject`. Group account and membership boundary are inherited by
Community, which additionally requires a shared-practice account. Institutional
description requires its organized-role account. Names and notes alone do not
replace these fields. Descriptive quality and identity remain assessable,
not proved by nonempty strings. Ordinary `source.create`, `record.revise`,
forms, catalog, both graph carriers and semantic property filters use the
existing `corpus-metadata-v1` profile reader, without per-kind Python dispatch.
Ancestry-aware queries retain one subject ID across the subtype and its bases.
Full scope/content is mandatory human-form context; no source assertion or
membership is inferred from the description or generated carrier.

The source-navigation adapter also carries adjacent human forms for the eight
native Corpus families, matching the source-claims carrier. It validates the
native source schema, typed identity and catalog digest before binding forms.
Source changes without matching catalog refresh fail; refreshed source with old
forms exposes their stale state rather than dropping them or reusing wording.
Native Link and Artifact formats do not acquire a Corpus form adapter through
this change. Default focus therefore need not lose a person's existing forms
merely because it selects the navigation carrier.

Relation registry version 12 adds eight `identity-relation-v1` predicates:

| Predicate | Subject → object | What is not inferred |
| --- | --- | --- |
| `social_member_of` | Agent/Organization → Organization | permanent membership, unanimous belief, membership in a similarly named school |
| `learned_from` | Agent → Agent | this teacher relationship from reading a text alone; causal influence |
| `studied_at` | Agent → InstitutionalBody | graduation, qualification, residence |
| `taught_at` | Agent → InstitutionalBody | a particular employment title, discipline or institutional endorsement |
| `collaborated_with` | Agent/Organization ↔ Agent/Organization | collaboration from co-presence or equal responsibility |
| `corresponded_with` | Agent/Organization ↔ Agent/Organization | a whole exchange from one unsent/addressed letter, delivery or reading |
| `friendship_with` | Agent ↔ Agent | agreement, influence or reciprocal self-description |
| `conflicted_with` | Agent/Organization ↔ Agent/Organization | equal fault or permanent opposition on all issues |

The shared `social-relation-claim` schema requires the attributed statement,
wording language/script, relation basis, social scope and historical time-scope
note. Unknown bounds must be stated explicitly. That note is preserved prose,
not an automatically normalized or sortable temporal assertion. Symmetry means
the described relation connects both parties, not that its source perspectives
or responsibilities are interchangeable. No predicate is transitive; none
creates a second reverse Claim, an influence edge or global membership closure.
Negation, dispute, counterevidence and source qualifications stay with each
Claim. Initial records/Claims and forms grant no assessment or admission.
Correction retains earlier source versions; changing the referent, identity
criterion, type or permission remains outside ordinary description correction.
Reader rollback does not remove new source records or their creation history.

## Intellectual formations: schools, traditions and movements

Entity registry v14 adds the abstract identity family `IntellectualFormation`
and three source-described profiles: `IntellectualSchool`,
`IntellectualTradition`, and `IntellectualMovement`. A school has a specified
teaching/inquiry lineage; a tradition has historical transmission through
reinterpretation and discontinuities; a movement has a shared historical
direction or undertaking. These are not individual thought moves, doctrine
versions, automatically closed social groups, buildings or atlas routes.
Existing Tradition/SchoolTradition navigation IDs retain their meanings.

`intellectual-formation-record` composes common source metadata, description
scope and content-language fields. All profiles require a formation account
and their own lineage, transmission or orientation account. The common
`formation-account`, `formation-scope-note` and `formation-identity-criterion`
properties inherit through the abstract family. Each profile's specific
content property is queryable by semantic ID. Source-copy forms carry the
complete declared scope and content, not merely the label. A nonempty account
does not certify research quality.

The existing corpus metadata reader supplies `source.create`, correction,
forms, catalog and both graph carriers; no new executable or per-kind reader
branch is selected by these declarations. Correcting wording retains subject
identity and exact predecessor bytes. Changing its continuity criterion,
referent or kind is not an ordinary description correction.

Relation registry v13 adds four non-transitive reified predicates:

| Predicate | Subject → object | Required distinction |
| --- | --- | --- |
| `intellectually_associated_with` | Agent/Organization → IntellectualFormation | source-attributed association, not social membership, universal endorsement or causality |
| `school_in_tradition` | IntellectualSchool → IntellectualTradition | grounded historical placement, not shared identity or exhaustive classification |
| `movement_reworks_tradition` | IntellectualMovement → IntellectualTradition | specified reworking, not mere resemblance or an unchanged inherited doctrine |
| `formation_articulated_in` | IntellectualFormation → IntellectualObject | specified articulation, not the view of every participant or a physical carrier |

All require a statement with wording language/script, relation basis,
intellectual scope and historical time note. Scholar reports and research
interpretations remain distinct assertion layers. The scope is queryable as
`claim-formation-scope`; existing Claim basis and time-note properties are
reused. Direction and reverse reading remain explicit without creating another
Claim. Unknown historical limits remain wording, not normalized date keys.
These profiles do not supply substantive assessment, admission, canon,
publication or access rights. Rolling back the derived reader leaves their
authored records and histories intact.

## Documents and declared source claims

Entity registry version 8 adds Document under the broader IntellectualObject
root and Letter under Document. Neither inherits Work or Artifact. Their
descriptions, versions and source-copy forms use `source-metadata-record` and
`document-record` schemas through the existing profile reader and separately
delegated `source.create`. Correcting description does not change the referent.
Unknown participants are possible; language, genre and a missing dispatch
claim do not create or destroy a Letter subtype. Native manuscript carriers
keep the Artifact adapter.

Relation registry version 7 declares `source_claim_profile` on concrete
evidence-bearing relations. One `source-claims.jsonl` source stream format
serves these profiles. `identity-relation-v1` reads an exact source schema
route plus the common source-claim record contract, enforces the relation's
specific domain/range with type ancestry, and retains the full source Claim.
The catalog, graph, ordinary semantic catalog and inspection expose the same
declaration and source ref. A new predicate in this reader is data in the
relation registry and its source schema, not another Python predicate branch.

| Predicate | Subject → object | Distinction retained |
| --- | --- | --- |
| `correspondence_sender` | Letter → Agent/Organization | sender is not automatically author, courier or copyist |
| `correspondence_addressee` | Letter → Agent/Organization | intended recipient is not proof of delivery, reading or agreement |
| `document_carried_by` | Document → Artifact | document is not its carrier; copies, custody and textual equivalence need their own claims |
| `historical_document` | HistoricalSituation → Document | association with a historical reconstruction is not causal influence |
| `document_concerns_work` | Document → Work | source-attributed identification need not be a literal mention in the document |
| `authored_by` | Work/Document → Agent | authorship stays distinct from sending, possession or intended readership |

The existing authorship relation is extended to Documents without retyping
the seven existing Works, changing their source claims, or inventing a second
author relation. Its inverse Russian wording now covers both Works and
Documents. Competing attributions are separate Claims; the number of authors
is not limited by the one-subject/one-object structure of one Claim.

Shared source-claim metadata preserves evidence, counterevidence, maker,
provenance, confidence, exact assessment refs, alternatives, qualifiers and
unknown extensions. Its `unreviewed` carrier flag is not current admission.
The specific schema and the common record rules both apply; a permissive
profile schema cannot erase the shared floor. Legacy bibliographic and
historical claim files keep their own schemas and adapters. Existing source
records or reviews are not relabeled into this format.

The reader refuses unknown predicates/versions, a missing/abstract or multiply
owned mapping, broad Thing/Identity fallback domains, nonidentity or unresolved
endpoints, wrong domain/range, unlisted assertion layers, nonpublic visibility,
duplicate JSON fields, nonfinite values, remote/undeclared schema resources
and symlink or payload/catalog paths. Each record is bounded to 1 MiB and each
shared claim file to 16 MiB; exceeding the bound fails rather than truncates.
Consumed schema/registry digests feed the graph and source-create dependency
checks. A previous-registry comparison requires profile-version advance,
retained historical schema/layer routes and no silent predicate/reader reuse.

Reading a valid claim does not substantively assess or admit it. This first
claim-profile reader handles identity endpoints, not arbitrary literal values,
temporal objects, new inference rules, claim-writing permissions or automatic
assessment. A separately delegated [source-owner `claims.create`](../../../mechanics/growth-cycle/parts/branch-growth-cycle/README.md#declared-source-claim-creation)
now writes initial bounded Claim batches using these same profile rules,
exact input bindings and atomic source publication. The reader does not grant
that permission. Revision and assessment still need their own integration;
the metadata and
claim reader alone do not finish the documentary or other Foundation profiles.

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

## Biography, periodization, generations and historical environment

Registry version 15 declares six content-bearing historical profiles through
the existing `corpus-metadata-v1` reader. Their schema is
[`historical-context-record.schema.json`](../../contracts/historical-context-record.schema.json).
They retain the shared source metadata, language-tagged account, research
scope and continuity criterion. Changing the description does not change the
referent; changing its identity criterion is not an ordinary correction.

| Profile | Historical distinction and required account |
| --- | --- |
| BiographicalEpisode → HistoricalEvent | Bounded occurrence and its documented biographical relevance, not an entire life. |
| BiographicalPhase → HistoricalSituation | A described life phase and its boundary basis; not necessarily one event or homogeneous process. |
| HistoricalPeriod → HistoricalSituation | A situated periodization through specified developments or configurations, with its basis; not a numeric interval or universal epoch. |
| HistoricalGeneration → Identity | A cohort and the criterion that identifies it; not automatically an interacting group, Organization, or interval. |
| HistoricalEnvironment → HistoricalState | A scoped configuration and at least one substantive political, economic, cultural, religious, educational or scientific-technological account. |
| LifeCircumstance → HistoricalState | A documented condition, its relevance and evidence limits; bodily circumstances are optional and attributed, not retrospectively diagnosed. |

The six environment domains are independently discoverable content properties,
not combinatorial subclasses. A record need not claim knowledge of all six;
unfilled domains remain unknown, not absent. These records are historical
identities, not authored semantic Context/State nodes. Inheritance only supplies
compatible vocabulary and operations; it is not historical acceptance.

[`historical-context-claim.schema.json`](../../contracts/historical-context-claim.schema.json)
requires a language-tagged statement, relation basis, context scope and time
note. Unknown time bounds may be stated explicitly; prose is not a sortable
date. Registered domain/range and inverse labels are executable for:

- `biographical_subject`: episode, phase or circumstance → Agent;
- `phase_contains_episode`: phase → episode;
- `generation_member`: generation → Agent, by the declared cohort criterion;
- `generation_in_period`: generation → period;
- `situation_in_period`: historical situation → period;
- `contextualized_by_environment`: Agent, Organization, historical situation,
  IntellectualObject or IntellectualFormation → environment;
- `conception_in_environment`: situated Conception → environment, through the
  semantic relation reader rather than retyping its endpoint;
- `circumstance_during_phase`: circumstance → phase, without asserting that
  the condition held throughout the phase.

None is transitive or an influence/causality shortcut. Contextual association
and periodization do not derive chronological containment. Participation and
place reuse `historical_participant` and `historical_place` through this same
Claim schema; a participant still requires an explicit `participation_role`.
Legacy `historical-claims.jsonl` remains on its original schema and adapter.
Dating reuses the separate temporal Claim profile below. Its relative anchor
must resolve to a registry-declared HistoricalSituation, including a new
historical subtype; a generation or person is not such an anchor. The shared
value schema checks ID syntax and the reader checks type and existence.
The old carrier retains its three original anchor kinds. Exact previous schema
bytes remain in the contract history rather than rewriting provenance inputs.

These profiles use the ordinary separately scoped `source.create`,
`record.revise`, `claims.create`, `claim.revise` and source-copy form routes;
the registry is not a grant of write or assessment authority. Both source
readers retain all account fields and unknown extensions. Tests protect the
profile/schema boundary, relative-date anchoring, reverse navigation and
language-selected forms. Synthetic examples do not prove real historical
content, biography-lens completeness, assessment quality or UI acceptance.

## Structured values and textual survival

Registry version 16 adds `structured-value-v1` to the declared source Claim
reader. It is not a universal object serializer. Each profile specifies one
`value_kind`, mapped to exactly one concrete subtype of `tos.entity.literal`,
and an exact local schema route; its subject must belong to a specific
identity or semantic family. That kind is immutable across profile revisions.
An incompatible meaning needs a successor, not a changed mapping.

The shared value contract requires the declared kind and nonempty wording
with explicit language/script, including honest unknowns. A profile schema
cannot weaken it. Permitted unknown fields survive as uninterpreted data.
Fields named `date`, `places` or `relative` do not create time, geography,
identity dependencies or executable instructions. The established temporal
reader keeps its own stronger grammar and explicit anchor dependency.

`textual_survival` connects IntellectualObject to a Claim-scoped
TextualSurvival value. `complete`, `fragmentary`, `not_extant` and `unknown`
describe the **reported text scope**, with mandatory scope and coverage notes.
They are not confidence, admission, access rights, loss of a particular copy,
or intrinsic Work types. A quotation or reconstruction does not establish
complete survival of the original. Unknown status does not mean absence;
competing reports and corrections preserve their own Claim lineage.

The literal stays separately focusable through its Claim; two Claims carrying
equal values do not acquire shared subject identity. Exact wording supplies
its source-language name and summary; the enclosing assertion remains
inspectable, with evidence, attribution, uncertainty and current assessment
limits. `tos.property.textual-survival-status` is queryable through the ordinary
semantic catalog and filters. No kind-specific UI or text-interpretation model
call is added by this reader.

Exact graph traces also bind each literal to the governing Claim context in
its own delivered node. Focusing a value cannot drop the Claim's negation,
qualification or uncertainty. This applies to legacy literal carriers too;
it neither rewrites their source payload nor merges the value with the Claim.
The final-node cache binds that context as an explicit dependency.

Creation and value correction require separate v3 grants in the
[source command](../../../mechanics/growth-cycle/parts/branch-growth-cycle/README.md#declared-source-claim-creation).
V1 identity/descriptive and v2 temporal permissions are not widened. Source,
catalog, graph and assessment input readers preserve the exact value and
source bindings; receipt validity does not accept its historical content.
Rolling back a derived reader does not erase the new sources or corrections.

## Textual fragments and quoting passages

The independently mapped scholarly-composite route below retains the modern
reconstruction object; it must not be substituted for either passage identity.

Registry version 17 distinguishes an addressable `textual-fragment` from a
`quotation-passage`. Both are intellectual identities using the existing
declared metadata reader, not physical artifacts or semantic Claim identities.
Their descriptions require substantive notes, an explicit scope/continuity
criterion and language-bearing content. The fragment records its account and
boundary basis; the quoting passage records its quotation and location
accounts. An editorial number is a source designation, not proof of an
original division or identity with another edition's fragment.

A quoting passage is identified in its containing context. The same words
quoted in another place do not create the same passage. A quotation is not
necessarily an exact reproduction: selection, translation, interpolation,
quotation/paraphrase uncertainty and attribution limits belong to the source
account and its Claims. A bare bibliographic citation is not a quotation
passage, and the act of quoting is not this intellectual portion.

| Source predicate | Forward reading | Inverse reading | What does not follow |
| --- | --- | --- | --- |
| `fragment_of` | textual fragment belongs to an intellectual object | object has the proposed fragment | exact original boundaries or complete survival |
| `quotation_in` | quoting passage is located in an intellectual object | object contains that passage | a particular physical copy or resolved text anchor |
| `quotation_preserves_fragment` | quoting passage transmits the proposed fragment | fragment is transmitted through that quotation | exact equality, full coverage or authorial authenticity |

These are separately identified, evidence-bearing Claims with a required
statement, language/script and scope note. They are neither transitive rules
nor direct graph facts. Competing identifications and preservation accounts
may coexist; no unique-container cardinality silently accepts one claim over
another. The normal focus operation supports either endpoint and the Claim
center, retaining its complete context.

`source.create`, profile metadata correction, source-copy human forms and
Claim create/revise use the existing owner commands. Shared fields retain
unknown content, while creation/form permission does not grant assessment.
The four fragment/quotation content properties are discoverable in the
semantic catalog; the UI requires no new screen or kind-specific branch.

These metadata profiles do not contain exact source text. Acquisition,
transcription, normalization, segmentation and exact source anchoring remain
the versioned source-witness layer. A reported locator is not a mechanically
resolved anchor, and a description of a fragment is not its reconstruction.
Scholarly reconstruction remains with the scholarly-composite source route;
its existing physical-member profile must not be populated with fictional
artifacts to fit textual transmission. Compatible textual-composite coverage
requires its own explicit owner extension and validation.

## Scholarly composites: existing source adapter

Registry version 18 maps the native `tos.composite.*` identity to
`tos.entity.composite`, an intellectual object rather than a physical artifact
or the reconstructed original. Native `tos_scholarly_composite_witness_v1`
records remain unchanged in `scholarly-composites/`. The bounded adapter reads
only that owner's `composite-witness.json` files and preserves their exact
record, digest, identity status, preferred label, editorial description,
provider observations, members, coverage, rights and authority limits.

The catalog is a source-bound mapping, not another authored record. Both
source-navigation and source-claim carriers expose the same persistent ID;
the scene maps them to one vertex while inspection retains the carriers.
The stored metadata does not silently produce witness-membership Claims,
accept a reconstruction, establish an ancient recension or infer time from an
editorial label. The native metadata description is available; qualified,
language-declared human forms are still a separate growth route. Unsupported
adjacent Corpus-form files fail closed rather than acquiring fabricated
`record_id` semantics.

Unknown versions, nonpublic metadata, identity/schema/digest drift, duplicate
identities, unsafe paths, duplicate JSON keys and over-budget records are
rejected by the native source route. Rolling back the reader leaves the source
records intact. An ordinary declared metadata profile cannot replace this
native namespace; the v1 physical-member observations must not be populated
with invented artifacts.

### Descriptive composite growth alongside native witnesses

Registry version 19 explicitly retains `scholarly-composite-v1` while adding
the common-metadata `tos_scholarly_composite_record_v1` source profile to the
same `tos.entity.composite` and `tos.composite.*` namespace. This is a second
supported record shape, not a second scholarly-object type or a migration of
the existing witness records. Both use `catalog/composites.jsonl`; duplicate
current identities across either shape fail before creation or graph reading.
Changing the retained adapter is an incompatible profile change.

New descriptive records use
`scholarly-composites/<method>/<tradition>/<identity>/composite.json`.
The shared source command enforces this owner home for creation, correction
and form operations. It uses the ordinary declared-profile `source.create`,
`record.revise` and human-form contracts, including exact dependencies,
predecessors, unchanged referent scope and idempotent retry. It does not rewrite
`composite-witness.json` or make native forms/assessment supported by coercion.

`semantic_content` requires `composition_account`, `editorial_method` and
`coverage_account`, separately discoverable as `tos.property.composite-*`
properties. Their values are optional at the general composite type because
native records retain their own fields, but mandatory in this exact new
schema. `semantic_scope` states the referent and continuity criterion;
historical existence, reconstructed original, compiler, containing publication,
members and their order require separately grounded Claims. A method account
is neither executable transformation provenance nor approval of an edition.
Exact quotations/text layers, physical witnesses, rights and scoped assessment
remain their own records and operations. Absence from a reported arrangement
does not establish loss or nonexistence of a passage.

Four non-transitive, evidence-bearing relation profiles use the ordinary
declared `claims.create` and Claim-form operations. Each requires a qualified
statement and scope; competing attributions are not limited to one compiler.

| Predicate | Domain → range | Reverse reading |
| --- | --- | --- |
| `composite_reconstructs` | Composite → IntellectualObject | reconstructed in this composition |
| `composite_included_in` | Composite → IntellectualObject | contains this editorial composition |
| `composite_contains_passage` | Composite → QuotationPassage | passage included in this composition |
| `composite_compiled_by` | Composite → Agent | compiler of this composition |

These are not physical witness membership, an exact passage sequence or a
claim that the editor inspected every reported source. The compiler role is
distinct from authorship of either the ancient poem or the containing book.

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

The declared `historical-temporal-v1` source Claim reader now carries this same
value grammar in `source-claims.jsonl` using `tos_source_temporal_claim_v1`.
It requires a historical-situation domain and a temporal-assertion range, not
an identity object or a general semantic category of time. The new schema
composes the existing `historicalDate` definition rather than replacing the
legacy historical adapter. It adds a source-authored qualified statement with
explicit language/script for the common Claim forms. Date profile extensions
cannot weaken the shared value grammar. Newly registered dating predicates
use the same source, assessment, graph and access readers, including relative
anchor navigation; no per-predicate Python branch is required.

The [shared command](../../../mechanics/growth-cycle/parts/branch-growth-cycle/README.md#declared-source-claim-creation)
creates and corrects these values only with separate v2 exact-value delegation.
It preserves source/evidence bindings, uncertainty, original attribution,
immutable predecessor bytes and all current source-copy forms. A value remains
Claim-scoped, not a new Date identity. The v1 identity-only creation and
descriptive-only correction grants retain their old scope. Legacy
`historical-claims.jsonl` remains under its original adapter and does not gain
revision history by reinterpretation.

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
The source-form command adapter can prepare/create/revise those adjacent forms.
Separately delegated historical subject creation and record correction now
use the [source-owner command route](../../../mechanics/growth-cycle/parts/branch-growth-cycle/README.md#initial-historical-subject-creation).
The metadata reader does not confer those permissions. The profile
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
