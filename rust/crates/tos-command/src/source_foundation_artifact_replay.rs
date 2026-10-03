//! Readonly Artifact history and correction evidence selected by the exact
//! current source cut. This adapter composes the existing VAL history reader,
//! the existing CMD correction replay, and one bounded current-cut transport;
//! it does not create a writer context or infer publication authority.

use super::foundation_capture::FoundationCapturedCut;
use super::foundation_entry::{
    CapturedReadonlyRecordCost, CapturedReadonlyRecordFiles, CapturedReadonlyRecordLimits,
};
use crate::source_command::{SourceCommandError, SourceFile};
use crate::source_revisions::{
    self, ArtifactCorrectionReplayObservation, ReadonlyRecordFiles, RecordVersionReadInput,
};
use std::collections::BTreeMap;
use std::mem::size_of;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::{Digest256, RelativePath, SourceRevision};
use tos_source_store::{CorpusCutReader, SourceMembershipV1};
use tos_validation::PredicateRead;
use tos_validation::item_rules::{ItemLimits, ItemRefusal};
use tos_validation::native_compound::{
    CandidateNativeRecordHistoryReadObservation, NativeRecordHistoryReadObservation,
    selected_record_history_from_cut, selected_record_history_from_input,
};
use tos_validation::record_biblio_cut::SourceCutRecordReport;
use tos_validation::record_biblio_cut::{
    SourceCutInput, SourceCutInputCoverage, SourceCutInputWithIdentity,
};
use tos_validation::source_cut::{
    CandidateCutWorkerSchemaExecutor, CutSchemaExecutor, CutWorkerSchemaExecutor,
};
use tos_validation::source_foundation_discovery::{
    ArtifactCorrectionReplayEvidence, ArtifactCorrectionReplayMap,
    CandidateArtifactCorrectionReplayEvidence, CandidateArtifactCorrectionReplayMap,
    CandidateArtifactInvalidSchemaProofs, CurrentArtifactInvalidSchemaProofs, SourcePhysicalFacts,
};
use tos_validation::source_foundation_records::{
    NATIVE_ARTIFACT_RECORD_SCHEMA_URI, SourceFoundationRecordKernelOutcome,
    SourceFoundationRecordsCollection, SourceFoundationRecordsCursor,
    SourceFoundationRecordsPageBudget, SourceFoundationRecordsReport,
    SourceFoundationRecordsStoredFact, SourceFoundationRecordsStreamedReport,
};

const ARTIFACTS: &str = "ToS/source-witnesses/artifacts/";
const ARTIFACT_LEAF: &str = "/artifact-witness.json";
const NATIVE_ARTIFACT_SCHEMA: &str = NATIVE_ARTIFACT_RECORD_SCHEMA_URI;
const ARTIFACT_V2_RECORD: &str = "tos_artifact_source_witness_v2";
const NATIVE_ARTIFACT_COMPANIONS: [&str; 4] = [
    "source-create-request.json",
    "source-create-receipt.json",
    "source-create-environment.json",
    "source-create-provenance.jsonl",
];
const TEMP_JSON_STATE_PER_BYTE: usize = 128;
const TEMP_JSON_FIXED_STATE: usize = 65_536;
const RETAINED_MAP_NODE_UPPER: usize = 256;
const REPLAY_OBSERVATION_UPPER: usize = 512;
const REPLAY_TRANSACTION_UPPER: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ArtifactReplaySkipKind {
    LegacyNoCreationCompanions,
    PartialCreationCompanions,
    CurrentSchemaDrift,
    CurrentJsonInvalid,
    CurrentRecordSchemaInvalid,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ArtifactReplaySkip {
    pub path: String,
    pub kind: ArtifactReplaySkipKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ArtifactReplayFailureClass {
    Budget,
    Deadline,
    Source,
    Incomplete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ArtifactReplayFailureStage {
    CutBinding,
    CompanionFacts,
    CurrentRecord,
    NativeHistory,
    ReadonlyCollection,
    SourceFileMerge,
    CorrectionReplay,
}

/// Measured bytes/state plus explicit failure reservations. A failed owner
/// kernel may not return its partial counter, so the entire remaining stage
/// slice is retained as the conservative failure upper bound.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ArtifactReplayCost {
    pub skip_state_bytes: usize,
    /// Peak storage upper bound for the temporary borrowed path-to-record
    /// index used while selecting current Artifact records.
    pub candidate_record_index_state_bytes: usize,
    /// Precharged row-scan and ordered-index comparison work telemetry, not a
    /// hard CPU quota.
    pub candidate_record_index_work_upper_bound: usize,
    pub candidate_source_read_bytes: u64,
    pub native_history_source_read_bytes: u64,
    pub readonly: CapturedReadonlyRecordCost,
    pub native_history_returned_state_bytes: usize,
    pub native_history_map_state_bytes: usize,
    pub replay_publication_state_bytes: usize,
    pub replay_returned_state_bytes: usize,
    pub replay_map_state_bytes: usize,
    pub replay_input_copy_state_bytes: usize,
    pub schema_proof_retained_state_bytes: usize,
    pub schema_proof_prepare_work_upper_bound: usize,
    pub schema_proof_validation_work_upper_bound: usize,
    pub schema_proof_hash_input_bytes_upper_bound: usize,
    pub peak_temporary_state_bytes: usize,
    pub failure_reserved_source_read_bytes: u64,
    pub failure_reserved_state_bytes: usize,
}

impl ArtifactReplayCost {
    pub(crate) fn measured_source_read_bytes(self) -> Option<u64> {
        self.candidate_source_read_bytes
            .checked_add(self.native_history_source_read_bytes)?
            .checked_add(self.readonly.read_bytes)
    }

    pub(crate) fn retained_state_upper_bound_bytes(self) -> Option<usize> {
        // The readonly report includes `state_already_reserved`, which is set
        // to the direct history/proof/replay reservation before transport
        // allocation. Its retained upper bound therefore overlaps `direct`;
        // max preserves that overlap while adding the adapter's subset view.
        let direct = self
            .native_history_returned_state_bytes
            .checked_add(self.native_history_map_state_bytes)?
            .checked_add(self.replay_publication_state_bytes)?
            .checked_add(self.replay_returned_state_bytes)?
            .checked_add(self.replay_map_state_bytes)?
            .checked_add(self.replay_input_copy_state_bytes)?
            .checked_add(self.schema_proof_retained_state_bytes)?
            .checked_add(self.skip_state_bytes)?;
        Some(direct.max(self.readonly.retained_state_upper_bound_bytes))
    }
}

#[derive(Debug)]
pub(crate) struct ArtifactReplayFailure {
    pub class: ArtifactReplayFailureClass,
    pub stage: ArtifactReplayFailureStage,
    pub path: Option<String>,
    pub cost: ArtifactReplayCost,
}

/// Successful-only history and correction observations for exact current
/// Artifact paths. Constructors are private to this adapter; callers borrow
/// these observations into the existing Discovery join.
pub(crate) struct ArtifactReplayEvidence<'cut> {
    histories: BTreeMap<String, NativeRecordHistoryReadObservation>,
    replays: BTreeMap<String, ArtifactCorrectionReplayObservation>,
    invalid_schema_proofs: CurrentArtifactInvalidSchemaProofs<'cut>,
    skips: Vec<ArtifactReplaySkip>,
    cost: ArtifactReplayCost,
}

impl<'cut> ArtifactReplayEvidence<'cut> {
    pub(crate) fn histories(&self) -> &BTreeMap<String, NativeRecordHistoryReadObservation> {
        &self.histories
    }

    /// Build the exact borrowed-map shape consumed by VAL. The cloned keys and
    /// BTree nodes are included in `cost.replay_map_state_bytes`.
    pub(crate) fn artifact_replays(&self) -> ArtifactCorrectionReplayMap<'_> {
        self.replays
            .iter()
            .map(|(path, observation)| {
                (
                    path.clone(),
                    observation as &dyn ArtifactCorrectionReplayEvidence,
                )
            })
            .collect()
    }

    pub(crate) fn skips(&self) -> &[ArtifactReplaySkip] {
        &self.skips
    }

    pub(crate) fn invalid_schema_proofs(&self) -> &CurrentArtifactInvalidSchemaProofs<'cut> {
        &self.invalid_schema_proofs
    }

    pub(crate) fn cost(&self) -> ArtifactReplayCost {
        self.cost
    }
}

/// Candidate-input history and correction evidence borrowed into the same
/// Discovery join. The proof remains tied to this exact streamed Records
/// report; its source identity is never translated into a revision.
pub(crate) struct CandidateArtifactReplayEvidence<'report, 'store, I: Copy + Eq> {
    histories: BTreeMap<String, CandidateNativeRecordHistoryReadObservation<I>>,
    replays: BTreeMap<String, source_revisions::CandidateArtifactCorrectionReplayObservation<I>>,
    invalid_schema_proofs: CandidateArtifactInvalidSchemaProofs<'report, 'store, I>,
    skips: Vec<ArtifactReplaySkip>,
    cost: ArtifactReplayCost,
}

impl<I: Copy + Eq> CandidateArtifactReplayEvidence<'_, '_, I> {
    pub(crate) fn histories(
        &self,
    ) -> &BTreeMap<String, CandidateNativeRecordHistoryReadObservation<I>> {
        &self.histories
    }

    pub(crate) fn artifact_replays(&self) -> CandidateArtifactCorrectionReplayMap<'_, I> {
        self.replays
            .iter()
            .map(|(path, observation)| {
                (
                    path.clone(),
                    observation as &dyn CandidateArtifactCorrectionReplayEvidence<I>,
                )
            })
            .collect()
    }

    pub(crate) fn skips(&self) -> &[ArtifactReplaySkip] {
        &self.skips
    }

    pub(crate) fn invalid_schema_proofs(&self) -> &CandidateArtifactInvalidSchemaProofs<'_, '_, I> {
        &self.invalid_schema_proofs
    }

    pub(crate) fn cost(&self) -> ArtifactReplayCost {
        self.cost
    }
}

#[derive(Clone, Copy)]
struct ReplayReservation {
    publication_state: usize,
    returned_state: usize,
    input_copy_state: usize,
    map_state: usize,
    worker_peak_state: usize,
}

fn failure(
    class: ArtifactReplayFailureClass,
    stage: ArtifactReplayFailureStage,
    path: Option<&str>,
    mut cost: ArtifactReplayCost,
    remaining_source: u64,
    remaining_state: usize,
) -> ArtifactReplayFailure {
    cost.failure_reserved_source_read_bytes = remaining_source;
    cost.failure_reserved_state_bytes = remaining_state;
    ArtifactReplayFailure {
        class,
        stage,
        path: path.map(str::to_owned),
        cost,
    }
}

fn refusal_class(refusal: &ItemRefusal) -> ArtifactReplayFailureClass {
    match refusal {
        ItemRefusal::Budget | ItemRefusal::BudgetCheck { .. } => ArtifactReplayFailureClass::Budget,
        ItemRefusal::Deadline => ArtifactReplayFailureClass::Deadline,
        ItemRefusal::Source(_) => ArtifactReplayFailureClass::Source,
        ItemRefusal::Unsupported(_) => ArtifactReplayFailureClass::Incomplete,
    }
}

fn command_class(error: &SourceCommandError) -> ArtifactReplayFailureClass {
    match error {
        SourceCommandError::Denied(_) | SourceCommandError::Conflict(_) => {
            ArtifactReplayFailureClass::Source
        }
        SourceCommandError::SchemaExecution { reason, .. } => refusal_class(reason),
        SourceCommandError::Invalid(_)
        | SourceCommandError::Unsupported(_)
        | SourceCommandError::MissingProductionAdmission => ArtifactReplayFailureClass::Incomplete,
    }
}

fn active(deadline: Instant, cancelled: &AtomicBool) -> Result<(), ArtifactReplayFailureClass> {
    if cancelled.load(Ordering::Relaxed) {
        Err(ArtifactReplayFailureClass::Source)
    } else if Instant::now() >= deadline {
        Err(ArtifactReplayFailureClass::Deadline)
    } else {
        Ok(())
    }
}

fn validate_membership<'a>(
    captured: &FoundationCapturedCut,
    current_paths: &[String],
    records: &'a SourceFoundationRecordsReport,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<
    (
        SourceRevision,
        SourceMembershipV1,
        &'a SourceCutRecordReport,
    ),
    ArtifactReplayFailureClass,
> {
    active(deadline, cancelled)?;
    let revision = captured.revision();
    let membership = captured.membership();
    if records.source_revision != revision || records.source_membership != membership {
        return Err(ArtifactReplayFailureClass::Source);
    }
    let record_report = match &records.records {
        SourceFoundationRecordKernelOutcome::Complete(report) => report,
        SourceFoundationRecordKernelOutcome::Refused { refusal, .. } => {
            return Err(refusal_class(refusal));
        }
    };
    if record_report.source_revision != revision || record_report.current_membership != membership {
        return Err(ArtifactReplayFailureClass::Source);
    }
    let mut members = captured.cut().current().members();
    for (index, path) in current_paths.iter().enumerate() {
        if index % 128 == 0 {
            active(deadline, cancelled)?;
        }
        if members
            .next()
            .is_none_or(|member| member.path.as_str() != path)
        {
            return Err(ArtifactReplayFailureClass::Source);
        }
    }
    if members.next().is_some() {
        return Err(ArtifactReplayFailureClass::Source);
    }
    Ok((revision, membership, record_report))
}

fn is_artifact_record_path(path: &str) -> bool {
    path.starts_with(ARTIFACTS) && path.ends_with(ARTIFACT_LEAF)
}

fn companion_state(physical: &SourcePhysicalFacts, artifact_path: &str) -> Option<[bool; 4]> {
    let parent = artifact_path.rsplit_once('/').map(|(parent, _)| parent)?;
    let mut values = [false; 4];
    for (index, name) in NATIVE_ARTIFACT_COMPANIONS.iter().enumerate() {
        let path = format!("{parent}/{name}");
        values[index] = physical.authored_paths.get(&path)?.exists;
    }
    Some(values)
}

fn candidate_companions_present<I: Eq>(
    input: &dyn SourceCutInputWithIdentity<I>,
    artifact_path: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<[bool; 4], ItemRefusal> {
    let parent = artifact_path
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .ok_or_else(|| ItemRefusal::Source("candidate Artifact path has no parent".into()))?;
    let mut present = [false; 4];
    for (index, name) in NATIVE_ARTIFACT_COMPANIONS.iter().enumerate() {
        if cancelled.load(Ordering::Relaxed) {
            return Err(ItemRefusal::Source(
                "candidate Artifact companion scan cancelled".into(),
            ));
        }
        if Instant::now() >= deadline {
            return Err(ItemRefusal::Deadline);
        }
        let companion_path = format!("{parent}/{name}");
        present[index] = input
            .path_presence(&companion_path, deadline, cancelled)?
            .is_some();
    }
    Ok(present)
}

fn candidate_artifact_member<I: Eq>(
    input: &dyn SourceCutInputWithIdentity<I>,
    path: &str,
    expected_size_bytes: u64,
    max_member_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(Option<serde_json::Value>, Digest256, u64), ItemRefusal> {
    let mut observed = None;
    input.with_current_member(
        path,
        max_member_bytes,
        deadline,
        cancelled,
        &mut |meta, raw| {
            if meta.path != path
                || meta.size_bytes != expected_size_bytes
                || raw.len() as u64 != expected_size_bytes
            {
                return Err(ItemRefusal::Source(
                    "candidate Artifact member metadata changed during read".into(),
                ));
            }
            let value = serde_json::from_slice::<serde_json::Value>(raw).ok();
            observed = Some((value, Digest256::of_bytes(raw), meta.size_bytes));
            Ok(())
        },
    )?;
    observed.ok_or_else(|| {
        ItemRefusal::Source("candidate Artifact member reader omitted its callback".into())
    })
}

fn refusal_failure(
    refusal: &ItemRefusal,
    stage: ArtifactReplayFailureStage,
    path: Option<&str>,
    cost: ArtifactReplayCost,
    remaining_source: u64,
    remaining_state: usize,
) -> ArtifactReplayFailure {
    failure(
        refusal_class(refusal),
        stage,
        path,
        cost,
        remaining_source,
        remaining_state,
    )
}

fn retain_candidate_skip(
    skips: &mut Vec<ArtifactReplaySkip>,
    cost: &mut ArtifactReplayCost,
    path: &str,
    kind: ArtifactReplaySkipKind,
    retained_state: &mut usize,
    live_temporary_state: usize,
    max_state: usize,
) -> Result<(), ItemRefusal> {
    let skip_state = size_of::<ArtifactReplaySkip>()
        .checked_add(map_state(path).ok_or(ItemRefusal::Budget)?)
        .ok_or(ItemRefusal::Budget)?;
    if retained_state
        .checked_add(live_temporary_state)
        .and_then(|state| state.checked_add(skip_state))
        .is_none_or(|state| state > max_state)
    {
        return Err(ItemRefusal::Budget);
    }
    retain_skip(skips, cost, path, kind, retained_state, max_state)
        .map_err(|_| ItemRefusal::Budget)?;
    cost.peak_temporary_state_bytes = cost.peak_temporary_state_bytes.max(
        retained_state
            .checked_add(live_temporary_state)
            .ok_or(ItemRefusal::Budget)?,
    );
    Ok(())
}

fn candidate_artifact_member_temporary_state(size_bytes: u64) -> Result<usize, ItemRefusal> {
    let size = usize::try_from(size_bytes).map_err(|_| ItemRefusal::Budget)?;
    size.checked_add(TEMP_JSON_FIXED_STATE)
        .and_then(|state| {
            size.checked_mul(TEMP_JSON_STATE_PER_BYTE)
                .and_then(|json| state.checked_add(json))
        })
        .and_then(|state| state.checked_add(8_192))
        .ok_or(ItemRefusal::Budget)
}

#[allow(clippy::too_many_arguments)]
fn process_candidate_artifact_path<I: Copy + Eq>(
    input: &dyn SourceCutInputWithIdentity<I>,
    input_identity: I,
    membership: SourceMembershipV1,
    invalid_schema_proofs: &CandidateArtifactInvalidSchemaProofs<'_, '_, I>,
    history_limits: ItemLimits,
    path: &str,
    expected_size_bytes: u64,
    indexed_record: Option<
        tos_validation::source_foundation_records::SourceFoundationArtifactRecordPathSummary,
    >,
    live_temporary_state: usize,
    retained_state: &mut usize,
    source_bytes_used: &mut u64,
    histories: &mut BTreeMap<String, CandidateNativeRecordHistoryReadObservation<I>>,
    skips: &mut Vec<ArtifactReplaySkip>,
    cost: &mut ArtifactReplayCost,
    cancelled: &AtomicBool,
) -> Result<(), ArtifactReplayFailure> {
    let deadline = history_limits.deadline;
    let max_source_bytes = history_limits.max_total_bytes;
    let max_state_bytes = history_limits.max_state_bytes;
    let remaining_source = max_source_bytes.saturating_sub(*source_bytes_used);
    let remaining_state = max_state_bytes.saturating_sub(*retained_state);
    active(deadline, cancelled).map_err(|class| {
        failure(
            class,
            ArtifactReplayFailureStage::CompanionFacts,
            Some(path),
            *cost,
            remaining_source,
            remaining_state,
        )
    })?;

    let companions =
        candidate_companions_present(input, path, deadline, cancelled).map_err(|refusal| {
            refusal_failure(
                &refusal,
                ArtifactReplayFailureStage::CompanionFacts,
                Some(path),
                *cost,
                remaining_source,
                remaining_state,
            )
        })?;
    let present_count = companions.iter().filter(|present| **present).count();
    match present_count {
        0 => {
            retain_candidate_skip(
                skips,
                cost,
                path,
                ArtifactReplaySkipKind::LegacyNoCreationCompanions,
                retained_state,
                live_temporary_state,
                max_state_bytes,
            )
            .map_err(|refusal| {
                refusal_failure(
                    &refusal,
                    ArtifactReplayFailureStage::CompanionFacts,
                    Some(path),
                    *cost,
                    remaining_source,
                    remaining_state,
                )
            })?;
            return Ok(());
        }
        1..4 => {
            retain_candidate_skip(
                skips,
                cost,
                path,
                ArtifactReplaySkipKind::PartialCreationCompanions,
                retained_state,
                live_temporary_state,
                max_state_bytes,
            )
            .map_err(|refusal| {
                refusal_failure(
                    &refusal,
                    ArtifactReplayFailureStage::CompanionFacts,
                    Some(path),
                    *cost,
                    remaining_source,
                    remaining_state,
                )
            })?;
            return Ok(());
        }
        4 => {}
        _ => {
            return Err(failure(
                ArtifactReplayFailureClass::Source,
                ArtifactReplayFailureStage::CompanionFacts,
                Some(path),
                *cost,
                remaining_source,
                remaining_state,
            ));
        }
    }

    let Some(schema_matches) = indexed_record.map(|record| record.schema_matches) else {
        // No current Records row means the raw current member is the only
        // source for the legacy schema/invalid-JSON branch.
        if expected_size_bytes > history_limits.max_member_bytes as u64 {
            return Err(failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::CurrentRecord,
                Some(path),
                *cost,
                remaining_source,
                remaining_state,
            ));
        }
        if expected_size_bytes > remaining_source {
            return Err(failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::CurrentRecord,
                Some(path),
                *cost,
                remaining_source,
                remaining_state,
            ));
        }
        let raw_temporary = candidate_artifact_member_temporary_state(expected_size_bytes)
            .map_err(|refusal| {
                refusal_failure(
                    &refusal,
                    ArtifactReplayFailureStage::CurrentRecord,
                    Some(path),
                    *cost,
                    remaining_source,
                    remaining_state,
                )
            })?;
        if retained_state
            .checked_add(live_temporary_state)
            .and_then(|state| state.checked_add(raw_temporary))
            .is_none_or(|state| state > max_state_bytes)
        {
            return Err(failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::CurrentRecord,
                Some(path),
                *cost,
                remaining_source,
                remaining_state,
            ));
        }
        cost.peak_temporary_state_bytes = cost.peak_temporary_state_bytes.max(
            retained_state
                .checked_add(live_temporary_state)
                .and_then(|state| state.checked_add(raw_temporary))
                .ok_or_else(|| {
                    failure(
                        ArtifactReplayFailureClass::Budget,
                        ArtifactReplayFailureStage::CurrentRecord,
                        Some(path),
                        *cost,
                        remaining_source,
                        remaining_state,
                    )
                })?,
        );
        let (parsed, member_sha256, member_size_bytes) = candidate_artifact_member(
            input,
            path,
            expected_size_bytes,
            history_limits.max_member_bytes,
            deadline,
            cancelled,
        )
        .map_err(|refusal| {
            refusal_failure(
                &refusal,
                ArtifactReplayFailureStage::CurrentRecord,
                Some(path),
                *cost,
                remaining_source,
                remaining_state,
            )
        })?;
        *source_bytes_used = source_bytes_used
            .checked_add(member_size_bytes)
            .filter(|used| *used <= max_source_bytes)
            .ok_or_else(|| {
                failure(
                    ArtifactReplayFailureClass::Budget,
                    ArtifactReplayFailureStage::CurrentRecord,
                    Some(path),
                    *cost,
                    remaining_source,
                    remaining_state,
                )
            })?;
        cost.candidate_source_read_bytes = cost
            .candidate_source_read_bytes
            .checked_add(member_size_bytes)
            .ok_or_else(|| {
                failure(
                    ArtifactReplayFailureClass::Budget,
                    ArtifactReplayFailureStage::CurrentRecord,
                    Some(path),
                    *cost,
                    remaining_source,
                    remaining_state,
                )
            })?;
        let Some(parsed) = parsed else {
            retain_candidate_skip(
                skips,
                cost,
                path,
                ArtifactReplaySkipKind::CurrentJsonInvalid,
                retained_state,
                live_temporary_state,
                max_state_bytes,
            )
            .map_err(|refusal| {
                refusal_failure(
                    &refusal,
                    ArtifactReplayFailureStage::CurrentRecord,
                    Some(path),
                    *cost,
                    max_source_bytes.saturating_sub(*source_bytes_used),
                    max_state_bytes.saturating_sub(*retained_state),
                )
            })?;
            return Ok(());
        };
        if parsed.get("$schema").and_then(serde_json::Value::as_str) != Some(NATIVE_ARTIFACT_SCHEMA)
        {
            retain_candidate_skip(
                skips,
                cost,
                path,
                ArtifactReplaySkipKind::CurrentSchemaDrift,
                retained_state,
                live_temporary_state,
                max_state_bytes,
            )
            .map_err(|refusal| {
                refusal_failure(
                    &refusal,
                    ArtifactReplayFailureStage::CurrentRecord,
                    Some(path),
                    *cost,
                    max_source_bytes.saturating_sub(*source_bytes_used),
                    max_state_bytes.saturating_sub(*retained_state),
                )
            })?;
            return Ok(());
        }
        drop(parsed);
        if invalid_schema_proofs.proves_invalid(
            path,
            &input_identity,
            membership,
            member_sha256,
            member_size_bytes,
        ) {
            retain_candidate_skip(
                skips,
                cost,
                path,
                ArtifactReplaySkipKind::CurrentRecordSchemaInvalid,
                retained_state,
                live_temporary_state,
                max_state_bytes,
            )
            .map_err(|refusal| {
                refusal_failure(
                    &refusal,
                    ArtifactReplayFailureStage::CurrentRecord,
                    Some(path),
                    *cost,
                    max_source_bytes.saturating_sub(*source_bytes_used),
                    max_state_bytes.saturating_sub(*retained_state),
                )
            })?;
            return Ok(());
        }
        return read_candidate_artifact_history(
            input,
            input_identity,
            membership,
            history_limits,
            path,
            live_temporary_state,
            retained_state,
            source_bytes_used,
            histories,
            cost,
            cancelled,
        );
    };

    if indexed_record.is_some_and(|record| record.record_count != 1) {
        return Err(failure(
            ArtifactReplayFailureClass::Source,
            ArtifactReplayFailureStage::CurrentRecord,
            Some(path),
            *cost,
            remaining_source,
            remaining_state,
        ));
    }
    if !schema_matches {
        retain_candidate_skip(
            skips,
            cost,
            path,
            ArtifactReplaySkipKind::CurrentSchemaDrift,
            retained_state,
            live_temporary_state,
            max_state_bytes,
        )
        .map_err(|refusal| {
            refusal_failure(
                &refusal,
                ArtifactReplayFailureStage::CurrentRecord,
                Some(path),
                *cost,
                remaining_source,
                remaining_state,
            )
        })?;
        return Ok(());
    }
    if expected_size_bytes > history_limits.max_member_bytes as u64
        || expected_size_bytes > remaining_source
    {
        return Err(failure(
            ArtifactReplayFailureClass::Budget,
            ArtifactReplayFailureStage::CurrentRecord,
            Some(path),
            *cost,
            remaining_source,
            remaining_state,
        ));
    }
    let raw_temporary =
        candidate_artifact_member_temporary_state(expected_size_bytes).map_err(|refusal| {
            refusal_failure(
                &refusal,
                ArtifactReplayFailureStage::CurrentRecord,
                Some(path),
                *cost,
                remaining_source,
                remaining_state,
            )
        })?;
    if retained_state
        .checked_add(live_temporary_state)
        .and_then(|state| state.checked_add(raw_temporary))
        .is_none_or(|state| state > max_state_bytes)
    {
        return Err(failure(
            ArtifactReplayFailureClass::Budget,
            ArtifactReplayFailureStage::CurrentRecord,
            Some(path),
            *cost,
            remaining_source,
            remaining_state,
        ));
    }
    cost.peak_temporary_state_bytes = cost.peak_temporary_state_bytes.max(
        retained_state
            .checked_add(live_temporary_state)
            .and_then(|state| state.checked_add(raw_temporary))
            .ok_or_else(|| {
                failure(
                    ArtifactReplayFailureClass::Budget,
                    ArtifactReplayFailureStage::CurrentRecord,
                    Some(path),
                    *cost,
                    remaining_source,
                    remaining_state,
                )
            })?,
    );
    let (parsed, member_sha256, member_size_bytes) = candidate_artifact_member(
        input,
        path,
        expected_size_bytes,
        history_limits.max_member_bytes,
        deadline,
        cancelled,
    )
    .map_err(|refusal| {
        refusal_failure(
            &refusal,
            ArtifactReplayFailureStage::CurrentRecord,
            Some(path),
            *cost,
            remaining_source,
            remaining_state,
        )
    })?;
    *source_bytes_used = source_bytes_used
        .checked_add(member_size_bytes)
        .filter(|used| *used <= max_source_bytes)
        .ok_or_else(|| {
            failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::CurrentRecord,
                Some(path),
                *cost,
                remaining_source,
                remaining_state,
            )
        })?;
    cost.candidate_source_read_bytes = cost
        .candidate_source_read_bytes
        .checked_add(member_size_bytes)
        .ok_or_else(|| {
            failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::CurrentRecord,
                Some(path),
                *cost,
                remaining_source,
                remaining_state,
            )
        })?;
    if !parsed.as_ref().is_some_and(|value| {
        value.get("$schema").and_then(serde_json::Value::as_str) == Some(NATIVE_ARTIFACT_SCHEMA)
    }) {
        return Err(failure(
            ArtifactReplayFailureClass::Source,
            ArtifactReplayFailureStage::CurrentRecord,
            Some(path),
            *cost,
            max_source_bytes.saturating_sub(*source_bytes_used),
            max_state_bytes.saturating_sub(*retained_state),
        ));
    }
    drop(parsed);
    if invalid_schema_proofs.proves_invalid(
        path,
        &input_identity,
        membership,
        member_sha256,
        member_size_bytes,
    ) {
        retain_candidate_skip(
            skips,
            cost,
            path,
            ArtifactReplaySkipKind::CurrentRecordSchemaInvalid,
            retained_state,
            live_temporary_state,
            max_state_bytes,
        )
        .map_err(|refusal| {
            refusal_failure(
                &refusal,
                ArtifactReplayFailureStage::CurrentRecord,
                Some(path),
                *cost,
                max_source_bytes.saturating_sub(*source_bytes_used),
                max_state_bytes.saturating_sub(*retained_state),
            )
        })?;
        return Ok(());
    }

    read_candidate_artifact_history(
        input,
        input_identity,
        membership,
        history_limits,
        path,
        live_temporary_state,
        retained_state,
        source_bytes_used,
        histories,
        cost,
        cancelled,
    )
}

#[allow(clippy::too_many_arguments)]
fn read_candidate_artifact_history<I: Copy + Eq>(
    input: &dyn SourceCutInputWithIdentity<I>,
    input_identity: I,
    membership: SourceMembershipV1,
    history_limits: ItemLimits,
    path: &str,
    live_temporary_state: usize,
    retained_state: &mut usize,
    source_bytes_used: &mut u64,
    histories: &mut BTreeMap<String, CandidateNativeRecordHistoryReadObservation<I>>,
    cost: &mut ArtifactReplayCost,
    cancelled: &AtomicBool,
) -> Result<(), ArtifactReplayFailure> {
    let max_source_bytes = history_limits.max_total_bytes;
    let max_state_bytes = history_limits.max_state_bytes;
    let remaining_source = max_source_bytes.saturating_sub(*source_bytes_used);
    let map_entry_state = map_state(path).ok_or_else(|| {
        failure(
            ArtifactReplayFailureClass::Budget,
            ArtifactReplayFailureStage::NativeHistory,
            Some(path),
            *cost,
            remaining_source,
            max_state_bytes.saturating_sub(*retained_state),
        )
    })?;
    let before_read = retained_state
        .checked_add(live_temporary_state)
        .and_then(|state| state.checked_add(map_entry_state))
        .ok_or_else(|| {
            failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::NativeHistory,
                Some(path),
                *cost,
                remaining_source,
                max_state_bytes.saturating_sub(*retained_state),
            )
        })?;
    let reader_state = max_state_bytes.checked_sub(before_read).ok_or_else(|| {
        failure(
            ArtifactReplayFailureClass::Budget,
            ArtifactReplayFailureStage::NativeHistory,
            Some(path),
            *cost,
            remaining_source,
            max_state_bytes.saturating_sub(*retained_state),
        )
    })?;
    let native_limits = ItemLimits {
        max_member_bytes: history_limits.max_member_bytes,
        max_total_bytes: remaining_source,
        max_state_bytes: reader_state,
        max_issues: history_limits.max_issues,
        deadline: history_limits.deadline,
    };
    let observation =
        selected_record_history_from_input(input, membership, path, native_limits, cancelled)
            .map_err(|refusal| {
                refusal_failure(
                    &refusal,
                    ArtifactReplayFailureStage::NativeHistory,
                    Some(path),
                    *cost,
                    remaining_source,
                    max_state_bytes.saturating_sub(*retained_state),
                )
            })?;
    if observation.input_identity() != &input_identity
        || observation.current_membership() != membership
        || observation.record_path() != path
    {
        return Err(failure(
            ArtifactReplayFailureClass::Source,
            ArtifactReplayFailureStage::NativeHistory,
            Some(path),
            *cost,
            remaining_source.saturating_sub(observation.bytes_read()),
            max_state_bytes.saturating_sub(*retained_state),
        ));
    }
    let next_source = source_bytes_used
        .checked_add(observation.bytes_read())
        .filter(|used| *used <= max_source_bytes)
        .ok_or_else(|| {
            failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::NativeHistory,
                Some(path),
                *cost,
                remaining_source,
                max_state_bytes.saturating_sub(*retained_state),
            )
        })?;
    let returned_state = observation.returned_state_bytes();
    let added_state = returned_state.checked_add(map_entry_state).ok_or_else(|| {
        failure(
            ArtifactReplayFailureClass::Budget,
            ArtifactReplayFailureStage::NativeHistory,
            Some(path),
            *cost,
            remaining_source.saturating_sub(observation.bytes_read()),
            max_state_bytes.saturating_sub(*retained_state),
        )
    })?;
    let next_retained = retained_state.checked_add(added_state).ok_or_else(|| {
        failure(
            ArtifactReplayFailureClass::Budget,
            ArtifactReplayFailureStage::NativeHistory,
            Some(path),
            *cost,
            remaining_source.saturating_sub(observation.bytes_read()),
            max_state_bytes.saturating_sub(*retained_state),
        )
    })?;
    if next_retained
        .checked_add(live_temporary_state)
        .is_none_or(|state| state > max_state_bytes)
    {
        return Err(failure(
            ArtifactReplayFailureClass::Budget,
            ArtifactReplayFailureStage::NativeHistory,
            Some(path),
            *cost,
            remaining_source.saturating_sub(observation.bytes_read()),
            max_state_bytes.saturating_sub(*retained_state),
        ));
    }
    *source_bytes_used = next_source;
    cost.native_history_source_read_bytes = cost
        .native_history_source_read_bytes
        .checked_add(observation.bytes_read())
        .ok_or_else(|| {
            failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::NativeHistory,
                Some(path),
                *cost,
                max_source_bytes.saturating_sub(*source_bytes_used),
                max_state_bytes.saturating_sub(*retained_state),
            )
        })?;
    cost.native_history_returned_state_bytes = cost
        .native_history_returned_state_bytes
        .checked_add(returned_state)
        .ok_or_else(|| {
            failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::NativeHistory,
                Some(path),
                *cost,
                max_source_bytes.saturating_sub(*source_bytes_used),
                max_state_bytes.saturating_sub(*retained_state),
            )
        })?;
    cost.native_history_map_state_bytes = cost
        .native_history_map_state_bytes
        .checked_add(map_entry_state)
        .ok_or_else(|| {
            failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::NativeHistory,
                Some(path),
                *cost,
                max_source_bytes.saturating_sub(*source_bytes_used),
                max_state_bytes.saturating_sub(*retained_state),
            )
        })?;
    cost.peak_temporary_state_bytes = cost
        .peak_temporary_state_bytes
        .max(next_retained.saturating_add(live_temporary_state));
    *retained_state = next_retained;
    histories.insert(path.to_owned(), observation);
    Ok(())
}

/// Build candidate-fenced native Artifact history/replay evidence from the
/// completed streamed Records index. Only Artifact rows are retained while
/// joining that index to the strict descendant metadata cursor; no whole
/// current-source member map or synthetic revision is created.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_candidate_artifact_replay<'report, 'store, I: Copy + Eq>(
    input: &dyn SourceCutInputWithIdentity<I>,
    full_coverage: &SourceCutInputCoverage,
    records: &'report SourceFoundationRecordsStreamedReport<'store, I>,
    source_root: &Path,
    effective_uid: u64,
    worker: &mut CandidateCutWorkerSchemaExecutor<I>,
    history_limits: ItemLimits,
    page_budget: SourceFoundationRecordsPageBudget,
    max_scan_rows: usize,
    max_diagnostic_validation_work: usize,
    cancelled: &AtomicBool,
) -> Result<CandidateArtifactReplayEvidence<'report, 'store, I>, ArtifactReplayFailure> {
    let mut cost = ArtifactReplayCost::default();
    let deadline = history_limits.deadline;
    let max_source_bytes = history_limits.max_total_bytes;
    let max_state_bytes = history_limits.max_state_bytes;
    let input_identity = *input.input_identity();
    let membership = *records.source_membership();

    if !source_root.is_absolute()
        || source_root.to_str().is_none()
        || max_scan_rows == 0
        || max_diagnostic_validation_work == 0
        || max_state_bytes == 0
        || input_identity != *records.input_identity()
        || full_coverage.membership() != membership
    {
        return Err(failure(
            ArtifactReplayFailureClass::Incomplete,
            ArtifactReplayFailureStage::CutBinding,
            None,
            cost,
            max_source_bytes,
            max_state_bytes,
        ));
    }
    let schema_identity = records.candidate_schema_identity().ok_or_else(|| {
        failure(
            ArtifactReplayFailureClass::Incomplete,
            ArtifactReplayFailureStage::CutBinding,
            None,
            cost,
            max_source_bytes,
            max_state_bytes,
        )
    })?;
    if worker.input_identity() != &input_identity
        || schema_identity.profile() != worker.profile()
        || schema_identity.schema_set_digest() != worker.schema_set_digest()
        || schema_identity.contract_selection_digest() != worker.contract_selection_digest()
        || schema_identity.prepared_execution_binding() != worker.prepared_execution_binding()
    {
        return Err(failure(
            ArtifactReplayFailureClass::Source,
            ArtifactReplayFailureStage::CutBinding,
            None,
            cost,
            max_source_bytes,
            max_state_bytes,
        ));
    }
    input
        .verify_current_fence(full_coverage, deadline, cancelled)
        .map_err(|refusal| {
            refusal_failure(
                &refusal,
                ArtifactReplayFailureStage::CutBinding,
                None,
                cost,
                max_source_bytes,
                max_state_bytes,
            )
        })?;

    let invalid_schema_proofs = CandidateArtifactInvalidSchemaProofs::prepare_candidate(
        input,
        records,
        page_budget,
        max_state_bytes,
        max_scan_rows,
        max_diagnostic_validation_work,
        deadline,
        cancelled,
    )
    .map_err(|refusal| {
        refusal_failure(
            &refusal,
            ArtifactReplayFailureStage::CurrentRecord,
            None,
            cost,
            max_source_bytes,
            max_state_bytes,
        )
    })?;
    let proof_cost = invalid_schema_proofs.cost();
    cost.schema_proof_retained_state_bytes = proof_cost.retained_state_bytes;
    cost.schema_proof_prepare_work_upper_bound = proof_cost
        .current_record_rows_scanned
        .checked_add(proof_cost.schema_diagnostic_rows_scanned)
        .and_then(|rows| rows.checked_add(proof_cost.diagnostic_validation_work_upper_bound))
        .ok_or_else(|| {
            failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::CurrentRecord,
                None,
                cost,
                max_source_bytes,
                max_state_bytes,
            )
        })?;
    cost.schema_proof_validation_work_upper_bound = proof_cost.current_path_probe_count;
    cost.schema_proof_hash_input_bytes_upper_bound = proof_cost.hash_input_bytes_upper_bound;
    let proof_scan_rows = proof_cost
        .current_record_rows_scanned
        .checked_add(proof_cost.schema_diagnostic_rows_scanned)
        .ok_or_else(|| {
            failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::CurrentRecord,
                None,
                cost,
                max_source_bytes,
                max_state_bytes,
            )
        })?;
    if proof_scan_rows > max_scan_rows {
        return Err(failure(
            ArtifactReplayFailureClass::Budget,
            ArtifactReplayFailureStage::CurrentRecord,
            None,
            cost,
            max_source_bytes,
            max_state_bytes,
        ));
    }
    invalid_schema_proofs
        .validate_report_binding(input, records, deadline, cancelled)
        .map_err(|refusal| {
            refusal_failure(
                &refusal,
                ArtifactReplayFailureStage::CurrentRecord,
                None,
                cost,
                max_source_bytes,
                max_state_bytes.saturating_sub(proof_cost.retained_state_bytes),
            )
        })?;

    let mut retained_state = proof_cost.retained_state_bytes;
    let proof_page_peak = proof_cost
        .retained_state_bytes
        .checked_add(proof_cost.index_page_peak_state_bytes)
        .ok_or_else(|| {
            failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::CurrentRecord,
                None,
                cost,
                max_source_bytes,
                max_state_bytes.saturating_sub(proof_cost.retained_state_bytes),
            )
        })?;
    cost.peak_temporary_state_bytes = cost.peak_temporary_state_bytes.max(proof_page_peak);
    let source_root = source_root.to_str().ok_or_else(|| {
        failure(
            ArtifactReplayFailureClass::Source,
            ArtifactReplayFailureStage::CutBinding,
            None,
            cost,
            max_source_bytes,
            max_state_bytes.saturating_sub(retained_state),
        )
    })?;
    let mut histories = BTreeMap::new();
    let mut replays = BTreeMap::new();
    let mut skips = Vec::new();
    let mut source_bytes_used = 0u64;
    let mut prefix_member_count = 0u64;
    let mut callback_failure = None;
    let prefix_result = input.for_each_current_member_meta_under(
        ARTIFACTS.trim_end_matches('/'),
        deadline,
        cancelled,
        &mut |meta| {
            if prefix_member_count % 128 == 0 {
                if cancelled.load(Ordering::Relaxed) {
                    return Err(ItemRefusal::Source(
                        "candidate Artifact prefix scan cancelled".into(),
                    ));
                }
                if Instant::now() >= deadline {
                    return Err(ItemRefusal::Deadline);
                }
            }
            prefix_member_count = prefix_member_count
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
            scan_rows_used = scan_rows_used
                .checked_add(1)
                .filter(|rows| *rows <= max_scan_rows)
                .ok_or(ItemRefusal::Budget)?;
            cost.candidate_record_index_work_upper_bound = cost
                .candidate_record_index_work_upper_bound
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
            if !is_artifact_record_path(meta.path) {
                return Ok(());
            }
            let canonical = RelativePath::parse(meta.path).map_err(|_| {
                ItemRefusal::Source("candidate Artifact path is not canonical".into())
            })?;
            if canonical.as_str() != meta.path {
                return Err(ItemRefusal::Source(
                    "candidate Artifact path is not canonical".into(),
                ));
            }
            let path_temporary = meta
                .path
                .len()
                .checked_mul(2)
                .and_then(|state| state.checked_add(size_of::<String>() + 1_024))
                .ok_or(ItemRefusal::Budget)?;
            let parent_bytes = meta
                .path
                .rsplit_once('/')
                .map(|(parent, _)| parent.len())
                .ok_or_else(|| {
                    ItemRefusal::Source("candidate Artifact path has no parent".into())
                })?;
            let companion_name_bytes = NATIVE_ARTIFACT_COMPANIONS
                .iter()
                .map(|name| name.len())
                .max()
                .ok_or(ItemRefusal::Budget)?;
            let companion_temporary = parent_bytes
                .checked_add(companion_name_bytes)
                .and_then(|state| state.checked_add(1 + size_of::<String>() + 1_024))
                .ok_or(ItemRefusal::Budget)?;
            let path_live_temporary = path_temporary.max(companion_temporary);
            let lookup_allowance = max_state_bytes
                .checked_sub(retained_state)
                .and_then(|state| state.checked_sub(path_live_temporary))
                .and_then(std::num::NonZeroUsize::new)
                .ok_or(ItemRefusal::Budget)?;
            let indexed_record = records.index().visit_candidate_artifact_record_path(
                meta.path,
                lookup_allowance,
                deadline,
                cancelled,
            )?;
            let lookup_state = indexed_record.map_or(0, |summary| summary.charged_state_bytes);
            let live_temporary = path_live_temporary
                .checked_add(lookup_state)
                .ok_or(ItemRefusal::Budget)?;
            if retained_state
                .checked_add(live_temporary)
                .is_none_or(|state| state > max_state_bytes)
            {
                return Err(ItemRefusal::Budget);
            }
            cost.candidate_record_index_state_bytes =
                cost.candidate_record_index_state_bytes.max(lookup_state);
            cost.candidate_record_index_work_upper_bound = cost
                .candidate_record_index_work_upper_bound
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
            cost.peak_temporary_state_bytes = cost
                .peak_temporary_state_bytes
                .max(retained_state.saturating_add(live_temporary));
            match process_candidate_artifact_path(
                input,
                input_identity,
                membership,
                &invalid_schema_proofs,
                history_limits,
                meta.path,
                meta.size_bytes,
                indexed_record,
                live_temporary,
                &mut retained_state,
                &mut source_bytes_used,
                &mut histories,
                &mut skips,
                &mut cost,
                cancelled,
            ) {
                Ok(()) => Ok(()),
                Err(failure) => {
                    let refusal = match failure.class {
                        ArtifactReplayFailureClass::Budget => ItemRefusal::Budget,
                        ArtifactReplayFailureClass::Deadline => ItemRefusal::Deadline,
                        ArtifactReplayFailureClass::Source => ItemRefusal::Source(
                            "candidate Artifact evidence callback failed".into(),
                        ),
                        ArtifactReplayFailureClass::Incomplete => ItemRefusal::Unsupported(
                            "candidate Artifact evidence callback incomplete".into(),
                        ),
                    };
                    callback_failure = Some(failure);
                    Err(refusal)
                }
            }
        },
    );
    if let Some(failure) = callback_failure {
        return Err(failure);
    }
    let prefix_coverage = prefix_result.map_err(|refusal| {
        refusal_failure(
            &refusal,
            ArtifactReplayFailureStage::CompanionFacts,
            None,
            cost,
            max_source_bytes.saturating_sub(source_bytes_used),
            max_state_bytes.saturating_sub(retained_state),
        )
    })?;
    let unvisited_artifact_records = records
        .index()
        .has_unvisited_candidate_artifact_record_paths(deadline, cancelled)
        .map_err(|refusal| {
            refusal_failure(
                &refusal,
                ArtifactReplayFailureStage::CurrentRecord,
                None,
                cost,
                max_source_bytes.saturating_sub(source_bytes_used),
                max_state_bytes.saturating_sub(retained_state),
            )
        })?;
    if prefix_coverage.directory() != ARTIFACTS.trim_end_matches('/')
        || prefix_coverage.member_count() != prefix_member_count
        || unvisited_artifact_records
    {
        return Err(failure(
            ArtifactReplayFailureClass::Source,
            ArtifactReplayFailureStage::CompanionFacts,
            None,
            cost,
            max_source_bytes.saturating_sub(source_bytes_used),
            max_state_bytes.saturating_sub(retained_state),
        ));
    }

    for (path, history) in &histories {
        if history.history_receipt_count() == 0 {
            continue;
        }
        active(deadline, cancelled).map_err(|class| {
            failure(
                class,
                ArtifactReplayFailureStage::CorrectionReplay,
                Some(path),
                cost,
                max_source_bytes.saturating_sub(source_bytes_used),
                max_state_bytes.saturating_sub(retained_state),
            )
        })?;
        let remaining_source =
            max_source_bytes
                .checked_sub(source_bytes_used)
                .ok_or_else(|| {
                    failure(
                        ArtifactReplayFailureClass::Budget,
                        ArtifactReplayFailureStage::CorrectionReplay,
                        Some(path),
                        cost,
                        0,
                        max_state_bytes.saturating_sub(retained_state),
                    )
                })?;
        let map_entry_state = map_state(path).ok_or_else(|| {
            failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::CorrectionReplay,
                Some(path),
                cost,
                remaining_source,
                max_state_bytes.saturating_sub(retained_state),
            )
        })?;
        let map_copy_state = map_entry_state.checked_mul(2).ok_or_else(|| {
            failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::CorrectionReplay,
                Some(path),
                cost,
                remaining_source,
                max_state_bytes.saturating_sub(retained_state),
            )
        })?;
        let replay_state_limit = max_state_bytes
            .checked_sub(retained_state)
            .and_then(|state| state.checked_sub(map_copy_state))
            .ok_or_else(|| {
                failure(
                    ArtifactReplayFailureClass::Budget,
                    ArtifactReplayFailureStage::CorrectionReplay,
                    Some(path),
                    cost,
                    remaining_source,
                    max_state_bytes.saturating_sub(retained_state),
                )
            })?;
        let (observation, source_bytes, input_copy_state) =
            source_revisions::replay_artifact_corrections_from_candidate(
                input,
                records,
                source_root,
                effective_uid,
                history,
                worker,
                history_limits.max_member_bytes,
                remaining_source,
                replay_state_limit,
                deadline,
                cancelled,
            )
            .map_err(|error| {
                failure(
                    command_class(&error),
                    ArtifactReplayFailureStage::CorrectionReplay,
                    Some(path),
                    cost,
                    remaining_source,
                    max_state_bytes.saturating_sub(retained_state),
                )
            })?;
        if observation.input_identity() != &input_identity
            || observation.current_membership() != membership
            || observation.source_path() != path
            || observation.record_id() != history.identity()
        {
            return Err(failure(
                ArtifactReplayFailureClass::Source,
                ArtifactReplayFailureStage::CorrectionReplay,
                Some(path),
                cost,
                remaining_source.saturating_sub(source_bytes),
                max_state_bytes.saturating_sub(retained_state),
            ));
        }
        let next_source = source_bytes_used
            .checked_add(source_bytes)
            .filter(|used| *used <= max_source_bytes)
            .ok_or_else(|| {
                failure(
                    ArtifactReplayFailureClass::Budget,
                    ArtifactReplayFailureStage::CorrectionReplay,
                    Some(path),
                    cost,
                    remaining_source,
                    max_state_bytes.saturating_sub(retained_state),
                )
            })?;
        let publication_state = observation.publication_state_bytes();
        let returned_state = observation.returned_state_bytes();
        let added_state = input_copy_state
            .checked_add(publication_state)
            .and_then(|state| state.checked_add(returned_state))
            .and_then(|state| state.checked_add(map_copy_state))
            .ok_or_else(|| {
                failure(
                    ArtifactReplayFailureClass::Budget,
                    ArtifactReplayFailureStage::CorrectionReplay,
                    Some(path),
                    cost,
                    remaining_source.saturating_sub(source_bytes),
                    max_state_bytes.saturating_sub(retained_state),
                )
            })?;
        let next_retained = retained_state.checked_add(added_state).ok_or_else(|| {
            failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::CorrectionReplay,
                Some(path),
                cost,
                remaining_source.saturating_sub(source_bytes),
                max_state_bytes.saturating_sub(retained_state),
            )
        })?;
        if next_retained > max_state_bytes {
            return Err(failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::CorrectionReplay,
                Some(path),
                cost,
                remaining_source.saturating_sub(source_bytes),
                max_state_bytes.saturating_sub(retained_state),
            ));
        }
        source_bytes_used = next_source;
        cost.candidate_source_read_bytes = cost
            .candidate_source_read_bytes
            .checked_add(source_bytes)
            .ok_or_else(|| {
                failure(
                    ArtifactReplayFailureClass::Budget,
                    ArtifactReplayFailureStage::CorrectionReplay,
                    Some(path),
                    cost,
                    max_source_bytes.saturating_sub(source_bytes_used),
                    max_state_bytes.saturating_sub(retained_state),
                )
            })?;
        cost.replay_input_copy_state_bytes = cost
            .replay_input_copy_state_bytes
            .checked_add(input_copy_state)
            .ok_or_else(|| {
                failure(
                    ArtifactReplayFailureClass::Budget,
                    ArtifactReplayFailureStage::CorrectionReplay,
                    Some(path),
                    cost,
                    max_source_bytes.saturating_sub(source_bytes_used),
                    max_state_bytes.saturating_sub(retained_state),
                )
            })?;
        cost.replay_publication_state_bytes = cost
            .replay_publication_state_bytes
            .checked_add(publication_state)
            .ok_or_else(|| {
                failure(
                    ArtifactReplayFailureClass::Budget,
                    ArtifactReplayFailureStage::CorrectionReplay,
                    Some(path),
                    cost,
                    max_source_bytes.saturating_sub(source_bytes_used),
                    max_state_bytes.saturating_sub(retained_state),
                )
            })?;
        cost.replay_returned_state_bytes = cost
            .replay_returned_state_bytes
            .checked_add(returned_state)
            .ok_or_else(|| {
                failure(
                    ArtifactReplayFailureClass::Budget,
                    ArtifactReplayFailureStage::CorrectionReplay,
                    Some(path),
                    cost,
                    max_source_bytes.saturating_sub(source_bytes_used),
                    max_state_bytes.saturating_sub(retained_state),
                )
            })?;
        cost.replay_map_state_bytes = cost
            .replay_map_state_bytes
            .checked_add(map_copy_state)
            .ok_or_else(|| {
                failure(
                    ArtifactReplayFailureClass::Budget,
                    ArtifactReplayFailureStage::CorrectionReplay,
                    Some(path),
                    cost,
                    max_source_bytes.saturating_sub(source_bytes_used),
                    max_state_bytes.saturating_sub(retained_state),
                )
            })?;
        cost.peak_temporary_state_bytes = cost.peak_temporary_state_bytes.max(next_retained);
        retained_state = next_retained;
        replays.insert(path.clone(), observation);
    }

    if input.input_identity() != &input_identity
        || records.input_identity() != &input_identity
        || *records.source_membership() != membership
        || records.candidate_schema_identity() != Some(schema_identity)
    {
        return Err(failure(
            ArtifactReplayFailureClass::Source,
            ArtifactReplayFailureStage::CutBinding,
            None,
            cost,
            max_source_bytes.saturating_sub(source_bytes_used),
            max_state_bytes.saturating_sub(retained_state),
        ));
    }
    input
        .verify_current_fence(full_coverage, deadline, cancelled)
        .map_err(|refusal| {
            refusal_failure(
                &refusal,
                ArtifactReplayFailureStage::CutBinding,
                None,
                cost,
                max_source_bytes.saturating_sub(source_bytes_used),
                max_state_bytes.saturating_sub(retained_state),
            )
        })?;
    let accounted_retained_state = cost.retained_state_upper_bound_bytes().ok_or_else(|| {
        failure(
            ArtifactReplayFailureClass::Budget,
            ArtifactReplayFailureStage::CorrectionReplay,
            None,
            cost,
            max_source_bytes.saturating_sub(source_bytes_used),
            max_state_bytes.saturating_sub(retained_state),
        )
    })?;
    if accounted_retained_state > max_state_bytes || retained_state > max_state_bytes {
        return Err(failure(
            ArtifactReplayFailureClass::Budget,
            ArtifactReplayFailureStage::CorrectionReplay,
            None,
            cost,
            max_source_bytes.saturating_sub(source_bytes_used),
            max_state_bytes.saturating_sub(retained_state),
        ));
    }
    cost.peak_temporary_state_bytes = cost
        .peak_temporary_state_bytes
        .max(accounted_retained_state);
    Ok(CandidateArtifactReplayEvidence {
        histories,
        replays,
        invalid_schema_proofs,
        skips,
        cost,
    })
}

fn fallback_record_value(
    cut: &CorpusCutReader,
    current_paths: &[String],
    path: &str,
    revision: SourceRevision,
    limits: ItemLimits,
    retained_state: usize,
    source_read_bytes: &mut u64,
    cost: &mut ArtifactReplayCost,
    cancelled: &AtomicBool,
) -> Result<Option<serde_json::Value>, ArtifactReplayFailureClass> {
    if current_paths
        .binary_search_by(|candidate| candidate.as_str().cmp(path))
        .is_err()
    {
        return Err(ArtifactReplayFailureClass::Source);
    }
    let relative = RelativePath::parse(path).map_err(|_| ArtifactReplayFailureClass::Source)?;
    let metadata = cut
        .current()
        .member(&relative)
        .ok_or(ArtifactReplayFailureClass::Source)?;
    let member_size =
        usize::try_from(metadata.size_bytes).map_err(|_| ArtifactReplayFailureClass::Budget)?;
    if member_size > limits.max_member_bytes {
        return Err(ArtifactReplayFailureClass::Budget);
    }
    let next_read_bytes = source_read_bytes
        .checked_add(metadata.size_bytes)
        .ok_or(ArtifactReplayFailureClass::Budget)?;
    if next_read_bytes > limits.max_total_bytes {
        return Err(ArtifactReplayFailureClass::Budget);
    }
    let mut identity_state = 0usize;
    // Use the same immutable per-path index that read_member uses for its
    // stable_ids allocation. Unrelated identities are not rescanned for each
    // missing record; these are carrier claims, not accepted identity facts.
    for (index, id) in cut.current().indexed_ids_for_path(&relative).enumerate() {
        if index % 128 == 0 {
            active(limits.deadline, cancelled)?;
        }
        identity_state = identity_state
            .checked_add(size_of::<String>() + 2 * size_of::<usize>())
            .and_then(|bytes| bytes.checked_add(id.len()))
            .ok_or(ArtifactReplayFailureClass::Budget)?;
    }
    let temporary = member_size
        .checked_mul(TEMP_JSON_STATE_PER_BYTE)
        .and_then(|bytes| bytes.checked_add(TEMP_JSON_FIXED_STATE))
        .and_then(|bytes| bytes.checked_add(member_size))
        .and_then(|bytes| bytes.checked_add(identity_state))
        .and_then(|bytes| bytes.checked_add(path.len() + 256))
        .ok_or(ArtifactReplayFailureClass::Budget)?;
    let peak = retained_state
        .checked_add(temporary)
        .ok_or(ArtifactReplayFailureClass::Budget)?;
    if peak > limits.max_state_bytes {
        return Err(ArtifactReplayFailureClass::Budget);
    }
    cost.peak_temporary_state_bytes = cost.peak_temporary_state_bytes.max(peak);
    active(limits.deadline, cancelled)?;
    let member = cut
        .read_member(
            revision,
            &relative,
            limits.max_member_bytes as u64,
            limits.deadline,
            cancelled,
        )
        .map_err(|_| ArtifactReplayFailureClass::Source)?;
    *source_read_bytes = next_read_bytes;
    cost.candidate_source_read_bytes = cost
        .candidate_source_read_bytes
        .checked_add(metadata.size_bytes)
        .ok_or(ArtifactReplayFailureClass::Budget)?;
    if member.revision != revision
        || member.path != relative
        || member.raw.len() as u64 != metadata.size_bytes
        || Digest256::of_bytes(&member.raw) != metadata.sha256
    {
        return Err(ArtifactReplayFailureClass::Source);
    }
    Ok(serde_json::from_slice(&member.raw).ok())
}

fn map_state(path: &str) -> Option<usize> {
    RETAINED_MAP_NODE_UPPER.checked_add(path.len())
}

fn retain_skip(
    skips: &mut Vec<ArtifactReplaySkip>,
    cost: &mut ArtifactReplayCost,
    path: &str,
    kind: ArtifactReplaySkipKind,
    retained_state: &mut usize,
    max_state: usize,
) -> Result<(), ArtifactReplayFailureClass> {
    let state = size_of::<ArtifactReplaySkip>()
        .checked_add(map_state(path).ok_or(ArtifactReplayFailureClass::Budget)?)
        .ok_or(ArtifactReplayFailureClass::Budget)?;
    let next_retained = retained_state
        .checked_add(state)
        .ok_or(ArtifactReplayFailureClass::Budget)?;
    if next_retained > max_state {
        return Err(ArtifactReplayFailureClass::Budget);
    }
    cost.skip_state_bytes = cost
        .skip_state_bytes
        .checked_add(state)
        .ok_or(ArtifactReplayFailureClass::Budget)?;
    *retained_state = next_retained;
    skips.push(ArtifactReplaySkip {
        path: path.to_owned(),
        kind,
    });
    Ok(())
}

fn replay_reservation(
    cut: &CorpusCutReader,
    current_paths: &[String],
    history: &NativeRecordHistoryReadObservation,
    source_root: &str,
    path: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<ReplayReservation, ArtifactReplayFailureClass> {
    let receipt_count = history.history_receipt_count();
    let transaction_count = history.transactions().len();
    if receipt_count > 128 || transaction_count > 128 {
        return Err(ArtifactReplayFailureClass::Budget);
    }
    let mut exact_path_bytes = 0usize;
    let mut exact_blob_bytes = 0usize;
    let mut exact_manifest_max = 0usize;
    let mut exact_path_text = 0usize;
    let mut exact_read_map_state = 0usize;
    let mut selected_source_file_bytes = 0usize;
    for (index, read) in history.reads().iter().enumerate() {
        if index % 128 == 0 {
            active(deadline, cancelled)?;
        }
        let PredicateRead::ExactPath {
            path: read_path, ..
        } = read
        else {
            continue;
        };
        let relative =
            RelativePath::parse(read_path).map_err(|_| ArtifactReplayFailureClass::Source)?;
        let metadata = cut
            .current()
            .member(&relative)
            .ok_or(ArtifactReplayFailureClass::Source)?;
        let bytes =
            usize::try_from(metadata.size_bytes).map_err(|_| ArtifactReplayFailureClass::Budget)?;
        exact_path_bytes = exact_path_bytes
            .checked_add(bytes)
            .ok_or(ArtifactReplayFailureClass::Budget)?;
        exact_path_text = exact_path_text
            .checked_add(read_path.len())
            .ok_or(ArtifactReplayFailureClass::Budget)?;
        exact_read_map_state = exact_read_map_state
            .checked_add(RETAINED_MAP_NODE_UPPER)
            .and_then(|n| n.checked_add(read_path.len()))
            .and_then(|n| {
                n.checked_add(match read {
                    PredicateRead::ExactPath { digest, .. } => digest.len(),
                    _ => 0,
                })
            })
            .ok_or(ArtifactReplayFailureClass::Budget)?;
        if read_path.ends_with(".blob") && read_path.contains("/.metadata-transactions/") {
            exact_blob_bytes = exact_blob_bytes
                .checked_add(bytes)
                .ok_or(ArtifactReplayFailureClass::Budget)?;
        } else if read_path.ends_with("/manifest.json")
            && read_path.contains("/.metadata-transactions/")
        {
            exact_manifest_max = exact_manifest_max.max(bytes);
        }
    }
    for raw in history.selected_package().values() {
        selected_source_file_bytes = selected_source_file_bytes
            .checked_add(raw.len())
            .ok_or(ArtifactReplayFailureClass::Budget)?;
    }
    let max_manifest_workspace = exact_manifest_max
        .checked_mul(TEMP_JSON_STATE_PER_BYTE)
        .and_then(|n| n.checked_add(TEMP_JSON_FIXED_STATE))
        .ok_or(ArtifactReplayFailureClass::Budget)?;
    let publication_state = exact_blob_bytes
        .checked_add(
            transaction_count
                .checked_mul(65_536 + 512)
                .ok_or(ArtifactReplayFailureClass::Budget)?,
        )
        .and_then(|n| n.checked_add(exact_path_text.checked_mul(4)?))
        .and_then(|n| n.checked_add(max_manifest_workspace))
        .and_then(|n| n.checked_add(65_536))
        .ok_or(ArtifactReplayFailureClass::Budget)?;
    let mut transaction_string_bytes = 0usize;
    for transaction in history.transactions() {
        active(deadline, cancelled)?;
        transaction_string_bytes = transaction_string_bytes
            .checked_add(transaction.transaction_id().len())
            .and_then(|n| n.checked_add(transaction.manifest_sha256().len()))
            .ok_or(ArtifactReplayFailureClass::Budget)?;
    }
    let returned_state = REPLAY_OBSERVATION_UPPER
        .checked_add(
            source_root
                .len()
                .checked_mul(2)
                .ok_or(ArtifactReplayFailureClass::Budget)?,
        )
        .and_then(|n| n.checked_add(path.len().checked_mul(2)?))
        .and_then(|n| n.checked_add(history.identity().len().checked_mul(2)?))
        .and_then(|n| n.checked_add(history.origin_record_sha256().len().checked_mul(2)?))
        .and_then(|n| n.checked_add(history.history_sha256().map_or(0, |value| value.len() * 2)))
        .and_then(|n| n.checked_add(transaction_count.checked_mul(REPLAY_TRANSACTION_UPPER)?))
        .and_then(|n| n.checked_add(transaction_string_bytes.checked_mul(3)?))
        .ok_or(ArtifactReplayFailureClass::Budget)?;
    let map_state = map_state(path).ok_or(ArtifactReplayFailureClass::Budget)?;
    let mut package_copy_state = 0usize;
    let parent = path
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .ok_or(ArtifactReplayFailureClass::Source)?;
    for name in history.selected_package().keys() {
        let full_path = parent
            .len()
            .checked_add(1)
            .and_then(|n| n.checked_add(name.len()))
            .ok_or(ArtifactReplayFailureClass::Budget)?;
        package_copy_state = package_copy_state
            .checked_add(RETAINED_MAP_NODE_UPPER)
            .and_then(|n| n.checked_add(full_path.checked_mul(2)?))
            .and_then(|n| n.checked_add(size_of::<SourceFile>()))
            .and_then(|n| n.checked_add(history.selected_package().get(name)?.len()))
            .ok_or(ArtifactReplayFailureClass::Budget)?;
    }
    let mut file_container_state = 0usize;
    for (index, current_path) in current_paths.iter().enumerate() {
        if index % 128 == 0 {
            active(deadline, cancelled)?;
        }
        file_container_state = file_container_state
            .checked_add(RETAINED_MAP_NODE_UPPER)
            .and_then(|n| n.checked_add(size_of::<SourceFile>()))
            .and_then(|n| n.checked_add(current_path.len().checked_mul(2)?))
            .ok_or(ArtifactReplayFailureClass::Budget)?;
    }
    let worker_peak_state = exact_path_bytes
        .checked_add(selected_source_file_bytes)
        .and_then(|n| n.checked_add(max_manifest_workspace))
        .and_then(|n| n.checked_add(2 * 8_388_608))
        .and_then(|n| n.checked_add(file_container_state))
        .and_then(|n| n.checked_add(65_536))
        .ok_or(ArtifactReplayFailureClass::Budget)?;
    let read_observation_state = history
        .reads()
        .len()
        .checked_mul(size_of::<PredicateRead>())
        .ok_or(ArtifactReplayFailureClass::Budget)?;
    let input_copy_state = package_copy_state
        .checked_add(exact_read_map_state)
        .and_then(|n| n.checked_add(file_container_state))
        .and_then(|n| n.checked_add(read_observation_state))
        .ok_or(ArtifactReplayFailureClass::Budget)?;
    Ok(ReplayReservation {
        publication_state,
        returned_state,
        input_copy_state,
        map_state,
        worker_peak_state,
    })
}

fn verify_file_current(
    cut: &CorpusCutReader,
    current_paths: &[String],
    path: &str,
    raw: &[u8],
) -> Result<RelativePath, ArtifactReplayFailureClass> {
    if current_paths
        .binary_search_by(|candidate| candidate.as_str().cmp(path))
        .is_err()
    {
        return Err(ArtifactReplayFailureClass::Source);
    }
    let relative = RelativePath::parse(path).map_err(|_| ArtifactReplayFailureClass::Source)?;
    let metadata = cut
        .current()
        .member(&relative)
        .ok_or(ArtifactReplayFailureClass::Source)?;
    if raw.len() as u64 != metadata.size_bytes || Digest256::of_bytes(raw) != metadata.sha256 {
        return Err(ArtifactReplayFailureClass::Source);
    }
    Ok(relative)
}

fn insert_file(
    files: &mut BTreeMap<String, SourceFile>,
    path: String,
    raw: Vec<u8>,
    expected_digest: Option<&str>,
    cut: &CorpusCutReader,
    current_paths: &[String],
) -> Result<(), ArtifactReplayFailureClass> {
    let relative = verify_file_current(cut, current_paths, &path, &raw)?;
    if expected_digest.is_some_and(|digest| Digest256::of_bytes(&raw).to_prefixed() != digest) {
        return Err(ArtifactReplayFailureClass::Source);
    }
    if let Some(existing) = files.get(&path) {
        if existing.raw != raw || existing.path != relative {
            return Err(ArtifactReplayFailureClass::Source);
        }
        return Ok(());
    }
    files.insert(
        path,
        SourceFile {
            path: relative,
            raw,
        },
    );
    Ok(())
}

fn build_replay_input_files(
    transport: &mut CapturedReadonlyRecordFiles<'_, '_>,
    max_member_bytes: u64,
    cut: &CorpusCutReader,
    current_paths: &[String],
    history: &NativeRecordHistoryReadObservation,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<SourceFile>, ArtifactReplayFailureClass> {
    let mut files = BTreeMap::<String, SourceFile>::new();
    let collected = source_revisions::collect_readonly_record_files(
        transport,
        history.record_path(),
        deadline,
        cancelled,
    )
    .map_err(|error| command_class(&error))?;
    for (index, file) in collected.into_iter().enumerate() {
        if index % 128 == 0 {
            active(deadline, cancelled)?;
        }
        let path = file.path.as_str().to_owned();
        insert_file(&mut files, path, file.raw, None, cut, current_paths)?;
    }

    let parent = history
        .record_path()
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .ok_or(ArtifactReplayFailureClass::Source)?;
    for (name, raw) in history.selected_package() {
        active(deadline, cancelled)?;
        let path = format!("{parent}/{name}");
        if let Some(existing) = files.get(&path) {
            if existing.raw != *raw {
                return Err(ArtifactReplayFailureClass::Source);
            }
            continue;
        }
        let relative = verify_file_current(cut, current_paths, &path, raw)?;
        files.insert(
            path,
            SourceFile {
                path: relative,
                raw: raw.clone(),
            },
        );
    }

    let mut expected_reads = BTreeMap::<String, String>::new();
    for (index, read) in history.reads().iter().enumerate() {
        if index % 128 == 0 {
            active(deadline, cancelled)?;
        }
        let PredicateRead::ExactPath { path, digest } = read else {
            continue;
        };
        if let Some(previous) = expected_reads.insert(path.clone(), digest.clone())
            && previous != *digest
        {
            return Err(ArtifactReplayFailureClass::Source);
        }
    }
    for (index, (path, digest)) in expected_reads.into_iter().enumerate() {
        if index % 128 == 0 {
            active(deadline, cancelled)?;
        }
        if let Some(existing) = files.get(&path) {
            if Digest256::of_bytes(&existing.raw).to_prefixed() != digest {
                return Err(ArtifactReplayFailureClass::Source);
            }
            continue;
        }
        let relative =
            RelativePath::parse(&path).map_err(|_| ArtifactReplayFailureClass::Source)?;
        if current_paths
            .binary_search_by(|candidate| candidate.as_str().cmp(&path))
            .is_err()
        {
            return Err(ArtifactReplayFailureClass::Source);
        }
        let metadata = cut
            .current()
            .member(&relative)
            .ok_or(ArtifactReplayFailureClass::Source)?;
        let max_bytes =
            usize::try_from(max_member_bytes).map_err(|_| ArtifactReplayFailureClass::Budget)?;
        let raw = transport
            .read(&path, max_bytes, deadline, cancelled)
            .map_err(|error| command_class(&error))?;
        if raw.len() as u64 != metadata.size_bytes || Digest256::of_bytes(&raw) != metadata.sha256 {
            return Err(ArtifactReplayFailureClass::Source);
        }
        insert_file(&mut files, path, raw, Some(&digest), cut, current_paths)?;
    }
    Ok(files.into_values().collect())
}

#[allow(clippy::too_many_arguments)]
fn replay_one(
    cut: &CorpusCutReader,
    current_paths: &[String],
    max_member_bytes: u64,
    history: &NativeRecordHistoryReadObservation,
    source_root: &str,
    effective_uid: u64,
    worker: &mut CutWorkerSchemaExecutor,
    transport: &mut CapturedReadonlyRecordFiles<'_, '_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<ArtifactCorrectionReplayObservation, ArtifactReplayFailureClass> {
    if history.history_receipt_count() == 0 {
        return Err(ArtifactReplayFailureClass::Incomplete);
    }
    if history
        .current_record()
        .get("schema_version")
        .and_then(serde_json::Value::as_str)
        != Some(ARTIFACT_V2_RECORD)
    {
        return Err(ArtifactReplayFailureClass::Incomplete);
    }
    let files = build_replay_input_files(
        transport,
        max_member_bytes,
        cut,
        current_paths,
        history,
        deadline,
        cancelled,
    )?;
    let input = RecordVersionReadInput {
        files: &files,
        source_revision: history.source_revision(),
        effective_uid,
        schema_source_path: history.record_path(),
    };
    source_revisions::replay_artifact_corrections_from_cut(
        &input,
        source_root,
        cut,
        history,
        worker,
        deadline,
        cancelled,
    )
    .map_err(|error| command_class(&error))
}

/// Reconstruct only native Artifact histories that Discovery's maintained
/// predicate will consume: exact authored Artifact path, all four observed
/// creation companions present, and the exact current v2 `$schema` URI. Known
/// Legacy, partial, invalid-schema, and invalid-JSON branches remain with
/// Discovery's existing owner rules. Missing observations, budget failures,
/// and source/kernel refusals are terminal and never become empty evidence.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_artifact_replay<'cut>(
    captured: &'cut FoundationCapturedCut,
    current_paths: &[String],
    records: &SourceFoundationRecordsReport,
    physical: &SourcePhysicalFacts,
    source_root: &Path,
    effective_uid: u64,
    worker: &mut CutWorkerSchemaExecutor,
    history_limits: ItemLimits,
    readonly_limits: CapturedReadonlyRecordLimits,
    cancelled: &AtomicBool,
) -> Result<ArtifactReplayEvidence<'cut>, ArtifactReplayFailure> {
    let mut cost = ArtifactReplayCost::default();
    let deadline = history_limits.deadline;
    let phase_remaining_source = history_limits.max_total_bytes;
    let phase_remaining_state = history_limits.max_state_bytes;
    let (revision, membership, record_report) =
        validate_membership(captured, current_paths, records, deadline, cancelled).map_err(
            |class| {
                failure(
                    class,
                    ArtifactReplayFailureStage::CutBinding,
                    None,
                    cost,
                    phase_remaining_source,
                    phase_remaining_state,
                )
            },
        )?;
    if worker.source_revision() != revision
        || !source_root.is_absolute()
        || source_root.to_str().is_none()
    {
        return Err(failure(
            ArtifactReplayFailureClass::Source,
            ArtifactReplayFailureStage::CutBinding,
            None,
            cost,
            phase_remaining_source,
            phase_remaining_state,
        ));
    }
    let source_root = source_root.to_str().ok_or_else(|| {
        failure(
            ArtifactReplayFailureClass::Source,
            ArtifactReplayFailureStage::CutBinding,
            None,
            cost,
            phase_remaining_source,
            phase_remaining_state,
        )
    })?;

    let invalid_schema_proofs = CurrentArtifactInvalidSchemaProofs::prepare(
        captured.cut(),
        record_report,
        phase_remaining_state,
        deadline,
        cancelled,
    )
    .map_err(|refusal| {
        failure(
            refusal_class(&refusal),
            ArtifactReplayFailureStage::CurrentRecord,
            None,
            cost,
            phase_remaining_source,
            phase_remaining_state,
        )
    })?;
    let proof_cost = invalid_schema_proofs.cost();
    cost.schema_proof_retained_state_bytes = proof_cost.retained_state_bytes;
    cost.schema_proof_prepare_work_upper_bound = proof_cost.preparation_scan_work_upper_bound;
    cost.schema_proof_validation_work_upper_bound = proof_cost.validation_scan_work_upper_bound;
    cost.schema_proof_hash_input_bytes_upper_bound = proof_cost.hash_input_bytes_upper_bound;

    // Count eligible report rows before allocating one ordered borrowed index.
    // Charge the whole first scan up front; the index itself retains only path
    // and value references into this exact Records report.
    cost.candidate_record_index_work_upper_bound = record_report.records.len();
    let mut artifact_record_count = 0usize;
    let mut max_artifact_record_path_bytes = 0usize;
    for (index, record) in record_report.records.values().enumerate() {
        if index % 128 == 0 {
            active(deadline, cancelled).map_err(|class| {
                failure(
                    class,
                    ArtifactReplayFailureStage::CurrentRecord,
                    None,
                    cost,
                    phase_remaining_source,
                    phase_remaining_state,
                )
            })?;
        }
        if is_artifact_record_path(&record.path) {
            artifact_record_count = artifact_record_count.checked_add(1).ok_or_else(|| {
                failure(
                    ArtifactReplayFailureClass::Budget,
                    ArtifactReplayFailureStage::CurrentRecord,
                    Some(&record.path),
                    cost,
                    phase_remaining_source,
                    phase_remaining_state,
                )
            })?;
            max_artifact_record_path_bytes = max_artifact_record_path_bytes.max(record.path.len());
        }
    }

    // Precharge a max-path-length bound for the current-path scan and a
    // conservative binary depth for every BTreeMap insert/lookup before any
    // index allocation. usize::BITS bounds the depth even for a degenerate
    // tree shape; path bytes bound each ordered string comparison.
    let index_work_with_path_scan = cost
        .candidate_record_index_work_upper_bound
        .checked_add(current_paths.len())
        .ok_or_else(|| {
            failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::CurrentRecord,
                None,
                cost,
                phase_remaining_source,
                phase_remaining_state,
            )
        })?;
    cost.candidate_record_index_work_upper_bound = index_work_with_path_scan;
    let mut max_current_path_bytes = 0usize;
    for (index, path) in current_paths.iter().enumerate() {
        if index % 128 == 0 {
            active(deadline, cancelled).map_err(|class| {
                failure(
                    class,
                    ArtifactReplayFailureStage::CurrentRecord,
                    None,
                    cost,
                    phase_remaining_source,
                    phase_remaining_state,
                )
            })?;
        }
        max_current_path_bytes = max_current_path_bytes.max(path.len());
    }
    // Bound BTreeMap string comparisons by a machine-word-sized height and a
    // conservative twelve comparisons per node, then charge bytes per key.
    let index_comparisons = (usize::BITS as usize).checked_mul(12).ok_or_else(|| {
        failure(
            ArtifactReplayFailureClass::Budget,
            ArtifactReplayFailureStage::CurrentRecord,
            None,
            cost,
            phase_remaining_source,
            phase_remaining_state,
        )
    })?;
    let index_insert_work = artifact_record_count
        .checked_mul(index_comparisons)
        .and_then(|work| work.checked_mul(max_artifact_record_path_bytes))
        .ok_or_else(|| {
            failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::CurrentRecord,
                None,
                cost,
                phase_remaining_source,
                phase_remaining_state,
            )
        })?;
    let index_lookup_work = current_paths
        .len()
        .checked_mul(index_comparisons)
        .and_then(|work| work.checked_mul(max_current_path_bytes))
        .ok_or_else(|| {
            failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::CurrentRecord,
                None,
                cost,
                phase_remaining_source,
                phase_remaining_state,
            )
        })?;
    // The main candidate pass also visits each current path once before the
    // bounded map lookup, so include that row scan in this index work charge.
    let index_build_scan_work = record_report
        .records
        .len()
        .checked_add(current_paths.len())
        .ok_or_else(|| {
            failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::CurrentRecord,
                None,
                cost,
                phase_remaining_source,
                phase_remaining_state,
            )
        })?;
    let candidate_record_index_work_upper_bound = cost
        .candidate_record_index_work_upper_bound
        .checked_add(index_build_scan_work)
        .and_then(|work| work.checked_add(index_insert_work))
        .and_then(|work| work.checked_add(index_lookup_work))
        .ok_or_else(|| {
            failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::CurrentRecord,
                None,
                cost,
                phase_remaining_source,
                phase_remaining_state,
            )
        })?;
    cost.candidate_record_index_work_upper_bound = candidate_record_index_work_upper_bound;

    let index_entry_state = size_of::<(&str, Result<&serde_json::Value, ()>)>()
        .checked_add(RETAINED_MAP_NODE_UPPER)
        .ok_or_else(|| {
            failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::CurrentRecord,
                None,
                cost,
                phase_remaining_source,
                phase_remaining_state,
            )
        })?;
    let record_index_state = artifact_record_count
        .checked_mul(index_entry_state)
        .and_then(|bytes| {
            bytes.checked_add(size_of::<BTreeMap<&str, Result<&serde_json::Value, ()>>>())
        })
        .ok_or_else(|| {
            failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::CurrentRecord,
                None,
                cost,
                phase_remaining_source,
                phase_remaining_state,
            )
        })?;
    cost.candidate_record_index_state_bytes = record_index_state;
    let retained_with_record_index = proof_cost
        .retained_state_bytes
        .checked_add(record_index_state)
        .ok_or_else(|| {
            failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::CurrentRecord,
                None,
                cost,
                phase_remaining_source,
                0,
            )
        })?;
    cost.peak_temporary_state_bytes = cost
        .peak_temporary_state_bytes
        .max(retained_with_record_index);
    if retained_with_record_index > phase_remaining_state {
        return Err(failure(
            ArtifactReplayFailureClass::Budget,
            ArtifactReplayFailureStage::CurrentRecord,
            None,
            cost,
            phase_remaining_source,
            phase_remaining_state.saturating_sub(proof_cost.retained_state_bytes),
        ));
    }
    let mut record_values_by_path = BTreeMap::<&str, Result<&serde_json::Value, ()>>::new();
    for (index, record) in record_report.records.values().enumerate() {
        if index % 128 == 0 {
            active(deadline, cancelled).map_err(|class| {
                failure(
                    class,
                    ArtifactReplayFailureStage::CurrentRecord,
                    None,
                    cost,
                    phase_remaining_source,
                    phase_remaining_state.saturating_sub(retained_with_record_index),
                )
            })?;
        }
        if !is_artifact_record_path(&record.path) {
            continue;
        }
        match record_values_by_path.entry(record.path.as_str()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(Ok(&record.value));
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                // Keep duplicate identity explicit and defer refusal until
                // the same companion-gated lookup point as the old scan.
                entry.insert(Err(()));
            }
        }
    }

    let mut histories = BTreeMap::new();
    let mut replays = BTreeMap::new();
    let mut skips = Vec::new();
    let mut history_source_used = 0u64;
    let mut retained_history_state = retained_with_record_index;
    for path in current_paths
        .iter()
        .filter(|path| is_artifact_record_path(path))
    {
        active(deadline, cancelled).map_err(|class| {
            failure(
                class,
                ArtifactReplayFailureStage::CompanionFacts,
                Some(path),
                cost,
                phase_remaining_source.saturating_sub(history_source_used),
                phase_remaining_state.saturating_sub(retained_history_state),
            )
        })?;
        let max_companion_name = NATIVE_ARTIFACT_COMPANIONS
            .iter()
            .map(|name| name.len())
            .max()
            .ok_or(ArtifactReplayFailureClass::Budget)
            .map_err(|class| {
                failure(
                    class,
                    ArtifactReplayFailureStage::CompanionFacts,
                    Some(path),
                    cost,
                    phase_remaining_source.saturating_sub(history_source_used),
                    phase_remaining_state.saturating_sub(retained_history_state),
                )
            })?;
        let companion_temp = path
            .rsplit_once('/')
            .map(|(parent, _)| parent.len())
            .and_then(|parent| parent.checked_add(max_companion_name + 1))
            .and_then(|path_bytes| path_bytes.checked_add(size_of::<[bool; 4]>() + 256))
            .ok_or_else(|| {
                failure(
                    ArtifactReplayFailureClass::Budget,
                    ArtifactReplayFailureStage::CompanionFacts,
                    Some(path),
                    cost,
                    phase_remaining_source.saturating_sub(history_source_used),
                    phase_remaining_state.saturating_sub(retained_history_state),
                )
            })?;
        let companion_peak = retained_history_state
            .checked_add(companion_temp)
            .ok_or_else(|| {
                failure(
                    ArtifactReplayFailureClass::Budget,
                    ArtifactReplayFailureStage::CompanionFacts,
                    Some(path),
                    cost,
                    phase_remaining_source.saturating_sub(history_source_used),
                    0,
                )
            })?;
        if companion_peak > phase_remaining_state {
            return Err(failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::CompanionFacts,
                Some(path),
                cost,
                phase_remaining_source.saturating_sub(history_source_used),
                phase_remaining_state.saturating_sub(retained_history_state),
            ));
        }
        cost.peak_temporary_state_bytes = cost.peak_temporary_state_bytes.max(companion_peak);
        let Some(companions) = companion_state(physical, path) else {
            return Err(failure(
                ArtifactReplayFailureClass::Incomplete,
                ArtifactReplayFailureStage::CompanionFacts,
                Some(path),
                cost,
                phase_remaining_source.saturating_sub(history_source_used),
                phase_remaining_state.saturating_sub(retained_history_state),
            ));
        };
        let present_count = companions.iter().filter(|present| **present).count();
        match present_count {
            0 => {
                retain_skip(
                    &mut skips,
                    &mut cost,
                    path,
                    ArtifactReplaySkipKind::LegacyNoCreationCompanions,
                    &mut retained_history_state,
                    phase_remaining_state,
                )
                .map_err(|class| {
                    failure(
                        class,
                        ArtifactReplayFailureStage::CompanionFacts,
                        Some(path),
                        cost,
                        phase_remaining_source.saturating_sub(history_source_used),
                        phase_remaining_state.saturating_sub(retained_history_state),
                    )
                })?;
                continue;
            }
            1..4 => {
                retain_skip(
                    &mut skips,
                    &mut cost,
                    path,
                    ArtifactReplaySkipKind::PartialCreationCompanions,
                    &mut retained_history_state,
                    phase_remaining_state,
                )
                .map_err(|class| {
                    failure(
                        class,
                        ArtifactReplayFailureStage::CompanionFacts,
                        Some(path),
                        cost,
                        phase_remaining_source.saturating_sub(history_source_used),
                        phase_remaining_state.saturating_sub(retained_history_state),
                    )
                })?;
                continue;
            }
            4 => {}
            _ => unreachable!(),
        }

        let schema_gate = match record_values_by_path.get(path.as_str()).copied() {
            Some(Ok(value)) => Some(
                value.get("$schema").and_then(serde_json::Value::as_str)
                    == Some(NATIVE_ARTIFACT_SCHEMA),
            ),
            Some(Err(())) => {
                return Err(failure(
                    ArtifactReplayFailureClass::Source,
                    ArtifactReplayFailureStage::CurrentRecord,
                    Some(path),
                    cost,
                    phase_remaining_source.saturating_sub(history_source_used),
                    phase_remaining_state.saturating_sub(retained_history_state),
                ));
            }
            None => {
                let remaining_state = phase_remaining_state
                    .checked_sub(retained_history_state)
                    .ok_or_else(|| {
                        failure(
                            ArtifactReplayFailureClass::Budget,
                            ArtifactReplayFailureStage::CurrentRecord,
                            Some(path),
                            cost,
                            phase_remaining_source.saturating_sub(history_source_used),
                            0,
                        )
                    })?;
                let remaining_source = phase_remaining_source
                    .checked_sub(history_source_used)
                    .ok_or_else(|| {
                        failure(
                            ArtifactReplayFailureClass::Budget,
                            ArtifactReplayFailureStage::CurrentRecord,
                            Some(path),
                            cost,
                            0,
                            remaining_state,
                        )
                    })?;
                let fallback_limits = ItemLimits {
                    max_member_bytes: history_limits.max_member_bytes,
                    max_total_bytes: remaining_source,
                    max_state_bytes: remaining_state,
                    max_issues: history_limits.max_issues,
                    deadline,
                };
                let mut current_read = history_source_used;
                let value = fallback_record_value(
                    captured.cut(),
                    current_paths,
                    path,
                    revision,
                    fallback_limits,
                    retained_history_state,
                    &mut current_read,
                    &mut cost,
                    cancelled,
                )
                .map_err(|class| {
                    failure(
                        class,
                        ArtifactReplayFailureStage::CurrentRecord,
                        Some(path),
                        cost,
                        phase_remaining_source.saturating_sub(history_source_used),
                        phase_remaining_state.saturating_sub(retained_history_state),
                    )
                })?;
                history_source_used = current_read;
                value.map(|value| {
                    value.get("$schema").and_then(serde_json::Value::as_str)
                        == Some(NATIVE_ARTIFACT_SCHEMA)
                })
            }
        };
        match schema_gate {
            None => {
                retain_skip(
                    &mut skips,
                    &mut cost,
                    path,
                    ArtifactReplaySkipKind::CurrentJsonInvalid,
                    &mut retained_history_state,
                    phase_remaining_state,
                )
                .map_err(|class| {
                    failure(
                        class,
                        ArtifactReplayFailureStage::CurrentRecord,
                        Some(path),
                        cost,
                        phase_remaining_source.saturating_sub(history_source_used),
                        phase_remaining_state.saturating_sub(retained_history_state),
                    )
                })?;
                continue;
            }
            Some(false) => {
                retain_skip(
                    &mut skips,
                    &mut cost,
                    path,
                    ArtifactReplaySkipKind::CurrentSchemaDrift,
                    &mut retained_history_state,
                    phase_remaining_state,
                )
                .map_err(|class| {
                    failure(
                        class,
                        ArtifactReplayFailureStage::CurrentRecord,
                        Some(path),
                        cost,
                        phase_remaining_source.saturating_sub(history_source_used),
                        phase_remaining_state.saturating_sub(retained_history_state),
                    )
                })?;
                continue;
            }
            Some(true) => {}
        }

        let relative = RelativePath::parse(path).map_err(|_| {
            failure(
                ArtifactReplayFailureClass::Source,
                ArtifactReplayFailureStage::CurrentRecord,
                Some(path),
                cost,
                phase_remaining_source.saturating_sub(history_source_used),
                phase_remaining_state.saturating_sub(retained_history_state),
            )
        })?;
        let metadata = captured.cut().current().member(&relative).ok_or_else(|| {
            failure(
                ArtifactReplayFailureClass::Source,
                ArtifactReplayFailureStage::CurrentRecord,
                Some(path),
                cost,
                phase_remaining_source.saturating_sub(history_source_used),
                phase_remaining_state.saturating_sub(retained_history_state),
            )
        })?;
        if invalid_schema_proofs.proves_invalid(
            path,
            revision,
            membership,
            metadata.sha256,
            metadata.size_bytes,
        ) {
            retain_skip(
                &mut skips,
                &mut cost,
                path,
                ArtifactReplaySkipKind::CurrentRecordSchemaInvalid,
                &mut retained_history_state,
                phase_remaining_state,
            )
            .map_err(|class| {
                failure(
                    class,
                    ArtifactReplayFailureStage::CurrentRecord,
                    Some(path),
                    cost,
                    phase_remaining_source.saturating_sub(history_source_used),
                    phase_remaining_state.saturating_sub(retained_history_state),
                )
            })?;
            continue;
        }

        let prior_source = history_source_used;
        let prior_state = retained_history_state;
        let remaining_source = phase_remaining_source
            .checked_sub(prior_source)
            .ok_or_else(|| {
                failure(
                    ArtifactReplayFailureClass::Budget,
                    ArtifactReplayFailureStage::NativeHistory,
                    Some(path),
                    cost,
                    0,
                    phase_remaining_state.saturating_sub(prior_state),
                )
            })?;
        let remaining_state = phase_remaining_state
            .checked_sub(prior_state)
            .ok_or_else(|| {
                failure(
                    ArtifactReplayFailureClass::Budget,
                    ArtifactReplayFailureStage::NativeHistory,
                    Some(path),
                    cost,
                    remaining_source,
                    0,
                )
            })?;
        let native_limits = ItemLimits {
            max_member_bytes: history_limits.max_member_bytes,
            max_total_bytes: remaining_source,
            max_state_bytes: remaining_state,
            max_issues: history_limits.max_issues,
            deadline,
        };
        let observation =
            selected_record_history_from_cut(captured.cut(), path, native_limits, cancelled)
                .map_err(|refusal| {
                    failure(
                        refusal_class(&refusal),
                        ArtifactReplayFailureStage::NativeHistory,
                        Some(path),
                        cost,
                        remaining_source,
                        remaining_state,
                    )
                })?;
        history_source_used = history_source_used
            .checked_add(observation.bytes_read())
            .ok_or_else(|| {
                failure(
                    ArtifactReplayFailureClass::Budget,
                    ArtifactReplayFailureStage::NativeHistory,
                    Some(path),
                    cost,
                    remaining_source,
                    remaining_state,
                )
            })?;
        cost.native_history_source_read_bytes = cost
            .native_history_source_read_bytes
            .checked_add(observation.bytes_read())
            .ok_or_else(|| {
                failure(
                    ArtifactReplayFailureClass::Budget,
                    ArtifactReplayFailureStage::NativeHistory,
                    Some(path),
                    cost,
                    0,
                    remaining_state,
                )
            })?;
        cost.native_history_returned_state_bytes = cost
            .native_history_returned_state_bytes
            .checked_add(observation.returned_state_bytes())
            .ok_or_else(|| {
                failure(
                    ArtifactReplayFailureClass::Budget,
                    ArtifactReplayFailureStage::NativeHistory,
                    Some(path),
                    cost,
                    phase_remaining_source.saturating_sub(history_source_used),
                    0,
                )
            })?;
        if observation.source_revision() != revision
            || observation.current_membership() != membership
        {
            return Err(failure(
                ArtifactReplayFailureClass::Source,
                ArtifactReplayFailureStage::NativeHistory,
                Some(path),
                cost,
                remaining_source.saturating_sub(observation.bytes_read()),
                remaining_state.saturating_sub(observation.returned_state_bytes()),
            ));
        }
        let next_retained_history_state = retained_history_state
            .checked_add(observation.returned_state_bytes())
            .and_then(|n| n.checked_add(map_state(path)?))
            .ok_or_else(|| {
                failure(
                    ArtifactReplayFailureClass::Budget,
                    ArtifactReplayFailureStage::NativeHistory,
                    Some(path),
                    cost,
                    phase_remaining_source.saturating_sub(history_source_used),
                    0,
                )
            })?;
        if next_retained_history_state > phase_remaining_state {
            return Err(failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::NativeHistory,
                Some(path),
                cost,
                phase_remaining_source.saturating_sub(history_source_used),
                phase_remaining_state.saturating_sub(retained_history_state),
            ));
        }
        retained_history_state = next_retained_history_state;
        let current_map_state = map_state(path).ok_or_else(|| {
            failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::NativeHistory,
                Some(path),
                cost,
                phase_remaining_source.saturating_sub(history_source_used),
                0,
            )
        })?;
        cost.native_history_map_state_bytes = cost
            .native_history_map_state_bytes
            .checked_add(current_map_state)
            .ok_or_else(|| {
                failure(
                    ArtifactReplayFailureClass::Budget,
                    ArtifactReplayFailureStage::NativeHistory,
                    Some(path),
                    cost,
                    phase_remaining_source.saturating_sub(history_source_used),
                    0,
                )
            })?;
        histories.insert(path.clone(), observation);
    }

    drop(record_values_by_path);
    retained_history_state = retained_history_state
        .checked_sub(record_index_state)
        .ok_or_else(|| {
            failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::CurrentRecord,
                None,
                cost,
                phase_remaining_source.saturating_sub(history_source_used),
                0,
            )
        })?;

    let mut has_corrections = false;
    for (index, observation) in histories.values().enumerate() {
        if index % 128 == 0 {
            active(deadline, cancelled).map_err(|class| {
                failure(
                    class,
                    ArtifactReplayFailureStage::CorrectionReplay,
                    None,
                    cost,
                    phase_remaining_source.saturating_sub(history_source_used),
                    phase_remaining_state.saturating_sub(retained_history_state),
                )
            })?;
        }
        has_corrections |= observation.history_receipt_count() > 0;
    }
    if !has_corrections {
        return Ok(ArtifactReplayEvidence {
            histories,
            replays,
            invalid_schema_proofs,
            skips,
            cost,
        });
    }

    let mut reserve = ReplayReservation {
        publication_state: 0,
        returned_state: 0,
        input_copy_state: 0,
        map_state: 0,
        worker_peak_state: 0,
    };
    for (path, history) in &histories {
        if history.history_receipt_count() == 0 {
            continue;
        }
        active(deadline, cancelled).map_err(|class| {
            failure(
                class,
                ArtifactReplayFailureStage::CorrectionReplay,
                Some(path),
                cost,
                phase_remaining_source.saturating_sub(history_source_used),
                phase_remaining_state.saturating_sub(retained_history_state),
            )
        })?;
        let estimate = replay_reservation(
            captured.cut(),
            current_paths,
            history,
            source_root,
            path,
            deadline,
            cancelled,
        )
        .map_err(|class| {
            failure(
                class,
                ArtifactReplayFailureStage::CorrectionReplay,
                Some(path),
                cost,
                phase_remaining_source.saturating_sub(history_source_used),
                phase_remaining_state.saturating_sub(retained_history_state),
            )
        })?;
        reserve.publication_state = reserve
            .publication_state
            .checked_add(estimate.publication_state)
            .ok_or_else(|| {
                failure(
                    ArtifactReplayFailureClass::Budget,
                    ArtifactReplayFailureStage::CorrectionReplay,
                    Some(path),
                    cost,
                    0,
                    0,
                )
            })?;
        reserve.returned_state = reserve
            .returned_state
            .checked_add(estimate.returned_state)
            .ok_or_else(|| {
                failure(
                    ArtifactReplayFailureClass::Budget,
                    ArtifactReplayFailureStage::CorrectionReplay,
                    Some(path),
                    cost,
                    0,
                    0,
                )
            })?;
        reserve.input_copy_state = reserve
            .input_copy_state
            .checked_add(estimate.input_copy_state)
            .ok_or_else(|| {
                failure(
                    ArtifactReplayFailureClass::Budget,
                    ArtifactReplayFailureStage::CorrectionReplay,
                    Some(path),
                    cost,
                    0,
                    0,
                )
            })?;
        reserve.map_state = reserve
            .map_state
            .checked_add(estimate.map_state)
            .ok_or_else(|| {
                failure(
                    ArtifactReplayFailureClass::Budget,
                    ArtifactReplayFailureStage::CorrectionReplay,
                    Some(path),
                    cost,
                    0,
                    0,
                )
            })?;
        reserve.worker_peak_state = reserve.worker_peak_state.max(estimate.worker_peak_state);
    }
    let history_map_state = retained_history_state;
    let read_only_source_used = history_source_used;
    let read_only_remaining_source = readonly_limits
        .remaining_source_bytes
        .checked_sub(read_only_source_used)
        .ok_or_else(|| {
            failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::ReadonlyCollection,
                None,
                cost,
                0,
                0,
            )
        })?;
    let read_only_remaining_record_bytes = readonly_limits
        .max_record_bytes
        .checked_sub(read_only_source_used)
        .ok_or_else(|| {
            failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::ReadonlyCollection,
                None,
                cost,
                0,
                0,
            )
        })?;
    let reserved_state = readonly_limits
        .state_already_reserved
        .checked_add(history_map_state)
        .and_then(|n| n.checked_add(reserve.publication_state))
        .and_then(|n| n.checked_add(reserve.returned_state))
        .and_then(|n| n.checked_add(reserve.input_copy_state))
        .and_then(|n| n.checked_add(reserve.map_state))
        .and_then(|n| n.checked_add(reserve.map_state))
        .and_then(|n| n.checked_add(reserve.worker_peak_state))
        .ok_or_else(|| {
            failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::ReadonlyCollection,
                None,
                cost,
                0,
                0,
            )
        })?;
    let reserved_replay_failure_state = reserve
        .publication_state
        .checked_add(reserve.returned_state)
        .and_then(|n| n.checked_add(reserve.input_copy_state))
        .and_then(|n| n.checked_add(reserve.map_state))
        .and_then(|n| n.checked_add(reserve.worker_peak_state))
        .ok_or_else(|| {
            failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::ReadonlyCollection,
                None,
                cost,
                0,
                0,
            )
        })?;
    if reserved_state > readonly_limits.max_state_bytes {
        return Err(failure(
            ArtifactReplayFailureClass::Budget,
            ArtifactReplayFailureStage::ReadonlyCollection,
            None,
            cost,
            read_only_remaining_source,
            0,
        ));
    }
    if current_paths.len() > readonly_limits.max_current_members
        || readonly_limits.max_files == 0
        || readonly_limits.max_read_calls == 0
        || readonly_limits.max_directory_entries == 0
        || read_only_remaining_record_bytes == 0
        || read_only_remaining_source == 0
        || readonly_limits.max_member_bytes == 0
    {
        return Err(failure(
            ArtifactReplayFailureClass::Budget,
            ArtifactReplayFailureStage::ReadonlyCollection,
            None,
            cost,
            read_only_remaining_source,
            readonly_limits
                .max_state_bytes
                .saturating_sub(reserved_state),
        ));
    }
    cost.replay_input_copy_state_bytes = reserve.input_copy_state;
    let transport_limits = CapturedReadonlyRecordLimits {
        max_current_members: readonly_limits.max_current_members,
        max_files: readonly_limits.max_files,
        max_read_calls: readonly_limits.max_read_calls,
        max_directory_entries: readonly_limits.max_directory_entries,
        max_record_bytes: read_only_remaining_record_bytes,
        remaining_source_bytes: read_only_remaining_source,
        max_member_bytes: readonly_limits.max_member_bytes,
        max_state_bytes: readonly_limits.max_state_bytes,
        state_already_reserved: reserved_state,
    };
    let mut transport = CapturedReadonlyRecordFiles::new(
        captured.cut(),
        revision,
        current_paths,
        transport_limits,
        deadline,
        cancelled,
    )
    .map_err(|error| {
        failure(
            command_class(&error),
            ArtifactReplayFailureStage::ReadonlyCollection,
            None,
            cost,
            read_only_remaining_source,
            readonly_limits
                .max_state_bytes
                .saturating_sub(reserved_state),
        )
    })?;

    for (path, history) in &histories {
        if history.history_receipt_count() == 0 {
            continue;
        }
        active(deadline, cancelled).map_err(|class| {
            cost.readonly = transport.cost();
            failure(
                class,
                ArtifactReplayFailureStage::CorrectionReplay,
                Some(path),
                cost,
                read_only_remaining_source.saturating_sub(cost.readonly.read_bytes),
                reserved_replay_failure_state,
            )
        })?;
        let estimate = replay_reservation(
            captured.cut(),
            current_paths,
            history,
            source_root,
            path,
            deadline,
            cancelled,
        )
        .map_err(|class| {
            failure(
                class,
                ArtifactReplayFailureStage::CorrectionReplay,
                Some(path),
                cost,
                read_only_remaining_source.saturating_sub(transport.cost().read_bytes),
                reserved_replay_failure_state,
            )
        })?;
        let observation = replay_one(
            captured.cut(),
            current_paths,
            readonly_limits.max_member_bytes,
            &histories[path],
            source_root,
            effective_uid,
            worker,
            &mut transport,
            deadline,
            cancelled,
        )
        .map_err(|class| {
            cost.readonly = transport.cost();
            failure(
                class,
                ArtifactReplayFailureStage::CorrectionReplay,
                Some(path),
                cost,
                read_only_remaining_source.saturating_sub(cost.readonly.read_bytes),
                reserved_replay_failure_state,
            )
        })?;
        if observation.source_revision() != revision
            || observation.current_membership() != membership
            || observation.source_path() != path
            || observation.record_id() != histories[path].identity()
        {
            cost.readonly = transport.cost();
            return Err(failure(
                ArtifactReplayFailureClass::Source,
                ArtifactReplayFailureStage::CorrectionReplay,
                Some(path),
                cost,
                read_only_remaining_source.saturating_sub(cost.readonly.read_bytes),
                reserved_replay_failure_state,
            ));
        }
        if observation.publication_state_bytes() > estimate.publication_state
            || observation.returned_state_bytes() > estimate.returned_state
        {
            cost.readonly = transport.cost();
            return Err(failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::CorrectionReplay,
                Some(path),
                cost,
                read_only_remaining_source.saturating_sub(cost.readonly.read_bytes),
                reserved_replay_failure_state,
            ));
        }
        cost.replay_publication_state_bytes = cost
            .replay_publication_state_bytes
            .checked_add(observation.publication_state_bytes())
            .ok_or_else(|| {
                failure(
                    ArtifactReplayFailureClass::Budget,
                    ArtifactReplayFailureStage::CorrectionReplay,
                    Some(path),
                    cost,
                    0,
                    0,
                )
            })?;
        cost.replay_returned_state_bytes = cost
            .replay_returned_state_bytes
            .checked_add(observation.returned_state_bytes())
            .ok_or_else(|| {
                failure(
                    ArtifactReplayFailureClass::Budget,
                    ArtifactReplayFailureStage::CorrectionReplay,
                    Some(path),
                    cost,
                    0,
                    0,
                )
            })?;
        cost.replay_map_state_bytes = cost
            .replay_map_state_bytes
            .checked_add(map_state(path).ok_or_else(|| {
                failure(
                    ArtifactReplayFailureClass::Budget,
                    ArtifactReplayFailureStage::CorrectionReplay,
                    Some(path),
                    cost,
                    0,
                    0,
                )
            })?)
            .ok_or_else(|| {
                failure(
                    ArtifactReplayFailureClass::Budget,
                    ArtifactReplayFailureStage::CorrectionReplay,
                    Some(path),
                    cost,
                    0,
                    0,
                )
            })?;
        replays.insert(path.clone(), observation);
    }
    cost.readonly = transport.cost();
    cost.replay_map_state_bytes = cost
        .replay_map_state_bytes
        .checked_add(cost.replay_map_state_bytes)
        .ok_or_else(|| {
            failure(
                ArtifactReplayFailureClass::Budget,
                ArtifactReplayFailureStage::CorrectionReplay,
                None,
                cost,
                0,
                reserved_replay_failure_state,
            )
        })?;
    cost.peak_temporary_state_bytes = cost
        .peak_temporary_state_bytes
        .max(cost.readonly.peak_state_upper_bound_bytes);
    Ok(ArtifactReplayEvidence {
        histories,
        replays,
        invalid_schema_proofs,
        skips,
        cost,
    })
}
