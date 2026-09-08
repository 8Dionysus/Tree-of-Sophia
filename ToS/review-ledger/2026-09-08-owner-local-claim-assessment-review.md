# Private Claim grounding in the shared assessment journal

Date: 2026-09-08. Reviewer: source-owner agent under the continuing Foundation
v1 operator mandate. This reviews a confidential source/assessment interface,
not a historical or linguistic claim, actual competence or publication rights.

## Source owners and inspected boundary

`scripts/source_owner_claim_profiles.py` composes the existing protected source
context, common `SourceClaimProfiles` shape validator, actual public/private
record profiles and native TextUnit resolver. It selects one explicit
`local_only` Claim in a private stream, its independently selected typed
endpoints and evidence, and optional current adjacent forms. No inline body
becomes an endpoint and no alternate predicate registry was introduced.
The mechanic's v4 configuration and operation contract remains in
`mechanics/growth-cycle/parts/branch-growth-cycle/README.md`.

The reader preserves Claim wording, qualifiers, unknown extensions and native
packet content; it does not infer meaning or follow an arbitrary citation,
source-looking extension, provenance event, alternative or supersession link.
Evidence must use an explicitly selected source-record or native adapter.
Additional packet/unit/layer/anchor evidence uses the existing native unit view
and layer, not fabricated packet records or copied independent sources.
The same native origin remains the same origin.

Metadata-only and exact-content modes are separate frozen selections. Exact
reading requires an explicit grant for the Claim and every native input;
existing recorded local-research rights are checked before representation
bytes. The rights predicate was extracted unchanged from the native writer;
the writer retains its wrapper and current behavior. This code interprets
recorded posture, not current law or fulfillment of conditional legal terms.

The optional v4 `owner_local_source_claims` arm does not change existing v4
configuration requirements. A private Claim and its forms use the shared
engine and journal. Source-required grounding is distinct from incidental
records loaded into the engine: every required exact record must be cited in
the submitted evidence. Corrected endpoints, native evidence or changed read
scope can invalidate use even when the Claim/form is unchanged. Old events
and commit-time receipts remain inspectable. Freeform materialization carries
the same closure rather than bypassing it through a display-only evaluation.

The Claim's languages and exact-read eligibility follow only its own grounding
closure. An unrelated metadata-only native selection does not contaminate its
scope. Claim-introduced native records remain supporting evidence; inventing
a subject entry does not authorize assessment of that native unit. Public and
private selections may share a supporting record only with the same body,
version and origin. Direct private targets cannot shadow public selections.

Public source Claim reads now refuse the reserved owner-local namespace before
file IO. The catalog refuses even a broken reserved-directory alias before
globbing. Shared shape validation does not weaken the public visibility gate.
Every v4 response remains `local_only`, with `publication_authorized: false`.
Descriptions expose exact references, not private paths, wording or native
short-span fixity. Explicit freeform and assessment prose can still contain
private material and are not public-safe projections.

## Verification and review observations

All added tests use temporary synthetic sources, rights, grants, competence
and assessments. No retained real private store was read or changed by these
tests. A bounded helper implemented the source reader and reviewed the root
consumer separately; root inspected the complete reader, helper tests, common
validator/rights diffs, adapter, journal and materializer seams.

The first source reader battery plus existing private profile/native readers
passed 69 tests (24 new reader tests, 18 private record tests and 27 native
binding tests) in the independent run; root reproduced all 69 in 83.303 s.
Root's first five Claim integration
tests passed together with the 64 common engine tests in 51.407 s. Subsequent
focused checks found and corrected language-tag case sensitivity in the new
consumer: required tags are compared case-insensitively without rewriting
source tags. A test initially patched nonexistent `_lock`; it now exercises
the actual `_locked` publication boundary and passes (1 test, 4.502 s).

Independent review also reproduced a package preflight gap: a later Claim
with an exact-read request but metadata-only grant was refused only after the
first permitted Claim's private source had been read. The outer selection
preflight now checks the Claim grant, bound source grants and additional native
grants against the requested mode, and rejects exact mode for non-native
metadata profiles, before any private source IO. Root's four-case regression
failed before that correction and passed after it; the independent original
probe then observed zero private-source and representation reads before refusal.

The expanded integration battery covers required evidence omission, unchanged
Claim with corrected endpoint, retained positive historical receipt versus
current refusal, metadata-only no-text access, Claim freeform dependency and
display invalidation, language scope, supporting-only targets, same-origin
public endpoint sharing, describe/append and lock-time drift, and unrelated
native scope isolation. Final root integration: 11 passed in 95.100 s.
Other final regressions: existing v4 21/74.542 s; native assessment 18/23.775 s;
common assessment 64/12.126 s; human forms 22/0.199 s; native writer
25/25.858 s; public Claim writer 26/232.217 s; bibliographic graph 81/245.199 s;
script/test topology 16/2.616 s. The source-home, corpus, documentation and
route-currentness companions follow their final regeneration/check route
after this review; no intermediate partial run is treated as a final gate.

```bash
python -m unittest tests.test_source_owner_claim_profiles tests.test_source_owner_record_profiles tests.test_native_text_binding -v
python -m unittest discover -s mechanics/growth-cycle/tests -p test_owner_local_claim_assessment.py -v
python -m unittest discover -s mechanics/growth-cycle/tests -p test_owner_local_assessment.py -v
python -m unittest discover -s mechanics/growth-cycle/tests -p test_native_text_assessment.py -v
python -m unittest discover -s mechanics/growth-cycle/tests -p test_knowledge_assessment.py -v
python -m unittest discover -s mechanics/growth-cycle/tests -p test_human_forms.py -v
python -m unittest discover -s mechanics/growth-cycle/tests -p test_source_text_unit_commands.py -v
python -m unittest discover -s mechanics/growth-cycle/tests -p test_source_claim_commands.py -v
python -m unittest tests.test_source_witness_bibliographic_graph tests.test_script_topology tests.test_test_topology -v
```

## Checklist, limits and next owner

Source traceability, authored/derived separation, native identity, retained
uncertainty/history, delegated access, language competence scope and current
evidence dependencies: yes for the inspected boundary. No assessment creates
its own authority, original source truth, rights, consent, canon or publication
permission. Historical reviewers are not relabeled. No per-record human gate
was added. Public gold/lived witness changes and AoA runtime/proof/memory
authority are not applicable to this slice.

Private Claim selection is bounded to understood semantic and identity
relation profiles. It does not yet implement arbitrary evidence adapters,
private Claim creation/revision, global private-store identity discovery,
content-quality evaluation, public export or UI delivery. Protected exact
snapshots do not isolate hostile same-account editors or create a distributed
transaction. The actual issuer still owns source stability, current grants,
execution provenance and verified competence.

The next source-owner step is normal private Claim growth on the existing real
native/Occurrence slice and source-visible assessment under actual authority
and competence. This note does not close L02 or the whole Foundation goal and
does not claim CI, merge, deployment or published-runtime health.
