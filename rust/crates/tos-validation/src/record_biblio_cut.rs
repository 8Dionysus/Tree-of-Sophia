//! Source-cut composition of existing record rules and bounded schema plans.
//! Retained cuts are read to EOF but never enter current identity joins.
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::{Digest256, RelativePath, SourceRevision};
use tos_source_store::{CorpusCutReader, SourceMembershipV1, SourcePresenceV1};
use crate::executor::{BoundedSchemaExecutor, ExactWorkerIdentity, ExecutorBudget, ExecutorFailure, ExecutorOutcome};
use crate::item_rules::{ItemLimits, ItemRefusal};
use crate::record_rules::{BoundedSchemaVerdict, BoundedMemberSchemaEvidence, RecordFactBudget, RecordFamily, RecordFamilyReport, RecordGlobalJoin, RecordObservation, RecordRuleError, RecordSchema, RecordSink, PathReferenceCheck};
use crate::{FormatProfile, SchemaResource};

const REGISTRY: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const REGISTRY_SCHEMA: &str = "ToS/contracts/semantic-entity-type-registry.schema.json";

/// Resource-plan execution reuses the same disposable Draft2020-12 worker;
/// common-schema plans do not fit a contract-path-only schema interface.
pub struct BiblioRecordExecutor {
    pub worker: ExactWorkerIdentity,
    pub budget: ExecutorBudget,
    pub profile: FormatProfile,
    pub max_executions: usize,
    executions: usize,
}
impl BiblioRecordExecutor {
    pub fn new(worker: ExactWorkerIdentity, budget: ExecutorBudget, profile: FormatProfile, max_executions: usize) -> Self {
        Self { worker, budget, profile, max_executions, executions: 0 }
    }
    fn evaluate(&mut self, resources: &[SchemaResource], root: &str, raw: &[u8], expected_set: Digest256, limits: ItemLimits, cancelled: &AtomicBool) -> Result<BoundedSchemaVerdict, ItemRefusal> {
        check(limits.deadline, cancelled)?;
        if self.executions >= self.max_executions || raw.len() > limits.max_member_bytes { return Err(ItemRefusal::Budget); }
        self.executions += 1;
        let mut budget = self.budget;
        budget.execution_wall = budget.execution_wall.min(limits.deadline.checked_duration_since(Instant::now()).ok_or(ItemRefusal::Deadline)?);
        let result = BoundedSchemaExecutor::evaluate_cancellable(&self.worker, resources, self.profile, root, raw, budget, cancelled);
        check(limits.deadline, cancelled)?;
        let (identity, valid) = match result {
            ExecutorOutcome::SchemaValid(identity) => (identity, true),
            ExecutorOutcome::SchemaInvalid(identity) => (identity, false),
            ExecutorOutcome::Indeterminate { reason: ExecutorFailure::Timeout, .. } => return Err(ItemRefusal::Deadline),
            ExecutorOutcome::Indeterminate { reason: ExecutorFailure::Cancelled, .. } => return Err(ItemRefusal::Source("record schema cancelled".into())),
            other => return Err(ItemRefusal::Unsupported(format!("record schema incomplete: {other:?}"))),
        };
        if identity.worker_sha256 != self.worker.sha256 || identity.instance_sha256 != Digest256::of_bytes(raw) || identity.schema_set_sha256 != expected_set || identity.profile != self.profile {
            return Err(ItemRefusal::Source("record worker evidence mismatch".into()));
        }
        Ok(BoundedSchemaVerdict { instance_sha256: identity.instance_sha256, schema_set_digest: identity.schema_set_sha256, format_profile: identity.profile, root_uri: root.into(), worker_protocol_id: "tos_bounded_schema_worker_v1".into(), worker_binary_digest: identity.worker_sha256, valid })
    }
}

#[derive(Debug, Clone)]
pub struct BiblioCurrentRecord {
    pub path: String,
    pub kind: String,
    pub value: serde_json::Value,
}
pub struct SourceCutRecordReport {
    pub source_revision: SourceRevision,
    pub current_membership: SourceMembershipV1,
    pub retained_memberships: Vec<(SourceRevision, SourceMembershipV1)>,
    pub records: BTreeMap<String, BiblioCurrentRecord>,
    pub observations: Vec<RecordObservation>,
    pub record_family: RecordFamilyReport,
    pub global_issues: u64,
    pub retained_profile_limits: Vec<String>,
}

struct BoundedSink<'a> { rows: Vec<RecordObservation>, bytes: usize, cap: usize, issues: usize, max_issues: usize, deadline: Instant, cancelled: &'a AtomicBool }
// The protected sink checks cancellation and deadline on each observation.
impl BoundedSink<'_> {
    fn emit(&mut self, row: RecordObservation) -> Result<(), RecordRuleError> {
        // The caller checks cancellation around each member and join. The sink
        // additionally checks deadline on each output, including long joins.
        if Instant::now() >= self.deadline { return Err(RecordRuleError::Sink { detail: "deadline".into() }); }
        if self.cancelled.load(Ordering::Relaxed) { return Err(RecordRuleError::Sink { detail: "cancelled".into() }); }
        if matches!(row, RecordObservation::Issue { .. }) {
            if self.issues >= self.max_issues { return Err(RecordRuleError::Budget { code: "record_issue_sink" }); }
            self.issues += 1;
        }
        let cost = format!("{row:?}").len().checked_add(64).ok_or(RecordRuleError::Budget { code: "biblio_sink" })?;
        self.bytes = self.bytes.checked_add(cost).filter(|n| *n <= self.cap).ok_or(RecordRuleError::Budget { code: "biblio_sink" })?;
        self.rows.push(row); Ok(())
    }
}

impl RecordSink for BoundedSink<'_> {
    fn push(&mut self, row: RecordObservation) -> Result<(), String> {
        self.emit(row).map_err(|error| match error {
            RecordRuleError::Budget { .. } => "budget".into(),
            RecordRuleError::Sink { detail } => detail,
            other => format!("{other:?}"),
        })
    }
}

pub fn inspect_records_from_cut(cut: &CorpusCutReader, limits: ItemLimits, cancelled: &AtomicBool, executor: &mut BiblioRecordExecutor) -> Result<SourceCutRecordReport, ItemRefusal> {
    check(limits.deadline, cancelled)?;
    let mut used = 0u64;
    let registry = current(cut, REGISTRY, limits, cancelled, &mut used)?;
    let contract = current(cut, REGISTRY_SCHEMA, limits, cancelled, &mut used)?;
    let mut schemas = Vec::new();
    let mut state = registry.len().checked_add(contract.len()).ok_or(ItemRefusal::Budget)?;
    for metadata in cut.current().members() {
        check(limits.deadline, cancelled)?;
        let path = metadata.path.as_str();
        if path.starts_with("ToS/contracts/") && path.ends_with(".schema.json") && path != REGISTRY_SCHEMA {
            let raw = current(cut, path, limits, cancelled, &mut used)?;
            reserve(&mut state, path.len() + raw.len(), limits.max_state_bytes)?;
            schemas.push((path.to_owned(), raw));
        }
    }
    let fact_budget = RecordFactBudget { max_facts: limits.max_state_bytes as u64 / 32, max_encoded_bytes: limits.max_state_bytes as u64 };
    let (resources, root, set, _) = RecordFamily::registry_schema_plan(&contract, &registry, executor.profile).map_err(record_error)?;
    let evidence = executor.evaluate(&resources, &root, &registry, set, limits, cancelled)?;
    let mut family = RecordFamily::new_with_bounded_registry(&registry, &contract, schemas.iter().map(|(path, raw)| RecordSchema { path, raw }), executor.profile, fact_budget, &evidence).map_err(record_error)?;
    let mut sink = BoundedSink { rows: Vec::new(), bytes: state, cap: limits.max_state_bytes, issues: 0, max_issues: limits.max_issues, deadline: limits.deadline, cancelled };
    family.emit_registry_read(&mut sink).map_err(record_error)?;
    let mut records = BTreeMap::new();
    let mut stream = cut.stream(cut.current().revision()).map_err(store_error)?;
    while let Some(member) = stream.next_member(limits.deadline, cancelled).map_err(store_error)? {
        check(limits.deadline, cancelled)?;
        account(&mut used, member.raw.len(), limits.max_total_bytes)?;
        if member.raw.len() > limits.max_member_bytes { return Err(ItemRefusal::Budget); }
        let path = member.path.as_str();
        if !path.starts_with("ToS/source-witnesses/") || !path.ends_with(".json") { continue; }
        let basename = path.rsplit('/').next().unwrap_or("");
        let semantic = basename.starts_with("semantic-annotation") && basename.ends_with(".json");
        let carrier = family.classify_current_member(path, &member.raw).map_err(record_error)?;
        if carrier.is_none() && !semantic { continue; }
        match family.member_schema_plan(path, &member.raw) {
            Ok(plan) => {
                let route = executor.evaluate(&plan.resources, &plan.route_uri, &member.raw, plan.schema_set_digest, limits, cancelled)?;
                let common = executor.evaluate(&plan.resources, &plan.common_uri, &member.raw, plan.schema_set_digest, limits, cancelled)?;
                family.inspect_member_with_bounded_schema(path, &member.raw, &BoundedMemberSchemaEvidence { route, common }, &mut sink).map_err(record_error)?;
            }
            Err(RecordRuleError::Unsupported { code: "unrecognized_record_basename", .. }) => {
                let plan = family.native_schema_plan(path, &member.raw).map_err(record_error)?;
                let verdict = executor.evaluate(&plan.resources, &plan.root_uri, &member.raw, plan.schema_set_digest, limits, cancelled)?;
                family.inspect_native_with_bounded_schema(path, &member.raw, &verdict, &mut sink).map_err(record_error)?;
            }
            Err(error) => return Err(record_error(error)),
        }
        if let Some(carrier) = carrier {
            // This is a source-authored identity index, never manifest IDs.
            reserve(&mut sink.bytes, path.len() + carrier.id.len() + carrier.kind.len() + member.raw.len() * 3, limits.max_state_bytes)?;
            let value = serde_json::from_slice(&member.raw).map_err(|_| ItemRefusal::Unsupported("record decoded representation".into()))?;
            records.entry(carrier.id).or_insert(BiblioCurrentRecord { path: path.into(), kind: carrier.kind, value });
        }
    }
    let current_membership = stream.coverage().ok_or_else(|| ItemRefusal::Source("record source EOF missing".into()))?;
    // Materialize sorted bounded facts from the protected sink. Larger corpora
    // need the owner's external-sort route rather than unbounded collections.
    let mut owners: Vec<_> = sink.rows.iter().cloned().filter_map(RecordObservation::into_global_id_fact).collect();
    let mut uris: Vec<_> = sink.rows.iter().cloned().filter_map(RecordObservation::into_link_uri_fact).collect();
    let mut refs: Vec<_> = sink.rows.iter().cloned().filter_map(RecordObservation::into_typed_id_ref_fact).collect();
    reserve(&mut sink.bytes, owners.iter().map(|r| r.id.len()+r.kind.len()+r.path.len()+128).sum::<usize>() + uris.iter().map(|r| r.uri.len()+r.id.len()+r.path.len()+128).sum::<usize>() + refs.iter().map(|r| r.target_id.len()+r.expected_kind.len()+r.from_path.len()+128).sum::<usize>(), limits.max_state_bytes)?;
    owners.sort_by(|a,b| a.id.cmp(&b.id)); uris.sort_by(|a,b| a.uri.cmp(&b.uri)); refs.sort_by(|a,b| a.target_id.cmp(&b.target_id));
    let mut global_issues = RecordGlobalJoin::check_id_collisions(owners.clone(), fact_budget, &mut sink).map_err(record_error)?;
    global_issues += RecordGlobalJoin::check_link_uri_collisions(uris, fact_budget, &mut sink).map_err(record_error)?;
    global_issues += RecordGlobalJoin::check_typed_references(owners, refs, fact_budget, &mut sink).map_err(record_error)?;
    let paths: Vec<_> = sink.rows.iter().filter_map(|row| match row { RecordObservation::Reference { from_path, target_path, check } => Some((from_path.clone(), target_path.clone(), *check)), _ => None }).collect();
    for (from_path, target_path, mode) in paths {
        check(limits.deadline, cancelled)?;
        if !target_path.starts_with("ToS/") { continue; }
        let relative = RelativePath::parse(&target_path).map_err(|_| ItemRefusal::Unsupported("record reference path".into()))?;
        let presence = cut.presence(cut.current().revision(), &relative);
        if presence.is_none() || (mode == PathReferenceCheck::FileIfToS && presence != Some(SourcePresenceV1::File)) {
            sink.emit(RecordObservation::Issue { path: from_path, code: "record_path_reference_missing" }).map_err(record_error)?;
            global_issues += 1;
        }
    }
    let mut retained_memberships = Vec::new();
    for snapshot in cut.revisions().skip(1) {
        check(limits.deadline, cancelled)?;
        let mut history = cut.stream(snapshot.revision()).map_err(store_error)?;
        while let Some(member) = history.next_member(limits.deadline, cancelled).map_err(store_error)? { account(&mut used, member.raw.len(), limits.max_total_bytes)?; check(limits.deadline, cancelled)?; }
        reserve(&mut sink.bytes, 128, limits.max_state_bytes)?;
        retained_memberships.push((snapshot.revision(), history.coverage().ok_or_else(|| ItemRefusal::Source("retained record EOF missing".into()))?));
    }
    check(limits.deadline, cancelled)?;
    Ok(SourceCutRecordReport { source_revision: cut.current().revision(), current_membership, retained_memberships, records, observations: sink.rows, record_family: family.finish(), global_issues, retained_profile_limits: vec!["retained EOF verifies carrier bytes only; frozen registry/profile and native compound lineage need owner verification".into()] })
}

pub(crate) fn check(deadline: Instant, cancelled: &AtomicBool) -> Result<(), ItemRefusal> {
    if cancelled.load(Ordering::Relaxed) { return Err(ItemRefusal::Source("bibliography cancelled".into())); }
    if Instant::now() >= deadline { return Err(ItemRefusal::Deadline); } Ok(())
}
pub(crate) fn reserve(used: &mut usize, amount: usize, max: usize) -> Result<(), ItemRefusal> { *used = used.checked_add(amount).filter(|n| *n <= max).ok_or(ItemRefusal::Budget)?; Ok(()) }
pub(crate) fn account(used: &mut u64, amount: usize, max: u64) -> Result<(), ItemRefusal> { *used = used.checked_add(amount as u64).filter(|n| *n <= max).ok_or(ItemRefusal::Budget)?; Ok(()) }
pub(crate) fn current(cut: &CorpusCutReader, path: &str, limits: ItemLimits, cancelled: &AtomicBool, used: &mut u64) -> Result<Vec<u8>, ItemRefusal> {
    check(limits.deadline, cancelled)?;
    let relative = RelativePath::parse(path).map_err(|_| ItemRefusal::Unsupported("bibliographic source path".into()))?;
    let member = cut.read_member(cut.current().revision(), &relative, limits.max_member_bytes as u64, limits.deadline, cancelled).map_err(store_error)?;
    account(used, member.raw.len(), limits.max_total_bytes)?; Ok(member.raw)
}
pub(crate) fn store_error(error: tos_source_store::StoreError) -> ItemRefusal {
    use tos_source_store::StoreErrorCode;
    match error.code { StoreErrorCode::BudgetExceeded => ItemRefusal::Budget, StoreErrorCode::UnsupportedFormat | StoreErrorCode::UnsupportedPlatform => ItemRefusal::Unsupported(error.to_string()), _ => ItemRefusal::Source(error.to_string()) }
}
fn record_error(error: RecordRuleError) -> ItemRefusal { match error { RecordRuleError::Budget { .. } => ItemRefusal::Budget, RecordRuleError::Sink { detail } if detail == "budget" => ItemRefusal::Budget, RecordRuleError::Sink { detail } if detail == "deadline" => ItemRefusal::Deadline, RecordRuleError::Sink { detail } if detail == "cancelled" => ItemRefusal::Source("record family cancelled".into()), other => ItemRefusal::Unsupported(format!("{other:?}")) } }
