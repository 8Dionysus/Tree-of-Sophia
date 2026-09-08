# Native description correction and registry change review

Date: 2026-09-08. Base: `29c26280992f1397dd3a6ff12cd42f384727a5bc`.
Scope: F01 native Corpus description revision and F08 explicit registry-change
validation. These are separate mechanical boundaries, not Foundation acceptance.
The primary implementation agent reviewed the source contracts and both diffs;
bounded helpers implemented the transition gate and independently reviewed the
native correction boundary.

## Source and authority review

- **Yes — separate identity and description.** The separately selected
  `tos_local_corpus_revision_owner_v1` applies the existing metadata transaction
  to Agent, Place, Organization and Work. The exact typed ID, source basename
  and Corpus schema are checked. A correction advances the record and source-copy
  forms, while the complete predecessor package remains addressable through
  retained history. External IDs, identity/equivalence status, alternate-name
  judgments and Claim/link fields are outside this descriptive permission.
- **Yes — no inherited grant.** Existing creation, historical/profile and
  form-only configurations are not widened. Native correction has its own
  exact configuration and field scope. Corpus has no visibility field; only
  this explicitly public-metadata adapter and exact schema permit that shape.
  A foreign schema or supplied visibility does not become a permissive fallback.
- **Yes — source preservation.** Unknown allowed language qualifications,
  previous forms and unselected companion bytes remain in source/history.
  The unchanged archive, writer lock and atomic directory exchange preserve
  retry, conflict and interruption behavior. Research assessment, canon, rights
  and publication are neither run nor granted by description correction.
- **Yes — explicit transition baseline.** The owner validation lane requires a
  full nonzero locally available commit OID, supplied by the caller or exact
  PR-base/push-before event. It reads baseline JSON/contracts, not old Python,
  and does not fetch or select a convenient newer ref. Current runtime readers
  remain Git-independent. Changed registries/profiles must advance their
  versions; historical identity, reader and schema routes remain protected by
  the existing comparison function.
- **Yes — first introduction is not evolution.** Both registry and both schema
  objects must be absent in the exact baseline, along with any earlier
  registered/declared-reader history, before the separate explicit introduction
  permission is usable. Its report says no previous registry was compared.
  Partial snapshots and deletion of earlier routes cannot masquerade as an
  initial model. A local Git graft that hid earlier registry history was
  reproduced during review; initial-introduction validation now refuses grafts.
- **Not applicable.** No source assertion, actual author description, private
  payload, competence grant, model invocation, philosophical Sign or historical
  review was created or revised by these tests. Practice/counterpart/compost,
  canon mirrors and public admission did not change.

## Verification and evidence limits

The native tests use synthetic records and temporary copies. The complete
source-revision module passed 40 tests in 89.341 s, 76.7 MiB peak, no swap.
The old form/create controls passed 21 tests in 21.296 s, 52 MiB, no swap.
Native cases cover all four created families through catalog → graph → access,
unchanged creation receipts/retry, exact prior bytes/forms, current permissions,
bad schemas, stale contracts, competing writers and real process interruption.

The connected-Agent case keeps existing synthetic Claim endpoints and complete
Claim bodies after the copied description changes. Peer review correctly noted
that the fixture rebuild serializes its Claim stream again. An added check now
compares original Claim bytes immediately after correction, **before** rebuilding;
that strengthened case passed separately in 1.853 s, 47.8 MiB, no swap. It is
not a historical correction to the real source record.

The initial native run failed on an invalid synthetic language subtag, corrected
to the schema-valid `x-test`. An early connected-case assertion looked for a
metadata `source_record` instead of the existing Claim `source_claim` slot;
the assertion was corrected, then the full 40-test module passed. Neither
fixture correction relaxed production validation.

The primary reviewer independently ran the final 13 transition tests, 10
validation-lane tests and 16 script/test topology tests: 39 passed in 16.257 s;
the complete launch took 18.160 s, 79.1 MiB peak, no swap. The lane suite's
deliberate failing-check fixture prints an error; the suite itself passed.
The gate then passed on both exact local baselines:

- `29c26280992f1397dd3a6ff12cd42f384727a5bc`: registry evolution, previous
  registry comparison performed.
- `a5a9eefccac80d36f8574d8f3f1676340e0e562a`: explicitly permitted initial
  introduction, no previous registry comparison. This is local `main`, not
  a claim about current remote `main`.

An independent helper also ran those 39 tests (10.249 s, 74.5 MiB, no swap)
and both real-baseline checks. Generated companions and final source/home
currentness are checked after this review text is finalized; their actual
terminal result belongs to the commit-bound checkpoint, not an advance claim
in this note.

## Remaining boundaries

The new native adapter does not correct every native family or perform general
multi-object transactions. Public exact-version delivery remains Claim-only;
the metadata command's `inspect-version` result must not be copied wholesale
into a historical consumer because it also contains current command context.

The registry gate enforces declared mechanical evolution, not philosophical
equivalence of arbitrary changed definitions, domain/range or properties. A
green transition does not accept a reference migration or content. Source
review retains those judgments. The full Foundation goal still needs actual
qualified assessment, complete profile/consumer coverage, scaling and CI.

`Repo Validation` retains its name and aggregation, but its Release Audit checks
now require the explicit registry baseline (and permission for a genuine first
introduction). This source change is not a successful remote CI run. The
existing ignored private-file source-foundation blocker was not read, moved or
deleted. No main merge, publication, deployment or runtime-health claim is made.
