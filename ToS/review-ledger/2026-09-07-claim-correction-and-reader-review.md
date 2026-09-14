# Declared Claim correction and current reader

Date: 2026-09-07. Parent: `02a597c077c5dcb74d7154dae02764a7cd88fb69`.
Owners: source relations and the existing Growth source-command adapter.

## Contract and manual review

`claim.revise` corrects one delegated Claim's allowed descriptive/evidence
fields and its source-copy forms atomically. The ID, endpoints, predicate,
assertion layer, origin maker/provenance, initial review flag, assessments and
visibility cannot change through this route. Issuer and correction reason are
recorded separately. Unknown qualifier keys survive the shallow field patch;
unselected stream rows and package files remain byte-identical. Original
creation requests, provenance and receipts are historical and remain unchanged.

The existing flat package archive, fsync and Linux directory exchange are
reused. Shared history orders different Claims' corrections and reconstructs
every stream transition from exact prior bytes. Missing/corrupt archives,
unrecorded sibling changes, a truncated initial history and noninitial versions
without history fail closed. Retries return their historical receipt plus fresh
current source/forms. Source/evidence bindings are explicit in request and
receipt as well as the dependency digest. Initial `claims.create` replay now
verifies its original stream through this history, without overwriting it.

Review checklist: source return, authored/derived separation, identity,
lineage, language, uncertainty and stronger-owner boundaries are preserved.
Source-copy forms are not automatic substantive assessment. Admission is
neither carried to changed digests nor granted by this command. Exact-scope
configuration comes from the operator-authorized source workflow, not Claim
prose. Sibling authority, rights and publication policy were not changed.
No new ADR is required: the existing metadata-correction package architecture
is reused for the shared Claim stream, with its bounds documented in the
[operation contract](../../mechanics/growth-cycle/parts/branch-growth-cycle/README.md#correction-of-a-declared-source-claim).

## Real application

The [source review](2026-09-07-jgb-claim-wording-source-review.md) identifies six
generic `thought_expressed_in` statements in `relations/jgb-freedom-research/`.
The separately delegated commands replaced them with distinct, passage-scoped
Russian statements for contradiction, responsibility, rejection, reconstructed
argument/transition and the addressed objection. This is a descriptive
correction of those same six Claims, not six new assertions or endorsements.

All six Claims and their statement forms advanced from version 1 to 2.
The other 22 stream rows, their forms and initial creation companions were
preserved. Each ordinary command was followed by exact replay from a new CLI
process and `inspect-version` byte verification. Initial creation of the whole
28-Claim package still returns its original receipt with `replayed: true`.

The six archives preserve successively 61, 62, 62, 62, 62 and 62 original
filenames (including empty form-writer locks), representing 215,915; 224,936;
233,909; 242,692; 251,527 and 260,352 bytes. Manifest content addressing may
share identical blobs *within* an archive; these are exact source-history
packages, not another current corpus. No abandoned staging was deleted.

The common reader was run from `ToSAccessCore.discover(root)`. It retained all
12 freedom-related subject descriptions and all 28 complete source Claims.
Russian statement selection returns the exact revised wording with the full
qualified Claim as required context and `admission: null`. Concept and
Objection focus still reach their expected conceptions or addressed transition
and thesis. No frontend files, camera, gestures or scene styling changed.

## Verification and measurements

- Claim creation/correction suite: 20 tests passed (100.176 s), plus focused
  missing-history/revocation (5.309 s) and interleaved-history (10.858 s) checks
  after strengthening baseline validation.
- Source commands: 37 tests passed (138.326 s).
- Metadata revisions: 25 tests passed (18.645 s).
- Human forms: 21 tests passed (0.206 s).
- Assessment engine: 49 tests passed (8.569 s).
- Source catalog and bibliographic graph rebuilt; source-foundation,
  graph parity and graph validator passed.
- Corpus index rebuilt and validated; source-home, nested route cards,
  mechanics topology and regenerated documentation currentness passed. The
  initial topology check exposed a missing new-script inventory entry; it was
  added to the existing owner inventory, then the check passed.
- Real correction + preparation + separate-process replay + archived-byte
  inspection: 7.074, 7.108, 7.373, 7.002, 6.969 and 7.277 s in one sequential
  local run. This is not isolated write latency, cold/warm distribution or p95.
- Cold common-graph probe: 21.594 s; subsequent two-hop concept/objection
  focus: 0.292/0.279 s; peak RSS 1,260,484 KiB. Concurrent local validation
  was running; these are integration observations, not an S03 benchmark.

Synthetic tests cover exact interleaved history, unknown fields, unchanged
siblings, domain/schema/evidence closure, stale preparation, revoked replay,
form-writer conflict, competing writers, and abrupt process exit before/after
exchange. They are not historical facts, model competence evidence, a hostile
same-UID sandbox or hardware power-loss testing.

## Limits and next owner

The writer still scans source metadata and fully verifies a bounded history;
it is not indexed or constant-cost. Limits are 64 files, 8 MiB per package,
1 MiB per Claim stream and 128 shared corrections, with earlier form-history
limits possible. Noninitial imports need an explicit history migration.
Legacy Claim formats, identity merge/split, assessment-source integration,
indexed processing and corpus-wide semantic form-quality coverage remain
Growth/source/processing work. Existing decisions and claims are not promoted.

This local integration does not establish calibrated independent assessment,
CI, merge, release, deployment, Cloudflare/D1 or real UI acceptance. The full
Foundation v1 goal remains open. Reader rollback must retain these source
versions and correction history.
