use std::collections::BTreeMap;

use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonMode, JsonValue, canonical_bytes_v1, parse_json,
};
use tos_query::SearchDocumentBudget;
use tos_query::search_candidate::{
    CandidateVerifyBudget, SelectedSearchCandidate, verify_search_candidate,
};
use tos_query::search_index::{
    GramSeed, GramSeekBudget, GramSeekCharge, GramStat, PostingPage, PostingSeekBudget,
    SearchGramModel, SearchPostingModel, choose_rarest_gram, visit_complete_postings,
};
use tos_query::search_v2::{
    CurrentPolicyBinding, INDEXED_SEARCH_V2_OPERATION, IndexedSearchV2Request,
    NormalizedIndexedSearchV2Request, QUERY_PRIMITIVE_PROFILE, QueryVocabularyBinding,
    SEARCH_UNICODE_PROFILE, SearchContinuationState, SearchKind, SearchOrderKey, SearchRank,
    SearchSelectionBinding, SearchV2ErrorCode, SelectedQueryVocabulary,
};

struct FixtureVocabulary {
    binding: QueryVocabularyBinding,
    sources: Vec<String>,
}

impl FixtureVocabulary {
    fn selected() -> Self {
        Self {
            binding: QueryVocabularyBinding {
                descriptor_sha256: Digest256::of_bytes(b"fixture vocabulary descriptor"),
                descriptor_version: 1,
            },
            sources: vec!["fixture-source-a".into(), "fixture-source-z".into()],
        }
    }
}

impl SelectedQueryVocabulary for FixtureVocabulary {
    fn binding(&self) -> &QueryVocabularyBinding {
        &self.binding
    }

    fn registered_source_ids(&self) -> &[String] {
        &self.sources
    }
}

#[derive(Default)]
struct FixtureGramModel {
    stats: BTreeMap<String, u64>,
    calls: Vec<String>,
    positions: Vec<u64>,
    posting_calls: usize,
}

impl SearchGramModel for FixtureGramModel {
    fn gram_stat(
        &mut self,
        _kind: SearchKind,
        gram: &str,
        max_vm_steps: u64,
        max_rows: u64,
        max_decoded_bytes: u64,
    ) -> Result<GramStat, tos_query::search_v2::SearchV2Error> {
        assert!(max_vm_steps > 0 && max_rows > 0 && max_decoded_bytes >= 8);
        self.calls.push(gram.to_owned());
        let postings = self.stats.get(gram).copied();
        let rows = u64::from(postings.is_some());
        Ok(GramStat {
            postings,
            charged: GramSeekCharge {
                lookups: 1,
                vm_steps: 1,
                rows,
                decoded_bytes: rows * 8,
            },
        })
    }
}

impl SearchPostingModel for FixtureGramModel {
    fn seek_postings(
        &mut self,
        _kind: SearchKind,
        _gram: &str,
        after: Option<u64>,
        max_rows: usize,
        max_vm_steps: u64,
        max_decoded_bytes: u64,
    ) -> Result<PostingPage, tos_query::search_v2::SearchV2Error> {
        assert!(max_rows > 0 && max_vm_steps > 0 && max_decoded_bytes >= max_rows as u64 * 8);
        self.posting_calls += 1;
        let positions: Vec<_> = self
            .positions
            .iter()
            .copied()
            .filter(|position| after.is_none_or(|last| *position > last))
            .take(max_rows)
            .collect();
        let rows = positions.len() as u64;
        Ok(PostingPage {
            exhausted: positions.len() < max_rows,
            positions,
            charged: GramSeekCharge {
                lookups: 1,
                vm_steps: 1,
                rows,
                decoded_bytes: rows * 8,
            },
        })
    }
}

fn gram_budget() -> GramSeekBudget {
    GramSeekBudget {
        max_lookups: 3,
        max_candidates: 10,
        max_vm_steps: 100,
        max_rows: 3,
        max_decoded_bytes: 24,
    }
}

#[test]
fn rarest_global_gram_is_admitted_before_any_posting_seek() {
    let vocabulary = FixtureVocabulary::selected();
    let selection = selection(&vocabulary);
    let mut request = request("alpha");
    request.sources.push("fixture-source-a".into());
    let normalized = request.normalize(&selection, &vocabulary).unwrap();
    let mut model = FixtureGramModel {
        stats: [("alp", 7), ("lph", 2), ("pha", 2)]
            .into_iter()
            .map(|(key, value)| (key.into(), value))
            .collect(),
        ..FixtureGramModel::default()
    };
    let seed =
        choose_rarest_gram(&mut model, SearchKind::Nodes, &normalized, gram_budget()).unwrap();
    assert_eq!(seed.gram.as_deref(), Some("lph"));
    assert_eq!(seed.postings, 2);
    assert_eq!(model.calls, ["alp", "lph", "pha"]);
    assert_eq!(seed.charged.rows, 3);
    assert_eq!(seed.charged.decoded_bytes, 24);

    let mut low = gram_budget();
    low.max_candidates = 1;
    assert_eq!(
        choose_rarest_gram(&mut model, SearchKind::Nodes, &normalized, low)
            .unwrap_err()
            .code,
        SearchV2ErrorCode::BudgetExceeded
    );
    let mut low = gram_budget();
    low.max_decoded_bytes = 23;
    let before = model.calls.len();
    assert_eq!(
        choose_rarest_gram(&mut model, SearchKind::Nodes, &normalized, low)
            .unwrap_err()
            .code,
        SearchV2ErrorCode::BudgetExceeded
    );
    assert_eq!(model.calls.len(), before);

    model.stats.remove("lph");
    let empty =
        choose_rarest_gram(&mut model, SearchKind::Nodes, &normalized, gram_budget()).unwrap();
    assert_eq!(empty.gram, None);
    assert_eq!(empty.postings, 0);
    assert_eq!(empty.charged.rows, 2);
}

#[test]
fn posting_pages_visit_exact_selected_count_and_admit_completion_probe() {
    let mut model = FixtureGramModel {
        positions: vec![0, 3, 8],
        ..FixtureGramModel::default()
    };
    let seed = GramSeed {
        gram: Some("alp".into()),
        postings: 3,
        charged: GramSeekCharge::default(),
    };
    let budget = PostingSeekBudget {
        max_probes: 2,
        max_rows: 4,
        max_decoded_bytes: 32,
        max_vm_steps: 10,
        page_rows: 2,
    };
    let mut visited = Vec::new();
    let charged = visit_complete_postings(
        &mut model,
        SearchKind::Nodes,
        &seed,
        budget,
        |_, position| {
            visited.push(position);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(visited, [0, 3, 8]);
    assert_eq!(charged.rows, 3);
    assert_eq!(charged.lookups, 2);

    let exact_page_seed = GramSeed {
        postings: 2,
        ..seed.clone()
    };
    model.positions = vec![0, 3];
    let exact_page_budget = PostingSeekBudget {
        max_rows: 3,
        max_decoded_bytes: 24,
        ..budget
    };
    let exact_page = visit_complete_postings(
        &mut model,
        SearchKind::Nodes,
        &exact_page_seed,
        exact_page_budget,
        |_, _| Ok(()),
    )
    .unwrap();
    assert_eq!(exact_page.lookups, 2); // one full page, then empty completion seek
    model.positions.push(8);

    let mut one_under = budget;
    one_under.max_rows = 3;
    let before = model.posting_calls;
    assert_eq!(
        visit_complete_postings(&mut model, SearchKind::Nodes, &seed, one_under, |_, _| Ok(
            ()
        ))
        .unwrap_err()
        .code,
        SearchV2ErrorCode::BudgetExceeded
    );
    assert_eq!(model.posting_calls, before);

    model.positions.pop();
    assert_eq!(
        visit_complete_postings(&mut model, SearchKind::Nodes, &seed, budget, |_, _| Ok(()))
            .unwrap_err()
            .code,
        SearchV2ErrorCode::IndexIncomplete
    );
}

fn selection(vocabulary: &FixtureVocabulary) -> SearchSelectionBinding {
    SearchSelectionBinding {
        model_abi: "tos_knowledge_read_model_v1".into(),
        vocabulary: vocabulary.binding.clone(),
        semantic_primitive_profile: QUERY_PRIMITIVE_PROFILE.into(),
        search_unicode_profile: SEARCH_UNICODE_PROFILE.into(),
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
        catalog_packet_sha256: Digest256::of_bytes(b"fixture catalog packet"),
        catalog_index_root_sha256: Digest256::of_bytes(b"fixture catalog index"),
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
        issuer_ref: "fixture-owner-issuer".into(),
        authorization_receipt_id: "fixture-owner-receipt".into(),
        policy_epoch: "fixture-policy-epoch-3".into(),
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
fn selected_authored_source_registration_must_be_strictly_ordered() {
    let mut vocabulary = FixtureVocabulary::selected();
    let selected = selection(&vocabulary);
    vocabulary.sources.swap(0, 1);
    assert_eq!(
        request("alpha")
            .normalize(&selected, &vocabulary)
            .unwrap_err()
            .code,
        SearchV2ErrorCode::StaleSelection
    );
    vocabulary.sources.sort();
    vocabulary.sources.push("fixture-source-z".into());
    assert_eq!(
        request("alpha")
            .normalize(&selected, &vocabulary)
            .unwrap_err()
            .code,
        SearchV2ErrorCode::StaleSelection
    );
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
    let mut unknown_terms = request("query");
    unknown_terms.kind_ids.push("unseen-owner-kind".into());
    unknown_terms
        .predicate_ids
        .push("unseen-owner-predicate".into());
    let normalized = unknown_terms.normalize(&selected, &vocabulary).unwrap();
    assert_eq!(normalized.kind_ids(), ["unseen-owner-kind"]);
    assert_eq!(normalized.predicate_ids(), ["unseen-owner-predicate"]);

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
    long_member_vocabulary.sources.push(long_id.clone());
    long_member_vocabulary.sources.sort();
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
    unsupported.search_unicode_profile = "host-default-lowercase".into();
    assert_eq!(
        request("query")
            .normalize(&unsupported, &vocabulary)
            .unwrap_err()
            .code,
        SearchV2ErrorCode::UnsupportedProfile
    );

    let mut wrong_authored_profile = selection(&vocabulary);
    wrong_authored_profile.semantic_primitive_profile = "another-query-family".into();
    assert_eq!(
        request("query")
            .normalize(&wrong_authored_profile, &vocabulary)
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
    stale.vocabulary.descriptor_sha256 = Digest256::of_bytes(b"different selected descriptor");
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
    for changed in [
        SearchSelectionBinding {
            catalog_packet_sha256: Digest256::of_bytes(b"new catalog packet"),
            ..selected.clone()
        },
        SearchSelectionBinding {
            catalog_index_root_sha256: Digest256::of_bytes(b"new catalog index"),
            ..selected.clone()
        },
    ] {
        assert_eq!(
            state
                .validate_resume(&normalized, &changed, &policy, &vocabulary)
                .unwrap_err()
                .code,
            SearchV2ErrorCode::StaleSelection
        );
    }
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
    let mut changed_epoch = policy.clone();
    changed_epoch.policy_epoch = "fixture-policy-epoch-4".into();
    assert_eq!(
        state
            .validate_resume(&normalized, &selected, &changed_epoch, &vocabulary)
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
        issuer_ref: "fixture-owner-issuer".into(),
        authorization_receipt_id: "fixture-owner-receipt".into(),
        policy_epoch: "fixture-policy-epoch-3".into(),
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

#[test]
fn selected_candidates_match_independent_cpython_rank_and_false_positive_oracle() {
    let oracle = parse_json(
        include_bytes!("fixtures/search_candidate_python_oracle.json"),
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap()
    .into_root();
    let mut vocabulary = FixtureVocabulary::selected();
    vocabulary
        .sources
        .extend(["philosophy".into(), "canon".into()]);
    vocabulary.sources.sort();
    let selected = selection(&vocabulary);
    let normalized = request(field(&oracle, "query").as_str().unwrap())
        .normalize(&selected, &vocabulary)
        .unwrap();
    let limits = CandidateVerifyBudget {
        document: SearchDocumentBudget {
            max_carrier_bytes: 4096,
            max_document_bytes: 8192,
            max_document_code_points: 8192,
            json: JsonLimits::default(),
        },
        max_rank_field_bytes: 4096,
        max_rank_values: 64,
    };
    let mut hits = Vec::new();
    for row in field(&oracle, "rows").as_array().unwrap() {
        let item = field(row, "item");
        let payload = canonical_bytes_v1(
            item,
            CanonicalProfile::SourceRecordDigestV1,
            JsonLimits::default(),
        )
        .unwrap();
        let candidate = SelectedSearchCandidate {
            kind: SearchKind::Nodes,
            position: field(row, "position").as_u64().unwrap(),
            id: field(item, "id").as_str().unwrap().into(),
            source_graph: field(item, "source_graph").as_str().unwrap().into(),
            kind_id: field(item, "kind_id").as_str().unwrap().into(),
            predicate_id: String::new(),
            id_lower: field(row, "id_lower").as_str().unwrap().into(),
            native_id_lower: field(row, "native_id_lower").as_str().unwrap().into(),
            identity_values: field(row, "identity_values").as_str().unwrap().into(),
            visible_values: field(row, "visible_values").as_str().unwrap().into(),
            document_chars: field(row, "document_chars").as_u64().unwrap(),
            document_digest: Digest256::from_hex(field(row, "document_digest").as_str().unwrap())
                .unwrap(),
            payload_sha256: Digest256::of_bytes(&payload),
            payload,
        };
        let result =
            verify_search_candidate(candidate.clone(), &normalized, &vocabulary, limits).unwrap();
        match field(row, "rank").as_u64() {
            Some(rank) => {
                let hit = result.expect("Python-ranked candidate must match");
                assert_eq!(hit.order.rank() as u64, rank);
                hits.push(hit);
            }
            None => assert!(result.is_none(), "gram false positive must be skipped"),
        }
        let mut corrupt = candidate;
        corrupt.identity_values.push_str("corrupt");
        assert_eq!(
            verify_search_candidate(corrupt, &normalized, &vocabulary, limits)
                .unwrap_err()
                .code,
            SearchV2ErrorCode::CorruptSelectedCarrier
        );
    }
    hits.sort_by(|left, right| left.order.cmp(&right.order));
    assert_eq!(
        hits.iter().map(|hit| hit.id.as_str()).collect::<Vec<_>>(),
        [
            "alpha",
            "node.native",
            "node.title",
            "alphabet",
            "node.visible",
            "node.metadata"
        ]
    );
}
