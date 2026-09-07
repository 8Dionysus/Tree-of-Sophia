# Bounded source record revision and exact retained packages

Scope: source/Growth implementation, reviewed by `model:codex` on 2026-09-07.
This records engineering inspection, not historical assessment or permission
to publish. Runtime session: `01a06cc7-0452-77f2-b89a-fb77fb86c3bf`.

## Changed owner surfaces

The source command contract now separately delegates historical-record field
correction. `source_revisions.py` publishes the record, explicit form successors
and retained request/receipt in one directory exchange. Original package bytes
are independently checked in a source-owned blob archive before replacement.
The shared corpus lock now also covers existing form writers. The source path,
ID, old claims, old creation provenance and unknown companion bytes remain
distinct from the new description and its form bindings. Access remains read-only.

## Risk review

- Source traceability / lineage: yes. Exact old source refs and whole-package
  digests resolve to tracked original byte blobs; current scanners do not
  rediscover archived versions as current identities or provenance events.
- Authored/derived and assessment boundaries: yes. The authored patch and
  reason are retained. Serialization, a source-copy form and a green check do
  not become research assessment, upstream model attestation or admission.
- Scope and identity: yes. An independently selected protected configuration
  limits fields, subject and form IDs. Identity, visibility, rights, claims and
  assessment decisions are not mutable through this adapter. Unknown values
  outside the explicit patch and unselected companion bytes are preserved.
- Related changes / recovery: yes within the declared flat package and
  cooperating local writers. Tests exercise actual process exit before and
  after exchange, replay, revocation, competing record/form writers, archive
  corruption and exact prior-version inspection. The old copy is removed from
  invocation-local staging only after independently verifying its archive.
- Snapshot limit: directory namespace exchange does not make an arbitrary
  multi-file reader traversal transactional. Existing source/form snapshot
  checks detect observed source drift; graph/catalog builds remain separate.
- Other checklist subjects (canon, lived witness, translation, counterpart,
  branch formation, calibration): not applicable; none is changed or admitted.

## Verification and limits

Test-first baseline: both initial revision tests failed because the command
configuration/operation did not exist. The implemented focused route passes
11 tests, including the actual existing catalog → graph → form reader. Existing
source-command regression passes 25 tests; the combined 35-test run before the
last graph integration test passed in 25.484 seconds. Topology discovery passes
26 tests in 4.580 seconds. Source-home validation and `git diff --check` pass.
The final full Growth suite passes 101 tests in 8.330 seconds. Those are local
checks over synthetic semantics and real command/filesystem mechanisms, not CI.

No real historical record has yet been revised in this implementation check.
No full-corpus migration, all-profile writer, cross-subject transaction, CI,
merge, public deployment or runtime acceptance is claimed. Full foundation
validation has an already recorded unrelated private-payload boundary; it is
not bypassed here. The active full foundation goal remains open.

Rollback uses a further source-version correction or a derived-reader rollback,
never deletion or relabeling of the committed history. Abrupt-loss staging stays
outside scanners for explicit owner recovery. Package/history byte ceilings are
safety bounds, not host capacity reservations or performance budgets.
