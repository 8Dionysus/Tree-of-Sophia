# Native Artifact creation boundary review

Date: 2026-09-09 UTC. Implementation baseline:
`d41df78c9b6aa2c41820fa8d777ca4c8594b3aac`.

This review covers the separately delegated native Artifact creation adapter,
its registration in the existing source command, the Artifact-only origin
branch of source-foundation validation, owner documentation and synthetic
tests. The source command's shared atomic-creation seam is the prerequisite
`adf74142354810762f57d7b07ce2a8f8d090912c`; it is not a second publication engine.

## Reviewed boundaries

- Source traceability: yes. The actual `artifact_id`, native v2 schema,
  record version, source-copy forms and exact rights/discovery/research byte
  bindings survive the retained request, receipt and serialization event.
  The evidence reader does not consult a mutable write grant.
- Layer separation: yes. The new event serializes Artifact metadata and forms.
  It does not claim to have generated the independent rights, discovery or
  research records. Research byte integrity is not research assessment.
- Authored versus derived: yes. Catalog and exact typed metadata readers
  retain the native record and `artifact_id`, without a synthetic `record_id`
  inside the authored record. Human-readable forms are source-copy renderings,
  not accepted semantic statements.
- Authority and ownership: yes. A separate exact grant is required; the old
  Corpus grant cannot create Artifacts. Creation starts unreviewed with no
  performed human review, planting refs or text/semantic/graph/canon/publication
  admission. Existing rights must already cover this metadata-only identity;
  this command cannot decide rights or create them.
- Identity and context: yes. The canonical tradition/site/physical-identity
  path is provider-independent. No physical/textual equivalence, containment,
  possession, identity merge or philosophical planting is inferred.
- Selected-history boundary: yes. Creation replay after a permitted native
  descriptive correction verifies the retained original and committed selected
  publication. Neither the creation retry nor origin verifier enumerates or
  reads Artifact descendants. Unrelated occupied target data is preserved.
- Honest failure: yes. Stale or substituted owner inputs, partial capture,
  forged output roles and interrupted publication fail closed or recover by
  exact retry. Missing exact earlier auxiliary bytes do not silently rebind
  an old event to current rights or research; their owner must restore
  availability. The evidence is unsigned mechanical evidence, not proof of
  authenticated execution or source assessment.
- Counterparts, calibration, multilingual canon, compost, lived witness and
  real philosophical branch acceptance: not applicable. No real Artifact,
  source text, private payload, rights record, discovery record or research
  packet was created or changed by this implementation.

## Validation and residual integration work

The Artifact, native descriptive correction, exact metadata reader and command
discovery checks passed together: 40 tests and 144 subtests. Three existing
shared-creation canaries also passed (37 unselected), covering exact v2
provenance/replay, catalog/graph/restart and stale-contract/recovery/revocation.
The fixtures are synthetic metadata exercises, not historical Artifact review.
Script/test topology and mechanic route checks passed: 28 tests and 752
subtests. Documentation-family parity, source-home validation and whitespace
checks also passed. The initial inventory entry's required failure-route
prefix was corrected before the successful topology rerun.

The complete source-witness foundation was executed. After adding the exact
old Claim schema archive, five retained provision/publication input bindings
still fail because their consumer branches directly hash the current Claim
schema. The failures are not Artifact origin failures. The integration owner
has the exact paths and owns the inputs-only historical-resolver correction;
this note does not claim a green combined foundation before that correction
and a fresh full run. No earlier provenance or output digest was restamped.

The remaining owner is the integration master for prerequisite assembly,
global generated companions, full combined validation and landing. Local
tests and this source-visible review are not CI, merge, publication, rights
clearance, real source assessment or canon acceptance.
