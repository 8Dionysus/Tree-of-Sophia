# Human forms of the same knowledge

A human form supplies wording through which a person can identify, read or
inspect a subject. It has its own versioned record and binds the exact subject
and source context. Its `tos.form.*` ID
remains stable through corrections. A change of subject identity requires
another form identity; changing wording, corrected role/language metadata or
exact authored source dependencies creates a successor version with an explicit
predecessor. A translation receives its own form; the source owner retains both wordings
and their derivation relationship.

The roles are `name`, `caption`, `hover`, `statement`, `grounds`, `history` and
`technical`. The roles describe the function of each wording. Multiple competing forms may
coexist, with their suitability assessed for the intended reading. Each
descriptive role requires substantive wording.

## Source and context

Each form binds an exact subject record and exact source fields through record
ID, version, canonical JSON digest and JSON Pointer. Record digests follow [knowledge assessment](KNOWLEDGE_ASSESSMENT.md),
alongside separate raw-file fixity. Source-copy includes the entire addressed
string, preserving its conditions and negations. Interpreters and tools handle
source text as data.

The source owner supplies mandatory context separately from the proposed form.
This includes material negation, uncertainty, conditions, attribution, dispute,
time and scope, according to the exact subject contract. Every qualifier required by the source owner must remain bound to the form.
Unknown context members and false, zero, null, an empty value and absence remain
different. The source adapter and substantive assessment determine which fields are
material.

The materialized form includes both wording and mandatory context. When
`standalone_reading` is false, consuming the wording alone is outside the
contract: caption, hover or exact reading must retain the context or offer a
non-assertive reference to a fuller form. A shorter authored form requires review of its meaning and context. Delivery
that exceeds the context budget returns `over-budget` and an explicit route to
complete inspection.

## Three production modes

### Readable governing context

The existing entity-type registry owns one versioned `context_presentation`
vocabulary, under this doctrine. It supplies finite multilingual labels and explanations for context fields;
authored source wording and review results retain their respective owners.
The normalized reader may attach `tos_readable_context_v1` without changing
the raw HumanForm materialization, subject record or assertion context.
The existing knowledge catalog exposes the exact vocabulary payload, identity,
version and canonical digest under `context_presentation`; its `source_revision`
binds that catalog to the graph snapshot. Consumers use that exact versioned vocabulary.

Every readable entry binds its exact raw value by a carrier JSON Pointer and
the original record ID/version/digest or assertion-context source digest.
Parent assessment, snapshot and linguistic context retain the exact complete
materialization binding. Those envelopes preserve the reading context and its assessment provenance. Source language/script is copied only from an explicit
source declaration; label language describes the label itself.

The companion's deduplicated `exact_materials` carries existing canonical JSON
text, its UTF-8 SHA-256 and actual carrier `origin_pointers`. This preserves
numeric distinctions and large integers through readers whose JSON numbers
otherwise lose precision. Source record digests and the normalized graph's existing IEEE-754 framing
retain their separate definitions. Verify each text hash and
raw origin. Resolve a record-bound value by the record digest and source pointer;
resolve assertion and materialization values by their exact value pointer under
a listed origin. Read and render numeric values from the verified text, preserving number
lexemes. Ordinary JSON parsing checks transport correspondence; preserved lexemes
establish the exact numeric representation. All material shares the same companion budget; refusal empties both
entries and exact materials.

Only enumerated owner-known mechanical fields may enter `technical` details.
Governing scope, negation, conditions, attribution, conflicts, source language,
unknown members and unknown enum values remain visible with their raw values.
An unknown value is `unclassified` and retains its raw representation. False,
zero, null, empty values and absence remain distinct. Labels accompany the complete mandatory context and preserve the form’s
standalone-reading status.

The vocabulary's explicit schema selectors include the retained historical
Claim carrier as well as the shared native Claim profiles. Their same declared
fields receive the same labels whether read directly from an exact Claim or
through its assertion context. This vocabulary labels fields while preserving their source values and
qualifications. A `claim_ref` remains governing context:
it identifies the assertion being discussed. Unrecognized
schemas and extension fields remain unclassified; schema support requires an explicit selector.
The separately declared `tos_document_catalogue_claim_v1` uses these same
finite Claim labels. Its whole catalogue attribution, selected source field,
wording and unresolved calendar remain governing source values with their original attribution.

`complete` reports coverage of the returned context. Source quality,
assessment and admission have their own fields. The bounded companion
refuses partial readiness: excess context produces `requires-exact-context`
with exact root pointers; invalid bindings produce `unavailable`. Raw packets
and historical references remain unchanged. A changed vocabulary requires a
higher presentation version; vocabulary and processor changes invalidate the
computed companion before source content changes. An older consumer must retain the exact raw context or explicitly decline the
companion.
Compact lens packets omit the optional sidecar because they omit its exact raw
source roots. Full inspection retains both; selected HumanForms keep their own
mandatory packet context. Each packet’s sidecar requires its own exact raw roots. Qualifications remain
with the source even when a compact packet omits the optional sidecar.

### Language and linguistic derivation

An optional `language_context` binds one exact source-owned metadata object
with `language`, `script`, `relation` and `source`. The relation distinguishes
`unknown`, `original`, `translation`, `transliteration` and `adaptation`.
Original describes linguistic derivation relative to this subject and owner
scope. Historical priority, authenticity and authorial state require their own
evidence. The reader preserves unknown metadata until the source owner
supplies a declaration.

The submitted binding must equal the separately supplied `FormScope` binding.
The metadata and, for a derived linguistic form, its complete source wording
are explicit form bindings, current dependencies and mandatory output context.
Additional source qualifications survive intact. `original` and `unknown`
have no derivation source; the other relations require an exact source field.
A form cannot translate itself. Form language and script must match the
metadata. Language, linguistic derivation and the rendering operation remain
separate: a source-copy can copy an already translated field; a freeform
translation still requires current competent assessment.

The materializer verifies reference closure; linguistic assessment evaluates
the declaration. Its source owner supplies the access-filtered, appropriately assessed
metadata; the selected owner scope controls its use. A correction creates a successor form binding and invalidates affected
assessments. No current source declaration means the original-role reader
reports unavailable. Readers can select an explicitly declared original
without adjudicating competing originals or certifying historical priority.
Linguistic provenance is carried by the context envelope. Human wording can
concentrate on its subject while the complete packet retains the template’s
required semantic guards.

### Rendering operations

- **Source-copy:** copy one complete, nonempty source field. Source-bound
  language and script metadata are supplied separately; absent metadata stays
  unknown rather than borrowing the interface language. Substantive assessment separately evaluates the statement and its prose
quality.
- **Template:** use one exact owner-admitted template, declared language and
  role. Its finite literal/slot sequence has only text and JSON-value rendering;
  execution is limited to the declared literal and slot sequence. Every
  required context binding must actually occur in the rendered slots. The
  owner assesses a template's meaning and domain once; mechanical repeat
  applications need no new per-record human signature. Template admission records the owner’s assessment separately from schema
validation.
- **Freeform:** a summary, translation or other newly composed wording needs
  current competent assessment under the `human_projection` layer of the
  assessment policy. The judgment targets the exact form, including all its
  dependencies. The materializer calls the existing policy engine with trusted
  authority, competence, execution binding and history; current policy and exact dependencies determine the form’s admission.

The authenticated source adapter owns maker identity, current subject and
source snapshot, risk, languages, requested use, access decision, mandatory
context and admitted template set. These inputs are resolved independently of the form submission. Source correction, changed template, revised policy, revoked
authority/competence or withdrawn assessment require fresh materialization;
previous forms and reviews remain in history. Publication, rights, consent and
canon retain their respective owner routes.

## Executable route and limits

`ToS/contracts/human-form.schema.json` and `human-form-template.schema.json`
define the source records. The pure owner mechanic is
`mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py`.
It returns `ready`, `invalid`, `unavailable`, `stale`, `restricted`,
`needs-assessment` or `over-budget`. `ready` reports successful rendering. Acceptance of the represented assertion
is recorded by its own assessment. Rejections and disputes remain in the separate
assessment result. Source-only and template rendering report their mechanical result with
assessment kept separate. An explicitly assessed source-copy instead reports its own current
form admission, without changing its production mode or treating the copied
Claim as admitted.

Rendering is a deterministic, in-memory operation. The materializer bounds
the selected input at 8 MiB, 512 source records, 64 templates and 256 predecessor
forms, and the output at 64 KiB; it refuses truncation. The schema bounds
bindings and template segments. These ceilings bound resource use; latency requires separate measurement. Persistence, ownership authentication and publication are
the source command adapter's responsibility; access only receives the derived
read contract. Current tests cover rendering, exact dependencies, context,
language, revision, refusal and agent-policy revocation. Source adapters,
real-language calibration, all-corpus forms and actual UI consumption remain
required work in [the foundation coverage map](FOUNDATION_V1.md), with evidence recorded separately from mechanic tests.

The source-bound assessment journal exposes a local `materialize-form`
operation for stored source-copy and freeform forms. It supplies current authenticated
policy/history and exact selected source records to the same pure materializer;
authority comes from the protected owner configuration. This lane requires complete
subject context and retains predecessor forms from their exact adjacent set.
For a source-copy, the copied field, role, language and script must also match
the actual source-owned field catalogue; assessment remains within that catalogue’s field and language declarations. The separate source-only writer/metadata reader
continues to return mechanical readiness with `admission: null`.
For existing source-copy forms the assessed owner supplies additional whole-subject
context as `owner:subject` when the field's authored context did not already
include it. This trusted input adds whole-subject context while preserving field guards,
stored bindings and form identity; admission follows the separate policy
check.
The exact subject remains a dependency and contributes to the output budget.
Linguistic context, when used, is separately selected by the source owner.
See the [command contract](../../mechanics/growth-cycle/parts/branch-growth-cycle/README.md#assessed-form-materialization).
This operation produces a current local materialization. Public graph
publication has its own owner route. The metadata-only graph adapter below still
refuses unassessed freeform wording.

In the explicit assessed lane, current native quality context is resolved by
the source owner separately from immutable authored form bindings. Each whole
current basis is an exact dependency and mandatory output context, under an
`owner:quality:<index>` slot which cannot be an authored binding name. The
admission carries inherited source limits together with the form's own limits.
The dependent review must cite the current source and basis references: a changed basis invalidates the old review and requires renewed assessment;
unchanged wording retains its version. An already authored explicit basis binding is
still exact; a newer owner-resolved basis cannot silently rebind or rescue it.
Missing, stale or denied inputs emit no wording. The protected owner configuration selects this mode, and the current policy
derives `can_use` from the independently resolved basis.
Freeform remains assessment-required outside this mode too; template admission
is not extended by the assessed source-form route.

For a v5 form of a Claim, the owner additionally selects that Claim's exact
same-use assessment scope. `subject_assessment` retains its current admission,
limits, journal observation and relevant historical withdrawal refs, under
the form/parent/quality lock set. This is mandatory reading context alongside
the whole raw Claim, whose initial `review_status` is not current assessment.
A positive form admission permits presentation of the Claim with its own
status: an unreviewed, rejected, disputed or withdrawn parent remains
explicitly visible as such. Parent history changes invalidate a previously prepared materialization
snapshot while preserving immutable wording; re-reading shows
the new status without inventing a new parent assessment. Missing or denied
parent read scope refuses this v5 route. The parent's judgment and its limits
remain separate from the form's own admission and current layer-quality gate.
Consumers must retain this companion and expose the form’s and Claim’s
separate statuses. The optional closed-schema
extension requires updated `human-form.schema.json`; older strict consumers
must reject it rather than strip it. Existing public freeform and source-only
metadata paths do not acquire a confidential parent-journal reader.

### Local assessed research snapshots

The existing bibliographic-graph and corpus-index builders can additionally
receive one explicitly constructed `AssessedFormSnapshot`. Its protected
configuration and selected form IDs come from the source owner, never from a
graph node, source text, query, model response or stored ready packet. The
adapter replaces only the selected forms on their existing exact source
carriers. Source and form references and paths must match the independently
resolved journal inputs. The selection preserves the existing subjects, relations, claims and source
records.

Each selected form is freshly materialized through the journal command. A
second collection compares the same source/configuration snapshot and committed
journal heads before returning the graph. A source, grant, assessment or
time-dependent admission change aborts assembly rather than mixing old and new
packets. The issuer must keep source/configuration inputs stable for assembly,
as with the underlying command. This observes a vector of committed heads; its validity is scoped to the captured snapshot and each subject’s committed
transaction. Missing or negative assessment still
returns a nonready form without wording, rather than blocking unrelated source
copies. An explicit selection that cannot resolve fails visibly.

The materialization's `assessment_snapshot` carries the owner-snapshot digest,
journal revision and batch count, with `publication_authorized=false` and
`current_runtime_grant=false`. Configuration and journal filesystem paths stay in the protected owner
context. Wording and complete required context stay in the same packet; the
64 KiB packet and 256 KiB per-carrier form-set limits include this binding and
refuse truncation. At most 256 form IDs may be selected per owner snapshot.
The source-claim and source-navigation carriers deliver the same packet through
the common reader and focus operation; access performs no assessment command.
When these two carriers represent the same subject, the common reader requires
identical source bodies and selected assessed packets. Mixing ordinary and
assessed carriers, different journal observations or different wording fails
assembly. Python and Worker consumers reject malformed snapshot annotations;
these checks validate transport; fresh grants and substantive review remain
with the assessment owner.
The optional `assessment_snapshot` field extends the closed materialization
schema. Consumers using an older copy must update the schema before accepting these
local packets, preserving the complete annotation.

Assessed source-copy and freeform packets use the same snapshot transport.
When a packet contains the separate parent `subject_assessment`, the observation
also declares `subject_assessment_required=true`. Losing either side fails the
reader instead of producing a standalone positive form. Python and Worker check
the exact parent subject, policy/use agreement, journal shape, withdrawal refs,
limits and non-endorsement boundary without re-evaluating a policy or treating a
parent rejection as rejection of its attributed wording. Unknown admission
details remain intact; an oversized companion yields an inspection reference with the complete limits
available at the source. This additive observation member requires updated readers;
confidential v4/v5 inputs retain their separate protected route.

This optional input produces a **local research candidate**. Context and assessment limits still need their own public-safety
and artifact-consumer clearance before publication or runtime connection. The
ordinary deterministic builders and source-parity checks use no private owner
configuration and keep their existing output. See the
[local builder commands](../../mechanics/growth-cycle/parts/branch-growth-cycle/README.md#local-assessed-graph-builds).
The local CLI requires a separate new JSON target outside repository sources
(or within private `.git` state), atomically creates it with mode `0600`, and
never replaces an existing file. `--check` compares that candidate with current
source/journal inputs without rewriting it. A failed build leaves the prior reader snapshot and all source/journal history
intact. Reader activation and public publication have separate owner
decisions.

## Native canonical nodes

The separate `tos_canonical_node_v1` opt-in under
[NODE_CONTRACT](NODE_CONTRACT.md) gives a canonical node an explicit native
`node_id`/`record_version` binding. Canonical nodes retain their native identity and schema; legacy adoption
requires an explicit source change.
An adjacent `node.human-forms.json` uses the existing form-set grammar and
retains form predecessors. First adoption of a legacy source is a reviewed source change with retained
original bytes.

The canonical field catalogue exposes `canonical.preferred-name`,
`canonical.variant-name:<index>` and `canonical.thesis`. They copy the complete
preferred/variant name or `distilled_thesis`, with roles `name` and `statement`.
Every form retains the **entire exact native node** as mandatory context,
including source anchor, interpretation layers, relations, qualifiers and
wording statuses. The wording and full node context form one reading packet. Names and statement
use source-copy production, with substantive assessment kept separate.
Unknown source field-language/script stay null; variant declarations concern
their own wording only. Language comes from explicit source declarations.

The separate `tos_local_canonical_form_owner_v1` delegation allows only the
existing `form.create`/`form.revise` operations for explicitly named form IDs
of one exact validated canonical `node.json`. It binds the native node schema
in preparation and currentness checks. Existing bibliographic grants do not
gain access to the canonical subtree. This command changes the adjacent form set. Node revision, assessment, canon
and publication have their own owner commands.

The corpus index retains the original node unchanged in `properties` and
attaches the derived form collection separately. Normalized readers bind it
to the exact canonical source digest; byte fixity of the original file remains
separate. Python and Worker use the same native identity check. Stale source or
form references remain stale, with no fallback to ID-derived wording. This is
an additive schema/adapter transition: older closed readers must update or
refuse it, not silently strip the source fields or form context. Full source
revision/history commands, assessed canonical freeform wording and full-corpus
prepared/D1 adoption remain separate work.

## Bibliographic metadata adapter

An adjacent `<record-stem>.human-forms.json` may hold a
`tos_human_form_set_v1` with current forms and retained predecessors, bound to
the exact bibliographic record. The subject retains its own record and identity. Updating the subject makes its forms `stale` without blocking other
graph objects until the source owner rebinds successor forms; older wording and exact dependencies remain in
`prior_forms`. The adjacent set stores wording and its exact source bindings.

`scripts/source_witness_human_forms.py` provides the first metadata-only
adapter: whole `preferred_label` and `variant_labels/*/value` names, and whole
`notes` hover text. Identity status and equivalence posture remain mandatory;
a variant also retains all its source metadata, including unknown members.
Semantic descriptions also retain their complete `semantic_scope` and
`semantic_content`. Written lexical forms additionally bind `form_identity`,
including the represented spelling, notation, language/script and Unicode
posture. A Russian description cannot conceal that its subject is a German
form or an unresolved transliteration; the description and its subject each retain their own language declarations.
Missing source language/script remain unknown. Other roles, templates and
freeform wording are explicitly unavailable on this adapter, until explicitly supported by the owner adapter. Declared creator identity records provenance; the command adapter
authenticates the principal.

Corpus and historical records may declare `field_languages.preferred_label`
and `field_languages.notes`, each with explicit `language` and `script`
(independently nullable). These declarations describe the exact metadata wording. A Work, Expression or
cited source retains its own language evidence. The adapter binds
the whole declaration as mandatory context, including additional qualifications;
omitting it invalidates the proposed form. A declaration without its complete
wording field is refused. Variant names retain their existing local language
and optional script fields. Tags use an extensible structural grammar, including
private-use tags; registry membership and language competence require evidence beyond the tag’s
structural grammar.

`field_languages` identifies the wording’s language. `language_context`
records originality, translation, transliteration or adaptation through exact
source-owned derivation. Adding or correcting a declaration changes
the source record version/digest; existing forms remain stale until explicit
successor bindings are made, and predecessors remain retained. Readers with
the earlier closed corpus schema must update to accept this optional additive
field; they must not strip it. The source-creation receipt retains the original serialization; successor
records and their history establish currentness.

The existing bibliographic graph builder carries the materializations and
adjacent source return in identity properties, with the set's input digest.
Materialized `dependencies` enumerate each exact record ref once. Repeated
field uses retain their own bindings/context but neither inflate independent
support nor duplicate record-level delivery cost. Different identities,
versions or digests are never coalesced merely because their wording agrees.

This read route uses public bibliographic metadata. Growth commands, payload
access and publication retain their separate owner scopes.
Each set is bounded at 2 MiB input, 32 current forms and 256 KiB output.
Every public-metadata subject in the supported source catalog is addressable
in the bibliographic reader even before any Claim refers to it. A standalone
subject carries its forms and source return; its identity and forms are sufficient for standalone navigation. Link records retain their separate object-link adapter.
The initial Jenseits set contains original-name, Russian-name and source-note
copies whose source provenance and assessment status remain explicit.
Originality requires an explicit language-context binding. The current metadata
adapter does not yet supply this context; adding it requires source-owned
metadata and successor form bindings, not a reader heuristic.

The separately delegated `record.revise` adapter can publish a historical
source correction and all selected source-copy successors together. It retains
the exact preceding package and form lineage; an old assessment does not bind
the new source digest. The operation and byte-history contract live in
`mechanics/growth-cycle/parts/branch-growth-cycle/README.md`, not in read-only
access or a new source ontology.

Public profile revision v2 (`tos_local_profile_revision_owner_v2`) additionally
allows an explicitly delegated correction of `semantic_scope` wording. The
source reviewer verifies that the scope and identity criterion still describe
the same referent, records the reason, and retains the exact predecessor and
all successor form bindings. A changed referent requires the identity route.
Existing v1 and private profile delegations retain their earlier field scope.

### Native material witnesses and scholarly composites

The same adjacent form-set grammar also binds the unchanged
`tos_artifact_source_witness_v1/v2` and
`tos_scholarly_composite_witness_v1` records. Their actual `artifact_id` or
`composite_id` and `record_version` form the subject ref; no `record_id` is
injected into their source payload. Native schema and owner path are checked
before a form command or an assessment-source selection.

For a physical witness, `metadata.preferred-name` copies the complete first
custody inventory number, and `metadata.source-note` copies `path_identity.note`.
The former preserves an attributed inventory label.
For a scholarly composite, these selectors copy `preferred_label` and
`editorial_object.description`. The name requires complete authority,
layer-separation and rights-reference fields, plus composite identity status
or artifact custody attribution. The hover requires the entire native record,
preserving members, coverage, provider observations, rights and authority
limits. This avoids repeating the whole source twice in compact delivery
without dropping the name's essential qualifications. Omitting required
context is invalid. A changed source makes
the former form stale; a restricted source emits no wording.

These native schemas have no field-language declarations. The copy's language
and script therefore stay null; source territory, ancient language, provider,
English-looking wording and interface locale cannot supply them implicitly.
Linguistic attribution, translation and independent content assessment remain
separate work. Source-copy reports mechanical readiness; substantive assessment records
semantic use.
Freeform production retains the existing assessment boundary.

Both source-claim and source-navigation readers carry the same forms and exact
subject payload. The standard local form command creates/revises only the
adjacent set, and binds the consumed native schema digest into its prepared
configuration and receipt. Source-bound assessment configuration can select
the native subject and its forms by their actual IDs and exact file digests;
selection identifies the subject for the separate assessment process. Native
source correction/creation is not granted by this form adapter.

## Source commands and retained change history

### Declared Claim statements

The shared `source-claims.jsonl` reader also accepts adjacent form sets for
each separately identified Claim. The filename is
`source-claims.<sha256-of-UTF-8-claim-id>.human-forms.json`; the exact subject
reference inside the set remains identity authority. This bounded filename
does not depend on row order, a mutable label or a user-supplied path. Moving
the source stream still requires explicit companion/reference migration.

`claim.statement` selects the complete nonempty `qualifiers.statement` string,
with nullable `qualifiers.statement_language` and `statement_script` as source
declarations. Their extensible tag grammar matches metadata forms. An absent statement field remains absent in the advertised forms.
These declarations describe the statement’s wording; witness language and
linguistic derivation have their own evidence. Invalid declared tags are refused.

The **entire exact Claim** is mandatory context, including its maker, endpoints,
predicate, layer, epistemic and initial review status, evidence, alternatives,
counterevidence, qualifications and unknown extensions. Thus a ready source-copy
is never standalone: a consumer must retain this context or return a reference
for inspection. The materializer refuses truncation and the existing bounded
delivery can withhold an oversized packet without shortening its assertion.
Templates, freeform paraphrase and derived linguistic-context admission remain
unavailable on this adapter. The Claim’s assessment and use result accompany its rendered wording
separately.

The graph builder validates the declared Claim profile, carries the forms on
the Claim node itself, and binds the adjacent file digest.
Python and Worker/D1 choose forms from `source_claim.claim_id/claim_version`
and the source digest. A carrier exposing both metadata and Claim bindings is
ambiguous and refused. This does not alter the stronger source Claim or create
a direct fact edge. Legacy Claim streams still need an explicit family adapter,
not silent native-profile coercion. Captured `historical.create` v2 packages have
a separately delegated descriptive historical Claim/form adapter in the
[Growth command route](../../mechanics/growth-cycle/parts/branch-growth-cycle/README.md#captured-legacy-historical-claims).
It preserves the historical schema, initial creation evidence and independent
record/Claim histories. Uncaptured legacy packages remain outside that writer.

The separate `tos_local_claim_form_owner_v1` Growth delegation uses the common
form commands below, selected by one exact `claim_id`. Its original creation
receipt remains distinct from subsequent form receipts. The optional
`growth_history[].source_contracts` retains exact registry/profile/schema input
digests; these also contribute to the expected configuration digest. Older
closed-schema readers must update or explicitly reject this additive receipt
field, never discard it. Historical receipts are not retroactively populated.

The three real letter-705 Claim statements have source-copy forms. Their
wording remains Russian with unspecified script, their assertions remain
unreviewed, and their materializations grant no semantic admission.

### Explicit compact Claim fields

A Claim may opt into `qualifiers.display_fields.schema_version =
tos_claim_display_fields_v1`. The source contract
`ToS/contracts/claim-display-fields.schema.json` defines optional `name`,
`caption` and `hover` wordings, each with explicit text, nullable language and
nullable script. It requires the complete statement and its language/script
declarations. Bounds of 160, 320 and 2048 Unicode code points are authoring
limits, that require a complete authored wording within each limit. Unknown markers and unversioned values
remain uninterpreted source data; a malformed understood version is refused.

`claim.name`, `claim.caption` and `claim.hover` copy only their complete declared
fields. As for `claim.statement`, the **entire exact Claim** is mandatory
context and the result is not standalone. Names should identify the Claim; the complete statement carries its
proposition. Captions and hover wording must preserve
material negation, attribution, uncertainty, dispute and scope. Source-visible
authoring/review owns that judgment; schema validity and mechanical source-copy
readiness cannot approve a misleading abbreviation. Source correction, form
revision, substantive assessment and scoped admission remain separate events.

The existing `tos_local_claim_form_owner_v1`, private Claim grants and compound
creation grants retain statement-only source-copy authority. The separate
`tos_local_claim_form_owner_v2` requires an exact `allowed_field_ids` list and
permits only source copies. Claim-correction grants may explicitly add
`allowed_form_field_ids`; absence means statement-only. This option does not
replace the separate date/value/layer grants or widen their predicate scope.
Preparation, raw application and retry check the selected field and the exact
current/retained predecessor. Revoking a name field cannot be bypassed by a
retry whose resulting form has already changed to a statement.

Source validation and command dependencies bind the display schema when its
marker is understood. Changing wording advances the Claim and all explicitly
rebound forms together, retaining source and form predecessors. No existing
record or unknown extension is automatically migrated. Ordinary HumanForm
delivery budgets still apply: a ready source materialization does not prove
that a multi-role packet fits compact access or has been accepted by the UI.
The rationale is [TOS-D-0064](../../docs/decisions/TOS-D-0064-scoped-claim-display-fields.md).

### Source-owned Claim navigation

The relation registry may declare one versioned `claim_navigation_template`
with the `claim-navigation-v1` reader. Its finite literal/slot syntax produces
a **navigation descriptor** for locating the Claim through its predicate,
endpoints and declared statuses. This distinction also applies
to legacy Claim streams without an adjacent form adapter. Their original
records, missing statements and unreviewed material remain unchanged.

The six mandatory slots name the Claim-record marker, exact predicate label,
declared epistemic and initial review statuses, subject label and object label.
Every rendering contains each slot exactly once and starts with the record
marker. Languages are explicit and case-insensitively unambiguous; default
selects an existing rendering, never an invented translation or source language.
The initial template supplies Russian and English navigation syntax, while
endpoint names remain whole, exact source strings. Status wording describes the source record’s declared status; later assessment
results remain separately identified.

Only a unique concrete reified predicate mapping with understood domain/range
is eligible. Without an explicit object adapter, both endpoints must be
identities. An exact mapping label takes precedence;
a relation-family label is usable only when that entry has one Claim-predicate
mapping. Endpoint names bind `/preferred_label`, or the native artifact adapter's
explicit `/custody/inventory_numbers/0`. An arbitrary carrier pointer, ID-derived
name, path fallback, shortened name or guessed personal-name expansion is not
eligible. An unavailable descriptor records one explicit reason: predicate,
object kind, endpoint type, source name, source status or predicate language
not understood/available, or output over budget. No partial title is substituted.

The descriptor binds the exact template/version, full source Claim/version,
used predicate entry and mapping, and both endpoint records/versions/names by
canonical JSON digests (or the explicit typed-value adapter binding below).
It does not depend on unrelated registry entries.
The Claim's entire qualifications, alternatives, evidence and unknown fields
remain source context. The field catalogue supports navigation. Substantive reading requires the
statement’s full negation, time, conditions, attribution and dispute context. To read those, inspect the exact Claim
and its available source-copy forms. Compact reading therefore stays missing
when navigation is the only wording.

The source exporter creates this separate carrier property. Access independently
checks its finite rendering and all bindings against supplied raw records and
the current registry before normalization or cache reuse. Display provenance
marks `navigation-template` and `source_title_available: false`; compact packets
retain the descriptor's exact Claim/template references. An older carrier without
the property retains its missing-title state; canonical source-backed export
validation still detects a stale or stripped projection. Source-name edits
invalidate dependent Claim and relation display tasks without text extraction.

Changing syntax or vocabulary requires a higher template version. The existing
previous/current registry validator checks that transition and rejects silent
repurposing of template identity, reader, purpose or owner. Removing or changing that contract incompatibly requires an explicit
migration.
See [TOS-D-0057](../../docs/decisions/TOS-D-0057-source-owned-claim-navigation.md)
for this boundary's rationale. Full legacy HumanForm migration remains separate.

Template version 2 opts into `historical-time-source-wording-v1` through the
optional `object_label_adapters` list. This adapter requires the existing
`historical-temporal-v1` source profile and an understood temporal range. It
copies only the whole `/object/source_wording/text` from a historical-time date,
interval, relative-order or unknown-date value, preserving its declared language
(including explicit null). It neither formats the normalized value nor supplies
a missing calendar, year numbering, precision or date. The literal carrier binds
the same full Claim, source file/line, Claim version/digest, exact object digest
and derived literal ID; it does not create an identity record for a value.
Canonical comparisons preserve false versus zero inside the retained value.

The result remains a nonstandalone navigation field catalogue. The original
Claim's qualifications, attribution, uncertainty, source-declared review status
and complete raw temporal object remain necessary reading context; an exact
wording string does not make a dating exact or accepted. Missing wording, an
unknown adapter/type, wrong range, ambiguous literal, stale binding or over-budget
title still produces no partial ready descriptor. A version-1 template without
the adapter retains identity-only behavior. The explicit successor rationale is
[TOS-D-0067](../../docs/decisions/TOS-D-0067-typed-time-claim-navigation.md).

Template version 3 additionally opts into
`document-catalogue-time-source-wording-v1` only for the separate
`document-catalogue-temporal-v1` profile and
`catalogue-assigned-document-date` role. It retains the same exact Claim,
literal, source-file/line and value binding, not a formatted normalized date.
The full catalogue field attribution remains mandatory context. Actual
source-copy HumanForms continue through `claim.statement`, whose complete
source-authored qualified statement is bound to the entire Claim. The short
catalogue date is not promoted to a standalone HumanForm or event assertion.
Version-2 historical navigation is unchanged; no old template silently gains
the new adapter.

### Command behavior

The source owner may delegate `form.create` and `form.revise` for explicitly
named form identities of one bibliographic subject. This is permission to
record source-owned forms, not permission to assess them, admit their wording,
publish them or change rights. Human and agent callers use the same command
grammar. The independently chosen owner configuration binds the local account,
creator, exact source route, authority reference, allowed operations/IDs and
expiry; submitted prose cannot supply or widen that scope.

`mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py`
implements this first Growth adapter. Discovery names semantic metadata fields;
preparation constructs complete source-copy bindings from the same field and
context rules as the existing bibliographic reader. Preparation writes nothing.
Application compares exact source, configuration and form-set revisions, then
commits the bounded related changes, unchanged neighboring forms, predecessors
and command receipt in one adjacent form-set file. Stable IDs stay stable;
successors advance exactly one version and cite the retained predecessor.

An optional `growth_history` in `tos_human_form_set_v1` records command identity,
input digest, authenticated account principal, delegated authority reference,
configuration digest, time, exact source and resulting form refs. Historical
sets need no fabricated receipt. An exact retry returns its historical receipt
and current reader state separately, including after source change; it cannot
resurrect stale wording. Current revocation still denies a write-command retry.
No command removes predecessors or silently truncates history.

Readers pinned to the earlier closed form-set schema must update before
consuming the optional history extension; they may reject the newer set, never
strip its receipts to make it appear compatible. A reader rollback leaves the
new source set and its history intact.

Freeform and template records can be retained as source proposals; this
metadata adapter still cannot materialize them as accepted wording. A proposed
source-copy must satisfy the actual reader, including mandatory context and
source language constraints. After a source correction, updating one form
does not update or erase the others: their unchanged exact bindings remain
explicitly stale. Neither successful preparation nor a source-write receipt
proves substantive quality or calibrated agent competence.

The atomicity boundary is one form set, not all subjects or the independently
edited source/configuration files. Cooperating command writers share a bounded
Unix lock; the issuer must keep other source/configuration writers quiescent
during the operation. Existing source records, payloads, graph exports and
read-only access remain untouched by the command. Per-set limits remain
32 current forms, 256 retained forms, 256 receipts and 2 MiB; reaching a limit
requires an explicit owner continuation design, never history deletion.
