//! Owner-side adapter for compiling one exact CMD event into a Record cohort.
//!
//! The caller selects the complete event-local source closure in one PG read
//! context and prepares its event-bound schema validator. This adapter binds
//! that selected input to the consumer cursor and event identity, then admits
//! the sealed cohort only after its incremental budget has been computed.

use tos_compiler::{
    source_bibliographic::BibliographicLimits,
    source_navigation_claim_event::NavigationClaimRevisionProofV1,
    source_navigation_event::{
        NavigationDependencyStateV1, NavigationRecordDependencyRefreshCauseV1,
        NavigationRecordEventBudgetCostV1, NavigationRecordEventCohortV1,
        NavigationRecordEventInputV1,
        project_navigation_record_claim_revision_refresh_from_event_cut_v1,
        project_navigation_record_dependency_refresh_from_event_cut_v1,
        project_navigation_record_dependency_refresh_set_from_event_cut_v1,
        project_navigation_record_event_from_event_cut_v1,
    },
    source_witness_catalog::CandidateEventSourceIdentityV1,
};
use tos_foundation::Digest256;

use crate::{
    DurableError, DurableNavigationEventContextV1, DurableResult,
    source_navigation_producer::NavigationEventSourceCutIdentityV1,
};

/// Compile one metadata mutation from the exact selected event context.
///
/// `input` is assembled from the final context, including the complete record,
/// forms, history, archive, registry, schema, and committed-member closure.
/// This function does not discover dependencies: an omitted committed member
/// or a stale cursor/validator binding is rejected before the cohort is
/// returned to the publisher.
pub(crate) fn project_record_event_cohort_v1(
    context: &DurableNavigationEventContextV1,
    identity: NavigationEventSourceCutIdentityV1,
    input: NavigationRecordEventInputV1<'_, '_>,
    limits: BibliographicLimits,
) -> DurableResult<(
    NavigationRecordEventCohortV1,
    NavigationRecordEventBudgetCostV1,
)> {
    project_record_event_with_basis(context, identity, input, limits, |input, limits| {
        project_navigation_record_event_from_event_cut_v1(input, limits)
    })
}

/// Reproject an unchanged Record after one exact dependency changes in the
/// selected event cut. The compiler seals the refresh basis from the changed
/// path and bytes after the shared context checks below pass.
pub(crate) fn project_record_dependency_refresh_cohort_v1(
    context: &DurableNavigationEventContextV1,
    identity: NavigationEventSourceCutIdentityV1,
    input: NavigationRecordEventInputV1<'_, '_>,
    changed_dependency_path: &str,
    changed_dependency_bytes: &[u8],
    limits: BibliographicLimits,
) -> DurableResult<(
    NavigationRecordEventCohortV1,
    NavigationRecordEventBudgetCostV1,
)> {
    project_record_event_with_basis(context, identity, input, limits, |input, limits| {
        project_navigation_record_dependency_refresh_from_event_cut_v1(
            input,
            changed_dependency_path,
            changed_dependency_bytes,
            limits,
        )
    })
}

/// Reproject one unchanged Record for a compound command, sealing every
/// changed dependency that participates in this owner's exact current input.
pub(crate) fn project_record_dependency_refresh_set_cohort_v1(
    context: &DurableNavigationEventContextV1,
    identity: NavigationEventSourceCutIdentityV1,
    input: NavigationRecordEventInputV1<'_, '_>,
    changed_dependencies: &[NavigationRecordDependencyRefreshCauseV1],
    limits: BibliographicLimits,
) -> DurableResult<(
    NavigationRecordEventCohortV1,
    NavigationRecordEventBudgetCostV1,
)> {
    project_record_event_with_basis(context, identity, input, limits, |input, limits| {
        project_navigation_record_dependency_refresh_set_from_event_cut_v1(
            input,
            changed_dependencies,
            limits,
        )
    })
}

/// Reproject an unchanged Record after one exact Claim revision. The Claim
/// proof is separately produced from its selected stream, history and archive
/// closure, then checked against the same CMD event identity.
pub(crate) fn project_record_claim_revision_refresh_cohort_v1(
    context: &DurableNavigationEventContextV1,
    identity: NavigationEventSourceCutIdentityV1,
    input: NavigationRecordEventInputV1<'_, '_>,
    proof: &NavigationClaimRevisionProofV1,
    limits: BibliographicLimits,
) -> DurableResult<(
    NavigationRecordEventCohortV1,
    NavigationRecordEventBudgetCostV1,
)> {
    project_record_event_with_basis(context, identity, input, limits, |input, limits| {
        project_navigation_record_claim_revision_refresh_from_event_cut_v1(input, proof, limits)
    })
}

fn project_record_event_with_basis(
    context: &DurableNavigationEventContextV1,
    identity: NavigationEventSourceCutIdentityV1,
    input: NavigationRecordEventInputV1<'_, '_>,
    limits: BibliographicLimits,
    project: impl FnOnce(
        NavigationRecordEventInputV1<'_, '_>,
        BibliographicLimits,
    ) -> tos_compiler::Result<NavigationRecordEventCohortV1>,
) -> DurableResult<(
    NavigationRecordEventCohortV1,
    NavigationRecordEventBudgetCostV1,
)> {
    let cursor = context.consumer_cursor();
    let predecessor_source_revision = cursor.source_revision.ok_or(DurableError::Conflict(
        "navigation Record consumer cursor has no source revision",
    ))?;
    if context.event_kind() != "command"
        || context.event_id() != input.event_id
        || context.event_sequence() != input.event_sequence
        || context.source_head_sequence() != input.source_head_sequence
        || context.source_head_sequence() != identity.source_head_sequence()
        || context.event_sequence() != identity.event_sequence()
        || cursor.sequence >= context.event_sequence()
        || input.predecessor_source_revision != predecessor_source_revision
    {
        return Err(DurableError::Conflict(
            "navigation Record input differs from the selected event cursor",
        ));
    }
    let selected_revision = input
        .validator
        .selected_event_source_revision()
        .map_err(|_| DurableError::Conflict("navigation Record validator binding differs"))?;
    if selected_revision != identity.event_source_revision() {
        return Err(DurableError::Conflict(
            "navigation Record validator uses another event source revision",
        ));
    }
    validate_committed_member_dependencies(context, &input)?;

    let cohort = project(input, limits)
        .map_err(|_| DurableError::Refused("selected navigation Record projection refused"))?;
    if cohort.event_id() != context.event_id()
        || cohort.event_sequence() != context.event_sequence()
        || cohort.source_revision() != identity.event_source_revision()
        || cohort.predecessor_source_revision() != predecessor_source_revision
    {
        return Err(DurableError::Conflict(
            "navigation Record cohort differs from its selected event context",
        ));
    }
    let cost = cohort
        .budget_cost()
        .map_err(|_| DurableError::Refused("selected navigation Record budget refused"))?;
    Ok((cohort, cost))
}

fn validate_committed_member_dependencies(
    context: &DurableNavigationEventContextV1,
    input: &NavigationRecordEventInputV1<'_, '_>,
) -> DurableResult<()> {
    for member in context.members() {
        let mut matches = input
            .dependencies
            .iter()
            .filter(|dependency| dependency.path.as_str() == member.source_path);
        let dependency = matches.next().ok_or(DurableError::Conflict(
            "navigation Record dependency closure omits a committed member",
        ))?;
        if matches.next().is_some() {
            return Err(DurableError::Corrupt(
                "navigation Record dependency closure repeats a committed member",
            ));
        }
        match (&dependency.state, member.is_source) {
            (NavigationDependencyStateV1::Present { sha256, size_bytes }, true)
                if Some(*sha256) == member.content_digest
                    && Some(*size_bytes) == member.content_length => {}
            (NavigationDependencyStateV1::Absent { .. }, false) => {}
            _ => {
                return Err(DurableError::Conflict(
                    "navigation Record dependency differs from a committed member",
                ));
            }
        }
    }

    let event_dependency = input
        .dependencies
        .iter()
        .find(|dependency| {
            dependency.role
                == tos_compiler::source_navigation_event::NavigationDependencyRoleV1::Event
                && dependency.path.as_str() == input.event_path
        })
        .ok_or(DurableError::Conflict(
            "navigation Record event dependency is absent",
        ))?;
    let event_digest = Digest256::of_bytes(input.event_file_bytes);
    match &event_dependency.state {
        NavigationDependencyStateV1::Present { sha256, size_bytes }
            if *sha256 == event_digest && *size_bytes == input.event_file_bytes.len() as u64 => {}
        _ => {
            return Err(DurableError::Conflict(
                "navigation Record event dependency bytes differ",
            ));
        }
    }
    Ok(())
}
