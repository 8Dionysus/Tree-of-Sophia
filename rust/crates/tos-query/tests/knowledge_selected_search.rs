#![cfg(not(target_arch = "wasm32"))]

//! One producer-built CMP selection through QRY ranked packet continuation.
//! All custody and disclosure callbacks here are synthetic test holders.

use std::{
    collections::BTreeMap,
    io::Write,
    process::{Command, Stdio},
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
use tos_query::search_index::{GramSeekBudget, PostingSeekBudget, SearchPostingModel};
use tos_query::search_v2::{
    CurrentPolicyBinding, IndexedSearchV2Request, SearchContinuationState, SearchKind,
    SearchV2Error, SearchV2ErrorCode,
};
use tos_query::{
    CatalogBudget, CatalogCurrentAuthority, CatalogDisclosureLease, CatalogDisclosureScope,
    CatalogError, CatalogErrorCode, IndexedDisclosureLease, IndexedDisclosureScope,
    IndexedKnowledgeAuthority, IndexedPageBudget, IndexedWireCursorCodec, ObservedSearchCandidate,
    SearchDocumentBudget, SearchKindBudget, bind_verified_knowledge, execute_indexed_search_page,
    execute_selected_catalog,
};

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

struct SyntheticCatalogLease(Arc<AtomicUsize>);
impl CatalogDisclosureLease for SyntheticCatalogLease {
    fn recheck(&mut self) -> Result<(), CatalogError> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

struct SyntheticCatalogAuthority {
    policy: CurrentPolicyBinding,
    scope: CatalogDisclosureScope,
    checks: Arc<AtomicUsize>,
    expected_sha: Digest256,
}
impl<'hold> CatalogCurrentAuthority<'hold> for SyntheticCatalogAuthority {
    fn policy_binding(&self) -> CurrentPolicyBinding {
        self.policy.clone()
    }
    fn disclosure_scope(&self) -> CatalogDisclosureScope {
        self.scope.clone()
    }
    fn check_selected(&mut self) -> Result<(), CatalogError> {
        Ok(())
    }
    fn authorize_current(&mut self, sha: Digest256) -> Result<(), CatalogError> {
        assert_eq!(sha, self.expected_sha);
        Ok(())
    }
    fn acquire_disclosure(
        &mut self,
        _: &CatalogDisclosureScope,
        sha: Digest256,
    ) -> Result<Box<dyn CatalogDisclosureLease + 'hold>, CatalogError> {
        assert_eq!(sha, self.expected_sha);
        Ok(Box::new(SyntheticCatalogLease(Arc::clone(&self.checks))))
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
        _: &[ObservedSearchCandidate],
    ) -> Result<Box<dyn IndexedDisclosureLease>, SearchV2Error> {
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
    if let Some(path) = std::env::var_os("TOS_CMP_GRAPH_INPUT_EXPORT") {
        std::fs::write(path, &fixture.graph_input_bytes).unwrap();
    }
    let oracle = parse_json(
        include_bytes!("fixtures/cmp_knowledge_search_python_oracle.json"),
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap()
    .into_root();
    // The selected fixture retains the actual normalized source order. Run
    // the maintained Python reader over those exact bytes, then keep the
    // frozen complete packets as an independent semantic guard.
    let script = r#"
import hashlib,json,sys
from pathlib import Path
repo=Path(sys.argv[1]);sys.path.insert(0,str(repo/'access/src'))
from tos_access import knowledge as k
raw=sys.stdin.buffer.read(int(sys.argv[2]));graph=json.loads(raw)
descriptor=json.load(sys.stdin.buffer)
k.KNOWLEDGE_SOURCES=tuple(s['source_graph_id'] for s in descriptor['sources'])
oracle=json.loads((repo/'rust/crates/tos-query/tests/fixtures/cmp_knowledge_search_python_oracle.json').read_bytes())
sources=oracle['sources']
json.dump({'input_sha256':hashlib.sha256(raw).hexdigest(),
           'query':oracle['query'],'false_positive_query':oracle['false_positive_query'],
           'sources':sources,
           'reference':k.search_knowledge_graph(graph,oracle['query'],sources=sources),
           'false_positive_reference':k.search_knowledge_graph(graph,oracle['false_positive_query'],sources=sources)},
          sys.stdout,ensure_ascii=False,allow_nan=False)
"#;
    let mut child = Command::new("python3")
        .arg("-B")
        .arg("-c")
        .arg(script)
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.."))
        .arg(fixture.graph_input_bytes.len().to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    input.write_all(&fixture.graph_input_bytes).unwrap();
    input.write_all(&fixture.descriptor_bytes).unwrap();
    drop(input);
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let current_oracle = parse_json(
        &output.stdout,
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap()
    .into_root();
    assert_eq!(
        Digest256::of_bytes(&fixture.graph_input_bytes).to_hex(),
        field(&current_oracle, "input_sha256").as_str().unwrap()
    );
    for packet in ["reference", "false_positive_reference"] {
        assert_eq!(
            canonical(field(&current_oracle, packet)),
            canonical(field(&oracle, packet))
        );
    }
    let oracle = current_oracle;
    let reference = field(&oracle, "reference");
    let reference_nodes = field(reference, "nodes").as_array().unwrap();
    assert_eq!(reference_nodes.len(), 2);
    let cold = fixture.open().unwrap();
    let bound =
        bind_verified_knowledge(&cold, &fixture.vocabulary, &fixture.descriptor_bytes).unwrap();
    let mut reader = cold.fork_reader_with_vm_budget(1_000_000).unwrap();
    let first_posting = reader
        .seek_postings(SearchKind::Nodes, "alp", None, 1, 1_000_000, 808)
        .unwrap();
    assert_eq!(first_posting.positions.len(), 1);
    assert!(first_posting.charged.decoded_bytes > 8);
    let continued_posting = reader
        .seek_postings(
            SearchKind::Nodes,
            "alp",
            Some(first_posting.positions[0]),
            1,
            1_000_000,
            808,
        )
        .unwrap();
    assert_eq!(continued_posting.positions.len(), 1);
    assert_eq!(
        continued_posting.charged.decoded_bytes,
        first_posting.charged.decoded_bytes
    );
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
            field(&packet, "source_revision"),
            field(reference, "source_revision")
        );
        assert_eq!(
            canonical(field(&packet, "authority_boundary")),
            canonical(field(reference, "authority_boundary"))
        );
        let nodes = field(&packet, "nodes").as_array().unwrap();
        assert_eq!(nodes.len(), 1);
        assert!(field(&packet, "relations").as_array().unwrap().is_empty());
        let item = &nodes[0];
        let id = field(item, "id").as_str().unwrap();
        assert_eq!(
            id,
            field(&reference_nodes[page_index], "id").as_str().unwrap()
        );
        assert_eq!(canonical(item), canonical(&reference_nodes[page_index]));
        returned.push(id.to_owned());
        assert!(field(field(&packet, "counts"), "matching_nodes").is_null());
        token = field(field(&packet, "page"), "next_cursor")
            .as_str()
            .map(str::to_owned);
        assert_eq!(token.is_some(), page_index == 0);
        drop(page);
    }
    assert_eq!(
        returned,
        reference_nodes
            .iter()
            .map(|item| field(item, "id").as_str().unwrap())
            .collect::<Vec<_>>()
    );
    let prior_consulted = authority.consulted.len();
    let mut false_positive_page = execute_indexed_search_page(
        &mut reader,
        &bound,
        &mut authority,
        &mut cursor,
        IndexedSearchV2Request {
            query: field(&oracle, "false_positive_query")
                .as_str()
                .unwrap()
                .into(),
            ..request
        },
        None,
        budget(),
    )
    .unwrap();
    false_positive_page.recheck().unwrap();
    let false_packet = parse_json(
        &false_positive_page,
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap()
    .into_root();
    assert_eq!(
        canonical(field(&false_packet, "nodes")),
        canonical(field(field(&oracle, "false_positive_reference"), "nodes"))
    );
    assert!(
        authority.consulted[prior_consulted..]
            .iter()
            .any(|id| id == FALSE_POSITIVE)
    );
    assert!(checks.load(Ordering::Relaxed) >= 6);

    // Reuse the exact same producer-selected inode for the complete bounded
    // catalog compatibility packet. A one-byte cap must refuse before BLOB
    // transfer; the admitted request returns the selected packet unchanged.
    let policy = authority.policy.clone();
    let mut catalog_authority = SyntheticCatalogAuthority {
        scope: CatalogDisclosureScope {
            operation_id: "tos.knowledge.catalog".into(),
            carrier_layer: "tos_knowledge_public_graph_projection_v1".into(),
            intended_use: "read_only_public_knowledge_catalog_v1".into(),
            selected_model_receipt_id: bound.owner_receipt_id().into(),
            source_cut: bound.selection().source_cut.clone(),
            through_commit_seq: bound.selection().through_commit_seq,
            source_membership_root: bound.selection().source_membership_root,
            descriptor_sha256: bound.selection().vocabulary.descriptor_sha256,
            selected_index_sha256: bound.selection().index_root_sha256,
            catalog_packet_sha256: bound.selection().catalog_packet_sha256,
            policy_issuer_ref: policy.issuer_ref.clone(),
            policy_receipt_id: policy.authorization_receipt_id.clone(),
            policy_scope: policy.scope.clone(),
            policy_epoch: policy.policy_epoch.clone(),
            withdrawal_generation: policy.withdrawal_generation.clone(),
        },
        policy,
        checks: Arc::clone(&checks),
        expected_sha: bound.selection().catalog_packet_sha256,
    };
    let catalog_budget = CatalogBudget {
        max_open_vm_steps: 100_000_000,
        max_read_vm_steps: 1_000_000,
        max_packet_bytes: 1_000_000,
        max_decoded_bytes: 1_000_032,
        json: JsonLimits::default(),
    };
    let refused = execute_selected_catalog(
        &mut reader,
        &bound,
        &mut catalog_authority,
        CatalogBudget {
            max_packet_bytes: 1,
            max_decoded_bytes: 33,
            ..catalog_budget
        },
    );
    assert!(matches!(
        refused,
        Err(CatalogError {
            code: CatalogErrorCode::BudgetExceeded,
            ..
        })
    ));
    let mut catalog =
        execute_selected_catalog(&mut reader, &bound, &mut catalog_authority, catalog_budget)
            .unwrap();
    catalog.recheck().unwrap();
    assert_eq!(
        Digest256::of_bytes(&catalog),
        bound.selection().catalog_packet_sha256
    );
    let packet = parse_json(&catalog, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
    assert_eq!(
        field(packet.root(), "schema").as_str(),
        Some("tos_knowledge_catalog_v1")
    );
    assert_eq!(
        field(packet.root(), "source_revision").as_str(),
        bound.source_revision()
    );
}
