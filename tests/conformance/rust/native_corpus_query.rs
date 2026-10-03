// Private continuation of the existing native corpus producer case.
// This is finite selected projection mechanics, not a production rights issuer.

const NATIVE_CORPUS_QUERY_ORACLE: &str = r#"
from tos_access.core import ToSAccessCore
index_path=root/sys.argv[4]
index_path.parent.mkdir(parents=True,exist_ok=True)
index_path.write_text(owner.render_payload(payload),encoding='utf-8')
core=ToSAccessCore.discover(tos_root=root)
def first(collection,key):
 return next(item for item in payload[collection] if isinstance(item,dict) and isinstance(item.get(key),str))
node=first('nodes','node_id');pack=first('relation_packs','pack_id')
edge=first('relation_edges','from_id');resource=first('resources','resource_kind')
query=node.get('label') if isinstance(node.get('label'),str) else node['node_id']
kind=resource.get('resource_kind');branch=resource.get('owner_branch')
assert branch is None or isinstance(branch,str)
cases={
 'status':core.status(),'summary':core.summary(),
 'search':core.search(query,limit=2),
 'search-filtered':core.search('',limit=2,resource_kind=kind),
 'resources':core.resources(resource_kind=kind,owner_branch=branch,limit=1),
 'node':core.node(node['node_id']),'endpoint':core.node(edge['from_id']),
 'pack':core.relation_pack(pack['pack_id']),
 'topology':core.graph_view('corpus-topology',limit=1),
 'route':core.graph_view('route-graph',limit=1),
 'promotion':core.graph_view('promotion-flow',limit=1),
 'packet':core.packet(query=' ',view_id='route-graph',limit=1),
 'packet-empty':core.packet(query='',view_id='',limit=1),
}
(root/'native-corpus-query-oracle.json').write_text(json.dumps({
 'node_id':node['node_id'],'endpoint_id':edge['from_id'],'pack_id':pack['pack_id'],
 'query':query,'resource_kind':kind,'owner_branch':branch,
 'tos_root':core.tos_root.as_posix(),'index_path':core.index_path.as_posix(),
 'cases':cases},ensure_ascii=False,allow_nan=False),encoding='utf-8')
"#;

fn assert_native_corpus_query_packets(
    selected: &tos_compiler::knowledge_full_fixture::FullKnowledgeFixture,
    projection: &tos_compiler::source_corpus::NativeCorpusProjection,
    expected: &serde_json::Value,
    oracle_root: &std::path::Path,
    output_path: &str,
    budget: tos_query::corpus_read::CorpusReadBudget,
) -> Vec<(
    tos_query::corpus_read::CorpusReadRequest,
    tos_foundation::JsonValue,
)> {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use tos_foundation::{
        CanonicalProfile, Digest256, JsonMode, JsonValue, canonical_bytes_v1, parse_json,
    };
    use tos_query::corpus_read::{
        CORPUS_INTENDED_USE, CorpusReadContext, CorpusReadRequest as R, execute_selected_corpus,
    };
    use tos_query::search_v2::{CurrentPolicyBinding, SearchV2Error, SearchV2ErrorCode};
    use tos_query::{
        IndexedDisclosureScope, InspectCurrentAuthority, InspectDisclosureLease, InspectedCarrier,
        ObservedInspectCarrier, bind_verified_knowledge,
    };
    fn withdrawn() -> SearchV2Error {
        SearchV2Error {
            code: SearchV2ErrorCode::StalePolicy,
            message: "finite projection hold withdrawn",
        }
    }
    struct Hold(Arc<AtomicBool>);
    impl InspectDisclosureLease for Hold {
        fn recheck(&mut self) -> Result<(), SearchV2Error> {
            if self.0.load(Ordering::SeqCst) {
                Err(withdrawn())
            } else {
                Ok(())
            }
        }
    }
    // The same selected original/current-hold law used by the existing finite
    // corpus oracle. Its explicit synthetic policy does not activate main.
    struct ProjectionAuthority {
        policy: CurrentPolicyBinding,
        scope: IndexedDisclosureScope,
        receipt: tos_compiler::CorpusOriginalReceipt,
        rows: Vec<(tos_compiler::CorpusOriginalCollection, u64)>,
        deny: bool,
        withdrawn: Arc<AtomicBool>,
    }
    impl<'hold> InspectCurrentAuthority<'hold> for ProjectionAuthority {
        fn policy_binding(&self) -> CurrentPolicyBinding {
            self.policy.clone()
        }
        fn disclosure_scope(&self) -> IndexedDisclosureScope {
            self.scope.clone()
        }
        fn check_selected(&mut self) -> Result<(), SearchV2Error> {
            if self.withdrawn.load(Ordering::SeqCst) {
                Err(withdrawn())
            } else {
                Ok(())
            }
        }
        fn authorize_current(&mut self, _: &InspectedCarrier) -> Result<(), SearchV2Error> {
            Err(SearchV2Error {
                code: SearchV2ErrorCode::Unavailable,
                message: "corpus oracle grants originals only",
            })
        }
        fn authorize_corpus_original_current(
            &mut self,
            receipt: &tos_compiler::CorpusOriginalReceipt,
            collection: tos_compiler::CorpusOriginalCollection,
            ordinal: u64,
            raw: &[u8],
            sha: Digest256,
        ) -> Result<(), SearchV2Error> {
            self.check_selected()?;
            if self.deny {
                return Err(SearchV2Error {
                    code: SearchV2ErrorCode::Unavailable,
                    message: "finite original grant denied",
                });
            }
            assert_eq!(
                receipt.component_root_sha256,
                self.receipt.component_root_sha256
            );
            assert_eq!(receipt.descriptor_sha256, self.receipt.descriptor_sha256);
            assert_eq!(receipt.source_cut, self.receipt.source_cut);
            assert_eq!(receipt.membership_root, self.receipt.membership_root);
            assert_eq!(Digest256::of_bytes(raw), sha);
            self.rows.push((collection, ordinal));
            Ok(())
        }
        fn acquire_disclosure(
            &mut self,
            scope: &IndexedDisclosureScope,
            consulted: &[ObservedInspectCarrier],
        ) -> Result<Box<dyn InspectDisclosureLease + 'hold>, SearchV2Error> {
            self.check_selected()?;
            assert_eq!(scope.operation_id, self.scope.operation_id);
            assert_eq!(scope.intended_use, CORPUS_INTENDED_USE);
            assert_eq!(
                self.rows.first(),
                Some(&(tos_compiler::CorpusOriginalCollection::Header, 0))
            );
            assert!(consulted.is_empty());
            Ok(Box::new(Hold(self.withdrawn.clone())))
        }
    }
    let mut cold = selected.open().unwrap();
    let receipt = cold.corpus_original_receipt().unwrap().clone();
    assert_eq!(
        receipt.profile,
        tos_compiler::NATIVE_CORPUS_ORIGINAL_PROFILE
    );
    assert_eq!(receipt.origin.profile, "native-corpus-producer-v1");
    assert_eq!(receipt.origin.source_path, output_path);
    assert_eq!(
        receipt.origin.source_sha256,
        Digest256::of_bytes(projection.output_bytes()).to_hex()
    );
    assert_eq!(
        receipt.origin.source_size_bytes,
        u64::try_from(projection.output_bytes().len()).unwrap()
    );
    assert_eq!(
        serde_json::to_vec(receipt.origin.native_producer.as_ref().unwrap()).unwrap(),
        serde_json::to_vec(projection.receipt()).unwrap()
    );
    assert_eq!(projection.value(), expected);
    assert!(
        receipt.origin.source_git_commit.is_none()
            && receipt.origin.source_git_tree.is_none()
            && receipt.origin.capture_manifest_sha256.is_none()
    );
    let bound =
        bind_verified_knowledge(&cold, &selected.vocabulary, &selected.descriptor_bytes).unwrap();
    let oracle = parse_json(
        &std::fs::read(oracle_root.join("native-corpus-query-oracle.json")).unwrap(),
        JsonMode::PublishedStrict,
        budget.inspect.json,
    )
    .unwrap()
    .into_root();
    let field = |name| oracle.object_get(name).unwrap();
    let string = |name| field(name).as_str().unwrap().to_owned();
    let optional = |name| match field(name) {
        JsonValue::Null => None,
        value => Some(value.as_str().unwrap().to_owned()),
    };
    let context = CorpusReadContext {
        tos_root: string("tos_root"),
        index_path: string("index_path"),
    };
    let authority = |request: &R| {
        let policy = CurrentPolicyBinding {
            scope: "finite-native-corpus-oracle".into(),
            issuer_ref: "synthetic-fixture".into(),
            authorization_receipt_id: "synthetic-fixture".into(),
            policy_epoch: "fixture-epoch".into(),
            withdrawal_generation: "fixture-generation".into(),
        };
        ProjectionAuthority {
            scope: IndexedDisclosureScope {
                operation_id: request.operation_id().into(),
                carrier_layer: "tos_knowledge_public_graph_projection_v1".into(),
                intended_use: CORPUS_INTENDED_USE.into(),
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
            },
            policy,
            receipt: receipt.clone(),
            rows: vec![],
            deny: false,
            withdrawn: Arc::new(AtomicBool::new(false)),
        }
    };
    let view = |view_id: &str| R::GraphView {
        view_id: view_id.into(),
        limit: 1,
    };
    let requests = [
        ("status", R::Status),
        ("summary", R::Summary),
        (
            "search",
            R::Search {
                query: string("query"),
                limit: 2,
                resource_kind: None,
            },
        ),
        (
            "search-filtered",
            R::Search {
                query: String::new(),
                limit: 2,
                resource_kind: optional("resource_kind"),
            },
        ),
        (
            "resources",
            R::Resources {
                resource_kind: optional("resource_kind"),
                owner_branch: optional("owner_branch"),
                limit: 1,
            },
        ),
        (
            "node",
            R::Node {
                node_id: string("node_id"),
            },
        ),
        (
            "endpoint",
            R::Node {
                node_id: string("endpoint_id"),
            },
        ),
        (
            "pack",
            R::RelationPack {
                pack_id: string("pack_id"),
            },
        ),
        ("topology", view("corpus-topology")),
        ("route", view("route-graph")),
        ("promotion", view("promotion-flow")),
        (
            "packet",
            R::Packet {
                query: " ".into(),
                view_id: Some("route-graph".into()),
                limit: 1,
            },
        ),
        (
            "packet-empty",
            R::Packet {
                query: String::new(),
                view_id: Some(String::new()),
                limit: 1,
            },
        ),
    ];
    for (name, request) in &requests {
        let mut current = authority(request);
        let mut packet =
            execute_selected_corpus(&mut cold, &bound, &mut current, &context, request, budget)
                .unwrap();
        let expected = canonical_bytes_v1(
            field("cases").object_get(name).unwrap(),
            CanonicalProfile::SourceRecordDigestV1,
            budget.inspect.json,
        )
        .unwrap();
        assert_eq!(&*packet, expected.as_slice(), "native corpus {name}");
        packet.recheck().unwrap();
    }
    let request = &requests[5].1;
    let mut current = authority(request);
    let mut held =
        execute_selected_corpus(&mut cold, &bound, &mut current, &context, request, budget)
            .unwrap();
    let addressed_rows = u64::try_from(current.rows.len()).unwrap();
    let total_rows = receipt.collections.iter().map(|c| c.rows).sum::<u64>() + 1;
    assert!(
        addressed_rows < total_rows,
        "addressed node must not materialize the whole native corpus"
    );
    let mut addressed = budget;
    addressed.inspect.max_rows = addressed_rows;
    execute_selected_corpus(
        &mut cold,
        &bound,
        &mut authority(request),
        &context,
        request,
        addressed,
    )
    .unwrap()
    .recheck()
    .unwrap();
    addressed.inspect.max_rows = addressed_rows.checked_sub(1).unwrap();
    assert_eq!(
        execute_selected_corpus(
            &mut cold,
            &bound,
            &mut authority(request),
            &context,
            request,
            addressed
        )
        .err()
        .unwrap()
        .code,
        SearchV2ErrorCode::BudgetExceeded
    );
    current.withdrawn.store(true, Ordering::SeqCst);
    assert_eq!(
        held.recheck().unwrap_err().code,
        SearchV2ErrorCode::StalePolicy
    );
    let mut denied = authority(request);
    denied.deny = true;
    assert_eq!(
        execute_selected_corpus(&mut cold, &bound, &mut denied, &context, request, budget)
            .err()
            .unwrap()
            .code,
        SearchV2ErrorCode::Unavailable
    );
    requests
        .into_iter()
        .map(|(name, request)| (request, field("cases").object_get(name).unwrap().clone()))
        .collect()
}
