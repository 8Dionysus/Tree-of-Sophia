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

The existing bibliographic graph builder carries the materializations and
adjacent source return in identity properties, with the set's input digest.
This route uses already public bibliographic metadata, not payload text or
private source layers. It has no growth-command or publication authority.
Each set is bounded at 2 MiB input, 32 current forms and 256 KiB output.
The initial Jenseits set contains original-name, Russian-name and source-note
copies, not a new translation, historical assessment or complete Forms profile.
Its original-name ID is not language-context evidence. The current metadata
adapter does not yet supply this context; adding it requires source-owned
metadata and successor form bindings, not a reader heuristic.
