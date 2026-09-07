# Human forms of the same knowledge

A human form is a versioned record about a subject, not the subject, its name
identity, an assertion of truth, or an admission decision. Its `tos.form.*` ID
remains stable through corrections. A change of subject identity requires
another form identity; changing wording, corrected role/language metadata or
exact source dependencies creates a successor version with an explicit
predecessor. A newly authored translation must not overwrite its source form;
the source owner retains both forms and their derivation relationship.

The roles are `name`, `caption`, `hover`, `statement`, `grounds`, `history` and
`technical`. They name levels of data, not a prescribed panel layout. Multiple
competing forms may coexist. None acquires priority because its text is shorter,
its creator is human, or its creator is an agent. Empty text, an ID and a generic
placeholder do not complete a descriptive role.

## Source and context

Each form binds an exact subject record and exact source fields through record
ID, version, canonical JSON digest and JSON Pointer. Record digests follow
[knowledge assessment](KNOWLEDGE_ASSESSMENT.md); they do not replace raw-file
fixity. Source-copy includes the entire addressed string, not an unreviewed
substring which can cut off a negation or condition. Source text is data, never
an instruction to an interpreter or a tool.

The source owner supplies mandatory context separately from the proposed form.
This includes material negation, uncertainty, conditions, attribution, dispute,
time and scope, according to the exact subject contract. A form creator cannot
decide that an inconvenient qualifier is optional by omitting its binding.
Unknown context members and false, zero, null, an empty value and absence remain
different. Deciding which source fields are material belongs to the source
adapter and substantive assessment, not a heuristic in a visual client.

The materialized form includes both wording and mandatory context. When
`standalone_reading` is false, consuming the wording alone is outside the
contract: caption, hover or exact reading must retain the context or offer a
non-assertive reference to a fuller form. A string ending in an ellipsis is not
a semantic shortening rule. A client must not silently drop context to satisfy
a character limit. If the bounded result cannot contain it, the result is
`over-budget`, not a stronger abbreviated assertion.

## Three production modes

### Language and linguistic derivation

An optional `language_context` binds one exact source-owned metadata object
with `language`, `script`, `relation` and `source`. The relation distinguishes
`unknown`, `original`, `translation`, `transliteration` and `adaptation`.
Original is relative to this subject and owner scope, not a claim that a text
is the earliest historical witness, authentic, author-final or unmediated.
Unknown metadata is not promoted to original by an ID, the preferred label,
the interface language or the fact that rendering copied a source field.

The submitted binding must equal the separately supplied `FormScope` binding.
The metadata and, for a derived linguistic form, its complete source wording
are explicit form bindings, current dependencies and mandatory output context.
Additional source qualifications survive intact. `original` and `unknown`
have no derivation source; the other relations require an exact source field.
A form cannot translate itself. Form language and script must match the
metadata. Language, linguistic derivation and the rendering operation remain
separate: a source-copy can copy an already translated field; a freeform
translation still requires current competent assessment.

The materializer verifies reference closure, not the linguistic declaration's
truth. Its source owner supplies the access-filtered, appropriately assessed
metadata; putting a declaration in a submitted record does not grant it that
scope. A correction creates a successor form binding and invalidates affected
assessments. No current source declaration means the original-role reader
reports unavailable. Readers can select an explicitly declared original
without adjudicating competing originals or certifying historical priority.
Linguistic provenance remains in the context envelope; it need not be pasted
as raw metadata or source text into a template's human wording. This does not
relax the template's separately required semantic guards.

### Rendering operations

- **Source-copy:** copy one complete, nonempty source field. Source-bound
  language and script metadata are supplied separately; absent metadata stays
  unknown rather than borrowing the interface language. This does not assess
  the copied statement's truth or certify the source's own prose quality.
- **Template:** use one exact owner-admitted template, declared language and
  role. Its finite literal/slot sequence has only text and JSON-value rendering;
  it cannot evaluate code, access undeclared fields or invoke tools. Every
  required context binding must actually occur in the rendered slots. The
  owner assesses a template's meaning and domain once; mechanical repeat
  applications need no new per-record human signature. Passing the template
  schema is not that owner admission.
- **Freeform:** a summary, translation or other newly composed wording needs
  current competent assessment under the `human_projection` layer of the
  assessment policy. The judgment targets the exact form, including all its
  dependencies. The materializer calls the existing policy engine with trusted
  authority, competence, execution binding and history; a submitted `accepted`
  flag or old receipt cannot authorize the form.

The authenticated source adapter owns maker identity, current subject and
source snapshot, risk, languages, requested use, access decision, mandatory
context and admitted template set. A form submission cannot populate these
trusted inputs. Source correction, changed template, revised policy, revoked
authority/competence or withdrawn assessment require fresh materialization;
they do not erase previous forms or reviews. Research use does not confer
publication, rights, consent or canon authority.

## Executable route and limits

`ToS/contracts/human-form.schema.json` and `human-form-template.schema.json`
define the source records. The pure owner mechanic is
`mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py`.
It returns `ready`, `invalid`, `unavailable`, `stale`, `restricted`,
`needs-assessment` or `over-budget`. `ready` means this rendering operation
succeeded; it does not mean the represented historical or philosophical
assertion is accepted. Rejections and disputes remain in the separate
assessment result. A source or template rendering has no invented assessment.

Rendering has no network calls, writes or model calls. The materializer bounds
the selected input at 8 MiB, 512 source records, 64 templates and 256 predecessor
forms, and the output at 64 KiB; it refuses truncation. The schema bounds
bindings and template segments. These are safety ceilings, not measured
latency guarantees. Persistence, ownership authentication and publication are
the source command adapter's responsibility; access only receives the derived
read contract. Current tests cover rendering, exact dependencies, context,
language, revision, refusal and agent-policy revocation. Source adapters,
real-language calibration, all-corpus forms and actual UI consumption remain
required work in [the foundation coverage map](FOUNDATION_V1.md), not facts
established by those tests.

## Bibliographic metadata adapter

An adjacent `<record-stem>.human-forms.json` may hold a
`tos_human_form_set_v1` with current forms and retained predecessors, bound to
the exact bibliographic record. The set does not change the subject record or
its ID. Updating the subject makes its forms `stale` without blocking other
graph objects until the source owner rebinds successor forms; older wording and exact dependencies remain in
`prior_forms`. The set is not an independent corpus registry or an admission
receipt.

`scripts/source_witness_human_forms.py` provides the first metadata-only
adapter: whole `preferred_label` and `variant_labels/*/value` names, and whole
`notes` hover text. Identity status and equivalence posture remain mandatory;
a variant also retains all its source metadata, including unknown members.
Missing source language/script remain unknown. Other roles, templates and
freeform wording are explicitly unavailable on this adapter, not automatically
accepted. Declared creator identity is provenance, not authentication.

Corpus and historical records may declare `field_languages.preferred_label`
and `field_languages.notes`, each with explicit `language` and `script`
(independently nullable). These describe the exact metadata wording, not the
language of a Work, Expression, cited source or interface. The adapter binds
the whole declaration as mandatory context, including additional qualifications;
omitting it invalidates the proposed form. A declaration without its complete
wording field is refused. Variant names retain their existing local language
and optional script fields. Tags use an extensible structural grammar, including
private-use tags; a structurally accepted tag is not a certification of registry
membership or language competence.

Field language is not `language_context`: it does not establish originality,
translation, transliteration or adaptation. Those still require their exact
source-owned linguistic derivation. Adding or correcting a declaration changes
the source record version/digest; existing forms remain stale until explicit
successor bindings are made, and predecessors remain retained. Readers with
the earlier closed corpus schema must update to accept this optional additive
field; they must not strip it. The existing source-creation receipt remains an
immutable account of the original serialization, not a mutable currentness seal.

The existing bibliographic graph builder carries the materializations and
adjacent source return in identity properties, with the set's input digest.
Materialized `dependencies` enumerate each exact record ref once. Repeated
field uses retain their own bindings/context but neither inflate independent
support nor duplicate record-level delivery cost. Different identities,
versions or digests are never coalesced merely because their wording agrees.

This route uses already public bibliographic metadata, not payload text or
private source layers. It has no growth-command or publication authority.
Each set is bounded at 2 MiB input, 32 current forms and 256 KiB output.
Every public-metadata subject in the supported source catalog is addressable
in the bibliographic reader even before any Claim refers to it. A standalone
subject carries its forms and source return; it does not acquire an invented
authorship, participant, date, evidence or other fact edge merely to make it
visible. Link records retain their separate object-link adapter.
The initial Jenseits set contains original-name, Russian-name and source-note
copies, not a new translation, historical assessment or complete Forms profile.
Its original-name ID is not language-context evidence. The current metadata
adapter does not yet supply this context; adding it requires source-owned
metadata and successor form bindings, not a reader heuristic.

The separately delegated `record.revise` adapter can now publish a historical
source correction and all selected source-copy successors together. It retains
the exact preceding package and form lineage; an old assessment does not bind
the new source digest. The operation and byte-history contract live in
`mechanics/growth-cycle/parts/branch-growth-cycle/README.md`, not in read-only
access or a new source ontology.

### Native material witnesses and scholarly composites

The same adjacent form-set grammar also binds the unchanged
`tos_artifact_source_witness_v1/v2` and
`tos_scholarly_composite_witness_v1` records. Their actual `artifact_id` or
`composite_id` and `record_version` form the subject ref; no `record_id` is
injected into their source payload. Native schema and owner path are checked
before a form command or an assessment-source selection.

For a physical witness, `metadata.preferred-name` copies the complete first
custody inventory number, and `metadata.source-note` copies `path_identity.note`.
The former is an attributed inventory label, not an assessed object title.
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
separate work. A source-copy is mechanically ready, not semantically admitted.
Freeform production retains the existing assessment boundary.

Both source-claim and source-navigation readers carry the same forms and exact
subject payload. The standard local form command creates/revises only the
adjacent set, and binds the consumed native schema digest into its prepared
configuration and receipt. Source-bound assessment configuration can select
the native subject and its forms by their actual IDs and exact file digests;
selection is not a substantive assessment or a permission grant. Native
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
declarations. Their extensible tag grammar matches metadata forms. No field
means no advertised statement, not a generated sentence, title or ID fallback.
These declarations concern this wording, not the cited witness language or an
assessment of translation/originality. Invalid declared tags are refused.

The **entire exact Claim** is mandatory context, including its maker, endpoints,
predicate, layer, epistemic and initial review status, evidence, alternatives,
counterevidence, qualifications and unknown extensions. Thus a ready source-copy
is never standalone: a consumer must retain this context or return a reference
for inspection. The materializer refuses truncation and the existing bounded
delivery can withhold an oversized packet without shortening its assertion.
Templates, freeform paraphrase and derived linguistic-context admission remain
unavailable on this adapter. Rendering does not reassess or grant use of a Claim.

The graph builder validates the declared Claim profile, carries the forms on
the Claim node (not its subject or object), and binds the adjacent file digest.
Python and Worker/D1 choose forms from `source_claim.claim_id/claim_version`
and the source digest. A carrier exposing both metadata and Claim bindings is
ambiguous and refused. This does not alter the stronger source Claim or create
a direct fact edge. Legacy Claim streams still need their own adapter migration.

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
