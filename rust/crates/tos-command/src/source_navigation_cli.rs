//! Protected selected-owner bridge for the PostgreSQL source-navigation QRY.
//!
//! The owner configuration is the one source of the selected CMD binding,
//! retained V2 proof/model, explicit disclosure receipt, and private store
//! limits. It is reopened and hash-bound for every request; no producer build
//! or default policy is manufactured here.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};

use serde::Deserialize;
use tos_foundation::Digest256;
use tos_query::{
    AbortProbe, Budget, DisclosableSourceDescend, QueryError, SourceDescendRequest, source_descend,
};
use tos_segment_store::{AuthenticatedTreeLimitsV1, SegmentLimits, SegmentStore};
use tos_validation::{
    executor::{ExactWorkerIdentity, ExecutorBudget},
    source_cut::CutWorkerLimits,
};

use crate::{
    DurableNavigationEventLimits, DurableProjectionProfile,
    source_managed_selection::{
        ManagedAgentSelectedParent, ManagedSelectionError, ManagedSelectionResult,
        SourceNavigationCurrentLeaseV1,
    },
};

const OWNER_SCHEMA: &str = "tos_local_source_navigation_owner_v1";
const SELECTION_SCHEMA: &str = "tos_local_pg_source_navigation_selection_v1";
const MAX_OWNER_CONFIG: usize = 1_048_576;

#[derive(Clone, Debug)]
pub struct SelectedSourceNavigation {
    owner_configuration: PathBuf,
}

impl SelectedSourceNavigation {
    /// Bind an explicit absolute protected source-navigation owner path.
    /// Contents and current durable policy are rechecked for each request.
    pub fn new(owner_configuration: &Path) -> Result<Self, &'static str> {
        if !owner_configuration.is_absolute() {
            return Err("source navigation selection requires an absolute path");
        }
        Ok(Self {
            owner_configuration: owner_configuration.to_owned(),
        })
    }

    pub fn owner_configuration(&self) -> &Path {
        &self.owner_configuration
    }
}

#[derive(Debug)]
pub enum SourceNavigationSelectionError {
    Invalid(&'static str),
    Managed(ManagedSelectionError),
}

/// A published read may be ready or explicitly pending. The callback only
/// runs for a ready generation and completes while the owner/policy/PG fences
/// remain held.
pub enum SelectedSourceNavigationRead<T> {
    Ready(T),
    NotReady,
}

fn read_selected_owner_configuration(
    path: &Path,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<
    (
        crate::source_creation_store::CreationFilesystem,
        OwnerConfiguration,
        Digest256,
    ),
    SourceNavigationSelectionError,
> {
    let (filesystem, raw_owner) =
        crate::source_creation_store::CreationFilesystem::select_protected_native_owner_bounded(
            path,
            MAX_OWNER_CONFIG,
            deadline,
            cancelled,
        )
        .map_err(|_| {
            SourceNavigationSelectionError::Invalid("protected source navigation owner refused")
        })?;
    let parsed = crate::source_command::parse(&raw_owner)
        .map_err(|_| SourceNavigationSelectionError::Invalid("source navigation owner JSON"))?;
    let canonical = crate::source_command::canonical(&parsed)
        .map_err(|_| SourceNavigationSelectionError::Invalid("source navigation owner encoding"))?;
    let owner: OwnerConfiguration = serde_json::from_slice(&canonical)
        .map_err(|_| SourceNavigationSelectionError::Invalid("source navigation owner fields"))?;
    if owner.schema_version != OWNER_SCHEMA
        || owner.uid != u64::from(rustix::process::geteuid().as_raw())
        || owner.source_root.is_empty()
        || owner.expires_at.is_empty()
    {
        return Err(SourceNavigationSelectionError::Invalid(
            "source navigation owner scope",
        ));
    }
    let owner_digest = crate::source_command::record_digest(&parsed)
        .map_err(|_| SourceNavigationSelectionError::Invalid("source navigation owner digest"))?;
    Ok((filesystem, owner, owner_digest))
}

/// Run one bounded producer callback under the same protected source-owner
/// configuration and corpus mutex used by source writers and navigation reads.
/// The callback receives only parsed, validated settings and cannot retain the
/// owner fence beyond its call.
pub(crate) fn with_selected_navigation_producer_owner<T>(
    selected: &SelectedSourceNavigation,
    deadline: Instant,
    cancelled: &AtomicBool,
    consume: impl FnOnce(
        &crate::source_creation_store::CreationFilesystem,
        &crate::source_creation_store::CreationOwnerFence<'_>,
        &NavigationProducerOwnerSelection<'_>,
    ) -> crate::DurableResult<T>,
) -> Result<T, SourceNavigationSelectionError> {
    let (filesystem, owner, owner_configuration_sha256) =
        read_selected_owner_configuration(&selected.owner_configuration, deadline, cancelled)?;
    let view = navigation_producer_owner_selection(&owner, owner_configuration_sha256)?;
    let owner_fence = filesystem
        .hold_source_navigation_owner(deadline, cancelled)
        .map_err(|error| {
            SourceNavigationSelectionError::Managed(ManagedSelectionError::Source(error))
        })?;
    let result = consume(&filesystem, &owner_fence, &view).map_err(|error| {
        SourceNavigationSelectionError::Managed(ManagedSelectionError::Durable(error))
    })?;
    owner_fence
        .verify_current(deadline, cancelled)
        .map_err(|error| {
            SourceNavigationSelectionError::Managed(ManagedSelectionError::Source(error))
        })?;
    Ok(result)
}

/// Run a policy-owner mutation under the protected source-navigation owner
/// configuration and filesystem owner fence. This deliberately does not
/// require the optional event-producer configuration, open the model, or
/// start a PostgreSQL transaction; the callback owns the single policy
/// transaction and its database fences.
pub(crate) fn with_selected_navigation_policy_owner<T>(
    selected: &SelectedSourceNavigation,
    deadline: Instant,
    cancelled: &AtomicBool,
    consume: impl FnOnce(
        &crate::source_creation_store::CreationFilesystem,
        &crate::source_creation_store::CreationOwnerFence<'_>,
        &NavigationPolicyOwnerViewV1<'_>,
    ) -> crate::DurableResult<T>,
) -> Result<T, SourceNavigationSelectionError> {
    if Instant::now() >= deadline || cancelled.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(SourceNavigationSelectionError::Invalid(
            "source navigation policy deadline exhausted",
        ));
    }
    let (filesystem, owner, owner_configuration_sha256) =
        read_selected_owner_configuration(&selected.owner_configuration, deadline, cancelled)?;
    let selection = &owner.source_navigation;
    let source_contract = validate_navigation_selection_scope(selection)?;
    selection
        .source_proof_v2
        .validate()
        .map_err(|_| SourceNavigationSelectionError::Invalid("source navigation V2 proof"))?;
    let anchor = &selection.source_proof_v2.generation;
    if anchor.domain != selection.domain {
        return Err(SourceNavigationSelectionError::Invalid(
            "source navigation proof domain",
        ));
    }
    let anchor_digest = |value: &str| {
        Digest256::from_hex(value)
            .map_err(|_| SourceNavigationSelectionError::Invalid("source navigation proof digest"))
    };
    let bootstrap_source_revision = anchor_digest(&anchor.bootstrap_source_revision)?;
    let source_definition_digest = anchor_digest(&anchor.definition_sha256)?;
    let bootstrap_membership_digest = anchor_digest(&anchor.bootstrap_membership_sha256)?;
    let domain_digest = anchor_digest(&anchor.domain_sha256)?;
    let (policy_receipt, policy_receipt_sha256) =
        crate::source_navigation_policy::parse_policy_object(&selection.policy_receipt).map_err(
            |_| SourceNavigationSelectionError::Invalid("source navigation policy receipt"),
        )?;
    let view = NavigationPolicyOwnerViewV1 {
        source_root: filesystem.protected_root_path(),
        domain: &selection.domain,
        source_contract,
        bootstrap_source_revision,
        source_epoch: anchor.epoch,
        source_definition_digest,
        bootstrap_membership_digest,
        bootstrap_membership_count: anchor.bootstrap_members,
        domain_digest,
        database_oid: anchor.database_oid,
        anchor_through_sequence: anchor.through_commit_seq,
        rule_version: selection.rule_version,
        rights_version: selection.rights_version,
        job_id: &selection.job_id,
        job_fence: selection.job_fence,
        consumer_id: &selection.consumer_id,
        profile_id: &selection.profile_id,
        profile_version: selection.profile_version,
        policy_receipt,
        policy_receipt_sha256,
        owner_configuration_sha256,
    };
    let owner_fence = filesystem
        .hold_source_navigation_owner(deadline, cancelled)
        .map_err(|error| {
            SourceNavigationSelectionError::Managed(ManagedSelectionError::Source(error))
        })?;
    let result = consume(&filesystem, &owner_fence, &view).map_err(|error| {
        SourceNavigationSelectionError::Managed(ManagedSelectionError::Durable(error))
    })?;
    owner_fence
        .verify_current(deadline, cancelled)
        .map_err(|error| {
            SourceNavigationSelectionError::Managed(ManagedSelectionError::Source(error))
        })?;
    Ok(result)
}

struct NavigationOwnerContext {
    filesystem: crate::source_creation_store::CreationFilesystem,
    owner: OwnerConfiguration,
    owner_configuration_sha256: Digest256,
    store: SegmentStore,
    coordinator: crate::DurablePgCoordinator,
    generation: crate::source_current_cut::ManagedCurrentSourceGeneration,
}

fn open_navigation_owner_context(
    selected: &SelectedSourceNavigation,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<NavigationOwnerContext, SourceNavigationSelectionError> {
    if Instant::now() >= deadline || cancelled.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(SourceNavigationSelectionError::Invalid(
            "source navigation producer deadline exhausted",
        ));
    }
    let (filesystem, owner, owner_configuration_sha256) =
        read_selected_owner_configuration(&selected.owner_configuration, deadline, cancelled)?;
    let selection = &owner.source_navigation;
    validate_navigation_selection_scope(selection)?;
    let view = navigation_producer_owner_selection(&owner, owner_configuration_sha256)?;
    selection
        .source_proof_v2
        .validate()
        .map_err(|_| SourceNavigationSelectionError::Invalid("source navigation V2 proof"))?;
    let store = SegmentStore::open_existing(&view.store_root, selection.store_limits.into_limits())
        .map_err(|_| SourceNavigationSelectionError::Invalid("source navigation store refused"))?;
    let client = crate::source_projection_cli::connect_selected_source_pg16()
        .map_err(|_| SourceNavigationSelectionError::Invalid("selected PostgreSQL unavailable"))?;
    let mut coordinator = crate::DurablePgCoordinator::from_configured_client(client);
    let generation = coordinator
        .reopen_persisted_addressed_navigation_anchor(
            &store,
            &selection.source_proof_v2,
            selection.tree_limits.into_limits(),
            deadline,
            cancelled,
        )
        .map_err(|error| {
            SourceNavigationSelectionError::Managed(ManagedSelectionError::Durable(error))
        })?;
    Ok(NavigationOwnerContext {
        filesystem,
        owner,
        owner_configuration_sha256,
        store,
        coordinator,
        generation,
    })
}

/// Ordinary native work uses the admitted PG publication and bounded immutable
/// source descriptor. No SQLite model or whole-corpus capability is reopened.
/// Current source, rights, audit, job and protected-owner holds remain identical
/// to initial bootstrap and cover the complete callback/delivery.
pub(crate) fn with_selected_navigation_current<T>(
    selected: &SelectedSourceNavigation,
    deadline: Instant,
    cancelled: &AtomicBool,
    consume: impl for<'lease, 'tx> FnOnce(
        &'lease SourceNavigationCurrentLeaseV1<'lease>,
        &crate::source_current_cut::ManagedCurrentSourceGeneration,
        &tos_compiler::managed_source::ManagedSourceProofV2,
        &tos_compiler::KnowledgeSelectedExpectation,
        &NavigationProducerHeldOwnerIdentity,
        &mut postgres::Transaction<'tx>,
        &NavigationProducerOwnerSelection<'_>,
        &SegmentStore,
    ) -> ManagedSelectionResult<T>,
) -> Result<T, SourceNavigationSelectionError> {
    let NavigationOwnerContext {
        filesystem,
        owner,
        owner_configuration_sha256,
        store,
        mut coordinator,
        generation,
    } = open_navigation_owner_context(selected, deadline, cancelled)?;
    let selection = &owner.source_navigation;
    let view = navigation_producer_owner_selection(&owner, owner_configuration_sha256)?;
    coordinator
        .with_current_source_navigation_lease(
            &store,
            &generation,
            &selection.source_proof_v2,
            &filesystem,
            view.source_contract,
            selection.rule_version,
            selection.rights_version,
            &selection.job_id,
            selection.job_fence,
            deadline,
            cancelled,
            |lease, tx| {
                let facts = NavigationProducerHeldOwnerIdentity::from_selected(
                    &selection.source_proof_v2,
                    &selection.expectation,
                    lease,
                    &view,
                    generation.digest(),
                    generation.member_count(),
                )
                .map_err(|_| crate::DurableError::Conflict("source navigation producer binding"))?;
                Ok(consume(
                    lease,
                    &generation,
                    &selection.source_proof_v2,
                    &selection.expectation,
                    &facts,
                    tx,
                    &view,
                    &store,
                ))
            },
        )
        .map_err(|error| {
            SourceNavigationSelectionError::Managed(ManagedSelectionError::Durable(error))
        })?
        .map_err(SourceNavigationSelectionError::Managed)
}

/// Admit the bootstrap against the persisted selected model and current PG
/// source owner. Unlike the lower-level owner-config reader, this callback is
/// entered only after the retained V2 descriptor/model have been reopened and
/// the source, rights, job, audit, and owner fences are held in one RR read.
pub(crate) fn with_selected_navigation_anchor_current<T>(
    selected: &SelectedSourceNavigation,
    deadline: Instant,
    cancelled: &AtomicBool,
    consume: impl for<'model, 'lease, 'tx> FnOnce(
        &'model tos_compiler::VerifiedKnowledgeModel<'static>,
        &'lease SourceNavigationCurrentLeaseV1<'lease>,
        &ManagedAgentSelectedParent<tos_compiler::managed_source::ManagedSourceProofV2>,
        &tos_compiler::managed_source::ManagedSourceProofV2,
        &tos_compiler::KnowledgeSelectedExpectation,
        &NavigationProducerHeldOwnerIdentity,
        &mut postgres::Transaction<'tx>,
        &NavigationProducerOwnerSelection<'_>,
        &SegmentStore,
    ) -> ManagedSelectionResult<T>,
) -> Result<T, SourceNavigationSelectionError> {
    let NavigationOwnerContext {
        filesystem,
        owner,
        owner_configuration_sha256,
        store,
        mut coordinator,
        generation,
    } = open_navigation_owner_context(selected, deadline, cancelled)?;
    let selection = &owner.source_navigation;
    let contract = validate_navigation_selection_scope(selection)?;
    let view = navigation_producer_owner_selection(&owner, owner_configuration_sha256)?;
    selection
        .cold_limits
        .validate()
        .map_err(|_| SourceNavigationSelectionError::Invalid("source navigation cold limits"))?;
    selection
        .process_limits
        .verify_current()
        .map_err(|_| SourceNavigationSelectionError::Invalid("source navigation process limits"))?;
    let model_path = crate::source_text_owner::normalized_absolute(&selection.model_path)
        .map_err(|_| SourceNavigationSelectionError::Invalid("source navigation model path"))?;
    let parent = ManagedAgentSelectedParent::<
        tos_compiler::managed_source::ManagedSourceProofV2,
    >::open_persisted_navigation_selection(
        generation,
        selection.source_proof_v2.clone(),
        &model_path,
        selection.expectation.clone(),
        selection.process_limits,
        selection.cold_limits,
    )
    .map_err(SourceNavigationSelectionError::Managed)?;
    parent
        .with_navigation_anchor_current_and_tx(
            &mut coordinator,
            &store,
            &filesystem,
            contract,
            selection.rule_version,
            selection.rights_version,
            &selection.job_id,
            selection.job_fence,
            deadline,
            cancelled,
            |model, lease, tx| {
                let facts = NavigationProducerHeldOwnerIdentity::from_selected(
                    parent.source_proof(),
                    parent.selection_expectation(),
                    lease,
                    &view,
                    parent.generation().digest(),
                    parent.generation().member_count(),
                )
                .map_err(|_| {
                    ManagedSelectionError::Source(
                        crate::source_command::SourceCommandError::Invalid(
                            "source navigation producer binding",
                        ),
                    )
                })?;
                consume(
                    model,
                    lease,
                    &parent,
                    parent.source_proof(),
                    parent.selection_expectation(),
                    &facts,
                    tx,
                    &view,
                    &store,
                )
            },
        )
        .map_err(SourceNavigationSelectionError::Managed)
}

fn navigation_producer_owner_selection(
    owner: &OwnerConfiguration,
    owner_configuration_sha256: Digest256,
) -> Result<NavigationProducerOwnerSelection<'_>, SourceNavigationSelectionError> {
    let selection = &owner.source_navigation;
    let contract = validate_navigation_selection_scope(selection)?;
    let producer = selection
        .producer
        .as_ref()
        .ok_or(SourceNavigationSelectionError::Invalid(
            "source navigation producer selection absent",
        ))?;
    validate_producer_selection(producer)?;
    let aggregate = producer.aggregate_limits;
    let profile = DurableProjectionProfile {
        id: selection.profile_id.clone(),
        version: selection.profile_version,
        source_contract: contract,
    };
    let store_root = crate::source_text_owner::normalized_absolute(&selection.store_root)
        .map_err(|_| SourceNavigationSelectionError::Invalid("source navigation store root"))?;
    Ok(NavigationProducerOwnerSelection {
        domain: &selection.domain,
        source_contract: contract,
        rule_version: selection.rule_version,
        rights_version: selection.rights_version,
        job_id: &selection.job_id,
        job_fence: selection.job_fence,
        consumer_id: &selection.consumer_id,
        policy_receipt_value: &selection.policy_receipt,
        profile,
        owner_configuration_sha256,
        store_root,
        store_limits: SegmentLimits {
            max_segment_bytes: selection.store_limits.max_segment_bytes,
            max_frame_bytes: selection.store_limits.max_frame_bytes,
            max_frames: selection.store_limits.max_frames,
            max_journal_bytes: selection.store_limits.max_journal_bytes,
        },
        event_path: &producer.event_path,
        requested_paths: &producer.requested_paths,
        schema_paths: &producer.schema_paths,
        schema_worker: ExactWorkerIdentity {
            absolute_path: crate::source_text_owner::normalized_absolute(
                &producer.schema_worker_path,
            )
            .map_err(|_| {
                SourceNavigationSelectionError::Invalid("source navigation schema worker path")
            })?,
            sha256: Digest256::from_hex(&producer.schema_worker_sha256).map_err(|_| {
                SourceNavigationSelectionError::Invalid("source navigation schema worker digest")
            })?,
        },
        schema_profile: tos_validation::FormatProfile::LegacyPythonObserved20260923,
        schema_budget: ExecutorBudget {
            execution_wall: std::time::Duration::from_millis(producer.schema_execution_wall_ms),
            // As in initial model preparation, the outer allowance cannot
            // enlarge the schema executor's hard child cleanup ceiling.
            cleanup_grace: std::time::Duration::from_millis(
                producer
                    .schema_cleanup_grace_ms
                    .min(ExecutorBudget::MAX_SCALAR_CLEANUP_GRACE_MILLIS),
            ),
            cpu_seconds: producer.schema_cpu_seconds,
            address_space_bytes: producer.schema_address_space_bytes,
        },
        schema_limits: CutWorkerLimits {
            max_receipts: producer.schema_max_receipts,
            max_receipt_bytes: producer.schema_max_receipt_bytes,
        },
        max_events: producer.max_events,
        event_limits: producer.event_limits.into_limits(),
        max_total_rows: aggregate.max_total_rows,
        max_total_encoded_bytes: aggregate.max_total_encoded_bytes,
        max_total_dependency_bytes: aggregate.max_total_dependency_bytes,
        max_total_work_units: aggregate.max_total_work_units,
    })
}

fn validate_producer_selection(
    producer: &NavigationProducerSelection,
) -> Result<(), SourceNavigationSelectionError> {
    if producer.schema_version != "tos_local_pg_source_navigation_producer_v1"
        || !(1..=256).contains(&producer.max_events)
        || producer.event_path.is_empty()
        || !tos_foundation::RelativePath::parse(&producer.event_path)
            .is_ok_and(|path| path.as_str() == producer.event_path)
        || producer.requested_paths.is_empty()
        || producer.requested_paths.len() > producer.event_limits.max_dependencies
        || !producer
            .requested_paths
            .windows(2)
            .all(|pair| pair[0] < pair[1])
        || producer
            .requested_paths
            .binary_search(&producer.event_path)
            .is_err()
    {
        return Err(SourceNavigationSelectionError::Invalid(
            "source navigation producer event selection",
        ));
    }
    for path in &producer.requested_paths {
        if !tos_foundation::RelativePath::parse(path).is_ok_and(|parsed| parsed.as_str() == path) {
            return Err(SourceNavigationSelectionError::Invalid(
                "source navigation producer dependency path",
            ));
        }
    }
    let event_limits = producer.event_limits;
    let aggregate = producer.aggregate_limits;
    if aggregate.max_total_rows == 0
        || aggregate.max_total_encoded_bytes == 0
        || aggregate.max_total_dependency_bytes == 0
        || aggregate.max_total_work_units == 0
        || event_limits.statement_timeout_ms == 0
        || event_limits.statement_timeout_ms > 300_000
        || producer.schema_paths.is_empty()
        || producer.schema_paths.len() > 512
        || !producer
            .schema_paths
            .windows(2)
            .all(|pair| pair[0] < pair[1])
        || producer.schema_worker_path.is_empty()
        || !Path::new(&producer.schema_worker_path).is_absolute()
        || Digest256::from_hex(&producer.schema_worker_sha256).is_err()
        || producer.schema_execution_wall_ms == 0
        || producer.schema_execution_wall_ms > 300_000
        || producer.schema_cleanup_grace_ms == 0
        || producer.schema_cleanup_grace_ms > 60_000
        || producer.schema_cpu_seconds == 0
        || producer.schema_cpu_seconds > ExecutorBudget::MAX_SCALAR_CPU_SECONDS
        || producer.schema_address_space_bytes == 0
        || producer.schema_address_space_bytes == u64::MAX
        || producer.schema_max_receipts == 0
        || producer.schema_max_receipts > 100_000
        || producer.schema_max_receipt_bytes == 0
        || producer.schema_max_receipt_bytes > 64 * 1024 * 1024
    {
        return Err(SourceNavigationSelectionError::Invalid(
            "source navigation producer resource limits",
        ));
    }
    for path in &producer.schema_paths {
        if !path.starts_with("ToS/contracts/")
            || !path.ends_with(".schema.json")
            || !tos_foundation::RelativePath::parse(path)
                .is_ok_and(|parsed| parsed.as_str() == path)
        {
            return Err(SourceNavigationSelectionError::Invalid(
                "source navigation schema closure path",
            ));
        }
    }
    Ok(())
}

fn validate_navigation_selection_scope(
    selection: &NavigationSelection,
) -> Result<Digest256, SourceNavigationSelectionError> {
    if selection.schema_version != SELECTION_SCHEMA
        || selection.domain.is_empty()
        || selection.domain.len() > 1_024
        || selection.domain.contains('\0')
        || selection.job_id.is_empty()
        || selection.job_id.len() > 256
        || selection.job_id.contains('\0')
        || selection.consumer_id.is_empty()
        || selection.consumer_id.len() > 256
        || selection.consumer_id.contains('\0')
        || selection.profile_id.is_empty()
        || selection.profile_id.len() > 128
        || selection.profile_id.contains('\0')
        || selection.job_fence == 0
        || selection.profile_version == 0
    {
        return Err(SourceNavigationSelectionError::Invalid(
            "source navigation selection scope",
        ));
    }
    Digest256::from_hex(&selection.source_contract_sha256)
        .map_err(|_| SourceNavigationSelectionError::Invalid("source navigation contract digest"))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnerConfiguration {
    schema_version: String,
    uid: u64,
    source_root: String,
    expires_at: String,
    source_navigation: NavigationSelection,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NavigationSelection {
    schema_version: String,
    domain: String,
    source_contract_sha256: String,
    rule_version: u64,
    rights_version: u64,
    job_id: String,
    job_fence: u64,
    consumer_id: String,
    profile_id: String,
    profile_version: u32,
    store_root: String,
    store_limits: StoreLimits,
    tree_limits: TreeLimits,
    model_path: String,
    source_proof_v2: tos_compiler::managed_source::ManagedSourceProofV2,
    expectation: tos_compiler::KnowledgeSelectedExpectation,
    process_limits: tos_compiler::NativeProcessLimits,
    cold_limits: tos_compiler::ColdOpenLimits,
    policy_receipt: serde_json::Value,
    #[serde(default)]
    producer: Option<NavigationProducerSelection>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NavigationProducerSelection {
    schema_version: String,
    event_path: String,
    requested_paths: Vec<String>,
    schema_paths: Vec<String>,
    schema_worker_path: String,
    schema_worker_sha256: String,
    schema_execution_wall_ms: u64,
    schema_cleanup_grace_ms: u64,
    schema_cpu_seconds: u64,
    schema_address_space_bytes: u64,
    schema_max_receipts: usize,
    schema_max_receipt_bytes: usize,
    max_events: usize,
    event_limits: NavigationProducerEventLimits,
    aggregate_limits: NavigationProducerAggregateLimits,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct NavigationProducerEventLimits {
    max_dependencies: usize,
    max_cohort_rows: usize,
    max_current_cohorts: usize,
    max_current_rows: usize,
    max_adjacency_rows: usize,
    max_adjacency_bytes: u64,
    max_row_bytes: usize,
    max_total_bytes: u64,
    statement_timeout_ms: u64,
}

impl NavigationProducerEventLimits {
    fn into_limits(self) -> DurableNavigationEventLimits {
        DurableNavigationEventLimits {
            max_dependencies: self.max_dependencies,
            max_cohort_rows: self.max_cohort_rows,
            max_current_cohorts: self.max_current_cohorts,
            max_current_rows: self.max_current_rows,
            max_adjacency_rows: self.max_adjacency_rows,
            max_adjacency_bytes: self.max_adjacency_bytes,
            max_row_bytes: self.max_row_bytes,
            max_total_bytes: self.max_total_bytes,
            statement_timeout: std::time::Duration::from_millis(self.statement_timeout_ms),
        }
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct NavigationProducerAggregateLimits {
    max_total_rows: u64,
    max_total_encoded_bytes: u64,
    max_total_dependency_bytes: u64,
    max_total_work_units: u64,
}

/// Minimal validated scope for source-navigation policy mutations. It can be
/// built without opening the optional model or producer inputs.
pub(crate) struct NavigationPolicyOwnerViewV1<'a> {
    pub source_root: &'a Path,
    pub domain: &'a str,
    pub source_contract: Digest256,
    pub bootstrap_source_revision: Digest256,
    pub source_epoch: u64,
    pub source_definition_digest: Digest256,
    pub bootstrap_membership_digest: Digest256,
    pub bootstrap_membership_count: u64,
    pub domain_digest: Digest256,
    pub database_oid: u64,
    pub anchor_through_sequence: u64,
    pub rule_version: u64,
    pub rights_version: u64,
    pub job_id: &'a str,
    pub job_fence: u64,
    pub consumer_id: &'a str,
    pub profile_id: &'a str,
    pub profile_version: u32,
    pub policy_receipt: crate::source_navigation_policy::SourceNavigationPolicyReceiptV1,
    pub policy_receipt_sha256: Digest256,
    pub owner_configuration_sha256: Digest256,
}

/// Validated owner-selected settings handed to the bounded event producer
/// while the protected source owner is held.
pub(crate) struct NavigationProducerOwnerSelection<'a> {
    pub domain: &'a str,
    pub source_contract: Digest256,
    pub rule_version: u64,
    pub rights_version: u64,
    pub job_id: &'a str,
    pub job_fence: u64,
    pub consumer_id: &'a str,
    pub policy_receipt_value: &'a serde_json::Value,
    pub profile: DurableProjectionProfile,
    pub owner_configuration_sha256: Digest256,
    pub store_root: PathBuf,
    pub store_limits: SegmentLimits,
    pub event_path: &'a str,
    pub requested_paths: &'a [String],
    pub schema_paths: &'a [String],
    pub schema_worker: ExactWorkerIdentity,
    pub schema_profile: tos_validation::FormatProfile,
    pub schema_budget: ExecutorBudget,
    pub schema_limits: CutWorkerLimits,
    pub max_events: usize,
    pub event_limits: DurableNavigationEventLimits,
    pub max_total_rows: u64,
    pub max_total_encoded_bytes: u64,
    pub max_total_dependency_bytes: u64,
    pub max_total_work_units: u64,
}

/// Exact bounded PG and immutable-model selection presented only from inside
/// the held current-read callback. `model_membership_root` names the selected
/// model expectation; the current CMD address-tree commitment is kept distinct.
#[derive(Clone, Debug)]
pub(crate) struct NavigationProducerHeldOwnerIdentity {
    pub proof_sha256: Digest256,
    pub domain: String,
    pub domain_sha256: Digest256,
    pub store_id: [u8; 16],
    pub source_head_sequence: u64,
    pub source_generation: u64,
    pub source_epoch: u64,
    pub selected_audit_generation: u64,
    pub database_oid: u64,
    /// Immutable proof-named tree commitment; it is not today's PG tree.
    pub anchor_addressed_tree_commitment: Digest256,
    pub managed_generation_digest: Digest256,
    /// Count and projection from the immutable bootstrap proof.
    pub anchor_member_count: u64,
    pub anchor_projection_digest: Digest256,
    /// These PG columns retain the bootstrap membership root and count.
    pub bootstrap_membership_root: Digest256,
    pub bootstrap_membership_count: u64,
    pub current_selected_generation_digest: Option<Digest256>,
    pub current_source_projection_digest: Option<Digest256>,
    pub schema_profile_digest: Digest256,
    pub model_sha256: Digest256,
    pub model_membership_root: Digest256,
    pub owner_configuration_sha256: Digest256,
    pub source_contract: Digest256,
    pub rule_version: u64,
    pub rights_version: u64,
    pub job_id_sha256: Digest256,
    pub job_fence: u64,
}

impl NavigationProducerHeldOwnerIdentity {
    fn from_selected(
        proof: &tos_compiler::managed_source::ManagedSourceProofV2,
        expectation: &tos_compiler::KnowledgeSelectedExpectation,
        current: &SourceNavigationCurrentLeaseV1<'_>,
        owner: &NavigationProducerOwnerSelection<'_>,
        managed_generation_digest: Digest256,
        anchor_member_count: u64,
    ) -> Result<Self, SourceNavigationSelectionError> {
        let g = &proof.generation;
        let digest = |value: &str, label| {
            Digest256::from_hex(value).map_err(|_| SourceNavigationSelectionError::Invalid(label))
        };
        if g.domain != owner.domain
            || g.through_commit_seq != expectation.through_commit_seq
            || g.current_members != anchor_member_count
            || current.head_sequence() != current.source_generation()
            || !current.rights_allowed()
            || current.database_oid() != g.database_oid
            || current.source_membership_root()
                != digest(
                    &g.bootstrap_membership_sha256,
                    "source navigation bootstrap membership root",
                )?
            || current.source_membership_count() != g.bootstrap_members
        {
            return Err(SourceNavigationSelectionError::Invalid(
                "source navigation producer selected binding differs",
            ));
        }
        let selected_generation = digest(
            &g.installed_generation_sha256,
            "source navigation producer generation digest",
        )?;
        if selected_generation != managed_generation_digest {
            return Err(SourceNavigationSelectionError::Invalid(
                "source navigation producer generation differs",
            ));
        }
        let proof_root = proof
            .root_sha256()
            .map_err(|_| SourceNavigationSelectionError::Invalid("source navigation proof root"))?;
        Ok(Self {
            proof_sha256: digest(&proof_root, "source navigation producer proof digest")?,
            domain: g.domain.clone(),
            domain_sha256: digest(&g.domain_sha256, "source navigation domain digest")?,
            store_id: g.store_id,
            source_head_sequence: current.head_sequence(),
            source_generation: current.source_generation(),
            source_epoch: g.epoch,
            selected_audit_generation: current.audit_generation(),
            database_oid: current.database_oid(),
            anchor_addressed_tree_commitment: digest(
                &g.addressed_current_tree_sha256,
                "source navigation current tree commitment",
            )?,
            managed_generation_digest,
            anchor_member_count,
            anchor_projection_digest: digest(
                &g.inventory_projection_sha256,
                "source navigation source projection digest",
            )?,
            bootstrap_membership_root: current.source_membership_root(),
            bootstrap_membership_count: current.source_membership_count(),
            current_selected_generation_digest: current.selected_generation_digest(),
            current_source_projection_digest: current.source_projection_digest(),
            schema_profile_digest: current.schema_profile_digest(),
            model_sha256: digest(&expectation.model_sha256, "source navigation model digest")?,
            model_membership_root: digest(
                &expectation.membership_root,
                "source navigation model membership root",
            )?,
            owner_configuration_sha256: owner.owner_configuration_sha256,
            source_contract: owner.source_contract,
            rule_version: current.rule_version(),
            rights_version: current.rights_version(),
            job_id_sha256: Digest256::of_bytes(owner.job_id.as_bytes()),
            job_fence: owner.job_fence,
        })
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoreLimits {
    max_segment_bytes: u64,
    max_frame_bytes: u64,
    max_frames: u32,
    max_journal_bytes: usize,
}

impl StoreLimits {
    fn into_limits(self) -> SegmentLimits {
        SegmentLimits {
            max_segment_bytes: self.max_segment_bytes,
            max_frame_bytes: self.max_frame_bytes,
            max_frames: self.max_frames,
            max_journal_bytes: self.max_journal_bytes,
        }
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct TreeLimits {
    max_key_bytes: usize,
    max_value_bytes: usize,
    max_kind_bytes: usize,
    max_node_bytes: usize,
    max_children: usize,
    max_nodes: u64,
    max_total_bytes: u64,
    max_rows: u64,
}

impl TreeLimits {
    fn into_limits(self) -> AuthenticatedTreeLimitsV1 {
        AuthenticatedTreeLimitsV1 {
            max_key_bytes: self.max_key_bytes,
            max_value_bytes: self.max_value_bytes,
            max_kind_bytes: self.max_kind_bytes,
            max_node_bytes: self.max_node_bytes,
            max_children: self.max_children,
            max_nodes: self.max_nodes,
            max_total_bytes: self.max_total_bytes,
            max_rows: self.max_rows,
        }
    }
}

/// Reopen the retained immutable navigation anchor, then run QRY and the
/// supplied delivery callback under the live source lease, publication, and
/// explicit public-metadata policy holds.
#[allow(clippy::too_many_arguments)]
pub fn with_selected_source_navigation<T, E>(
    selected: &SelectedSourceNavigation,
    request: SourceDescendRequest,
    budget: Budget,
    deadline: Instant,
    probe: Arc<dyn AbortProbe>,
    consume: impl for<'hold> FnOnce(Result<DisclosableSourceDescend<'hold>, QueryError>) -> Result<T, E>,
) -> Result<SelectedSourceNavigationRead<Result<T, E>>, SourceNavigationSelectionError> {
    let cancelled = AtomicBool::new(false);
    if Instant::now() >= deadline || probe.reason().is_some() {
        return Err(SourceNavigationSelectionError::Invalid(
            "source navigation request deadline exhausted",
        ));
    }
    let query_result = with_selected_navigation_current(
        selected,
        deadline,
        &cancelled,
        |source, generation, proof, expectation, facts, tx, owner, _store| {
            crate::source_navigation_query::with_published_source_navigation_v2(
                source,
                generation,
                proof,
                expectation,
                facts,
                tx,
                owner,
                &owner.profile,
                DurableNavigationEventLimits::default(),
                deadline,
                &cancelled,
                |mut read_model| {
                    read_model.set_abort_probe(Arc::clone(&probe));
                    consume(source_descend(&mut read_model, &request, budget))
                },
            )
        },
    )?;
    Ok(match query_result {
        crate::source_navigation_query::SourceNavigationPublishedReadV2::Ready(result) => {
            SelectedSourceNavigationRead::Ready(result)
        }
        crate::source_navigation_query::SourceNavigationPublishedReadV2::NotReady => {
            SelectedSourceNavigationRead::NotReady
        }
    })
}
