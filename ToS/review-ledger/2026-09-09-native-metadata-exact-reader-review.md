# Native metadata exact reader and descriptive correction — 2026-09-09

## Reviewed scope

Reviewed against base `856f0669d6bb8ef21cad207774e512d3efdb5872` in the
`codex/tos-native-corpus-completion-20260909` local source branch. This is
an implementation and boundary review, not a judgment on any real witness.

The exact public metadata reader now recognizes native Artifact, retained
scholarly Composite and Link. `resolve_typed` adds a validated owner descriptor
to its ordinary envelope. Persistent subject, exact descriptive record,
schema and Claim identity remain distinct. The actual native `artifact_id`
and `composite_id` survive; neither format becomes a Corpus shadow record.
Unavailable evidence carries no descriptor and never falls back to latest.

The separate native descriptive grant reuses the existing selected three-file
publication, archive, continuous revision, replay and explicit recovery
mechanics. Older grants keep their previous families and fields. Source-copy
forms retain complete context and prior versions; readiness is not assessment.
Artifact/Composite descendants, representations and payloads remain uninspected
and unchanged. Link description cannot alter its URI, observation, association
Claims, provenance or original authority posture.

Source traceability, exact lineage, source/derived separation, bounded grant
scope and no semantic/rights/canon inference were inspected and pass the
applicable source review checklist. No source record or generated catalog was
changed in this implementation slice. Artifact creation, Collection attachment
and compound Link creation are separate remaining work, not implied complete.

## Validation

The real native correction/recovery tests plus existing metadata reader and
selected revision suites passed **37 tests and 63 subtests** in 34.12 seconds.
Synthetic fixture records exercise exact current/historical resolution,
raw-byte return, both native identity fields, original form retention, replay,
interrupted recovery/rollback and refusal of structural or old-grant widening.
This is local mechanical evidence, not CI, merge, deployment or real source
assessment. Broader completion review remains with the integration owner.

Compatibility coverage then passed 99 tests and 1,171 subtests with two test
failures: the explicit discovery inventory lacked the new grant, and an older
guard-removal test selected an obsolete binding name rather than its actual
`/identity_status` pointer. The inventory was updated and the test now removes
that exact semantic guard without depending on a generated binding key. The
targeted rerun, native/metadata readers, discovery and script/test topology
passed **48 tests and 858 subtests** in 36.45 seconds. The old generic form grant
was explicitly checked to remain closed for Link; its forms are available
through the separately delegated native record route. No failed check was
silently waived or interpreted as source acceptance.
