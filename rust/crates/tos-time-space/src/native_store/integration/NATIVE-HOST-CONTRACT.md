# Exact common native host seam

Common C4 local commit0e3252add3ef42785c438c6306d63a2fc2ad4a07, Library18
(4702693B, SHA256 a1d46de3eefacda8d7b7257e766ca021e55c2f2f9f7b992e101bf2b3bd571ad5)
now supplies
source_native_roles::NativeRoleResolver/NativeRoleRecord and the three real
HTTPS TS Record schema resources. TS does not edit those common-owned files.
The current Definition wrapper is {definition_kind,specification}; this codec
selects definition_kind bounded-ts-rule-v1 and the closed specification
{domain,rule,premises}. It is aligned with that actual source, not an assumed
earlier body shape.

Common applies module-export.patch, ci-real-native-input.patch and (after source
review) ts-role-bindings.patch. The role table is source-controlled input,
not an authority. Its exact registry_id/version/canonical digest must be bound
from the real selected source; no candidate JSON hash is relabelled as admission.
contracts/role-registry-fragment.json supplies this domain registry's complete
closed file contract, composed with C4's actual shared row schema. The protected
host must validate the actual selected registry under that full-file contract
as well as C4's required shared row/profile checks. This is a candidate schema
fragment, not an already registered native schema execution receipt.

Addressed C4 integration defect TS-C4-URI-1: common C4's role schema refers to
three fragments of https://tree-of-sophia.local/.../semantic-entity-type-registry.schema.json,
but the actual selected entity schema has the maintained $id
https://treeofsophia.local/ToS/contracts/semantic-entity-type-registry.schema.json.
The maintained backend checks resource.uri == the source $id and supplies no
implicit host alias. The current C4 registry reference check is Unresolvable.
common-role-schema-uri-correction.patch supplies the exact three-ref correction
for the common owner; it is not applied to shared source. The candidate corrected
composition is checked separately from the current C4 failure. This affects
native role registry composition, not the accepted TS mathematics/erratum.

The actual command-owned NativeTsSourceOwner implementation must:

1. Hold the real CommandContext, immutable CorpusCutReader, OwnerTextContext,
   PrivateAssessmentSources, CutWorkerSchemaExecutor, original OperationBudget,
   deadline and cancellation. Construct NativeRoleResolver from those frames
   and actual RoleRegistrySelection; never fabricate NativeRoleRecord or a
   successful access/schema report.
2. Use NativeRole::TsDefinition/TsValueProfile/TsOperationResult for the three
   NativeTsRole variants, passing the actual selected entity-registry kind_binding
   and exact source Record reference. No current/latest substitution. Native
   role registry and entity registry are distinct refs in NativeTsSelection.
3. Retain the actual source Record's PublishedStrict JsonDocument and exact
   history/source evidence. A duplicate-collapsing serde Value or parsing a new
   self-authored wrapper cannot certify the original strict source. The source
   reader/holder owns this input; the TS store only borrows it.
4. Retain all source/currentness frames while the immutable checked native
   records/documents are cached. resolve may return a cached native holder only
   after rechecking the same actual current frame. Fences still call the real
   NativeRoleResolver::check_current / PrivateAssessmentSources::verify_current.
   No callback Ok(()) or wire access_allowed flag substitutes for those calls.
5. Derive NativeTsDescriptor from the actual readonly descriptor and selected
   schema resource: record_type/schema_version from the real Record; source
   schema_ref from the declared profile; HTTPS $id from the actual schema bytes
   in the held worker/schema set. Never build a descriptor from the request.
   NativeTsSelection binds source revision, input identity, selection ref,
   entity-registry ref, role-registry ref and actual prepared schema binding.
6. Execute actual registered source schema + selected semantic/required
   membership and exact history before returning SelectedNativeTsRecord. Charge
   reads/schema work once to the original native OperationBudget and the admitted
   TS sub-budget; physical deduplication does not assert logical completeness.
7. Call NativeTsStore::check_output for current protected use, retaining original
   K0 input/operator/profile/context/selection/dependency bindings. Whole native
   Record verification -> /body codec -> typed DirectDomainValueV1 -> existing
   independent K0 exact output/profile/codomain checks are separate obligations.
   Input-question alignment, actual rule/domain applicability and exact order
   witness/cycle replay remain with the source-bound producer.
8. Keep the original NativeTsSession in a caller-owned RefCell. Its spent ledger
   survives failed preparation. NativeTsUsage records monotone logical charges;
   account them to the same native parent invocation without treating them as
   independent fresh limits. check_output conservatively reserves the accepted
   pure check's whole local allowance before use; resolver fees are additional.

contracts/native-authoring-inputs.json supplies ten genuine authored candidate
source-create inputs: five bounded rule descriptions and five value profiles.
Their proposed IDs/version1 must be collision/history checked by the real native
source-create gate. Candidate canonical content hashes are integrity evidence,
not issued ExactRefs, source receipts, native selection or authority. All ten
whole Records were checked against the actual common981593 schemas by a Python
reference validator; no native schema-worker execution is claimed.

No operation-result Record, Completed K0 run or creation/admission receipt is
manufactured. After real source creation/selection, native producer execution
must generate the operation result and its actual pins, preserving refusal/
partial execution states and original provenance. The existing real JGB Record
test verifies content binding only; its note explicitly does not establish
calendar, exact days or continuous actual extent.

Actual caller-held frames and source-create/registration outputs are still
unavailable to this TS session. This is a concrete native-host/source gate, not
an unresolved model criterion or a reason to redo accepted pure TS/K0 theory.
