# Letter 705: exact Claim revisions and authored display fields

Date: 2026-09-10 UTC. Reviewer/executor: `model:codex`, under the operator's
Foundation v1 implementation mandate. Command-code baseline:
`400b945c6e42fbc5f7e8f3d4e92c1afa704ba6fa` (G5 source candidate
`af45df629d06466d73dcea68494c7889a6bb1324`). This review records source authoring
and command integrity, not independent model consensus, substantive admission,
historical truth, canon, rights clearance or publication.

## Owner change

Eight existing public-metadata Claims in these relation packages advanced from
version 1 to version 2 through the separately delegated `claim.revise` command:

- `nietzsche-letter-705`: sender, commissioning-episode link and book attribution;
- `nietzsche-letter-705-addressee`: addressee attribution;
- `nietzsche-letter-705-carrier`: documentary-to-material-carrier attribution;
- `nietzsche-letter-705-catalogue-date`: catalogue-assigned date;
- `nietzsche-letter-705-catalogue-direction`: catalogue origin and destination.

Each revision adds source-owned Russian `name`, `caption` and `hover` fields.
Existing complete statements and every original qualifier are preserved. A
missing statement script declaration becomes null, not an inferred script;
new Russian display wordings explicitly declare `Cyrl`. The eight Claims keep
their IDs, predicates, endpoints/values, assertion layers, original makers and
provenance, evidence, visibility and initial `unreviewed` status. The separate
catalogue-date grant allowlists the unchanged exact value but does not allow
the `object` field to change. No grant permits new evidence or admission here.

The five existing statement-form IDs are retained and advance to version 2.
The three catalogue Claims receive their initial statement forms. Every Claim
also receives three separately identified compact forms. All four roles bind
the complete exact version-2 Claim as mandatory context. Thirty-two current
forms do not mean thirty-two independently assessed assertions.

## Source-visible review

The source review checklist was applied to identity, layering, context,
language, uncertainty and authority boundaries: **yes** for preservation.
Canon, lived-witness, counterpart and golden-entry transitions are
**not applicable**.

- Editorial sender/addressee attribution is not handwriting authentication,
  delivery, receipt or reading. The person is not equated with the firm.
- The book link and characterization of commissioning remain attributed to
  Sommer's commentary. The letter does not name the book. The letter and the
  commentary's citation are not independent witnesses to the same proposition.
- GSA's carrier attribution does not equate a document with a physical thing,
  authenticate handwriting or establish image rights.
- Catalogue date, origin and destination remain attributed to `GSA, Eintrag`.
  They do not establish actual writing, dispatch, delivery or receipt. The
  date retains unknown calendar/numbering; geographic identifications remain
  provisional. The new catalogue date does not replace the separate older
  commissioning-date Claim or its distinct adapter-normalization posture.
- A pre-execution identity audit found that a planned carrier-Claim name ID
  was already used by the Artifact's name. Before any affected write, the three
  new carrier-Claim compact IDs were changed to
  `tos.form.nietzsche-letter-705.carrier-claim.{name,caption,hover}-ru`.
  Existing Artifact and Claim statement forms were not renamed.

## Executed integrity and bounded delivery

For all eight Claims, native prepare/revise/replay/inspect-version completed
with exit 0. Per-Claim service times ranged from 9.851 to 18.994 seconds;
observed memory peaks ranged from 76.7 to 95.7 MiB, with zero swap. This measures
the scoped source workflow, not graph rebuild or user-query latency. The first
sender runner stopped before preparation because its own comparison omitted
the `sha256:` prefix; the unchanged source was checked before correcting that
scratch-only assertion and rerunning.

After application, a separate Node/standard-library audit checked all eight
exact successor bodies against their archived predecessors and reviewed
qualifier additions; all original statements and nonchanged fields matched.
It verified 56 retained archive-file bindings, package-revision digests,
request digests, form references and source references. Original creation
request, receipt, environment and provenance bytes match both the archives and
the Git baseline. The native runner separately verified unchanged sibling
rows, byte-invariant replay and exact predecessor inspection. A cross-subject
current/prior-form audit found no collision among the 32 selected current IDs.

The current source profile validator and materializer accept all eight Claims
and all 32 full forms. Selection was tested using actual Claim bindings, source
digests, form-file refs and a valid synthetic content-revision fence; this is a
reader-unit probe, not a current published graph or HTTP/UI test.

| Claim suffix | Compact conservative JSON-byte ceiling | Ready compact roles |
| --- | ---: | --- |
| sender | 13103 | name, caption, hover |
| commissioning letter link | 14416 | name, caption, hover |
| concerns-jenseits | 14258 | name, caption, hover |
| addressee | 13901 | name, caption, hover |
| carrier | 16093 | name, caption, hover |
| catalogue-date | 12217 | name, caption |
| catalogue-origin | 16253 | name, caption, hover |
| catalogue-destination | 16035 | name, caption, hover |

The unchanged selection budget is 16384. All complete statements, and the
catalogue-date hover, remain exact-reference-only in this multi-role compact
packet; their full materializations are intact and ready for explicit
inspection. This limit is not hidden by clipping source qualifications or
increasing the budget. Real compact/inspection consumption remains to verify.
`validate_tos_source_home.py` and `git diff --check` passed.

## Next owner and limits

The exact source/form versions in the adjacent histories are the inputs to
substantive assessment. Assessment, scoped use, final-union catalog/graph
generation, Worker parity and real UI consumption remain separate work.
The three older `historical-claims.jsonl` commissioning Claims are not covered
by these eight revisions; their schema-preserving adapter is a separate
implementation. No full-corpus validator, final CI, merge, runtime replacement,
deployment or Foundation v1 completion is claimed by this note.
