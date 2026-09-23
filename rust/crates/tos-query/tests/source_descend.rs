use std::collections::BTreeMap;

use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, JsonMode, JsonValue,
    canonical_bytes_v1, parse_json,
};
use tos_query::{
    AdjacencyPage, Binding, Budget, Charged, DisclosureLease, ExactNode, QueryError,
    QueryErrorCode, RawRecord, ReadModel, SourceDescendRequest, source_descend,
};

struct SyntheticLease;
impl DisclosureLease for SyntheticLease {
    fn recheck(&mut self) -> Result<(), QueryError> {
        Ok(())
    }
}

fn field<'a>(value: &'a JsonValue, name: &str) -> &'a JsonValue {
    value.object_get(name).expect("fixture field")
}

fn string<'a>(value: &'a JsonValue, name: &str) -> &'a str {
    field(value, name).as_str().expect("fixture string")
}

fn raw(value: &JsonValue, binding: &Binding) -> RawRecord {
    let raw = canonical_bytes_v1(
        value,
        CanonicalProfile::SourceRecordDigestV1,
        JsonLimits::default(),
    )
    .unwrap();
    RawRecord {
        binding: binding.clone(),
        sha256: Digest256::of_bytes(&raw),
        raw,
    }
}

struct SyntheticReadModel {
    binding: Binding,
    nodes: BTreeMap<String, RawRecord>,
    edges: BTreeMap<String, Vec<RawRecord>>,
    authority: RawRecord,
    bad_digest: bool,
    stale_page: bool,
    denied: bool,
    denied_id: Option<String>,
    late_denied_id: Option<String>,
    pin_checks: usize,
}

impl SyntheticReadModel {
    fn fixture() -> (Self, JsonValue, SourceDescendRequest) {
        let fixture = parse_json(
            include_bytes!("fixtures/source_descend_python_oracle.json"),
            JsonMode::PublishedStrict,
            JsonLimits::default(),
        )
        .unwrap()
        .into_root();
        let binding = Binding {
            source_cut: "synthetic-sealed-cut".into(),
            through_commit_seq: 17,
            membership_root: Digest256::of_bytes(b"membership"),
            projection_root: Digest256::of_bytes(b"projection"),
            index_root: Digest256::of_bytes(b"index"),
            index_generation: "generation-a".into(),
            route_map_version: "route-a".into(),
            reader_abi: "tos-source-navigation-visible-v1".into(),
            model_abi: "tos_source_navigation_read_model_v1".into(),
            selection_profile: "tos_source_navigation_visible_v1".into(),
        };
        let nodes = field(&fixture, "nodes")
            .as_array()
            .unwrap()
            .iter()
            .map(|value| (string(value, "node_id").to_owned(), raw(value, &binding)))
            .collect();
        let mut edges: BTreeMap<String, Vec<RawRecord>> = BTreeMap::new();
        for value in field(&fixture, "edges").as_array().unwrap() {
            edges
                .entry(string(value, "from_id").to_owned())
                .or_default()
                .push(raw(value, &binding));
        }
        for values in edges.values_mut() {
            values.sort_by_key(|value| {
                let parsed =
                    parse_json(&value.raw, JsonMode::PublishedStrict, JsonLimits::default())
                        .unwrap();
                string(parsed.root(), "edge_id").to_owned()
            });
        }
        let authority = raw(field(&fixture, "authority_note"), &binding);
        let request_value = field(&fixture, "request");
        let request = SourceDescendRequest {
            node_id: string(request_value, "node_id").to_owned(),
            max_depth: field(request_value, "max_depth").as_u64().unwrap() as u8,
            limit: field(request_value, "limit").as_u64().unwrap() as usize,
            at_least_commit_seq: None,
        };
        let expected = field(&fixture, "expected").clone();
        (
            Self {
                binding,
                nodes,
                edges,
                authority,
                bad_digest: false,
                stale_page: false,
                denied: false,
                denied_id: None,
                late_denied_id: None,
                pin_checks: 0,
            },
            expected,
            request,
        )
    }

    fn edge_id(record: &RawRecord) -> String {
        let parsed = parse_json(
            &record.raw,
            JsonMode::PublishedStrict,
            JsonLimits::default(),
        )
        .unwrap();
        string(parsed.root(), "edge_id").to_owned()
    }

    fn certificate(records: &[RawRecord]) -> (u64, Digest256) {
        let mut hasher = Digest256Hasher::new();
        for record in records {
            let key = Self::edge_id(record);
            hasher.update(&(key.len() as u64).to_be_bytes());
            hasher.update(key.as_bytes());
            hasher.update(record.sha256.as_bytes());
        }
        (records.len() as u64, hasher.finalize())
    }
}

impl ReadModel for SyntheticReadModel {
    fn selected_binding(&mut self) -> Result<Binding, QueryError> {
        Ok(self.binding.clone())
    }
    fn exact_visible_node(
        &mut self,
        id: &str,
        max_bytes: usize,
        max_carrier_bytes: usize,
        max_vm_steps: u64,
    ) -> Result<ExactNode, QueryError> {
        assert!(max_vm_steps > 0);
        if self
            .nodes
            .get(id)
            .is_some_and(|record| record.raw.len() > max_bytes.min(max_carrier_bytes))
        {
            return Err(QueryError {
                code: QueryErrorCode::BudgetExceeded,
                message: "synthetic selected row over byte cap",
            });
        }
        Ok(ExactNode {
            binding: self.binding.clone(),
            record: self.nodes.get(id).cloned(),
            complete_unique_lookup: true,
            charged: Charged::default(),
        })
    }
    fn visible_outgoing(
        &mut self,
        from_id: &str,
        after_edge_id: Option<&str>,
        max_rows: usize,
        max_bytes: usize,
        max_carrier_bytes: usize,
        max_vm_steps: u64,
    ) -> Result<AdjacencyPage, QueryError> {
        assert!(max_vm_steps > 0);
        let all = self.edges.get(from_id).map(Vec::as_slice).unwrap_or(&[]);
        let start = all
            .iter()
            .position(|record| {
                after_edge_id.is_none_or(|after| Self::edge_id(record).as_str() > after)
            })
            .unwrap_or(all.len());
        let end = (start + max_rows).min(all.len());
        if all[start..end]
            .iter()
            .any(|record| record.raw.len() > max_carrier_bytes)
            || all[start..end]
                .iter()
                .map(|record| record.raw.len())
                .sum::<usize>()
                > max_bytes
        {
            return Err(QueryError {
                code: QueryErrorCode::BudgetExceeded,
                message: "synthetic selected adjacency over byte cap",
            });
        }
        let (count, mut digest) = Self::certificate(all);
        if self.bad_digest {
            digest = Digest256::of_bytes(b"wrong sealed certificate");
        }
        let mut binding = self.binding.clone();
        if self.stale_page {
            binding.index_generation = "another-generation".into();
        }
        Ok(AdjacencyPage {
            binding,
            from_id: from_id.into(),
            after_edge_id: after_edge_id.map(str::to_owned),
            edges: all[start..end].to_vec(),
            exhausted: end == all.len(),
            expected_count: count,
            expected_digest: digest,
            charged: Charged::default(),
        })
    }
    fn authorize_current(&mut self, record: &RawRecord) -> Result<Charged, QueryError> {
        let selected = parse_json(
            &record.raw,
            JsonMode::PublishedStrict,
            JsonLimits::default(),
        )
        .unwrap();
        let record_id = selected
            .root()
            .object_get("node_id")
            .and_then(JsonValue::as_str);
        if self.denied
            || self
                .denied_id
                .as_deref()
                .is_some_and(|denied_id| record_id == Some(denied_id))
        {
            return Err(QueryError {
                code: QueryErrorCode::PolicyDenied,
                message: "synthetic current rights withdrawal",
            });
        }
        Ok(Charged {
            probes: 1,
            cpu_steps: 1,
            ..Charged::default()
        })
    }
    fn authority_boundary(&mut self, max_bytes: usize) -> Result<RawRecord, QueryError> {
        if self.authority.raw.len() > max_bytes {
            return Err(QueryError {
                code: QueryErrorCode::BudgetExceeded,
                message: "synthetic authority over byte cap",
            });
        }
        Ok(self.authority.clone())
    }
    fn check_pin(&mut self, binding: &Binding) -> Result<(), QueryError> {
        self.pin_checks += 1;
        if binding != &self.binding {
            return Err(QueryError {
                code: QueryErrorCode::StaleSelection,
                message: "synthetic pin changed",
            });
        }
        Ok(())
    }
    fn acquire_disclosure(
        &mut self,
        binding: &Binding,
        selected: &[RawRecord],
    ) -> Result<Box<dyn DisclosureLease>, QueryError> {
        self.check_pin(binding)?;
        if selected.is_empty() {
            return Err(QueryError {
                code: QueryErrorCode::Unavailable,
                message: "synthetic selection empty",
            });
        }
        for record in selected {
            self.authorize_current(record)?;
            let value = parse_json(
                &record.raw,
                JsonMode::PublishedStrict,
                JsonLimits::default(),
            )
            .unwrap();
            if self.late_denied_id.as_deref().is_some_and(|denied_id| {
                value
                    .root()
                    .object_get("node_id")
                    .and_then(JsonValue::as_str)
                    == Some(denied_id)
            }) {
                return Err(QueryError {
                    code: QueryErrorCode::PolicyDenied,
                    message: "synthetic revocation at disclosure acquisition",
                });
            }
        }
        Ok(Box::new(SyntheticLease))
    }
}

fn budget() -> Budget {
    Budget {
        max_probes: 100,
        max_rows: 100,
        max_bytes: 1_000_000,
        max_cpu_steps: 1000,
        max_edges: 10,
        max_response_bytes: 100_000,
        max_request_bytes: 4096,
        page_rows: 1,
        json: JsonLimits::default(),
    }
}

fn semantic_eq(left: &JsonValue, right: &JsonValue) -> bool {
    match (left, right) {
        (JsonValue::Object(a), JsonValue::Object(b)) => {
            a.len() == b.len()
                && a.iter().all(|(key, value)| {
                    let Some(name) = key.as_str() else {
                        return false;
                    };
                    b.iter()
                        .find(|(other, _)| other.as_str() == Some(name))
                        .is_some_and(|(_, other)| semantic_eq(value, other))
                })
        }
        (JsonValue::Array(a), JsonValue::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| semantic_eq(x, y))
        }
        _ => left == right,
    }
}

#[test]
fn source_descend_matches_frozen_python_oracle() {
    let (mut model, expected, request) = SyntheticReadModel::fixture();
    assert_eq!(
        field(&expected, "authority_note").as_str(),
        Some("source_navigation_derived_only_assessed_form_requires_owner_review")
    );
    let packet = source_descend(&mut model, &request, budget()).unwrap();
    let actual = parse_json(&packet, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
    assert!(
        semantic_eq(actual.root(), &expected),
        "Rust packet differs from Python oracle"
    );
    assert!(model.pin_checks >= 3);
}

#[test]
fn invalid_selected_authority_boundary_is_rejected() {
    let (mut model, _, request) = SyntheticReadModel::fixture();
    for invalid in [br#""""#.as_slice(), br#"{}"#.as_slice()] {
        let value = parse_json(invalid, JsonMode::PublishedStrict, JsonLimits::default())
            .unwrap()
            .into_root();
        model.authority = raw(&value, &model.binding);
        assert_eq!(
            source_descend(&mut model, &request, budget())
                .unwrap_err()
                .code,
            QueryErrorCode::CorruptSelectedCarrier
        );
    }
}

#[test]
fn empty_adjacency_is_a_complete_success() {
    let (mut model, _, mut request) = SyntheticReadModel::fixture();
    request.node_id = "δ".into();
    let packet = source_descend(&mut model, &request, budget()).unwrap();
    let actual = parse_json(&packet, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
    assert_eq!(
        field(field(actual.root(), "counts"), "edges").as_u64(),
        Some(0)
    );
}

#[test]
fn sealed_scope_and_current_policy_fail_closed() {
    let (mut model, _, request) = SyntheticReadModel::fixture();
    model.bad_digest = true;
    assert_eq!(
        source_descend(&mut model, &request, budget())
            .unwrap_err()
            .code,
        QueryErrorCode::IndexIncomplete
    );
    model.bad_digest = false;
    model.stale_page = true;
    assert_eq!(
        source_descend(&mut model, &request, budget())
            .unwrap_err()
            .code,
        QueryErrorCode::StaleSelection
    );
    model.stale_page = false;
    model.denied = true;
    assert_eq!(
        source_descend(&mut model, &request, budget())
            .unwrap_err()
            .code,
        QueryErrorCode::PolicyDenied
    );
}

#[test]
fn exact_and_one_over_selected_edge_budget() {
    let (mut model, _, request) = SyntheticReadModel::fixture();
    let mut limits = budget();
    limits.max_edges = 3;
    source_descend(&mut model, &request, limits).unwrap();
    limits.max_edges = 2;
    assert_eq!(
        source_descend(&mut model, &request, limits)
            .unwrap_err()
            .code,
        QueryErrorCode::BudgetExceeded
    );
}

#[test]
fn skipped_but_existing_target_cannot_leak_through_truncation() {
    let (mut model, _, request) = SyntheticReadModel::fixture();
    model.denied_id = Some("δ".into());
    assert_eq!(
        source_descend(&mut model, &request, budget())
            .unwrap_err()
            .code,
        QueryErrorCode::PolicyDenied
    );
}

#[test]
fn skipped_target_revoked_at_disclosure_cannot_leak_truncation() {
    let (mut model, _, request) = SyntheticReadModel::fixture();
    model.late_denied_id = Some("δ".into());
    assert_eq!(
        source_descend(&mut model, &request, budget())
            .unwrap_err()
            .code,
        QueryErrorCode::PolicyDenied
    );
}

#[test]
fn receipt_lower_bound_requires_a_sealed_publication() {
    let (mut model, _, mut request) = SyntheticReadModel::fixture();
    request.at_least_commit_seq = Some(18);
    assert_eq!(
        source_descend(&mut model, &request, budget())
            .unwrap_err()
            .code,
        QueryErrorCode::PublicationPending
    );
    request.at_least_commit_seq = Some(17);
    source_descend(&mut model, &request, budget()).unwrap();
}

#[test]
fn selected_row_byte_cap_is_enforced_before_transfer() {
    let (mut model, _, request) = SyntheticReadModel::fixture();
    let mut limits = budget();
    limits.max_bytes = 1;
    assert_eq!(
        source_descend(&mut model, &request, limits)
            .unwrap_err()
            .code,
        QueryErrorCode::BudgetExceeded
    );
}
