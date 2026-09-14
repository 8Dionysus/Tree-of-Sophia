# Historical Claim assessment consumer review — 2026-09-10

## Source-owner scope

This separately authorized companion starts from
`0ffc438b1de3219098b7a3c9a7f5b36ff41fc3dc`. It closes the source-bound
assessment v2 consumer gap for finite public historical Claim streams and
their adjacent HumanForms. The [Growth assessment route](../../mechanics/growth-cycle/parts/branch-growth-cycle/README.md#source-bound-configuration-v2)
owns this behavior; no writer, profile schema, real source assertion, authority,
competence, assessment or generated projection is changed.

## Reviewed boundaries

- Yes: only finite public `history/**/historical-claims.jsonl` paths receive
  the historical dependency adapter. The original historical Claim schema,
  predicate domain/range, evidence grammar and visibility remain authoritative.
  No native schema coercion or inferred endpoint is introduced.
- Yes: the exact selected Claim and its source-selected historical subject,
  typed object or relative-date anchor form the dependency closure. Existing
  endpoint schema/ID/basename checks apply. The unchanged orphan-form and
  canonical current-subject guards propagate that closure to forms; inline
  shadows and unsupported families cannot supply it.
- Yes: uncaptured legacy sources remain readable. This consumer neither
  selects nor verifies a creation receipt, and does not require a correction
  grant. Reading and substantive assessment retain separate authority.
- Yes: existing historical schema/display helpers accept an optional trusted
  JSON reader. The assessment reader protects local paths, bounds and charges
  unique source/contract inputs before decoding, and retains raw digests.
  Final rehash detects drift without double-charging those inputs; other
  profile dependencies and existing native identity reads remain counted.
- Yes: unknown display extensions remain opaque. Malformed known display v1
  fails. Evidence pointers are preserved and schema-checked, not fetched or
  converted into claims of independent observation.
- Yes: the actual v2 describe/materialize/journal consumer is exercised with
  separately configured synthetic authority and competence. Source-copy
  wording is unavailable before qualifying review and retains the complete
  Claim afterward. This demonstrates mechanics, not real calibration or
  historical acceptance.
- Not applicable: source acquisition, provenance migration, new write grants,
  rights/publication, canon, deployment or runtime acceptance.

## Focused evidence

Each XML receipt records worktree, HEAD, tracked patch digest, exact untracked
test digest and selected cases. The final new test digest is
`5a2c4cf1791138154338ef4395cefc13d66aaa0086e7ff08a22834fe1acc1559`.

| Run | Result | Resource observation | XML SHA-256 |
| --- | --- | --- | --- |
| Consumer plus native/legacy compatibility and topology | 14 passed, one inventory wording failure; 15 tests, 303 subtests; 55.447 s | 106.1 MiB peak, zero swap | `c7a3eae8b017eb185adb72420d84261a1704f59834e0fc13c64a7048fd73c369` |
| Final complete consumer and repaired inventory target | 5 tests, 140 subtests passed; 28.769 s | 98.8 MiB peak, zero swap | `cb9c790b9357404e5991d96ccdcbbf48cf90905b6acc472580c0a9a74d842018` |

The compatibility run binds tracked patch
`ad8ad2916d0889fcb2a45226662c3bb934c744f3b2845b2be57a0cab26280907`.
The final run binds
`1827be81d9c2654c9c1cd5097cecabd016c18b01a4d08410d2b6af0d73a71d92`;
the only intervening change replaces the inventory failure-route prefix
`Repair` with its required `Fix`. The other compatibility tests were not
rerun for that wording-only correction. This review note was added afterward.

The four durable new cases cover all four G5 source-copy roles through actual
v2 commands; missing/inline parent and endpoint refusal; relative-date anchor
closure; stale and boolean Form subject versions; unsupported source paths
and schema families; original schema, evidence and registry domain checks;
known versus unknown display versions; in-read and command-snapshot schema
drift; exactly 8 MiB of unique selected source/contract bytes succeeding and
one additional byte failing. Discovery is blocked by test guards. Selected
source/form bytes remain unchanged during assessment commands.

An earlier fixture run failed because its synthetic admission cited only
context evidence and no supporting origin. The fixture now explicitly cites
its selected Claim as support and endpoints as context. The engine's evidence
and authority checks were not weakened.

## Next owner and limits

The parent owns independent diff review, integration and any real assessment
preparation. No actual Claim/Form assessment, grant issuance, complete lane,
corpus rebuild, CI, merge or runtime health is claimed here. No independent
memo, proof or progression candidate is promoted from these synthetic results.
