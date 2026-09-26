#![cfg(not(target_arch = "wasm32"))]
//! Independent legacy packet oracle over the existing CMP producer fixture.
use tos_compiler::knowledge_full_fixture::build_fixture;
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonMode, JsonValue, canonical_bytes_v1, parse_json,
};
use tos_query::search_v2::{CurrentPolicyBinding, SearchKind, SearchV2Error, SearchV2ErrorCode};
use tos_query::{
    BoundCmpKnowledge, INSPECT_INTENDED_USE, IndexedDisclosureScope, InspectBudget,
    InspectCurrentAuthority, InspectDisclosureLease, InspectedCarrier, NODE_INSPECT_OPERATION,
    RELATION_INSPECT_OPERATION, bind_verified_knowledge, execute_selected_inspect,
};
struct Lease;
impl InspectDisclosureLease for Lease {
    fn recheck(&mut self) -> Result<(), SearchV2Error> {
        Ok(())
    }
}
struct Authority {
    scope: IndexedDisclosureScope,
    policy: CurrentPolicyBinding,
    consulted: Vec<String>,
    withdrawn: bool,
}
impl Authority {
    fn new(bound: &BoundCmpKnowledge<'_>, kind: SearchKind) -> Self {
        let policy = CurrentPolicyBinding {
            scope: "synthetic-inspect".into(),
            issuer_ref: "synthetic-issuer".into(),
            authorization_receipt_id: "synthetic-receipt".into(),
            policy_epoch: "synthetic-epoch".into(),
            withdrawal_generation: "synthetic-withdrawal".into(),
        };
        Self {
            scope: IndexedDisclosureScope {
                operation_id: if kind == SearchKind::Nodes {
                    NODE_INSPECT_OPERATION
                } else {
                    RELATION_INSPECT_OPERATION
                }
                .into(),
                carrier_layer: "tos_knowledge_public_graph_projection_v1".into(),
                intended_use: INSPECT_INTENDED_USE.into(),
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
            consulted: vec![],
            withdrawn: false,
        }
    }
}
impl InspectCurrentAuthority for Authority {
    fn policy_binding(&self) -> CurrentPolicyBinding {
        self.policy.clone()
    }
    fn disclosure_scope(&self) -> IndexedDisclosureScope {
        self.scope.clone()
    }
    fn check_selected(&mut self) -> Result<(), SearchV2Error> {
        Ok(())
    }
    fn authorize_current(&mut self, carrier: &InspectedCarrier) -> Result<(), SearchV2Error> {
        self.consulted.push(carrier.id.clone());
        Ok(())
    }
    fn acquire_disclosure(
        &mut self,
        _: &IndexedDisclosureScope,
        consulted: &[InspectedCarrier],
    ) -> Result<Box<dyn InspectDisclosureLease>, SearchV2Error> {
        if self.withdrawn {
            return Err(SearchV2Error {
                code: SearchV2ErrorCode::StalePolicy,
                message: "synthetic withdrawal",
            });
        }
        assert_eq!(
            consulted
                .iter()
                .map(|row| row.id.clone())
                .collect::<Vec<_>>(),
            self.consulted
        );
        Ok(Box::new(Lease))
    }
}
fn budget() -> InspectBudget {
    InspectBudget {
        max_open_vm_steps: 100_000_000,
        max_read_vm_steps: 1_000_000,
        max_matches: 64,
        max_rows: 1000,
        max_field_bytes: 8192,
        max_payload_bytes: 1_000_000,
        max_decoded_bytes: 8_000_000,
        max_response_bytes: 1_000_000,
        json: JsonLimits::default(),
    }
}
fn field<'a>(value: &'a JsonValue, key: &str) -> &'a JsonValue {
    value.object_get(key).unwrap()
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
fn producer_selected_inspect_matches_full_python_packets_and_refuses_budget_or_withdrawal() {
    let fixture = build_fixture();
    let oracle = parse_json(
        include_bytes!("fixtures/cmp_knowledge_inspect_python_oracle.json"),
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap()
    .into_root();
    assert_eq!(
        Digest256::of_bytes(&fixture.graph_input_bytes).to_hex(),
        field(&oracle, "input_sha256").as_str().unwrap()
    );
    let cold = fixture.open().unwrap();
    let bound =
        bind_verified_knowledge(&cold, &fixture.vocabulary, &fixture.descriptor_bytes).unwrap();
    let mut model = cold.fork_reader_with_vm_budget(1_000_000).unwrap();
    for case in field(&oracle, "cases").as_array().unwrap() {
        let kind = if field(case, "kind").as_str() == Some("nodes") {
            SearchKind::Nodes
        } else {
            SearchKind::Relations
        };
        let identifier = field(case, "identifier").as_str().unwrap();
        let JsonValue::Number(limit) = field(case, "relation_limit") else {
            panic!("integer limit")
        };
        let limit = limit.lexeme.parse().unwrap();
        let mut authority = Authority::new(&bound, kind);
        let mut result = execute_selected_inspect(
            &mut model,
            &bound,
            &mut authority,
            kind,
            identifier,
            limit,
            budget(),
        )
        .unwrap();
        result.recheck().unwrap();
        let actual = parse_json(&result, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
        assert_eq!(canonical(actual.root()), canonical(field(case, "packet")));
        for small in [
            InspectBudget {
                max_read_vm_steps: 1,
                ..budget()
            },
            InspectBudget {
                max_response_bytes: 1,
                ..budget()
            },
            InspectBudget {
                max_decoded_bytes: 1,
                ..budget()
            },
            InspectBudget {
                max_payload_bytes: 1,
                ..budget()
            },
        ] {
            let mut authority = Authority::new(&bound, kind);
            let result = execute_selected_inspect(
                &mut model,
                &bound,
                &mut authority,
                kind,
                identifier,
                limit,
                small,
            );
            assert!(matches!(
                result,
                Err(SearchV2Error {
                    code: SearchV2ErrorCode::BudgetExceeded,
                    ..
                })
            ));
        }
        let mut authority = Authority::new(&bound, kind);
        authority.withdrawn = true;
        assert!(matches!(
            execute_selected_inspect(
                &mut model,
                &bound,
                &mut authority,
                kind,
                identifier,
                limit,
                budget()
            ),
            Err(SearchV2Error {
                code: SearchV2ErrorCode::StalePolicy,
                ..
            })
        ));
    }
}
