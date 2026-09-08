# Delegated creation of declared source metadata

Date: 2026-09-07. Parent source baseline: `c31e657eb5ed5c2e998866fbb9a2253311812100`.
Scope: existing source-owner command and its metadata-profile contract.

## Boundary reviewed

`source.create` now reuses the historical writer's atomic directory protocol,
exact source-copy form materializer, current delegation, dependency check and
serialization capture. Its independently selected
`tos_local_profile_create_owner_v1` configuration names one concrete registered
profile, subject ID, source path and allowed form IDs. A profile alone grants
no writer; the old form configuration cannot execute source creation.

The source schema version is explicit, not selected by nearest-version or
latest-version guessing. Common metadata and profile-specific schema rules
both apply. Initial identity is provisional with no equivalence claim. The
request cannot change the source path, principal, grant or profile, and cannot
include historical claims. Source fields never choose code or execute it.
Native physical-artifact records retain their separate reader; they are not
coerced into this metadata shape.

Publication is six existing-format files: native source, adjacent forms,
request, environment, serialization provenance and receipt. No empty
historical claim file is manufactured. Caller research and model reasoning
are outside the serializer's captured execution; unsigned buffer digests
remain weaker than independent fixity or execution attestation. Source
creation, substantive assessment, admission, rights and publication remain
different actions. No actual historical record or prior receipt was changed.

## Executed checks

- The first test was red at configuration parsing: `source.create` had no
  delegated owner contract. After implementation, it passes atomic interruption,
  six-file publication, exact payload/forms in the ordinary graph reader,
  a new CLI process's exact replay, rejected claims and current revocation.
- A second test adds a synthetic kind and schema solely as fixture data.
  Creation, graph inspection and subsequent ordinary form revision all work
  without a Python branch for that kind. It preserves unknown nested values,
  source ID and exact source payload. The fixture is not a historical letter.
- Changing that schema after preparation conflicts even when the profile
  has no previous instances. Unknown schema version, wrong kind, nonpublic
  visibility, pre-verified identity and noninitial version are rejected.
- 39 source-command/revision tests passed in 18.746 seconds before the local
  helper naming cleanup; the final rerun is recorded below.
- The existing declared-profile reader test passed in 1.387 seconds.
- Final post-refactor run: all 39 source-command/revision tests passed in
  19.475 seconds. Source-home validation also passed. The additional refusal
  cases select a non-profile type, a foreign identity prefix and a catalog
  destination; all are configuration errors before source publication.
- Those added delegation negatives passed in the focused new-profile test
  (2.299 seconds). The corpus index was regenerated and its validator passed;
  only source-document inventory changed, not authored corpus records or
  the source claim graph.

The new test initially used an unqualified exception name; correcting its
test import/reference was a harness repair, not a production contract change.
Durable negative tests concern replay, permissions, exact identity/version,
dependency freshness and atomic publication, not the incidental exception
wording or output key order. Timings are local observations, not a performance
budget or scaling proof.

## Owner review and limits

Source return, lossless source fields, language/identity separation, explicit
delegation and authored/derived distinction: yes. No semantic acceptance,
agent competence, human signature, rights clearance or canon promotion is
asserted. The existing TOS-D-0044 rationale is sufficient; no new ADR merely
narrates this common writer extension.

2026-09-08 integration note: the preceding historical TOS-D-0044 reference
names the registry decision at
`d187c3c8ff7a7cb261d0be017c5dcb735643c8aa:docs/decisions/TOS-D-0044-stable-semantic-interchange-registry.md`,
now [TOS-D-0059](../../docs/decisions/TOS-D-0059-stable-semantic-interchange-registry.md)
after the explicit pre-landing ID-collision correction. It does not name the
independently landed KAG-unfreeze decision.

The command still scans metadata for global identity and dependency closure.
It does not establish indexed incremental growth, general record revision,
claim growth, identity merge/split or all nine subject profiles. Historical
creation/revision keep their own ABI. UI, D1, CI, merge, deployment and whole
foundation acceptance are not proved by these tests.

Rollback may remove this command capability without deleting source records,
their forms or retained history. Already created profile metadata remains
readable with the declared-profile reader from the parent baseline. An older
reader that lacks a profile must fail visibly rather than strip its fields.
