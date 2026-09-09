# Retained object-Link source-context review

Date: 2026-09-09 UTC. Combined source baseline:
`e770bfcd3653c86be075a6d12c03bba72fcedbfa`, the local integration of native
Link prerequisite `725477fac15437c4686ee083b32674830931464a` after the separately
reviewed identity, Collection, Artifact and historical-schema-input changes.

## Scope and source-visible review

The actual gap was five retained object-Link v1 Claims, not ten Claims. Their
five Link objects already had exact source-navigation bodies. The Claim
catalog retained each Claim, but the bibliographic graph reader explicitly
skipped its source stream; direct navigation retained only a reference and
selected status fields. This change returns the missing context, not a new
source assertion or a migration of those old bytes.

- Source traceability: yes. The explicit `source_object_link_read.py` adapter
  binds the exact retained stream, source schema, line, Claim ID and canonical
  digest. Strict JSON, regular non-symlink paths, bounded reads, duplicates,
  schema mismatch and source drift are checked. Undeclared schema dependencies
  fail locally without network retrieval. The complete raw Claim is
  preserved in both the reified Claim and its existing direct navigation edge.
- Layer separation: yes. Evidence, maker, provenance, unknown qualifier
  members, explicit false/null values, empty reviews, version and supersession
  remain source-declared context. A remote address is not fetched or treated
  as observed content. Availability still grants no rights.
- Native identity and source scope: yes. The legacy five-type domain remains
  Work/Expression/Edition/Collection/Item to Link. The separate v2 six-type
  profile supplies mappings, not v1 write admission. `SourceClaimProfiles`
  continues to reject these v1 Claims. Existing direct edge IDs, endpoints,
  predicates, review states and refs are unchanged; Link source bodies stay
  exact in both carriers, without inferred equivalence or identity rewriting.
- Missingness and authority: yes. No statement, language, Form, assessment,
  review, source revision or historical event is fabricated. Ordinary core
  inspection returns complete source fields and explicit missing Form roles.
  Source-visible assessment, rights, consent, publication and canon retain
  their owners; a serializer, digest or mapped predicate accepts none of them.
- Portable consistency: yes. The existing normalization exposes raw Claim
  fields and exact pointers. Marked carrier body/digest/endpoint disagreement
  is rejected, including disagreement between direct and reified carriers.
  Older unmarked readers keep their absent-context state. These checks do not
  authenticate unsigned exports or replace source validation.
- Authored versus derived: yes. Only the corpus-index transport schema gains
  optional exact legacy context. The old object-Link schema, Claim stream,
  Link records, reviews and histories are unchanged. Generated graph and
  index companions are rebuilt from source; they remain disposable readers.

Golden-kernel transfer, new philosophical branches, lived witness, calibration,
compost, multilingual canon, counterpart alignment and runtime ownership are
not applicable. No stronger sibling authority or new durable decision moved.

## Bounded integration corrections

The complete real core canary exposed a separate native metadata-version
consumer defect: both the exact-version view and its history join assumed
`record_id`. With the integration master's authorization, both now reuse the
existing portable native identity grammar already used by Form selection.
Actual Artifact and Composite bodies retain `artifact_id` and `composite_id`;
no new identity table or source shadow field was introduced. Version, canonical
digest, native-prefix, shadow-ID and exact history-list checks remain strict.

The independent identity and Link branches both used entity registry version
30. The combined source must advance to 31; relation registry 36 was already
unioned in the Link prerequisite. The exact transition against
`b59fe2344413f4193fd48ac826330ed0c19bdb7f` passes after that version correction.
This is registry currentness, not new legacy semantics or permission.

## Verification and remaining owner

The final focused legacy/full-core and existing version/history/native-Form
selection passed 17 tests and 209 subtests (70 unselected). This includes all
five real retained Claims through `ToSAccessCore.knowledge_node` and
`knowledge_relation`, exact Link bodies in both carriers, source preservation,
Form gaps, same-context pointers, v1 profile refusal, portable substitution
negatives, and actual Artifact/Composite version/history mismatch controls.
Script/test topology and mechanic routes passed 28 tests and 762 subtests.
The full source-witness foundation passed. The independent Link prerequisite
integration also passed 30 tests and 129 subtests across native Link, Artifact,
discovery and the bounded reader's initial checks.

The source-witness catalog, rebuilt bibliographic graph and its validator
passed their parity/source-return checks. The corpus index is rebuilt after
the final upstream graph and review inputs; its earlier intermediate parity
failure is not claimed green. Final index and documentation-family checks
belong to the exact committed closeout evidence.

The integration master owns complete source-batch assembly, whole-source M01
coverage, final generated sealing and landing. The full release suite and
runtime/deploy paths were not exercised. The artifact-bundle lane remains
operator-paused. Local tests and manual review are not CI, merge, publication,
source assessment, rights clearance or philosophical acceptance.
