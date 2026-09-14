# Source-owner creation of declared Claim batches

Date: 2026-09-07. Parent: `d2d929c7cc8080612eecb88f8232f2677df26752`.
Scope: Growth command adapter and existing source Claim profile contracts.

The same source-command entrypoint now dispatches a separately protected
`tos_local_claim_create_owner_v1` configuration to `claims.create`. Its exact
allowlists constrain IDs, subjects, objects, predicates, evidence and maker.
No metadata/form permission implies Claim permission. Schemas and concrete
inherited endpoint types come from the existing declared profiles, not another
registry or a per-predicate Python branch. Source prose cannot select code,
configuration or admission policy.

One new flat package contains the full Claim stream, canonical request,
environment, serialization provenance and receipt. No existing identity,
metadata, historical Claim or assessment record was changed in this checkpoint.
All endpoints must already exist. Selected raw/canonical source digests,
schema and record versions (null when the native shape lacks one), and exact
evidence locations/digests are explicitly bound in preparation, request and
receipt. The broader source/profile/schema/implementation dependency hash is
also checked before and after staging. Ordinary uncooperative same-account
editors still must remain quiescent; these rereads are not a global snapshot.

Shared source locking, protected paths, fsync, no-replace directory publication
and pre-commit serialization capture are reused. The event names the Claim
procedure and both implementation modules. It remains unsigned buffer capture,
not independent execution attestation or source reading. It invokes no model,
performs no substantive assessment and grants no admission or rights.

## Executed checks

- Test-first creation failed on the previously unsupported owner configuration;
  after implementation, the initial end-to-end test passed in 1.468 seconds.
- A later negative exposed acceptance of a corrupted receipt's admission flag.
  Replay now verifies the exact receipt/request/source-snapshot relationship,
  no-admission flag, Claim refs, complete file closure and stored byte digests.
  No forged admission is returned. A separate test first failed because
  preparation exposed no selected input bindings; these are now explicit.
- Eight focused tests then passed in 21.619 seconds, covering scope/schema,
  evidence/identity, source/schema drift, simultaneous writers, occupied and
  symlink paths, current revocation, staged drift, process loss before commit,
  lost response after commit, unchanged orphan staging, CLI replay and corruption.
- The added ninth test registers a synthetic predicate solely in data and
  writes one atomic two-subject batch. A bad second member publishes nothing;
  the complete valid batch passes source/catalog/graph/access in both directions.
  Unknown instruction-like prose stays inert source data. This is not a real
  historical claim. The focused test passed in 1.575 seconds.
- All 49 source-command/revision tests passed in 30.517 seconds. After the
  bounded import cleanup, all 114 Growth tests passed in 31.983 seconds.
  All 26 topology tests passed in 1.352 seconds; diff whitespace check passed.

The first test file briefly imported a TestCase class directly, so unittest
discovered its twelve existing tests too. Switching to a module reference
removed that duplicate discovery; the targeted initial red check was rerun
alone before implementation. No production change was made to fix the harness.
Timings are local checks, not performance budgets or scaling results.

## Review disposition and limits

Source traceability, identity/Claim distinction, delegated scope, no executable
source interpretation, unknown-field preservation, atomic publication and
retention of committed history: yes. The exact evidence allowlist must be
authorized by its source owner; it does not clear rights or permit private
content publication. Existing source packages and historical provenance keep
their original bytes and weaker evidential posture.

Initial creation is not Claim correction, retirement, assessment/admission,
cross-object identity creation, incremental indexing, UI integration or the
whole Growth profile. A later correction route must preserve initial packages
and extend replay/history deliberately. Rollback must not delete newly written
source or silently strip unknown profiles. Remaining source work is actual
Letter associations, person/carrier identity, common assessment integration
and the full nine-profile foundation. CI, merge, deployment, D1 and UI
acceptance were not executed or claimed here.
