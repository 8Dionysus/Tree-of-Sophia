# TS implementation contract v1.0.1 — 2026-10-23

## Owner seam and provenance

The owned edit set is precisely this new crate and its own native test target.
The source base is `433cb19f4d621dfdf4a98fe2a7ee202e66f8e388`. Common identity,
record/version/digest and operation envelope definitions remain in
`tos-foundation`; K0 commit `8f05ccd738d622ca47e9db47dd9844ee93759a5b` is a required
composition prerequisite, not a local stub. Its fixed interface is Library
`libfile_eaebcf9c8f78819182cf892ae222be98` v4, 568638B, SHA256
`9efa367a208978af4b52b110d5d6215bba4ea10cc2e4c7fb44b1cd6063061186`.
The K0 verdict is limited SOURCE/TEST-SOURCE ACCEPT, not a runtime claim.

The normative domain basis remains TS v1.1 SPEC.md/CCI-BINDING.md/reference.py/
PATCH-PROPOSAL.md in preserved ZIP Library
`libfile_951e755f7b9c8191ba7c6868186e2129`, 4608003B, SHA256
`97b03a31031ea2b14b9b6a2cad41730d74804df457bc85a5fe854539fb31ecb6`.
Its complete readable index is `libfile_f8d1f9ce46748191b27641e3fe93812a`;
JSON coverage index `libfile_8aff51799b588191a864323d024f669c` enumerates 51 files.
This port does not rewrite the archived accepted conditional mathematics.
The original seven oracles are Page
`page_8969e9febc1c8191b44194c6222f4d4f`, seq192, blocks
`841477c3-86e1-4726-8dd6-b966ec46a7d1` through
`0a334de6-0530-44dd-9c52-1a6a99d896fb`. Time Page seq10 and Space Page seq4
retain the source-envelope versus Allen distinction. Existing query runtime
`temporal_comparison` and `knowledge_temporal` remain unchanged.

Every ExactRecordRef is id + positive source record version + owner digest.
Library platform version, Page sequence and raw file SHA are evidence locators,
not substitute native record versions or owner canonical profiles. Supplying
structurally valid pins does not prove actual body binding, capability, source
truth, authority or completeness. The owning resolver must establish those
before exposing sourced results. Missing actual profiles remain NeedsBinding.

## Operation contracts

The public Rust signatures are normative for this implementation's value types.
All allocating domain operations have DomainLimits; pure fixed-size arithmetic,
interval primitives and constructors have constant-size inputs. Result<T> is
ordinary Rust Result<T,DomainError>, not a second common result product.

| Operation | Exact input → output | Preserved law and composition | Unknown/incompatible behaviour |
| --- | --- | --- | --- |
| Rational::new/checked_add/sub/mul/div/Ord | Signed i128 numerator/denominator → normalized exact Rational; MIN excluded | Equal rationals normalize identically; order uses continued fractions without cross-product overflow; values feed all temporal/spatial primitives | Zero denominator/divisor Invalid; checked arithmetic overflow or excluded MIN NumericRange; never float/round/saturate |
| Span::new/contains/intersection | Rational lower/upper plus explicit inclusivity → nonempty span, membership bool, Option<Span> meet | Meet commutative, associative, idempotent; None means proved empty membership set; singleton requires both ends closed | Reversed/empty constructor Invalid; no unknown or infinity silently admitted |
| resolve_boundaries | Two tagged Boundary values → Resolved(Span) only when both numeric bounds and inclusivity are explicit; otherwise Unresolved borrowing both original boundaries | Preserves all original tags; raw expression is not overwritten; an unresolved boundary result is type-distinct from None in an empty intersection | Unknown, Unspecified, NoBoundDeclared, Unbounded, NotApplicable and unknown inclusion remain unresolved, never infinite or empty |
| ProperInterval/ActualExtent/DatingPossibility | Span with lower<upper → proper interval; actual Instant or Interval is distinct from source possibility | No coercion from an ignorance bucket to duration/actual extent; instant not in Allen domain | A singleton may be actual instant or possibility region, but cannot construct proper interval |
| relate_intervals/allen | Two proper intervals + equal exact AxisBindings → one of 13 Allen endpoint labels | converse symmetry; exhaustive/disjoint proper-interval labels; compose via endpoint constraints and same axis | Different TRS/spec/instance/rules → Incompatible; no implicit calendar mapping; raw allen is fixed mathematical primitive with caller binding obligation |
| solve_order | OrderProblem exact pin, axis, explicit dense-Q domain, proper intervals, finite < / <= / = constraints and numeric anchors with source refs → Witness(all endpoints) or StrictCycle(actual premise edges) | Satisfying witness substitutes every constraint/proper pair/anchor; unsatisfiable graph cycle contains a strict edge and returns premises; compose constraints by conjunction on one problem | Unknown domain NeedsBinding; discrete Unsupported; bad indices Invalid; limits ResourceLimit; representability NumericRange is no logical contradiction |
| verify_order_witness/cycle | Full actual OrderProblem + candidate values/cycle → bool or typed error | Witness replays every anchor; cycle replays premise indices/direction/numeric anchor ordering and strictness; no one-line certificate label trusted | False/Invalid for bad proof; resource limits stay limits; no trusted proof-kernel or source-digest verification claimed |
| possible_allen | One OrderProblem and two interval indices → NoAdmissibleModel(cycle) or exact label image with full joint witness per label | Each label solves the SAME joint conjunction plus endpoint hypothesis; no Cartesian product of interval marginals; all 13 considered | Shared budget for base + all hypotheses + witness/cycle construction + all anchors. Any limit/range error aborts complete result; no partial set returned as exact |
| project_joint_truth | Frozen rows carrying common context/selection/scope/carrier, distinct scenario refs, booleans (P,Q), explicit upstream coverage assertion and fixed projection P/Q/AND → witnessed boolean image | Uses each row's joint pair; P and Q individually possible need not make P∧Q possible. Singleton false is value {false}, not Entailed(P∧Q) | Complete empty carrier → NoAdmissibleModel; observed empty → ObservedNoWitness; observed nonempty never claimed exhaustive; duplicate scenario Invalid; mismatched universe Incompatible |
| require_transport | Optional exact map/rule, TypedMap source/target kind+domain, input pin and resolved input applicability decision → admitted map borrow | Gate only. No calendar arithmetic or coordinate transformer duplicated from PR267; compose only after each map's actual domain and preservation warrant | Missing/unknown map applicability NeedsBinding; wrong endpoints or outside domain Incompatible. No synthetic map/authority or conversion value |
| TemporalPath::from_edge/compose_paths | Source-pinned typed endpoints, exact relation profile, joint witness, axis, actual/observed/unresolved validity and provenance → literal ordered edge path with separate necessary actual constraint, positive support and unresolved flag | Exact intermediate record AND kind; same joint context/cut/scope/carrier/scenario and time axis; intersect actual constraints separately from positive support; concatenate every original edge/provenance in order; path grouping associative where universe gates agree | Different scenarios excluded, no Cartesianization. Different matching-endpoint universe/axis Incompatible. Only empty actual-constraint meet rejects a path; empty observed-support meet establishes no absence. Unknown/censored paths remain unresolved beyond positively supported windows. No transitive source/admin/presence assertion inferred |
| select_states | Already frozen upstream HistoricalState list + world coordinate, axis and one joint witness → all included and all unresolved state borrows | Actual extent or observed positive window membership permits inclusion; only non-membership of an exhaustive Actual extent permits exclusion; preserve overlapping revisions and every exact state | Outside ObservedPrefix, at its excluded endpoint, or for wholly unknown validity, retain state unresolved. No eternal truth/absence from a cutoff. Wrong universe/axis Incompatible. No reconstruction of system/source/knowledge snapshot and no last-write-wins |
| check_transition | Explicit event, occurrence claim/span, before/after exact states and rule → interface consistency success | Same exact subject/kind, axis and joint witness; event and state assertions remain distinct | Does not prove transition occurrence, cause, exclusive states, boundary instant or exhaustiveness; mismatched subject/universe/axis Incompatible |
| compatible_frames | Optional exact frame pairs including spec/instance/CRS/rules/axes-units/profile/domain/capability → compatibility or error | Frame equality is a gate only; no number/label-based geographic alignment | Missing NeedsBinding; different domains/pins Incompatible without explicit transport |
| topology | Two GeometryAssertions (actual extent role), matching frame with explicit physical CartesianRational2d capability and Rectangle→Point/Rectangle predicate → exact bool | Closed rectangle Covers/Contains/Intersects/Touches; rect/rect Overlaps/SharedBoundarySegment/AdjacentAlongSegment; exact coordinate order, no subtraction overflow. Compose only with the selected historical state and actual source precision | Unknown capability NeedsBinding; geodetic, semantic/narrative physical topology, proxy bbox/centroid/representative/possible location, unsupported orientation/predicate Unsupported; no arbitrary polygon/metric/CRS engine |
| physical_distance_from_topology | Geometry assertions → Unsupported | Explicit graph/topology alone supplies no physical metric/unit; no fake Euclidean result | Always Unsupported by this restricted implementation; a separately bound metric backend is required |
| check_ts_output | Actual K0 CheckedOperationBinding, optional real TS record/profile store, expected TS codomain and independent local/value/profile budgets → produced actual value or preserved NoCompleteValue execution/value pair | Resolves both exact records, independently checks returned id/version/digest against K0 refs; applies named DirectDomainValueV1 projection to actual typed TS body; checks profile and actual value codomain. K0 structure does not replace this | Missing resolver NeedsBinding, wrong pins/profile/body kind Incompatible. Partial/Undefined/NoCompleteValue never relabelled Produced; resolver errors propagate. Source truth/applicability/assessment remain separate |

GeometryAssertion.subject denotes the extent's source subject; Rectangle itself
is not a PlaceRef. Frame and precision profiles stay attached to the assertion.
Historical boundary change is multiple HistoricalStates with geometry payloads,
explicit validity and event/transition assertions, not destructive polygon update.
Nesting/adjacency results apply to these geometries in this world-time state;
they do not infer an administrative or referent relation. Physical/local,
topological and semantic/narrative domains remain distinct.

TemporalContextTimes carries world axis, source time, knowledge cut, system cut
and query context separately. Its opaque refs are supplied by the existing
selector. World interval membership cannot establish publication or ingestion;
oracle3 exercises only the frozen-list consumer, not a history constructor.
The calendar conversion path is require_transport plus the actual PR267 owner
operator once bound; unsupported historical calendars are not proleptic defaults.

## Addressed correctness details

N6: the interpretation map is H→W, where W is the domain observable space.
Rational endpoint witnesses and bool/topology/path values are W-values, not
common evidential E-statuses. In particular a complete boolean image {false}
cannot establish Entailed(phi). Semantic assessment needs its own assessment
record and context. This crate defines no competing Entailed/Evidence kernel.
K0 value/execution/applicability/assessment are separate; check_ts_output only
checks produced value/profile consistency, not source admission or assessment.
Its codomain admission also rejects empty/duplicate/width-inconsistent exact
relation images, nonproper endpoint witnesses and malformed/nonstrict base-cycle
data. These are shape checks; actual constraints/anchors must still be replayed
with verify_order_witness/cycle on the exact resolved input problem. Q1 extraction
and input-to-question alignment cannot be inferred from an output type tag.

N8: m anchors are actual input items, not bounded by endpoint count n. Admit
scans every anchor/ref; graph generation retains every anchor and insertion
sort comparisons cost O(m²), with actual adjacent-anchor premise indices.
An anchor-derived proof step is checked against both numeric anchors, its
orientation and equality/strictness. Witness substitution checks every anchor,
including repeated variables and equal numeric values. Work depends on anchors,
constraints, generated arcs, reconstruction and verification, not merely n³.

The closure invariant is reach=0 (no path), 1 (weak path), 2 (path containing a
strict edge). Strict diagonal implies inconsistency. With no strict cycle,
mutual reachability forms equality classes; the quotient is acyclic. A linear
extension preserves all weak/strict constraints. Adjacent sorted anchor arcs
preserve numeric order and force equal-valued anchors into one class. Rational
interpolation between fixed pins and unit spacing outside construct a model
over abstract unbounded dense Q. The bounded i128 port reports NumericRange
when its arithmetic cannot represent that construction, never empty model.
The solver verifies the final substitution before returning it. For complete
Allen image, repeat this conjunction with each of 13 disjoint endpoint cases.

Port delta P1: unresolved path segments must retain the meet of all known
supports. A simple unknown-or-span result loses this information and can break
associativity: unknown A, B valid [0,1), C valid [2,3). The implementation keeps
constraint separately from unresolved and rejects both groupings as empty.
Original source snapshots/reference.py remain historical; this contract is the
precise current implementation clarification, not a claim that they contained it.

### TS-CENSOR-1 correction (v1.0.1)

Review pin supplied by root: verdict
`e542b3ca05f3ebd274e61cf9af48300e25d3a807`, intake SHA256
`58c19ab6702552f2f336d6fa7084af2e7a58e9c799acb2ba6fe3f325d45d5673`.
The verdict commit object is not in this checkout; the parent supplied the exact
HOLD and counterexample, and the affected source was read in full here. No
claim to have read the unavailable complete verdict is made.

The former WorldValidity::known conflated two directions of information:

* Actual declares the exhaustive actual extent. It supplies both positive support
  and a necessary actual-extent constraint. Outside it is excluded relative to
  that supplied claim.
* ObservedPrefix supplies positive support on its stated window only. It supplies
  no necessary upper constraint on actual extent. Outside the window, including
  beyond its open cutoff, is unresolved. No unknown end or onset is invented.
* Unresolved supplies neither a positive window nor an actual-extent constraint.

WorldValidity now has separate internal actual_constraint/positive_support
functions. TemporalPath exposes necessary_actual_constraint (renaming the
ambiguous known_support_constraint), positive_support and known_validity. The
last remains conservative: it returns a complete extent only for an all-Actual
path. None in necessary_actual_constraint means no supplied upper constraint;
None in positive_support means no established joint positive window. Neither
means an empty actual extent or a declared infinite one. Original edge borrows
retain censor_basis, validity claims and all provenance without rewriting them.

Composition maintains C = meet of supplied Actual extents only, P = meet of
positive windows when every edge has one, and unresolved = OR of non-Actual
flags. No actual constraint is a neutral constraint marker, not an eternal-valid
datum. A proved-empty C rejects the path. Empty/unknown P does not reject it.
Thus censored [0,1) followed by censored [2,3) remains an unresolved path; its
joint positive support is unestablished. Censored [0,1) followed by Actual [2,3)
keeps necessary constraint [2,3), without claiming positive support there.

For a fixed compatible universe, intersection of actual constraints is
associative with empty constraint absorbing; positive intersection is associative
with unestablished support absorbing; unresolved OR and edge concatenation are
associative. These independent accumulators preserve grouping through unknown
and censored segments. Actual disjoint extents still reject both groupings even
when another segment is censored. No rejection uses the observation cutoff.

The one added native regression is
`ts_censor_1_positive_support_is_not_actual_extent`: inside/cutoff/outside state
selection; disjoint/overlapping censored support; censored plus actual with
unknown in both groupings; preserved edge provenance; and rejection from actual
disjoint bounds only. It is authored, not executed: ordinary Rust tools are
still absent. Seven unchanged oracle checks are not rerun. Source acceptance
remains with the same R; this correction does not declare the HOLD lifted.

Port delta P2: reference.py's name `shared-boundary-segment` was implemented as
touching with positive overlap along one coordinate, which is an adjacency
predicate. Its exact behaviour is named AdjacentAlongSegment here. The separate
SharedBoundarySegment returns true for any coincident rectangle boundary segment,
including equal rectangles and partly coincident edges of overlapping extents.
This resolves a naming ambiguity without conflating touches, overlaps or place
adjacency. Both predicates exclude a corner-only intersection. Tests name both.

## Costs and limits

DomainLimits has positive max_work, max_requested_bytes, max_items. All counts
use checked usize arithmetic; allocation uses try_reserve_exact and returns
ResourceLimit on failure. No recursively unbounded object traversal or solver
stack is used. max_requested_bytes is cumulative requested logical payload,
not actual Vec excess capacity, retained-storage accounting, allocator overhead,
RSS, preexisting caller input or external resolver I/O. Each pushed element is
charged even when spare capacity exists, each possible reallocation charges
copy work; arrays charge requested slots. This deliberately separate metric
does not impersonate the common owner's retained-state accounting.

Work units charge primitive fixed-width comparisons and iterations, source-ID
bytes, all admitted constraints/anchors, graph creation, closure O(n³), anchor
sort O(m²), quotient linear extension O(n³), interpolation O(n²), BFS and witness
substitution. No implication that O(n³) alone is sufficient. Rational primitive
comparison uses a conservative constant bound of 256 continued-fraction loop
iterations (at most 1024 division/remainder operations) per comparison for the
explicit i128 profile. Arithmetic gcd loops have the same fixed-width bound.
possible_allen shares one Budget for admission/base/13 hypotheses/results; limits
abort the whole complete image. Output profile resolution has explicitly separate
local, value_resolver, profile_resolver budgets; resolver implementations must
honour their own limits, and their I/O is not concealed as local work.

## Current historical correction and remaining bindings

Current documentary selection: full matrix v0.3.3, Library
`libfile_ca5cae5e2a6081918cc873ecce1dbd04`, version0, 1837085B, SHA256
`f2bfab950875eb602a577319c88121e851e0ee8f215e0073385c695bbdca23d3`, together
with TS B2 erratum v1.0.1, Library `libfile_8b5ecd7c7ab481918f57ada843dc0cc5`,
version1, 83932B, SHA256
`26d614416b3826475cb1cdcec083fde255d64994fb055482a73e00ca5e4f300b`.
Its `current_implementation_dependency_override` selects these dependencies for
exact accepted TS package version1 (344893B, SHA256
`f7d552ab16cd831162f9ff67a54da7ea10065c45d138f86f36bb68ae0ba630a9`).
The package and its embedded original files retain their raw integrity hashes;
this integration note does not pretend those historical files contained the update.
Root reports Astra R5 full documentary ACCEPT, B5/N13 closed: review Library
`libfile_11cdd7a5619081919324f71c5791390a`, version4, 215531B, SHA256
`66427ecf9ecf292acfee28788d9deeae1610425fe75a8c662e946cdcd7ce4663`.
This closes the earlier documentary HOLD, not the separate native/runtime gate.

Historical source-delivery TS B2 erratum v1.0.0: Library `libfile_8b5ecd7c7ab481918f57ada843dc0cc5`, version0,
74461B, SHA256 `5dd0bf0f2c46cb42c4472d45f5505f7256c7f0faa397e3737b021de007daa662`.
It pins R3 review v2 SHA256
`28481b35dceb195f201284d81a4fdc45161108ad7aa7b4b47ef9bb49af7291df`, §10.7.3,
and corrected matrix candidate `libfile_9917fcc59b3081918dd828ab43550070` version0,
document v0.3.2, 1833445B, SHA256
`eb2a7e611f6b0ed2d74617d8964f993434380645a9ee891bd6c25a33908ef1f6`.
The exact three snapshot block dependencies and their historical raw hashes are
in that historical erratum. The current selection above also withdraws E009=EHESS and the false S051
raw-key contradiction. Actual v19 E009=Collège de France, E042=EPHE VI/EHESS;
R282 year/status unknown persists. Original MD/JSON/payload/ZIP bytes unchanged.
The original source delivery recorded full documentary acceptance as pending;
that status is superseded by the current R5 documentary ACCEPT above.

Addressed requirements to root/common owner:

1. Compose this new crate with actual K0 commit and apply supplied registration
   hunk. No additional common exports required beyond the existing K0 exports.
2. Resolve actual native TS definitions/profile records, versioned TRS/CRS/rules,
   dense-Q applicability and geometry precision/role from source-owned bindings.
   Fixture IDs are synthetic and never production substitutes. Existing coarse
   query labels cannot be silently replaced with Allen labels.
3. Supply actual TsOutputStore and exact profile bodies with established record/
   body digest binding. DirectDomainValueV1 is a named typed projection codepath,
   not a fabricated native profile registration. Other wrappers need their
   explicit reviewed projection; PWE /body is not implicitly a TS profile.
4. Upstream joint-carrier coverage/extraction, selector/cut and manifest evidence
   remain owner obligations. JointCoverage::Complete is a supplied assertion,
   not a ManifestCoverageEvidence issuer. No completeness/admission certification.
5. Use the common operation envelope to wrap local output/errors. Domain Invalid
   is execution Error; ResourceLimit is execution ResourceLimit; missing binding
   remains blocked/unresolved applicability as appropriate; known incompatible
   inputs have Undefined with the operation's domain reason, never false. A
   sound witness/cycle/exact image is a complete mathematical value, not a
   source semantic acceptance. Assessment/certificate pins need their own owner.
6. Run the authored one native target plus normal required compile/format/owner
   route gates on the approved ordinary toolchain and storage lane. No tests
   were executed here; no prior unchanged accepted checks repeated. No merge
   before source acceptance and all required green gates.
