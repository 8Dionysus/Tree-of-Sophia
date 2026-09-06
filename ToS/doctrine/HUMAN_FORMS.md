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
