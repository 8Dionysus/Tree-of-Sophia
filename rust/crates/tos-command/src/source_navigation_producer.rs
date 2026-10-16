//! Bounded PG-backed event producer for the private navigation projection.
//!
//! Source bytes and event membership come only from exact selected PG history
//! contexts. The protected owner callback supplies the current managed V2,
//! rights, audit, job and database identity; this module never treats its
//! bootstrap membership as a current whole-corpus root.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tos_compiler::source_bibliographic::BibliographicLimits;
use tos_compiler::source_navigation_claim_event::NavigationClaimEventSourceFileV1;
use tos_compiler::source_witness_catalog::{
    CandidateEventSourceIdentityV1, SourceCatalogLimits, SourceCatalogValidator,
};
use tos_foundation::{Digest256, Digest256Hasher, RelativePath};
use tos_segment_store::SegmentStore;
use tos_validation::SchemaResource;
use tos_validation::executor::VerifiedWorkerImageHandle;
use tos_validation::source_cut::CandidateCutWorkerSchemaExecutor;

use crate::PgRecordRevisionIdentity;

use crate::source_navigation_cli::{
    NavigationProducerHeldOwnerIdentity, NavigationProducerOwnerSelection,
};
use crate::{DurableError, DurableNavigationEventContextV1, DurablePgCoordinator, DurableResult};

// Owner reasons use bounded renderers; selected paths, source payloads and
// dynamic database diagnostics remain private.
fn producer_refusal(error: crate::source_navigation_cli::SourceNavigationSelectionError) -> String {
    use crate::source_command::SourceCommandError;
    use crate::source_managed_selection::ManagedSelectionError;
    use crate::source_navigation_cli::SourceNavigationSelectionError;
    match error {
        SourceNavigationSelectionError::Invalid(reason) => reason.to_owned(),
        SourceNavigationSelectionError::Managed(ManagedSelectionError::Durable(
            DurableError::Invalid(reason)
            | DurableError::Conflict(reason)
            | DurableError::Refused(reason)
            | DurableError::Corrupt(reason)
            | DurableError::Indeterminate(reason),
        )) => reason.to_owned(),
        SourceNavigationSelectionError::Managed(
            ManagedSelectionError::Source(error)
            | ManagedSelectionError::Durable(DurableError::Source(error)),
        ) => match error {
            SourceCommandError::Invalid(reason)
            | SourceCommandError::InvalidAt { reason, .. }
            | SourceCommandError::Conflict(reason)
            | SourceCommandError::Denied(reason)
            | SourceCommandError::Unsupported(reason) => reason.to_owned(),
            SourceCommandError::DeniedWithReason(reason) => reason,
            error @ SourceCommandError::SchemaExecution { .. } => error.public_reason(),
            SourceCommandError::MissingProductionAdmission => {
                "navigation source admission missing".into()
            }
        },
        SourceNavigationSelectionError::Managed(ManagedSelectionError::Compiler(error)) => {
            crate::source_command::public_compiler_reason(&error)
        }
        SourceNavigationSelectionError::Managed(ManagedSelectionError::Durable(
            DurableError::Database(error),
        )) => {
            if std::error::Error::source(&error)
                .is_some_and(|cause| cause.is::<postgres::types::WrongType>())
            {
                "navigation database parameter type mismatch".into()
            } else if error.as_db_error().is_some() {
                "navigation database statement refused".into()
            } else {
                "navigation database client operation failed".into()
            }
        }
        SourceNavigationSelectionError::Managed(ManagedSelectionError::Durable(
            DurableError::Storage(_),
        )) => "navigation source storage refused".into(),
        SourceNavigationSelectionError::Managed(ManagedSelectionError::Query(_)) => {
            "navigation query selection refused".into()
        }
    }
}

// Use the command owner's bounded renderer: worker evidence survives while
// dynamic source, filesystem and database details remain redacted.
pub(crate) fn compiler_refusal(error: tos_compiler::Error) -> DurableError {
    DurableError::Source(crate::source_command::SourceCommandError::DeniedWithReason(
        crate::source_command::public_compiler_reason(&error),
    ))
}

/// Run the selected producer while the managed source, rights, audit, job,
/// admission metadata, and owner-configuration fences remain held through publication.
pub(crate) fn run(
    selection_path: &Path,
    requested_max_events: usize,
    cancelled: &AtomicBool,
) -> Result<Value, String> {
    if !(1..=256).contains(&requested_max_events) {
        return Err("max_events must be 1..256".into());
    }
    let deadline = Instant::now() + Duration::from_secs(300);
    let selected = crate::source_navigation_cli::SelectedSourceNavigation::new(selection_path)
        .map_err(|_| "source navigation selection refused")?;
    crate::source_navigation_cli::with_selected_navigation_current(
        &selected,
        deadline,
        cancelled,
        |lease, generation, proof, expectation, owner, owner_tx, selection, store| {
            // The prior publication must be bound to this protected selection.
            // Do not take a consumer FOR SHARE here: publication uses its own
            // bounded transactions under this same source-owner hold.
            let guard = crate::durable_adapter::durable_projection::read_navigation_published_guard_with_tx_v2(
                owner_tx, selection.domain, selection.consumer_id,
                &selection.profile, selection.event_limits,
            ).map_err(crate::source_managed_selection::ManagedSelectionError::Durable)?;
            let crate::durable_adapter::durable_projection::DurableNavigationPublishedGenerationV2::Ready(guard) = guard else {
                return Err(crate::source_managed_selection::ManagedSelectionError::Durable(
                    DurableError::Conflict("navigation initial publication required")));
            };
            crate::source_navigation_query::verify_selected_binding(
                generation, expectation, proof, lease, owner, &guard, selection,
            ).map_err(|_| crate::source_managed_selection::ManagedSelectionError::Durable(
                DurableError::Conflict("navigation published anchor differs from protected selection")))?;
            run_held_owner(
                store,
                owner,
                owner_tx,
                selection,
                requested_max_events,
                deadline,
                cancelled,
            )
            .map_err(crate::source_managed_selection::ManagedSelectionError::Durable)
        },
    )
    .map_err(producer_refusal)
}

/// Bootstrap the selected private navigation read model while the managed
/// source, rights, audit, job, model, and owner-configuration fences remain
/// held across this invocation's bounded page transactions and publication.
pub(crate) fn run_bootstrap(
    selection_path: &Path,
    max_batches: usize,
    cancelled: &AtomicBool,
) -> Result<Value, String> {
    let deadline = Instant::now() + Duration::from_secs(300);
    let selected = crate::source_navigation_cli::SelectedSourceNavigation::new(selection_path)
        .map_err(|_| "source navigation selection refused")?;
    crate::source_navigation_cli::with_selected_navigation_anchor_current(
        &selected,
        deadline,
        cancelled,
        |model, _lease, selected, _proof, expectation, owner, owner_tx, selection, store| {
            if Instant::now() >= deadline || cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(
                    crate::source_managed_selection::ManagedSelectionError::Durable(
                        DurableError::Refused("navigation bootstrap deadline or cancellation"),
                    ),
                );
            }
            let result = (|| {
                let mut coordinator =
                    open_selected_event_reader(store, owner, selection, owner_tx)?;
                let progress = coordinator.capture_navigation_initial_v1(
                    store,
                    selection.domain,
                    selection.consumer_id,
                    &selection.profile,
                    selected,
                    model,
                    selection.source_contract,
                    selection.rule_version,
                    selection.rights_version,
                    selection.job_id,
                    selection.job_fence,
                    selection.event_limits,
                    max_batches,
                    deadline,
                    cancelled,
                )?;
                let mut response = serde_json::to_value(progress)
                    .map_err(|_| DurableError::Invalid("navigation initial progress encoding"))?;
                let metadata = json!({
                    "schema_version": "tos_source_navigation_bootstrap_result_v2",
                    "domain": selection.domain,
                    "consumer_id": selection.consumer_id,
                    "profile": selection.profile.id,
                    "source_revision": expectation.membership_root,
                    "source_membership_count": selected.generation().member_count(),
                    "source_head_sequence": owner.source_head_sequence,
                });
                response
                    .as_object_mut()
                    .ok_or(DurableError::Invalid("navigation initial progress object"))?
                    .extend(metadata.as_object().unwrap().clone());
                Ok(response)
            })();
            result.map_err(crate::source_managed_selection::ManagedSelectionError::Durable)
        },
    )
    .map_err(producer_refusal)
}

fn run_held_owner(
    store: &SegmentStore,
    owner: &NavigationProducerHeldOwnerIdentity,
    owner_tx: &mut postgres::Transaction<'_>,
    selection: &NavigationProducerOwnerSelection<'_>,
    requested_max_events: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<Value> {
    let mut event_reader = open_selected_event_reader(store, owner, selection, owner_tx)?;
    let max_events = requested_max_events.min(selection.max_events);
    if Instant::now() >= deadline || cancelled.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(DurableError::Refused(
            "navigation producer deadline or cancellation",
        ));
    }
    match event_reader.read_navigation_event_range_seed_v1(
        selection.domain,
        selection.consumer_id,
        &selection.profile,
        max_events,
        selection.event_limits,
    )? {
        crate::durable_adapter::durable_projection::DurableNavigationEventRangeSeedV1::UpToDate {
            cursor_sequence,
            source_head_sequence,
        } => Ok(json!({
            "schema_version": "tos_source_navigation_advance_result_v1",
            "profile": selection.profile.id,
            "cursor_before": cursor_sequence,
            "cursor_sequence": cursor_sequence,
            "source_head_sequence": source_head_sequence,
            "published": [],
            "stop": "up_to_date",
        })),
        crate::durable_adapter::durable_projection::DurableNavigationEventRangeSeedV1::Pending {
            cursor_sequence,
            source_head_sequence,
            reason,
        } => Ok(json!({
            "schema_version": "tos_source_navigation_advance_result_v1",
            "profile": selection.profile.id,
            "cursor_before": cursor_sequence,
            "cursor_sequence": cursor_sequence,
            "source_head_sequence": source_head_sequence,
            "published": [],
            "stop": "pending",
            "pending_reason": match reason {
                crate::durable_adapter::durable_projection::DurableNavigationEventRangePendingReasonV1::EventBudgetExceeded => {
                    "event_budget_exceeded"
                }
                crate::durable_adapter::durable_projection::DurableNavigationEventRangePendingReasonV1::MissingEventProof => {
                    "missing_event_proof"
                }
            },
        })),
        crate::durable_adapter::durable_projection::DurableNavigationEventRangeSeedV1::Ready(coverage) => {
            if Instant::now() >= deadline || cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(DurableError::Refused(
                    "navigation producer deadline or cancellation",
                ));
            }
            let cursor_before = coverage.cursor_before();
            let cursor_through = coverage.through_sequence();
            let source_head_sequence = coverage.held_source_head_sequence();
            if source_head_sequence != owner.source_head_sequence
                || source_head_sequence != owner.source_generation
                || cursor_through > source_head_sequence
            {
                return Err(DurableError::Conflict(
                    "navigation event range differs from the held source head",
                ));
            }
            let (published, work) = process_ready_range(
                &mut event_reader,
                &store,
                &coverage,
                owner,
                owner_tx,
                selection,
                deadline,
                cancelled,
            )?;
            Ok(json!({
                "schema_version": "tos_source_navigation_advance_result_v1",
                "profile": selection.profile.id,
                "cursor_before": cursor_before,
                "cursor_sequence": cursor_through,
                "source_head_sequence": source_head_sequence,
                "range_root": coverage.coverage_root().to_hex(),
                "published": [published],
                "work": work,
                "stop": if cursor_through == source_head_sequence {
                    "up_to_date"
                } else {
                    "more_pending"
                },
                "pending_reason": if cursor_through == source_head_sequence {
                    Value::Null
                } else {
                    json!("event_budget_exceeded")
                },
            }))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn process_ready_range(
    event_reader: &mut DurablePgCoordinator,
    store: &SegmentStore,
    coverage: &crate::durable_adapter::durable_projection::DurableNavigationEventRangeCoverageV1,
    owner: &NavigationProducerHeldOwnerIdentity,
    owner_tx: &mut postgres::Transaction<'_>,
    selection: &NavigationProducerOwnerSelection<'_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<(Value, Value)> {
    use crate::source_claim_dependency_index::ClaimNavigationReferenceKind;
    use tos_compiler::source_navigation_claim_event::{
        NavigationClaimEventRangeDependencyCauseInputV1,
        NavigationClaimEventRangeEmptyClosureInputV1, NavigationClaimEventRangeLookupKeyV1,
        NavigationClaimEventRangeLookupKindV1, NavigationClaimEventRangeManifestInputV1,
        NavigationClaimEventRangeMutationInputV1, NavigationClaimEventRangePriorOwnerBasisV1,
        NavigationClaimEventRangePriorOwnerV1,
    };
    use tos_compiler::source_navigation_record_range::{
        NavigationRecordEventRangeClaimCauseInputV1,
        NavigationRecordEventRangeDependencyCauseInputV1, NavigationRecordEventRangeInputV1,
        NavigationRecordEventRangeMutationCauseInputV1,
        NavigationRecordEventRangeMutationKindInputV1, NavigationRecordEventRangeOwnerInputV1,
        NavigationRecordEventRangeOwnerKeyV1, NavigationRecordEventRangePriorOwnerInputV1,
    };

    let check_active = || {
        if Instant::now() >= deadline || cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            Err(DurableError::Refused(
                "navigation producer deadline or cancellation",
            ))
        } else {
            Ok(())
        }
    };
    check_active()?;
    let compiler_events = coverage
        .events()
        .iter()
        .map(
            |event| tos_compiler::source_navigation_event::NavigationEventRangeEventV1 {
                sequence: event.sequence,
                event_id: event.event_id.clone(),
                command_id: event.command_id.clone(),
                event_kind: event.event_kind.clone(),
                member_root: event.member_root,
                members: event
                    .members
                    .iter()
                    .map(|member| {
                        tos_compiler::source_navigation_event::NavigationEventRangeMemberV1 {
                            source_path: member.source_path.clone(),
                            revision: member.revision,
                            is_source: member.is_source,
                            size_bytes: member.size_bytes,
                            sha256: member.sha256,
                        }
                    })
                    .collect(),
            },
        )
        .collect::<Vec<_>>();
    let compiler_coverage_input =
        tos_compiler::source_navigation_event::NavigationEventRangeCoverageInputV1 {
            cursor_before: coverage.cursor_before(),
            through_sequence: coverage.through_sequence(),
            terminal_event_id: coverage.terminal_event_id(),
            events: &compiler_events,
            coverage_root: coverage.coverage_root(),
            manifest_raw: coverage.manifest_raw(),
        };
    let compiler_coverage = seal_compiler_range_coverage(coverage)?;

    let changed_source_paths = coverage.changed_source_paths();
    let changed_tos_paths = changed_source_paths
        .iter()
        .filter(|path| path.starts_with("ToS/"))
        .cloned()
        .collect::<Vec<_>>();
    let range_source_bytes = coverage.events().iter().try_fold(0u64, |total, event| {
        event
            .members
            .iter()
            .filter(|member| member.is_source)
            .try_fold(total, |sum, member| {
                sum.checked_add(member.size_bytes)
                    .ok_or(DurableError::Refused(
                        "navigation range source byte overflow",
                    ))
            })
    })?;
    if range_source_bytes > selection.event_limits.max_total_bytes as u64 {
        return Err(DurableError::Refused(
            "navigation event range source byte budget",
        ));
    }

    let range_creates =
        crate::source_navigation_record_input::direct_source_create_records_for_range_v1(
            coverage,
            selection.event_limits.max_current_cohorts,
            |sequence, path| {
                event_reader.read_navigation_event_range_member_source_v1(
                    store,
                    selection.domain,
                    &selection.profile,
                    coverage,
                    sequence,
                    path,
                    selection.event_limits,
                )
            },
        )?;
    let range_revisions =
        crate::source_navigation_record_input::direct_record_revisions_for_range_v1(
            coverage,
            selection.event_limits.max_current_cohorts,
            |sequence, path| {
                event_reader.read_navigation_event_range_member_source_v1(
                    store,
                    selection.domain,
                    &selection.profile,
                    coverage,
                    sequence,
                    path,
                    selection.event_limits,
                )
            },
        )?;
    let range_claim_mutations =
        direct_claim_mutations_for_range(event_reader, store, selection, coverage)?;

    let prior_record_owners = if changed_source_paths.is_empty() {
        Vec::new()
    } else {
        event_reader.read_navigation_event_range_prior_record_owners_v1(
            selection.domain,
            selection.consumer_id,
            coverage,
            &changed_source_paths,
            &selection.profile,
            selection.event_limits,
        )?
    };
    let prior_claim_owners = if changed_source_paths.is_empty() {
        Vec::new()
    } else {
        event_reader.read_navigation_event_range_prior_claim_owners_v1(
            selection.domain,
            selection.consumer_id,
            coverage,
            &changed_source_paths,
            &selection.profile,
            selection.event_limits,
        )?
    };

    let mut record_owners_by_key =
        BTreeMap::<(String, String), crate::source_navigation_record_input::RecordOwnerV1>::new();
    for previous in &prior_record_owners {
        let key = (previous.source_path.clone(), previous.record_id.clone());
        record_owners_by_key.entry(key).or_insert_with(|| {
            crate::source_navigation_record_input::RecordOwnerV1 {
                source_path: previous.source_path.clone(),
                record_id: previous.record_id.clone(),
                origin:
                    crate::source_navigation_record_input::RecordOwnerOriginV1::PersistedAffected,
            }
        });
    }
    for created in &range_creates {
        let key = (
            created.owner.source_path.clone(),
            created.owner.record_id.clone(),
        );
        if record_owners_by_key.contains_key(&key)
            || record_owners_by_key
                .values()
                .any(|previous| previous.record_id == created.owner.record_id)
        {
            return Err(DurableError::Conflict(
                "navigation range direct Record creation reuses a prior owner identity",
            ));
        }
        record_owners_by_key.insert(key, created.owner.clone());
    }
    for revision in &range_revisions {
        let key = (
            revision.owner.source_path.clone(),
            revision.owner.record_id.clone(),
        );
        record_owners_by_key
            .entry(key)
            .or_insert_with(|| revision.owner.clone());
    }
    if record_owners_by_key.len() > selection.event_limits.max_current_cohorts {
        return Err(DurableError::Refused(
            "navigation range Record candidate owner budget",
        ));
    }

    let mut ids_by_path = BTreeMap::<String, BTreeSet<String>>::new();
    for previous in &prior_record_owners {
        ids_by_path
            .entry(previous.cause_path.clone())
            .or_default()
            .insert(previous.record_id.clone());
    }
    for created in &range_creates {
        ids_by_path
            .entry(created.source_path.clone())
            .or_default()
            .insert(created.owner.record_id.clone());
    }
    for revision in &range_revisions {
        ids_by_path
            .entry(revision.source_path.clone())
            .or_default()
            .insert(revision.owner.record_id.clone());
    }
    for mutation in &range_claim_mutations {
        ids_by_path
            .entry(mutation.claim_stream_member_path.clone())
            .or_default()
            .extend(mutation.mutation_claim_ids.iter().cloned());
    }
    let changed_ids = ids_by_path
        .values()
        .flat_map(|ids| ids.iter().cloned())
        .collect::<BTreeSet<_>>();

    let mut held_readsets = Vec::<crate::source_cohort::revision::SourceRevisionReadSet>::new();
    let claim_closure = if changed_tos_paths.is_empty() {
        None
    } else {
        let reference_keys = claim_navigation_lookup_keys(
            &changed_tos_paths,
            &changed_ids.iter().cloned().collect::<Vec<_>>(),
        )?;
        let (_, readset) = event_reader.with_source_revision_session(
            store,
            selection.domain,
            selection.job_id,
            selection.job_fence,
            deadline,
            cancelled,
            |session| {
                let selected = session.selection().clone();
                verify_identity_session(owner, selection, &selected.identity())?;
                let _observations = session.lookup_claim_navigation_references_at(
                    &reference_keys,
                    coverage.through_sequence(),
                    deadline,
                    cancelled,
                )?;
                Ok(())
            },
        )?;
        let closure = readset
            .claim_navigation_closure()
            .cloned()
            .ok_or(DurableError::Conflict(
                "navigation range as-of Claim closure was not retained",
            ))?;
        if closure.selection_identity() != readset.selection().identity() {
            return Err(DurableError::Conflict(
                "navigation range Claim closure differs from its readset selection",
            ));
        }
        let identity = closure.selection_identity();
        if closure.domain() != selection.domain
            || closure.as_of_sequence() != coverage.through_sequence()
            || closure.held_source_head_sequence() != coverage.held_source_head_sequence()
            || closure.baseline_sequence() > coverage.through_sequence()
            || identity.head_seq != coverage.held_source_head_sequence()
            || identity.source_generation != owner.source_generation
            || identity.epoch != owner.source_epoch
        {
            return Err(DurableError::Conflict(
                "navigation range as-of Claim closure differs from the held owner",
            ));
        }
        held_readsets.push(readset);
        Some(closure)
    };

    let (current_claim_ids, current_claim_stream_paths, claim_dependency_causes) =
        if let Some(closure) = claim_closure.as_ref() {
            let (ids, paths) = claim_navigation_reverse_owners(closure);
            let causes =
                range_dependency_causes(coverage, &prior_claim_owners, closure, &ids_by_path)?;
            (ids, paths, causes)
        } else {
            (Vec::new(), Vec::new(), Vec::new())
        };

    let mut terminal_claim_stream_paths = current_claim_stream_paths
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    terminal_claim_stream_paths.extend(
        prior_claim_owners
            .iter()
            .map(|owner| owner.claim_stream_path.clone()),
    );
    terminal_claim_stream_paths.extend(
        range_claim_mutations
            .iter()
            .map(|mutation| mutation.claim_stream_member_path.clone()),
    );
    if terminal_claim_stream_paths.len() > selection.event_limits.max_current_cohorts {
        return Err(DurableError::Refused(
            "navigation range Claim stream count budget",
        ));
    }

    let initial_paths = changed_tos_paths.iter().cloned().collect::<BTreeSet<_>>();
    let mut context = read_current_range_context(
        event_reader,
        store,
        owner,
        selection,
        coverage,
        &initial_paths,
    )?;
    let mut record_plan = if record_owners_by_key.is_empty() {
        None
    } else {
        let candidate_owners = record_owners_by_key.values().cloned().collect::<Vec<_>>();
        Some(
            crate::source_navigation_record_input::required_paths_for_owners_v1(
                &context,
                &candidate_owners,
                selection.event_path,
                selection.schema_paths,
                selection.event_limits.max_dependencies,
            )?,
        )
    };
    let mut context_additions = BTreeSet::new();
    if let Some(plan) = &record_plan {
        context_additions.extend(plan.paths.iter().cloned());
    }
    context_additions.extend(terminal_claim_stream_paths.iter().cloned());
    stage_range_context(
        event_reader,
        store,
        owner,
        selection,
        coverage,
        &mut context,
        context_additions,
    )?;

    let mut claim_identity_owners =
        Vec::<crate::source_navigation_claim_selection::ClaimEventIdentityOwnerV1>::new();
    let mut claim_endpoint_paths = Vec::new();
    if !terminal_claim_stream_paths.is_empty() {
        let stream_paths = terminal_claim_stream_paths
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        // Select support files before deriving any addresses from their bytes.
        // Archive manifests are resolved below, after history is in the context.
        let paths =
            crate::source_navigation_claim_selection::claim_event_range_current_owner_paths_v1(
                &context,
                &stream_paths,
            )?;
        stage_range_context(
            event_reader,
            store,
            owner,
            selection,
            coverage,
            &mut context,
            paths,
        )?;

        let requests =
            crate::source_navigation_claim_selection::claim_event_range_identity_requests_v1(
                &context,
                &stream_paths,
            )?;
        let identity_tokens = requests
            .iter()
            .map(|request| request.id.clone())
            .collect::<BTreeSet<_>>();
        let (identity_closure, identity_readset) = event_reader.with_source_revision_session(
            store,
            selection.domain,
            selection.job_id,
            selection.job_fence,
            deadline,
            cancelled,
            |session| {
                let selected = session.selection().clone();
                verify_identity_session(owner, selection, &selected.identity())?;
                if let Some(closure) = claim_closure.as_ref() {
                    if selected.identity() != closure.selection_identity() {
                        return Err(DurableError::Conflict(
                            "Claim identity lookup uses another as-of source owner epoch",
                        ));
                    }
                }
                let identities =
                    session.lookup_source_identities(&identity_tokens, deadline, cancelled)?;
                for identity_owner in identities.owners().values().flatten() {
                    let path = RelativePath::parse(identity_owner.1.as_str())
                        .map_err(|_| DurableError::Corrupt("Claim identity source path"))?;
                    session.read_current_member(
                        &path,
                        selection.event_limits.max_total_bytes,
                        deadline,
                        cancelled,
                    )?;
                }
                Ok(identities)
            },
        )?;
        identity_closure.verify_readset(&identity_readset)?;
        claim_identity_owners =
            requests
                .iter()
                .map(|request| {
                    let indexed = identity_closure.owners().get(&request.id).ok_or(
                        DurableError::Conflict("Claim identity lookup result absent"),
                    )?;
                    Ok(
                        crate::source_navigation_claim_selection::ClaimEventIdentityOwnerV1 {
                            id: request.id.clone(),
                            kind: indexed.as_ref().map(|(kind, _)| kind.clone()),
                            source_path: indexed.as_ref().map(|(_, path)| path.as_str().to_owned()),
                        },
                    )
                })
                .collect::<DurableResult<Vec<_>>>()?;
        let identity_paths = claim_identity_owners
            .iter()
            .filter_map(|owner| owner.source_path.clone())
            .collect::<BTreeSet<_>>();
        stage_range_context(
            event_reader,
            store,
            owner,
            selection,
            coverage,
            &mut context,
            identity_paths,
        )?;
        verify_range_identity_owners_at_cut(
            &context,
            &claim_identity_owners,
            selection.event_limits.max_total_bytes,
        )?;
        held_readsets.push(identity_readset);

        let indexed_paths =
            crate::source_navigation_claim_selection::claim_event_range_indexed_paths_v1(
                &context,
                &stream_paths,
                &claim_identity_owners,
            )?;
        stage_range_context(
            event_reader,
            store,
            owner,
            selection,
            coverage,
            &mut context,
            indexed_paths,
        )?;
        let schema_paths =
            crate::source_navigation_claim_selection::claim_event_range_schema_paths_v1(
                &context,
                &stream_paths,
                &claim_identity_owners,
            )?;
        require_configured_schemas(selection, &schema_paths)?;
        stage_range_context(
            event_reader,
            store,
            owner,
            selection,
            coverage,
            &mut context,
            schema_paths,
        )?;
        claim_endpoint_paths = crate::source_navigation_claim_selection::claim_event_range_historical_endpoint_paths_v1(
            &context,
            &stream_paths,
            &claim_identity_owners,
        )?;
        let capture_paths = crate::source_navigation_claim_selection::claim_event_range_historical_capture_paths_v1(
            &context,
            &stream_paths,
            &claim_identity_owners,
        )?;
        stage_range_context(
            event_reader,
            store,
            owner,
            selection,
            coverage,
            &mut context,
            claim_endpoint_paths.clone(),
        )?;
        stage_range_context(
            event_reader,
            store,
            owner,
            selection,
            coverage,
            &mut context,
            capture_paths,
        )?;
        let archive_manifests =
            crate::source_navigation_claim_selection::claim_event_range_archive_manifest_paths_v1(
                &context,
                &stream_paths,
            )?;
        stage_range_context(
            event_reader,
            store,
            owner,
            selection,
            coverage,
            &mut context,
            archive_manifests.clone(),
        )?;
        let archive_blobs =
            crate::source_navigation_claim_selection::claim_event_range_archive_blob_paths_v1(
                &context,
                &archive_manifests,
            )?;
        stage_range_context(
            event_reader,
            store,
            owner,
            selection,
            coverage,
            &mut context,
            archive_blobs,
        )?;
        let identity_archive_manifests = crate::source_navigation_claim_selection::claim_event_range_identity_archive_manifest_paths_v1(&context, &stream_paths, &claim_identity_owners)?;
        stage_range_context(
            event_reader,
            store,
            owner,
            selection,
            coverage,
            &mut context,
            identity_archive_manifests.clone(),
        )?;
        let identity_archive_blobs = crate::source_navigation_claim_selection::claim_event_range_identity_archive_blob_paths_v1(&context, &stream_paths, &claim_identity_owners)?;
        stage_range_context(
            event_reader,
            store,
            owner,
            selection,
            coverage,
            &mut context,
            identity_archive_blobs,
        )?;
    } else if !claim_dependency_causes.is_empty()
        || !range_claim_mutations.is_empty()
        || !prior_claim_owners.is_empty()
    {
        return Err(DurableError::Conflict(
            "navigation Claim range has causes but no terminal owner streams",
        ));
    }

    let limits = bibliographic_limits(selection, deadline)?;
    if let Some(plan) = record_plan.as_mut() {
        let files = selected_bibliographic_files(&context, &selection.event_limits)?;
        let manifests = crate::source_navigation_record_input::archive_manifest_paths(
            &plan.owners,
            &files,
            limits,
        )?;
        if !manifests.is_empty() {
            stage_range_context(
                event_reader,
                store,
                owner,
                selection,
                coverage,
                &mut context,
                manifests.clone(),
            )?;
        }
        let files = selected_bibliographic_files(&context, &selection.event_limits)?;
        let blobs = crate::source_navigation_record_input::archive_blob_paths(
            &plan.owners,
            &manifests,
            &files,
            limits,
        )?;
        if !blobs.is_empty() {
            stage_range_context(
                event_reader,
                store,
                owner,
                selection,
                coverage,
                &mut context,
                blobs,
            )?;
        }
        plan.paths.sort();
        plan.paths.dedup();
    }

    let identity = range_source_cut_identity(owner, selection, coverage, &context)?;
    let files = if context.selected_sources().is_empty() {
        None
    } else {
        Some(selected_bibliographic_files(
            &context,
            &selection.event_limits,
        )?)
    };
    let mut meter = NavigationEventBudgetMeterV1::default();
    let mut forms = crate::source_forms_compiler::NativeBibliographicForms;

    let mut claim_prior_inputs = prior_claim_owners
        .iter()
        .map(|row| NavigationClaimEventRangePriorOwnerV1 {
            cause_path: row.cause_path.clone(),
            claim_stream_path: row.claim_stream_path.clone(),
            claim_id: row.claim_id.clone(),
            basis: match row.predecessor_basis {
                crate::durable_adapter::durable_projection::DurableNavigationEventRangePriorOwnerBasisV1::EventCohort {
                    cohort_root,
                } => NavigationClaimEventRangePriorOwnerBasisV1::EventCohort { cohort_root },
                crate::durable_adapter::durable_projection::DurableNavigationEventRangePriorOwnerBasisV1::Bootstrap { owner_root } => {
                    NavigationClaimEventRangePriorOwnerBasisV1::Bootstrap { owner_root }
                }
            },
        })
        .collect::<Vec<_>>();
    claim_prior_inputs.sort_by(|left, right| {
        (&left.cause_path, &left.claim_stream_path, &left.claim_id).cmp(&(
            &right.cause_path,
            &right.claim_stream_path,
            &right.claim_id,
        ))
    });
    let claim_mutation_inputs = range_claim_mutations
        .iter()
        .map(|mutation| {
            let event = coverage
                .event(mutation.event_sequence)
                .ok_or(DurableError::Corrupt("range Claim mutation event absent"))?;
            let command_id = event
                .command_id
                .as_deref()
                .ok_or(DurableError::Corrupt("range Claim command identity absent"))?;
            Ok(NavigationClaimEventRangeMutationInputV1 {
                event_sequence: mutation.event_sequence,
                event_id: &mutation.event_id,
                claim_stream_member_path: &mutation.claim_stream_member_path,
                claim_stream_sha256: mutation.claim_stream_sha256,
                claim_stream_raw: &mutation.claim_stream_raw,
                receipt_path: &mutation.receipt_path,
                receipt_sha256: mutation.receipt_sha256,
                receipt_raw: &mutation.receipt_raw,
                mutation_claim_ids: &mutation.mutation_claim_ids,
                revision_history_path: mutation.revision_history_path.as_deref(),
                revision_history_sha256: mutation.revision_history_sha256,
                revision_history_raw: mutation.revision_history_raw.as_deref(),
            })
        })
        .collect::<DurableResult<Vec<_>>>()?;
    let claim_dependency_inputs = claim_dependency_causes
        .iter()
        .map(|cause| NavigationClaimEventRangeDependencyCauseInputV1 {
            event_sequence: cause.event_sequence,
            event_id: &cause.event_id,
            changed_dependency_path: &cause.changed_dependency_path,
            changed_dependency_sha256: cause.changed_dependency_sha256,
            predecessor_claim_ids: &cause.predecessor_claim_ids,
            current_claim_ids: &cause.current_claim_ids,
        })
        .collect::<Vec<_>>();

    let claim_inventory = if terminal_claim_stream_paths.is_empty() {
        let source_paths_present = coverage.events().iter().any(|event| {
            event
                .members
                .iter()
                .any(|member| member.is_source && member.source_path.starts_with("ToS/"))
        });
        if source_paths_present {
            let closure = claim_closure.as_ref().ok_or(DurableError::Conflict(
                "ToS source range lacks Claim closure",
            ))?;
            let lookup_keys = closure
                .lookups()
                .keys()
                .map(|(kind, key)| NavigationClaimEventRangeLookupKeyV1 {
                    kind: match kind {
                        ClaimNavigationReferenceKind::TosId => {
                            NavigationClaimEventRangeLookupKindV1::TosId
                        }
                        ClaimNavigationReferenceKind::SourcePath => {
                            NavigationClaimEventRangeLookupKindV1::SourcePath
                        }
                    },
                    key: key.clone(),
                })
                .collect::<Vec<_>>();
            tos_compiler::source_navigation_claim_event::prepare_navigation_claim_event_empty_source_range_owner_inventory_v1(
                &compiler_coverage_input,
                &claim_mutation_inputs,
                &claim_dependency_inputs,
                &claim_prior_inputs,
                NavigationClaimEventRangeEmptyClosureInputV1 {
                    closure_root: closure.closure_root(),
                    lookup_keys: &lookup_keys,
                },
            )
            .map_err(|_| DurableError::Conflict("empty Claim source range closure differs"))?
        } else {
            tos_compiler::source_navigation_claim_event::prepare_navigation_claim_event_no_projectable_tos_members_inventory_v1(&compiler_coverage_input)
                .map_err(|_| DurableError::Conflict("empty Claim no-ToS range evidence differs"))?
        }
    } else {
        let closure = claim_closure.as_ref().ok_or(DurableError::Conflict(
            "Claim owner range lacks reverse closure",
        ))?;
        let stream_paths = terminal_claim_stream_paths
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        let direct_stream_paths = range_claim_mutations
            .iter()
            .map(|mutation| mutation.claim_stream_member_path.clone())
            .collect::<BTreeSet<_>>();
        let source_files = claim_source_files_for_range(
            &context,
            closure,
            coverage,
            &stream_paths,
            &claim_endpoint_paths,
            &direct_stream_paths,
        )?;
        tos_compiler::source_navigation_claim_event::prepare_navigation_claim_event_owner_inventory_with_endpoints_v1(
            &source_files,
            &claim_endpoint_paths,
            limits,
        )
        .map_err(compiler_refusal)?
    };

    if !claim_inventory.scan_costs().is_empty() {
        let mut inventory_rows = 0u64;
        let mut inventory_bytes = 0u64;
        for cost in claim_inventory.scan_costs() {
            inventory_rows = inventory_rows
                .checked_add(cost.rows_scanned)
                .ok_or(DurableError::Refused("Claim inventory row count"))?;
            if let Some(source) = context
                .selected_sources()
                .iter()
                .find(|source| source.source_path == cost.source_path)
            {
                inventory_bytes = inventory_bytes
                    .checked_add(source.content_length.unwrap_or(0))
                    .ok_or(DurableError::Refused("Claim inventory bytes"))?;
            }
        }
        meter.admit(
            inventory_rows,
            0,
            inventory_bytes,
            inventory_rows,
            selection,
            &selection.event_limits,
        )?;
    }

    let claim_stream_predecessors = event_reader
        .read_navigation_event_range_claim_stream_predecessors_v1(
            selection.domain,
            selection.consumer_id,
            &selection.profile,
            coverage,
            &terminal_claim_stream_paths
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            selection.event_limits,
        )?;
    let claim_manifest =
        tos_compiler::source_navigation_claim_event::seal_navigation_claim_event_range_manifest_v1(
            NavigationClaimEventRangeManifestInputV1 {
                coverage: compiler_coverage_input,
                mutations: &claim_mutation_inputs,
                dependency_causes: &claim_dependency_inputs,
                prior_owners: &claim_prior_inputs,
                stream_predecessors: &claim_stream_predecessors,
                terminal_inventory: &claim_inventory,
            },
        )
        .map_err(|_| DurableError::Conflict("Claim range manifest refused"))?;

    let mut record_prior_by_key = BTreeMap::<(String, String), Digest256>::new();
    let mut record_prior_by_cause =
        BTreeMap::<String, Vec<NavigationRecordEventRangePriorOwnerInputV1>>::new();
    for row in &prior_record_owners {
        let root = match row.predecessor_basis {
            crate::durable_adapter::durable_projection::DurableNavigationEventRangePriorOwnerBasisV1::EventCohort { cohort_root } => {
                cohort_root
            }
            crate::durable_adapter::durable_projection::DurableNavigationEventRangePriorOwnerBasisV1::Bootstrap { owner_root } => {
                owner_root
            }
        };
        let key = (row.source_path.clone(), row.record_id.clone());
        if record_prior_by_key
            .insert(key.clone(), root)
            .is_some_and(|previous| previous != root)
        {
            return Err(DurableError::Conflict(
                "Record range predecessor root changed within cause set",
            ));
        }
        record_prior_by_cause
            .entry(row.cause_path.clone())
            .or_default()
            .push(NavigationRecordEventRangePriorOwnerInputV1 {
                cause_path: row.cause_path.clone(),
                source_path: row.source_path.clone(),
                record_id: row.record_id.clone(),
                predecessor_cohort_root: root,
            });
    }
    for owners in record_prior_by_cause.values_mut() {
        owners.sort_by(|left, right| {
            (&left.cause_path, &left.source_path, &left.record_id).cmp(&(
                &right.cause_path,
                &right.source_path,
                &right.record_id,
            ))
        });
        owners.dedup_by(|left, right| {
            left.cause_path == right.cause_path
                && left.source_path == right.source_path
                && left.record_id == right.record_id
        });
    }
    let mut record_prior_inputs = prior_record_owners
        .iter()
        .map(|row| {
            let predecessor_cohort_root = match row.predecessor_basis {
                crate::durable_adapter::durable_projection::DurableNavigationEventRangePriorOwnerBasisV1::EventCohort {
                    cohort_root,
                } => cohort_root,
                crate::durable_adapter::durable_projection::DurableNavigationEventRangePriorOwnerBasisV1::Bootstrap { owner_root } => {
                    owner_root
                }
            };
            NavigationRecordEventRangePriorOwnerInputV1 {
                cause_path: row.cause_path.clone(),
                source_path: row.source_path.clone(),
                record_id: row.record_id.clone(),
                predecessor_cohort_root,
            }
        })
        .collect::<Vec<_>>();
    record_prior_inputs.sort_by(|left, right| {
        (&left.cause_path, &left.source_path, &left.record_id).cmp(&(
            &right.cause_path,
            &right.source_path,
            &right.record_id,
        ))
    });
    record_prior_inputs.dedup_by(|left, right| {
        left.cause_path == right.cause_path
            && left.source_path == right.source_path
            && left.record_id == right.record_id
    });

    let claim_revision_paths = range_claim_mutations
        .iter()
        .filter(|mutation| mutation.revision_history_path.is_some())
        .map(|mutation| {
            (
                mutation.event_sequence,
                mutation.claim_stream_member_path.clone(),
            )
        })
        .collect::<BTreeSet<_>>();
    let mut record_owner_inputs = Vec::new();
    for ((source_path, record_id), record_owner) in &record_owners_by_key {
        if selected_optional_source_bytes(&context, source_path)?.is_none() {
            // An exact terminal absence retracts an old owner. Direct causes
            // are checked below and cannot silently disappear this way.
            continue;
        }
        let Some(history_bytes) =
            selected_optional_source_bytes(&context, &record_history_path(source_path)?)?
        else {
            // A source revision cause must remain present at the terminal cut.
            if range_revisions
                .iter()
                .any(|revision| revision.source_path == *source_path)
            {
                return Err(DurableError::Conflict(
                    "Record revision history absent at range terminal",
                ));
            }
            record_owner_inputs.push(NavigationRecordEventRangeOwnerInputV1 {
                key: NavigationRecordEventRangeOwnerKeyV1 {
                    source_path: source_path.clone(),
                    record_id: record_id.clone(),
                },
                terminal_history_bytes: None,
                mutations: record_mutation_inputs_for_owner(
                    record_owner,
                    source_path,
                    record_id,
                    &range_creates,
                    &range_revisions,
                )?,
            });
            continue;
        };
        record_owner_inputs.push(NavigationRecordEventRangeOwnerInputV1 {
            key: NavigationRecordEventRangeOwnerKeyV1 {
                source_path: source_path.clone(),
                record_id: record_id.clone(),
            },
            terminal_history_bytes: Some(history_bytes),
            mutations: record_mutation_inputs_for_owner(
                record_owner,
                source_path,
                record_id,
                &range_creates,
                &range_revisions,
            )?,
        });
    }

    let mut record_preview_dependencies = BTreeMap::<
        (String, String),
        Vec<tos_compiler::source_navigation_event::NavigationDependencyV1>,
    >::new();
    if !record_owner_inputs.is_empty() {
        let files = files.as_ref().ok_or(DurableError::Conflict(
            "Record range has no selected source files",
        ))?;
        with_range_schema_validator(
            owner,
            selection,
            coverage,
            &context,
            identity,
            deadline,
            cancelled,
            |validator| {
                for ((source_path, record_id), record_owner) in &record_owners_by_key {
                    let source_present =
                        selected_optional_source_bytes(&context, source_path)?.is_some();
                    if !source_present {
                        continue;
                    }
                    let input = record_owner_inputs
                        .iter()
                        .find(|input| {
                            input.key.source_path == *source_path
                                && input.key.record_id == *record_id
                        })
                        .ok_or(DurableError::Conflict(
                            "Record range terminal owner input absent",
                        ))?;
                    let predecessor = record_prior_by_key
                        .get(&(source_path.clone(), record_id.clone()))
                        .copied();
                    let origin_forms = range_creates
                        .iter()
                        .find(|created| {
                            created.owner.source_path == *source_path
                                && created.owner.record_id == *record_id
                        })
                        .and_then(|created| created.forms_bytes.as_deref());
                    let (dependencies, cost) = crate::source_navigation_record_input::preview_record_range_terminal_dependencies_v1(
                    &context,
                    identity,
                    validator,
                    files,
                    record_owner,
                    predecessor,
                    &compiler_coverage,
                    origin_forms,
                    &mut forms,
                    limits,
                )?;
                    meter.admit(
                        cost.rows,
                        cost.encoded_bytes,
                        cost.dependency_bytes,
                        cost.dependency_count,
                        selection,
                        &selection.event_limits,
                    )?;
                    record_preview_dependencies
                        .insert((source_path.clone(), record_id.clone()), dependencies);
                    let _ = input;
                }
                Ok(())
            },
        )?;
    }

    // A direct create/revision must still name a terminal owner. No deletion
    // operation in the current ToS source contract can retire one of these.
    for created in &range_creates {
        if !record_owner_inputs.iter().any(|input| {
            input.key.source_path == created.owner.source_path
                && input.key.record_id == created.owner.record_id
        }) {
            return Err(DurableError::Conflict(
                "direct Record creation is absent at terminal cut",
            ));
        }
    }
    for revision in &range_revisions {
        if !record_owner_inputs.iter().any(|input| {
            input.key.source_path == revision.owner.source_path
                && input.key.record_id == revision.owner.record_id
        }) {
            return Err(DurableError::Conflict(
                "Record revision is absent at terminal cut",
            ));
        }
    }

    let mut record_dependency_storage = Vec::<RecordRangeDependencyStorageV1>::new();
    if !record_owners_by_key.is_empty() {
        for event in coverage
            .events()
            .iter()
            .filter(|event| event.event_kind == "command")
        {
            for member in event.members.iter().filter(|member| {
                member.is_source
                    && member.source_path.starts_with("ToS/")
                    && !member.source_path.ends_with("/source-create-receipt.json")
                    && !member
                        .source_path
                        .ends_with("/source-revision-history.json")
                    && !member.source_path.ends_with("/claim-revision-history.json")
                    && !claim_revision_paths.contains(&(event.sequence, member.source_path.clone()))
            }) {
                let predecessor_owners = record_prior_by_cause
                    .get(&member.source_path)
                    .into_iter()
                    .flatten()
                    .filter(|owner| owner.source_path != member.source_path)
                    .cloned()
                    .collect::<Vec<_>>();
                let current_owners = record_owners_by_key
                    .iter()
                    .filter_map(|((source_path, record_id), _)| {
                        if source_path == &member.source_path {
                            return None;
                        }
                        let key = NavigationRecordEventRangeOwnerKeyV1 {
                            source_path: source_path.clone(),
                            record_id: record_id.clone(),
                        };
                        let dependencies = record_preview_dependencies.get(&(source_path.clone(), record_id.clone()))?;
                        dependencies.iter().any(|dependency| {
                            dependency.path.as_str() == member.source_path
                                && matches!(dependency.state, tos_compiler::source_navigation_event::NavigationDependencyStateV1::Present { .. })
                        }).then_some(key)
                    })
                    .collect::<Vec<_>>();
                if predecessor_owners.is_empty() && current_owners.is_empty() {
                    continue;
                }
                let bytes = event_reader.read_navigation_event_range_member_source_v1(
                    store,
                    selection.domain,
                    &selection.profile,
                    coverage,
                    event.sequence,
                    &member.source_path,
                    selection.event_limits,
                )?;
                record_dependency_storage.push(RecordRangeDependencyStorageV1 {
                    event_sequence: event.sequence,
                    event_id: event.event_id.clone(),
                    command_id: event.command_id.clone(),
                    path: member.source_path.clone(),
                    bytes,
                    predecessor_owners,
                    current_owners,
                });
            }
        }
    }
    let record_dependency_inputs = record_dependency_storage
        .iter()
        .map(|cause| NavigationRecordEventRangeDependencyCauseInputV1 {
            event_sequence: cause.event_sequence,
            event_id: &cause.event_id,
            command_id: cause.command_id.as_deref(),
            changed_dependency_path: &cause.path,
            changed_dependency_bytes: &cause.bytes,
            predecessor_owners: &cause.predecessor_owners,
            current_owners: &cause.current_owners,
        })
        .collect::<Vec<_>>();

    let mut claim_cause_storage = Vec::<RecordRangeClaimCauseStorageV1>::new();
    for mutation in range_claim_mutations
        .iter()
        .filter(|mutation| mutation.revision_history_path.is_some())
    {
        if mutation.mutation_claim_ids.len() != 1 {
            return Err(DurableError::Conflict(
                "Claim revision range has multiple IDs",
            ));
        }
        let claim_id = mutation.mutation_claim_ids[0].clone();
        let history_path = mutation
            .revision_history_path
            .as_deref()
            .ok_or(DurableError::Corrupt("Claim revision history path absent"))?;
        let mut predecessor_owners = prior_record_owners
            .iter()
            .filter(|owner| {
                owner.source_path != mutation.claim_stream_member_path
                    && owner.source_path != history_path
                    && (owner.cause_path == mutation.claim_stream_member_path
                        || owner.cause_path == history_path)
            })
            .map(|owner| {
                let root = match owner.predecessor_basis {
                    crate::durable_adapter::durable_projection::DurableNavigationEventRangePriorOwnerBasisV1::EventCohort {
                        cohort_root,
                    } => cohort_root,
                    crate::durable_adapter::durable_projection::DurableNavigationEventRangePriorOwnerBasisV1::Bootstrap {
                        owner_root,
                    } => owner_root,
                };
                NavigationRecordEventRangePriorOwnerInputV1 {
                    cause_path: owner.cause_path.clone(),
                    source_path: owner.source_path.clone(),
                    record_id: owner.record_id.clone(),
                    predecessor_cohort_root: root,
                }
            })
            .collect::<Vec<_>>();
        predecessor_owners.sort_by(|left, right| {
            (&left.cause_path, &left.source_path, &left.record_id).cmp(&(
                &right.cause_path,
                &right.source_path,
                &right.record_id,
            ))
        });
        if predecessor_owners.windows(2).any(|pair| {
            pair[0].cause_path == pair[1].cause_path
                && pair[0].source_path == pair[1].source_path
                && pair[0].record_id == pair[1].record_id
                && pair[0].predecessor_cohort_root != pair[1].predecessor_cohort_root
        }) {
            return Err(DurableError::Conflict(
                "Claim revision Record predecessor roots disagree",
            ));
        }
        predecessor_owners.dedup_by(|left, right| {
            left.cause_path == right.cause_path
                && left.source_path == right.source_path
                && left.record_id == right.record_id
        });
        let current_owners = record_owners_by_key
            .iter()
            .filter_map(|((source_path, record_id), _)| {
                let dependencies = record_preview_dependencies.get(&(source_path.clone(), record_id.clone()))?;
                dependencies.iter().any(|dependency| {
                    (dependency.path.as_str() == mutation.claim_stream_member_path || dependency.path.as_str() == history_path)
                        && matches!(dependency.state, tos_compiler::source_navigation_event::NavigationDependencyStateV1::Present { .. })
                }).then_some(NavigationRecordEventRangeOwnerKeyV1 { source_path: source_path.clone(), record_id: record_id.clone() })
            })
            .collect::<Vec<_>>();
        if predecessor_owners.is_empty() && current_owners.is_empty() {
            continue;
        }
        claim_cause_storage.push(RecordRangeClaimCauseStorageV1 {
            event_sequence: mutation.event_sequence,
            event_id: mutation.event_id.clone(),
            claim_id,
            predecessor_owners,
            current_owners,
        });
    }
    let record_claim_inputs = claim_cause_storage
        .iter()
        .map(|cause| NavigationRecordEventRangeClaimCauseInputV1 {
            event_sequence: cause.event_sequence,
            event_id: &cause.event_id,
            claim_id: &cause.claim_id,
            predecessor_owners: &cause.predecessor_owners,
            current_owners: &cause.current_owners,
        })
        .collect::<Vec<_>>();

    let claim_range_owner_set = if claim_manifest.terminal_owner_ids().is_empty() {
        tos_compiler::source_navigation_claim_event::seal_navigation_claim_event_range_owner_set_v1(
            &claim_manifest,
            Vec::new(),
        )
        .map_err(|_| DurableError::Conflict("empty Claim range owner set refused"))?
    } else {
        let files = files.as_ref().ok_or(DurableError::Conflict(
            "Claim range has no selected source files",
        ))?;
        let mut contributions = Vec::new();
        with_range_schema_validator(
            owner,
            selection,
            coverage,
            &context,
            identity,
            deadline,
            cancelled,
            |validator| {
                for stream in claim_manifest.terminal_streams() {
                    for claim_id in stream.owner_ids() {
                        let predecessor = claim_manifest
                            .stream_predecessors()
                            .iter()
                            .find(|owner| {
                                owner.claim_stream_path == stream.source_path()
                                    && owner.claim_id == *claim_id
                            })
                            .and_then(|owner| match owner.basis {
                                NavigationClaimEventRangePriorOwnerBasisV1::EventCohort {
                                    cohort_root,
                                } => Some(cohort_root),
                                NavigationClaimEventRangePriorOwnerBasisV1::Bootstrap {
                                    ..
                                } => None,
                            });
                        let (cohort, cost) = crate::source_navigation_claim_input::assemble_selected_claim_event_range_owner_v1(
                        &context,
                        identity,
                        validator,
                        files,
                        &claim_inventory,
                        &claim_manifest,
                        &claim_identity_owners,
                        stream.source_path(),
                        claim_id,
                        predecessor,
                        &mut forms,
                        limits,
                    )?;
                        meter.admit(
                            cost.rows,
                            cost.encoded_bytes,
                            cost.dependency_bytes,
                            cost.claim_stream_rows_scanned,
                            selection,
                            &selection.event_limits,
                        )?;
                        contributions.push(cohort);
                    }
                }
                Ok(())
            },
        )?;
        tos_compiler::source_navigation_claim_event::seal_navigation_claim_event_range_owner_set_v1(
            &claim_manifest,
            contributions,
        )
        .map_err(|_| DurableError::Conflict("Claim range owner set refused"))?
    };

    let record_range_set = if record_owner_inputs.is_empty() {
        tos_compiler::source_navigation_record_range::project_navigation_record_range_with_rows_v1(
            NavigationRecordEventRangeInputV1 {
                coverage: &compiler_coverage,
                owners: Vec::new(),
                prior_owners: &record_prior_inputs,
                dependency_causes: &record_dependency_inputs,
                claim_manifest: Some(&claim_manifest),
                claim_causes: &record_claim_inputs,
            },
            |_key, _predecessor, _cause_root| {
                Err(tos_compiler::Error::Invalid("unexpected empty Record row"))
            },
        )
        .map_err(|_| DurableError::Conflict("empty Record range set refused"))?
    } else {
        let files = files
            .as_ref()
            .ok_or(DurableError::Conflict("Record range has no selected files"))?;
        let owners_input = record_owner_inputs;
        let mut project_error = None;
        let set_result = with_range_schema_validator(
            owner,
            selection,
            coverage,
            &context,
            identity,
            deadline,
            cancelled,
            |validator| {
                tos_compiler::source_navigation_record_range::project_navigation_record_range_with_rows_v1(
                NavigationRecordEventRangeInputV1 {
                    coverage: &compiler_coverage,
                    owners: owners_input,
                    prior_owners: &record_prior_inputs,
                    dependency_causes: &record_dependency_inputs,
                    claim_manifest: Some(&claim_manifest),
                    claim_causes: &record_claim_inputs,
                },
                |key, predecessor, cause_root| {
                    let owner_record = record_owners_by_key.get(&(key.source_path.clone(), key.record_id.clone()))
                        .ok_or(tos_compiler::Error::Invalid("Record range terminal owner disappeared"))?;
                    let origin_forms = range_creates.iter().find(|created| created.owner.source_path == key.source_path && created.owner.record_id == key.record_id).and_then(|created| created.forms_bytes.as_deref());
                    match crate::source_navigation_record_input::assemble_record_range_terminal_cohort_v1(
                        &context,
                        identity,
                        validator,
                        files,
                        owner_record,
                        predecessor,
                        &compiler_coverage,
                        cause_root,
                        origin_forms,
                        &mut forms,
                        limits,
                    ) {
                        Ok((cohort, cost)) => {
                            if let Err(error) = meter.admit(cost.rows, cost.encoded_bytes, cost.dependency_bytes, cost.dependency_count, selection, &selection.event_limits) {
                                project_error = Some(error);
                                return Err(tos_compiler::Error::Invalid("Record range aggregate budget"));
                            }
                            Ok(cohort)
                        }
                        Err(error) => {
                            project_error = Some(error);
                            Err(tos_compiler::Error::Invalid("Record range terminal projection refused"))
                        }
                    }
                },
            )
            .map_err(|_| DurableError::Conflict("Record range set refused"))
            },
        )?;
        if let Some(error) = project_error {
            return Err(error);
        }
        set_result
    };

    check_active()?;
    let apply = DurablePgCoordinator::publish_navigation_event_range_cohort_set_v1_in_tx(
        owner_tx,
        store,
        selection.domain,
        selection.consumer_id,
        &selection.profile,
        coverage,
        &context,
        &record_range_set,
        &claim_range_owner_set,
        claim_closure.as_ref(),
        identity.event_source_revision(),
        owner.selected_audit_generation,
        selection.source_contract,
        selection.rule_version,
        selection.rights_version,
        selection.job_id,
        selection.job_fence,
        selection.event_limits,
    )?;
    drop(held_readsets);

    let selected_source_count = context
        .selected_sources()
        .iter()
        .filter(|source| source.is_source)
        .count();
    let selected_source_bytes = context
        .selected_sources()
        .iter()
        .filter(|source| source.is_source)
        .try_fold(0u64, |total, source| {
            total
                .checked_add(source.content_length.unwrap_or(0))
                .ok_or(DurableError::Refused(
                    "selected navigation source byte count",
                ))
        })?;
    let range_member_count = coverage
        .events()
        .iter()
        .map(|event| event.members.len())
        .sum::<usize>();
    let range_member_bytes = coverage.events().iter().try_fold(0u64, |total, event| {
        event.members.iter().try_fold(total, |sum, member| {
            sum.checked_add(member.size_bytes)
                .ok_or(DurableError::Refused("navigation range member byte count"))
        })
    })?;
    Ok((
        json!({
            "cursor_before": apply.cursor_before,
            "through_sequence": apply.through_sequence,
            "terminal_event_id": apply.terminal_event_id,
            "coverage_root": apply.coverage_root.to_hex(),
            "terminal_cohort_root": apply.terminal_cohort_root.to_hex(),
            "record_owner_set_root": record_range_set.root().to_hex(),
            "claim_owner_set_root": claim_range_owner_set.root().to_hex(),
            "claim_range_manifest_root": claim_manifest.root().to_hex(),
            "record_replaced_owner_count": record_range_set.rows().len(),
            "claim_replaced_owner_count": claim_range_owner_set.cohorts().len(),
            "record_removed_owner_count": record_range_set.removed_owners().len(),
            "claim_removed_owner_count": claim_range_owner_set.removed_owners().len(),
            "replacement_owner_count": apply.replacement_owner_count,
            "replayed": apply.replayed,
        }),
        json!({
            "events": coverage.events().len(),
            "cursor_lag_events": coverage.held_source_head_sequence().saturating_sub(coverage.cursor_before()),
            "range_event_count": coverage.events().len(),
            "range_member_count": range_member_count,
            "range_member_bytes": range_member_bytes,
            "terminal_selected_source_count": selected_source_count,
            "terminal_selected_source_bytes": selected_source_bytes,
            "source_members": coverage.events().iter().map(|event| event.members.iter().filter(|member| member.is_source).count()).sum::<usize>(),
            "source_bytes": range_source_bytes,
            "selected_source_count": selected_source_count,
            "selected_source_bytes": selected_source_bytes,
            "record_candidates": record_owners_by_key.len(),
            "record_rows": record_range_set.rows().len(),
            "claim_streams": claim_manifest.terminal_streams().len(),
            "claim_rows": claim_range_owner_set.cohorts().len(),
            "max_events": selection.max_events,
        }),
    ))
}

/// Seal the durable owner's exact contiguous range in the compiler's distinct
/// range type. This keeps event identities, command identities, and each
/// event's member set separate from the terminal current-source context.
fn seal_compiler_range_coverage(
    coverage: &crate::durable_adapter::durable_projection::DurableNavigationEventRangeCoverageV1,
) -> DurableResult<tos_compiler::source_navigation_event::NavigationEventRangeCoverageV1> {
    use tos_compiler::source_navigation_event::{
        NavigationEventRangeCoverageInputV1, NavigationEventRangeEventV1,
        NavigationEventRangeMemberV1, seal_navigation_event_range_coverage_v1,
    };

    let events = coverage
        .events()
        .iter()
        .map(|event| NavigationEventRangeEventV1 {
            sequence: event.sequence,
            event_id: event.event_id.clone(),
            command_id: event.command_id.clone(),
            event_kind: event.event_kind.clone(),
            member_root: event.member_root,
            members: event
                .members
                .iter()
                .map(|member| NavigationEventRangeMemberV1 {
                    source_path: member.source_path.clone(),
                    revision: member.revision,
                    is_source: member.is_source,
                    size_bytes: member.size_bytes,
                    sha256: member.sha256,
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    seal_navigation_event_range_coverage_v1(NavigationEventRangeCoverageInputV1 {
        cursor_before: coverage.cursor_before(),
        through_sequence: coverage.through_sequence(),
        terminal_event_id: coverage.terminal_event_id(),
        events: &events,
        coverage_root: coverage.coverage_root(),
        manifest_raw: coverage.manifest_raw(),
    })
    .map_err(|_| DurableError::Conflict("navigation event range coverage reseal differs"))
}

fn read_current_range_context(
    event_reader: &mut DurablePgCoordinator,
    store: &SegmentStore,
    owner: &NavigationProducerHeldOwnerIdentity,
    selection: &NavigationProducerOwnerSelection<'_>,
    coverage: &crate::durable_adapter::durable_projection::DurableNavigationEventRangeCoverageV1,
    paths: &BTreeSet<String>,
) -> DurableResult<DurableNavigationEventContextV1> {
    let terminal_has_source = coverage
        .events()
        .last()
        .is_some_and(|event| event.members.iter().any(|member| member.is_source));
    let mut selected_paths = paths.clone();
    if !terminal_has_source {
        if let Some(terminal) = coverage.events().last() {
            extend_event_paths(
                &mut selected_paths,
                terminal
                    .members
                    .iter()
                    .map(|member| member.source_path.clone()),
                selection.event_limits.max_dependencies,
            )?;
        }
    }
    // The configured event file is a source-cut selection anchor, not a claim
    // that a control event has that event-member path. Read it only when the
    // caller needs a source context or the terminal event has source members.
    if !selected_paths.is_empty() || terminal_has_source {
        extend_event_paths(
            &mut selected_paths,
            [selection.event_path.to_owned()],
            selection.event_limits.max_dependencies,
        )?;
    }
    let selected_path_set = selected_paths;
    let source_cut_anchor_path = if selected_path_set.is_empty() {
        ""
    } else {
        selection.event_path
    };
    let selected_paths = selected_path_set.iter().cloned().collect::<Vec<_>>();
    let context = event_reader.read_navigation_event_range_current_context_v1(
        store,
        selection.domain,
        selection.consumer_id,
        source_cut_anchor_path,
        &selected_paths,
        &selection.profile,
        coverage,
        selection.event_limits,
    )?;
    verify_range_terminal_context(owner, selection, coverage, &context, &selected_path_set)?;
    Ok(context)
}

fn stage_range_context(
    event_reader: &mut DurablePgCoordinator,
    store: &SegmentStore,
    owner: &NavigationProducerHeldOwnerIdentity,
    selection: &NavigationProducerOwnerSelection<'_>,
    coverage: &crate::durable_adapter::durable_projection::DurableNavigationEventRangeCoverageV1,
    context: &mut DurableNavigationEventContextV1,
    additions: impl IntoIterator<Item = String>,
) -> DurableResult<()> {
    let additions = additions.into_iter().collect::<BTreeSet<_>>();
    let terminal_has_source = coverage
        .events()
        .last()
        .is_some_and(|event| event.members.iter().any(|member| member.is_source));
    let mut paths = BTreeSet::new();
    extend_event_paths(
        &mut paths,
        context
            .selected_sources()
            .iter()
            .map(|source| source.source_path.clone()),
        selection.event_limits.max_dependencies,
    )?;
    extend_event_paths(
        &mut paths,
        additions,
        selection.event_limits.max_dependencies,
    )?;
    if !paths.is_empty() || terminal_has_source {
        extend_event_paths(
            &mut paths,
            [selection.event_path.to_owned()],
            selection.event_limits.max_dependencies,
        )?;
    }
    *context = read_current_range_context(event_reader, store, owner, selection, coverage, &paths)?;
    Ok(())
}

fn verify_range_terminal_context(
    owner: &NavigationProducerHeldOwnerIdentity,
    selection: &NavigationProducerOwnerSelection<'_>,
    coverage: &crate::durable_adapter::durable_projection::DurableNavigationEventRangeCoverageV1,
    context: &DurableNavigationEventContextV1,
    requested_paths: &BTreeSet<String>,
) -> DurableResult<()> {
    let terminal = coverage.events().last().ok_or(DurableError::Corrupt(
        "navigation event range terminal event is absent",
    ))?;
    let empty_terminal_no_source =
        terminal.members.iter().all(|member| !member.is_source) && context.members().is_empty();
    if coverage.through_sequence() > coverage.held_source_head_sequence()
        || coverage.held_source_head_sequence() != owner.source_head_sequence
        || context.event_id() != coverage.terminal_event_id()
        || context.event_sequence() != coverage.through_sequence()
        || context.source_head_sequence() != owner.source_head_sequence
        || context.source_head_sequence() != owner.source_generation
        || context.consumer_cursor().sequence != coverage.cursor_before()
        || terminal.event_id != context.event_id()
        || terminal.event_kind != context.event_kind()
        || (!empty_terminal_no_source && context.members().len() != terminal.members.len())
    {
        return Err(DurableError::Conflict(
            "navigation terminal source context differs from exact event range",
        ));
    }
    for member in terminal
        .members
        .iter()
        .filter(|_| !empty_terminal_no_source)
    {
        let selected_member = context
            .members()
            .iter()
            .find(|selected| selected.source_path == member.source_path)
            .ok_or(DurableError::Conflict(
                "terminal navigation member left the exact event range",
            ))?;
        if selected_member.revision != Some(member.revision)
            || selected_member.is_source != member.is_source
            || selected_member.content_length != Some(member.size_bytes)
            || selected_member.content_digest != Some(member.sha256)
            || selected_member.commit_sequence != Some(coverage.through_sequence())
        {
            return Err(DurableError::Conflict(
                "terminal navigation member differs from range coverage",
            ));
        }
    }
    let mut allowed_selected_paths = requested_paths.clone();
    allowed_selected_paths.extend(
        context
            .members()
            .iter()
            .map(|member| member.source_path.clone()),
    );
    if context
        .selected_sources()
        .iter()
        .any(|source| !allowed_selected_paths.contains(&source.source_path))
    {
        return Err(DurableError::Conflict(
            "terminal navigation selection contains an unrequested dependency",
        ));
    }
    for path in requested_paths {
        let selected = context
            .selected_sources()
            .iter()
            .find(|source| source.source_path == *path)
            .ok_or(DurableError::Conflict(
                "terminal navigation dependency path is not selected",
            ))?;
        if let Some(bytes) = selected.exact_source_bytes.as_deref() {
            if selected.content_digest != Some(Digest256::of_bytes(bytes))
                || selected.content_length != Some(bytes.len() as u64)
            {
                return Err(DurableError::Conflict(
                    "terminal navigation dependency bytes differ from their pin",
                ));
            }
        }
        if let Some(member) = coverage.events().iter().rev().find_map(|event| {
            event
                .members
                .iter()
                .find(|member| member.source_path == *path)
                .map(|member| (event.sequence, member))
        }) {
            let selected = context
                .selected_sources()
                .iter()
                .find(|source| source.source_path == *path)
                .ok_or(DurableError::Conflict(
                    "range changed navigation dependency is not selected",
                ))?;
            if selected.is_source != member.1.is_source
                || selected.commit_sequence != Some(member.0)
                || selected.revision != Some(member.1.revision)
                || selected.content_length != Some(member.1.size_bytes)
                || selected.content_digest != Some(member.1.sha256)
            {
                return Err(DurableError::Conflict(
                    "terminal navigation dependency differs from its latest range member",
                ));
            }
        }
    }
    if owner.domain != selection.domain
        || owner.domain_sha256 != Digest256::of_bytes(selection.domain.as_bytes())
        || owner.source_contract != selection.source_contract
        || owner.owner_configuration_sha256 != selection.owner_configuration_sha256
    {
        return Err(DurableError::Conflict(
            "navigation range owner differs from the protected selection",
        ));
    }
    Ok(())
}

/// Collect every direct Claim stream mutation in a complete event range at
/// its exact historical event cut. Receipt and stream bytes are never taken
/// from the terminal context, which may already have superseded them.
fn direct_claim_mutations_for_range(
    event_reader: &mut DurablePgCoordinator,
    store: &SegmentStore,
    selection: &NavigationProducerOwnerSelection<'_>,
    coverage: &crate::durable_adapter::durable_projection::DurableNavigationEventRangeCoverageV1,
) -> DurableResult<
    Vec<crate::durable_adapter::durable_projection::DurableNavigationEventRangeClaimMutationV1>,
> {
    let mut mutations = Vec::new();
    for event in coverage.events() {
        if event.event_kind != "command" {
            continue;
        }
        let changed_paths = event
            .members
            .iter()
            .filter(|member| member.is_source)
            .map(|member| member.source_path.as_str())
            .collect::<BTreeSet<_>>();
        let streams = changed_paths
            .iter()
            .copied()
            .filter(|path| path.ends_with("/source-claims.jsonl"))
            .collect::<Vec<_>>();
        let receipt_paths = changed_paths
            .iter()
            .copied()
            .filter(|path| {
                path.ends_with("/source-create-receipt.json")
                    || path.ends_with("/claim-revision-history.json")
            })
            .collect::<BTreeSet<_>>();
        if streams.is_empty() {
            // Record create/revision operations use the same receipt and
            // history filenames. Only a matching changed Claim stream makes
            // one of those members a Claim mutation receipt.
            continue;
        }

        let claim_receipt_paths = streams
            .iter()
            .map(|stream_path| {
                let home = stream_path
                    .rsplit_once('/')
                    .map(|(home, _)| home)
                    .ok_or(DurableError::Invalid("Claim mutation stream home"))?;
                Ok([
                    format!("{home}/source-create-receipt.json"),
                    format!("{home}/claim-revision-history.json"),
                ])
            })
            .collect::<DurableResult<Vec<_>>>()?
            .into_iter()
            .flatten()
            .filter(|path| receipt_paths.contains(path.as_str()))
            .collect::<BTreeSet<_>>();
        let mut matched_receipts = BTreeSet::new();
        for stream_path in streams {
            let home = stream_path
                .rsplit_once('/')
                .map(|(home, _)| home)
                .ok_or(DurableError::Invalid("Claim mutation stream home"))?;
            let candidates = [
                format!("{home}/source-create-receipt.json"),
                format!("{home}/claim-revision-history.json"),
            ];
            let receipts = candidates
                .iter()
                .filter(|path| receipt_paths.contains(path.as_str()))
                .collect::<Vec<_>>();
            if receipts.len() != 1 {
                return Err(DurableError::Conflict(
                    "range Claim mutation lacks one exact receipt member",
                ));
            }
            let receipt_path = receipts[0].as_str();
            if !matched_receipts.insert(receipt_path.to_owned()) {
                return Err(DurableError::Conflict(
                    "range Claim receipt is bound to multiple streams",
                ));
            }

            let stream_raw = event_reader.read_navigation_event_range_member_source_v1(
                store,
                selection.domain,
                &selection.profile,
                coverage,
                event.sequence,
                stream_path,
                selection.event_limits,
            )?;
            let receipt_raw = event_reader.read_navigation_event_range_member_source_v1(
                store,
                selection.domain,
                &selection.profile,
                coverage,
                event.sequence,
                receipt_path,
                selection.event_limits,
            )?;
            if stream_raw.len() > selection.event_limits.max_row_bytes
                || receipt_raw.len() > selection.event_limits.max_row_bytes
            {
                return Err(DurableError::Refused(
                    "range Claim mutation member byte budget",
                ));
            }
            let mutation_claim_ids =
                crate::source_navigation_claim_selection::claim_event_mutation_claim_ids_v1(
                    &receipt_raw,
                    selection.event_limits.max_row_bytes,
                )?;
            let revision_history = receipt_path.ends_with("/claim-revision-history.json");
            if revision_history && mutation_claim_ids.len() != 1 {
                return Err(DurableError::Conflict(
                    "range Claim revision history does not bind one Claim",
                ));
            }
            let stream_member =
                coverage
                    .member(event.sequence, stream_path)
                    .ok_or(DurableError::Conflict(
                        "range Claim stream member left exact coverage",
                    ))?;
            let receipt_member =
                coverage
                    .member(event.sequence, receipt_path)
                    .ok_or(DurableError::Conflict(
                        "range Claim receipt member left exact coverage",
                    ))?;
            mutations.push(crate::durable_adapter::durable_projection::DurableNavigationEventRangeClaimMutationV1 {
                event_sequence: event.sequence,
                event_id: event.event_id.clone(),
                claim_stream_member_path: stream_path.to_owned(),
                claim_stream_sha256: stream_member.sha256,
                claim_stream_raw: stream_raw,
                receipt_path: receipt_path.to_owned(),
                receipt_sha256: receipt_member.sha256,
                receipt_raw: receipt_raw.clone(),
                mutation_claim_ids,
                revision_history_path: revision_history.then(|| receipt_path.to_owned()),
                revision_history_sha256: revision_history.then_some(receipt_member.sha256),
                revision_history_raw: revision_history.then_some(receipt_raw),
            });
        }
        if matched_receipts != claim_receipt_paths {
            return Err(DurableError::Conflict(
                "range Claim event contains an unbound receipt member",
            ));
        }
    }
    mutations.sort_by(|left, right| {
        left.event_sequence
            .cmp(&right.event_sequence)
            .then_with(|| {
                left.claim_stream_member_path
                    .cmp(&right.claim_stream_member_path)
            })
    });
    if mutations.windows(2).any(|pair| {
        (pair[0].event_sequence, &pair[0].claim_stream_member_path)
            >= (pair[1].event_sequence, &pair[1].claim_stream_member_path)
    }) {
        return Err(DurableError::Corrupt(
            "range Claim mutations are not uniquely ordered",
        ));
    }
    Ok(mutations)
}

struct RecordRangeDependencyStorageV1 {
    event_sequence: u64,
    event_id: String,
    command_id: Option<String>,
    path: String,
    bytes: Vec<u8>,
    predecessor_owners: Vec<
        tos_compiler::source_navigation_record_range::NavigationRecordEventRangePriorOwnerInputV1,
    >,
    current_owners:
        Vec<tos_compiler::source_navigation_record_range::NavigationRecordEventRangeOwnerKeyV1>,
}

struct RecordRangeClaimCauseStorageV1 {
    event_sequence: u64,
    event_id: String,
    claim_id: String,
    predecessor_owners: Vec<
        tos_compiler::source_navigation_record_range::NavigationRecordEventRangePriorOwnerInputV1,
    >,
    current_owners:
        Vec<tos_compiler::source_navigation_record_range::NavigationRecordEventRangeOwnerKeyV1>,
}

fn record_mutation_inputs_for_owner<'a>(
    owner: &crate::source_navigation_record_input::RecordOwnerV1,
    source_path: &str,
    record_id: &str,
    creates: &'a [crate::source_navigation_record_input::RecordRangeSourceCreateV1],
    revisions: &'a [crate::source_navigation_record_input::RecordRangeRevisionV1],
) -> DurableResult<Vec<tos_compiler::source_navigation_record_range::NavigationRecordEventRangeMutationCauseInputV1<'a>>>{
    use tos_compiler::source_navigation_record_range::{
        NavigationRecordEventRangeMutationCauseInputV1 as Cause,
        NavigationRecordEventRangeMutationKindInputV1 as Kind,
    };

    if owner.source_path != source_path || owner.record_id != record_id {
        return Err(DurableError::Conflict(
            "Record range mutation lookup differs from owner key",
        ));
    }
    let mut mutations = Vec::new();
    for create in creates.iter().filter(|create| {
        create.owner.source_path == source_path && create.owner.record_id == record_id
    }) {
        if owner.origin
            != crate::source_navigation_record_input::RecordOwnerOriginV1::DirectSourceCreate
        {
            return Err(DurableError::Conflict(
                "Record direct creation owner origin differs",
            ));
        }
        mutations.push(Cause {
            event_sequence: create.event_sequence,
            event_id: &create.event_id,
            command_id: &create.command_id,
            kind: Kind::DirectCreate {
                source_path: &create.source_path,
                source_bytes: &create.source_bytes,
                receipt_path: &create.receipt_path,
                receipt_bytes: &create.receipt_bytes,
                provenance_event_path: &create.provenance_event_path,
                provenance_event_bytes: &create.provenance_event_bytes,
                origin_forms_path: create.forms_path.as_deref(),
                origin_forms_bytes: create.forms_bytes.as_deref(),
            },
        });
    }
    for revision in revisions.iter().filter(|revision| {
        revision.owner.source_path == source_path && revision.owner.record_id == record_id
    }) {
        mutations.push(Cause {
            event_sequence: revision.event_sequence,
            event_id: &revision.event_id,
            command_id: &revision.command_id,
            kind: Kind::Revision {
                source_path: &revision.source_path,
                source_bytes: &revision.source_bytes,
                history_path: &revision.history_path,
                history_bytes: &revision.history_bytes,
            },
        });
    }
    mutations.sort_by_key(|cause| cause.event_sequence);
    if mutations
        .windows(2)
        .any(|pair| pair[0].event_sequence >= pair[1].event_sequence)
    {
        return Err(DurableError::Conflict(
            "Record range mutation events repeat or are unordered",
        ));
    }
    Ok(mutations)
}

fn record_history_path(source_path: &str) -> DurableResult<String> {
    let source_path = RelativePath::parse(source_path)
        .map_err(|_| DurableError::Invalid("navigation Record source path"))?;
    let (home, basename) = source_path
        .as_str()
        .rsplit_once('/')
        .ok_or(DurableError::Invalid("navigation Record source home"))?;
    if basename.strip_suffix(".json").is_none() {
        return Err(DurableError::Invalid("navigation Record source suffix"));
    }
    Ok(format!("{home}/source-revision-history.json"))
}

fn selected_optional_source_bytes<'a>(
    context: &'a DurableNavigationEventContextV1,
    path: &str,
) -> DurableResult<Option<&'a [u8]>> {
    let mut selected = context
        .selected_sources()
        .iter()
        .filter(|source| source.source_path == path);
    let source = selected.next().ok_or(DurableError::Conflict(
        "navigation terminal path was not selected",
    ))?;
    if selected.next().is_some() {
        return Err(DurableError::Corrupt(
            "navigation terminal path selection is ambiguous",
        ));
    }
    if source.is_source {
        return selected_source_bytes(context, path).map(Some);
    }
    if source.exact_source_bytes.is_some()
        || source.content_digest.is_some()
        || source.content_length.is_some()
    {
        return Err(DurableError::Conflict(
            "navigation terminal absence has content metadata",
        ));
    }
    Ok(None)
}

/// The source identity index is an append-only navigation aid and is read at
/// the held head. A range terminal may be an earlier cut, so a current index
/// answer is usable only when that exact typed identity still exists at the
/// terminal path with matching bytes. This rejects future-only identity
/// lookahead instead of letting it alter the prefix Claim projection.
fn verify_range_identity_owners_at_cut(
    context: &DurableNavigationEventContextV1,
    owners: &[crate::source_navigation_claim_selection::ClaimEventIdentityOwnerV1],
    max_source_bytes: u64,
) -> DurableResult<()> {
    for owner in owners {
        match (owner.kind.as_deref(), owner.source_path.as_deref()) {
            (None, None) => continue,
            (Some(kind), Some(path)) => {
                let parsed = RelativePath::parse(path)
                    .map_err(|_| DurableError::Corrupt("Claim identity source path"))?;
                if parsed.as_str() != path {
                    return Err(DurableError::Conflict(
                        "Claim identity source path is not canonical",
                    ));
                }
                if !path.starts_with("ToS/") {
                    return Err(DurableError::Conflict(
                        "Claim identity source path is outside ToS",
                    ));
                }
                let raw = selected_optional_source_bytes(context, path)?.ok_or(
                    DurableError::Conflict(
                        "held-head Claim identity is absent at the terminal source cut",
                    ),
                )?;
                if raw.len() as u64 > max_source_bytes {
                    return Err(DurableError::Refused(
                        "Claim identity terminal source byte budget",
                    ));
                }
                let matches = match kind {
                    "metadata" => {
                        let value: Value = serde_json::from_slice(raw)
                            .map_err(|_| DurableError::Invalid("Claim identity metadata JSON"))?;
                        value
                            .get("record_id")
                            .or_else(|| value.get("artifact_id"))
                            .or_else(|| value.get("composite_id"))
                            .and_then(Value::as_str)
                            == Some(owner.id.as_str())
                    }
                    "claim" => range_jsonl_identity_matches(raw, "claim_id", &owner.id)?,
                    "event" => range_jsonl_identity_matches(raw, "event_id", &owner.id)?,
                    "anchor" => range_jsonl_identity_matches(raw, "anchor_id", &owner.id)?,
                    "form" => {
                        let value: Value = serde_json::from_slice(raw)
                            .map_err(|_| DurableError::Invalid("Claim identity form JSON"))?;
                        ["forms", "prior_forms"]
                            .iter()
                            .flat_map(|section| {
                                value
                                    .get(*section)
                                    .and_then(Value::as_array)
                                    .into_iter()
                                    .flatten()
                            })
                            .filter(|entry| {
                                entry.get("form_id").and_then(Value::as_str)
                                    == Some(owner.id.as_str())
                            })
                            .count()
                            == 1
                    }
                    _ => {
                        return Err(DurableError::Conflict(
                            "Claim identity index kind is unsupported at terminal cut",
                        ));
                    }
                };
                if !matches {
                    return Err(DurableError::Conflict(
                        "held-head Claim identity differs from the terminal typed source",
                    ));
                }
            }
            _ => {
                return Err(DurableError::Conflict(
                    "Claim identity kind and path presence disagree",
                ));
            }
        }
    }
    Ok(())
}

fn range_jsonl_identity_matches(raw: &[u8], id_field: &str, expected: &str) -> DurableResult<bool> {
    let mut match_count = 0usize;
    for line in raw.split(|byte| *byte == b'\n') {
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let value: Value = serde_json::from_slice(line)
            .map_err(|_| DurableError::Invalid("Claim identity JSONL source"))?;
        if value.get(id_field).and_then(Value::as_str) == Some(expected) {
            match_count = match_count
                .checked_add(1)
                .ok_or(DurableError::Refused("Claim identity match count"))?;
        }
    }
    Ok(match_count == 1)
}

fn claim_source_files_for_range<'a>(
    context: &'a DurableNavigationEventContextV1,
    closure: &crate::source_cohort::revision::SourceRevisionClaimNavigationClosure,
    coverage: &crate::durable_adapter::durable_projection::DurableNavigationEventRangeCoverageV1,
    current_stream_paths: &'a [String],
    historical_endpoint_paths: &'a [String],
    direct_mutation_stream_paths: &BTreeSet<String>,
) -> DurableResult<Vec<NavigationClaimEventSourceFileV1<'a>>> {
    let mut paths = BTreeSet::new();
    for path in current_stream_paths.iter().chain(historical_endpoint_paths) {
        if !paths.insert(path.as_str()) {
            return Err(DurableError::Conflict(
                "Claim range inventory source path is repeated",
            ));
        }
    }
    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
        let bytes = selected_source_bytes(context, path)?;
        let selected = context
            .selected_sources()
            .iter()
            .find(|source| source.source_path == path)
            .ok_or(DurableError::Conflict(
                "Claim range inventory source is not selected",
            ))?;
        if !selected.is_source
            || selected.content_digest != Some(Digest256::of_bytes(bytes))
            || selected.content_length != Some(bytes.len() as u64)
        {
            return Err(DurableError::Conflict(
                "Claim range inventory source bytes differ from terminal cut",
            ));
        }
        if let Some(fact) = closure.member_facts().get(path) {
            if selected.commit_sequence != Some(fact.commit_seq)
                || selected.content_digest != Some(fact.digest)
                || selected.content_length != Some(fact.size_bytes)
                || fact.digest != Digest256::of_bytes(bytes)
                || fact.size_bytes != bytes.len() as u64
            {
                return Err(DurableError::Conflict(
                    "Claim range stream bytes differ from as-of reverse closure",
                ));
            }
        } else if direct_mutation_stream_paths.contains(path) {
            let latest_member = coverage.events().iter().rev().find_map(|event| {
                event
                    .members
                    .iter()
                    .find(|member| member.source_path == path)
                    .map(|member| (event.sequence, member))
            });
            let Some((sequence, member)) = latest_member else {
                return Err(DurableError::Conflict(
                    "direct Claim range stream lacks a historical event member",
                ));
            };
            if !member.is_source
                || selected.commit_sequence != Some(sequence)
                || selected.content_digest != Some(member.sha256)
                || selected.content_length != Some(member.size_bytes)
                || member.sha256 != Digest256::of_bytes(bytes)
                || member.size_bytes != bytes.len() as u64
            {
                return Err(DurableError::Conflict(
                    "direct Claim range stream differs from latest covered mutation",
                ));
            }
        }
        files.push(NavigationClaimEventSourceFileV1 { path, bytes });
    }
    Ok(files)
}

/// Bind every changed ToS source member to both cursor-before Claim owners and
/// the held terminal reverse-Claim lookup. Repeated source paths remain
/// distinct event causes, each carrying those same two boundary observations;
/// no intermediate reverse-index state is inferred from the terminal cut.
fn range_dependency_causes(
    coverage: &crate::durable_adapter::durable_projection::DurableNavigationEventRangeCoverageV1,
    prior_owners: &[crate::durable_adapter::durable_projection::DurableNavigationEventRangePriorClaimOwnerV1],
    closure: &crate::source_cohort::revision::SourceRevisionClaimNavigationClosure,
    ids_by_path: &BTreeMap<String, BTreeSet<String>>,
) -> DurableResult<
    Vec<crate::durable_adapter::durable_projection::DurableNavigationEventRangeDependencyCauseV1>,
> {
    use crate::source_claim_dependency_index::ClaimNavigationReferenceKind;

    let mut causes = Vec::new();
    for event in coverage.events() {
        if event.event_kind != "command" {
            continue;
        }
        for member in event.members.iter().filter(|member| {
            member.is_source
                && member.source_path.starts_with("ToS/")
                && !member.source_path.ends_with("/source-claims.jsonl")
                && !member.source_path.ends_with("/source-create-receipt.json")
                && !member.source_path.ends_with("/claim-revision-history.json")
        }) {
            let mut predecessor_roots = prior_owners
                .iter()
                .filter(|owner| owner.cause_path == member.source_path)
                .cloned()
                .collect::<Vec<_>>();
            predecessor_roots.sort_by(|left, right| {
                left.claim_stream_path
                    .cmp(&right.claim_stream_path)
                    .then_with(|| left.claim_id.cmp(&right.claim_id))
            });
            if predecessor_roots.windows(2).any(|pair| {
                (
                    pair[0].claim_stream_path.as_str(),
                    pair[0].claim_id.as_str(),
                ) >= (
                    pair[1].claim_stream_path.as_str(),
                    pair[1].claim_id.as_str(),
                )
            }) {
                return Err(DurableError::Corrupt(
                    "range dependency predecessor owners repeat",
                ));
            }
            let predecessor_claim_ids = predecessor_roots
                .iter()
                .map(|owner| owner.claim_id.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();

            let mut current_claim_ids = BTreeSet::new();
            if let Some(observation) = closure.lookups().get(&(
                ClaimNavigationReferenceKind::SourcePath,
                member.source_path.clone(),
            )) {
                current_claim_ids.extend(observation.rows.iter().map(|row| row.claim_id.clone()));
            } else {
                return Err(DurableError::Conflict(
                    "range dependency source-path reverse lookup is absent",
                ));
            }
            if let Some(changed_ids) = ids_by_path.get(&member.source_path) {
                for changed_id in changed_ids {
                    let observation = closure
                        .lookups()
                        .get(&(ClaimNavigationReferenceKind::TosId, changed_id.clone()))
                        .ok_or(DurableError::Conflict(
                            "range dependency ToS-ID reverse lookup is absent",
                        ))?;
                    current_claim_ids
                        .extend(observation.rows.iter().map(|row| row.claim_id.clone()));
                }
            }
            causes.push(crate::durable_adapter::durable_projection::DurableNavigationEventRangeDependencyCauseV1 {
                event_sequence: event.sequence,
                event_id: event.event_id.clone(),
                changed_dependency_path: member.source_path.clone(),
                changed_dependency_sha256: member.sha256,
                predecessor_claim_ids,
                current_claim_ids: current_claim_ids.into_iter().collect(),
                predecessor_owner_roots: predecessor_roots,
            });
        }
    }
    causes.sort_by(|left, right| {
        left.event_sequence
            .cmp(&right.event_sequence)
            .then_with(|| {
                left.changed_dependency_path
                    .cmp(&right.changed_dependency_path)
            })
    });
    if causes.windows(2).any(|pair| {
        (
            pair[0].event_sequence,
            pair[0].changed_dependency_path.as_str(),
        ) >= (
            pair[1].event_sequence,
            pair[1].changed_dependency_path.as_str(),
        )
    }) {
        return Err(DurableError::Corrupt(
            "range dependency causes are not uniquely ordered",
        ));
    }
    Ok(causes)
}

fn require_configured_schemas(
    selection: &NavigationProducerOwnerSelection<'_>,
    paths: &[String],
) -> DurableResult<()> {
    if paths
        .iter()
        .any(|path| selection.schema_paths.binary_search(path).is_err())
    {
        return Err(DurableError::Refused(
            "Claim dependency schema is outside the configured candidate worker",
        ));
    }
    Ok(())
}

fn bibliographic_limits(
    selection: &NavigationProducerOwnerSelection<'_>,
    deadline: Instant,
) -> DurableResult<BibliographicLimits> {
    let dependency_bytes =
        usize::try_from(selection.event_limits.max_total_bytes.min(16 * 1024 * 1024))
            .map_err(|_| DurableError::Refused("bibliographic dependency budget"))?;
    let row_bytes = selection.event_limits.max_row_bytes.min(1024 * 1024);
    let output_row_bytes = row_bytes.min(32_768).max(1);
    let max_claim_rows = selection
        .event_limits
        .max_current_rows
        .min(16_384)
        .min((128 * 1024 * 1024) / output_row_bytes)
        .max(1);
    let max_claim_bytes = max_claim_rows.saturating_mul(output_row_bytes);
    let max_output_bytes = selection
        .event_limits
        .max_total_bytes
        .min(selection.max_total_encoded_bytes)
        .min(128 * 1024 * 1024);
    let limits = BibliographicLimits {
        catalog: SourceCatalogLimits {
            max_files: selection.event_limits.max_dependencies.min(4096).max(1) as u64,
            max_rows: selection.event_limits.max_current_rows.min(65_536).max(1) as u64,
            max_file_bytes: dependency_bytes,
            max_row_bytes: row_bytes.max(1),
            max_contract_bytes: dependency_bytes
                .min(SourceCatalogLimits::MAX_CONTRACT_BYTES)
                .max(1),
            max_output_row_bytes: output_row_bytes.max(1),
        },
        max_claim_cohort_rows: max_claim_rows,
        max_claim_cohort_bytes: max_claim_bytes,
        max_output_rows: selection
            .max_total_rows
            .min(selection.event_limits.max_current_rows as u64)
            .max(1),
        max_output_bytes: max_output_bytes.max(1),
        deadline,
    };
    limits.validate().map_err(compiler_refusal)?;
    Ok(limits)
}

/// Open a second selected PG client for the event reader while proving it
/// addresses the exact database and STO held by the protected owner callback.
/// The callback's transaction is the owner-side database witness; the producer
/// never trusts a fresh environment lookup without comparing both witnesses.
// Keep the audited store instance from the held selection. Reopening even the
// same root creates a distinct custody handle and cannot carry that audit.
fn open_selected_event_reader(
    store: &SegmentStore,
    owner: &NavigationProducerHeldOwnerIdentity,
    selection: &NavigationProducerOwnerSelection<'_>,
    owner_tx: &mut postgres::Transaction<'_>,
) -> DurableResult<DurablePgCoordinator> {
    if store.custody_domain() != selection.domain.as_bytes() || store.store_id() != owner.store_id {
        return Err(DurableError::Conflict(
            "selected navigation source store differs from held owner",
        ));
    }

    let owner_oid = database_oid(owner_tx)?;
    if owner_oid != owner.database_oid {
        return Err(DurableError::Conflict(
            "held navigation PostgreSQL database identity differs",
        ));
    }
    let mut client = crate::source_projection_cli::connect_selected_source_pg16()
        .map_err(|_| DurableError::Refused("selected navigation PostgreSQL unavailable"))?;
    client
        .batch_execute("SET statement_timeout='15s'; SET lock_timeout='5s'")
        .map_err(|_| DurableError::Refused("navigation PostgreSQL session configuration"))?;
    if database_oid(&mut client)? != owner_oid {
        return Err(DurableError::Conflict(
            "navigation event reader PostgreSQL database differs from held owner",
        ));
    }
    Ok(DurablePgCoordinator::from_configured_client(client))
}

fn database_oid(client: &mut impl postgres::GenericClient) -> DurableResult<u64> {
    let row = client
        .query_one(
            "SELECT oid::bigint FROM pg_database WHERE datname=current_database()",
            &[],
        )
        .map_err(|_| DurableError::Refused("navigation PostgreSQL database identity"))?;
    let oid: i64 = row
        .try_get(0)
        .map_err(|_| DurableError::Corrupt("navigation PostgreSQL database identity"))?;
    u64::try_from(oid).map_err(|_| DurableError::Corrupt("navigation PostgreSQL database OID"))
}

/// Typed source-cut fence shared by the candidate schema worker and both
/// event renderers. Its digest binds the held PG/model identity to the exact
/// committed event member set; it is not a V1 SourceRevision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NavigationEventSourceCutIdentityV1 {
    owner_binding: Digest256,
    event_member_binding: Digest256,
    range_coverage_binding: Option<Digest256>,
    event_source_revision: Digest256,
    source_head_sequence: u64,
    event_sequence: u64,
}

impl CandidateEventSourceIdentityV1 for NavigationEventSourceCutIdentityV1 {
    fn event_source_revision(&self) -> Digest256 {
        self.event_source_revision
    }
}

impl NavigationEventSourceCutIdentityV1 {
    pub(crate) fn owner_binding(&self) -> Digest256 {
        self.owner_binding
    }

    pub(crate) fn event_member_binding(&self) -> Digest256 {
        self.event_member_binding
    }

    pub(crate) fn range_coverage_binding(&self) -> Option<Digest256> {
        self.range_coverage_binding
    }

    pub(crate) fn source_head_sequence(&self) -> u64 {
        self.source_head_sequence
    }

    pub(crate) fn event_sequence(&self) -> u64 {
        self.event_sequence
    }
}

/// Build the schema-worker identity for a terminal source cut backed by a
/// complete event range. The digest remains an event-source identity, never a
/// claim that the V2 source owner has a current `SourceRevision`.
fn range_source_cut_identity(
    owner: &NavigationProducerHeldOwnerIdentity,
    selection: &NavigationProducerOwnerSelection<'_>,
    coverage: &crate::durable_adapter::durable_projection::DurableNavigationEventRangeCoverageV1,
    context: &DurableNavigationEventContextV1,
) -> DurableResult<NavigationEventSourceCutIdentityV1> {
    if coverage.through_sequence() > coverage.held_source_head_sequence()
        || coverage.held_source_head_sequence() != owner.source_head_sequence
        || coverage.held_source_head_sequence() != owner.source_generation
        || context.event_id() != coverage.terminal_event_id()
        || context.event_sequence() != coverage.through_sequence()
        || context.source_head_sequence() != owner.source_head_sequence
        || context.consumer_cursor().sequence != coverage.cursor_before()
        || owner.domain != selection.domain
        || owner.domain_sha256 != Digest256::of_bytes(selection.domain.as_bytes())
        || owner.source_contract != selection.source_contract
        || owner.owner_configuration_sha256 != selection.owner_configuration_sha256
    {
        return Err(DurableError::Conflict(
            "navigation range source identity differs from the held owner",
        ));
    }
    let owner_binding = owner_identity_digest(owner);
    let event_member_binding = event_member_digest(context)?;
    let range_coverage_binding = coverage.coverage_root();
    let mut hasher = Digest256Hasher::new();
    hash_part(&mut hasher, b"tos-navigation-event-range-source-cut-v1");
    hash_part(&mut hasher, owner_binding.as_bytes());
    hash_part(&mut hasher, event_member_binding.as_bytes());
    hash_part(&mut hasher, range_coverage_binding.as_bytes());
    hash_part(&mut hasher, context.event_id().as_bytes());
    hash_part(&mut hasher, &context.event_sequence().to_be_bytes());
    hash_part(&mut hasher, &context.source_head_sequence().to_be_bytes());
    Ok(NavigationEventSourceCutIdentityV1 {
        owner_binding,
        event_member_binding,
        range_coverage_binding: Some(range_coverage_binding),
        event_source_revision: hasher.finalize(),
        source_head_sequence: context.source_head_sequence(),
        event_sequence: context.event_sequence(),
    })
}

/// Require the separately opened source-identity read session to be the same
/// held source owner epoch used by the managed V2 navigation selection. The
/// bootstrap revision/membership in this identity remain bootstrap facts; they
/// are never substituted for the current event source revision.
pub(crate) fn verify_identity_session(
    owner: &NavigationProducerHeldOwnerIdentity,
    selection: &NavigationProducerOwnerSelection<'_>,
    identity: &PgRecordRevisionIdentity,
) -> DurableResult<()> {
    if identity.domain_digest != Digest256::of_bytes(selection.domain.as_bytes())
        || identity.store_id != owner.store_id
        || identity.epoch != owner.source_epoch
        || identity.source_generation != owner.source_generation
        || identity.head_seq != owner.source_head_sequence
        || identity.audit_generation != owner.selected_audit_generation
        || identity.rights_version != owner.rights_version
        || identity.rule_version != owner.rule_version
        || identity.contract != selection.source_contract
        || identity.job_id_digest != owner.job_id_sha256
        || identity.job_fence != owner.job_fence
        || identity.cohort_epoch != owner.source_epoch
        || identity.cohort_generation != owner.source_generation
    {
        return Err(DurableError::Conflict(
            "navigation identity lookup differs from the held current source owner",
        ));
    }
    Ok(())
}

/// Bound the union of addresses before asking the event reader to retain their
/// source bytes. Every address is a validated relative ToS path; duplicate
/// routes collapse in the same canonical set used for the final cut.
pub(crate) fn extend_event_paths(
    paths: &mut BTreeSet<String>,
    additions: impl IntoIterator<Item = String>,
    max_paths: usize,
) -> DurableResult<()> {
    for path in additions {
        let checked = tos_foundation::RelativePath::parse(&path)
            .map_err(|_| DurableError::Invalid("navigation dependency path"))?;
        if checked.as_str() != path {
            return Err(DurableError::Invalid("navigation dependency path"));
        }
        paths.insert(path);
        if paths.len() > max_paths {
            return Err(DurableError::Refused(
                "navigation event dependency path budget",
            ));
        }
    }
    Ok(())
}

/// Materialize the final event reader's exact presence/absence observations
/// into the compiler's bounded file carrier. An unrequested address remains
/// unknown; only selected non-source paths become explicit absences.
pub(crate) fn selected_bibliographic_files(
    context: &DurableNavigationEventContextV1,
    limits: &crate::DurableNavigationEventLimits,
) -> DurableResult<tos_compiler::SelectedBibliographicFilesV1> {
    let max_files = limits.max_dependencies.min(4_096);
    let max_bytes = usize::try_from(limits.max_total_bytes.min(64 * 1024 * 1024))
        .map_err(|_| DurableError::Refused("navigation selected source byte budget"))?;
    if context.selected_sources().is_empty() || max_files == 0 || max_bytes == 0 {
        return Err(DurableError::Refused(
            "navigation selected source closure is empty",
        ));
    }
    let mut files = BTreeMap::new();
    let mut absent = BTreeSet::new();
    for source in context.selected_sources() {
        let checked = tos_foundation::RelativePath::parse(&source.source_path)
            .map_err(|_| DurableError::Corrupt("navigation selected source path"))?;
        if checked.as_str() != source.source_path {
            return Err(DurableError::Corrupt(
                "navigation selected source path is not canonical",
            ));
        }
        match (source.is_source, source.exact_source_bytes.as_deref()) {
            (true, Some(raw))
                if source.content_digest == Some(Digest256::of_bytes(raw))
                    && source.content_length == Some(raw.len() as u64) =>
            {
                if files
                    .insert(source.source_path.clone(), raw.to_vec())
                    .is_some()
                {
                    return Err(DurableError::Corrupt(
                        "navigation selected source path is repeated",
                    ));
                }
            }
            (false, None) => {
                if !absent.insert(source.source_path.clone()) {
                    return Err(DurableError::Corrupt(
                        "navigation selected absence path is repeated",
                    ));
                }
            }
            _ => {
                return Err(DurableError::Conflict(
                    "navigation selected source bytes or absence differ",
                ));
            }
        }
    }
    tos_compiler::SelectedBibliographicFilesV1::new(files, absent, max_files, max_bytes)
        .map_err(compiler_refusal)
}

/// Select reverse-Claim index predicates for every exact changed path and
/// stable owner ID. The source-revision session records the complete index
/// generations, including empty results, and reopens every referenced Claim
/// stream before returning its sealed closure.
pub(crate) fn claim_navigation_lookup_keys(
    changed_paths: &[String],
    changed_ids: &[String],
) -> DurableResult<
    BTreeSet<(
        crate::source_claim_dependency_index::ClaimNavigationReferenceKind,
        String,
    )>,
> {
    use crate::source_claim_dependency_index::ClaimNavigationReferenceKind;

    if changed_paths.is_empty() && changed_ids.is_empty() {
        return Err(DurableError::Refused(
            "navigation reverse Claim lookup has no changed addresses",
        ));
    }
    let mut keys = BTreeSet::new();
    for path in changed_paths {
        let parsed = tos_foundation::RelativePath::parse(path)
            .map_err(|_| DurableError::Invalid("navigation changed source path"))?;
        if parsed.as_str() != path || !path.starts_with("ToS/") {
            return Err(DurableError::Invalid("navigation changed source path"));
        }
        keys.insert((ClaimNavigationReferenceKind::SourcePath, path.clone()));
    }
    for id in changed_ids {
        if id.is_empty() || id.len() > 4_096 || !id.starts_with("tos.") {
            return Err(DurableError::Invalid("navigation changed source identity"));
        }
        keys.insert((ClaimNavigationReferenceKind::TosId, id.clone()));
    }
    if keys.len() > crate::source_claim_dependency_index::MAX_NAVIGATION_REFERENCE_LOOKUPS {
        return Err(DurableError::Refused(
            "navigation reverse Claim lookup count budget",
        ));
    }
    Ok(keys)
}

/// Return the canonical union of Claim owners and source streams proven by
/// the exact reverse-index closure. Duplicate references collapse, but the
/// underlying readset still retains every queried predicate generation.
pub(crate) fn claim_navigation_reverse_owners(
    closure: &crate::source_cohort::revision::SourceRevisionClaimNavigationClosure,
) -> (Vec<String>, Vec<String>) {
    let mut claim_ids = BTreeSet::new();
    let mut source_paths = BTreeSet::new();
    for observation in closure.lookups().values() {
        for row in &observation.rows {
            claim_ids.insert(row.claim_id.clone());
            source_paths.insert(row.path.clone());
        }
    }
    (
        claim_ids.into_iter().collect(),
        source_paths.into_iter().collect(),
    )
}

/// Incremental aggregate admission. Call before retaining each sealed cohort
/// so over-budget work never grows the in-memory cohort set.
#[derive(Default)]
pub(crate) struct NavigationEventBudgetMeterV1 {
    rows: u64,
    encoded_bytes: u64,
    dependency_bytes: u64,
    work_units: u64,
}

impl NavigationEventBudgetMeterV1 {
    pub(crate) fn admit(
        &mut self,
        rows: u64,
        encoded_bytes: u64,
        dependency_bytes: u64,
        work_units: u64,
        selection: &NavigationProducerOwnerSelection<'_>,
        limits: &crate::DurableNavigationEventLimits,
    ) -> DurableResult<()> {
        let rows = self
            .rows
            .checked_add(rows)
            .ok_or(DurableError::Refused("navigation aggregate row count"))?;
        let encoded_bytes = self
            .encoded_bytes
            .checked_add(encoded_bytes)
            .ok_or(DurableError::Refused("navigation aggregate output bytes"))?;
        let dependency_bytes =
            self.dependency_bytes
                .checked_add(dependency_bytes)
                .ok_or(DurableError::Refused(
                    "navigation aggregate dependency bytes",
                ))?;
        let work_units = self
            .work_units
            .checked_add(work_units)
            .ok_or(DurableError::Refused("navigation aggregate work units"))?;
        if rows > selection.max_total_rows
            || encoded_bytes > selection.max_total_encoded_bytes
            || encoded_bytes > limits.max_total_bytes
            || dependency_bytes > selection.max_total_dependency_bytes
            || work_units > selection.max_total_work_units
        {
            return Err(DurableError::Refused(
                "navigation aggregate event budget exceeded",
            ));
        }
        self.rows = rows;
        self.encoded_bytes = encoded_bytes;
        self.dependency_bytes = dependency_bytes;
        self.work_units = work_units;
        Ok(())
    }
}

fn owner_identity_digest(owner: &NavigationProducerHeldOwnerIdentity) -> Digest256 {
    let mut hasher = Digest256Hasher::new();
    hash_part(&mut hasher, b"tos-navigation-held-pg-owner-v2");
    hash_part(&mut hasher, owner.proof_sha256.as_bytes());
    hash_part(&mut hasher, owner.domain.as_bytes());
    hash_part(&mut hasher, owner.domain_sha256.as_bytes());
    hash_part(&mut hasher, &owner.store_id);
    hash_part(&mut hasher, &owner.source_head_sequence.to_be_bytes());
    hash_part(&mut hasher, &owner.source_generation.to_be_bytes());
    hash_part(&mut hasher, &owner.source_epoch.to_be_bytes());
    hash_part(&mut hasher, &owner.selected_audit_generation.to_be_bytes());
    hash_part(&mut hasher, &owner.database_oid.to_be_bytes());
    hash_part(
        &mut hasher,
        owner.anchor_addressed_tree_commitment.as_bytes(),
    );
    hash_part(&mut hasher, owner.managed_generation_digest.as_bytes());
    hash_part(&mut hasher, &owner.anchor_member_count.to_be_bytes());
    hash_part(&mut hasher, owner.anchor_projection_digest.as_bytes());
    hash_part(&mut hasher, owner.bootstrap_membership_root.as_bytes());
    hash_part(&mut hasher, &owner.bootstrap_membership_count.to_be_bytes());
    if let Some(value) = owner.current_selected_generation_digest {
        hash_part(&mut hasher, b"selected-generation-present");
        hash_part(&mut hasher, value.as_bytes());
    } else {
        hash_part(&mut hasher, b"selected-generation-absent");
    }
    if let Some(value) = owner.current_source_projection_digest {
        hash_part(&mut hasher, b"source-projection-present");
        hash_part(&mut hasher, value.as_bytes());
    } else {
        hash_part(&mut hasher, b"source-projection-absent");
    }
    hash_part(&mut hasher, owner.schema_profile_digest.as_bytes());
    hash_part(&mut hasher, owner.model_sha256.as_bytes());
    hash_part(&mut hasher, owner.model_membership_root.as_bytes());
    hash_part(&mut hasher, owner.owner_configuration_sha256.as_bytes());
    hash_part(&mut hasher, owner.source_contract.as_bytes());
    hash_part(&mut hasher, &owner.rule_version.to_be_bytes());
    hash_part(&mut hasher, &owner.rights_version.to_be_bytes());
    hash_part(&mut hasher, owner.job_id_sha256.as_bytes());
    hash_part(&mut hasher, &owner.job_fence.to_be_bytes());
    hasher.finalize()
}

fn event_member_digest(context: &DurableNavigationEventContextV1) -> DurableResult<Digest256> {
    let mut members = context.members().iter().collect::<Vec<_>>();
    members.sort_by(|left, right| left.source_path.cmp(&right.source_path));
    if members
        .windows(2)
        .any(|pair| pair[0].source_path == pair[1].source_path)
    {
        return Err(DurableError::Corrupt(
            "navigation event repeats a committed source member",
        ));
    }

    let mut hasher = Digest256Hasher::new();
    hash_part(&mut hasher, b"tos-navigation-event-members-v1");
    hash_part(&mut hasher, context.event_id().as_bytes());
    hash_part(&mut hasher, &context.event_sequence().to_be_bytes());
    hash_part(&mut hasher, &context.source_head_sequence().to_be_bytes());
    for member in members {
        hash_part(&mut hasher, member.source_path.as_bytes());
        hash_option_u64(&mut hasher, member.revision);
        hash_option_u64(&mut hasher, member.commit_sequence);
        match member.content_digest {
            Some(digest) => {
                hash_part(&mut hasher, &[1]);
                hash_part(&mut hasher, digest.as_bytes());
            }
            None => hash_part(&mut hasher, &[0]),
        }
        hash_option_u64(&mut hasher, member.content_length);
        hash_part(&mut hasher, &[u8::from(member.is_source)]);
        if member.is_source {
            let bytes = selected_source_bytes(context, &member.source_path)?;
            if member.content_digest != Some(Digest256::of_bytes(bytes))
                || member.content_length != Some(bytes.len() as u64)
            {
                return Err(DurableError::Conflict(
                    "navigation event selected member bytes differ",
                ));
            }
        }
    }
    Ok(hasher.finalize())
}

fn selected_source_bytes<'a>(
    context: &'a DurableNavigationEventContextV1,
    path: &str,
) -> DurableResult<&'a [u8]> {
    let mut selected = context
        .selected_sources()
        .iter()
        .filter(|source| source.source_path == path);
    let source = selected.next().ok_or(DurableError::Conflict(
        "navigation event source path was not selected",
    ))?;
    if selected.next().is_some() || !source.is_source {
        return Err(DurableError::Corrupt(
            "navigation event source selection is ambiguous",
        ));
    }
    let bytes = source
        .exact_source_bytes
        .as_deref()
        .ok_or(DurableError::Conflict(
            "navigation event source bytes are absent",
        ))?;
    if source.content_digest != Some(Digest256::of_bytes(bytes))
        || source.content_length != Some(bytes.len() as u64)
    {
        return Err(DurableError::Conflict(
            "navigation event source selection digest differs",
        ));
    }
    Ok(bytes)
}

fn with_range_schema_validator<T>(
    owner_identity: &NavigationProducerHeldOwnerIdentity,
    selection: &NavigationProducerOwnerSelection<'_>,
    coverage: &crate::durable_adapter::durable_projection::DurableNavigationEventRangeCoverageV1,
    context: &DurableNavigationEventContextV1,
    identity: NavigationEventSourceCutIdentityV1,
    deadline: Instant,
    cancelled: &AtomicBool,
    consume: impl FnOnce(&SourceCatalogValidator<'_>) -> DurableResult<T>,
) -> DurableResult<T> {
    if range_source_cut_identity(owner_identity, selection, coverage, context)? != identity {
        return Err(DurableError::Conflict(
            "navigation range schema validator source identity changed",
        ));
    }
    with_selected_schema_validator(selection, context, identity, deadline, cancelled, consume)
}

fn with_selected_schema_validator<T>(
    selection: &NavigationProducerOwnerSelection<'_>,
    context: &DurableNavigationEventContextV1,
    identity: NavigationEventSourceCutIdentityV1,
    deadline: Instant,
    cancelled: &AtomicBool,
    consume: impl FnOnce(&SourceCatalogValidator<'_>) -> DurableResult<T>,
) -> DurableResult<T> {
    let mut selected_resources = Vec::with_capacity(selection.schema_paths.len());
    for path in selection.schema_paths {
        let raw = selected_source_bytes(context, path)?.to_vec();
        let value: serde_json::Value = serde_json::from_slice(&raw)
            .map_err(|_| DurableError::Invalid("selected navigation schema JSON"))?;
        let uri = value
            .get("$id")
            .and_then(serde_json::Value::as_str)
            .filter(|uri| !uri.is_empty())
            .ok_or(DurableError::Invalid(
                "selected navigation schema identifier",
            ))?
            .to_owned();
        selected_resources.push((path.clone(), SchemaResource { uri, raw }));
    }
    let image = VerifiedWorkerImageHandle::prepare(
        selection.schema_worker.clone(),
        selection.schema_budget,
        deadline,
        cancelled,
    )
    .map_err(|_| DurableError::Refused("selected navigation schema worker refused"))?;
    let mut schemas = CandidateCutWorkerSchemaExecutor::from_selected_source_resources(
        identity,
        selected_resources,
        selection.schema_profile,
        &image,
        selection.schema_budget,
        selection.schema_limits,
        deadline,
        cancelled,
    )
    .map_err(|_| DurableError::Refused("selected navigation schema set refused"))?;
    let validator = SourceCatalogValidator::from_candidate_event_prepared(
        &selection.schema_worker,
        selection.schema_budget,
        cancelled,
        deadline,
        &identity,
        &mut schemas,
    )
    .map_err(compiler_refusal)?;
    let result = consume(&validator)?;
    validator.finish().map_err(compiler_refusal)?;
    Ok(result)
}

fn hash_part(hasher: &mut Digest256Hasher, bytes: &[u8]) {
    hasher.update(&(bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn hash_option_u64(hasher: &mut Digest256Hasher, value: Option<u64>) {
    match value {
        Some(value) => {
            hash_part(hasher, &[1]);
            hash_part(hasher, &value.to_be_bytes());
        }
        None => hash_part(hasher, &[0]),
    }
}
