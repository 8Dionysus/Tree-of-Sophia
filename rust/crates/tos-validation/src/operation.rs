//! Exact source-operation binding shared by native command handlers.
//!
//! The protected configuration is a separately selected byte input. It is not
//! an authored corpus member, and observing its digest grants no current right.
//! The private result binds mechanics to a complete *carrier* traversal, not
//! to accepted source membership, owner assessment or a commit attestation.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, JsonMode, RelativePath,
    SourceRevision, canonical_bytes_v1, parse_json,
};
use tos_source_store::{CorpusCutReader, SourceMembershipV1};

/// The source-derived, general-audit rows. A source operation may exclude a
/// row only through a separately reviewed owner profile, never by omission
/// from a caller-supplied module list.
pub(crate) const REQUIRED_GENERAL_ROWS: [&str; 14] = [
    "tos.val.source.member-shape.v1",
    "tos.val.source.profile-route.v1",
    "tos.val.source.identity-version.v1",
    "tos.val.source.bibliographic-links.v1",
    "tos.val.source.item-fixity.v1",
    "tos.val.source.rights-visibility.v1",
    "tos.val.source.provenance.v1",
    "tos.val.source.claim-closure.v1",
    "tos.val.source.bibliographic-topology.v1",
    "tos.val.source.artifact-representation.v1",
    "tos.val.source.text-layers.v1",
    "tos.val.source.transfer-research.v1",
    "tos.val.source.catalog-currentness.v1",
    "tos.val.source.retirement.v1",
];

use crate::PredicateRead;
use crate::biblio_rules::{SourceCutBiblioReport, inspect_bibliography_from_cut};
use crate::item_rules::{ItemFamilyReport, ItemLimits, ItemRefusal};
use crate::layer_family_cut::{SourceCutLayerFamilyReport, inspect_layers_from_cut};
use crate::record_biblio_cut::{
    BiblioRecordExecutor, SourceCutRecordReport, inspect_records_from_cut,
};
use crate::record_rules::RecordFamily;
use crate::retirement_rules::{
    RetirementFamilyReport, RetirementLimits, RetirementRefusal, inspect_retirements_from_cut,
};
use crate::rights_rules::{SourceRightsReport, inspect_rights_from_cut};
use crate::source_cut::{
    CutExecutionBinding, CutPayloadReader, CutSchemaReceipt, CutWorkerSchemaExecutor,
    inspect_items_from_cut,
};
use crate::source_shapes::{SourceShapeReport, inspect_source_shapes_from_cut};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationChange {
    pub path: RelativePath,
    pub before: Option<Digest256>,
    pub after: Option<Digest256>,
}

/// Raw source writes are selected through the candidate cut, not a second
/// caller buffer. The same digest may appear before and after a mode change.
#[derive(Debug, Clone)]
pub struct OperationProposal {
    pub handler_id: String,
    pub operation: String,
    pub base_revision: SourceRevision,
    pub candidate_revision: SourceRevision,
    pub request_raw: Vec<u8>,
    pub request_canonical_sha256: Digest256,
    pub configuration_path: RelativePath,
    pub configuration_raw: Vec<u8>,
    pub configuration_raw_sha256: Digest256,
    pub configuration_canonical_sha256: Digest256,
    pub changes: Vec<OperationChange>,
}

#[derive(Debug, Clone, Copy)]
pub struct OperationLimits {
    pub max_member_bytes: usize,
    pub max_total_bytes: u64,
    pub max_state_bytes: usize,
    pub max_reads: usize,
    pub max_changes: usize,
    pub deadline: Instant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationRefusal {
    Budget,
    Deadline,
    InvalidProposal(&'static str),
    Source(String),
    Unsupported(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtectedConfigurationBinding {
    pub path: RelativePath,
    pub raw_sha256: Digest256,
    pub canonical_sha256: Digest256,
}

/// No public constructor: only the exact cut reader can establish this binding.
#[derive(Debug, Clone)]
pub struct BoundOperation {
    handler_id: String,
    operation: String,
    base_revision: SourceRevision,
    candidate_revision: SourceRevision,
    request_canonical_sha256: Digest256,
    configuration: ProtectedConfigurationBinding,
    delta_sha256: Digest256,
    candidate_carrier: SourceMembershipV1,
    reads: Vec<PredicateRead>,
    raw_bytes_read: u64,
}

impl BoundOperation {
    pub fn handler_id(&self) -> &str {
        &self.handler_id
    }
    pub fn operation(&self) -> &str {
        &self.operation
    }
    pub fn base_revision(&self) -> SourceRevision {
        self.base_revision
    }
    pub fn candidate_revision(&self) -> SourceRevision {
        self.candidate_revision
    }
    pub fn request_canonical_sha256(&self) -> Digest256 {
        self.request_canonical_sha256
    }
    pub fn configuration(&self) -> &ProtectedConfigurationBinding {
        &self.configuration
    }
    pub fn delta_sha256(&self) -> Digest256 {
        self.delta_sha256
    }
    pub fn candidate_carrier(&self) -> SourceMembershipV1 {
        self.candidate_carrier
    }
    pub fn reads(&self) -> &[PredicateRead] {
        &self.reads
    }
    pub fn raw_bytes_read(&self) -> u64 {
        self.raw_bytes_read
    }
}

fn check(limits: OperationLimits, cancelled: &AtomicBool) -> Result<(), OperationRefusal> {
    if cancelled.load(Ordering::Relaxed) || Instant::now() >= limits.deadline {
        return Err(OperationRefusal::Deadline);
    }
    if limits.max_member_bytes == 0
        || limits.max_member_bytes == usize::MAX
        || limits.max_total_bytes == 0
        || limits.max_total_bytes == u64::MAX
        || limits.max_state_bytes == 0
        || limits.max_state_bytes == usize::MAX
        || limits.max_reads == 0
        || limits.max_reads == usize::MAX
        || limits.max_changes == 0
        || limits.max_changes == usize::MAX
    {
        return Err(OperationRefusal::Budget);
    }
    Ok(())
}

fn canonical_digest(raw: &[u8], limits: OperationLimits) -> Result<Digest256, OperationRefusal> {
    let json_limits = JsonLimits::new(limits.max_member_bytes, 64, 300_000, 4_300)
        .map_err(|_| OperationRefusal::Budget)?;
    let document = parse_json(raw, JsonMode::PublishedStrict, json_limits).map_err(|error| {
        OperationRefusal::Unsupported(format!("operation strict JSON: {error:?}"))
    })?;
    let canonical = canonical_bytes_v1(
        document.root(),
        CanonicalProfile::SourceCommandInputV1,
        json_limits,
    )
    .map_err(|error| {
        OperationRefusal::Unsupported(format!("operation canonical JSON: {error:?}"))
    })?;
    Ok(Digest256::of_bytes(&canonical))
}

fn feed(hash: &mut Digest256Hasher, value: &[u8]) {
    hash.update(&(value.len() as u64).to_be_bytes());
    hash.update(value);
}

/// Verify every declared addition/removal/replacement against exact selected
/// current/base metadata and bytes. Undeclared changes, duplicate paths, wrong
/// before hashes, mismatched candidate ancestry and incomplete EOF all refuse.
/// Retained profiles are not reinterpreted as current records during binding.
pub fn bind_operation_from_cut(
    cut: &CorpusCutReader,
    proposal: &OperationProposal,
    limits: OperationLimits,
    cancelled: &AtomicBool,
) -> Result<BoundOperation, OperationRefusal> {
    check(limits, cancelled)?;
    if proposal.handler_id.is_empty()
        || proposal.operation.is_empty()
        || proposal.handler_id.chars().any(char::is_control)
        || proposal.operation.chars().any(char::is_control)
    {
        return Err(OperationRefusal::InvalidProposal("operation identity"));
    }
    if cut.current().revision() != proposal.candidate_revision
        || cut.current().base_revision() != Some(proposal.base_revision)
    {
        return Err(OperationRefusal::InvalidProposal(
            "source cut or ancestry mismatch",
        ));
    }
    let base = cut
        .revisions()
        .find(|snapshot| snapshot.revision() == proposal.base_revision)
        .ok_or(OperationRefusal::InvalidProposal("base outside source cut"))?;
    if Digest256::of_bytes(&proposal.configuration_raw) != proposal.configuration_raw_sha256
        || canonical_digest(&proposal.configuration_raw, limits)?
            != proposal.configuration_canonical_sha256
        || canonical_digest(&proposal.request_raw, limits)? != proposal.request_canonical_sha256
    {
        return Err(OperationRefusal::InvalidProposal(
            "request or configuration digest mismatch",
        ));
    }
    let mut state = proposal
        .request_raw
        .len()
        .checked_add(proposal.configuration_raw.len())
        .and_then(|n| n.checked_add(proposal.handler_id.len()))
        .and_then(|n| n.checked_add(proposal.operation.len()))
        .and_then(|n| n.checked_add(proposal.configuration_path.as_str().len()))
        .filter(|n| *n <= limits.max_state_bytes)
        .ok_or(OperationRefusal::Budget)?;
    if proposal.changes.len() > limits.max_changes {
        return Err(OperationRefusal::Budget);
    }
    let mut declared = BTreeMap::new();
    for change in &proposal.changes {
        check(limits, cancelled)?;
        state = state
            .checked_add(change.path.as_str().len() + 128)
            .filter(|n| *n <= limits.max_state_bytes)
            .ok_or(OperationRefusal::Budget)?;
        if declared.insert(change.path.clone(), change).is_some()
            || (change.before.is_none() && change.after.is_none())
        {
            return Err(OperationRefusal::InvalidProposal(
                "duplicate or empty change",
            ));
        }
    }
    let mut observed = BTreeSet::new();
    let mut reads = Vec::new();
    let mut raw_bytes_read = 0u64;
    let mut delta = Digest256Hasher::new();
    delta.update(b"tos-source-operation-delta-v1\0");
    feed(&mut delta, proposal.base_revision.0.to_hex().as_bytes());
    feed(
        &mut delta,
        proposal.candidate_revision.0.to_hex().as_bytes(),
    );
    // A path-ordered merge protects additions and absent keys as well as removals.
    let mut before = base.members().peekable();
    let mut after = cut.current().members().peekable();
    loop {
        check(limits, cancelled)?;
        let path = match (before.peek(), after.peek()) {
            (None, None) => break,
            (Some(old), None) => old.path.clone(),
            (None, Some(new)) => new.path.clone(),
            (Some(old), Some(new)) => std::cmp::min(&old.path, &new.path).clone(),
        };
        let old = if before.peek().is_some_and(|member| member.path == path) {
            before.next()
        } else {
            None
        };
        let new = if after.peek().is_some_and(|member| member.path == path) {
            after.next()
        } else {
            None
        };
        if old == new {
            continue;
        }
        let change = declared
            .get(&path)
            .ok_or(OperationRefusal::InvalidProposal(
                "undeclared source change",
            ))?;
        if change.before != old.map(|member| member.sha256)
            || change.after != new.map(|member| member.sha256)
        {
            return Err(OperationRefusal::InvalidProposal(
                "source change digest mismatch",
            ));
        }
        observed.insert(path.clone());
        feed(&mut delta, path.as_str().as_bytes());
        for (snapshot, metadata) in [(base, old), (cut.current(), new)] {
            match metadata {
                Some(metadata) => {
                    delta.update(&[1]);
                    feed(&mut delta, metadata.sha256.to_hex().as_bytes());
                    delta.update(&metadata.size_bytes.to_be_bytes());
                    delta.update(&metadata.mode.to_be_bytes());
                    // The current bytes are read once by the complete stream below.
                    if snapshot.revision() == proposal.base_revision {
                        let member = cut
                            .read_member(
                                snapshot.revision(),
                                &path,
                                limits.max_member_bytes as u64,
                                limits.deadline,
                                cancelled,
                            )
                            .map_err(|error| OperationRefusal::Source(format!("{error:?}")))?;
                        raw_bytes_read = raw_bytes_read
                            .checked_add(member.raw.len() as u64)
                            .filter(|n| *n <= limits.max_total_bytes)
                            .ok_or(OperationRefusal::Budget)?;
                        push_read(
                            &mut reads,
                            &mut state,
                            PredicateRead::ExactBytes {
                                locator: format!(
                                    "{}:{}",
                                    snapshot.revision().0.to_hex(),
                                    path.as_str()
                                ),
                                digest: metadata.sha256.to_hex(),
                            },
                            limits,
                        )?;
                    }
                }
                None => {
                    delta.update(&[0]);
                    push_read(
                        &mut reads,
                        &mut state,
                        PredicateRead::AbsentKey {
                            namespace: format!("source-path:{}", snapshot.revision().0.to_hex()),
                            key: path.as_str().into(),
                        },
                        limits,
                    )?;
                }
            }
        }
    }
    if observed.len() != declared.len() {
        return Err(OperationRefusal::InvalidProposal(
            "declared unchanged source change",
        ));
    }
    let mut stream = cut
        .stream(proposal.candidate_revision)
        .map_err(|error| OperationRefusal::Source(format!("{error:?}")))?;
    while let Some(member) = stream
        .next_member(limits.deadline, cancelled)
        .map_err(|error| OperationRefusal::Source(format!("{error:?}")))?
    {
        check(limits, cancelled)?;
        raw_bytes_read = raw_bytes_read
            .checked_add(member.raw.len() as u64)
            .filter(|n| *n <= limits.max_total_bytes)
            .ok_or(OperationRefusal::Budget)?;
        push_read(
            &mut reads,
            &mut state,
            PredicateRead::ExactPath {
                path: member.path.as_str().into(),
                digest: Digest256::of_bytes(&member.raw).to_hex(),
            },
            limits,
        )?;
    }
    let candidate_carrier = stream.coverage().ok_or(OperationRefusal::InvalidProposal(
        "incomplete candidate EOF",
    ))?;
    check(limits, cancelled)?;
    Ok(BoundOperation {
        handler_id: proposal.handler_id.clone(),
        operation: proposal.operation.clone(),
        base_revision: proposal.base_revision,
        candidate_revision: proposal.candidate_revision,
        request_canonical_sha256: proposal.request_canonical_sha256,
        configuration: ProtectedConfigurationBinding {
            path: proposal.configuration_path.clone(),
            raw_sha256: proposal.configuration_raw_sha256,
            canonical_sha256: proposal.configuration_canonical_sha256,
        },
        delta_sha256: delta.finalize(),
        candidate_carrier,
        reads,
        raw_bytes_read,
    })
}

fn push_read(
    reads: &mut Vec<PredicateRead>,
    state: &mut usize,
    read: PredicateRead,
    limits: OperationLimits,
) -> Result<(), OperationRefusal> {
    if reads.len() >= limits.max_reads {
        return Err(OperationRefusal::Budget);
    }
    let bytes = match &read {
        PredicateRead::ExactPath { path, digest } => path.len() + digest.len(),
        PredicateRead::ExactBytes { locator, digest } => locator.len() + digest.len(),
        PredicateRead::AbsentKey { namespace, key } => namespace.len() + key.len(),
        _ => {
            return Err(OperationRefusal::Unsupported(
                "operation read accounting".into(),
            ));
        }
    };
    *state = state
        .checked_add(bytes + 64)
        .filter(|n| *n <= limits.max_state_bytes)
        .ok_or(OperationRefusal::Budget)?;
    reads.push(read);
    Ok(())
}

/// The scope states exactly which executable mechanics ran. It is weaker
/// than the command owner's full-source transition and admission requirements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationFamilyScope {
    ItemCompanions,
    RetirementNarrow,
    GeneralSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationIssue {
    pub path: String,
    pub code: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationFamilyState {
    MissingRules {
        rule_ids: Vec<String>,
    },
    Rejected {
        issues: Vec<OperationIssue>,
    },
    /// Complete only in the explicitly named local family scope. This grants
    /// no general source acceptance, permission, semantic review or publication.
    MechanicsComplete,
}

/// Private construction prevents a handler from asserting a successful rule
/// invocation. Accessors expose replay evidence, not an attestation constructor.
#[derive(Debug, Clone)]
pub struct OperationFamilyReport {
    binding: BoundOperation,
    scope: OperationFamilyScope,
    state: OperationFamilyState,
    worker: CutExecutionBinding,
    schema_receipts: Vec<CutSchemaReceipt>,
    executed_rules: Vec<String>,
    item_family: Option<ItemFamilyReport>,
}

impl OperationFamilyReport {
    pub fn binding(&self) -> &BoundOperation {
        &self.binding
    }
    pub fn scope(&self) -> OperationFamilyScope {
        self.scope
    }
    pub fn state(&self) -> &OperationFamilyState {
        &self.state
    }
    pub fn worker(&self) -> &CutExecutionBinding {
        &self.worker
    }
    pub fn schema_receipts(&self) -> &[CutSchemaReceipt] {
        &self.schema_receipts
    }
    pub fn executed_rules(&self) -> &[String] {
        &self.executed_rules
    }
    pub fn item_family(&self) -> Option<&ItemFamilyReport> {
        self.item_family.as_ref()
    }
    pub fn general_source_missing_rules(&self) -> Vec<String> {
        // Local family results cannot satisfy any entire general row merely
        // because a positive fixture or a worker instance was green.
        REQUIRED_GENERAL_ROWS
            .iter()
            .map(|rule| (*rule).into())
            .collect()
    }
}

fn worker_matches(
    cut: &CorpusCutReader,
    schemas: &CutWorkerSchemaExecutor,
) -> Result<(), OperationRefusal> {
    if schemas.source_revision() != cut.current().revision() {
        Err(OperationRefusal::InvalidProposal(
            "worker selected another source cut",
        ))
    } else {
        Ok(())
    }
}

fn item_error(error: ItemRefusal) -> OperationRefusal {
    match error {
        ItemRefusal::Budget => OperationRefusal::Budget,
        ItemRefusal::Deadline => OperationRefusal::Deadline,
        ItemRefusal::Source(reason) => OperationRefusal::Source(reason),
        ItemRefusal::Unsupported(reason) => OperationRefusal::Unsupported(reason),
    }
}

/// Actual proposal -> selected raw bytes -> bounded worker -> Item compound
/// report. The operation owner's remaining general rules and current rights
/// fences are exposed separately, never implicitly satisfied by this function.
pub fn inspect_item_operation(
    cut: &CorpusCutReader,
    proposal: &OperationProposal,
    operation_limits: OperationLimits,
    item_limits: ItemLimits,
    require_local_payloads: bool,
    cancelled: &AtomicBool,
    record_routes: &RecordFamily,
    schemas: &mut CutWorkerSchemaExecutor,
    payloads: &mut impl CutPayloadReader,
) -> Result<OperationFamilyReport, OperationRefusal> {
    worker_matches(cut, schemas)?;
    let binding = bind_operation_from_cut(cut, proposal, operation_limits, cancelled)?;
    let receipt_start = schemas.receipts().len();
    let result = inspect_items_from_cut(
        cut,
        item_limits,
        require_local_payloads,
        cancelled,
        record_routes,
        schemas,
        payloads,
    )
    .map_err(item_error)?;
    if result.carrier_membership != binding.candidate_carrier {
        return Err(OperationRefusal::InvalidProposal(
            "family carrier differs from proposal",
        ));
    }
    let issues = result
        .item_family
        .issues
        .iter()
        .map(|issue| OperationIssue {
            path: issue.path.clone(),
            code: issue.code.into(),
        })
        .collect::<Vec<_>>();
    check(operation_limits, cancelled)?;
    Ok(OperationFamilyReport {
        binding,
        scope: OperationFamilyScope::ItemCompanions,
        state: if issues.is_empty() {
            OperationFamilyState::MechanicsComplete
        } else {
            OperationFamilyState::Rejected { issues }
        },
        worker: schemas.execution_binding(),
        schema_receipts: schemas.receipts()[receipt_start..].to_vec(),
        executed_rules: vec!["tos.val.item-compound.current@1".into()],
        item_family: Some(result.item_family),
    })
}

/// The exact narrow retirement route preserves the owner's fallback to full
/// validation. A wider edit becomes MissingRules; no caller flag can opt out.
pub fn inspect_retirement_operation(
    cut: &CorpusCutReader,
    proposal: &OperationProposal,
    operation_limits: OperationLimits,
    retirement_limits: RetirementLimits,
    cancelled: &AtomicBool,
    schemas: &mut CutWorkerSchemaExecutor,
) -> Result<OperationFamilyReport, OperationRefusal> {
    worker_matches(cut, schemas)?;
    let binding = bind_operation_from_cut(cut, proposal, operation_limits, cancelled)?;
    let receipt_start = schemas.receipts().len();
    let result = inspect_retirements_from_cut(cut, retirement_limits, cancelled, schemas).map_err(
        |error| match error {
            RetirementRefusal::Budget => OperationRefusal::Budget,
            RetirementRefusal::Deadline => OperationRefusal::Deadline,
            RetirementRefusal::Source(reason) => OperationRefusal::Source(reason),
            RetirementRefusal::Unsupported(reason) => OperationRefusal::Unsupported(reason),
        },
    )?;
    let state = if result.membership_transition.is_some() {
        OperationFamilyState::MechanicsComplete
    } else {
        OperationFamilyState::MissingRules {
            rule_ids: REQUIRED_GENERAL_ROWS
                .iter()
                .map(|rule| (*rule).into())
                .collect(),
        }
    };
    check(operation_limits, cancelled)?;
    Ok(OperationFamilyReport {
        binding,
        scope: OperationFamilyScope::RetirementNarrow,
        state,
        worker: schemas.execution_binding(),
        schema_receipts: schemas.receipts()[receipt_start..].to_vec(),
        executed_rules: vec!["tos.val.source.retirement@1".into()],
        item_family: None,
    })
}

#[derive(Debug, Clone, Copy)]
pub struct GeneralOperationLimits {
    pub operation: OperationLimits,
    pub family: ItemLimits,
    /// Conservative logical reservation over simultaneously retained family
    /// outputs. This is not an allocator/RSS or host isolation certificate.
    pub max_composed_state_bytes: usize,
    pub max_composed_read_bytes: u64,
}

/// Every report is the result of the actual owner function, not supplied
/// externally as a caller claim. The operation state remains fail-closed while
/// named profile, current authority and generated artifact gaps are open.
pub struct GeneralOperationFamilyReport {
    operation: OperationFamilyReport,
    pub records: SourceCutRecordReport,
    pub bibliography: SourceCutBiblioReport,
    pub layers: SourceCutLayerFamilyReport,
    pub rights: SourceRightsReport,
    pub source_shapes: SourceShapeReport,
    pub retirement: RetirementFamilyReport,
}
impl GeneralOperationFamilyReport {
    pub fn operation(&self) -> &OperationFamilyReport {
        &self.operation
    }
}

/// Compose the independently owned families on the same source revision and
/// actual selected worker. No profile list or permission bool selects rules.
/// Source catalog parity is owned by the compiler's exact renderer; its
/// admission/currentness seam remains an explicit missing general rule here.
pub fn inspect_general_operation(
    cut: &CorpusCutReader,
    proposal: &OperationProposal,
    limits: GeneralOperationLimits,
    cancelled: &AtomicBool,
    record_routes: &RecordFamily,
    record_executor: &mut BiblioRecordExecutor,
    schemas: &mut CutWorkerSchemaExecutor,
    payloads: &mut impl CutPayloadReader,
    require_local_payloads: bool,
) -> Result<GeneralOperationFamilyReport, OperationRefusal> {
    // Seven families retain separate outputs. Reserve their full logical caps
    // before starting, rather than claiming each cap as one overall envelope.
    let state_reservation = limits
        .family
        .max_state_bytes
        .checked_mul(7)
        .and_then(|n| n.checked_add(limits.operation.max_state_bytes))
        .ok_or(OperationRefusal::Budget)?;
    let read_reservation = limits
        .family
        .max_total_bytes
        .checked_mul(7)
        .and_then(|n| n.checked_add(limits.operation.max_total_bytes))
        .ok_or(OperationRefusal::Budget)?;
    if limits.max_composed_state_bytes == 0
        || limits.max_composed_state_bytes == usize::MAX
        || limits.max_composed_read_bytes == 0
        || limits.max_composed_read_bytes == u64::MAX
        || state_reservation > limits.max_composed_state_bytes
        || read_reservation > limits.max_composed_read_bytes
        || limits.family.deadline > limits.operation.deadline
    {
        return Err(OperationRefusal::Budget);
    }
    worker_matches(cut, schemas)?;
    let worker = schemas.execution_binding();
    if record_executor.worker.sha256 != worker.worker_sha256
        || record_executor.profile != worker.schema_profile
    {
        return Err(OperationRefusal::InvalidProposal(
            "record executor selected another worker profile",
        ));
    }
    let registry = RelativePath::parse("ToS/doctrine/semantic-interchange/entity-types.v1.json")
        .map_err(|_| OperationRefusal::Unsupported("entity registry path".into()))?;
    let registry_digest = cut
        .current()
        .member(&registry)
        .ok_or_else(|| OperationRefusal::Unsupported("missing current entity registry".into()))?
        .sha256;
    if record_routes.registry_digest() != registry_digest.to_hex()
        && record_routes.registry_digest() != registry_digest.to_prefixed()
    {
        return Err(OperationRefusal::InvalidProposal(
            "Item record registry selected another cut",
        ));
    }
    let binding = bind_operation_from_cut(cut, proposal, limits.operation, cancelled)?;
    let receipt_start = schemas.receipts().len();
    let source_shapes = inspect_source_shapes_from_cut(cut, limits.family, cancelled, schemas)
        .map_err(item_error)?;
    let records = inspect_records_from_cut(cut, limits.family, cancelled, record_executor)
        .map_err(item_error)?;
    let bibliography =
        inspect_bibliography_from_cut(cut, &records, limits.family, cancelled, schemas)
            .map_err(item_error)?;
    let layers =
        inspect_layers_from_cut(cut, limits.family, cancelled, schemas).map_err(item_error)?;
    let rights =
        inspect_rights_from_cut(cut, limits.family, cancelled, schemas).map_err(item_error)?;
    let item = inspect_items_from_cut(
        cut,
        limits.family,
        require_local_payloads,
        cancelled,
        record_routes,
        schemas,
        payloads,
    )
    .map_err(item_error)?;
    let retirement = inspect_retirements_from_cut(
        cut,
        RetirementLimits {
            max_member_bytes: limits.family.max_member_bytes,
            max_total_bytes: limits.family.max_total_bytes,
            max_state_bytes: limits.family.max_state_bytes,
            max_entries: limits.family.max_issues,
            deadline: limits.family.deadline,
        },
        cancelled,
        schemas,
    )
    .map_err(|error| match error {
        RetirementRefusal::Budget => OperationRefusal::Budget,
        RetirementRefusal::Deadline => OperationRefusal::Deadline,
        RetirementRefusal::Source(reason) => OperationRefusal::Source(reason),
        RetirementRefusal::Unsupported(reason) => OperationRefusal::Unsupported(reason),
    })?;
    if source_shapes.carrier_membership != binding.candidate_carrier
        || records.current_membership != binding.candidate_carrier
        || bibliography.carrier_membership != binding.candidate_carrier
        || layers.carrier_membership != binding.candidate_carrier
        || rights.carrier_membership != binding.candidate_carrier
        || item.carrier_membership != binding.candidate_carrier
        || retirement.revision != binding.candidate_revision
    {
        return Err(OperationRefusal::InvalidProposal(
            "composed owner carrier mismatch",
        ));
    }
    let mut report_bytes = state_reservation;
    let mut issues = Vec::new();
    let mut add_issue = |path: &str, code: &str| -> Result<(), OperationRefusal> {
        report_bytes = report_bytes
            .checked_add(path.len() + code.len() + 96)
            .filter(|n| *n <= limits.max_composed_state_bytes)
            .ok_or(OperationRefusal::Budget)?;
        issues.push(OperationIssue {
            path: path.into(),
            code: code.into(),
        });
        Ok(())
    };
    for (path, code) in &source_shapes.issues {
        add_issue(path, code)?;
    }
    for observation in &records.observations {
        if let crate::record_rules::RecordObservation::Issue { path, code } = observation {
            add_issue(path, code)?;
        }
    }
    for issue in &bibliography.shadow.issues {
        add_issue(&issue.location, issue.code)?;
    }
    for issue in &layers.layer_family.issues {
        add_issue(&issue.path, issue.code)?;
    }
    for issue in &rights.issues {
        add_issue(&issue.path, issue.code)?;
    }
    for issue in &item.item_family.issues {
        add_issue(&issue.path, issue.code)?;
    }
    for receipt in &schemas.receipts()[receipt_start..] {
        report_bytes = report_bytes
            .checked_add(receipt.path.len() + receipt.contract.len() + 256)
            .filter(|n| *n <= limits.max_composed_state_bytes)
            .ok_or(OperationRefusal::Budget)?;
    }
    let state = if issues.is_empty() {
        OperationFamilyState::MissingRules {
            rule_ids: REQUIRED_GENERAL_ROWS
                .iter()
                .map(|rule| (*rule).into())
                .collect(),
        }
    } else {
        OperationFamilyState::Rejected { issues }
    };
    check(limits.operation, cancelled)?;
    Ok(GeneralOperationFamilyReport {
        operation: OperationFamilyReport {
            binding,
            scope: OperationFamilyScope::GeneralSource,
            state,
            worker: schemas.execution_binding(),
            schema_receipts: schemas.receipts()[receipt_start..].to_vec(),
            executed_rules: vec![
                "tos.val.record.registry-shape-identity.current@1".into(),
                "tos.val.source.instance-schema-ref.current@1".into(),
                "tos.val.claim-bibliography.current@1".into(),
                "tos.val.layer-family.current@1".into(),
                "tos.val.rights-record.current@1".into(),
                "tos.val.item-compound.current@1".into(),
                "tos.val.source.retirement@1".into(),
            ],
            item_family: Some(item.item_family),
        },
        records,
        bibliography,
        layers,
        rights,
        source_shapes,
        retirement,
    })
}
