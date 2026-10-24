# Native TS Record codecs and output store

Current continuation: [source-held adapter v1.2.0](integration/SOURCE-HELD-v1.2.0.md),
based on integrated PR274 head6b38212419aaa5b585530ce15731d99498ca5ec7 and the
actual Common65814871262375f5716d68bdda95597019dd5828 document port. Common Library
platform34 is the current dependency; the v18 pins/TS-C4-URI-1 refusal below are
historical v1.1 evidence. The three URI references are corrected in Common658.
The complete full-file registry schema and concrete command adapter are supplied
in TS-owned paths with guarded common integration hunks. They are not yet installed
native call sites, issued instances, a compiled result or completed production use.

## Historical v1.1.0 input contract and evidence

Owned-path/interface ACK: common Library libfile_eaebcf9c8f78819182cf892ae222be98
platform17,3791979B,SHA256
3783ccb0e84dcf72fca2556f9fa909ad6ffd60e44ff869c66a2e5f4daadfc8f7,
OWNERSHIP_ACK_NATIVE_CONTRACT_V1.md (10265B,
a6e12ff98e581315e01e60d3eaaa13aba8a53a234b9dbbde4306204c3b3c8aee).
Common explicitly does not write native_store/** or tests/native_store_native.rs.
Shared schema/registry/exports and native host registration remain common-owned.

Current common C4 dependency is local commit
0e3252add3ef42785c438c6306d63a2fc2ad4a07, tree
07f273d26fbc8be3459cb16e7e92212ca35e14d8, Library platform18,
4702693B/SHA256 a1d46de3eefacda8d7b7257e766ca021e55c2f2f9f7b992e101bf2b3bd571ad5.
Its complete 7706B contract was read and independently reconstructed with SHA256
c681014df490ae04a1ebc9af5a19ae7d2ad20ab84e9302b976942fb7ad71d0c3.
The previous remote981593 source is historical intake, not a claim that C4 was
pushed or passed Rust/native gates. TS does not retry common's denied PR action.

This module implements the domain continuation, not a new common kernel. Native
Records use the maintained record_id/record_version, PublishedStrict and whole
SourceRecordDigestV1. No new Record identity, raw-hash equivalence, source grant,
assessment, universal-world/completeness issuer or native creation receipt is
invented. Original metadata/provenance remain borrowed and source-returnable.

The source branch is based on accepted TS6b8dc051; the new-file delta applies to
integrated main6d2e9d00/treec0fd7c83. Common applies the separate single lib.rs
export after acceptance. Existing TS/K0/PWE files, Cargo, canonical sources,
calendar/Text/PR267/SVB/mirror and documentary matrices/errata are unchanged.

## Implemented maps and exact grammar

1. Native Record binding checks the whole strict source document, actual native
   identity/version and maintained canonical digest. NativeTsStore requires the
   owner-derived selected descriptor and one of the three agreed HTTPS schema
   resources; it independently matches actual schema_version/record_type. The
   real owner executes schema and selected membership/history before returning.
2. Native /body decoding: profile is closed {projection,codomain}, with
   projection DirectDomainValueV1 and one of SpanMeet, AllenRelation,
   OrderSolution, RelationImage, TopologyValue. Result is closed {codomain,value}.
   The Record wrapper is not a domain value and is never validated as one.
3. Typed DirectDomainValueV1 is the identity on the resolved TsOutputValue, with
   actual codomain/shape checks. typed_projection is explicit; check_output also
   uses the independent accepted K0 check_ts_output and fences the entire call.

contracts/body-fragments.json provides exact JSON Schema fragments for common's
three native schemas and the precise payload grammar. Those fragments have no
fictional native $id/ExactRef and are not registered worker resources by their
mere presence here. Common's native schemas select the actual HTTPS resources
and registered kind/schema tokens. Candidate bodies contain no native IDs.

Rational is closed {n,d}, canonical decimal STRINGS. n is in
[-170141183460469231731687303715884105727,170141183460469231731687303715884105727],
d is in [1,170141183460469231731687303715884105727]. No +, leading zeros, -0,
negative/zero denominator, float, rounding or non-reduced representation.
The exact supported Rational profile excludes i128::MIN; overflow is NumericRange.

Span is closed {lower,upper,lower_closed,upper_closed}, lower<=upper; equality
requires both flags true. SpanMeet is null for a computed empty meet, or that
nonempty Span. It never encodes an unknown dating boundary as null/empty.
AllenRelation is exactly a PascalCase name of the existing thirteen labels.
TopologyValue is a JSON boolean. No physical metric/geodetic map is added.

OrderSolution is {kind:"Witness",endpoints:[Rational,...]} or
{kind:"StrictCycle",steps:[CycleStep,...]}. Endpoints are nonempty, even-width
proper (start,end) pairs in original variable order. CycleStep is closed
{from,to,strict,premise}; indices are canonical unsigned decimal strings fitting
the target's existing usize profile. Premise is ProperInterval+interval,
Constraint+constraint, AnchorOrder+left_anchor/right_anchor, or Hypothesis+
hypothesis at the syntax decoder. Complete base-cycle values reject Hypothesis;
diagnostic/hypothetical data stay in their original source rather than being
rewritten into a base certificate. A valid base cycle is nonempty, closed and
has a strict edge. Source constraints/anchors still need independent replay
against the exact resolved input problem by verify_order_witness/cycle.

RelationImage is {kind:"NoAdmissibleModel",cycle:[...]} or
{kind:"Exact",possible:[{relation,endpoints},...]}. Exact has1..13 distinct
labels, equal witness widths and proper endpoint pairs, retaining order and all
witnesses. This codec checks domain data, not whether an observed list is an
exhaustive possible-world carrier. NoCompleteValue/Partial/Undefined/ResourceLimit
are separate existing K0 dispositions/execution states; they are not fabricated
as one of these five complete domain payloads. Unsupported/malformed native
payloads return typed refusal with their original source document preserved.

Definition body follows the actual common981593 schema: closed
{definition_kind:"bounded-ts-rule-v1",specification:{domain,rule,premises}}.
Accepted domain/rule names are
only the existing bounded TS operations listed in the fragments; premises are
nonempty source text retained as text. Text is not executed as a proof or source
admission rule. Definition applicability remains input-specific and partial;
unknown definition/registration/schema/frame is NeedsBinding/Unsupported, not
a universal time axis, arbitrary geometry, authority or criterion.

## Selected source and resource boundary

NativeTsStore implements the existing TsOutputStore concretely. It preloads only
explicit exact requests from NativeTsSourceOwner, verifies whole-record binding,
decodes the native body, retains the native source and resolves exact output/
profile references without latest fallback. Same ID/version with a different
digest is incompatible. An unprepared reference is NeedsBinding, not absence.
Definition/profile/result roles are independently schema-dispatched. Source
revision, input identity, selection, entity-registry AND role-registry refs and schema execution binding
must match the owner-derived data exactly. An explicit request list is a cache,
not a complete source/candidate/historical universe.

NativeTsSourceOwner is a required held-native-owner port, not a supplied positive
mock. Its actual host must use the maintained selected Record/history route,
execute the real schema and required source membership, retain original native
access/currentness/lineage frames, and charge source/schema work to the original
host operation. Missing registered definitions/profiles/host frames refuse.
No production NativeTsSourceOwner implementation is manufactured from booleans.
Those common native registration outputs/frames remain actual gate inputs.

verify_current surrounds every source read/preparation and query. check_output
surrounds the entire independent K0/output/profile check as well. Returned
immutable historical data are not a permission to reuse a stale current query;
the protected caller uses the current-use wrapper and original owner fences.

NativeTsSession has no default or reset. Original work/requested-state/items,
source-files/source-bytes, cancellation and deadline persist through preparation
and all resolutions. The host retains it in a caller-owned RefCell, borrowed by
the store; a failed prepare cannot discard the spent ledger. usage() exposes
monotone counters for the native parent operation's accounting. Per-resolution DomainLimits add a local cap, never reset
original counters. Native owner callbacks receive that same session. Canonical
binding pre-reserves a finite common-codec state allowance and uses the maintained
cooperative state/visit/admission visitor. All decode/encode array growth, string
scans, duplicate-label/pin comparisons and reallocation copy work are charged;
allocation uses try_reserve_exact. Logical requested-state fees exclude allocator
excess/RSS and caller-held source/parser/schema workers. Deadline/cancellation or
resource exhaustion yields ResourceLimit, never an exact prefix result.
check_output reserves the full finite OutputCheckLimits.local in the original
ledger before invoking the unchanged accepted pure checker. The reservation is
not refunded; resolver charges are additional. The pure check is bounded by that
local allowance, with cancellation/deadline fenced before and after the call;
its existing internals are not claimed to be a newly cooperative visitor.

## Verification and exact remaining gate

Two narrow native targets are authored in tests/native_store_native.rs: actual
source-record pin versus raw-file hash, and five payload codecs/projection/
malformed-case/budget boundaries. The native historical-process input is a real
unchanged repo record with its existing creation receipt. Pure codec values are
not claimed to be registered records or nativeauthority. No Rust compile/test/
format execution is claimed until an ordinary approved toolchain runs them;
no installation or archived Rust execution is performed by this owner.

Actual source Record1810B and canonical1633B match its source-create-receipt:
raw3b1e769b8a5f494e586a7f0913717a226034b1a41d43c8316eb727b37c14b4a1,
canonical8b36a058c5d3cd97554cf62e461e0eca31fe3437c5d8ae49365c0eb02eb48f87.
That executed Python reference byte comparison is not a Rust/native test.

Common C4 now supplies the three schema resources and declared native
type/role reader. This codec follows its actual definition_kind/specification
wrapper. Ten whole-Record authoring candidates and an exact TS role-table hunk
are supplied; they are not native creation/admission receipts or fixture authority.

TS-C4-URI-1 is a concrete current common dependency: three role-schema $refs use
tree-of-sophia.local while the maintained entity schema $id uses
treeofsophia.local. Current C4 registry composition is Unresolvable under the
actual-ID-only reference registry. The exact three-line correction hunk is
supplied for the common owner, separately verified as a proposed composition;
shared source is unchanged. Payload schemas and all ten candidate Records pass
their new reference checks. No full native schema PASS is inferred from them.

Remaining concrete inputs: actual source-create/selection/registration of the
definition/profile/result records with actual IDs, versions and canonical
digests; their held selected source/schema/native host frames; then this store's
actual protected owner instantiation and one complete registered-source
operation run. No fabricated pins or source receipts fill those inputs. This
module can be reviewed/compiled through its separate export without inventing
those records, but full production binding/admission remains an explicit gate.
