# Semantic interchange registry

This directory owns the stable machine vocabulary used when ToS material is
composed into read-only knowledge lenses. The registry connects source-owned meanings across readers through stable
types, relations and explicit mappings. Entity registry 39 and relation
registry 45 state their definitions through each subject’s properties, purpose
and relations. Source-specific uncertainty and substantive negation remain
part of the authored meaning.

## Scoped composition and research corpora

Entity registry 32 adds the persistent `research-corpus` source profile and two
distinct Claim-scoped value types. Relation registry 37 connects
`intellectual_part_composition` (IntellectualObject → qualified proper-part
composition) and `research_corpus_membership` (ResearchCorpus → qualified
research selection). The first accepts declared IntellectualObject subtypes;
the latter accepts IntellectualObject, Expression, Edition, Item, Artifact,
Collection and ResearchCorpus members. Each predicate declares its own endpoint families.
Entity registry 33 / relation registry 38 add the separate
`physical_part_composition` predicate and physical-part value: both its whole and all members must be Artifacts. Joining, position, original
completeness, custody and restoration require separately grounded accounts.

Entity registry 34 / relation registry 39 add `collection_member_order`
(Collection → qualified Work ordering). Its `collection-membership-versions-v1`
basis adapter binds an exact Collection version and one existing positive
`contains_work` Claim version per member, as its membership evidence.
Current and retained native versions are resolved without latest fallback;
retained legacy Collection streams expose only their available current version
and explicitly do not claim a native correction chain. Missing basis bytes
refuse preparation or projection; they never invent historical membership.

All four reuse `structured-reference-value-v1` with the explicit
`object_reference_set.structure_adapter: scoped-members-v1`. Concrete profile
bounds remain authoritative: existing motif proposals still permit only 2–8
Occurrences; these structural profiles permit 1–128 members. The shared schema
and reader validate scope, coverage, no self membership, exact member closure,
acyclic local precedence and comparability when total ordering is claimed.
Unknown extensions remain uninterpreted. The detailed meaning and evidence requirements live in [Corpus Foundation](../CORPUS_FOUNDATION.md).

`public-profile-create` / `source.create` and public record revision handle the
corpus description and source-copy name/notes forms. The existing v4
`public-claim-create-v4` and reference-value revision routes create/correct the
whole composition Claim: both the exact value and every member role require
current delegation. Prepare, expected dependency/version checks, atomic
publication, replay, retained previous bytes and independently scoped
assessment remain the same operation grammar. Discover exact handler names and
request shapes through [source-command discovery](../../../mechanics/growth-cycle/parts/branch-growth-cycle/docs/SOURCE_COMMAND_DISCOVERY.md).

Human forms bind the complete Claim, including its qualified statement and
every declared member. The existing value-member graph edges expose every
declared member through the Claim in either direction; their assessment context stays with the governing Claim. Named properties expose scope, coverage, membership basis,
order mode/basis and limitations for inspection and filtering. Source wording
and the full ordered value remain available; free wording still needs
source-visible quality assessment. Order comes from the Claim’s explicit order fields and basis. Existing publication-Collection membership remains on its
separate `contains_work` compound-operation route.

These contracts describe bounded intellectual/physical parts, research
membership and attributed publication-Collection order through qualified
Claims. A future concrete predicate can reuse the adapter
and shared shape without adding a per-type writer or graph screen; its meaning,
endpoint family, evidence, schema and review route must still be declared.

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
metadata kinds. Source status, domain/range, identity and review requirements retain their
declared values. The source CSV remains the owner of its wording; the
access contract test checks crosswalk parity rather than accepting new meaning.

Unknown source vocabulary is represented by the explicit
`tos.entity.unmapped` or `tos.relation.unmapped` fallback. It must never be
silently coerced into the nearest familiar type. A new stable type is added by
extending the registry, declaring its owner and lifecycle, validating the
hierarchy and crosswalk, and bumping `registry_version` when a released
registry changes. Incompatible meaning receives a successor ID and an
explicit `supersedes_*` link rather than reusing an old ID.

Hierarchy checks follow the declared parent edges, including multiple
inheritance and shared ancestors. Each endpoint ancestry lookup expands a
reachable type once; registry cycle checks use an explicit
traversal stack. Depth follows the selected registry and its input budget.
Missing parents and cycles remain errors. Compatible extension preserves
these checks across source reading, navigation and semantic validation.

## Candidate-only dossier relations

Relation registry 44 adds three distinct, directed `philosophy` / `edge`
crosswalks for source candidates already retained by the atlas source-return
route. Each mapping retains a specific endpoint pair:

| Exact native predicate | Stable relation ID | Domain → range |
| --- | --- | --- |
| `figure_anchor` | `tos.relation.candidate-authorizing-figure` | `tos.entity.text-corpus` → `tos.entity.figure` |
| `translates_into` | `tos.relation.candidate-translator-involvement` | `tos.entity.figure` → `tos.entity.text-corpus` |
| `uses_medium` | `tos.relation.candidate-material-realization` | `tos.entity.language-script` → `tos.entity.medium` |

All endpoints retain their existing pre-canon identities: figure, text-corpus,
language-script and medium. The first relation reports the dossier’s
authorizing or intratextual figure. The second reports translator involvement
**from figure to corpus**, retaining the original language/comment wording.
Its direction and endpoints are specific to that report. The third reports
material realization, including an information system’s cord structure, with
its frontier constraints as mandatory source context. Historical authorship,
translated Expressions, identity equivalence and Artifact identification each
require their own source account and review.

Forward and inverse RU/EN labels explicitly qualify the relations as reported
by a dossier. Exact native predicates, source labels, comments, complete source
bodies, evidence refs, confidence, manual-review requirements and pre-canon
status remain unchanged. `assertion_mode: direct` describes the existing candidate edge carrier. These
read-only mappings apply exclusively to the philosophy candidate edge scope.
Reified source Claims, source writes and admission use their separately
declared routes. Unknown vocabulary still uses the explicit fallback.

The [source-visible boundary review](../../review-ledger/2026-09-12-philosophy-candidate-relation-mappings-review.md)
records the exact five relations, eight endpoints, stream digests and immutable
pre-change registry baseline. The earlier
[atlas source-return review](../../review-ledger/2026-09-09-philosophy-atlas-source-return-review.md)
retains its then-unmapped state and original review date.

## Semantic boundaries

- Agent is a persistent responsibility bearer. Typed relations describe authorship, translation, editing, design and other
responsibilities in context.
- Work, Expression, Edition, Item, File, and Link remain distinct. A Link records an observed access identity, with its target and observation
context.
- Place is a persistent geographic identity. A source-navigation Region groups records for browsing; Place records identify
geographic referents.
- Event identity is separate from TemporalAssertion. Dates, intervals,
  precision, calendars, and publication stages remain source-returnable
  assertion values.
- Bibliographic and other evidence-bearing assertions use reified Claim
  topology: subject, predicate family, object, evidence, maker, provenance,
  review, version, and supersession stay inspectable.

`tos.relation.claim-counterevidenced-by` preserves the existing source-claims
`counterevidenced_by` edge as a separate Claim → Evidence role: the Claim cites
that Evidence as counterevidence. Its inverse reads “cited as counterevidence
by claim”. This derived edge records the Claim’s explicit counterevidence role. The
[bibliographic producer](../../../scripts/source_witness_bibliographic_graph_common.py)
derives this edge only from the Claim's explicit `counterevidence_refs` and
retains its exact source, version, qualified context and recorded review state.
Counterevidence is optional; absence does not establish that a search was done.
The same Evidence may support the qualified Claim while limiting an overreading,
as in `tos.claim.jgb21-conception-inversion`; the two roles retain their own meaning, with judgment and admission supplied
by the Claim’s assessment route. Unknown source
vocabulary outside this exact edge mapping retains its existing fallback.

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
graphs, D1 tables, catalogs, and LensResults are disposable read models. Source, rights, translation, semantic, identity and canon judgments belong to
their authorized review routes.

## Retained object-Link v1 context

The retained `relations/object-link/object-link-claims.jsonl` stream has an
explicit read-only adapter in `scripts/source_object_link_read.py`. It reads
the unchanged `tos_object_link_claim_v1` contract: Work, Expression, Edition,
Collection or Item as subject, Link as object, and the four declared access
predicates. The newer six-kind native v2 write route has its own contract; legacy v1
retains the subject domain and read-only adapter stated here.

Every retained Claim has both its existing direct navigation edge and an
additive reified source-claim carrier. Both preserve the exact raw Claim,
source file, line and canonical digest. Evidence, maker, provenance, qualifiers,
empty reviews, version and supersession return through ordinary core/agent
inspection. The existing Link navigation body remains unchanged; its second source-claims endpoint carries the same declared Link ID in reified
Claim topology.

The adapter preserves the legacy record’s available wording, language, Forms,
assessments and history. Missing Forms remain explicit missing roles. A Link
returns the recorded observation; acquisition, rights clearance and content
assessment use their respective owner routes. The portable consumer checks exact
body/digest/endpoint agreement and rejects conflicting marked carriers; source validation and export authentication remain independent checks. Older
unmarked projections retain their absent-context state.

Exact metadata-version views reuse the same portable native identity grammar
as Form selection. Artifact and scholarly Composite keep `artifact_id` and
`composite_id`; the view uses those native identity fields directly. Exact source version
and canonical byte-body binding remain required, with no current-use grant.

## Executable boundary and text spine

Registry version 2 rejects abstract instances, missing or cyclic supersession
targets, incompatible endpoint types and missing supporting Claim references.
A bibliographic Claim has exactly one subject and one object. A literal object stays local to its enclosing Claim; `claim_ref` identifies
the Claim. `same-as` requires resolved exact-version review and evidence nodes.
Multiple representations of one persistent ID remain separate; `projects` retains each representation, including several from one source.

Property descriptors publish type, applicability, inheritance and operators.
`semantics.type_ancestors contains <type-id>` selects a type and descendants.
Comparable time bounds retain precision and declare calendar and year numbering.
Unknown dates/calendars do not gain invented order keys.

Public text-unit and semantic-annotation-v2 packets expose an addressable route
from Work through TextLayer, TextUnit/Anchor, Occurrence/Sign/Concept and Claim
to Evidence/Review. Competing interpretations remain separate. The projector
does not read private text; metadata-only packets omit lexical hashes and
declare content availability. Publication and scoped assessment of a private lexical workbench each require
their own owner decision.

Native v2 `occurrence`, `lexeme` and `lexical_sense` entities use the distinct
`annotation-occurrence`, `annotation-lexeme` and `annotation-lexical-sense`
adapter kinds and corresponding `tos.entity.*` types. They retain every native
ID, `entity_kind`, source field, anchor and Claim/evidence route; Their parent is `semantic-object`; authored description profiles have separate
types and required semantic accounts. Authored profiles and their required accounts stay unchanged.

Missing reviews, unresolved source endpoints and synthesized descriptions remain
visible gaps. Broad legacy relation families retain native predicates; their philosophical endpoint semantics require source-visible review before a
tighter mapping is declared.

## Declared source-metadata profiles

Entity registry version 7 gives the three historical identities an executable
`source_record_profile`. The profile declares the source-to-reader contract on the existing type entry.
Each source family keeps its own supported record shapes. The first supported reader is `corpus-metadata-v1`; native
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
The common reader resolves it from the declaration. The ordinary knowledge
catalog and `tos.knowledge.contracts` expose the declaration, including when
there are no instances. Catalog entry fields and their source digest are
checked against the exact source record; all public fields, including
uninterpreted `extensions`, survive inspection. Additional catalog families
are admitted only by a declared profile, never by a permissive catalog schema
alone. Missing or unrecognized profiles fail closed without deleting source.

Adjacent human-form sets reuse the existing bounded metadata materializer.
Complete source-copy names and notes retain language, script, context, exact
source version, provenance and visible quality state. An addressable source
record may have no Claims. Source description, semantic relations, wording
assessment, scoped admission and publication have distinct contracts. Shared
operation law applies to every profile below: discover the type and schema;
select an explicit owner grant for a write; preserve exact dependencies and
previous versions; assess the source-visible result under the applicable
policy. A prose definition explains its subject, properties and relations.
Source-specific uncertainty, hypothetical force and genuine negation belong in
that explanation. Executable permissions and review states belong in their
typed fields and owner contracts. Source-write commands remain separately delegated: the
[source-owner command](../../../mechanics/growth-cycle/parts/branch-growth-cycle/README.md#declared-profile-subject-creation)
`source.create` now creates an initial public-metadata subject and its forms
from an independently selected profile configuration. It reuses the declared reader/schema route and the creation scope selected by
the owner configuration. Historical creation with initial claims retains its
narrower contract. The separately delegated `record.revise` profile
configuration corrects the same declared metadata kinds through the existing
source-revision transaction. It preserves stable identity, the exact prior
flat source package and forms, and validates the current registry/schema
dependencies before publication. Type, source schema version, identity status, rights and admission stay
immutable under this descriptive revision grant. Public v1 retains its
original field scope. Explicit public v2 delegation also permits wording
corrections in `semantic_scope` while preserving the referent and continuity
criterion’s meaning. A change of referent or criterion meaning requires the
identity route. Native non-profile records and Claims
retain separate write routes.

The reader rejects duplicate kind/prefix/basename/catalog ownership, mappings
owned by another type, abstract identity instances, reserved native-adapter
collisions, unknown reader modes and schema versions, nonpublic visibility,
duplicate JSON keys, nonfinite numbers, symlink paths, metadata above 1 MiB,
undeclared schema dependencies and catalog/source drift. Schema resources are
local and exact; consumed registry/schema bytes supply the graph dependency
digests. Source-visible assessment owns knowledge admission; the rights owner
authorizes publication of source contents.

The blocking `semantic_registry_transition` lane compares current working-tree
registries with an explicitly selected pre-change commit. Set
`TOS_SEMANTIC_REGISTRY_BASELINE_COMMIT=FULL_COMMIT_OID` before
`python scripts/validation_lanes.py --run semantic_registry_transition`;
the direct validator also accepts `--baseline-commit`. This source-contract
operation has its own result under
[the independent release boundaries](../../../docs/RELEASING.md#registry-source-contract-changes).
Only a nonzero full commit OID backed by local Git objects is valid; evolution
also requires both registry/contract snapshots. There is no missing-baseline
skip, moving ref, implicit predecessor,
replacement object or automatic fetch. The source-change owner selects and
records the exact pre-change commit. Software CI reports its selected software
checks separately. A first introduction with both registries and both
contracts absent requires the separate `--allow-initial-introduction` option
or `TOS_SEMANTIC_REGISTRY_ALLOW_INITIAL_INTRODUCTION=1`, selected by that owner.
Complete baseline ancestry must also contain no
earlier registry/contract or `scripts/source_record_profiles.py`; shallow
history and local Git grafts are refused. A partial snapshot, deleted prior
reader or missing Git history is not an introduction.
The report names `initial-introduction` and no previous-registry comparison;
the option never bypasses comparison when previous registries exist.
The selected baseline must precede the complete change being reviewed. The gate validates each
snapshot against its own registry contracts, then reuses the semantic
validator's transition rules: a changed profile advances `profile_version`,
preserves all earlier schema routes, and does not repurpose its kind or ID
prefix; registry changes also advance the registry version. Compatible source
evolution adds a schema route; an incompatible identity meaning needs an
explicit successor and reference migration. A catalog path move likewise
needs coordinated reference migration; it never changes identity merely
because a path changed. The gate checks declared mechanics. Review must also establish semantic
compatibility of definitions, domain/range and properties, and verify any
required reference migration. Ordinary current-snapshot readers remain Git-independent
and do not select or validate a change baseline.

The first migration moves the three historical readers' hard-coded schema and
catalog choices into their owner type entries. Existing historical and
artifact source bytes are unchanged. An old reader must be upgraded together
with its registry/catalog schema before consuming a new profile. Reader
rollback does not erase source records, judgments, creation receipts, retained
record history or human-form predecessors.

This route provides declared metadata extension. Its reader test uses the
synthetic `fixture-document` to check contract behavior; historical evidence
and source admission are established through actual source records and review.

## Reasoning objects, contextual roles and addressed objections

Entity registry version 10 adds Thesis, Argument, InferenceStep and Objection
as specific SemanticObjects, without reclassifying existing canon nodes.
They use the same `semantic-metadata-v1` source/create/read/revise/form route.
`thought-description-record.schema.json` composes the shared source metadata
and semantic scope constraints; it adds a required `semantic_content` with
its own language/script and the following substantive fields:

| Kind | Required account | Boundary |
| --- | --- | --- |
| Thesis / Тезис | `proposition`, `assertion_force` | a proposition under examination, with its stated assertion force |
| Argument / Аргумент | `reconstruction_note`, `coverage` | a reasoning reconstruction with an explicit account of its coverage |
| InferenceStep / Шаг вывода | `transition_account`, `reasoning_mode` | a described transition from premises to a proposed conclusion |
| Objection / Возражение | `challenge_account` | a reasoned challenge addressed to specified content or reasoning |

`coverage` is `partial`, `claimed_complete` or `unknown`: a completeness claim retains its attributed research posture and requires
assessment. Force and
reasoning mode preserve the source's wording without requiring one logical
school's taxonomy. Unknown nested fields remain source data, never commands.
Descriptions need a source-visible assessment for substantive adequacy.

Names and notes carry the complete semantic scope **and content** as mandatory
reading context. A hypothetical premise must not appear as an unconditional
fact after a short label is selected. The seven named content properties in
the registry support semantic-ID queries through the ordinary catalog and
snapshot-bound property filter. The common reader supplies these properties.
The separately delegated correction route may update `semantic_content` only
where the source schema and grant allow it, retaining exact previous bytes
and rebuilding forms; the referent criterion’s meaning and admission remain fixed.
A changed account of the *same* referent is distinct from historical thought
change or a new referent, which need their own subject and grounded relations.

Relation registry version 9 adds the following reified source Claims:

| Predicate | Subject → object | Meaning and limit |
| --- | --- | --- |
| `conception_has_thesis` | Conception → Thesis | membership in the specified situated account |
| `argument_for_thesis` | Argument → Thesis | reasoning offered in support of the thesis |
| `argument_has_step` | Argument → InferenceStep | `qualifiers.step_position` is a required nonnegative integer position in this reconstruction |
| `step_has_premise` | InferenceStep → Thesis | premise role in this transition, possibly granted only hypothetically |
| `step_has_conclusion` | InferenceStep → Thesis | proposed conclusion role in this transition |
| `objection_to_thesis` | Objection → Thesis | challenge to this proposition; use this to challenge a premise as content |
| `objection_to_step` | Objection → InferenceStep | challenge to the transition, distinct from challenging its premise |
| `objection_to_argument` | Objection → Argument | challenge to the specified reasoning structure as a whole |
| `objection_to_conception` | Objection → Conception | challenge to specified commitments of a situated account |
| `objection_developed_by_argument` | Objection → Argument | reasoning develops the challenge; the referents stay distinct |
| `thought_expressed_in` | Thesis/Argument/InferenceStep/Objection → Work/Expression/Document | interpreted expression in the specified intellectual object |
| `thought_attributed_to` | Thesis/Argument/InferenceStep/Objection → Agent/Organization | attribution to a thinker or collective within the stated scope |

Premise and conclusion identify the roles a thesis plays in a particular
inference step.
One thesis may fill different roles in different steps. No global acyclicity,
one-author rule or universal cardinality is imposed. A partially reconstructed
argument may lack known steps or premises; unknown content is not silently
invented to complete a graph. Rival step positions live in separate qualified
Claims rather than overwriting one another. All twelve predicates have
specific endpoints, both reading directions, mandatory statement/basis and
ordinary evidence, provenance, uncertainty and separate assessment. Logical validity and historical truth remain questions for source-visible
assessment of the qualified Claims.

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
| Aspect / Аспект | `perspective_account` | a dimension along which a subject is examined |
| PhilosophicalCategory / Философская категория | `category_account` | a philosophical category used to organize inquiry |
| Problem / Проблема | `problem_statement`, `inquiry_stakes` | an inquiry’s difficulty and the stakes of addressing it |
| ProblemFamily / Семейство проблем | `grouping_basis` | a grouping of distinguishable problems under an explicit basis |
| Question / Вопрос | `question_text`, `presupposition_account` | an interrogative formulation whose presuppositions need not be accepted |
| Position / Позиция | `stance_account` | a stance with specified commitments and limits |
| Distinction / Различение | `differentiation_criterion` | a differentiation in a stated respect, with declared coverage |
| Opposition / Оппозиция | `differentiation_criterion`, `opposition_basis` | a subtype of Distinction with an explicit basis for opposition |

The ten content properties are discoverable by `tos.property.*` IDs in
the ordinary catalog and execute in the same snapshot-bound node/path filter.
Opposition inherits the Distinction differentiation property with the same
property ID. These profiles use the shared reader and writer.
`source.create` and separately authorized `record.revise` use the selected
profile schema, exact dependencies, predecessor retention and the existing
transaction boundaries. A revision can correct the account within its selected grant; the referent
criterion’s meaning, type, identity, source schema and admission remain fixed. Unknown
nested semantic fields survive as uninterpreted source data, not executable
instructions. Nonempty wording satisfies the structural requirement; substantive adequacy
requires source-visible assessment.

Relation registry version 10 adds ten specific, nontransitive reified Claims:

| Predicate | Subject → object | Scope |
| --- | --- | --- |
| `problem_family_member` | ProblemFamily → Problem | membership under the stated grouping basis and coverage |
| `problem_has_question` | Problem → Question | articulation of the problem through a question |
| `question_proposed_answer` | Question → Thesis | proposed answer with its own assertion context |
| `position_has_thesis` | Position → Thesis | specified commitment, with separate holder attribution |
| `position_addresses_problem` | Position → Problem | engagement with the specified problem |
| `conception_has_aspect` | Conception → Aspect | perspective on this situated account |
| `aspect_of_concept` | Aspect → CrosscuttingConcept | the perspective's crosscutting subject |
| `category_organizes_concept` | PhilosophicalCategory → CrosscuttingConcept | organization of a concept through the philosophical category |
| `distinction_first_term` | Distinction (including Opposition) → CrosscuttingConcept/Conception/PhilosophicalCategory/Thesis/Position/Aspect | first term in this stated differentiation |
| `distinction_second_term` | same domain/range | second term in this stated differentiation |

`thought_expressed_in` and `thought_attributed_to` also admit these eight
profiles with the same limited meanings described above. The attribution’s thinker or collective and the research Claim’s maker each
retain their separate roles; endorsement and authorship require their own
accounts. All relationships retain source,
statement language, explicit relation basis, uncertainty, provenance, review
posture and both reading directions. Partial and competing term/membership
Claims can coexist. No global two-term completeness, exhaustive family tree,
one-holder cardinality or graph-wide acyclicity is inferred. The type hierarchy
itself remains acyclic. Substantive comparison examines the term Claims and differentiation criterion
together.

Existing canonical and atlas Concept, Method, institution and category-like
records retain their source identity and mappings. A new mapping or revision
requires explicit review. This extension provides the declared profiles and
operations; substantive assessment proceeds on the actual source records.

## Methods, hypothetical inquiry, imagery and valuation

Entity registry version 12 adds ten source-described profiles through the
same semantic metadata reader and source-owner commands. The shared
`semanticContentFields` contract owns the language/script declaration for
reasoning, inquiry and practice content. Existing record shapes are unchanged;
`thought-practice-record.schema.json` adds only the following requirements.

| Profile | Required content | Boundary |
| --- | --- | --- |
| ThoughtMethod / Метод мышления | `method_account`, `applicability_conditions` | a method of inquiry with stated applicability conditions |
| ThoughtOperation / Операция мышления | `operation_account`, `prerequisites` | a conceptual operation with its prerequisites |
| ThoughtMove / Ход мысли | `movement_account`, `context_requirement` | a reframing or movement of thought within a specified context |
| ThoughtExperiment / Мысленный эксперимент | `scenario_account`, `assumptions`, `assumption_coverage`, `examined_consequence` | a hypothetical scenario, its assumptions and the consequence being examined |
| ThoughtImage / Образ мысли | `image_account`, `image_mode` | an imaginative presentation with a stated mode |
| RhetoricalFigure / Риторическая фигура | `figure_account` | an expressive arrangement in the source account |
| Metaphor / Метафора | inherited `figure_account`, `source_domain`, `target_domain`, `mapping_basis` | a subtype of RhetoricalFigure proposing a transfer between stated domains |
| Value / Ценность | `value_account`, `valuation_context` | an evaluative criterion within its described valuation context |
| Ideal / Идеал | `ideal_account`, `realization_posture` | a normative model with its declared realization posture |
| OntologicalCommitment / Онтологическое обязательство | `commitment_account`, `commitment_force` | an account’s scoped commitment with its stated force and conditions |

Conditions, prerequisites and assumptions are string arrays: empty means none
recorded, not proof that none exist. Assumption coverage is `explicit_only`,
`reconstructed_partial`, `claimed_complete` or `unknown`. Realization posture
is `normative_model`, `proposed_realization`, `claimed_realized` or `unknown`.
Completeness and realization claims require assessment of their grounds. The
remaining account fields retain source-described wording without mandating one
philosophical or aesthetic taxonomy. Values need not become separate entities;
these profiles are for referents whose independent identity is useful.

All 22 added content properties are discoverable and executable by semantic
property ID, including array membership filters. Metaphor inherits the RhetoricalFigure account property with the same ID. Short human forms
carry the complete scope and content; conditions and normative posture remain mandatory context. The ordinary create/correct/read/form route retains
unknown nested fields, exact predecessor bytes and one subject ID. Source
content cannot supply a command, choose a reader implementation or grant itself
assessment/admission powers. Source-visible assessment evaluates descriptive quality.

Relation registry version 11 adds thirteen specific, nontransitive reified
Claims with language, statement, grounds, uncertainty and separate assessment:

| Predicate | Subject → object | Scope |
| --- | --- | --- |
| `method_uses_operation` | ThoughtMethod → ThoughtOperation | conceptual use within the method |
| `move_uses_operation` | ThoughtMove → ThoughtOperation | operation used within this movement |
| `experiment_uses_method` | ThoughtExperiment → ThoughtMethod | method used by this hypothetical inquiry |
| `experiment_assumes_thesis` | ThoughtExperiment → Thesis | granted hypothetically for the trial |
| `experiment_tests_thesis` | ThoughtExperiment → Thesis | proposition examined by the experiment |
| `experiment_adopts_commitment` | ThoughtExperiment → OntologicalCommitment | adoption within the scenario and its conditions |
| `conception_has_commitment` | Conception → OntologicalCommitment | commitment with the account's scope and force |
| `thought_uses_image` | Conception/Argument/Thesis/ThoughtExperiment/Position/Ideal → ThoughtImage | expressive use within the thought account |
| `thought_uses_figure` | same domain → RhetoricalFigure, including Metaphor | interpreted use of the expressive arrangement |
| `ideal_exemplifies_value` | Ideal → Value | normative exemplification through the ideal |
| `position_affirms_value` | Position → Value | valuation within the position’s stated scope |
| `method_guided_by_value` | ThoughtMethod → Value | a norm guiding the method |
| `image_presents_conception` | ThoughtImage → Conception | presents the conception, including when it is a critical target |

Each has an explicit inverse reading. `thought_expressed_in` and
`thought_attributed_to` include the ten new kinds without strengthening their
existing meanings. Partial reconstructions need not fabricate every assumption,
operation or bearer to validate. Competing interpretation Claims remain possible.
No global experiment completeness, one-figure taxonomy or historical order is
imposed. Existing atlas Method and Figure entries and canon Analogy/Principle
nodes retain their original identities, types and owner routes. The shared reader and writer consume each profile’s declared data and
contracts.

## Concepts, situated conceptions and transformations

Entity registry version 9 adds `CrosscuttingConcept` as a subtype of the
existing broad Concept family, and Conception as a distinct SemanticObject.
Existing canon and philosophy Concept nodes keep their IDs, mappings and
scoped meanings. For example, `tos.concept.becoming` retains the scope of the authored
Zarathustra-prologue interpretation. Relating it to a crosscutting subject
requires an explicit source-grounded Claim.

`semantic-metadata-v1` reads concrete semantic-family descriptions through the
common metadata pipeline while preserving their semantic-family type. The exact
`semantic-description-record` schema requires substantive notes, declared
wording languages and `semantic_scope`: a scope note and a referent continuity
criterion with their own language/script. Those fields state the research account’s scope and continuity commitments. A
complete description includes them alongside the name and substantive content.
Mechanical validation checks their presence; source-visible assessment
evaluates their adequacy.

Source-near descriptions live in
`ToS/source-witnesses/semantic-descriptions/<stable-subject>/`. They describe the source-visible subject. Separate source Claims express
interpretation, membership and comparison. Philosophy and candidate-intake
retain their authored surfaces and review routes. Canon, admission and source interpretation retain their
existing review and assessment owners. Initial records are provisional under
the separately delegated `source.create`; mere reading grants no permission.

Human-form names and notes carry the complete `semantic_scope` as mandatory
context, alongside identity posture and wording language. Changing that scope
stales exact-version forms and prepared dependent Claims. Source correction
uses `record.revise` and retains prior bytes and form history. It changes the
description version, not the conception's identity. The correction route preserves the referent and the scope/identity criterion’s
meaning, kind, ID and admission. The explicit public v2 grant described above
permits corrections to scope wording. A
different referent needs a separately created subject and an explicit grounded
transition; it must not be smuggled in as a corrected note. Correctness of a
same-referent prose correction remains a content-assessment question.

Relation registry version 8 declares these grounded semantic predicates:

| Predicate | Subject → object | Required distinction |
| --- | --- | --- |
| `conception_of` | Conception → CrosscuttingConcept | membership explained through the declared continuity criterion |
| `conception_attributed_to` | Conception → Agent/Organization | the thinker or collective to whom the conception is attributed |
| `conception_expressed_in` | Conception → Work/Expression/Document | interpreted expression with the stated textual scope |
| `conception_redefines` | Conception → Conception | changed definition, with retained and changed features |
| `conception_rejects` | Conception → Conception | rejection of the specified commitments |
| `conception_narrows` / `conception_expands` | Conception → Conception | a specified comparison dimension and restricted or extended scope |
| `conception_secularizes` | Conception → Conception | specified theological commitments reworked in a non-theological register |
| `conception_psychologizes` | Conception → Conception | reworking through a mental-process explanation |
| `conception_politicizes` | Conception → Conception | reworking through specified political commitments or relations |
| `conception_inverts` | Conception → Conception | a specified ordering, valuation or explanatory direction reversed |

The transforming conception is the subject; the conception it reworks is the
object. Each predicate has forward/inverse Russian and English labels, a
concrete domain/range and an explicit definition. None is transitive. There
is no graph-wide acyclicity rule or one-conception/one-author restriction.
Reading, direct influence and historical priority require their own grounded
Claims; the registry retains its declared type meanings.

`semantic-relation-v1` requires at least one specific semantic endpoint and
allows the relation's declared specific identity endpoints. It cannot replace
the old identity reader silently or use Thing/Identity/SemanticObject fallback
roots. Its exact source schema requires a full statement, explicit wording
language/script and `relation_basis`, as well as ordinary evidence, maker,
provenance, uncertainty, counterevidence and separate assessment. Assessment evaluates the relation basis against its source grounds. Negated, disputed and competing Claims
remain distinct and source-returnable through the ordinary catalog, both
graph readers, focus, inspection, compact Claim paths and `claims.create`.
Every relation retains its source qualifications and recorded assessment
context. Unknown extension fields retain their bytes
and do not select executable behavior.

The common pipeline supports new concrete semantic metadata and relation
profiles as registry/schema data, within these reader modes and independent
write/assessment permissions. Source-mode changes require an explicit
successor, not a higher version that silently retypes old records.
The [decision rationale](../../../docs/decisions/TOS-D-0053-source-described-conceptions.md)
explains the additive subtype and source/review boundary. Synthetic tests verify these contracts. Real concept-history accounts,
occurrence links and substantive assessment proceed through their source
owners.

## Social bodies and source-attributed relationships

Entity registry version 13 adds `SocialGroup` and `InstitutionalBody` under
the existing collective `Organization` identity family, and `Community`
under `SocialGroup`. SocialGroup identifies a group through its social membership boundary.
Community adds continuing shared practice or belonging; InstitutionalBody
identifies an organized body through its roles and continuity. These
historical research referents have source-described identities.
Existing `Tradition`, `SchoolTradition` and `Institution` navigation types and
their IDs retain their meanings. Intellectual school, tradition and movement profiles use the formation
contract below.

`social-body-record` composes the common metadata and description fields.
Its `semantic_scope` field names the research description’s scope and
continuity criterion; its type remains in the Organization family. Group account and membership boundary are inherited by
Community, which additionally requires a shared-practice account. Institutional
description requires its organized-role account. These content fields are required alongside names and notes. Assessment
evaluates descriptive quality and identity. Ordinary `source.create`, `record.revise`,
forms, catalog, both graph carriers and semantic property filters use the
existing `corpus-metadata-v1` profile reader, without per-kind Python dispatch.
Ancestry-aware queries retain one subject ID across the subtype and its bases.
Full scope/content is mandatory human-form context. Separate qualified Claims
describe attributed relations and membership.

The source-navigation adapter also carries adjacent human forms for the eight
native Corpus families, matching the source-claims carrier. It validates the
native source schema, typed identity and catalog digest before binding forms.
Source changes without matching catalog refresh fail; refreshed source with old
forms exposes their stale state rather than dropping them or reusing wording.
Native Link and Artifact formats use their own form adapters under [Human
Forms](../HUMAN_FORMS.md). Default focus therefore need not lose a person's existing forms
merely because it selects the navigation carrier.

Relation registry version 12 adds eight `identity-relation-v1` predicates:

| Predicate | Subject → object | Attributed relationship |
| --- | --- | --- |
| `social_member_of` | Agent/Organization → Organization | membership with its stated period, basis and limits |
| `learned_from` | Agent → Agent | a teacher relationship supported by its particular sources |
| `studied_at` | Agent → InstitutionalBody | study at the institution within the described scope |
| `taught_at` | Agent → InstitutionalBody | teaching at the institution in the stated capacity |
| `collaborated_with` | Agent/Organization ↔ Agent/Organization | collaboration with each party’s attributed responsibilities |
| `corresponded_with` | Agent/Organization ↔ Agent/Organization | correspondence within the documented exchange and coverage |
| `friendship_with` | Agent ↔ Agent | friendship as described by the identified sources |
| `conflicted_with` | Agent/Organization ↔ Agent/Organization | conflict over specified issues within the stated period |

The shared `social-relation-claim` schema requires the attributed statement,
wording language/script, relation basis, social scope and historical time-scope
note. Unknown bounds must be stated explicitly. That note retains its source wording. Sortable dating uses a
TemporalAssertion. A symmetric relation connects both parties while preserving
each source’s perspective and the parties’ attributed responsibilities. No predicate is transitive; none
creates a second reverse Claim, an influence edge or global membership closure.
Negation, dispute, counterevidence and source qualifications stay with each
Claim. Assessment and admission use their separately delegated routes.
Correction retains earlier source versions; changing the referent, identity
criterion, type or permission remains outside ordinary description correction.
Reader rollback does not remove new source records or their creation history.

## Intellectual formations: schools, traditions and movements

Entity registry v14 adds the abstract identity family `IntellectualFormation`
and three source-described profiles: `IntellectualSchool`,
`IntellectualTradition`, and `IntellectualMovement`. A school has a specified
teaching/inquiry lineage; a tradition has historical transmission through
reinterpretation and discontinuities; a movement has a shared historical
direction or undertaking. Each formation has its own historically situated continuity criterion.
Existing Tradition/SchoolTradition navigation IDs retain their meanings.

`intellectual-formation-record` composes common source metadata, description
scope and content-language fields. All profiles require a formation account
and their own lineage, transmission or orientation account. The common
`formation-account`, `formation-scope-note` and `formation-identity-criterion`
properties inherit through the abstract family. Each profile's specific
content property is queryable by semantic ID. Source-copy forms carry the label together with complete declared scope and
content. Source-visible assessment evaluates the account’s quality.

The existing corpus metadata reader supplies `source.create`, correction,
forms, catalog and both graph carriers; no new executable or per-kind reader
branch is selected by these declarations. Correcting wording retains subject
identity and exact predecessor bytes. Changing its continuity criterion,
referent or kind is not an ordinary description correction.

Relation registry v13 adds four non-transitive reified predicates:

| Predicate | Subject → object | Required distinction |
| --- | --- | --- |
| `intellectually_associated_with` | Agent/Organization → IntellectualFormation | source-attributed intellectual association within a stated scope |
| `school_in_tradition` | IntellectualSchool → IntellectualTradition | grounded historical placement of the school within the tradition |
| `movement_reworks_tradition` | IntellectualMovement → IntellectualTradition | reworking of the tradition through specified changes |
| `formation_articulated_in` | IntellectualFormation → IntellectualObject | articulation in the specified intellectual object and textual scope |

All require a statement with wording language/script, relation basis,
intellectual scope and historical time note. Scholar reports and research
interpretations remain distinct assertion layers. The scope is queryable as
`claim-formation-scope`; existing Claim basis and time-note properties are
reused. Direction and reverse reading remain explicit without creating another
Claim. Unknown historical limits remain explicit in the source wording. Substantive
assessment, admission, canon, publication and access rights retain their owner
routes. Rolling back the derived reader leaves their
authored records and histories intact.

## Documents and declared source claims

Relation registry version 43 adds three catalogue-field attributions: `document_catalogue_date` (Document → TemporalAssertion),
`document_catalogue_origin` and `document_catalogue_destination` (Document →
Place). Letter inherits this Document domain. The latter two retain
`identity-relation-v1`; their separate predicates and required field roles
prevent origin/destination collapse. Each location predicate has the specific Document → Place endpoint pair.

`tos_document_catalogue_claim_v1` requires a qualified statement and
`qualifiers.catalogue_attribution`: an `evidence_ref` present in the Claim's
evidence, the original `source_field` label, an exact `field_role`
(`assigned-date`, `origin`, or `destination`) and original `source_wording`.
These fields record attributed catalogue declarations. Source review checks
the cited catalogue and evaluates the attribution. Date wording must match the
whole value's `source_wording`, including language. Catalogue-field spelling retains the provider’s original wording.

Only `document-catalogue-temporal-v1` reads the date value. Its role is
`catalogue-assigned-document-date`; date, interval and explicit unknown reuse
the established elementary time-field contracts without a historical role or
relative anchor. Unknown calendar/year numbering stay null and cannot become
comparison keys. Historical dating/place profiles and grants retain their
old domains. Composition, dispatch, receipt, commissioning and event-location Claims
require their own evidence and owner grammar. Catalogue attributions preserve
what the identified catalogue reports.

The date reader has separately discoverable creation/revision delegations;
old public v1–v4 and owner-local grants do not acquire it. Place attributions
use the existing exact-predicate identity delegation. The owner configuration separately delegates creation of Places and Claims. See
[TOS-D-0063](../../../docs/decisions/TOS-D-0063-document-catalogue-attributions.md).

Entity registry version 8 adds Document under the broader IntellectualObject
root and Letter under Document. Their ancestry remains IntellectualObject → Document → Letter. Their
descriptions, versions and source-copy forms use `source-metadata-record` and
`document-record` schemas through the existing profile reader and separately
delegated `source.create`. Correcting description does not change the referent.
Unknown participants are possible; language, genre and a missing dispatch claim each retain their own source meaning; the declared
Letter type remains stable. Native manuscript carriers
keep the Artifact adapter.

Relation registry version 7 declares `source_claim_profile` on concrete
evidence-bearing relations. One `source-claims.jsonl` source stream format
serves these profiles. `identity-relation-v1` reads an exact source schema
route plus the common source-claim record contract, enforces the relation's
specific domain/range with type ancestry, and retains the full source Claim.
The catalog, graph, ordinary semantic catalog and inspection expose the same
declaration and source ref. The common reader consumes each new predicate’s declaration and source schema.

Version 34 adds the existing Collection-to-Work `contains_work` relation to
this declared reader. Introduction is still restricted to the independently
delegated Collection membership compound; readable profile support does not
permit standalone writes or accept the membership account.

| Predicate | Subject → object | Distinction retained |
| --- | --- | --- |
| `correspondence_sender` | Letter → Agent/Organization | the attributed sender, with authorship and other responsibilities separately described |
| `correspondence_addressee` | Letter → Agent/Organization | the intended recipient in the source account |
| `document_carried_by` | Document → Artifact | physical carrying of the document by the specified Artifact |
| `historical_document` | HistoricalSituation → Document | association with the stated historical reconstruction |
| `document_concerns_work` | Document → Work | source-attributed identification need not be a literal mention in the document |
| `authored_by` | Work/Document → Agent | authorship stays distinct from sending, possession or intended readership |

The existing authorship relation is extended to Documents without retyping
the seven existing Works, changing their source claims, or inventing a second
author relation. Its inverse Russian wording now covers both Works and
Documents. Competing attributions are separate Claims; the number of authors
is not limited by the one-subject/one-object structure of one Claim.

Shared source-claim metadata preserves evidence, counterevidence, maker,
provenance, confidence, exact assessment refs, alternatives, qualifiers and
unknown extensions. Its `unreviewed` carrier flag records the source posture; current admission
comes from the assessment journal.
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

The `identity-relation-v1` reader handles declared identity endpoints. Literal
and temporal values use the specific readers described in their sections.
Source-visible assessment evaluates the qualified Claim and grants scoped
admission. A separately delegated [source-owner `claims.create`](../../../mechanics/growth-cycle/parts/branch-growth-cycle/README.md#declared-source-claim-creation)
now writes initial bounded Claim batches using these same profile rules,
exact input bindings and atomic source publication. Claim revision and assessment use their separately selected command
configurations and exact source dependencies.

## Physical artifacts: existing-source adapter

Entity registry version 6 maps the existing `tos.artifact.*` identity to
`tos.entity.artifact`, identifying a physical source object through its native
witness record. The native
`artifact-witness.json` v1/v2 records remain authoritative and unchanged; the adapter reads their native schema directly.

The catalog's optional `artifacts.jsonl` binds `artifact_id` to `record_id`
without changing its value, and records the exact source schema and canonical
source digest. The first declared custody inventory number supplies an attributed navigation
label with its `label_source_pointer`.
`identity_status=null` explicitly means that these source schemas have no
Corpus identity-assessment field; the artifact's separate native
`authority.review_status` remains intact. Null is not admitted for ordinary
Corpus catalog entries.

The existing graph and access focus/inspection routes expose the full native
record, exact copies of its identity-boundary note and review/visibility
metadata, and the pointers behind those display fields. Custody, dates, inventory schemes, reported joins, visual links, genre and
planting refs retain their recorded context. Grounded relationship Claims have
their own source route. Display language and script remain unknown when
undeclared. Adjacent forms use the [native material-witness
adapter](../HUMAN_FORMS.md#native-material-witnesses-and-scholarly-composites)
and its exact native identity binding; form assessment follows the shared
assessment contract.

The corpus-index source-navigation reader uses that same adapter, replacing
its previous ID-only planting placeholder while retaining the exact navigation
node ID and authored planting edges. Both navigation and claim-graph carriers
map to the same artifact type and persistent entity ID. Their existing `projects` relation connects these representations of the same
persistent source ID.
Relation registry version 6 explicitly includes Artifact in the range of
`grounds-source-backlog-anchor`, matching the existing artifact alternative in
`philosophy-source-planting.schema.json`; unrelated types remain outside that
range. This structural planting link returns to the source-backlog anchor.
The same registry version maps the three existing historical families in
source-navigation as well as source-claims. The navigation reader validates
their exact source schema, ID, family, digest and visibility before using the
same adjacent-form materializer. Both carriers expose the historical type and source-bound forms.

Unknown schema versions, nonpublic metadata, source/catalog mapping drift,
duplicate IDs, symlink paths and records above 1 MiB are refused. Refusal does
not delete or silently normalize source. The catalog and graph builders can
regenerate this disposable adapter; rolling back the reader leaves the
physical-source records and any newer research untouched. The native source
validator still owns artifact/rights/provenance reference closure. Media acquisition, rights decisions and artifact writes use their respective
source-owner routes. Evidence-bearing document/carrier relationships, artifact growth,
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
| BiographicalEpisode → HistoricalEvent | Bounded occurrence and its documented relevance within a biography. |
| BiographicalPhase → HistoricalSituation | A described life phase with its boundary basis and internal variation. |
| HistoricalPeriod → HistoricalSituation | A situated periodization through specified developments or configurations, with its basis. |
| HistoricalGeneration → Identity | A historical cohort and the criterion that identifies its members. |
| HistoricalEnvironment → HistoricalState | A scoped configuration and at least one substantive political, economic, cultural, religious, educational or scientific-technological account. |
| LifeCircumstance → HistoricalState | A documented condition, its relevance and evidence limits; bodily circumstances, when supplied, retain their attribution and evidence limits. |

The six environment domains are independently discoverable content properties. A record need not claim knowledge of all six;
unfilled domains remain unknown, not absent. These records have historical identities. Inheritance supplies compatible
vocabulary and operations; assessment evaluates the historical account.

[`historical-context-claim.schema.json`](../../contracts/historical-context-claim.schema.json)
requires a language-tagged statement, relation basis, context scope and time
note. Unknown time bounds stay explicit in the prose; sortable dates use the
temporal Claim contract. Registered domain/range and inverse labels are executable for:

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

These relations are nontransitive. Influence, causality and chronological
containment require separately grounded Claims. Participation and
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
each operation requires its selected owner grant. Both source
readers retain all account fields and unknown extensions. Tests protect the
profile/schema boundary, relative-date anchoring, reverse navigation and
language-selected forms. Synthetic examples check contract behavior; source review evaluates real
historical content and assessment quality.

## Reception, historical recognition and later life

Entity registry version 20 adds five source profiles under the abstract
`reception-history`, itself a HistoricalSituation. The shared
[`reception-record.schema.json`](../../contracts/reception-record.schema.json)
requires an attributed reception account and receiving context, in addition to
the source metadata, research scope and continuity criterion. Its fields are discoverable, inherited properties; explicit Claims describe
relationships.

| Profile | Required distinction |
| --- | --- |
| ReceptionProcess → HistoricalProcess | `engagement_basis`: documented practices of reading, response or transmission. |
| HistoricalCanonization → ReceptionProcess | `selection_basis` and `authority_scope`: criteria and authority of the particular historical community. |
| HistoricalForgetting → HistoricalProcess | `evidence_boundary`: support for diminished transmission in the receiving context; missing catalog rows do not establish forgetting. |
| RediscoveryEpisode → HistoricalEvent | `prior_access_boundary`: whose access or attention was renewed, with its historical scope. |
| IntellectualLegacy → HistoricalState | `transmission_basis`: continuity, transformations and gaps in the documented transmission. |

Correcting an account preserves the historical referent. Splitting a process,
changing its identity criterion or identifying another community's episode is
not an ordinary description correction. No profile requires one universal
periodization or claims that silence proves a total historical absence.

Relation registry version 19 adds `receives`, `historically_canonizes`,
`historically_forgets`, `rediscovers`, `legacy_of` and `reception_carrier`.
Each has a concrete domain, typed targets and inverse reading. Targets include
intellectual objects, specified thought profiles, agents and intellectual
formations; rediscovery additionally supports physical artifacts. The carrier relation identifies an intellectual object or artifact conveying
reception. Target and research evidence retain their own roles.
All six use reified, non-transitive scholarly-report Claims through the
existing historical-context Claim schema. Statement, language/script, relation
basis, context scope and time-scope note are mandatory. The time-scope note preserves its wording; historical dating Claims provide
structured dates. Participants and places reuse existing HistoricalSituation
predicates. A receiving community can remain a qualified description until a
separate identity and participation Claim are warranted.

The existing `source.create`, `record.revise`, `claims.create`, `claim.revise`
and source-copy form commands operate these profiles without a new reader or
write permission. Both graph carriers retain the complete original record and
unknown content extensions; human packets retain their bound context. Tests
cover required content, type and layer errors, historical versus ToS authority,
creation, exact retry, correction, prior versions, property filters and forms.
Source-visible assessment evaluates historical judgments and wording quality.

The [Pennsylvania-tablet source reading](../../review-ledger/2026-09-07-reception-source-reading.md)
now supplies a scoped access episode and critical reception process through
these commands. Their [local reader review](../../review-ledger/2026-09-07-reception-profile-review.md)
keeps the existing artifact, ancient transmission cluster and modern scholarly
Work separate, with three same-origin, unreviewed Claims and no admission.

## Structured values and textual survival

Registry version 16 adds `structured-value-v1` to the declared source Claim
reader. Each structured value follows its declared kind and schema. Each profile specifies one
`value_kind`, mapped to exactly one concrete subtype of `tos.entity.literal`,
and an exact local schema route; its subject must belong to a specific
identity or semantic family. That kind is immutable across profile revisions.
An incompatible meaning requires a successor ID and explicit migration.

The shared value contract requires the declared kind and nonempty wording
with explicit language/script, including honest unknowns. A profile schema
cannot weaken it. Permitted unknown fields survive as uninterpreted data.
Only the declared reader contract interprets temporal values, geographic
identities and identity dependencies; other field names retain their ordinary
data meaning. The established temporal
reader keeps its own stronger grammar and explicit anchor dependency.

`textual_survival` connects IntellectualObject to a Claim-scoped
TextualSurvival value. `complete`, `fragmentary`, `not_extant` and `unknown`
describe the **reported text scope**, with mandatory scope and coverage notes.
Confidence, admission, access rights and the survival of a particular physical
copy each retain their separate fields and Claims. A quotation or reconstruction does not establish
complete survival of the original. Unknown status does not mean absence;
competing reports and corrections preserve their own Claim lineage.

The literal stays separately focusable through its Claim; two Claims carrying
equal values do not acquire shared subject identity. Exact wording supplies
its source-language name and summary; the enclosing assertion remains
inspectable, with evidence, attribution, uncertainty and current assessment
limits. `tos.property.textual-survival-status` is queryable through the ordinary
semantic catalog and filters. The common reader supplies these value properties and their complete source
context.

Exact graph traces also bind each literal to the governing Claim context in
its own delivered node. Focusing a value cannot drop the Claim's negation,
qualification or uncertainty. This applies to legacy literal carriers too;
their source payload and the distinction between value and Claim remain
intact.
The final-node cache binds that context as an explicit dependency.

Creation and value correction require separate v3 grants in the
[source command](../../../mechanics/growth-cycle/parts/branch-growth-cycle/README.md#declared-source-claim-creation).
V1 identity/descriptive and v2 temporal permissions are not widened. Source,
catalog, graph and assessment input readers preserve the exact value and
source bindings; source-visible assessment evaluates the value’s historical content.
Rolling back a derived reader does not erase the new sources or corrections.

## Qualified motif proposals and explicit member dependencies

`occurrence_motif_proposal` relates one focal Occurrence to a qualified
`motif-proposal` value. The **Claim ID** identifies the stable candidate; the motif value carries that
Claim’s proposed grouping and meaning.
Equal values in different Claims do not merge the candidates. Every declared
member is an exact Occurrence with its own native TextUnit binding. The focal
must be a member; it supplies an entry to the whole proposal, with every remaining occurrence
retained.

The new `structured-reference-value-v1` reader makes one fixed slot,
`/object/members`, explicit through the profile's `object_reference_set`.
The registry declares specific member types, finite bounds and whether the
subject belongs to the set. All members must resolve through their real source
profiles and become mandatory Claim dependencies. Other fields, nested
`members`, apparent IDs and extensions are inert. The older
`structured-value-v1` remains entirely non-reference-bearing; merely adding
a member-looking field to old data does not activate this interpretation.

The motif value supplies proposed signification, grouping basis, source scope,
contrast and limitations. Its `source_wording.wording_kind` explicitly records `research_paraphrase`. Exact witness quotations
retain their separate anchor-bearing `supporting_quotes`. The full statement
and source-copy form carry the entire qualified Claim as mandatory context.
The hypothesis may be disputed or uncertain without losing its addressability.

The source graph emits a separate Claim-to-member structural return for every
member, retaining the governing Claim and digest. `tos.relation.claim-value-member` returns each member to the governing Claim
and its assessment context.
Each member can be focused and followed back to the same complete hypothesis.
Compact Claim reading requires every declared member node and edge; missing
one keeps the Claim unfolded with an explicit incomplete-context reason.
The compact focal-to-value line is only an entrance to this full context.

Creation and member-set correction require separately versioned grants:
public v4 and confidential-source v2. Every member, including the focal in its
member role, needs explicit object-reference scope as well as an exact allowed
object value. Subject permission alone is insufficient. Changing the set is
an object correction, never an unchecked qualifier edit. The focal and Claim
identity remain immutable; removing the focal requires an explicit successor
or reformulated proposal, not silent reassignment of the existing Claim.
Source-visible assessment must read the entire set, its exact native grounds
and the proposed interpretation together. Semantic admission and promotion require their independently delegated
assessment routes.

The initial motif profile allows two to eight members within the existing
bounded native reader. This is the current bounded execution limit. Larger
sets need an explicit bounded continuation/storage contract; truncation must
not masquerade as a complete hypothesis. Sign issuance has the separate bounded
route below; competent source-visible assessment evaluates the motif’s historical or
philosophical merit.
The rationale is [TOS-D-0056](../../../docs/decisions/TOS-D-0056-claim-scoped-reference-values.md).

## Sign after a qualified candidate

An authored `tos.entity.sign` uses `tos_sign_description_record_v1` in
`sign.json`, read by the common semantic metadata profile into the source
catalog, graph, focus, inspection and source-copy forms. Its identity belongs to one exact concrete candidate, retained in its
immutable promotion basis.
Its immutable `promotion_basis` retains the Claim version/digest, policy,
assessment references, full source closure, journal snapshot and limitations.
This is historical issuance evidence with `grants_current_use: false`.
Description corrections do not change that basis or repair withdrawn judgment.

The [Sign command](../../../mechanics/growth-cycle/parts/branch-growth-cycle/README.md#sign-issuance-through-the-shared-source-command)
executes the doctrine's candidate-before-Sign rule. It accepts a qualified
public motif Claim under separately delegated `sign.promote`, exact native
reading and fresh competence-scoped assessment for `sign-promotion` use.
The registry's `creation_gate` prevents generic public or private creation
from minting Sign IDs. Sign issuance requires its specific promotion assessment. Identity equivalence
and canon retain their own review routes. Policy v2 and existing
source/journal locks preserve scope, independent review and atomicity.

An existing native semantic-annotation-v2 `sign` remains its own source-owned
record and ID, now exposed through `tos.entity.annotation-sign`; the adapter retains its native description and the original reviewer’s
attribution. Native IDs stay reserved against authored-profile collisions.
Both human forms and technical inspection preserve limits and the original
issuance context. Current use still needs its own fresh judgment.

The command’s supported input is a qualified public motif Claim. Synthetic
tests check the source and command contracts; real issuance requires the
competence and assessment grounds described above. The exact historical candidate remains
in the source record and is traversable through the version view below, with its original assertion context and exact version.

### Exact record-version views

`tos.entity.record-version` is a derived evidence view, distinct from the Claim,
its subject and the source record's persistent identity. Its native ID is
`record-version:` plus SHA-256 of the canonical exact `{id, version, digest}`
reference (UTF-8 JSON, sorted keys, no spacing, Unicode retained, finite values).
Two Signs with the same exact basis share the same derived version view.
The structural `promotion_basis_version` relation runs from Sign to this view;
reverse traversal reads “Sign issued from this exact record version”.

The [version-view contract](../../contracts/record-version-view.schema.json)
retains the exact pointer with `available`, `missing`, `stale`, `corrupt`,
`access-restricted` or `over-budget` status. The source builder uses the bounded
[public Claim reader](../../../mechanics/growth-cycle/parts/branch-growth-cycle/README.md#read-only-exact-claim-versions)
to bind current catalog metadata, original stream bytes and the complete
retained correction chain. An available result holds the whole selected record,
including unknown fields, and distinguishes `current` from `historical`.
An unavailable result withholds record bytes and provenance rather than
substituting the latest version, dropping the reference or opening private data.

The access reader validates the transported reference, content digest and
separate identity without reading source archives or authority configuration.
It accepts only the contract's closed `navigationNode` envelope: extra generic
wording, review or authority fields are rejected, not silently discarded or
interpreted. Original extensions belong inside the digest-bound source record.
The structural relation must match the exact candidate retained by its Sign;
two correctly typed endpoints alone cannot authorize a different version.
Full inspection retains the record and byte/archive provenance. Compact packets
retain the exact reference, availability and complete assertion context;
source-authored statement wording is quoted in its own language. Navigation titles identify the derived version view. Historical versions use
their retained wording; current Claim HumanForms stay bound to their own exact
version. Archived free-form wording and other record families remain separate
extensions. Fixity establishes the returned bytes; current use and canon each
require their applicable owner judgment.

## Independent genre, content form, medium and file format

Entity registry 21 and relation registry 20 add four Claim-scoped
classification values through the existing `structured-value-v1` reader.
The [classification schema](../../contracts/source-classification-claim.schema.json)
requires the term and its language/script, source wording, classification
basis and scope. Classification terms remain open to further source-described distinctions. The
abstract ClassificationValue supplies the common property contract; each value
remains scoped to its Claim.

| Predicate | Subject → value | What remains distinct |
| --- | --- | --- |
| `classified_genre` | IntellectualObject → GenreClassification | literary or scholarly genre attributed to the subject |
| `classified_content_form` | IntellectualObject → ContentFormClassification | letter, article, lecture, aphorism or commentary as a described content form |
| `classified_communication_medium` | IntellectualObject → CommunicationMediumClassification | written, spoken, performed or audiovisual expression in the stated context |
| `classified_carrier_medium` | Artifact / Item → CarrierMediumClassification | papyrus, codex, clay tablet or printed book as a carrier category |

Genre and form can share a term such as “dialogue” without collapsing their
different questions. A Work can take letter form while retaining its Work identity; an existing
Letter can receive a form classification on that same record. Carrier
category, measured material composition and intellectual membership each
answer their own question. Membership in a school, tradition or movement uses
its separately grounded relation.

The pre-canon atlas navigation types `tos.entity.genre` and `tos.entity.medium`
retain their identities and mappings. Atlas promotion retains its explicit review route. Two Claims with the same value retain distinct values,
evidence and assertion contexts; negation, competing classifications and
uncertainty remain visible. The predicates have no transitivity or global
cardinality limit. Each fixes exactly one value kind and specific subject
families; a value from a neighboring facet is rejected.

The catalog exposes `tos.property.classification-term`,
`tos.property.classification-term-language`,
`tos.property.classification-term-script`,
`tos.property.classification-basis` and `tos.property.classification-scope` on
these values, inherited from ClassificationValue. Term language/script, classified-subject language and source-wording language
have separate declarations. Combine their ordinary
property filters with the named predicate and focus through the Claim to find
the classified subject. Inspect the Claim for attribution and evidence; assessment evaluates the complete attributed classification. Source wording supplies
the honest source-language display, with fallback rather than invented
translations; full statement forms retain the qualified assertion.

Technical format uses the existing File record instead:
`tos.property.file-media-type` reads the exact source-item manifest's declared
`payload_files[].media_type`, already carried by source navigation as
`attributes.media_type`. The filter is applicable only to File. Missing MIME
metadata stays unknown, and a Work with a similarly named field does not
match it. This property reports manifest metadata. Payload verification and access use
their owner routes; literary genre has its own classification predicate.

Creation and correction use the existing separately delegated v3 Claim value
commands, exact dependency/version checks and retained history. A compatible facet uses the common reader through its registry and schema
declarations.
Rolling back a reader never removes the source Claims or assessment history.
The bounded actual inputs and source-reading limits are recorded in
[`2026-09-07-classification-source-reading.md`](../../review-ledger/2026-09-07-classification-source-reading.md).

## Scoped lexical translatability

Entity registry 26 and relation registry 25 add `lexical_translatability`
through the existing structured-value reader.
Its subject is a Lexeme, a situated LexicalSense or an Occurrence with its
existing exact native TextUnit binding. The value stays scoped to its Claim, including when another Claim carries an
equal value. A correction
of this report preserves the subject and retains prior Claim versions.

The [source contract](../../contracts/source-lexical-translatability-claim.schema.json)
separates these questions:

| Field | Meaning and limit |
| --- | --- |
| `source_language`, `target_language`, `source_scope`, `target_scope` | The languages, usage scopes and situated translation task. |
| `aspects_in_scope`, `aspect_transfer` | Reported full, partial, no or undetermined transfer of the specified aspects only. |
| `rendering_judgment` | Adequate, inadequate or undetermined for the declared task. |
| `renderings_considered` | Explicit wording alternatives, each with its own language/script and considered scope. |
| `preserved_aspects`, `limitations` | Attributed account of what survives and what is lost, unexamined or unknown. |
| `search_report` | Null when no report is supplied; otherwise the reported outcome, sought criterion, coverage and optional method account. |

`none_found` is only a reported search outcome for its declared criterion and
coverage. It does not imply linguistic impossibility and may coexist with
partial renderings considered while searching for a fuller one. Conversely,
a task can judge partial transfer adequate. The schema therefore does not
equate these axes or infer the enclosing Claim's polarity from their values.
Missing search information, a supplied report with an undetermined outcome,
and a report of no matching result remain distinct. An actual search execution retains its own provenance and receipt.

Source wording, the qualified statement and each candidate rendering retain
their separate language/script declarations, including explicit unknowns.
All scoped scalar fields are discoverable through
`tos.property.translatability-*`; the complete alternative wording packets
remain inspectable in the value. Unknown extension members remain uninterpreted source data.

This predicate is distinct from `lexical_translation_correspondence`, which
compares two separately identified Senses. Translation activities, translated Works and accepted correspondences each
have their own source and assessment routes. The general
v3 exact-value delegation owns creation and correction; ordinary form commands
copy the complete qualified statement with the entire Claim as mandatory
context. Source-visible competent assessment evaluates substantive quality and grants
scoped admission.

## Textual fragments and quoting passages

The independently mapped scholarly-composite route below retains the modern
reconstruction object; it must not be substituted for either passage identity.

Registry version 17 distinguishes an addressable `textual-fragment` from a
`quotation-passage`. Both are intellectual identities using the existing declared metadata reader.
Their descriptions require substantive notes, an explicit scope/continuity
criterion and language-bearing content. The fragment records its account and
boundary basis; the quoting passage records its quotation and location
accounts. An editorial number identifies the passage within the edition’s own
designation system. Original boundaries and correspondences with other
editions require separately grounded accounts.

A quoting passage is identified in its containing context. The same words
quoted in another place do not create the same passage. A quotation is not
necessarily an exact reproduction: selection, translation, interpolation,
quotation/paraphrase uncertainty and attribution limits belong to the source
account and its Claims. The quotation-passage record identifies the intellectual portion in its
containing context; bibliographic references and quoting activities have their
own descriptions.

| Source predicate | Forward reading | Inverse reading | Required source account |
| --- | --- | --- | --- |
| `fragment_of` | textual fragment belongs to an intellectual object | object has the proposed fragment | boundary and survival limits |
| `quotation_in` | quoting passage is located in an intellectual object | object contains that passage | containing context and reported locator |
| `quotation_preserves_fragment` | quoting passage transmits the proposed fragment | fragment is transmitted through that quotation | transmission, coverage and attribution limits |

These are separately identified, evidence-bearing Claims with a required
statement, language/script and scope note. They retain reified, nontransitive Claim topology. Competing identifications and preservation accounts
may coexist; no unique-container cardinality silently accepts one claim over
another. The normal focus operation supports either endpoint and the Claim
center, retaining its complete context.

`source.create`, profile metadata correction, source-copy human forms and
Claim create/revise use the existing owner commands. Shared fields retain
unknown content, with assessment under its separately selected owner grant.
The four fragment/quotation content properties are discoverable in the
semantic catalog; the common catalog and readers expose them by property ID.

These profiles describe intellectual portions and their reported locations.
Acquisition, transcription, normalization, segmentation and exact source
anchoring use the versioned source-witness layer. Exact anchors retain their
mechanical resolution; reconstructions retain their editorial method and
source grounds.
Scholarly reconstruction remains with the scholarly-composite source route;
its existing physical-member profile must not be populated with fictional
artifacts to fit textual transmission. Compatible textual-composite coverage
requires its own explicit owner extension and validation.

## Scholarly composites: existing source adapter

Registry version 18 maps the native `tos.composite.*` identity to
`tos.entity.composite`, identifying an editorial reconstruction as an
intellectual object. Native `tos_scholarly_composite_witness_v1`
records remain unchanged in `scholarly-composites/`. The bounded adapter reads
only that owner's `composite-witness.json` files and preserves their exact
record, digest, identity status, preferred label, editorial description,
provider observations, members, coverage, rights and authority limits.

The catalog maps the exact source-owned record into read-only delivery. Both
source-navigation and source-claim carriers expose the same persistent ID;
the scene maps them to one vertex while inspection retains the carriers.
Witness membership, reconstruction assessment, ancient recension and
historical dating each require their own source-grounded Claims and review. The native metadata description and adjacent source-copy
forms are available through the [native human-form adapter](../HUMAN_FORMS.md#native-material-witnesses-and-scholarly-composites).
Forms bind the native `composite_id` and the exact unchanged record. Language remains unknown unless separately established;
description assessment uses the shared source-visible route. Native subjects/forms can be
explicit source-bound assessment inputs without becoming admitted knowledge.

Unknown versions, nonpublic metadata, identity/schema/digest drift, duplicate
identities, unsafe paths, duplicate JSON keys and over-budget records are
rejected by the native source route. Rolling back the reader leaves the source
records intact. An ordinary declared metadata profile cannot replace this
native namespace; the v1 physical-member observations must not be populated
with invented artifacts.

### Descriptive composite growth alongside native witnesses

Registry version 19 explicitly retains `scholarly-composite-v1` while adding
the common-metadata `tos_scholarly_composite_record_v1` source profile to the
same `tos.entity.composite` and `tos.composite.*` namespace. Both supported record shapes describe the same scholarly-object type and
retain their native storage contracts. Both use `catalog/composites.jsonl`; duplicate
current identities across either shape fail before creation or graph reading.
Changing the retained adapter is an incompatible profile change.

New descriptive records use
`scholarly-composites/<method>/<tradition>/<identity>/composite.json`.
The shared source command enforces this owner home for creation, correction
and form operations. It uses the ordinary declared-profile `source.create`,
`record.revise` and human-form contracts, including exact dependencies,
predecessors, unchanged referent scope and idempotent retry. Native `composite-witness.json`, its forms and assessment inputs retain their
own adapters.

`semantic_content` requires `composition_account`, `editorial_method` and
`coverage_account`, separately discoverable as `tos.property.composite-*`
properties. Their values are optional at the general composite type because
native records retain their own fields, but mandatory in this exact new
schema. `semantic_scope` states the referent and continuity criterion;
historical existence, reconstructed original, compiler, containing publication,
members and their order require separately grounded Claims. Executable transformation provenance and edition review retain their own
records, linked to the described method where applicable.
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

Physical witness membership, passage sequence and the editor’s source
inspection each require their own account. The compiler role identifies
responsibility for this composition; ancient-poem and containing-book
authorship retain their separate attribution.

## Historical situations: source profile

Registry version 4 introduces `historical-event`, `historical-process`, and
`historical-state` under the abstract `historical-situation`. These source-described historical identities have their own referents and
continuity criteria. Authored semantic `event`/`state`, claim-scoped
`provision-activity` and provenance events retain their existing source
contracts. Duration alone never changes an event into a process or state.

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
not itself grant admission. The assessment route supports an authorized, competent human or agent
reviewer.
The compatibility `review_status=unreviewed` describes the initial source record. The assessment journal owns current
scoped admission; that journal state remains outside this historical graph
adapter’s output. Four registered predicates are executable (relation
registry version 5 adds historical dating):

| Predicate | Domain → range | Qualification |
| --- | --- | --- |
| `historical_participant` | historical situation → Agent or Organization | `qualifiers.participation_role` is required source wording describing the participant’s role |
| `historical_place` | historical situation → Place | the attributed location of this situation |
| `historical_work` | historical situation → Work | the attributed topical association with this Work |
| `historical_dating` | historical situation → TemporalAssertion value | the attributed historical time of this situation |

A dating value has an explicit `kind` (`date-assertion`, `interval-assertion`,
`relative-order`, or `unknown-date`), `role=historical-time`, `calendar`,
`year_numbering`, `certainty`, and exact `source_wording` with its language
(which may be unknown). A date uses `value`; an interval uses `interval.start`
and/or `interval.end`; a relative date uses `relative.relation` (before, after,
during, overlaps) and a resolved historical `anchor_ref`. Unknown dates have
none of those absolute/relative values. An open bound records an unspecified endpoint.
Additional uninterpreted fields live in `extensions` and survive inspection.

The declared `historical-temporal-v1` source Claim reader now carries this same
value grammar in `source-claims.jsonl` using `tos_source_temporal_claim_v1`.
It requires a historical-situation domain and a temporal-assertion range. The new schema
composes the existing `historicalDate` definition rather than replacing the
legacy historical adapter. It adds a source-authored qualified statement with
explicit language/script for the common Claim forms. Date profile extensions
cannot weaken the shared value grammar. Newly registered dating predicates
use the same source, assessment, graph and access readers, including relative
anchor navigation; the common reader consumes their declarations.

The [shared command](../../../mechanics/growth-cycle/parts/branch-growth-cycle/README.md#declared-source-claim-creation)
creates and corrects these values only with separate v2 exact-value delegation.
It preserves source/evidence bindings, uncertainty, original attribution,
immutable predecessor bytes and all current source-copy forms. Each temporal value stays scoped to its governing Claim. The v1 identity-only creation and
descriptive-only correction grants retain their old scope. Legacy
`historical-claims.jsonl` remains under its original adapter and does not gain
revision history by reinterpretation.

For example, this **synthetic test** value
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
assertion contexts remain mandatory in compact delivery. This display copies the source wording with its temporal assertion context.

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
source-visible assessment evaluates whether the reported event happened. Private historical metadata is
refused, not silently projected or dropped. Other historical predicates are
not admitted by this carrier until their owner contract is implemented.

The ordinary access catalog, focus, type-ancestry filters, Claim inspection,
and adjacent metadata-form reader consume these records through the common profile contracts.
The source-form command adapter can prepare/create/revise those adjacent forms.
Separately delegated historical subject creation and record correction now
use the [source-owner command route](../../../mechanics/growth-cycle/parts/branch-growth-cycle/README.md#initial-historical-subject-creation).
The selected owner configuration supplies the operation’s grant. Date sources remain explicit: historical dating Claims, witness dating, capture
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

## Languages, varieties, scripts and transliteration schemes

The declared `linguistic-description-record` profile keeps four concrete
research subjects: `Language`, `LinguisticVariety`, `Script` and
`TransliterationScheme`. Each requires substantive content, declared scope and
referent-continuity criteria, source references and explicit description
languages. The scope and continuity criterion identify the referent independently of
changing names or codes. A broad writing tradition describes its repertoire,
internal variation and historical coverage.

`LinguisticSystem` groups languages and varieties for endpoint typing. Each
source account states the criterion it uses to distinguish them. The grounded,
nontransitive `dialect_of` and `historical_language_stage_of` Claims preserve
their different criteria and scope; their historical criteria require their own source grounds.
The atlas `language_script` navigation category retains its existing identity
and mapping.

`inscription_language` connects an Artifact or Item to a LinguisticSystem;
`inscription_script` connects the carrier to a Script. Each Claim names the
particular inscription and limits in mandatory `attestation_scope`, alongside
its statement, source language, grounds, evidence and review posture. The attribution covers the inscription and limits named in
`attestation_scope`. Missing, competing and negated attributions remain distinct. Work-language assignments and script assignments each require an explicit
supported relation and evidence.

`transliteration_source_script` and `transliteration_notation_script` distinguish
the source writing from the notation basis of a named convention. The notation basis may include additional numerals and metacharacters.
Executed transliteration, text layers, pronunciation, linguistic segmentation,
translation and alignment retain their versioned text and annotation
contracts.

All four kinds use `semantic-metadata-v1` and the common
[source.create / record.revise / form commands](../../../mechanics/growth-cycle/parts/branch-growth-cycle/README.md#declared-profile-subject-creation).
Relations use separately delegated `claims.create` and correction, the common
semantic Claim reader, exact expected versions and idempotent receipts. Shared
access catalog, type/property filters, focus in either direction, inspection
and source-copy human forms use these shared profile contracts.
Content properties and inherited scope/continuity properties are discoverable
by semantic property IDs. `semantic_content.language/script` describe the account’s wording. Language
and script attributed to the researched object use their own Claims.

The [bounded source reading](../../review-ledger/2026-09-08-linguistic-source-reading.md)
grounds six provisional subjects and seven unreviewed Claims across Akkadian,
Sumerian, Old Babylonian, cuneiform, Latin and ORACC ATF, linked to the existing
Penn CBS 07771 and Louvre AO 5473 artifacts. The inspected Penn page supports
language only; the Louvre page supports both language and script, with French
source values even on its English URL. The ATF account explicitly retains its
indexed-text/direct-access limitation. These provisional research accounts retain their limited reading scope.
Ancient-language interpretation, sign reading and translation require
separately qualified assessment.
The ATF notation Claim's initial report-layer label was corrected through a
separate exact layer transition to `linguistic_analysis`, preserving version 1
and the unchanged proposition; no source reading was retroactively promoted.

The focused linguistic contract test in `test_source_witness_bibliographic_graph`
covers mandatory content/scope, wrong endpoint types, language/script swaps,
no embedded admission, equal labels with distinct identities, unknown-field
retention, exact form context and inverse focus. Source and reader measurement
results live in the [profile review](../../review-ledger/2026-09-08-linguistic-profile-review.md).
The [text layer law](../CORPUS_FOUNDATION.md#text-bearing-layers) remains stronger
for actual transcriptions and transformations. Reader rollback does not remove
new subjects, source corrections or their histories.

### Lexemes, written forms and contextual senses

The [lexical description contract](../../contracts/lexical-description-record.schema.json)
extends the existing semantic metadata reader for lexical descriptions. Its three referents remain distinct:

| Referent | Source kind / ID | Required content |
| --- | --- | --- |
| lexical grouping | `lexeme` / `tos.lexeme.*` | lexical and grammatical accounts, scope and continuity criterion |
| written representation | `lexical-form` / `tos.lexical-form.*` | exact declared `form_identity`, form account, scope and continuity criterion |
| situated lexical reading | `sense` / `tos.sense.*` | reading, interpretive context, semantic range, scope and continuity criterion |

`tos.entity.lexical-sense` retains its existing semantic identity and legacy
`source-navigation` mapping. The metadata kind `sense` uses `tos.sense.*`. Written lexical forms use
`tos.lexical-form.*`; human display packets use `tos.form.*`; legacy computed
groupings retain `lexical-form:sha256:*`. Each route keeps its own referent
and identity grammar. Existing native lexical records, exact occurrences and
historical packets retain their adapters.

Native semantic-packet entities retain their IDs even when their content is
not publicly projected. The common profile reader and catalog reject a
standalone subject using an already occupied native ID, including existing
record/form/assessment readers and exact creation retries. Initial creation,
record correction, form configuration and assessment snapshots bind an opaque
fingerprint of the exact native metadata inventory. Added, removed or changed
packets invalidate pending commands; the protected command readers recheck
inventory membership and bytes. Historical requests, receipts and predecessor
bytes are not rewritten or compared to current dependency fingerprints on an
exact replay; the current subject must still satisfy the identity guard.
The check exposes only the inventory fingerprint and identity-availability
result; hidden packet bodies and locators remain private. Existing-record commands inspect that inventory
only for the native v2 identity spaces (`occurrence`, `lexeme`, `sense`, `sign`,
`concept`); unrelated selected Document/Language records do not acquire a
private-corpus dependency. Catalog/creation still reserve the complete native
inventory. A native contract expanding those identity spaces requires an
explicit adapter transition. The local inventory is bounded to 1024 metadata
packets of at most 1 MiB each and 8 MiB in total, refusing unsupported schemas or overflow rather
than silently treating uninspected IDs as free. These are the current operation budgets; broader corpus indexing requires a
separately bounded route.

`form_identity` contains the supplied string, declared language and script,
representation kind, notation scope and Unicode posture. It is immutable
through the ordinary descriptive revision grant; no case folding, Unicode
normalization or string-based ID is performed. A matching string may still
belong to another referent or unresolved homograph. Description corrections advance the record and its source-bound forms while
preserving the subject, scope meaning, frozen form identity and earlier
history. Scope wording correction requires the explicit public v2 grant. An incompatible referent needs
the explicit identity-transition route, not a revised spelling field.

`lexical_form_of`, `lexical_sense_of` and `lexeme_in_language` are separate,
grounded, nontransitive Claims with concrete endpoints. Each requires a
relation basis, attestation scope and qualified statement. They may record
scholarly reporting, linguistic analysis or semantic interpretation without
conflating those layers. There is no global one-form/one-lexeme or
one-lexeme/one-sense constraint: rival assignments and negations can coexist.
Resolving rival assignments requires examining their evidence and complete
assertion contexts.
The source-visible assessment route owns judgment and scoped admission.

The ordinary `source.create`, `record.revise` and `claims.create` operations
apply with their separate exact owner grants, version/dependency checks,
idempotent receipts and rollback history. The entity registry supplies the profile; the selected owner configuration
supplies the write grant. The common reader returns the
unchanged record through both source navigation and source Claims, with
one shared subject identity. Typed property filters expose the lexical,
grammatical, reading, range and written-representation fields; source-owning
human forms preserve the full semantic context and `form_identity`.
Description wording and lexical form each retain their own language
declaration. The common delivery and inspection readers expose the same model.

The [bounded JGB reading](../../review-ledger/2026-09-08-jgb-lexical-source-reading.md)
provides the first real lexical subjects and linguistic-analysis Claims.
They are provisional research records with the stated JGB reading scope and
assessment posture. Exact Occurrence, TextLayer,
Anchor and TextUnit remain the native text-evidence owner's next route; an exact token reference requires that native text-evidence binding. Reader rollback retains these sources and their operation history.

### Lexical history and translation comparison

The [lexical comparison contract](../../contracts/lexical-comparison-claim.schema.json)
adds source Claims to the existing lexical subjects and common command plane.
Each comparison retains the existing lexical identities and its own qualified
Claim. These predicates have distinct source meanings:

| Predicate | Endpoints and reading | Required specific account |
| --- | --- | --- |
| `lexical_inherited_from` | later Lexeme → proposed predecessor Lexeme | `chronology_basis` describing proposed historical inheritance |
| `lexical_borrowed_from` | borrowing Lexeme → proposed donor Lexeme | `chronology_basis` describing the proposed borrowing and its scope |
| `lexical_formed_from` | formed Lexeme → proposed lexical base | `chronology_basis` describing proposed lexical word formation |
| `lexical_cognate_with` | Lexeme ↔ Lexeme | `common_origin_basis` describing the proposed shared origin |
| `lexical_sense_developed_from` | later situated Sense → proposed earlier Sense | `chronology_basis` describing the proposed historical sense change |
| `lexical_translation_correspondence` | source Sense → proposed target Sense | `translation_scope`, `preserved_aspects`, `limitations` |

Every Claim also requires the common qualified statement, relation basis and
attestation scope, plus separate `source_scope`, `target_scope`,
`source_language` and `target_language`. These languages describe the compared usage scopes; the statement and source
record declare their wording languages separately. Unknown language remains
null or explicitly undetermined. Language identities, attested assignments and
competence grants each use their own records. Scope and basis fields retain
their descriptive content.

No predicate is transitively closed. Cognacy is read symmetrically within the one recorded Claim and its two
endpoints. No
one-base, one-predecessor, one-sense or one-rendering cardinality is imposed.
Several proposed formation bases or rival etymologies remain separate Claims
with their own evidence and qualifiers. This formation route covers Lexeme-to-Lexeme relations; morpheme analysis
requires its own source representation.

Chronology wording preserves the source’s order, bounds and uncertainty.
Correcting it advances the Claim record and retains its prior version.
Historical change and record correction each retain their own time and
provenance. Historical dating and normalized time queries use their separate
source contracts.

A proposed translation pair relates two situated senses within the declared
translation scope. Its preserved aspects and limitations must remain in the human
reading context. A missing target Sense, absent pair or negative Claim does
not establish universal untranslatability. The lexical-translatability value described above records whether an
equivalent was found, how fully a use can be rendered and the reported search
scope.

The common `claims.create`, `claim.revise`, source-copy forms and qualified
assessment routes retain their separate grants and exact snapshots. All
comparisons have concrete endpoint types and source-visible evidence. The
independently selected assessment scope must cover both compared languages,
the description language, task and risk; source qualifiers cannot supply that
authority. Assessment evaluates each comparison from its source grounds and qualified
statement.

Discovery exposes the nine comparison fields through
`tos.property.claim-lexical-*` property IDs on Claim nodes. The ordinary reader
returns the complete source Claim and its unmodified unknown fields; a
source-copy statement retains that Claim as mandatory context. The common readers retain the complete value and Claim context.
These are open Claim properties: a field filter alone does not assert that a
matching Claim uses one of these six predicates. A lexical-only selection also
names its relation/predicate condition.

### Exact-bound occurrence descriptions

The [Occurrence profile](../../contracts/occurrence-description-record.schema.json)
describes a particular use through the existing semantic metadata reader. Its
`native_text_binding` is an immutable return to a native TextUnit, segmentation,
ordered Anchor set and frozen TextLayer, retaining the exact references to those evidence objects. The description has its own `tos.occurrence.*` ID. Required
`occurrence_account`, `context_account`, scope and continuity criterion retain
the researched meaning and uncertainty; unit kind and proposed boundary status
stay with the native packet. The native unit’s declared kind remains visible, including sentence and
paragraph units. Existing native semantic occurrences keep their IDs and adapters.

The source profile explicitly declares `native_binding_adapter:
source-text-unit-v1`. Missing adapters, unknown native schemas, wrong IDs or
versions, changed bytes, wrong ordered anchors, broken source/rights closure
and nonpublic bindings fail closed. The common public catalog/form/revision
readers validate metadata without opening text. Initial `source.create`
additionally verifies the exact, separately public UTF-8 representation.
`public_content_declared` reports the declared visibility and
`content_verified` reports exact content verification. Publication
authorization and linguistic assessment use their owner routes. A public
description of a private unit is refused even when it contains no quotation:
a lexical join or short-span digest can itself disclose private content.

Description correction preserves `native_text_binding` and the semantic
scope’s meaning; correcting scope wording requires the explicit public v2
grant.
A different source use requires explicit identity handling. The same native
unit may support competing descriptions; neither its address nor a shared
spelling merges them automatically. All human name/hover forms retain the
binding as required context alongside scope and semantic content. The shared
readers preserve unknown content and extensions, through the common profile readers.

`occurrence_has_form`, `occurrence_of_lexeme` and `occurrence_has_sense` are
separate, grounded linguistic-analysis Claims to a written form, Lexeme and
contextual sense respectively. Each requires a qualified statement, relation
basis and attestation scope. They are nontransitive and have no global
one-reading cardinality rule. Source-visible linguistic assessment evaluates each assignment against the
exact binding. The common source-Claim contract also accepts an explicit
`polarity` of `positive`, `negative` or `unknown`; omission leaves polarity unspecified. Polarity is separate from uncertainty, dispute and admission.
Opposed propositions use distinct Claim identities; ordinary descriptive
correction cannot flip this identity-bearing field. Source-visible assessment
checks agreement between polarity and wording; quoted source wording retains its exact text.

An occurrence and any source-bound form or Claim selecting it require an exact
read of that same **complete binding** through the explicit v3 native selection
before scoped assessment admission is usable. An unrelated unit, metadata-only
selection or a past successful creation does not satisfy this gate. Metadata
inspection remains available without text, but cannot recover a usable old
admission by switching back to v2. This derived source-read gate is separate
from access permission, policy qualification and the immutable review history;
see the [assessment command contract](../../../mechanics/growth-cycle/parts/branch-growth-cycle/README.md#native-textunit-return-and-assessment).

Public release of DTA, eKGWB or operator-held IA text requires the applicable
rights and publication decisions. The existing real lexical descriptions remain provisional;
their private source returns are not replaced with fabricated public packets.
Private authored occurrence storage, real admitted linguistic analysis, native
writer operations and UI consumption remain explicit foundation work.

### Identity transition proposals

The [identity-transition Claim](../../contracts/source-identity-transition-claim.schema.json)
records a **proposed** merge (two to eight predecessors to one
successor) or split (one predecessor to two to eight successors). Every endpoint
already has an independently created source identity and is frozen as an exact
`{id, version, digest}` reference. The complete mapping, grounds, counterreading,
scope and unresolved related Claims are mandatory parts of the proposal.
Choosing a focal predecessor supplies an entry point; the complete mapping retains every predecessor and successor in its declared
role.

`identity-transition-v1` is a distinct reader. The relation's polymorphic domain
is narrowed to a concrete source-mapped `object_role: identity` whose exact
metadata version is available through the owner's typed reader. A bare Identity
ancestor, an ID prefix, a submitted type or an unavailable adapter does not
qualify. Ordinary structured-value and reference-value grants do not authorize
this reader. The subject creation route remains separate from proposal creation.

The separate [subject identity-transition Claim](../../contracts/subject-identity-transition-claim.schema.json),
predicate `subject_identity_transition_proposal` and `identity-transition-v2`
reader add declared semantic subjects without changing v1. In addition to the
v1 source identities, v2 accepts only concrete public subject metadata profiles
that explicitly declare `identity_proposal_adapter: exact-semantic-metadata-v1`.
The selected profile must retain `semantic-metadata-v1`, its exact kind/ID/file
and schema routes, and source-claims and source-navigation mappings. Each frozen
endpoint must resolve with the matching owner-derived typed descriptor, exact
record version/digest and canonical public metadata path. Semantic ancestry,
shared spelling, a submitted descriptor, an opaque annotation packet, an abstract
type, Claim, literal, private/payload-only record or unsupported profile is not
an endpoint capability. Future types must opt in through their source contract.

The declared semantic description families include concepts and conceptions,
lexical subjects, language/script descriptions, inquiry and reasoning subjects,
Occurrences and already-issued Signs. This is proposal eligibility only:
Occurrence native bindings and Sign issuance/promotion bases stay immutable,
and neither binding nor Sign status transfers to another participant. A proposed
successor must already exist through its own creation or promotion route.
No new semantic subjects, equivalences or transition acceptance are introduced
by enabling this adapter. V1 owner grants cannot dispatch v2 proposals, and v2
grants do not silently replace v1 or authorize ordinary Claims.

Corrections retain the proposal Claim ID, but freeze operation, membership,
exact endpoint versions, mapping and predecessor proposal. Changed participants
or topology require a new Claim and an exact `supersedes_proposal`. Its legacy
`supersedes_claim_ref` is only an ID navigation companion and must match that
exact reference. This is **proposal succession**, distinct from the proposed
predecessor/successor subjects. An unresolved link is an exact referenced Claim
and an authored question, never permission to migrate that Claim's endpoints.
A v2 successor proposal may cite an exact v1 or v2 predecessor proposal; a v1
proposal may cite only v1. Both historical schemas and reader routes remain
available, with no retrospective broadening of old proposals or grants.

The existing high-risk `identity` assessment profile remains unchanged: research
use requires two reviewers, two independence groups, two supporting origins,
counterevidence search and no self-review. Current assessment must see all
frozen exact subjects and related Claims selected by its independent owner
configuration. A later source version cannot stand in for a frozen version.
Assessment admits a bounded research reading of the proposal. Execution would
require its own supported contract and grant. Source-copy forms keep the
complete qualified Claim as required context; each revision needs assessment
bound to that exact version.

Old subject IDs still resolve to their original source records. Member and supersession edges return to the reified proposal and its exact
source context. No
`same-as`, link migration, actual merge/split, source deletion, source-ID reuse,
publication or canon operation is implemented by this proposal route.
