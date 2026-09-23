#![cfg(not(target_arch = "wasm32"))]

//! One producer-built CMP selection through QRY ranked packet continuation.
//! All custody and disclosure callbacks here are synthetic test holders.

use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use tos_compiler::knowledge_full_fixture::build_fixture;
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonMode, JsonValue, canonical_bytes_v1, parse_json,
};
use tos_query::search_candidate::{
    CandidateReadBudget, CandidateVerifyBudget, SelectedSearchCandidate,
};
use tos_query::search_index::{GramSeekBudget, PostingSeekBudget};
use tos_query::search_v2::{
    CurrentPolicyBinding, IndexedSearchV2Request, SearchContinuationState, SearchV2Error,
    SearchV2ErrorCode,
};
use tos_query::{
    IndexedDisclosureLease, IndexedDisclosureScope, IndexedKnowledgeAuthority, IndexedPageBudget,
    IndexedWireCursorCodec, ObservedSearchCandidate, SearchDocumentBudget, SearchKindBudget,
    bind_verified_knowledge, execute_indexed_search_page,
};

const PYTHON_ALPHA_ORDER: [&str; 2] = ["eighth:alpha", "eighth:visible"];
const FALSE_POSITIVE: &str = "eighth:alp-false-positive";

fn error(code: SearchV2ErrorCode, message: &'static str) -> SearchV2Error {
    SearchV2Error { code, message }
}

struct SyntheticLease(Arc<AtomicUsize>);
impl IndexedDisclosureLease for SyntheticLease {
    fn recheck(&mut self) -> Result<(), SearchV2Error> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

struct SyntheticAuthority {
    policy: CurrentPolicyBinding,
    scope: IndexedDisclosureScope,
    consulted: Vec<String>,
    lease_checks: Arc<AtomicUsize>,
}
impl IndexedKnowledgeAuthority for SyntheticAuthority {
    fn policy_binding(&self) -> CurrentPolicyBinding {
        self.policy.clone()
    }
    fn disclosure_scope(&self) -> IndexedDisclosureScope {
        self.scope.clone()
    }
    fn check_selected(&mut self) -> Result<(), SearchV2Error> {
        Ok(())
    }
    fn authorize_current(
        &mut self,
        candidate: &SelectedSearchCandidate,
    ) -> Result<(), SearchV2Error> {
        self.consulted.push(candidate.id.clone());
        Ok(())
    }
    fn acquire_disclosure(
        &mut self,
        _: &IndexedDisclosureScope,
        consulted: &[ObservedSearchCandidate],
    ) -> Result<Box<dyn IndexedDisclosureLease>, SearchV2Error> {
        assert!(consulted.iter().any(|row| row.id == FALSE_POSITIVE));
        Ok(Box::new(SyntheticLease(Arc::clone(&self.lease_checks))))
    }
}

#[derive(Default)]
struct SyntheticCursor {
    next: u64,
    states: BTreeMap<String, SearchContinuationState>,
}
impl IndexedWireCursorCodec for SyntheticCursor {
    fn decode(&mut self, token: &str) -> Result<SearchContinuationState, SearchV2Error> {
        self.states.get(token).cloned().ok_or_else(|| {
            error(
                SearchV2ErrorCode::InvalidRequest,
                "unknown synthetic cursor",
            )
        })
    }
    fn encode(&mut self, state: &SearchContinuationState) -> Result<String, SearchV2Error> {
        self.next += 1;
        let token = format!("synthetic-cursor-{}", self.next);
        self.states.insert(token.clone(), state.clone());
        Ok(token)
    }
}

fn kind_budget() -> SearchKindBudget {
    SearchKindBudget {
        grams: GramSeekBudget {
            max_lookups: 256,
            max_candidates: 100,
            max_vm_steps: 1_000_000,
            max_rows: 256,
            max_decoded_bytes: 2048,
        },
        postings: PostingSeekBudget {
            max_probes: 100,
            max_rows: 101,
            max_decoded_bytes: 808,
            max_vm_steps: 1_000_000,
            page_rows: 2,
        },
        candidate: CandidateReadBudget {
            max_vm_steps: 1_000_000,
            max_decoded_bytes: 2_000_000,
            max_payload_bytes: 1_000_000,
            max_field_bytes: 8192,
            max_document_chars: 1_000_000,
        },
        verify: CandidateVerifyBudget {
            document: SearchDocumentBudget {
                max_carrier_bytes: 1_000_000,
                max_document_bytes: 4_000_000,
                max_document_code_points: 1_000_000,
                json: JsonLimits::default(),
            },
            max_rank_field_bytes: 8192,
            max_rank_values: 64,
        },
        max_candidate_vm_steps: 4_000_000,
        max_candidate_decoded_bytes: 8_000_000,
        max_verified_chars: 4_000_000,
        max_verified_bytes: 8_000_000,
        max_observed_candidates: 100,
        max_observed_bytes: 100_000,
        max_selected_result_bytes: 2_000_000,
    }
}

fn budget() -> IndexedPageBudget {
    IndexedPageBudget {
        nodes: kind_budget(),
        relations: kind_budget(),
        max_open_vm_steps: 100_000_000,
        max_response_bytes: 1_000_000,
        max_cursor_bytes: 1024,
        json: JsonLimits::default(),
    }
}

fn field<'a>(value: &'a JsonValue, key: &str) -> &'a JsonValue {
    value.object_get(key).expect("fixture packet field")
}
fn canonical(value: &JsonValue) -> Vec<u8> {
    canonical_bytes_v1(
        value,
        CanonicalProfile::SourceRecordDigestV1,
        JsonLimits::default(),
    )
    .unwrap()
}

#[test]
fn producer_selected_indexed_pages_match_python_rank_and_original_carriers() {
    let fixture = build_fixture();
    let input = parse_json(
        &fixture.graph_input_bytes,
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap()
    .into_root();
    let input_nodes = field(&input, "nodes").as_array().unwrap();
    assert_eq!(input_nodes.len(), 4);
    let cold = fixture.open().unwrap();
    let bound =
        bind_verified_knowledge(&cold, &fixture.vocabulary, &fixture.descriptor_bytes).unwrap();
    let mut reader = cold.fork_reader_with_vm_budget(1_000_000).unwrap();
    let policy = CurrentPolicyBinding {
        scope: "synthetic-public-projection".into(),
        issuer_ref: "synthetic-issuer".into(),
        authorization_receipt_id: "synthetic-receipt".into(),
        policy_epoch: "synthetic-epoch-1".into(),
        withdrawal_generation: "synthetic-withdrawal-1".into(),
    };
    let scope = IndexedDisclosureScope {
        operation_id: "tos.knowledge.search".into(),
        carrier_layer: "tos_knowledge_public_graph_projection_v1".into(),
        intended_use: "read_only_public_knowledge_search_v1".into(),
        selected_model_receipt_id: bound.owner_receipt_id().into(),
        source_cut: bound.selection().source_cut.clone(),
        through_commit_seq: bound.selection().through_commit_seq,
        source_membership_root: bound.selection().source_membership_root,
        descriptor_sha256: bound.selection().vocabulary.descriptor_sha256,
        selected_index_sha256: bound.selection().index_root_sha256,
        policy_issuer_ref: policy.issuer_ref.clone(),
        policy_receipt_id: policy.authorization_receipt_id.clone(),
        policy_scope: policy.scope.clone(),
        policy_epoch: policy.policy_epoch.clone(),
        withdrawal_generation: policy.withdrawal_generation.clone(),
    };
    let checks = Arc::new(AtomicUsize::new(0));
    let mut authority = SyntheticAuthority {
        policy,
        scope,
        consulted: Vec::new(),
        lease_checks: Arc::clone(&checks),
    };
    let mut cursor = SyntheticCursor::default();
    let request = IndexedSearchV2Request {
        query: "alpha".into(),
        sources: vec!["eighth".into(), "zero".into()],
        kind_ids: Vec::new(),
        predicate_ids: Vec::new(),
        limit: 1,
    };
    let mut token: Option<String> = None;
    let mut returned = Vec::new();
    for page_index in 0..2 {
        let mut page = execute_indexed_search_page(
            &mut reader,
            &bound,
            &mut authority,
            &mut cursor,
            request.clone(),
            token.as_deref(),
            budget(),
        )
        .unwrap();
        page.recheck().unwrap();
        let packet = parse_json(&page, JsonMode::PublishedStrict, JsonLimits::default())
            .unwrap()
            .into_root();
        assert_eq!(
            field(&packet, "schema").as_str(),
            Some("tos_knowledge_search_indexed_v2")
        );
        assert_eq!(
            field(&packet, "source_revision").as_str(),
            Some("2".repeat(64).as_str())
        );
        assert_eq!(
            canonical(field(&packet, "authority_boundary")),
            canonical(field(&input, "authority_boundary"))
        );
        let nodes = field(&packet, "nodes").as_array().unwrap();
        assert_eq!(nodes.len(), 1);
        assert!(field(&packet, "relations").as_array().unwrap().is_empty());
        let item = &nodes[0];
        let id = field(item, "id").as_str().unwrap();
        assert_eq!(id, PYTHON_ALPHA_ORDER[page_index]);
        let original = input_nodes
            .iter()
            .find(|source| field(source, "id").as_str() == Some(id))
            .unwrap();
        assert_eq!(canonical(item), canonical(original));
        returned.push(id.to_owned());
        assert!(field(field(&packet, "counts"), "matching_nodes").is_null());
        token = field(field(&packet, "page"), "next_cursor")
            .as_str()
            .map(str::to_owned);
        assert_eq!(token.is_some(), page_index == 0);
        drop(page);
    }
    assert_eq!(returned, PYTHON_ALPHA_ORDER);
    assert!(authority.consulted.iter().any(|id| id == FALSE_POSITIVE));
    assert!(checks.load(Ordering::Relaxed) >= 4);
    assert_eq!(
        Digest256::of_bytes(&fixture.graph_input_bytes)
            .to_hex()
            .len(),
        64
    );
}
