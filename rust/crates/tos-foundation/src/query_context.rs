//! Frozen knowledge selection. Callers enforce access before resolving a context.
//! This module does not infer dates, grant access, persist callers, or filter world time.
use crate::{ExactRecordRef, JsonValue, SourceRevision, StableId};

/// A source-owned cutoff value, retained exactly with its explicitly supplied TRS.
#[derive(Clone, Debug, PartialEq)]
pub struct Cutoff {
    pub value: JsonValue,
    pub trs: Option<ExactRecordRef>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct SourceSnapshotRef {
    pub id: StableId,
    pub revision: SourceRevision,
}
#[derive(Clone, Debug, PartialEq)]
pub struct SourceSnapshot {
    pub reference: SourceSnapshotRef,
    pub current_cutoff: Cutoff,
}
#[derive(Clone, Debug, PartialEq)]
pub enum SystemState {
    Current,
    AsOf(Cutoff),
}
#[derive(Clone, Debug, PartialEq)]
pub struct AttributedKnowledge {
    pub knower: StableId,
    pub cutoff: Cutoff,
}
#[derive(Clone, Debug, PartialEq)]
pub struct KnowledgeSelection {
    pub source_snapshot: SourceSnapshotRef,
    pub system_as_of: Cutoff,
    pub attributed_knowledge: Option<AttributedKnowledge>,
    pub source_publication_as_of: Option<Cutoff>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ChronologyVersion {
    pub reference: ExactRecordRef,
    pub selection: KnowledgeSelection,
    pub evidence_policy: ExactRecordRef,
    pub selection_policy: ExactRecordRef,
    pub member_claim_revisions: Vec<ExactRecordRef>,
}
/// None means omitted/inherit; Some(None) explicitly requests absence.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct QueryContext {
    pub system_state: Option<SystemState>,
    pub attributed_knowledge: Option<Option<AttributedKnowledge>>,
    pub source_publication_as_of: Option<Option<Cutoff>>,
    pub world_valid_time: Option<JsonValue>,
    pub display_perspective: Option<JsonValue>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MismatchDimension {
    SourceSnapshotRef,
    SourceRevisionFence,
    SystemState,
    AttributedKnowledgeAsOf,
    SourcePublicationAsOf,
}
#[derive(Clone, Debug, PartialEq)]
pub struct EffectiveQueryContext {
    pub selection: KnowledgeSelection,
    pub chronology_version: Option<ExactRecordRef>,
    pub world_valid_time: Option<JsonValue>,
    pub display_perspective: Option<JsonValue>,
}
#[derive(Clone, Debug, PartialEq)]
pub enum ContextResolution {
    Resolved(EffectiveQueryContext),
    MissingSelection,
    ContextMismatch(Vec<MismatchDimension>),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextError {
    InvalidCutoff,
    DuplicateClaimRevision,
}

pub(crate) fn same_json(a: &JsonValue, b: &JsonValue) -> bool {
    match (a, b) {
        (JsonValue::Object(a), JsonValue::Object(b)) => {
            a.len() == b.len()
                && a.iter().all(|(key, value)| {
                    b.iter()
                        .find(|(k, _)| k == key)
                        .is_some_and(|(_, other)| same_json(value, other))
                })
        }
        (JsonValue::Array(a), JsonValue::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same_json(a, b))
        }
        _ => a == b,
    }
}
fn same_cutoff(a: &Cutoff, b: &Cutoff) -> bool {
    a.trs == b.trs && same_json(&a.value, &b.value)
}
fn same_optional_cutoff(a: &Option<Cutoff>, b: &Option<Cutoff>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => same_cutoff(a, b),
        (None, None) => true,
        _ => false,
    }
}
fn same_attribution(a: &Option<AttributedKnowledge>, b: &Option<AttributedKnowledge>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a.knower == b.knower && same_cutoff(&a.cutoff, &b.cutoff),
        (None, None) => true,
        _ => false,
    }
}
fn valid_cutoff(c: &Cutoff) -> bool {
    !matches!(&c.value, JsonValue::Null)
        && !matches!(&c.value, JsonValue::String(s) if s.units().is_empty())
}
fn validate(selection: &KnowledgeSelection) -> Result<(), ContextError> {
    if !valid_cutoff(&selection.system_as_of)
        || selection
            .attributed_knowledge
            .as_ref()
            .is_some_and(|a| !valid_cutoff(&a.cutoff))
        || selection
            .source_publication_as_of
            .as_ref()
            .is_some_and(|c| !valid_cutoff(c))
    {
        return Err(ContextError::InvalidCutoff);
    }
    Ok(())
}
/// Resolve Current against the supplied snapshot, never the host clock.
/// Mismatches are exhaustive and ordered; a new snapshot and fence report both.
pub fn resolve_query_context(
    snapshot: &SourceSnapshot,
    request: &QueryContext,
    chronology: Option<&ChronologyVersion>,
) -> Result<ContextResolution, ContextError> {
    let system = request.system_state.as_ref().map(|s| match s {
        SystemState::Current => snapshot.current_cutoff.clone(),
        SystemState::AsOf(c) => c.clone(),
    });
    if system.as_ref().is_some_and(|c| !valid_cutoff(c)) {
        return Err(ContextError::InvalidCutoff);
    }
    let (selection, chronology_version) = if let Some(frozen) = chronology {
        validate(&frozen.selection)?;
        let mut mismatches = Vec::new();
        let selected = &frozen.selection;
        if snapshot.reference.id != selected.source_snapshot.id {
            mismatches.push(MismatchDimension::SourceSnapshotRef);
        }
        if snapshot.reference.revision != selected.source_snapshot.revision {
            mismatches.push(MismatchDimension::SourceRevisionFence);
        }
        if system
            .as_ref()
            .is_some_and(|c| !same_cutoff(c, &selected.system_as_of))
        {
            mismatches.push(MismatchDimension::SystemState);
        }
        if request
            .attributed_knowledge
            .as_ref()
            .is_some_and(|v| !same_attribution(v, &selected.attributed_knowledge))
        {
            mismatches.push(MismatchDimension::AttributedKnowledgeAsOf);
        }
        if request
            .source_publication_as_of
            .as_ref()
            .is_some_and(|v| !same_optional_cutoff(v, &selected.source_publication_as_of))
        {
            mismatches.push(MismatchDimension::SourcePublicationAsOf);
        }
        if !mismatches.is_empty() {
            return Ok(ContextResolution::ContextMismatch(mismatches));
        }
        (selected.clone(), Some(frozen.reference.clone()))
    } else {
        let Some(system_as_of) = system else {
            return Ok(ContextResolution::MissingSelection);
        };
        (
            KnowledgeSelection {
                source_snapshot: snapshot.reference.clone(),
                system_as_of,
                attributed_knowledge: request.attributed_knowledge.clone().flatten(),
                source_publication_as_of: request.source_publication_as_of.clone().flatten(),
            },
            None,
        )
    };
    validate(&selection)?;
    Ok(ContextResolution::Resolved(EffectiveQueryContext {
        selection,
        chronology_version,
        world_valid_time: request.world_valid_time.clone(),
        display_perspective: request.display_perspective.clone(),
    }))
}
/// Construct a concrete immutable-value manifest. Storage must append it through
/// the existing authoritative writer; constructing this value grants no admission.
pub fn create_chronology_version(
    reference: ExactRecordRef,
    snapshot: &SourceSnapshot,
    request: &QueryContext,
    evidence_policy: ExactRecordRef,
    selection_policy: ExactRecordRef,
    member_claim_revisions: Vec<ExactRecordRef>,
) -> Result<Option<ChronologyVersion>, ContextError> {
    let mut seen = std::collections::BTreeSet::new();
    for member in &member_claim_revisions {
        if !seen.insert(member.clone()) {
            return Err(ContextError::DuplicateClaimRevision);
        }
    }
    let ContextResolution::Resolved(effective) = resolve_query_context(snapshot, request, None)?
    else {
        return Ok(None);
    };
    Ok(Some(ChronologyVersion {
        reference,
        selection: effective.selection,
        evidence_policy,
        selection_policy,
        member_claim_revisions,
    }))
}
