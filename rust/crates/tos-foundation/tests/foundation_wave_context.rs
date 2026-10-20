//! Independent I/C context acceptance: no wall clock, locale or ambient snapshot.
use tos_foundation::query_context::*;
use tos_foundation::{
    Digest256, ExactRecordRef, JsonString, JsonValue, RecordVersion, SourceRevision, StableId,
};

fn text(s: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(s))
}
fn exact(id: &str, version: u64) -> ExactRecordRef {
    ExactRecordRef {
        id: StableId::parse(id).unwrap(),
        version: RecordVersion::new(version).unwrap(),
        digest: Digest256::of_bytes(format!("{id}@{version}").as_bytes()),
    }
}
fn cutoff(s: &str) -> Cutoff {
    Cutoff {
        value: text(s),
        trs: Some(exact("fixture-clock", 1)),
    }
}
fn snapshot() -> SourceSnapshot {
    SourceSnapshot {
        reference: SourceSnapshotRef {
            id: StableId::parse("S1").unwrap(),
            revision: SourceRevision(Digest256::of_bytes(b"source-rev-42")),
        },
        current_cutoff: cutoff("2026-10-19T05:00:00Z"),
    }
}
fn attribution() -> AttributedKnowledge {
    AttributedKnowledge {
        knower: StableId::parse("school-A").unwrap(),
        cutoff: cutoff("1800"),
    }
}
fn manifest() -> ChronologyVersion {
    ChronologyVersion {
        reference: exact("CV1", 1),
        selection: KnowledgeSelection {
            source_snapshot: snapshot().reference,
            system_as_of: snapshot().current_cutoff,
            attributed_knowledge: Some(attribution()),
            source_publication_as_of: Some(cutoff("1900")),
        },
        evidence_policy: exact("evidence-policy", 1),
        selection_policy: exact("selection-policy", 1),
        member_claim_revisions: vec![exact("claim-A", 1), exact("claim-B", 3)],
    }
}
fn resolved(r: Result<ContextResolution, ContextError>) -> EffectiveQueryContext {
    match r.unwrap() {
        ContextResolution::Resolved(value) => value,
        other => panic!("{other:?}"),
    }
}

#[test]
fn omitted_and_exact_selectors_replay_frozen_selection() {
    let frozen = manifest();
    let before = frozen.clone();
    let omitted = resolved(resolve_query_context(
        &snapshot(),
        &QueryContext::default(),
        Some(&frozen),
    ));
    assert_eq!(omitted.selection, frozen.selection);
    assert_eq!(omitted.chronology_version, Some(frozen.reference.clone()));
    let matching = QueryContext {
        system_state: Some(SystemState::AsOf(frozen.selection.system_as_of.clone())),
        attributed_knowledge: Some(frozen.selection.attributed_knowledge.clone()),
        source_publication_as_of: Some(frozen.selection.source_publication_as_of.clone()),
        ..Default::default()
    };
    assert_eq!(
        resolved(resolve_query_context(&snapshot(), &matching, Some(&frozen))).selection,
        omitted.selection
    );
    assert_eq!(frozen, before);
}

#[test]
fn explicit_absence_is_not_omission_and_each_axis_is_independent() {
    let frozen = manifest();
    for (request, expected) in [
        (
            QueryContext {
                attributed_knowledge: Some(None),
                ..Default::default()
            },
            MismatchDimension::AttributedKnowledgeAsOf,
        ),
        (
            QueryContext {
                source_publication_as_of: Some(None),
                ..Default::default()
            },
            MismatchDimension::SourcePublicationAsOf,
        ),
        (
            QueryContext {
                system_state: Some(SystemState::AsOf(cutoff("2020"))),
                ..Default::default()
            },
            MismatchDimension::SystemState,
        ),
        (
            QueryContext {
                attributed_knowledge: Some(Some(AttributedKnowledge {
                    knower: StableId::parse("school-B").unwrap(),
                    cutoff: cutoff("1800"),
                })),
                ..Default::default()
            },
            MismatchDimension::AttributedKnowledgeAsOf,
        ),
        (
            QueryContext {
                source_publication_as_of: Some(Some(cutoff("1899"))),
                ..Default::default()
            },
            MismatchDimension::SourcePublicationAsOf,
        ),
    ] {
        assert_eq!(
            resolve_query_context(&snapshot(), &request, Some(&frozen)),
            Ok(ContextResolution::ContextMismatch(vec![expected]))
        );
    }
}

#[test]
fn snapshot_and_fence_mismatches_are_exhaustive_and_ordered() {
    let frozen = manifest();
    let mut newer = snapshot();
    newer.reference.id = StableId::parse("S2").unwrap();
    newer.reference.revision = SourceRevision(Digest256::of_bytes(b"source-rev-43"));
    newer.current_cutoff = cutoff("2026-10-20T05:00:00Z");
    assert_eq!(
        resolve_query_context(&newer, &QueryContext::default(), Some(&frozen)),
        Ok(ContextResolution::ContextMismatch(vec![
            MismatchDimension::SourceSnapshotRef,
            MismatchDimension::SourceRevisionFence
        ]))
    );
    let request = QueryContext {
        system_state: Some(SystemState::Current),
        attributed_knowledge: Some(None),
        source_publication_as_of: Some(None),
        ..Default::default()
    };
    assert_eq!(
        resolve_query_context(&newer, &request, Some(&frozen)),
        Ok(ContextResolution::ContextMismatch(vec![
            MismatchDimension::SourceSnapshotRef,
            MismatchDimension::SourceRevisionFence,
            MismatchDimension::SystemState,
            MismatchDimension::AttributedKnowledgeAsOf,
            MismatchDimension::SourcePublicationAsOf
        ]))
    );
}

#[test]
fn fence_only_and_trs_version_mismatches_do_not_succeed() {
    let frozen = manifest();
    let mut changed = snapshot();
    changed.reference.id = StableId::parse("different-snapshot-same-fence").unwrap();
    assert_eq!(
        resolve_query_context(&changed, &QueryContext::default(), Some(&frozen)),
        Ok(ContextResolution::ContextMismatch(vec![
            MismatchDimension::SourceSnapshotRef
        ]))
    );
    changed = snapshot();
    changed.reference.revision = SourceRevision(Digest256::of_bytes(b"new-fence"));
    assert_eq!(
        resolve_query_context(&changed, &QueryContext::default(), Some(&frozen)),
        Ok(ContextResolution::ContextMismatch(vec![
            MismatchDimension::SourceRevisionFence
        ]))
    );
    let mut other_rules = frozen.selection.system_as_of.clone();
    other_rules.trs = Some(exact("fixture-clock", 2));
    let request = QueryContext {
        system_state: Some(SystemState::AsOf(other_rules)),
        ..Default::default()
    };
    assert_eq!(
        resolve_query_context(&snapshot(), &request, Some(&frozen)),
        Ok(ContextResolution::ContextMismatch(vec![
            MismatchDimension::SystemState
        ]))
    );
}

#[test]
fn current_freezes_supplied_cutoff_members_and_policies_then_replays() {
    let snap = snapshot();
    let members = vec![exact("claim-A", 1), exact("claim-A", 2)];
    let request = QueryContext {
        system_state: Some(SystemState::Current),
        attributed_knowledge: Some(Some(attribution())),
        world_valid_time: Some(text("1945")),
        display_perspective: Some(text("display-only")),
        ..Default::default()
    };
    let frozen = create_chronology_version(
        exact("created", 7),
        &snap,
        &request,
        exact("evidence-policy", 4),
        exact("selection-policy", 9),
        members.clone(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(frozen.selection.system_as_of, snap.current_cutoff);
    assert_eq!(frozen.selection.source_snapshot, snap.reference);
    assert_eq!(frozen.selection.attributed_knowledge, Some(attribution()));
    assert_eq!(frozen.member_claim_revisions, members);
    assert_eq!(frozen.evidence_policy, exact("evidence-policy", 4));
    assert_eq!(frozen.selection_policy, exact("selection-policy", 9));
    let at_t2 = QueryContext {
        world_valid_time: Some(text("1946")),
        ..Default::default()
    };
    let replay = resolved(resolve_query_context(&snap, &at_t2, Some(&frozen)));
    assert_eq!(replay.selection, frozen.selection);
    assert_eq!(replay.world_valid_time, Some(text("1946")));
    assert_eq!(replay.display_perspective, None);
    assert_eq!(frozen.member_claim_revisions, members);
}

#[test]
fn world_filter_and_display_do_not_change_knowledge_selection() {
    let frozen = manifest();
    for world in ["1344", "1945", "future-or-unresolved"] {
        let request = QueryContext {
            world_valid_time: Some(text(world)),
            display_perspective: Some(text("arbitrary label")),
            ..Default::default()
        };
        let result = resolved(resolve_query_context(&snapshot(), &request, Some(&frozen)));
        assert_eq!(result.selection, frozen.selection);
        assert_eq!(result.world_valid_time, Some(text(world)));
        assert_eq!(result.display_perspective, request.display_perspective);
    }
}

#[test]
fn no_frozen_version_requires_explicit_system_selection() {
    assert_eq!(
        resolve_query_context(&snapshot(), &QueryContext::default(), None),
        Ok(ContextResolution::MissingSelection)
    );
    let request = QueryContext {
        system_state: Some(SystemState::Current),
        attributed_knowledge: Some(Some(attribution())),
        ..Default::default()
    };
    let current = resolved(resolve_query_context(&snapshot(), &request, None));
    assert_eq!(current.selection.system_as_of, snapshot().current_cutoff);
    assert_eq!(current.selection.attributed_knowledge, Some(attribution()));
    assert_eq!(current.selection.source_publication_as_of, None);
    assert_eq!(current.chronology_version, None);
}

#[test]
fn missing_cutoff_and_duplicate_exact_member_are_errors() {
    let invalid = QueryContext {
        system_state: Some(SystemState::AsOf(Cutoff {
            value: JsonValue::Null,
            trs: None,
        })),
        ..Default::default()
    };
    assert_eq!(
        resolve_query_context(&snapshot(), &invalid, None),
        Err(ContextError::InvalidCutoff)
    );
    let current = QueryContext {
        system_state: Some(SystemState::Current),
        ..Default::default()
    };
    let member = exact("claim-A", 1);
    assert_eq!(
        create_chronology_version(
            exact("created", 1),
            &snapshot(),
            &current,
            exact("evidence", 1),
            exact("selection", 1),
            vec![member.clone(), member]
        ),
        Err(ContextError::DuplicateClaimRevision)
    );
}

#[test]
fn identities_and_cutoff_values_are_data_not_special_case_constants() {
    for (id, date) in [
        ("arbitrary/snapshot-902", "1037"),
        ("snapshot-117", "2099-12-30T04:11:00+03:00"),
    ] {
        let mut snap = snapshot();
        snap.reference.id = StableId::parse(id).unwrap();
        snap.current_cutoff = cutoff(date);
        let current = QueryContext {
            system_state: Some(SystemState::Current),
            ..Default::default()
        };
        let frozen = create_chronology_version(
            exact("renamed-chronology", 3),
            &snap,
            &current,
            exact("different-evidence", 7),
            exact("different-selection", 8),
            vec![exact("renamed-claim", 6)],
        )
        .unwrap()
        .unwrap();
        assert_eq!(frozen.selection.system_as_of.value, text(date));
        let replay = resolved(resolve_query_context(
            &snap,
            &QueryContext::default(),
            Some(&frozen),
        ));
        assert_eq!(replay.selection, frozen.selection);
    }
}
