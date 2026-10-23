# PWE native consumer/projection seams — implementation 0.1.1

Normative input: accepted PWE application 1.2.0 and common CCI-1/K0 fixed
interface. This directory is an isolated implementation candidate. It has no
production registration, command entry, source writer, identity merge, new
authority issuer or completeness issuer. It is not reachable from the crate
until the separately supplied compiler-export.patch is accepted and applied by
the shared-file owner. K0 commit 8f05ccd738d622ca47e9db47dd9844ee93759a5b is an
integration prerequisite; the PWE branch itself starts at 433cb19f4d621dfdf4a98fe2a7ee202e66f8e388.

## Implemented surfaces

`Session::prepare` implements the three closed value grammars, all variants of
assignees and bindings, typed/exact refs, qualification and OriginAddress fields.
It preserves unresolved values. It performs no historical resolution. Language
tags and scripts are checked as complete ASCII strings under the declared
language/subtag and four-letter script grammar; prose length counts Unicode
scalars and uses the schema ECMA whitespace set. A caller must pass the original
source owner's PublishedStrict-parsed values; a serde Value cannot certify raw
duplicate-key handling or recover discarded lexical source bytes.

`qualify` checks whole TypedRef scope/context aliases, resolves required Q1
binding slots and checks the identity focal subject. `check_dispatch` checks
outer account kind, exact signature/predicate/qualification profile and the
ordered full-account argument list. It does not guess a short same-as predicate
from the full qualified identity account. `DispatchRow` is resolved data from
the owner port, not registration or a warrant. The owner must independently
check predicate membership in the exact signature and its argument types.

`exact_basis` is the only nested reference extractor. Each TypedRef contributes
both kind_binding and record; qualification, context/scope, criterion, method,
source locations, namespace, supersedes and every alternative assignee remain
in occurrence order. A BasisSlot keeps role, outer array index and inner index;
within a TypedRef, inner 2*j is kind_binding and 2*j+1 is record. Bare members
are checked against endpoints, never resolved through a latest fallback.
Multiple versions in explicit basis slots survive. Unresolved required slots
return NeedsBinding while the entire borrowed original value remains available.
An unresolved similarity method does not prevent preserve-only preparation.

`registry` checks exact actual registry bytes/version, with no registry writer.
`metadata` delegates supported source kind/profile/home dispatch to the existing
`source_bibliographic::supplied_metadata_descriptor`, then checks exact record,
catalog digest, kind and schema descriptor binding. Its sealed CheckedMetadata
means route/exact-content checked; the existing facade does not execute schemas
or authenticate membership. It cannot become a source or admission receipt.
`resolve_basis` calls the source owner separately for each exact role occurrence
and rechecks canonical record content. The result is bound to the original
account, so another account's resolved basis cannot be substituted.

`check_role_matrix` enforces endpoint/member closure and no ambiguous same-ID
endpoint versions, name-used-by versus signature-in, exact metadata-derived
responsibility subject/bearer roles, and same-level correspondence. A title may
name a Work; an assignee must satisfy the responsibility bearer role even when
it is also the subject. A collective pseudonym needs a collective bearer policy.
Publisher/provision activity, cross-level correspondence and unsupported owner
descriptors remain explicit NeedsBinding. No identity is inferred from a name,
translation, file, material carrier, quotation, composition or similar structure.

`preserved_candidate` independently re-admits the native K0 envelope under a
precharged caller allowance and requires Completed/Produced. It resolves the
actual output/profile references by canonical digest, validates the fixed local
profile-v2 shape/projection and three actual schema records, checks the wrapper,
selects /body without rewriting, checks the PreservedCandidate container, then
checks every individual value. It supports the accepted local schema pins only;
no urn is registered as a production profile. The return retains wrapper,
container and items separately; semantic_resolution_executed remains false.
The producer does not execute identity or attribution consumers.

These Rust Result values are not persisted K0 RunResult envelopes. A protected
host must create its own separate consumer run and exact output/profile records;
NeedsBinding never inherits the producer's Completed/Produced. This module does
not mint run IDs, output refs, schemas or a synthetic completed semantic run.

`identity_view` is the separate selected-account consumer. It binds one exact
criterion, scope, complete context, source revision/input identity, account
modality/layer, and current assessment selector/policy/schema worker. It selects
one explicit joint-carrier/scenario exact reference; the native owner checks
every edge's compatibility with that SAME scenario before dispatch. These are
common exact references under the owner's selected profile, not a new common
World sort. Context equality alone does not establish joint validity. It uses
native AssessmentMechanicsReport rather than a JSON admission assertion; those
reports remain mechanics results, never authorization tokens. Only explicit
positive same-referent adds a same edge; explicit positive distinct adds a
symmetric nontransitive constraint. Negative, unknown/omitted, candidates and
ineligible accounts have distinct observations and never produce complements.
The result keeps its exact ViewBasis, source Claim refs, all exact dependencies
and the native admission observation. Paths keep exact edge Claim origins.
Conflicts return both distinct Claim and same-path basis. A missing path says
only NoEligiblePathInSelectedCut. Native current fences run before and after
construction and each public path/conflict query; any fence failure returns no
query result. This immutable view cannot silently be reused as current after a
different active selection, revocation or correction.

`attribution` checks one named exact joint carrier/context/scope. Each world has
separately warranted explicit positive/negative facts; arrays are not assumed
complete assigned sets. Possible_joint requires both people in one same world.
Positive witnesses and explicit counterexamples can be returned on partial
observations. No observed witness is not false; empty observations do not prove
an empty admissible class. Necessary/excluded remain NeedsCoverage in the absence
of an owner-bound universal argument. There is deliberately no completeness bool
and no conversion from source-claimed-exhaustive/exclusive to a production proof.
No existential union of world-wise identity closures is labelled a PER.

`independent_correction` checks new Claim identity/version, null initial
supersedes and a separate exact predecessor/successor/provenance/selection
lineage payload. `description_revision` requires the same Claim with the next
version and freezes proposition/evidence/binding fields; only explanatory
`/object/limitations`, `/object/account`, `/object/grounds` and
`/object/counterreading` may vary when present in the selected value grammar.
At the top level only `claim_version` may change, to the exact next version;
`object` receives only that explicit nested allowance. All other top-level
values and the complete field set remain unchanged. This includes evidence_refs,
supporting_quotes (anchors/exact wording/posture), counterevidence_refs,
alternative_claim_refs, maker, provenance_event_ref, qualifiers, confidence,
epistemic/review posture, schema, supersedes/assessment refs, visibility,
extensions and any profile-specific/unknown source field. Required shared
source-Claim fields must be present in both inputs; optional missing/null/empty
states remain distinct. Native source schema execution still belongs to its
owner. The more general retained-Claim correction route may revise evidence;
that does not make an evidence change description-only here. These checks write nothing,
deactivate nothing and preserve both old exact records. Existing subject
merge/split proposals and scoped-members-v1 composition remain with their
maintained owners. No author, completeness or rights propagation is added.

## Native owner integration obligations and explicit unsupported routes

1. The current relation registry supports structured-reference-value-v1, but its
   object_reference_set.structure_adapter is only scoped-members-v1 and
   basis_adapter only collection-membership-versions-v1. PWE source Claims and
   pwe-exact-basis-v1 have no accepted actual source registration in this cut.
   Do not invent a registry row, profile ID, native receipt or authority to make
   this compile seam look production-supported. Source navigation must not use
   the generic reader's owner-evidence skip for new PWE values.
2. ConsumerOwner must select the actual registered Claim schema/profile, resolve
   the exact signature/dispatch row, validate argument kind roles and signature
   membership and compatibility with this one exact joint scenario, perform
   source schema execution and selected-cut membership, and
   verify all source/assessment/access/active-lineage fences. Its DispatchRow
   must be derived from that actual source, not copied from request JSON. Until
   that source route exists, return NeedsBinding. No concrete production owner
   implementation is delivered here because those bindings are absent.
3. Resolve every BasisSlot through the existing owner-derived source input path
   in source_navigation_claim_input; pin selection, registry/schema/locator and
   currentness in that same fence. Do not rebuild SVB occurrence/open_rows or
   transport namespaces here. SuppliedExact is input transport, not evidence.
4. Historical view requires an actual interpretation contract and W port, which
   is not registered here: the current Historical request returns NeedsBinding.
   Admission-with-limits likewise requires its actual limits interpreter; it is
   not promoted to an unrestricted identity edge. Cross-scope/context/criterion
   transport needs a separately bound common partial-map consumer.
5. JointCarrierOwner must establish that worlds belong to this one actual
   carrier and that their positive/negative atoms have appropriate warrants.
   Independent coverage/universal proof and a nonempty admissible world class
   are still required for necessary/excluded truth. K0 supplies no public
   ManifestCoverageEvidence constructor and this module adds none.

These are actual unimplemented production owner bindings, not proof that
mathematical Q1--Q4 laws are missing. The code delivers the pure consumers and
the mandatory owner seams; it does not claim a command or historical corpus
operation is already available. The accepted v1.2.0 normative definitions and
D1--D4 are unchanged.

## Resource and verification boundary

One Session has monotone finite byte/node/work/ref/item/state charges, an
original deadline and cancellation flag. There is no public budget reset or
default. New vectors reserve before growth; indexed traversal is bounded and
quadratic work remains charged. K0 allowance is prepaid because its checker
does not report usage. The existing canonical codec and metadata facade are
reused; their pure calls use a conservatively prepaid logical helper envelope.
Caller parsing/source/schema worker allocations and immutable retained inputs
remain caller-owned and must share the host's admitted operation budget. Logical
state/work fees are not an allocator/RSS theorem. Native allocator sizing,
ordinary-toolchain compilation, formatting, performance and deadline behavior
have not been established by this source-only delivery. No physical resource
acceptance or complete protected-call integration is asserted.

One addressed unit source target, tests::pwe_native_contract, covers B1
wrapper/container versus independently valid items, wrong output projection,
and Q3 joint/marginal/coverage distinctions. Fixtures are byte-preserved local
v1.2.0 candidate records. The test contains no AssessmentMechanicsReport factory.
It is AUTHORED, NOT EXECUTED. Ordinary cargo/rustc/rustfmt are absent on this host;
none was downloaded or run from an archive.

One additional narrow regression source target,
tests::description_revision_preserves_source_evidence, addresses R HOLD
PWE-D2-EVIDENCE. Every counterexample has the next Claim version and a newly
computed correct exact digest, so changed evidence cannot fail merely through
a stale reference. A permitted object explanation edit is the positive control;
adjacent source/evidence fields and presence changes share the same bounded
table. This target is also AUTHORED, NOT EXECUTED.

After source acceptance, accepted K0 and the shared export hunk, an approved
ordinary pinned toolchain can run the narrow target from rust/:

```sh
cargo +1.98.1 test --locked -p tos-compiler --features source-tools --lib pwe_native::tests::pwe_native_contract -- --exact
cargo +1.98.1 test --locked -p tos-compiler --features source-tools --lib pwe_native::tests::description_revision_preserves_source_evidence -- --exact
```

Use the already approved external CARGO_TARGET_DIR and required source/CI gates.
No gate is green merely because this command is written here. Shared lib.rs,
Cargo/toolchain, TypeScript, PR267/calendar/Text, SVB, mirror and canonical data
are outside this patch. Merge requires source acceptance and applicable green
gates; this source-only candidate has not been pushed, merged or deployed.
