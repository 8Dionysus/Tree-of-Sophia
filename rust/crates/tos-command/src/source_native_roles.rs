//! Source-backed formal-role records over the maintained native Record reader.
//! No role row, descriptor, exact pin or returned value grants current use.
use crate::{
    source_command::{
        self as cmd, CommandContext, SourceCommandError as Error, SourceCommandResult as Result,
    },
    source_native_cli::private_assessment::OperationBudget,
    source_private_assessment_sources::PrivateAssessmentSources,
    source_revisions::{self, ResolvedRecordVersion},
    source_text_owner::OwnerTextContext,
};
use std::{
    collections::BTreeSet,
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};
use tos_foundation::{
    CanonicalProfile, Digest256, ExactRecordRef, FoundationError, FoundationErrorCode, JsonLimits,
    JsonValue, RelativePath, SourceRevision,
    canonical_bytes_v1_with_state_budget_and_visits_and_admission,
};
use tos_source_store::CorpusCutReader;
use tos_validation::{item_rules::ItemLimits, source_cut::CutWorkerSchemaExecutor};

const ENTITIES: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const ENTITY_SCHEMA: &str = "ToS/contracts/semantic-entity-type-registry.schema.json";
const ROLE_SCHEMA: &str = "ToS/contracts/native-role-bindings.schema.json";

/// Dispatch roles are local to these consumers, not a universal type system.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeRole {
    Signature,
    Predicate,
    QualificationProfile,
    Context,
    Scope,
    CriterionPolicy,
    JointCarrier,
    Scenario,
    TsDefinition,
    TsValueProfile,
    TsOperationResult,
}
impl NativeRole {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Signature => "signature",
            Self::Predicate => "predicate",
            Self::QualificationProfile => "qualification-profile",
            Self::Context => "context",
            Self::Scope => "scope",
            Self::CriterionPolicy => "criterion-policy",
            Self::JointCarrier => "joint-carrier",
            Self::Scenario => "scenario",
            Self::TsDefinition => "ts-definition",
            Self::TsValueProfile => "ts-value-profile",
            Self::TsOperationResult => "ts-operation-result",
        }
    }
}

/// Selected source declaration, not a wire-selected configuration or authority.
pub(crate) struct RoleRegistrySelection<'a> {
    pub(crate) path: &'a str,
    pub(crate) exact: &'a ExactRecordRef,
}

/// A mechanically checked native result with original source/history evidence.
/// Only this module constructs it. Its owner must fence again before later use.
pub(crate) struct NativeRoleRecord {
    role: NativeRole,
    exact: ExactRecordRef,
    kind_binding: ExactRecordRef,
    role_registry: ExactRecordRef,
    role_registry_path: String,
    source_revision: SourceRevision,
    resolved: ResolvedRecordVersion,
}
impl NativeRoleRecord {
    pub(crate) fn role(&self) -> NativeRole {
        self.role
    }
    pub(crate) fn exact(&self) -> &ExactRecordRef {
        &self.exact
    }
    pub(crate) fn kind_binding(&self) -> &ExactRecordRef {
        &self.kind_binding
    }
    pub(crate) fn role_registry(&self) -> &ExactRecordRef {
        &self.role_registry
    }
    pub(crate) fn role_registry_path(&self) -> &str {
        &self.role_registry_path
    }
    pub(crate) fn source_revision(&self) -> SourceRevision {
        self.source_revision
    }
    pub(crate) fn evidence(&self) -> &ResolvedRecordVersion {
        &self.resolved
    }
}

/// Borrow the actual protected native frames; there is no JSON/boolean issuer.
/// This first host is the immutable-cut native route. Candidate/PG identities
/// cannot be coerced into SourceRevision to enter it.
pub(crate) struct NativeRoleResolver<'a> {
    ctx: &'a CommandContext,
    cut: &'a CorpusCutReader,
    owner: &'a OwnerTextContext,
    sources: &'a PrivateAssessmentSources,
    worker: &'a mut CutWorkerSchemaExecutor,
    budget: &'a mut OperationBudget,
    entities: JsonValue,
    entity_digest: Option<Digest256>,
    bindings: JsonValue,
    role_registry: ExactRecordRef,
    role_registry_path: String,
    deadline: Instant,
    cancelled: &'a AtomicBool,
}
impl<'a> NativeRoleResolver<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        ctx: &'a CommandContext,
        cut: &'a CorpusCutReader,
        owner: &'a OwnerTextContext,
        sources: &'a PrivateAssessmentSources,
        worker: &'a mut CutWorkerSchemaExecutor,
        budget: &'a mut OperationBudget,
        registry: RoleRegistrySelection<'_>,
        deadline: Instant,
        cancelled: &'a AtomicBool,
    ) -> Result<Self> {
        budget.limits()?;
        ctx.check()?;
        sources.verify_current(owner, ctx, cut, deadline, cancelled)?;
        if worker.source_revision() != ctx.base_revision
            || cut.current().revision() != ctx.base_revision
        {
            return Err(Error::Conflict(
                "native role source/worker selection differs",
            ));
        }
        // Verify custody of every selected authored input, including schemas,
        // role declarations and retained history. Charge BEFORE reading copies.
        // Physical membership alone grants neither semantic membership nor use.
        for file in ctx
            .files
            .iter()
            .filter(|f| f.path.as_str().starts_with("ToS/"))
        {
            budget.charge_bytes(file.raw.len())?;
            budget.charge_work(file.raw.len())?;
            let actual = cut
                .read_member(
                    ctx.base_revision,
                    &file.path,
                    file.raw.len() as u64,
                    deadline,
                    cancelled,
                )
                .map_err(|_| {
                    Error::Unsupported("native role selected source custody incomplete")
                })?;
            if actual.raw != file.raw {
                return Err(Error::Conflict("native role selected source bytes differ"));
            }
        }
        let entity_path = RelativePath::parse(ENTITIES)
            .map_err(|_| Error::Invalid("native entity registry path"))?;
        let entity_raw = ctx
            .file(&entity_path)?
            .ok_or(Error::Unsupported("native entity registry is not selected"))?;
        budget.charge_work(entity_raw.len())?;
        let entities =
            source_revisions::validate_source_profile_registry(worker, deadline, cancelled, ctx)?;
        let path = RelativePath::parse(registry.path)
            .map_err(|_| Error::Invalid("native role registry path"))?;
        if !registry
            .path
            .starts_with("ToS/doctrine/semantic-interchange/")
            || !registry.path.ends_with(".json")
        {
            return Err(Error::Invalid("native role registry source home"));
        }
        let raw = ctx
            .file(&path)?
            .ok_or(Error::Unsupported("native role registry is not selected"))?;
        budget.charge_bytes(raw.len())?;
        budget.charge_work(raw.len())?;
        let bindings = cmd::parse(raw)?;
        check_pin(
            &bindings,
            registry.exact,
            "registry_id",
            "registry_version",
            budget,
            deadline,
            cancelled,
        )?;
        source_revisions::schema_for_source_path_worker(
            worker,
            deadline,
            cancelled,
            ctx,
            &[ROLE_SCHEMA.into(), ENTITY_SCHEMA.into()],
            ROLE_SCHEMA,
            registry.path,
            &bindings,
        )?;
        let row_work = cmd::array(&bindings, "role_bindings")?
            .len()
            .checked_mul(cmd::array(&entities, "types")?.len())
            .ok_or(Error::Unsupported("native role dispatch work overflow"))?;
        budget.charge_work(row_work)?;
        check_role_rows(&bindings, &entities)?;
        sources.verify_current(owner, ctx, cut, deadline, cancelled)?;
        reserve_reference_copies(budget, &[registry.exact], registry.path)?;
        Ok(Self {
            ctx,
            cut,
            owner,
            sources,
            worker,
            budget,
            entities,
            entity_digest: None,
            bindings,
            role_registry: registry.exact.clone(),
            role_registry_path: registry.path.to_owned(),
            deadline,
            cancelled,
        })
    }

    pub(crate) fn check_current(&self) -> Result<()> {
        self.budget.limits()?;
        if self.cancelled.load(Ordering::Relaxed) || Instant::now() >= self.deadline {
            return Err(Error::Denied("native role operation cancelled or expired"));
        }
        self.sources.verify_current(
            self.owner,
            self.ctx,
            self.cut,
            self.deadline,
            self.cancelled,
        )
    }

    pub(crate) fn resolve(
        &mut self,
        role: NativeRole,
        kind_binding: &ExactRecordRef,
        exact: &ExactRecordRef,
    ) -> Result<NativeRoleRecord> {
        self.check_current()?;
        // This private source value is immutable for this resolver's lifetime.
        // Reuse only its admitted content digest, never a current-use grant.
        let entity_digest = match self.entity_digest {
            Some(digest) => digest,
            None => {
                let digest = admitted_digest(
                    &self.entities,
                    self.budget,
                    self.deadline,
                    self.cancelled,
                )?;
                self.entity_digest = Some(digest);
                digest
            }
        };
        check_pin_fields(
            &self.entities,
            kind_binding,
            "registry_id",
            "registry_version",
            entity_digest,
            self.budget,
        )?;
        let scan_work = self
            .ctx
            .files
            .iter()
            .filter(|f| f.path.as_str().starts_with("ToS/"))
            .try_fold(0usize, |n, f| n.checked_add(f.raw.len()))
            .ok_or(Error::Unsupported(
                "native role owner-discovery work overflow",
            ))?;
        self.budget.charge_work(scan_work)?;
        let allowance = self.budget.limits()?;
        let read_cap = allowance
            .batch
            .max_total_raw_bytes
            .min(allowance.max_work / 4);
        if read_cap == 0 {
            return Err(Error::Unsupported("native role remaining read/work budget"));
        }
        let limits = ItemLimits {
            max_member_bytes: allowance.max_input_bytes.min(read_cap),
            max_total_bytes: read_cap as u64,
            max_state_bytes: allowance.max_input_bytes.min(read_cap),
            max_issues: 256,
            deadline: allowance.deadline.min(self.deadline),
        };
        let wire_cost = exact
            .id
            .as_str()
            .len()
            .checked_add(256)
            .ok_or(Error::Unsupported("native role wire pin size overflow"))?;
        self.budget.charge_work(wire_cost)?;
        self.budget.charge_bytes(wire_cost)?;
        let pin = wire_pin(exact);
        let mut resolved = source_revisions::resolve_role_record_version_from_cut(
            self.ctx,
            self.cut,
            &pin,
            limits,
            self.worker,
            self.cancelled,
        )?;
        self.budget.charge_bytes(
            usize::try_from(resolved.bytes_read)
                .map_err(|_| Error::Unsupported("native role read accounting overflow"))?,
        )?;
        self.budget.charge_work(
            usize::try_from(resolved.bytes_read)
                .ok()
                .and_then(|n| n.checked_mul(4))
                .ok_or(Error::Unsupported("native role read/work overflow"))?,
        )?;
        self.budget.charge_bytes(resolved.returned_state_bytes)?;
        // The maintained reader verifies exact current/retained versions, source
        // schema and history. Recheck exact content and role after that read.
        check_pin(
            &resolved.record,
            exact,
            "record_id",
            "record_version",
            self.budget,
            self.deadline,
            self.cancelled,
        )?;
        let descriptor_work = cmd::array(&self.entities, "types")?
            .len()
            .checked_add(cmd::array(&self.bindings, "role_bindings")?.len())
            .ok_or(Error::Unsupported("native role descriptor work overflow"))?;
        self.budget.charge_work(descriptor_work)?;
        let descriptor = resolved.readonly_descriptor(&self.entities)?;
        check_resolved_role(&self.bindings, role, &resolved.record, &descriptor)?;
        // Counting and emitting are both prospectively admitted by the same
        // owner before traversal/serialization; no serialize-then-charge pass.
        drop(admitted_canonical(
            &resolved.record,
            self.budget,
            self.deadline,
            self.cancelled,
        )?);
        drop(admitted_canonical(
            &descriptor,
            self.budget,
            self.deadline,
            self.cancelled,
        )?);
        reserve_reference_copies(
            self.budget,
            &[exact, kind_binding, &self.role_registry],
            &self.role_registry_path,
        )?;
        resolved.descriptor = Some(descriptor);
        self.check_current()?;
        Ok(NativeRoleRecord {
            role,
            exact: exact.clone(),
            kind_binding: kind_binding.clone(),
            role_registry: self.role_registry.clone(),
            role_registry_path: self.role_registry_path.clone(),
            source_revision: self.ctx.base_revision,
            resolved,
        })
    }
}

fn admitted_canonical(
    value: &JsonValue,
    budget: &mut OperationBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>> {
    // Check the original remaining owner allowance BEFORE inspecting value.
    let remaining = budget.limits()?;
    let defaults = JsonLimits::default();
    let limits = JsonLimits {
        max_bytes: remaining.max_input_bytes,
        max_visits: defaults.max_visits.min(remaining.max_work),
        ..defaults
    };
    let mut admission_error = None;
    let encoded = {
        let mut check = || {
            if cancelled.load(Ordering::Relaxed)
                || Instant::now() >= deadline.min(remaining.deadline)
            {
                Err(FoundationError::new(
                    FoundationErrorCode::BudgetExceeded,
                    "native role canonical cutoff",
                ))
            } else {
                Ok(())
            }
        };
        let mut admit = |bytes: usize, visits: usize| {
            let failure = || {
                FoundationError::new(
                    FoundationErrorCode::BudgetExceeded,
                    "native role canonical admission",
                )
            };
            let work = bytes.checked_add(visits).ok_or_else(failure)?;
            // Retain every admitted prefix on later refusal. Count-pass bytes
            // reserve output before its allocation; emit prefixes are charged
            // too. The same owner budget is never cloned, reset or enlarged.
            if let Err(error) = budget
                .charge_work(work)
                .and_then(|_| budget.charge_bytes(bytes))
            {
                admission_error = Some(error);
                return Err(failure());
            }
            Ok(())
        };
        canonical_bytes_v1_with_state_budget_and_visits_and_admission(
            value,
            CanonicalProfile::SourceRecordDigestV1,
            limits,
            remaining.max_input_bytes,
            &mut check,
            &mut admit,
        )
    };
    encoded.map(|(bytes, _)| bytes).map_err(|_| {
        admission_error.unwrap_or(Error::Invalid("native role canonical source record"))
    })
}
fn admitted_digest(
    value: &JsonValue,
    budget: &mut OperationBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Digest256> {
    let encoded = admitted_canonical(value, budget, deadline, cancelled)?;
    budget.charge_work(encoded.len())?;
    Ok(Digest256::of_bytes(&encoded))
}
fn check_pin(
    value: &JsonValue,
    expected: &ExactRecordRef,
    id: &str,
    version: &str,
    budget: &mut OperationBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<()> {
    let actual = admitted_digest(value, budget, deadline, cancelled)?;
    check_pin_fields(value, expected, id, version, actual, budget)
}
fn check_pin_fields(
    value: &JsonValue,
    expected: &ExactRecordRef,
    id: &str,
    version: &str,
    actual: Digest256,
    budget: &mut OperationBudget,
) -> Result<()> {
    let fields = value
        .as_object()
        .ok_or(Error::Invalid("native role pinned object"))?;
    budget.charge_work(
        fields
            .len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(expected.id.as_str().len()))
            .ok_or(Error::Unsupported("native role pin comparison work overflow"))?,
    )?;
    let version_value = cmd::field(value, version)?;
    if let JsonValue::Number(number) = version_value {
        budget.charge_work(number.lexeme.len())?;
    }
    if cmd::text(value, id)? != expected.id.as_str()
        || version_value.as_u64() != Some(expected.version.get())
        || actual != expected.digest
    {
        return Err(Error::Conflict(
            "native role exact identity/version/digest differs",
        ));
    }
    Ok(())
}
fn reserve_reference_copies(
    budget: &mut OperationBudget,
    refs: &[&ExactRecordRef],
    path: &str,
) -> Result<()> {
    let bytes = refs
        .iter()
        .try_fold(path.len(), |n, r| n.checked_add(r.id.as_str().len()))
        .ok_or(Error::Unsupported("native role reference-copy size overflow"))?;
    budget.charge_work(bytes)?;
    budget.charge_bytes(bytes)
}
fn wire_pin(exact: &ExactRecordRef) -> JsonValue {
    cmd::object(vec![
        ("id", cmd::string(exact.id.as_str())),
        ("version", cmd::number(exact.version.get())),
        ("digest", cmd::string(&exact.digest.to_prefixed())),
    ])
}

fn check_role_rows(bindings: &JsonValue, entities: &JsonValue) -> Result<()> {
    let mut keys = BTreeSet::new();
    for row in cmd::array(bindings, "role_bindings")? {
        let key = (
            cmd::text(row, "role")?,
            cmd::text(row, "record_type")?,
            cmd::text(row, "schema_version")?,
        );
        if !keys.insert(key) {
            return Err(Error::Conflict("ambiguous native role binding"));
        }
        let owners = cmd::array(entities, "types")?
            .iter()
            .filter(|e| cmd::text(e, "type_id").ok() == cmd::text(row, "type_id").ok())
            .collect::<Vec<_>>();
        if owners.len() != 1 {
            return Err(Error::Unsupported("native role type has no selected owner"));
        }
        let profile = cmd::field(owners[0], "source_record_profile")?;
        if cmd::text(profile, "record_type")? != key.1 {
            return Err(Error::Conflict("native role kind mapping differs"));
        }
        let routes = cmd::array(profile, "schemas")?
            .iter()
            .filter(|r| cmd::text(r, "schema_version").ok() == Some(key.2))
            .collect::<Vec<_>>();
        if routes.len() != 1 || cmd::text(routes[0], "schema_ref")? != cmd::text(row, "schema_ref")?
        {
            return Err(Error::Unsupported(
                "native role exact schema route absent or ambiguous",
            ));
        }
    }
    Ok(())
}
fn check_resolved_role(
    bindings: &JsonValue,
    role: NativeRole,
    record: &JsonValue,
    descriptor: &JsonValue,
) -> Result<()> {
    let rows = cmd::array(bindings, "role_bindings")?
        .iter()
        .filter(|r| {
            cmd::text(r, "role").ok() == Some(role.as_str())
                && cmd::text(r, "record_type").ok() == cmd::text(record, "record_type").ok()
                && cmd::text(r, "schema_version").ok() == cmd::text(record, "schema_version").ok()
        })
        .collect::<Vec<_>>();
    if rows.len() != 1 {
        return Err(Error::Unsupported(
            "requested native role has no unique source binding",
        ));
    }
    let row = rows[0];
    if cmd::text(descriptor, "adapter")? != "declared-profile"
        || cmd::text(descriptor, "identity_field")? != "record_id"
        || cmd::text(descriptor, "profile_type_id")? != cmd::text(row, "type_id")?
        || cmd::text(descriptor, "source_schema_ref")? != cmd::text(row, "schema_ref")?
    {
        return Err(Error::Conflict("resolved native role descriptor differs"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_work_is_admitted_before_output_and_retains_spent_prefix() {
        let deadline = Instant::now() + std::time::Duration::from_secs(30);
        let invocation = serde_json::json!({"budgets": {
            "worker_cpu_seconds": 1,
            "worker_address_space_bytes": 1_073_741_824u64
        }});
        // A numerical budget fixture is not a source/authority frame factory.
        let mut budget = OperationBudget::new(&invocation, deadline).unwrap();
        let cancelled = AtomicBool::new(false);
        let value = cmd::parse(br#"{"a":1}"#).unwrap();
        let expected = tos_foundation::canonical_bytes_v1(
            &value,
            CanonicalProfile::SourceRecordDigestV1,
            JsonLimits::default(),
        )
        .unwrap();
        assert_eq!(
            admitted_canonical(&value, &mut budget, deadline, &cancelled).unwrap(),
            expected
        );
        let remaining = budget.limits().unwrap().max_work;
        budget.charge_work(remaining - 1).unwrap();
        assert!(admitted_canonical(&value, &mut budget, deadline, &cancelled).is_err());
        // One root visit was admitted; later refusal neither refunds it nor
        // resets the original owner allowance for another canonical attempt.
        assert!(budget.limits().is_err());
        assert!(admitted_digest(&value, &mut budget, deadline, &cancelled).is_err());
    }
    #[test]
    fn native_role_requires_actual_declared_profile_and_requested_role() {
        // Actual source registry. The existing canon Context type is not a
        // declared native metadata profile merely because its label matches.
        let entities = cmd::parse(include_bytes!(
            "../../../../ToS/doctrine/semantic-interchange/entity-types.v1.json"
        ))
        .unwrap();
        let canon = cmd::parse(br#"{"role_bindings":[{"role":"context","type_id":"tos.entity.context","record_type":"context","schema_version":"tos_node_v1","schema_ref":"ToS/contracts/tos-node-contract.schema.json"}]}"#).unwrap();
        assert!(check_role_rows(&canon, &entities).is_err());
        let declared = cmd::parse(br#"{"role_bindings":[{"role":"ts-value-profile","type_id":"tos.entity.ts-native-value-profile","record_type":"ts-native-value-profile","schema_version":"tos_ts_native_value_profile_record_v1","schema_ref":"ToS/contracts/ts-native-value-profile.schema.json"}]}"#).unwrap();
        check_role_rows(&declared, &entities).unwrap();
        let record = cmd::parse(br#"{"record_type":"ts-native-value-profile","schema_version":"tos_ts_native_value_profile_record_v1"}"#).unwrap();
        let descriptor = cmd::parse(br#"{"adapter":"declared-profile","identity_field":"record_id","profile_type_id":"tos.entity.ts-native-value-profile","source_schema_ref":"ToS/contracts/ts-native-value-profile.schema.json"}"#).unwrap();
        check_resolved_role(&declared, NativeRole::TsValueProfile, &record, &descriptor).unwrap();
        assert!(check_resolved_role(&declared, NativeRole::Context, &record, &descriptor).is_err());
        let mut duplicate = declared.clone();
        if let JsonValue::Array(rows) = cmd::field(&declared, "role_bindings").unwrap() {
            cmd::set(
                &mut duplicate,
                "role_bindings",
                JsonValue::Array(vec![rows[0].clone(), rows[0].clone()]),
            )
            .unwrap();
        }
        assert!(check_role_rows(&duplicate, &entities).is_err());
    }
}
