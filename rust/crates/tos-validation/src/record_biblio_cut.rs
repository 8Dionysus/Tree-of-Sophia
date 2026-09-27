//! Source-cut composition of existing record rules and bounded schema plans.
//! Retained cuts are read to EOF but never enter current identity joins.
use crate::executor::{
    BatchStreamBudget, ExactWorkerIdentity, ExecutorBudget, ExecutorFailure, ExecutorOutcome,
    VerifiedWorkerImage,
};
use crate::item_rules::{ItemLimits, ItemRefusal};
use crate::record_rules::{
    BoundedMemberSchemaEvidence, BoundedSchemaVerdict, PathReferenceCheck, RecordFactBudget,
    RecordFamily, RecordFamilyReport, RecordGlobalJoin, RecordObservation, RecordRuleError,
    RecordSchema, RecordSink,
};
use crate::{FormatProfile, SchemaResource};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::{Digest256, RelativePath, SourceRevision};
use tos_source_store::{CorpusCutReader, SourceMembershipV1, SourcePresenceV1};

const REGISTRY: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const REGISTRY_SCHEMA: &str = "ToS/contracts/semantic-entity-type-registry.schema.json";

/// Resource-plan execution reuses the same disposable Draft2020-12 worker;
/// common-schema plans do not fit a contract-path-only schema interface.
/// The exact image is retained through this operation's first deadline; each
/// varying schema plan retains its own digest. A deliberate closure change
/// finalizes/reaps the old child before the same operation-owned image starts
/// another closure, without resetting aggregate CPU, wall, wire or selector caps.
pub struct BiblioRecordExecutor {
    pub worker: ExactWorkerIdentity,
    pub budget: ExecutorBudget,
    pub profile: FormatProfile,
    pub max_executions: usize,
    executions: usize,
    image: Option<VerifiedWorkerImage>,
    operation_budget: BatchStreamBudget,
    finished: bool,
}
impl BiblioRecordExecutor {
    pub fn new(
        worker: ExactWorkerIdentity,
        budget: ExecutorBudget,
        profile: FormatProfile,
        max_executions: usize,
    ) -> Self {
        let mut operation_budget = BatchStreamBudget::laboratory();
        operation_budget.max_chunks = max_executions as u64;
        operation_budget.max_total_units = max_executions as u64;
        operation_budget.batch.total_execution_wall = budget.execution_wall;
        operation_budget.batch.startup_wall = budget.execution_wall;
        operation_budget.batch.per_unit_wall = budget.execution_wall;
        operation_budget.batch.cleanup_grace = budget.cleanup_grace;
        operation_budget.operation_cpu_seconds = budget.cpu_seconds;
        operation_budget.operation_address_space_bytes = budget.address_space_bytes;
        Self {
            worker,
            budget,
            profile,
            max_executions,
            executions: 0,
            image: None,
            operation_budget,
            finished: false,
        }
    }
    pub fn set_operation_budget(&mut self, budget: BatchStreamBudget) -> Result<(), ItemRefusal> {
        if self.executions != 0 || self.image.is_some() || self.finished {
            return Err(ItemRefusal::Unsupported(
                "record operation already started".into(),
            ));
        }
        budget.validate().map_err(|reason| {
            ItemRefusal::Unsupported(format!("record operation envelope: {reason:?}"))
        })?;
        self.operation_budget = budget;
        Ok(())
    }
    pub(crate) fn is_unused(&self) -> bool {
        self.executions == 0 && self.image.is_none() && !self.finished
    }

    pub(crate) fn operation_budget(&self) -> BatchStreamBudget {
        self.operation_budget
    }

    pub fn finish(&mut self, deadline: Instant, cancelled: &AtomicBool) -> Result<(), ItemRefusal> {
        if self.finished {
            return Err(ItemRefusal::Unsupported(
                "record operation finalized".into(),
            ));
        }
        self.finished = true;
        if self.image.is_none() {
            check(deadline, cancelled)?;
        }
        if let Some(image) = self.image.as_mut() {
            image
                .finish(deadline, cancelled)
                .map_err(|reason| match reason {
                    ExecutorFailure::Timeout => ItemRefusal::Deadline,
                    ExecutorFailure::Cancelled => {
                        ItemRefusal::Source("record operation cancelled".into())
                    }
                    other => ItemRefusal::Unsupported(format!(
                        "record operation finalization: {other:?}; original exchange: {:?}", image.exchange_failure()
                    )),
                })?;
        }
        Ok(())
    }
    fn evaluate(
        &mut self,
        resources: &[SchemaResource],
        root: &str,
        raw: &[u8],
        expected_set: Digest256,
        limits: ItemLimits,
        cancelled: &AtomicBool,
    ) -> Result<BoundedSchemaVerdict, ItemRefusal> {
        if self.finished {
            return Err(ItemRefusal::Unsupported(
                "record operation finalized".into(),
            ));
        }
        if let Some(image) = self.image.as_mut() {
            image
                .preflight(limits.deadline, cancelled)
                .map_err(|reason| match reason {
                    ExecutorFailure::Timeout => ItemRefusal::Deadline,
                    ExecutorFailure::Cancelled => {
                        ItemRefusal::Source("record operation cancelled".into())
                    }
                    other => {
                        ItemRefusal::Unsupported(format!("record operation refused: {other:?}; original exchange: {:?}", image.exchange_failure()))
                    }
                })?;
        } else if cancelled.load(Ordering::Relaxed) || Instant::now() >= limits.deadline {
            self.finished = true;
            return Err(ItemRefusal::Deadline);
        }
        check(limits.deadline, cancelled)?;
        if self.executions >= self.max_executions {
            return Err(ItemRefusal::BudgetCheck {check:"record schema executions",used:(self.executions as u64).checked_add(1),limit:Some(self.max_executions as u64)});
        }
        if raw.len() > limits.max_member_bytes {
            return Err(ItemRefusal::BudgetCheck {check:"record schema member bytes",used:Some(raw.len() as u64),limit:Some(limits.max_member_bytes as u64)});
        }
        self.executions += 1;
        let mut budget = self.budget;
        budget.execution_wall = budget.execution_wall.min(
            limits
                .deadline
                .checked_duration_since(Instant::now())
                .ok_or(ItemRefusal::Deadline)?,
        );
        if self
            .image
            .as_ref()
            .is_some_and(|image| !image.matches(&self.worker))
        {
            self.image
                .as_mut()
                .unwrap()
                .poison(ExecutorFailure::WorkerIdentity);
            return Err(ItemRefusal::Source(
                "record operation worker identity changed".into(),
            ));
        }
        let started = Instant::now();
        if self.image.is_none() {
            self.image = Some(
                VerifiedWorkerImage::prepare(&self.worker, budget, limits.deadline, cancelled)
                    .map_err(|reason| match reason {
                        ExecutorFailure::Timeout => ItemRefusal::Deadline,
                        ExecutorFailure::Cancelled => {
                            ItemRefusal::Source("record schema cancelled".into())
                        }
                        other => ItemRefusal::Unsupported(format!(
                            "record worker preparation: {other:?}"
                        )),
                    })?,
            );
        }
        if self.executions == 1 {
            self.image
                .as_mut()
                .unwrap()
                .set_operation_budget(self.operation_budget)
                .map_err(|reason| {
                    ItemRefusal::Unsupported(format!("record operation envelope: {reason:?}"))
                })?;
        }
        budget.execution_wall = budget.execution_wall.saturating_sub(started.elapsed());
        if budget.execution_wall.is_zero() {
            return Err(ItemRefusal::Deadline);
        }
        let result = self.image.as_mut().unwrap().evaluate(
            resources,
            self.profile,
            root,
            raw,
            budget,
            limits.deadline,
            cancelled,
        );
        if matches!(
            &result,
            ExecutorOutcome::SchemaValid(_) | ExecutorOutcome::SchemaInvalid(_)
        ) {
            self.image
                .as_mut()
                .unwrap()
                .preflight(limits.deadline, cancelled)
                .map_err(|reason| match reason {
                    ExecutorFailure::Timeout => ItemRefusal::Deadline,
                    ExecutorFailure::Cancelled => {
                        ItemRefusal::Source("record operation cancelled".into())
                    }
                    other => {
                        ItemRefusal::Unsupported(format!("record operation refused: {other:?}; original exchange: {:?}", self.image.as_ref().and_then(VerifiedWorkerImage::exchange_failure)))
                    }
                })?;
        }
        let (identity, valid) = match result {
            ExecutorOutcome::SchemaValid(identity) => (identity, true),
            ExecutorOutcome::SchemaInvalid(identity) => (identity, false),
            ExecutorOutcome::Indeterminate {
                reason: ExecutorFailure::Timeout,
                ..
            } => return Err(ItemRefusal::Deadline),
            ExecutorOutcome::Indeterminate {
                reason: ExecutorFailure::Cancelled,
                ..
            } => return Err(ItemRefusal::Source("record schema cancelled".into())),
            other => {
                return Err(ItemRefusal::Unsupported(format!(
                    "record schema incomplete: {other:?}"
                )));
            }
        };
        if identity.worker_sha256 != self.worker.sha256
            || identity.instance_sha256 != Digest256::of_bytes(raw)
            || identity.schema_set_sha256 != expected_set
            || identity.profile != self.profile
        {
            return Err(ItemRefusal::Source(
                "record worker evidence mismatch".into(),
            ));
        }
        Ok(BoundedSchemaVerdict {
            instance_sha256: identity.instance_sha256,
            schema_set_digest: identity.schema_set_sha256,
            format_profile: identity.profile,
            root_uri: root.into(),
            worker_protocol_id: "tos_bounded_schema_worker_v1".into(),
            worker_binary_digest: identity.worker_sha256,
            valid,
        })
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

struct BoundedSink<'a> {
    rows: Vec<RecordObservation>,
    bytes: usize,
    cap: usize,
    issues: usize,
    max_issues: usize,
    deadline: Instant,
    cancelled: &'a AtomicBool,
}
// The protected sink checks cancellation and deadline on each observation.
impl BoundedSink<'_> {
    fn emit(&mut self, row: RecordObservation) -> Result<(), RecordRuleError> {
        // The caller checks cancellation around each member and join. The sink
        // additionally checks deadline on each output, including long joins.
        if Instant::now() >= self.deadline {
            return Err(RecordRuleError::Sink {
                detail: "deadline".into(),
            });
        }
        if self.cancelled.load(Ordering::Relaxed) {
            return Err(RecordRuleError::Sink {
                detail: "cancelled".into(),
            });
        }
        if matches!(row, RecordObservation::Issue { .. }) {
            if self.issues >= self.max_issues {
                return Err(RecordRuleError::Budget {
                    code: "record_issue_sink",
                });
            }
            self.issues += 1;
        }
        let cost = format!("{row:?}")
            .len()
            .checked_add(64)
            .ok_or(RecordRuleError::Budget {
                code: "biblio_sink",
            })?;
        self.bytes = self
            .bytes
            .checked_add(cost)
            .filter(|n| *n <= self.cap)
            .ok_or(RecordRuleError::Budget {
                code: "biblio_sink",
            })?;
        self.rows.push(row);
        Ok(())
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

pub fn inspect_records_from_cut(
    cut: &CorpusCutReader,
    limits: ItemLimits,
    cancelled: &AtomicBool,
    executor: &mut BiblioRecordExecutor,
) -> Result<SourceCutRecordReport, ItemRefusal> {
    check(limits.deadline, cancelled)?;
    let mut used = 0u64;
    let registry = current(cut, REGISTRY, limits, cancelled, &mut used)?;
    let contract = current(cut, REGISTRY_SCHEMA, limits, cancelled, &mut used)?;
    let mut schemas = Vec::new();
    let mut state = registry
        .len()
        .checked_add(contract.len())
        .ok_or(ItemRefusal::Budget)?;
    for metadata in cut.current().members() {
        check(limits.deadline, cancelled)?;
        let path = metadata.path.as_str();
        if path.starts_with("ToS/contracts/")
            && path.ends_with(".schema.json")
            && path != REGISTRY_SCHEMA
        {
            let raw = current(cut, path, limits, cancelled, &mut used)?;
            reserve(&mut state, path.len() + raw.len(), limits.max_state_bytes)?;
            schemas.push((path.to_owned(), raw));
        }
    }
    let fact_budget = RecordFactBudget {
        max_facts: limits.max_state_bytes as u64 / 32,
        max_encoded_bytes: limits.max_state_bytes as u64,
    };
    let (resources, root, set, _) =
        RecordFamily::registry_schema_plan(&contract, &registry, executor.profile)
            .map_err(record_error)?;
    let evidence = executor.evaluate(&resources, &root, &registry, set, limits, cancelled)?;
    let mut family = RecordFamily::new_with_bounded_registry(
        &registry,
        &contract,
        schemas.iter().map(|(path, raw)| RecordSchema { path, raw }),
        executor.profile,
        fact_budget,
        &evidence,
    )
    .map_err(record_error)?;
    let mut sink = BoundedSink {
        rows: Vec::new(),
        bytes: state,
        cap: limits.max_state_bytes,
        issues: 0,
        max_issues: limits.max_issues,
        deadline: limits.deadline,
        cancelled,
    };
    family.emit_registry_read(&mut sink).map_err(record_error)?;
    let mut records = BTreeMap::new();
    let mut stream = cut.stream(cut.current().revision()).map_err(store_error)?;
    while let Some(member) = stream
        .next_member(limits.deadline, cancelled)
        .map_err(store_error)?
    {
        check(limits.deadline, cancelled)?;
        account(&mut used, member.raw.len(), limits.max_total_bytes)?;
        if member.raw.len() > limits.max_member_bytes {
            return Err(ItemRefusal::Budget);
        }
        let path = member.path.as_str();
        if !path.starts_with("ToS/source-witnesses/") || !path.ends_with(".json") {
            continue;
        }
        let basename = path.rsplit('/').next().unwrap_or("");
        let semantic = basename.starts_with("semantic-annotation") && basename.ends_with(".json");
        let carrier = family
            .classify_current_member(path, &member.raw)
            .map_err(record_error)?;
        if carrier.is_none() && !semantic {
            continue;
        }
        match family.member_schema_plan(path, &member.raw) {
            Ok(plan) => {
                let route = executor.evaluate(
                    &plan.resources,
                    &plan.route_uri,
                    &member.raw,
                    plan.schema_set_digest,
                    limits,
                    cancelled,
                )?;
                let common = executor.evaluate(
                    &plan.resources,
                    &plan.common_uri,
                    &member.raw,
                    plan.schema_set_digest,
                    limits,
                    cancelled,
                )?;
                family
                    .inspect_member_with_bounded_schema(
                        path,
                        &member.raw,
                        &BoundedMemberSchemaEvidence { route, common },
                        &mut sink,
                    )
                    .map_err(record_error)?;
            }
            Err(RecordRuleError::Unsupported {
                code: "unrecognized_record_basename",
                ..
            }) => {
                let plan = family
                    .native_schema_plan(path, &member.raw)
                    .map_err(record_error)?;
                let verdict = executor.evaluate(
                    &plan.resources,
                    &plan.root_uri,
                    &member.raw,
                    plan.schema_set_digest,
                    limits,
                    cancelled,
                )?;
                family
                    .inspect_native_with_bounded_schema(path, &member.raw, &verdict, &mut sink)
                    .map_err(record_error)?;
            }
            Err(error) => return Err(record_error(error)),
        }
        if let Some(carrier) = carrier {
            // This is a source-authored identity index, never manifest IDs.
            reserve(
                &mut sink.bytes,
                path.len() + carrier.id.len() + carrier.kind.len() + member.raw.len() * 3,
                limits.max_state_bytes,
            )?;
            let value = serde_json::from_slice(&member.raw)
                .map_err(|_| ItemRefusal::Unsupported("record decoded representation".into()))?;
            records.entry(carrier.id).or_insert(BiblioCurrentRecord {
                path: path.into(),
                kind: carrier.kind,
                value,
            });
        }
    }
    let current_membership = stream
        .coverage()
        .ok_or_else(|| ItemRefusal::Source("record source EOF missing".into()))?;
    // Materialize sorted bounded facts from the protected sink. Larger corpora
    // need the owner's external-sort route rather than unbounded collections.
    let mut owners: Vec<_> = sink
        .rows
        .iter()
        .cloned()
        .filter_map(RecordObservation::into_global_id_fact)
        .collect();
    let mut uris: Vec<_> = sink
        .rows
        .iter()
        .cloned()
        .filter_map(RecordObservation::into_link_uri_fact)
        .collect();
    let mut refs: Vec<_> = sink
        .rows
        .iter()
        .cloned()
        .filter_map(RecordObservation::into_typed_id_ref_fact)
        .collect();
    reserve(
        &mut sink.bytes,
        owners
            .iter()
            .map(|r| r.id.len() + r.kind.len() + r.path.len() + 128)
            .sum::<usize>()
            + uris
                .iter()
                .map(|r| r.uri.len() + r.id.len() + r.path.len() + 128)
                .sum::<usize>()
            + refs
                .iter()
                .map(|r| r.target_id.len() + r.expected_kind.len() + r.from_path.len() + 128)
                .sum::<usize>(),
        limits.max_state_bytes,
    )?;
    owners.sort_by(|a, b| a.id.cmp(&b.id));
    uris.sort_by(|a, b| a.uri.cmp(&b.uri));
    refs.sort_by(|a, b| a.target_id.cmp(&b.target_id));
    let mut global_issues =
        RecordGlobalJoin::check_id_collisions(owners.clone(), fact_budget, &mut sink)
            .map_err(record_error)?;
    global_issues += RecordGlobalJoin::check_link_uri_collisions(uris, fact_budget, &mut sink)
        .map_err(record_error)?;
    global_issues += RecordGlobalJoin::check_typed_references(owners, refs, fact_budget, &mut sink)
        .map_err(record_error)?;
    let paths: Vec<_> = sink
        .rows
        .iter()
        .filter_map(|row| match row {
            RecordObservation::Reference {
                from_path,
                target_path,
                check,
            } => Some((from_path.clone(), target_path.clone(), *check)),
            _ => None,
        })
        .collect();
    for (from_path, target_path, mode) in paths {
        check(limits.deadline, cancelled)?;
        if !target_path.starts_with("ToS/") {
            continue;
        }
        let relative = RelativePath::parse(&target_path)
            .map_err(|_| ItemRefusal::Unsupported("record reference path".into()))?;
        let presence = cut.presence(cut.current().revision(), &relative);
        if presence.is_none()
            || (mode == PathReferenceCheck::FileIfToS && presence != Some(SourcePresenceV1::File))
        {
            sink.emit(RecordObservation::Issue {
                path: from_path,
                code: "record_path_reference_missing",
            })
            .map_err(record_error)?;
            global_issues += 1;
        }
    }
    let mut retained_memberships = Vec::new();
    for snapshot in cut.revisions().skip(1) {
        check(limits.deadline, cancelled)?;
        let mut history = cut.stream(snapshot.revision()).map_err(store_error)?;
        while let Some(member) = history
            .next_member(limits.deadline, cancelled)
            .map_err(store_error)?
        {
            account(&mut used, member.raw.len(), limits.max_total_bytes)?;
            check(limits.deadline, cancelled)?;
        }
        reserve(&mut sink.bytes, 128, limits.max_state_bytes)?;
        retained_memberships.push((
            snapshot.revision(),
            history
                .coverage()
                .ok_or_else(|| ItemRefusal::Source("retained record EOF missing".into()))?,
        ));
    }
    check(limits.deadline, cancelled)?;
    Ok(SourceCutRecordReport { source_revision: cut.current().revision(), current_membership, retained_memberships, records, observations: sink.rows, record_family: family.finish(), global_issues, retained_profile_limits: vec!["retained EOF verifies carrier bytes only; frozen registry/profile and native compound lineage need owner verification".into()] })
}

pub(crate) fn check(deadline: Instant, cancelled: &AtomicBool) -> Result<(), ItemRefusal> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(ItemRefusal::Source("bibliography cancelled".into()));
    }
    if Instant::now() >= deadline {
        return Err(ItemRefusal::Deadline);
    }
    Ok(())
}
pub(crate) fn reserve(used: &mut usize, amount: usize, max: usize) -> Result<(), ItemRefusal> {
    let next=used.checked_add(amount);
    *used=next.filter(|n|*n<=max).ok_or(ItemRefusal::BudgetCheck {
        check:"record/bibliography logical state bytes",used:next.map(|n|n as u64),limit:Some(max as u64)})?;
    Ok(())
}
pub(crate) fn account(used: &mut u64, amount: usize, max: u64) -> Result<(), ItemRefusal> {
    let next=used.checked_add(amount as u64);
    *used=next.filter(|n|*n<=max).ok_or(ItemRefusal::BudgetCheck {
        check:"record/bibliography read bytes",used:next,limit:Some(max)})?;
    Ok(())
}
pub(crate) fn current(
    cut: &CorpusCutReader,
    path: &str,
    limits: ItemLimits,
    cancelled: &AtomicBool,
    used: &mut u64,
) -> Result<Vec<u8>, ItemRefusal> {
    check(limits.deadline, cancelled)?;
    let relative = RelativePath::parse(path)
        .map_err(|_| ItemRefusal::Unsupported("bibliographic source path".into()))?;
    let member = cut
        .read_member(
            cut.current().revision(),
            &relative,
            limits.max_member_bytes as u64,
            limits.deadline,
            cancelled,
        )
        .map_err(store_error)?;
    account(used, member.raw.len(), limits.max_total_bytes)?;
    Ok(member.raw)
}
pub(crate) fn store_error(error: tos_source_store::StoreError) -> ItemRefusal {
    use tos_source_store::StoreErrorCode;
    match error.code {
        StoreErrorCode::BudgetExceeded => ItemRefusal::Budget,
        StoreErrorCode::UnsupportedFormat | StoreErrorCode::UnsupportedPlatform => {
            ItemRefusal::Unsupported(error.to_string())
        }
        _ => ItemRefusal::Source(error.to_string()),
    }
}

pub(crate) fn decoded_wire_size(value:&serde_json::Value,limit:usize)->Result<usize,ItemRefusal> {
    serialized_wire_size(limit,|writer|serde_json::to_writer(writer,value))
}
pub(crate) fn serialized_wire_size(limit:usize,write:impl FnOnce(&mut dyn std::io::Write)->serde_json::Result<()>)->Result<usize,ItemRefusal> {
    struct Counter {bytes:usize,limit:usize,exhausted:bool}
    impl std::io::Write for Counter {
        fn write(&mut self,bytes:&[u8])->std::io::Result<usize> {
            let next=self.bytes.checked_add(bytes.len());
            if next.is_none_or(|n|n>self.limit) {self.exhausted=true;return Err(std::io::Error::other("logical serialization budget"));}
            self.bytes=next.unwrap();Ok(bytes.len())
        }
        fn flush(&mut self)->std::io::Result<()> {Ok(())}
    }
    let mut counter=Counter{bytes:0,limit,exhausted:false};
    let outcome=write(&mut counter);
    if counter.exhausted {return Err(ItemRefusal::BudgetCheck{check:"decoded JSON serialization bytes",used:None,limit:Some(limit as u64)});}
    outcome.map_err(|_|ItemRefusal::Unsupported("decoded JSON serialization".into()))?;
    Ok(counter.bytes)
}

// Logical state counts Rust value slots and their retained string/byte payloads.
// BTree/HashMap allocator nodes, buckets, alignment and allocator rounding are
// deliberately not claimed as RSS. Codec byte/depth/visit limits bound parsing
// separately; callers must count each simultaneously retained representation.
pub(crate) fn decoded_state(value: &serde_json::Value) -> Result<usize, ItemRefusal> {
    fn heap(value: &serde_json::Value) -> Option<usize> {
        use serde_json::Value;
        match value {
            Value::Number(n) => Some(n.as_str().len()),
            Value::String(s) => Some(s.len()),
            Value::Array(items) => items.iter().try_fold(
                items.len().checked_mul(std::mem::size_of::<Value>())?,
                |sum, item| sum.checked_add(heap(item)?),
            ),
            Value::Object(items) => items.iter().try_fold(
                items.len().checked_mul(std::mem::size_of::<(String, Value)>())?,
                |sum, (key, item)| sum.checked_add(key.len())?.checked_add(heap(item)?),
            ),
            _ => Some(0),
        }
    }
    std::mem::size_of::<serde_json::Value>().checked_add(heap(value).ok_or(ItemRefusal::Budget)?).ok_or(ItemRefusal::Budget)
}
pub(crate) fn ordered_state(value: &tos_foundation::JsonValue) -> Result<usize, ItemRefusal> {
    fn string(s: &tos_foundation::JsonString) -> Option<usize> {
        s.units().len().checked_mul(std::mem::size_of::<u16>())?.checked_add(s.as_str().map_or(0, str::len))
    }
    fn heap(value: &tos_foundation::JsonValue) -> Option<usize> {
        use tos_foundation::JsonValue;
        match value {
            JsonValue::Number(n) => Some(n.lexeme.len()),
            JsonValue::String(s) => string(s),
            JsonValue::Array(items) => items.iter().try_fold(
                items.len().checked_mul(std::mem::size_of::<JsonValue>())?,
                |sum, item| sum.checked_add(heap(item)?),
            ),
            JsonValue::Object(items) => items.iter().try_fold(
                items.len().checked_mul(std::mem::size_of::<(tos_foundation::JsonString, JsonValue)>())?,
                |sum, (key, item)| sum.checked_add(string(key)?)?.checked_add(heap(item)?),
            ),
            _ => Some(0),
        }
    }
    std::mem::size_of::<tos_foundation::JsonValue>().checked_add(heap(value).ok_or(ItemRefusal::Budget)?).ok_or(ItemRefusal::Budget)
}
// Peak logical strict-parser tree plus duplicate-key index slots/payloads.
// Ancestor object indexes can coexist; summing the actual object indexes is a
// structural upper bound, independent of corpus size or serialized multipliers.
pub(crate) fn ordered_codec_state(value:&tos_foundation::JsonValue)->Result<usize,ItemRefusal> {
    fn indexes(value:&tos_foundation::JsonValue)->Option<usize> {
        use tos_foundation::JsonValue;
        match value {
            JsonValue::Object(items)=>items.iter().try_fold(std::mem::size_of::<std::collections::HashMap<Vec<u16>,usize>>(),|n,(key,value)|n.checked_add(std::mem::size_of::<(Vec<u16>,usize)>())?.checked_add(key.units().len().checked_mul(std::mem::size_of::<u16>())?)?.checked_add(indexes(value)?)),
            JsonValue::Array(items)=>items.iter().try_fold(0usize,|n,v|n.checked_add(indexes(v)?)),
            _=>Some(0),
        }
    }
    ordered_state(value)?.checked_add(indexes(value).ok_or(ItemRefusal::Budget)?).ok_or(ItemRefusal::Budget)
}
// Canonical emission keeps borrowed duplicate-key and sorted-entry indexes.
// Nested object indexes can coexist; this prices their logical slots, without
// cloning keys or interpreting allocator buckets as retained payloads.
pub(crate) fn ordered_emit_state(value:&tos_foundation::JsonValue)->Result<usize,ItemRefusal> {
    use tos_foundation::{JsonString,JsonValue};
    fn indexes(value:&JsonValue)->Option<usize> {match value {
        JsonValue::Object(items)=>items.iter().try_fold(std::mem::size_of::<std::collections::HashSet<&Vec<u16>>>()+std::mem::size_of::<Vec<&(JsonString,JsonValue)>>()+items.len().checked_mul(std::mem::size_of::<&Vec<u16>>()+std::mem::size_of::<&(JsonString,JsonValue)>())?,|n,(_,value)|n.checked_add(indexes(value)?)),
        JsonValue::Array(items)=>items.iter().try_fold(0usize,|n,value|n.checked_add(indexes(value)?)),
        _=>Some(0),
    }}
    indexes(value).ok_or(ItemRefusal::Budget)
}
pub(crate) fn bounded_ordered(
    raw:&[u8],limits:tos_foundation::JsonLimits,available:usize,deadline:Instant,cancelled:&AtomicBool,
)->Result<tos_foundation::JsonValue,ItemRefusal> {
    bounded_ordered_mode(raw,limits,available,deadline,cancelled,tos_foundation::JsonMode::PublishedStrict)
}
fn bounded_ordered_mode(
    raw:&[u8], mut limits:tos_foundation::JsonLimits, available:usize,
    deadline:Instant,cancelled:&AtomicBool,mode:tos_foundation::JsonMode,
)->Result<tos_foundation::JsonValue,ItemRefusal> {
    check(deadline,cancelled)?;
    // During Foundation parsing a decoded key may retain UTF16 both in the
    // ordered tree and the duplicate-key index, plus its cached UTF8. Their
    // total lengths cannot exceed two UTF16 copies and one UTF8 copy of input.
    // Each value visit can own one value slot, one key, one index entry and one
    // object index header. These are logical slots, not hash bucket/RSS bounds.
    let strings=raw.len().checked_mul(2*std::mem::size_of::<u16>()+std::mem::size_of::<u8>()).ok_or(ItemRefusal::Budget)?;
    let slot=std::mem::size_of::<tos_foundation::JsonValue>()
        +std::mem::size_of::<tos_foundation::JsonString>()
        +std::mem::size_of::<(Vec<u16>,usize)>()
        +std::mem::size_of::<std::collections::HashMap<Vec<u16>,usize>>();
    let remaining=available.checked_sub(strings).ok_or(ItemRefusal::BudgetCheck {check:"strict JSON logical string workspace",used:Some(strings as u64),limit:Some(available as u64)})?;
    limits.max_visits=limits.max_visits.min(remaining/slot);
    if limits.max_visits==0 {return Err(ItemRefusal::BudgetCheck {check:"strict JSON logical node workspace",used:Some(slot as u64),limit:Some(remaining as u64)});}
    let result=tos_foundation::parse_json(raw,mode,limits)
        .map_err(|e|if e.code==tos_foundation::FoundationErrorCode::BudgetExceeded {
            ItemRefusal::BudgetCheck {check:"strict JSON codec bytes/depth/visits/integer",used:None,limit:None}
        } else {ItemRefusal::Unsupported(format!("strict JSON: {e:?}"))})?.into_root();
    check(deadline,cancelled)?;
    let state=ordered_state(&result)?;
    if state>available {return Err(ItemRefusal::BudgetCheck {check:"strict JSON retained ordered state",used:Some(state as u64),limit:Some(available as u64)});}
    Ok(result)
}
// Existing legacy JSON consumers keep last-key-wins; this bounds the same
// codec workspace without imposing PublishedStrict duplicate-key semantics.
// Integer text was bounded only by the member bytes in that serde route.
pub(crate) fn bounded_legacy_decoded_state(raw:&[u8],max_bytes:usize,available:usize,deadline:Instant,cancelled:&AtomicBool)->Result<(serde_json::Value,usize),ItemRefusal> {
    let limits=tos_foundation::JsonLimits::new(max_bytes,128,available.max(1),max_bytes.max(1)).map_err(|_|ItemRefusal::Budget)?;
    drop(bounded_ordered_mode(raw,limits,available,deadline,cancelled,tos_foundation::JsonMode::RequestLastWins)?);
    let value=serde_json::from_slice(raw).map_err(|_|ItemRefusal::Unsupported("decoded JSON representation".into()))?;
    let state=decoded_state(&value)?;
    if state>available {return Err(ItemRefusal::BudgetCheck{check:"legacy JSON retained decoded state",used:Some(state as u64),limit:Some(available as u64)});}
    check(deadline,cancelled)?;Ok((value,state))
}
pub(crate) fn bounded_decoded_state(raw:&[u8],limits:tos_foundation::JsonLimits,available:usize,deadline:Instant,cancelled:&AtomicBool)->Result<(serde_json::Value,usize),ItemRefusal> {
    // Strict validation and its duplicate-key index finish before decoding;
    // there is no simultaneous retained Foundation and serde tree here.
    drop(bounded_ordered(raw,limits,available,deadline,cancelled)?);
    let value=serde_json::from_slice(raw).map_err(|_|ItemRefusal::Unsupported("decoded JSON representation".into()))?;
    let state=decoded_state(&value)?;
    if state>available {return Err(ItemRefusal::BudgetCheck {check:"strict JSON retained decoded state",used:Some(state as u64),limit:Some(available as u64)});}
    check(deadline,cancelled)?;
    Ok((value,state))
}
fn record_error(error: RecordRuleError) -> ItemRefusal {
    match error {
        RecordRuleError::Budget { code } => ItemRefusal::BudgetCheck {check:code,used:None,limit:None},
        RecordRuleError::Sink { detail } if detail == "budget" => ItemRefusal::Budget,
        RecordRuleError::Sink { detail } if detail == "deadline" => ItemRefusal::Deadline,
        RecordRuleError::Sink { detail } if detail == "cancelled" => {
            ItemRefusal::Source("record family cancelled".into())
        }
        other => ItemRefusal::Unsupported(format!("{other:?}")),
    }
}

// Owned enum slot and owned string payloads; no debug serialization or allocator estimate.
pub(crate) fn predicate_state(read:&crate::PredicateRead)->Result<usize,ItemRefusal> {
    use crate::PredicateRead::*;
    let strings:&[&str]=match read {
        ExactRecord{id,version,digest}=>&[id,version,digest],
        ExactPath{path,digest}=>&[path,digest],
        ExactBytes{locator,digest}=>&[locator,digest],
        IdentityKey{namespace,key,..}|AbsentKey{namespace,key}=>&[namespace,key],
        RefEndpoint{endpoint_type,id,..}=>&[endpoint_type,id],
        UniqueKey{namespace,key,owner}=>&[namespace,key,owner],
        Range{namespace,lower,upper,generation}=>&[namespace,lower,upper,generation],
        Prefix{namespace,prefix,generation}=>&[namespace,prefix,generation],
        ReverseRefs{target,relation,generation}=>&[target,relation,generation],
        Interval{scope,generation,..}=>&[scope,generation],
        SchemaResource{uri,digest}=>&[uri,digest],
        Registry{uri,version,digest}=>&[uri,version,digest],
    };
    strings.iter().try_fold(std::mem::size_of::<crate::PredicateRead>(),|sum,s|sum.checked_add(s.len())).ok_or(ItemRefusal::Budget)
}
