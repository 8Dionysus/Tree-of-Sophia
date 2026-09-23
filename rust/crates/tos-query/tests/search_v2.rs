use std::collections::BTreeSet;

use tos_foundation::{Digest256, JsonLimits, JsonMode, JsonValue, parse_json};
use tos_query::search_v2::{
    CurrentPolicyBinding, INDEXED_SEARCH_V2_OPERATION, IndexedSearchV2Request,
    NormalizedIndexedSearchV2Request, QueryVocabularyBinding, SEARCH_UNICODE_PROFILE,
    SearchContinuationState, SearchKind, SearchOrderKey, SearchRank, SearchSelectionBinding,
    SearchV2ErrorCode, SelectedQueryVocabulary,
};

struct FixtureVocabulary {
    binding: QueryVocabularyBinding,
    sources: BTreeSet<String>,
    kinds: BTreeSet<String>,
    predicates: BTreeSet<String>,
}

impl FixtureVocabulary {
    fn selected() -> Self {
        Self {
            binding: QueryVocabularyBinding {
                descriptor_sha256: Digest256::of_bytes(b"fixture vocabulary descriptor"),
                descriptor_version: "fixture-v1".into(),
                membership_root: Digest256::of_bytes(b"fixture vocabulary membership"),
            },
            sources: ["fixture-source-a", "fixture-source-z"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            kinds: ["fixture-kind-a", "fixture-kind-z"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            predicates: ["fixture-predicate-a", "fixture-predicate-z"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        }
    }
}

impl SelectedQueryVocabulary for FixtureVocabulary {
    fn binding(&self) -> &QueryVocabularyBinding {
        &self.binding
    }

    fn contains_source_id(&self, id: &str) -> bool {
        self.sources.contains(id)
    }

    fn contains_kind_id(&self, id: &str) -> bool {
        self.kinds.contains(id)
    }

    fn contains_predicate_id(&self, id: &str) -> bool {
        self.predicates.contains(id)
    }
}

fn selection(vocabulary: &FixtureVocabulary) -> SearchSelectionBinding {
    SearchSelectionBinding {
        model_abi: "tos_knowledge_read_model_v1".into(),
        vocabulary: vocabulary.binding.clone(),
        semantic_primitive_profile: SEARCH_UNICODE_PROFILE.into(),
        source_cut: "fixture-cut-a".into(),
        through_commit_seq: 19,
        source_membership_root: Digest256::of_bytes(b"fixture source membership"),
        history_root_sha256: Some(Digest256::of_bytes(b"fixture history")),
        entity_registry_id: "fixture-entity-registry".into(),
        entity_registry_version: "fixture-entity-v1".into(),
        entity_registry_sha256: Digest256::of_bytes(b"fixture entity registry"),
        relation_registry_id: "fixture-relation-registry".into(),
        relation_registry_version: "fixture-relation-v1".into(),
        relation_registry_sha256: Digest256::of_bytes(b"fixture relation registry"),
        graph_root_sha256: Digest256::of_bytes(b"fixture graph root"),
        catalog_root_sha256: Digest256::of_bytes(b"fixture catalog root"),
        source_scope_root_sha256: Digest256::of_bytes(b"fixture source scope root"),
        search_index_root_sha256: Digest256::of_bytes(b"fixture search index root"),
        index_root_sha256: Digest256::of_bytes(b"fixture sqlite file root"),
        index_generation: "fixture-generation-a".into(),
        route_map_version: "fixture-route-map-v1".into(),
        reader_abi: "tos-knowledge-search-reader-v1".into(),
        complete: true,
    }
}

fn request(query: impl Into<String>) -> IndexedSearchV2Request {
    IndexedSearchV2Request {
        query: query.into(),
        sources: Vec::new(),
        kind_ids: Vec::new(),
        predicate_ids: Vec::new(),
        limit: 40,
    }
}

fn current_policy() -> CurrentPolicyBinding {
    CurrentPolicyBinding {
        scope: "fixture-owner-policy-scope".into(),
        withdrawal_generation: "fixture-withdrawal-generation-3".into(),
    }
}

fn field<'a>(value: &'a JsonValue, name: &str) -> &'a JsonValue {
    value.object_get(name).expect("fixture field")
}

fn rank(value: &str) -> SearchRank {
    match value {
        "exact" => SearchRank::ExactIdentity,
        "prefix" => SearchRank::IdentityPrefix,
        "display" => SearchRank::VisibleDisplaySubstring,
        "carrier" => SearchRank::OtherSerializedCarrierSubstring,
        _ => panic!("unknown oracle rank"),
    }
}

fn order_keys(value: &JsonValue) -> Vec<SearchOrderKey> {
    value
        .as_array()
        .expect("oracle order-key array")
        .iter()
        .map(|item| {
            SearchOrderKey::new(
                rank(field(item, "rank").as_str().unwrap()),
                field(item, "lower_id").as_str().unwrap().to_owned(),
                field(item, "source_position").as_u64().unwrap(),
            )
            .unwrap()
        })
        .collect()
}

#[test]
fn operation_is_explicit_indexed_v2() {
    assert_eq!(
        INDEXED_SEARCH_V2_OPERATION,
        "tos_knowledge_search_indexed_v2"
    );
}

#[test]
fn query_normalization_matches_frozen_cpython_oracle() {
    let fixture = parse_json(
        include_bytes!("fixtures/search_v2_python_oracle.json"),
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap()
    .into_root();
    assert_eq!(
        field(&fixture, "profile").as_str(),
        Some(SEARCH_UNICODE_PROFILE)
    );
    assert!(
        field(&fixture, "oracle")
            .as_str()
            .unwrap()
            .starts_with("CPython 3.14.")
    );
    let vocabulary = FixtureVocabulary::selected();
    let selected = selection(&vocabulary);

    for case in field(&fixture, "cases").as_array().unwrap() {
        let name = field(case, "name").as_str().unwrap();
        let raw = field(case, "raw").as_str().unwrap();
        assert_eq!(
            raw.chars().count() as u64,
            field(case, "raw_code_points").as_u64().unwrap(),
            "{name}: frozen Python raw length"
        );
        let result = request(raw)
            .normalize(&selected, &vocabulary)
            .map(|normalized| normalized.query().to_owned());
        match field(case, "error").as_str() {
            Some("QueryTooLong") => {
                assert_eq!(
                    result.unwrap_err().code,
                    SearchV2ErrorCode::QueryTooLong,
                    "{name}"
                );
            }
            Some("QueryTooShort") => {
                assert_eq!(
                    result.unwrap_err().code,
                    SearchV2ErrorCode::QueryTooShort,
                    "{name}"
                );
            }
            Some(other) => panic!("{name}: unknown frozen error {other}"),
            None => {
                let expected = field(case, "normalized").as_str().unwrap();
                assert_eq!(result.as_deref(), Ok(expected), "{name}");
                assert_eq!(
                    expected.chars().count() as u64,
                    field(case, "normalized_code_points").as_u64().unwrap(),
                    "{name}: frozen Python normalized length"
                );
            }
        }
    }
}

#[test]
fn filters_are_exact_membership_checked_deduplicated_and_sorted() {
    let vocabulary = FixtureVocabulary::selected();
    let selected = selection(&vocabulary);
    let mut value = request("Query");
    value.sources = vec![
        "fixture-source-z".into(),
        "fixture-source-a".into(),
        "fixture-source-a".into(),
    ];
    value.kind_ids = vec!["fixture-kind-z".into(), "fixture-kind-z".into()];
    value.predicate_ids = vec!["fixture-predicate-a".into()];
    let normalized = value.normalize(&selected, &vocabulary).unwrap();
    assert_eq!(normalized.query(), "query");
    assert_eq!(
        normalized
            .sources()
            .unwrap()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["fixture-source-a", "fixture-source-z"]
    );
    assert_eq!(
        normalized
            .kind_ids()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["fixture-kind-z"]
    );
    assert_eq!(
        normalized
            .predicate_ids()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["fixture-predicate-a"]
    );

    let unrestricted = request("query").normalize(&selected, &vocabulary).unwrap();
    assert_eq!(unrestricted.sources(), None);
    assert!(unrestricted.kind_ids().is_empty());
    assert!(unrestricted.predicate_ids().is_empty());
}

#[test]
fn request_refuses_unknown_membership_and_invalid_page_limits() {
    let vocabulary = FixtureVocabulary::selected();
    let selected = selection(&vocabulary);
    let mut unknown_source = request("query");
    unknown_source
        .sources
        .push("not-in-selected-descriptor".into());
    assert_eq!(
        unknown_source
            .normalize(&selected, &vocabulary)
            .unwrap_err()
            .code,
        SearchV2ErrorCode::InvalidRequest
    );
    let mut unknown_kind = request("query");
    unknown_kind
        .kind_ids
        .push("not-in-selected-descriptor".into());
    assert_eq!(
        unknown_kind
            .normalize(&selected, &vocabulary)
            .unwrap_err()
            .code,
        SearchV2ErrorCode::InvalidRequest
    );
    let mut unknown_predicate = request("query");
    unknown_predicate
        .predicate_ids
        .push("not-in-selected-descriptor".into());
    assert_eq!(
        unknown_predicate
            .normalize(&selected, &vocabulary)
            .unwrap_err()
            .code,
        SearchV2ErrorCode::InvalidRequest
    );

    for limit in [0, 101] {
        let mut invalid = request("query");
        invalid.limit = limit;
        assert_eq!(
            invalid.normalize(&selected, &vocabulary).unwrap_err().code,
            SearchV2ErrorCode::InvalidRequest
        );
    }

    // Count raw filter entries before deduplication or source membership
    // work; otherwise repeated input can cause unbounded allocation/lookup.
    let mut too_many = request("query");
    too_many.sources = vec!["fixture-source-a".into(); 101];
    assert_eq!(
        too_many.normalize(&selected, &vocabulary).unwrap_err().code,
        SearchV2ErrorCode::InvalidRequest
    );

    let mut long_member_vocabulary = FixtureVocabulary::selected();
    let long_id = "x".repeat(257);
    long_member_vocabulary.sources.insert(long_id.clone());
    let long_selected = selection(&long_member_vocabulary);
    let mut too_long = request("query");
    too_long.sources.push(long_id);
    assert_eq!(
        too_long
            .normalize(&long_selected, &long_member_vocabulary)
            .unwrap_err()
            .code,
        SearchV2ErrorCode::InvalidRequest
    );
}

#[test]
fn selection_must_be_complete_profiled_and_bound_to_selected_vocabulary() {
    let vocabulary = FixtureVocabulary::selected();
    let mut incomplete = selection(&vocabulary);
    incomplete.complete = false;
    assert_eq!(
        request("query")
            .normalize(&incomplete, &vocabulary)
            .unwrap_err()
            .code,
        SearchV2ErrorCode::SelectionIncomplete
    );

    let mut unsupported = selection(&vocabulary);
    unsupported.semantic_primitive_profile = "host-default-lowercase".into();
    assert_eq!(
        request("query")
            .normalize(&unsupported, &vocabulary)
            .unwrap_err()
            .code,
        SearchV2ErrorCode::UnsupportedProfile
    );

    let mut unsupported_model = selection(&vocabulary);
    unsupported_model.model_abi = "unknown_search_model".into();
    assert_eq!(
        request("query")
            .normalize(&unsupported_model, &vocabulary)
            .unwrap_err()
            .code,
        SearchV2ErrorCode::UnsupportedModel
    );

    let mut stale = selection(&vocabulary);
    stale.vocabulary.membership_root = Digest256::of_bytes(b"different selected root");
    assert_eq!(
        request("query")
            .normalize(&stale, &vocabulary)
            .unwrap_err()
            .code,
        SearchV2ErrorCode::StaleSelection
    );
}

#[test]
fn continuation_binds_query_selection_and_independent_kind_positions() {
    let vocabulary = FixtureVocabulary::selected();
    let selected = selection(&vocabulary);
    let normalized = request("query").normalize(&selected, &vocabulary).unwrap();
    let policy = current_policy();
    let mut state = SearchContinuationState::new(
        selected.clone(),
        normalized.clone(),
        policy.clone(),
        &vocabulary,
    )
    .unwrap();
    let node_key =
        SearchOrderKey::new(SearchRank::VisibleDisplaySubstring, "alpha".into(), 7).unwrap();
    state
        .advance(SearchKind::Nodes, Some(node_key.clone()), false)
        .unwrap();
    assert_eq!(state.after(SearchKind::Nodes), Some(&node_key));
    assert_eq!(state.after(SearchKind::Relations), None);
    assert!(!state.is_exhausted(SearchKind::Nodes));
    assert_eq!(state.request(), &normalized);
    state
        .validate_resume(&normalized, &selected, &policy, &vocabulary)
        .unwrap();

    let other_request = request("other").normalize(&selected, &vocabulary).unwrap();
    assert_eq!(
        state
            .validate_resume(&other_request, &selected, &policy, &vocabulary)
            .unwrap_err()
            .code,
        SearchV2ErrorCode::StaleContinuation
    );
    let mut other_selection = selected.clone();
    other_selection.search_index_root_sha256 = Digest256::of_bytes(b"new search root");
    assert_eq!(
        state
            .validate_resume(&normalized, &other_selection, &policy, &vocabulary)
            .unwrap_err()
            .code,
        SearchV2ErrorCode::StaleSelection
    );
    let mut other_membership = selected.clone();
    other_membership.source_membership_root = Digest256::of_bytes(b"new source membership");
    assert_eq!(
        state
            .validate_resume(&normalized, &other_membership, &policy, &vocabulary)
            .unwrap_err()
            .code,
        SearchV2ErrorCode::StaleSelection
    );
    let mut other_history = selected.clone();
    other_history.history_root_sha256 = Some(Digest256::of_bytes(b"new history root"));
    assert_eq!(
        state
            .validate_resume(&normalized, &other_history, &policy, &vocabulary)
            .unwrap_err()
            .code,
        SearchV2ErrorCode::StaleSelection
    );

    let mut withdrawn = policy.clone();
    withdrawn.withdrawal_generation = "fixture-withdrawal-generation-4".into();
    assert_eq!(
        state
            .validate_resume(&normalized, &selected, &withdrawn, &vocabulary)
            .unwrap_err()
            .code,
        SearchV2ErrorCode::StalePolicy
    );
}

#[test]
fn continuation_refuses_missing_owner_policy_binding() {
    let vocabulary = FixtureVocabulary::selected();
    let selected = selection(&vocabulary);
    let normalized = request("query").normalize(&selected, &vocabulary).unwrap();
    let missing = CurrentPolicyBinding {
        scope: "fixture-owner-policy-scope".into(),
        withdrawal_generation: String::new(),
    };
    assert_eq!(
        SearchContinuationState::new(selected, normalized, missing, &vocabulary)
            .unwrap_err()
            .code,
        SearchV2ErrorCode::PolicyBindingUnavailable
    );
}

#[test]
fn continuation_requires_strict_progress_and_tracks_exhaustion() {
    let vocabulary = FixtureVocabulary::selected();
    let selected = selection(&vocabulary);
    let normalized = request("query").normalize(&selected, &vocabulary).unwrap();
    let mut state =
        SearchContinuationState::new(selected, normalized, current_policy(), &vocabulary).unwrap();
    let first = SearchOrderKey::new(SearchRank::IdentityPrefix, "alpha".into(), 4).unwrap();
    let second = SearchOrderKey::new(SearchRank::IdentityPrefix, "alpha".into(), 5).unwrap();
    let later_rank =
        SearchOrderKey::new(SearchRank::VisibleDisplaySubstring, "aardvark".into(), 0).unwrap();

    assert_eq!(
        state
            .advance(SearchKind::Nodes, None, false)
            .unwrap_err()
            .code,
        SearchV2ErrorCode::MissingProgress
    );
    state
        .advance(SearchKind::Nodes, Some(first.clone()), false)
        .unwrap();
    assert_eq!(
        state
            .advance(SearchKind::Nodes, Some(first.clone()), false)
            .unwrap_err()
            .code,
        SearchV2ErrorCode::NonMonotoneProgress
    );
    state
        .advance(SearchKind::Nodes, Some(second.clone()), false)
        .unwrap();
    assert!(second > first);
    assert!(later_rank > second);

    state.advance(SearchKind::Relations, None, true).unwrap();
    assert!(state.is_exhausted(SearchKind::Relations));
    assert_eq!(state.after(SearchKind::Relations), None);
    assert_eq!(
        state
            .advance(SearchKind::Relations, None, true)
            .unwrap_err()
            .code,
        SearchV2ErrorCode::StaleContinuation
    );
}

#[test]
fn order_key_uses_rank_then_lower_id_then_source_position() {
    let fixture = parse_json(
        include_bytes!("fixtures/search_v2_python_oracle.json"),
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap()
    .into_root();
    let case = &field(&fixture, "order_cases").as_array().unwrap()[0];
    let mut keys = order_keys(field(case, "input"));
    keys.sort();
    assert_eq!(keys, order_keys(field(case, "expected")));
}

#[test]
fn normalized_request_keeps_backend_page_size() {
    let vocabulary = FixtureVocabulary::selected();
    let selected = selection(&vocabulary);
    for limit in [1, 100] {
        let mut value = request("query");
        value.limit = limit;
        let normalized = value.normalize(&selected, &vocabulary).unwrap();
        assert_eq!(normalized.limit(), limit);
        let _: NormalizedIndexedSearchV2Request = normalized;
    }
}
