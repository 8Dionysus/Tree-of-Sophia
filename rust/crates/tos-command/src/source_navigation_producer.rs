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

use serde_json::{json, Value};
use tos_compiler::source_witness_catalog::{
    CandidateEventSourceIdentityV1, SourceCatalogValidator,
};
use tos_compiler::{
    source_bibliographic::{BibliographicLimits, SourceCatalogLimits},
    source_navigation_claim_event::{
        NavigationClaimDependencyRefreshCauseInputV1, NavigationClaimEventContributionV1,
        NavigationClaimEventSourceFileV1,
    },
    source_navigation_event::NavigationRecordDependencyRefreshCauseV1,
};
use tos_foundation::{Digest256, Digest256Hasher, RelativePath};
use tos_segment_store::SegmentStore;
use tos_validation::source_cut::CandidateCutWorkerSchemaExecutor;
use tos_validation::{SchemaResource, VerifiedWorkerImageHandle};

use crate::PgRecordRevisionIdentity;

use crate::source_navigation_cli::{
    NavigationProducerHeldOwnerIdentity, NavigationProducerOwnerSelection,
};
use crate::{DurableError, DurableNavigationEventContextV1, DurablePgCoordinator, DurableResult};

/// Run the selected producer while the managed source, rights, audit, job,
/// model, and owner-configuration fences remain held through publication.
pub(crate) fn run(
    selection_path: &Path,
    requested_max_events: usize,
    cancelled: &AtomicBool,
) -> Result<Value, &'static str> {
    if !(1..=256).contains(&requested_max_events) {
        return Err("max_events must be 1..256");
    }
    let deadline = Instant::now() + Duration::from_secs(300);
    let selected = crate::source_navigation_cli::SelectedSourceNavigation::new(selection_path)
        .map_err(|_| "source navigation selection refused")?;
    crate::source_navigation_cli::with_selected_navigation_producer_current(
        &selected,
        deadline,
        cancelled,
        |_model, _lease, _proof, _expectation, owner, owner_tx, selection| {
            run_held_owner(
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
    .map_err(|_| "source navigation event producer refused")
}

fn run_held_owner(
    owner: &NavigationProducerHeldOwnerIdentity,
    owner_tx: &mut postgres::Transaction<'_>,
    selection: &NavigationProducerOwnerSelection<'_>,
    requested_max_events: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<Value> {
    let (store, mut event_reader) = open_selected_event_store(owner, selection, owner_tx)?;
    let max_events = requested_max_events.min(selection.max_events);
    let mut cursor_before = None;
    let mut cursor_sequence = 0u64;
    let mut source_head_sequence = owner.source_head_sequence;
    let mut published = Vec::new();
    let mut stop = "step_budget_reached";

    for _ in 0..max_events {
        if Instant::now() >= deadline || cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(DurableError::Refused(
                "navigation producer deadline or cancellation",
            ));
        }
        match event_reader.read_navigation_next_event_seed_v1(
            &store,
            selection.domain,
            selection.consumer_id,
            &selection.profile,
            selection.event_path,
            selection.event_limits,
        )? {
            crate::DurableNavigationEventSeedV1::UpToDate {
                cursor_sequence: cursor,
                source_head_sequence: head,
            } => {
                cursor_before.get_or_insert(cursor);
                cursor_sequence = cursor;
                source_head_sequence = head;
                stop = "up_to_date";
                break;
            }
            crate::DurableNavigationEventSeedV1::Pending {
                cursor_sequence: cursor,
                source_head_sequence: head,
                event_sequence,
                reason,
            } => {
                cursor_before.get_or_insert(cursor);
                cursor_sequence = cursor;
                source_head_sequence = head;
                stop = match reason {
                    crate::DurableNavigationEventPendingReasonV2::NoNavigationEffectProof => {
                        "no_navigation_effect_proof"
                    }
                    crate::DurableNavigationEventPendingReasonV2::UnsupportedEventKind => {
                        "unsupported_event_kind"
                    }
                    crate::DurableNavigationEventPendingReasonV2::EventNotCurrentSourceHead => {
                        "event_not_current_source_head"
                    }
                };
                if cursor_before.is_none() {
                    cursor_before = Some(event_sequence.saturating_sub(1));
                }
                break;
            }
            crate::DurableNavigationEventSeedV1::Ready(seed) => {
                cursor_before.get_or_insert(seed.consumer_cursor().sequence);
                cursor_sequence = seed.consumer_cursor().sequence;
                source_head_sequence = seed.source_head_sequence();
                let Some(apply) = process_ready_event(
                    &mut event_reader,
                    &store,
                    &seed,
                    owner,
                    selection,
                    deadline,
                    cancelled,
                )?
                else {
                    stop = "no_navigation_effect_proof";
                    break;
                };
                cursor_sequence = apply.event_sequence;
                source_head_sequence = seed.source_head_sequence();
                published.push(json!({
                    "event_sequence": apply.event_sequence,
                    "event_id": apply.event_id,
                    "output_root": apply.output_root.to_hex(),
                    "replayed": apply.replayed,
                }));
                if cursor_sequence == source_head_sequence {
                    stop = "up_to_date";
                    break;
                }
            }
        }
    }

    Ok(json!({
        "schema_version": "tos_source_navigation_advance_result_v1",
        "profile": selection.profile.id,
        "cursor_before": cursor_before.unwrap_or(cursor_sequence),
        "cursor_sequence": cursor_sequence,
        "source_head_sequence": source_head_sequence,
        "published": published,
        "stop": stop,
    }))
}

/// Compile and atomically publish one exact current CMD event. `None` is a
/// verified event with no supported Record or Claim owner outcome; the cursor
/// remains in place for a later owner route to handle it.
#[allow(clippy::too_many_arguments)]
fn process_ready_event(
    event_reader: &mut DurablePgCoordinator,
    store: &SegmentStore,
    seed: &DurableNavigationEventContextV1,
    owner: &NavigationProducerHeldOwnerIdentity,
    selection: &NavigationProducerOwnerSelection<'_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<Option<crate::DurableNavigationEventApply>> {
    let changed_paths = changed_event_source_paths(seed)?;
    let changed_tos_paths = changed_paths
        .iter()
        .filter(|path| path.starts_with("ToS/"))
        .cloned()
        .collect::<Vec<_>>();
    if changed_tos_paths.is_empty() {
        return Ok(None);
    }
    let claim_mutation = direct_claim_mutation(
        seed,
        &changed_tos_paths,
        selection.event_limits.max_row_bytes,
    )?;

    let mut affected_by_path = BTreeMap::new();
    let mut affected_owners =
        BTreeMap::<(String, String), crate::DurableNavigationEventAffectedOwnerV1>::new();
    for path in &changed_paths {
        let owners = event_reader.read_navigation_event_affected_owners_v1(
            selection.domain,
            selection.consumer_id,
            seed,
            std::slice::from_ref(path),
            &selection.profile,
            selection.event_limits,
        )?;
        for affected in &owners {
            if affected.kind() == crate::DurableNavigationEventAffectedOwnerKindV1::Record {
                affected_owners.insert(
                    (
                        affected.source_path().to_owned(),
                        affected.record_id().to_owned(),
                    ),
                    affected.clone(),
                );
            }
        }
        affected_by_path.insert(path.clone(), owners);
    }
    let affected_owners = affected_owners.into_values().collect::<Vec<_>>();
    let record_plan = crate::source_navigation_record_input::required_paths(
        seed,
        &affected_owners,
        selection.event_path,
        selection.schema_paths,
        selection.event_limits.max_dependencies,
    )?;

    // Each changed path retains its own affected Record identities. Those
    // exact IDs drive TosId reverse lookups; the path lookups independently
    // cover direct path references and empty results.
    let mut ids_by_path = BTreeMap::<String, BTreeSet<String>>::new();
    for (path, owners) in &affected_by_path {
        let ids = ids_by_path.entry(path.clone()).or_default();
        for affected in owners.iter().filter(|owner| {
            owner.kind() == crate::DurableNavigationEventAffectedOwnerKindV1::Record
        }) {
            ids.insert(affected.record_id().to_owned());
        }
    }
    for record in &record_plan.owners {
        if changed_tos_paths.binary_search(&record.source_path).is_ok() {
            ids_by_path
                .entry(record.source_path.clone())
                .or_default()
                .insert(record.record_id.clone());
        }
    }
    let mut changed_ids = ids_by_path
        .values()
        .flat_map(|ids| ids.iter().cloned())
        .collect::<BTreeSet<_>>();
    if let Some((_, _, mutation_ids)) = &claim_mutation {
        changed_ids.extend(mutation_ids.iter().cloned());
    }
    let reference_keys = claim_navigation_lookup_keys(
        &changed_tos_paths,
        &changed_ids.iter().cloned().collect::<Vec<_>>(),
    )?;
    let source_member_budget = selection.event_limits.max_total_bytes;

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
            let identities = session.lookup_source_identities(&changed_ids, deadline, cancelled)?;
            for owner in identities.owners().values().flatten() {
                let path = RelativePath::parse(owner.1.as_str())
                    .map_err(|_| DurableError::Corrupt("navigation identity source path"))?;
                session.read_current_member(&path, source_member_budget, deadline, cancelled)?;
            }
            session.lookup_claim_navigation_references(&reference_keys, deadline, cancelled)?;
            Ok(identities)
        },
    )?;
    identity_closure.verify_readset(&identity_readset)?;
    let claim_closure =
        identity_readset
            .claim_navigation_closure()
            .cloned()
            .ok_or(DurableError::Conflict(
                "navigation reverse Claim closure was not retained",
            ))?;
    if claim_closure.domain() != selection.domain
        || claim_closure.selection_identity().head_seq != owner.source_head_sequence
        || claim_closure.selection_identity().source_generation != owner.source_generation
        || claim_closure.selection_identity().epoch != owner.source_epoch
    {
        return Err(DurableError::Conflict(
            "navigation reverse Claim closure differs from the held source owner",
        ));
    }
    let (_reverse_claim_ids, reverse_claim_paths) = claim_navigation_reverse_owners(&claim_closure);
    let mut held_identity_readsets = vec![identity_readset];

    let mut context_paths = initial_event_paths(selection, seed)?
        .into_iter()
        .collect::<BTreeSet<_>>();
    extend_event_paths(
        &mut context_paths,
        record_plan.paths.iter().cloned(),
        selection.event_limits.max_dependencies,
    )?;
    extend_event_paths(
        &mut context_paths,
        reverse_claim_paths.iter().cloned(),
        selection.event_limits.max_dependencies,
    )?;
    let mut context =
        read_current_event_context(event_reader, store, selection, seed, &context_paths)?;
    verify_claim_member_facts(&context, &claim_closure)?;

    let limits = bibliographic_limits(selection, deadline)?;
    if !record_plan.owners.is_empty() {
        let files = selected_bibliographic_files(&context, &selection.event_limits)?;
        let manifests = crate::source_navigation_record_input::archive_manifest_paths(
            &record_plan.owners,
            &files,
            limits,
        )?;
        if !manifests.is_empty() {
            stage_event_context(
                event_reader,
                store,
                selection,
                seed,
                &mut context,
                manifests.clone(),
            )?;
        }
        let files = selected_bibliographic_files(&context, &selection.event_limits)?;
        let blobs =
            crate::source_navigation_record_input::archive_blob_paths(&manifests, &files, limits)?;
        if !blobs.is_empty() {
            stage_event_context(event_reader, store, selection, seed, &mut context, blobs)?;
        }
    }

    let (reverse_claim_ids, _) = claim_navigation_reverse_owners(&claim_closure);
    let mut claim_manifest = None;
    let mut claim_mutation_manifest = None;
    let mut claim_mutation_inventory = None;
    let mut claim_mutation_receipt_path = None;
    let mut claim_identity_owners = Vec::new();
    let mut claim_predecessors = BTreeMap::<(String, String), Option<Digest256>>::new();
    if let Some((mutation_source_path, receipt_path, mutation_ids)) = &claim_mutation {
        let (inventory, manifest) = prepare_direct_claim_mutation(
            event_reader,
            store,
            owner,
            selection,
            seed,
            &mut context,
            &claim_closure,
            mutation_source_path,
            receipt_path,
            mutation_ids,
            &reverse_claim_ids,
            &reverse_claim_paths,
            limits,
            deadline,
            cancelled,
            &mut claim_identity_owners,
            &mut claim_predecessors,
            &mut held_identity_readsets,
        )?;
        claim_mutation_inventory = Some(inventory);
        claim_mutation_manifest = Some(manifest);
        claim_mutation_receipt_path = Some(receipt_path.clone());
    } else if !reverse_claim_ids.is_empty() {
        let stream_files = claim_source_files(&context, &claim_closure, &reverse_claim_paths, &[])?;
        let inventory = tos_compiler::source_navigation_claim_event::
            prepare_navigation_claim_event_owner_inventory_v1(&stream_files, limits)
            .map_err(|_| DurableError::Refused("selected Claim owner inventory refused"))?;
        let cause_storage =
            claim_refresh_causes(&context, &claim_closure, &changed_tos_paths, &ids_by_path)?;
        let cause_inputs = cause_storage
            .iter()
            .map(|cause| NavigationClaimDependencyRefreshCauseInputV1 {
                changed_dependency_path: &cause.path,
                changed_dependency_sha256: cause.sha256,
                reverse_claim_ids: &cause.reverse_claim_ids,
            })
            .collect::<Vec<_>>();
        let provisional_manifest = inventory
            .affected_dependency_stream_set(&cause_inputs, claim_closure.closure_root())
            .map_err(|_| DurableError::Conflict("Claim reverse owner manifest differs"))?;

        let paths = crate::source_navigation_claim_selection::
            claim_event_dependency_refresh_stream_paths_v1(&provisional_manifest)?;
        stage_event_context(event_reader, store, selection, seed, &mut context, paths)?;
        let paths = crate::source_navigation_claim_selection::
            claim_event_dependency_refresh_current_owner_paths_v1(&context, &provisional_manifest)?;
        stage_event_context(event_reader, store, selection, seed, &mut context, paths)?;

        let requests = crate::source_navigation_claim_selection::
            claim_event_dependency_refresh_identity_requests_v1(&context, &provisional_manifest)?;
        let identity_tokens = requests
            .iter()
            .map(|request| request.id.clone())
            .collect::<BTreeSet<_>>();
        let (claim_identity_closure, claim_identity_readset) = event_reader
            .with_source_revision_session(
                store,
                selection.domain,
                selection.job_id,
                selection.job_fence,
                deadline,
                cancelled,
                |session| {
                    let selected = session.selection().clone();
                    verify_identity_session(owner, selection, &selected.identity())?;
                    if selected.identity() != claim_closure.selection_identity() {
                        return Err(DurableError::Conflict(
                            "Claim identity lookup uses another source owner epoch",
                        ));
                    }
                    let identities =
                        session.lookup_source_identities(&identity_tokens, deadline, cancelled)?;
                    for owner in identities.owners().values().flatten() {
                        let path = RelativePath::parse(owner.1.as_str())
                            .map_err(|_| DurableError::Corrupt("Claim identity source path"))?;
                        session.read_current_member(
                            &path,
                            source_member_budget,
                            deadline,
                            cancelled,
                        )?;
                    }
                    Ok(identities)
                },
            )?;
        claim_identity_closure.verify_readset(&claim_identity_readset)?;
        claim_identity_owners = requests
            .iter()
            .map(|request| {
                let owner = claim_identity_closure.owners().get(&request.id).ok_or(
                    DurableError::Conflict("Claim identity lookup result absent"),
                )?;
                Ok(
                    crate::source_navigation_claim_selection::ClaimEventIdentityOwnerV1 {
                        id: request.id.clone(),
                        kind: owner.as_ref().map(|(kind, _)| kind.clone()),
                        source_path: owner.as_ref().map(|(_, path)| path.as_str().to_owned()),
                    },
                )
            })
            .collect::<DurableResult<Vec<_>>>()?;
        held_identity_readsets.push(claim_identity_readset);

        let paths = crate::source_navigation_claim_selection::
            claim_event_dependency_refresh_indexed_paths_v1(
                &context,
                &provisional_manifest,
                &claim_identity_owners,
            )?;
        stage_event_context(event_reader, store, selection, seed, &mut context, paths)?;
        let schema_paths = crate::source_navigation_claim_selection::
            claim_event_dependency_refresh_schema_paths_v1(
                &context,
                &provisional_manifest,
                &claim_identity_owners,
            )?;
        require_configured_schemas(selection, &schema_paths)?;
        stage_event_context(
            event_reader,
            store,
            selection,
            seed,
            &mut context,
            schema_paths,
        )?;
        let historical_endpoint_paths = crate::source_navigation_claim_selection::
            claim_event_dependency_refresh_historical_endpoint_paths_v1(
                &context,
                &provisional_manifest,
                &claim_identity_owners,
            )?;
        stage_event_context(
            event_reader,
            store,
            selection,
            seed,
            &mut context,
            historical_endpoint_paths.clone(),
        )?;
        let historical_capture_paths = crate::source_navigation_claim_selection::
            claim_event_dependency_refresh_historical_capture_paths_v1(
                &context,
                &provisional_manifest,
                &claim_identity_owners,
            )?;
        stage_event_context(
            event_reader,
            store,
            selection,
            seed,
            &mut context,
            historical_capture_paths,
        )?;
        let archive_manifests = crate::source_navigation_claim_selection::
            claim_event_dependency_refresh_archive_manifest_paths_v1(
                &context,
                &provisional_manifest,
            )?;
        stage_event_context(
            event_reader,
            store,
            selection,
            seed,
            &mut context,
            archive_manifests.clone(),
        )?;
        let archive_blobs = crate::source_navigation_claim_selection::
            claim_event_dependency_refresh_archive_blob_paths_v1(
                &context,
                &provisional_manifest,
            )?;
        stage_event_context(
            event_reader,
            store,
            selection,
            seed,
            &mut context,
            archive_blobs,
        )?;
        let identity_archive_manifests = crate::source_navigation_claim_selection::
            claim_event_dependency_refresh_identity_archive_manifest_paths_v1(
                &context,
                &provisional_manifest,
                &claim_identity_owners,
            )?;
        stage_event_context(
            event_reader,
            store,
            selection,
            seed,
            &mut context,
            identity_archive_manifests.clone(),
        )?;
        let identity_archive_blobs = crate::source_navigation_claim_selection::
            claim_event_dependency_refresh_identity_archive_blob_paths_v1(
                &context,
                &provisional_manifest,
                &claim_identity_owners,
            )?;
        stage_event_context(
            event_reader,
            store,
            selection,
            seed,
            &mut context,
            identity_archive_blobs,
        )?;

        let final_stream_files = claim_source_files(
            &context,
            &claim_closure,
            &reverse_claim_paths,
            &historical_endpoint_paths,
        )?;
        let final_inventory = tos_compiler::source_navigation_claim_event::
            prepare_navigation_claim_event_owner_inventory_with_endpoints_v1(
                &final_stream_files,
                &historical_endpoint_paths,
                limits,
            )
            .map_err(|_| DurableError::Refused("final Claim owner inventory refused"))?;
        let final_manifest = final_inventory
            .affected_dependency_stream_set(&cause_inputs, claim_closure.closure_root())
            .map_err(|_| DurableError::Conflict("final Claim owner manifest differs"))?;
        if final_manifest.streams() != provisional_manifest.streams() {
            return Err(DurableError::Conflict(
                "Claim owner closure changed after endpoint selection",
            ));
        }
        claim_predecessors = read_claim_predecessors(event_reader, selection, &final_manifest)?;
        claim_manifest = Some((final_inventory, final_manifest));
    }

    if record_plan.owners.is_empty()
        && claim_manifest.is_none()
        && claim_mutation_manifest.is_none()
    {
        return Ok(None);
    }
    let identity = event_source_cut_identity(owner, selection, &context)?;
    let files = selected_bibliographic_files(&context, &selection.event_limits)?;
    let mut meter = NavigationEventBudgetMeterV1::default();
    let mut forms = crate::source_forms_compiler::NativeBibliographicForms;
    let source_revision_closure = claim_closure;

    let set = with_event_schema_validator(
        owner,
        selection,
        &context,
        identity,
        deadline,
        cancelled,
        |validator| {
            let mut claim_contributions = Vec::new();
            let mut claim_revision_proofs = Vec::new();
            let mut claim_mutation_binding = None;
            let mut claim_refresh_manifest = None;
            if let (Some(inventory), Some(manifest), Some(receipt_path)) = (
                &claim_mutation_inventory,
                &claim_mutation_manifest,
                &claim_mutation_receipt_path,
            ) {
                for stream in manifest.streams() {
                    for claim_id in stream.owner_ids() {
                        let key = (stream.source_path().to_owned(), claim_id.clone());
                        let predecessor = claim_predecessors.get(&key).copied().flatten();
                        let (cohort, cost) = crate::source_navigation_claim_input::
                            assemble_selected_claim_event_cohort_v1(
                                &context,
                                identity,
                                validator,
                                &files,
                                inventory,
                                manifest,
                                &claim_identity_owners,
                                receipt_path,
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
                        if let Some(proof) = cohort.revision_proof() {
                            claim_revision_proofs.push(proof.clone());
                        }
                        claim_contributions
                            .push(NavigationClaimEventContributionV1::Mutation(cohort));
                    }
                }
                claim_mutation_binding = Some(manifest);
            } else if let Some((inventory, manifest)) = &claim_manifest {
                for stream in manifest.streams() {
                    for claim_id in stream.owner_ids() {
                        let key = (stream.source_path().to_owned(), claim_id.clone());
                        let predecessor = claim_predecessors.get(&key).copied().flatten();
                        let (cohort, cost) = crate::source_navigation_claim_input::
                            assemble_selected_claim_dependency_refresh_set_cohort_v1(
                                &context,
                                identity,
                                validator,
                                &files,
                                inventory,
                                manifest,
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
                        claim_contributions.push(
                            NavigationClaimEventContributionV1::DependencyRefresh(cohort),
                        );
                    }
                }
                claim_refresh_manifest = Some(manifest);
            }

            if claim_revision_proofs.len() > 1 {
                return Err(DurableError::Conflict(
                    "one navigation event produced multiple Claim revision proofs",
                ));
            }
            let claim_revision_proof = claim_revision_proofs.first();
            let mut record_cohorts = Vec::new();
            for record in &record_plan.owners {
                let predecessor = read_record_predecessor(
                    event_reader,
                    selection,
                    &record.source_path,
                    &record.record_id,
                )?;
                let (cohort, cost) = if record.origin
                    == crate::source_navigation_record_input::RecordOwnerOriginV1::DirectSourceCreate
                    || changed_paths.binary_search(&record.source_path).is_ok()
                {
                    crate::source_navigation_record_input::assemble_record_event_cohort_v1(
                        &context,
                        identity,
                        validator,
                        &files,
                        record,
                        predecessor,
                        &mut forms,
                        limits,
                    )?
                } else {
                    let causes = record_refresh_causes(record, &affected_by_path, &context)?;
                    if causes.is_empty() {
                        return Err(DurableError::Conflict(
                            "affected Record has no exact changed dependency cause",
                        ));
                    }
                    let revision_proof = claim_revision_proof.filter(|proof| {
                        causes.iter().any(|cause| {
                            cause.path.as_str() == proof.claim_stream_path().as_str()
                                || cause.path.as_str() == proof.history_path().as_str()
                        })
                    });
                    if let Some(proof) = revision_proof {
                        crate::source_navigation_record_input::assemble_record_claim_revision_refresh_cohort_v1(
                            &context,
                            identity,
                            validator,
                            &files,
                            record,
                            predecessor,
                            proof,
                            &mut forms,
                            limits,
                        )?
                    } else {
                        crate::source_navigation_record_input::assemble_record_dependency_refresh_set_cohort_v1(
                            &context,
                            identity,
                            validator,
                            &files,
                            record,
                            predecessor,
                            &causes,
                            &mut forms,
                            limits,
                        )?
                    }
                };
                meter.admit(
                    cost.rows,
                    cost.encoded_bytes,
                    cost.dependency_bytes,
                    cost.dependency_count,
                    selection,
                    &selection.event_limits,
                )?;
                record_cohorts.push(cohort);
            }
            crate::DurableNavigationEventCohortSetV2::from_mixed_context_with_claim_navigation_closure(
                selection.domain,
                &context,
                record_cohorts,
                claim_contributions,
                claim_mutation_binding,
                claim_refresh_manifest,
                &source_revision_closure,
                selection.event_limits,
            )
        },
    )?;
    let apply = event_reader.publish_navigation_event_cohort_set_v2(
        store,
        selection.domain,
        selection.consumer_id,
        &selection.profile,
        &set,
        selection.event_limits,
    )?;
    // Keep all exact identity readsets alive through the atomic publisher.
    drop(held_identity_readsets);
    Ok(Some(apply))
}

fn read_current_event_context(
    event_reader: &mut DurablePgCoordinator,
    store: &SegmentStore,
    selection: &NavigationProducerOwnerSelection<'_>,
    seed: &DurableNavigationEventContextV1,
    paths: &BTreeSet<String>,
) -> DurableResult<DurableNavigationEventContextV1> {
    let selected_paths = paths.iter().cloned().collect::<Vec<_>>();
    let context = event_reader.read_navigation_current_event_sources_v1(
        store,
        selection.domain,
        selection.consumer_id,
        seed.event_sequence(),
        selection.event_path,
        &selected_paths,
        &selection.profile,
        selection.event_limits,
    )?;
    if context.event_id() != seed.event_id()
        || context.event_sequence() != seed.event_sequence()
        || context.source_head_sequence() != seed.source_head_sequence()
        || context.consumer_cursor().sequence != seed.consumer_cursor().sequence
        || event_member_digest(&context)? != event_member_digest(seed)?
    {
        return Err(DurableError::Conflict(
            "navigation event changed while selecting its dependency closure",
        ));
    }
    Ok(context)
}

fn stage_event_context(
    event_reader: &mut DurablePgCoordinator,
    store: &SegmentStore,
    selection: &NavigationProducerOwnerSelection<'_>,
    seed: &DurableNavigationEventContextV1,
    context: &mut DurableNavigationEventContextV1,
    additions: impl IntoIterator<Item = String>,
) -> DurableResult<()> {
    let mut paths = initial_event_paths(selection, seed)?
        .into_iter()
        .collect::<BTreeSet<_>>();
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
    *context = read_current_event_context(event_reader, store, selection, seed, &paths)?;
    Ok(())
}

fn verify_claim_member_facts(
    context: &DurableNavigationEventContextV1,
    closure: &crate::source_cohort::revision::SourceRevisionClaimNavigationClosure,
) -> DurableResult<()> {
    for (path, fact) in closure.member_facts() {
        let selected = context
            .selected_sources()
            .iter()
            .find(|source| source.source_path == *path)
            .ok_or(DurableError::Conflict(
                "reverse Claim stream is not selected",
            ))?;
        let bytes = selected_source_bytes(context, path)?;
        let event_member = context
            .members()
            .iter()
            .find(|member| member.source_path == *path);
        if !selected.is_source
            || selected.commit_sequence != Some(fact.commit_seq)
            || selected.content_digest != Some(fact.digest)
            || selected.content_length != Some(fact.size_bytes)
            || Digest256::of_bytes(bytes) != fact.digest
            || bytes.len() as u64 != fact.size_bytes
            || event_member.is_some_and(|member| {
                !member.is_source
                    || member.content_digest != Some(fact.digest)
                    || member.content_length != Some(fact.size_bytes)
                    || member.commit_sequence != Some(fact.commit_seq)
            })
        {
            return Err(DurableError::Conflict(
                "reverse Claim stream differs from the event source cut",
            ));
        }
    }
    Ok(())
}

struct OwnedClaimRefreshCauseV1 {
    path: String,
    sha256: Digest256,
    reverse_claim_ids: Vec<String>,
}

/// Resolve the exact receipt for one directly changed Claim stream from the
/// committed member set. Creation uses the retained create receipt; revision
/// uses the changed Claim history, whose latest receipt is checked by the
/// compiler's revision-proof builder.
fn direct_claim_mutation(
    seed: &DurableNavigationEventContextV1,
    changed_paths: &[String],
    max_receipt_bytes: usize,
) -> DurableResult<Option<(String, String, Vec<String>)>> {
    let streams = changed_paths
        .iter()
        .filter(|path| path.ends_with("/source-claims.jsonl"))
        .collect::<Vec<_>>();
    if streams.is_empty() {
        return Ok(None);
    }
    if streams.len() != 1 {
        return Err(DurableError::Refused(
            "one navigation event cannot bind multiple directly mutated Claim streams",
        ));
    }
    let stream_path = streams[0].as_str();
    let home = stream_path
        .rsplit_once('/')
        .map(|(home, _)| home)
        .ok_or(DurableError::Invalid("Claim mutation stream home"))?;
    let candidates = [
        format!("{home}/source-create-receipt.json"),
        format!("{home}/claim-revision-history.json"),
    ];
    let receipts = candidates
        .into_iter()
        .filter(|path| changed_paths.binary_search(path).is_ok())
        .collect::<Vec<_>>();
    if receipts.len() != 1 {
        return Err(DurableError::Refused(
            "direct Claim mutation lacks one exact create or revision receipt member",
        ));
    }
    let receipt_path = receipts.into_iter().next().expect("one receipt checked");
    let receipt_bytes = selected_source_bytes(seed, &receipt_path)?;
    let mutation_ids = crate::source_navigation_claim_selection::claim_event_mutation_claim_ids_v1(
        receipt_bytes,
        max_receipt_bytes,
    )?;
    if receipt_path.ends_with("/claim-revision-history.json") && mutation_ids.len() != 1 {
        return Err(DurableError::Refused(
            "Claim history receipt does not bind one exact revision",
        ));
    }
    Ok(Some((stream_path.to_owned(), receipt_path, mutation_ids)))
}

fn claim_refresh_causes(
    context: &DurableNavigationEventContextV1,
    closure: &crate::source_cohort::revision::SourceRevisionClaimNavigationClosure,
    changed_paths: &[String],
    ids_by_path: &BTreeMap<String, BTreeSet<String>>,
) -> DurableResult<Vec<OwnedClaimRefreshCauseV1>> {
    use crate::source_claim_dependency_index::ClaimNavigationReferenceKind;

    let mut causes = Vec::with_capacity(changed_paths.len());
    for path in changed_paths {
        if path.ends_with("/source-claims.jsonl") {
            return Err(DurableError::Refused(
                "Claim mutation cannot be a dependency-refresh cause",
            ));
        }
        let mut reverse_ids = BTreeSet::new();
        if let Some(observation) = closure
            .lookups()
            .get(&(ClaimNavigationReferenceKind::SourcePath, path.clone()))
        {
            reverse_ids.extend(observation.rows.iter().map(|row| row.claim_id.clone()));
        }
        if let Some(changed_ids) = ids_by_path.get(path) {
            for id in changed_ids {
                if let Some(observation) = closure
                    .lookups()
                    .get(&(ClaimNavigationReferenceKind::TosId, id.clone()))
                {
                    reverse_ids.extend(observation.rows.iter().map(|row| row.claim_id.clone()));
                }
            }
        }
        let bytes = selected_source_bytes(context, path)?;
        causes.push(OwnedClaimRefreshCauseV1 {
            path: path.clone(),
            sha256: Digest256::of_bytes(bytes),
            reverse_claim_ids: reverse_ids.into_iter().collect(),
        });
    }
    if causes.is_empty() {
        return Err(DurableError::Refused(
            "navigation Claim refresh has no ToS source causes",
        ));
    }
    Ok(causes)
}

fn claim_source_files<'a>(
    context: &'a DurableNavigationEventContextV1,
    closure: &crate::source_cohort::revision::SourceRevisionClaimNavigationClosure,
    current_stream_paths: &[String],
    historical_endpoint_paths: &[String],
) -> DurableResult<Vec<NavigationClaimEventSourceFileV1<'a>>> {
    let mut paths = BTreeSet::new();
    for path in current_stream_paths.iter().chain(historical_endpoint_paths) {
        paths.insert(path.as_str());
    }
    if paths.len() != current_stream_paths.len() + historical_endpoint_paths.len() {
        return Err(DurableError::Conflict(
            "Claim inventory source path is repeated",
        ));
    }
    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
        let bytes = selected_source_bytes(context, path)?;
        if let Some(fact) = closure.member_facts().get(path) {
            let selected = context
                .selected_sources()
                .iter()
                .find(|source| source.source_path == path)
                .ok_or(DurableError::Conflict(
                    "Claim inventory stream member is absent",
                ))?;
            if selected.commit_sequence != Some(fact.commit_seq)
                || selected.content_digest != Some(fact.digest)
                || selected.content_length != Some(fact.size_bytes)
                || Digest256::of_bytes(bytes) != fact.digest
                || bytes.len() as u64 != fact.size_bytes
            {
                return Err(DurableError::Conflict(
                    "Claim inventory stream bytes differ from readset closure",
                ));
            }
        } else if current_stream_paths.iter().any(|stream| stream == path) {
            // A directly mutated stream is already pinned by the exact CMD
            // member. It need not also appear as a reverse-index result.
            let member = context.members().iter().find(|member| {
                member.source_path == path
                    && member.is_source
                    && member.commit_sequence == Some(context.event_sequence())
            });
            let selected = context
                .selected_sources()
                .iter()
                .find(|source| source.source_path == path);
            if !member.is_some_and(|member| {
                member.content_digest == Some(Digest256::of_bytes(bytes))
                    && member.content_length == Some(bytes.len() as u64)
            }) || !selected.is_some_and(|source| {
                source.is_source
                    && source.commit_sequence == Some(context.event_sequence())
                    && source.content_digest == Some(Digest256::of_bytes(bytes))
                    && source.content_length == Some(bytes.len() as u64)
            }) {
                return Err(DurableError::Conflict(
                    "Claim inventory stream is neither reverse-indexed nor an exact event member",
                ));
            }
        }
        files.push(NavigationClaimEventSourceFileV1 { path, bytes });
    }
    Ok(files)
}

#[allow(clippy::too_many_arguments)]
fn prepare_direct_claim_mutation(
    event_reader: &mut DurablePgCoordinator,
    store: &SegmentStore,
    owner: &NavigationProducerHeldOwnerIdentity,
    selection: &NavigationProducerOwnerSelection<'_>,
    seed: &DurableNavigationEventContextV1,
    context: &mut DurableNavigationEventContextV1,
    closure: &crate::source_cohort::revision::SourceRevisionClaimNavigationClosure,
    mutation_source_path: &str,
    mutation_receipt_path: &str,
    mutation_ids: &[String],
    reverse_claim_ids: &[String],
    reverse_claim_paths: &[String],
    limits: BibliographicLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    identity_owners: &mut Vec<crate::source_navigation_claim_selection::ClaimEventIdentityOwnerV1>,
    predecessors: &mut BTreeMap<(String, String), Option<Digest256>>,
    held_readsets: &mut Vec<crate::source_cohort::revision::SourceRevisionReadSet>,
) -> DurableResult<(
    tos_compiler::source_navigation_claim_event::NavigationClaimEventOwnerInventoryV1,
    tos_compiler::source_navigation_claim_event::NavigationClaimEventAffectedOwnerManifestV1,
)> {
    let mut current_stream_paths = reverse_claim_paths.to_vec();
    current_stream_paths.push(mutation_source_path.to_owned());
    current_stream_paths.sort();
    current_stream_paths.dedup();
    let initial_files = claim_source_files(context, closure, &current_stream_paths, &[])?;
    let inventory = tos_compiler::source_navigation_claim_event::
        prepare_navigation_claim_event_owner_inventory_v1(&initial_files, limits)
        .map_err(|_| DurableError::Refused("selected Claim owner inventory refused"))?;
    let manifest = inventory
        .affected_owner_streams(reverse_claim_ids, mutation_source_path, mutation_ids)
        .map_err(|_| DurableError::Conflict("Claim mutation owner manifest differs"))?;

    let paths = crate::source_navigation_claim_selection::claim_event_affected_stream_paths_v1(
        context, &manifest,
    )?;
    stage_event_context(event_reader, store, selection, seed, context, paths)?;
    let paths =
        crate::source_navigation_claim_selection::claim_event_affected_current_owner_paths_v1(
            context, &manifest,
        )?;
    stage_event_context(event_reader, store, selection, seed, context, paths)?;

    let requests =
        crate::source_navigation_claim_selection::claim_event_affected_identity_requests_v1(
            context, &manifest,
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
            if selected.identity() != closure.selection_identity() {
                return Err(DurableError::Conflict(
                    "Claim identity lookup uses another source owner epoch",
                ));
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
    *identity_owners = requests
        .iter()
        .map(|request| {
            let identity_owner =
                identity_closure
                    .owners()
                    .get(&request.id)
                    .ok_or(DurableError::Conflict(
                        "Claim identity lookup result absent",
                    ))?;
            Ok(
                crate::source_navigation_claim_selection::ClaimEventIdentityOwnerV1 {
                    id: request.id.clone(),
                    kind: identity_owner.as_ref().map(|(kind, _)| kind.clone()),
                    source_path: identity_owner
                        .as_ref()
                        .map(|(_, path)| path.as_str().to_owned()),
                },
            )
        })
        .collect::<DurableResult<Vec<_>>>()?;
    held_readsets.push(identity_readset);

    let paths = crate::source_navigation_claim_selection::claim_event_affected_indexed_paths_v1(
        context,
        &manifest,
        identity_owners,
    )?;
    stage_event_context(event_reader, store, selection, seed, context, paths)?;
    let schema_paths =
        crate::source_navigation_claim_selection::claim_event_affected_schema_paths_v1(
            context,
            &manifest,
            identity_owners,
        )?;
    require_configured_schemas(selection, &schema_paths)?;
    stage_event_context(event_reader, store, selection, seed, context, schema_paths)?;
    let historical_endpoint_paths = crate::source_navigation_claim_selection::
        claim_event_affected_historical_endpoint_paths_v1(context, &manifest, identity_owners)?;
    stage_event_context(
        event_reader,
        store,
        selection,
        seed,
        context,
        historical_endpoint_paths.clone(),
    )?;
    let historical_capture_paths =
        crate::source_navigation_claim_selection::claim_event_affected_historical_capture_paths_v1(
            context,
            &manifest,
            identity_owners,
        )?;
    stage_event_context(
        event_reader,
        store,
        selection,
        seed,
        context,
        historical_capture_paths,
    )?;
    let archive_manifests =
        crate::source_navigation_claim_selection::claim_event_affected_archive_manifest_paths_v1(
            context, &manifest,
        )?;
    stage_event_context(
        event_reader,
        store,
        selection,
        seed,
        context,
        archive_manifests,
    )?;
    let archive_blobs =
        crate::source_navigation_claim_selection::claim_event_affected_archive_blob_paths_v1(
            context, &manifest,
        )?;
    stage_event_context(event_reader, store, selection, seed, context, archive_blobs)?;
    let identity_archive_manifests = crate::source_navigation_claim_selection::
        claim_event_affected_identity_archive_manifest_paths_v1(context, &manifest, identity_owners)?;
    stage_event_context(
        event_reader,
        store,
        selection,
        seed,
        context,
        identity_archive_manifests,
    )?;
    let identity_archive_blobs = crate::source_navigation_claim_selection::
        claim_event_affected_identity_archive_blob_paths_v1(context, &manifest, identity_owners)?;
    stage_event_context(
        event_reader,
        store,
        selection,
        seed,
        context,
        identity_archive_blobs,
    )?;

    let historical_endpoint_paths = crate::source_navigation_claim_selection::
        claim_event_affected_historical_endpoint_paths_v1(context, &manifest, identity_owners)?;
    let final_stream_paths = manifest
        .streams()
        .iter()
        .map(|stream| stream.source_path().to_owned())
        .collect::<Vec<_>>();
    let final_files = claim_source_files(
        context,
        closure,
        &final_stream_paths,
        &historical_endpoint_paths,
    )?;
    let final_inventory = tos_compiler::source_navigation_claim_event::
        prepare_navigation_claim_event_owner_inventory_with_endpoints_v1(
            &final_files,
            &historical_endpoint_paths,
            limits,
        )
        .map_err(|_| DurableError::Refused("final Claim owner inventory refused"))?;
    let final_manifest = final_inventory
        .affected_owner_streams(reverse_claim_ids, mutation_source_path, mutation_ids)
        .map_err(|_| DurableError::Conflict("final Claim mutation manifest differs"))?;
    if final_manifest.streams() != manifest.streams()
        || selected_source_bytes(context, mutation_receipt_path).is_err()
    {
        return Err(DurableError::Conflict(
            "Claim mutation owner closure or receipt changed after selection",
        ));
    }
    for stream in final_manifest.streams() {
        for claim_id in stream.owner_ids() {
            let root =
                read_record_predecessor(event_reader, selection, stream.source_path(), claim_id)?;
            predecessors.insert((stream.source_path().to_owned(), claim_id.clone()), root);
        }
    }
    Ok((final_inventory, final_manifest))
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

fn read_record_predecessor(
    event_reader: &mut DurablePgCoordinator,
    selection: &NavigationProducerOwnerSelection<'_>,
    source_path: &str,
    record_id: &str,
) -> DurableResult<Option<Digest256>> {
    Ok(event_reader
        .read_navigation_event_cohort_v1(
            selection.domain,
            selection.consumer_id,
            &selection.profile,
            source_path,
            record_id,
            selection.event_limits,
        )?
        .map(|cohort| cohort.output_root))
}

fn read_claim_predecessors(
    event_reader: &mut DurablePgCoordinator,
    selection: &NavigationProducerOwnerSelection<'_>,
    manifest: &tos_compiler::source_navigation_claim_event::
        NavigationClaimDependencyRefreshOwnerManifestSetV1,
) -> DurableResult<BTreeMap<(String, String), Option<Digest256>>> {
    let mut predecessors = BTreeMap::new();
    for stream in manifest.streams() {
        for claim_id in stream.owner_ids() {
            let root =
                read_record_predecessor(event_reader, selection, stream.source_path(), claim_id)?;
            predecessors.insert((stream.source_path().to_owned(), claim_id.clone()), root);
        }
    }
    Ok(predecessors)
}

fn record_refresh_causes(
    owner: &crate::source_navigation_record_input::RecordOwnerV1,
    affected_by_path: &BTreeMap<String, Vec<crate::DurableNavigationEventAffectedOwnerV1>>,
    context: &DurableNavigationEventContextV1,
) -> DurableResult<Vec<NavigationRecordDependencyRefreshCauseV1>> {
    let mut causes = Vec::new();
    for (path, affected) in affected_by_path {
        if path == &owner.source_path
            || !affected.iter().any(|candidate| {
                candidate.kind() == crate::DurableNavigationEventAffectedOwnerKindV1::Record
                    && candidate.source_path() == owner.source_path
                    && candidate.record_id() == owner.record_id
            })
        {
            continue;
        }
        let bytes = selected_source_bytes(context, path)?;
        causes.push(NavigationRecordDependencyRefreshCauseV1 {
            path: RelativePath::parse(path)
                .map_err(|_| DurableError::Corrupt("Record refresh cause path"))?,
            sha256: Digest256::of_bytes(bytes),
        });
    }
    causes.sort_by(|left, right| left.path.cmp(&right.path));
    if causes.windows(2).any(|pair| pair[0].path >= pair[1].path) {
        return Err(DurableError::Corrupt(
            "Record refresh cause paths are not unique",
        ));
    }
    Ok(causes)
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
    limits
        .validate()
        .map_err(|_| DurableError::Refused("bibliographic event limits refused"))?;
    Ok(limits)
}

/// Open a second selected PG client for the event reader while proving it
/// addresses the exact database and STO held by the protected owner callback.
/// The callback's transaction is the owner-side database witness; the producer
/// never trusts a fresh environment lookup without comparing both witnesses.
pub(crate) fn open_selected_event_store(
    owner: &NavigationProducerHeldOwnerIdentity,
    selection: &NavigationProducerOwnerSelection<'_>,
    owner_tx: &mut postgres::Transaction<'_>,
) -> DurableResult<(SegmentStore, DurablePgCoordinator)> {
    let store = SegmentStore::open_existing(&selection.store_root, selection.store_limits)
        .map_err(|_| DurableError::Refused("selected navigation source store unavailable"))?;
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
    Ok((store, DurablePgCoordinator::from_configured_client(client)))
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

    pub(crate) fn source_head_sequence(&self) -> u64 {
        self.source_head_sequence
    }

    pub(crate) fn event_sequence(&self) -> u64 {
        self.event_sequence
    }
}

/// Compute the event-only source identity after confirming that the protected
/// owner and selected PG event describe the same current source head. Every
/// source member is reopened by the caller into `selected_sources`; this check
/// verifies its exact bytes against the committed member digest and length.
pub(crate) fn event_source_cut_identity(
    owner: &NavigationProducerHeldOwnerIdentity,
    selection: &NavigationProducerOwnerSelection<'_>,
    context: &DurableNavigationEventContextV1,
) -> DurableResult<NavigationEventSourceCutIdentityV1> {
    if context.event_kind() != "command"
        || context.event_sequence() == 0
        || context.event_sequence() != context.source_head_sequence()
        || context.source_head_sequence() != owner.source_head_sequence
        || context.source_head_sequence() != owner.source_generation
        || owner.domain != selection.domain
        || owner.domain_sha256 != Digest256::of_bytes(selection.domain.as_bytes())
        || owner.source_contract != selection.source_contract
        || owner.owner_configuration_sha256 != selection.owner_configuration_sha256
    {
        return Err(DurableError::Conflict(
            "navigation event differs from the held current source owner",
        ));
    }

    let owner_binding = owner_identity_digest(owner);
    let event_member_binding = event_member_digest(context)?;
    let mut hasher = Digest256Hasher::new();
    hash_part(&mut hasher, b"tos-navigation-event-source-cut-v1");
    hash_part(&mut hasher, owner_binding.as_bytes());
    hash_part(&mut hasher, event_member_binding.as_bytes());
    hash_part(&mut hasher, context.event_id().as_bytes());
    hash_part(&mut hasher, &context.event_sequence().to_be_bytes());
    hash_part(&mut hasher, &context.source_head_sequence().to_be_bytes());
    Ok(NavigationEventSourceCutIdentityV1 {
        owner_binding,
        event_member_binding,
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

/// Seed the first addressed reread with the event source, all exact members,
/// owner-requested addresses, and schema-worker inputs. This list is bounded
/// before the PG reader retains bytes and forms the base for later owner
/// closure stages.
pub(crate) fn initial_event_paths(
    selection: &NavigationProducerOwnerSelection<'_>,
    seed: &DurableNavigationEventContextV1,
) -> DurableResult<Vec<String>> {
    let maximum = selection.event_limits.max_dependencies;
    let mut paths = BTreeSet::new();
    extend_event_paths(&mut paths, [selection.event_path.to_owned()], maximum)?;
    extend_event_paths(
        &mut paths,
        selection.requested_paths.iter().cloned(),
        maximum,
    )?;
    extend_event_paths(&mut paths, selection.schema_paths.iter().cloned(), maximum)?;
    extend_event_paths(
        &mut paths,
        seed.members()
            .iter()
            .map(|member| member.source_path.clone()),
        maximum,
    )?;
    Ok(paths.into_iter().collect())
}

/// Extract and authenticate every source member introduced by this exact
/// command. These paths drive both persisted owner lookup and reverse Claim
/// lookup; historical or unchanged members are never treated as causes.
pub(crate) fn changed_event_source_paths(
    context: &DurableNavigationEventContextV1,
) -> DurableResult<Vec<String>> {
    if context.event_kind() != "command"
        || context.event_sequence() == 0
        || context.event_sequence() != context.source_head_sequence()
    {
        return Err(DurableError::Conflict(
            "navigation changed-source event is not the current command head",
        ));
    }
    let mut paths = Vec::new();
    for member in context.members().iter().filter(|member| {
        member.is_source && member.commit_sequence == Some(context.event_sequence())
    }) {
        let bytes = selected_source_bytes(context, &member.source_path)?;
        if member.content_digest != Some(Digest256::of_bytes(bytes))
            || member.content_length != Some(bytes.len() as u64)
        {
            return Err(DurableError::Conflict(
                "navigation changed-source bytes differ from event member",
            ));
        }
        paths.push(member.source_path.clone());
    }
    paths.sort();
    paths.dedup();
    if paths.is_empty() {
        return Err(DurableError::Refused(
            "navigation event has no source-change proof",
        ));
    }
    Ok(paths)
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
        .map_err(|_| DurableError::Refused("navigation selected source file set refused"))
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
    hash_part(&mut hasher, b"tos-navigation-held-pg-owner-v1");
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
        owner.current_addressed_tree_commitment.as_bytes(),
    );
    hash_part(&mut hasher, owner.managed_generation_digest.as_bytes());
    hash_part(
        &mut hasher,
        &owner.current_source_member_count.to_be_bytes(),
    );
    hash_part(&mut hasher, owner.source_projection_digest.as_bytes());
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

/// Build one candidate schema worker from exact configured schema paths in
/// the final event context, run the owner compiler callback, then close the
/// worker before the caller returns a publishable cohort set.
pub(crate) fn with_event_schema_validator<T>(
    owner_identity: &NavigationProducerHeldOwnerIdentity,
    selection: &NavigationProducerOwnerSelection<'_>,
    context: &DurableNavigationEventContextV1,
    identity: NavigationEventSourceCutIdentityV1,
    deadline: Instant,
    cancelled: &AtomicBool,
    consume: impl FnOnce(&SourceCatalogValidator<'_>) -> DurableResult<T>,
) -> DurableResult<T> {
    if event_source_cut_identity(owner_identity, selection, context)? != identity {
        return Err(DurableError::Conflict(
            "navigation event schema validator source identity changed",
        ));
    }
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
    .map_err(|_| DurableError::Refused("selected navigation catalog binding refused"))?;
    let result = consume(&validator)?;
    validator
        .finish()
        .map_err(|_| DurableError::Refused("selected navigation schema worker finish refused"))?;
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
