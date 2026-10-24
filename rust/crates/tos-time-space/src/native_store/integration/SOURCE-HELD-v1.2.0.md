# Native TS source-held continuation v1.2.0

This delta starts at actual PR274 head6b38212419aaa5b585530ce15731d99498ca5ec7,
tree16383d9c7c2e98c465914ee2f6ce86d4b9583b9b. No old d4 package is reapplied.
The current common seam is65814871262375f5716d68bdda95597019dd5828,
source_native_roles.rs blob8a72a428155a105589cbf002a9396b2f7af657f5,
raw SHA256 9da148154b76a6e6d20d660a20dd370780e7f8011a89b3558d3bcafb759df532.
Common Library libfile_eaebcf9c8f78819182cf892ae222be98 platform34,27018816B,
expected SHA256 39d6ddd825a78a26add0e15c9e174ab5d52ddd481e7e381b0b091680d9748392
is the parent-supplied complete dependency pin. The relevant complete source,
document-port intake and current ownership contracts were actually read; this
does not claim an independent download/hash of all27MB historical content.

## Concrete implementation

source_ts_native_owner.rs is an actual command-crate adapter in a TS-owned path.
The separate integration hunk includes it as a crate-private module; there is
no new public wire constructor. NativeTsHeldOwner owns real NativeRoleDocument
holders and retains the actual selected cut, CommandContext, OwnerTextContext,
PrivateAssessmentSources and original OperationBudget in a caller-owned RefCell.
The temporary NativeRoleResolver borrows the actual worker/parent for resolution,
then releases those mutable borrows. The actual prepared execution binding is
retained; later assessment can borrow the SAME parent/worker rather than minting
another ledger. Move an existing OperationBudget into the RefCell; do not call
OperationBudget::new to fill this parameter. Original strict documents come exclusively from
Common658 resolve_document. No JSON serialization of a derived value supplies
the original document, and no positive fake source owner is used.

Constructor inputs are those actual protected frames, the existing selection
Record's exact reference and selected source path, actual entity/role registry
selection, exact requested Records and the SAME caller-owned TS ledger. The
constructor performs the full selected TS registry schema check in the actual
worker, verifies all three domain resources' source raw digests against the
prepared worker, and takes resource URI from the actual selected schema $id.
Whole schema execution identity, source revision, input identity, native role,
entity registry, role registry and exact Record identity/version/content remain
bound. For this immutable native route, input identity is the actual
RecordRead<CommandContext>::Identity SourceRevision.0; PG/candidate opaque
identities are not cast into that type.

The selection reference is not derived from a revision or invented for the
cache. Its actual current Record goes through the maintained exact registered
Record reader, strict source content/pin and selected-cut custody checks; that
reader's evidence is retained. Source-owned selection meaning/requested use must
already be chosen by the protected caller. No historical criterion is supplied
by this adapter. A historical SELECTION Record currently refuses explicitly;
formal definition/profile/result Records still support the maintained current
AND historical archive routes through Common658. Supporting historical selection
documents requires Common's generic exact original-document port; a current
fallback is not an implementation of that missing operation.

Cache queries recheck actual current fences, match the original ledger identity
and full selection, then return the original native document and source-derived
descriptor. A cache is only the explicit requested Records, never a universe.
Missing entries return NeedsBinding, incompatible role/pins return Incompatible.
The actual native source error remains accessible without cloning in
last_source_refusal; an after-Err fence cannot overwrite the first source error.
Immutable history observations grant no current use.

NativeTsStore::check_output now additionally compares K0 selection_ref to the
actual held selection by ID/version/digest. It attempts the actual owner fence
after success AND failure, preserves the first failure, and never returns a
successful value across a failed fence. Body decoding, typed projection,
independent K0 checking, exact-input witness replay and admission remain distinct.

## Accounting and remaining shared obligations

NativeTsSession::remaining_allowance exposes only the unspent portion of that
same ledger, without refund/reset, and refuses nested reservations. The host
reserves its full remaining finite work/state suballowance prospectively from
the original native parent before TS execution. Original cancellation identity
must match; native deadlines are lowered to the original TS deadline. Native
source/worker/document work is additionally charged through that same Common
parent. No new OperationBudget, external dependency or larger cap is created.
Actual schema-map search uses contract_digest_with_admission, paying prefixes
before comparison. Local TS retained-input charges are not misrepresented as
proof that an earlier foreign native read was prospectively paid.

Common v34's already identified C3-W debt over the maintained foreign whole
reader/worker/history call is INHERITED, not closed by this TS adapter. The
selection exact read uses that maintained C4 bounded reader and preserves this
same limitation. A future common live-payer interface must replace that seam,
not reinterpret post-return accounting as a whole-call proof. Original spent
parent and TS prefixes survive a later refusal; no partial cache is returned.

## Single-owner integration

common-c4-owner-registration.patch is guarded against the actual Common658
files. Only the common owner applies it. It adds:

- the complete ToS/contracts/ts-native-bindings.schema.json resource;
- crate-private visibility for existing admitted_document/check_pin helpers,
  retaining their implementation byte bodies and private document constructors;
- command module inclusion of the TS-owned adapter;
- the existing workspace tos-time-space dependency edge and matching lock entry.

The full TS registry source is checked under its own selected-file schema, in
addition to the shared row/profile checks. Required selected resources are the
new full registry schema, shared native-role/actual entity schema, and all three
TS native schemas plus their existing source-metadata/semantic-description/
corpus dependencies. The actual worker must contain their exact current bytes.
Missing resource registration/selection refuses; a Python schema check does not
replace native worker execution. Common source/generation/resource registration
and protected caller dispatch remain single-owner work outside TS paths.

Next actual inputs: registered native selection/definition/profile Records and
exact current/history references, protected caller instantiation and a real
source-bound operation-result/K0 run. The ten previously authored Records remain
source-create inputs only. No record, grant, authority, Completed result or
completion bool is fabricated. Native instance creation/admission is not implied
by the control role table or schema registration.

## Verification boundary

One new numerical ledger regression is authored; it constructs no native source
owner/profile fixture. The ordinary cargo invocation failed before execution
with exit127, cargo: command not found. Rust compile/format, new test execution,
physical current/archive source reads and protected production call sites remain
unverified. No compiler/dependency installation or archived Rust execution occurs.
Existing PR274/Common658 green CI is historical evidence for their old blobs,
not a PASS for this delta. New source review and required native gates precede
merge. No separate Astra session, deployment, credentials/security changes,
canonical data edit or new bulk planting is performed.
