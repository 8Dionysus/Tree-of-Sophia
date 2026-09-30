use std::collections::BTreeMap;

use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, JsonMode, JsonString, JsonValue,
    canonical_bytes_v1, parse_json,
};
use tos_query::{
    AdjacencyPage, Binding, Budget, Charged, DisclosureLease, ExactNode, QueryError,
    QueryErrorCode, RawRecord, ReadModel, SOURCE_DESCEND_D1_METER_V1, SOURCE_DESCEND_SESSION_V1,
    SessionAdvance, SessionNeed, SessionNeedKind, SessionResponse, SessionResponseKind,
    SourceDescendRequest, SourceDescendSession, source_descend,
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
        max_work_probes: u64,
        max_work_rows: u64,
    ) -> Result<ExactNode, QueryError> {
        assert!(max_vm_steps > 0);
        if max_work_probes == 0 || (self.nodes.contains_key(id) && max_work_rows == 0) {
            return Err(QueryError {
                code: QueryErrorCode::BudgetExceeded,
                message: "synthetic exact work allowance exhausted",
            });
        }
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
            charged: Charged {
                probes: 1,
                rows: u64::from(self.nodes.contains_key(id)),
                ..Charged::default()
            },
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
        max_work_probes: u64,
        max_work_rows: u64,
    ) -> Result<AdjacencyPage, QueryError> {
        assert!(max_vm_steps > 0);
        if max_work_probes == 0 {
            return Err(QueryError {
                code: QueryErrorCode::BudgetExceeded,
                message: "synthetic adjacency probe allowance exhausted",
            });
        }
        let all = self.edges.get(from_id).map(Vec::as_slice).unwrap_or(&[]);
        let start = all
            .iter()
            .position(|record| {
                after_edge_id.is_none_or(|after| Self::edge_id(record).as_str() > after)
            })
            .unwrap_or(all.len());
        let end = (start + max_rows).min(all.len());
        if (end - start) as u64 > max_work_rows {
            return Err(QueryError {
                code: QueryErrorCode::BudgetExceeded,
                message: "synthetic adjacency row allowance exhausted",
            });
        }
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
            charged: Charged {
                probes: 1,
                rows: (end - start) as u64,
                ..Charged::default()
            },
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

fn synthetic_response(model: &mut SyntheticReadModel, need: &SessionNeed) -> SessionResponse {
    let (kind, charged) = match &need.kind {
        SessionNeedKind::CheckPin => {
            model.check_pin(&need.binding).unwrap();
            (SessionResponseKind::PinHeld, Charged::default())
        }
        SessionNeedKind::ExactNode { id } => {
            let mut got = model
                .exact_visible_node(
                    id,
                    need.caps.bytes as usize,
                    need.caps.carrier_bytes,
                    need.caps.cpu_steps,
                    need.caps.probes,
                    need.caps.rows,
                )
                .unwrap();
            got.charged.bytes = got
                .record
                .as_ref()
                .map_or(0, |record| record.raw.len() as u64);
            let charged = got.charged;
            (SessionResponseKind::ExactNode(got), charged)
        }
        SessionNeedKind::Outgoing {
            from_id,
            after_edge_id,
        } => {
            let mut page = model
                .visible_outgoing(
                    from_id,
                    after_edge_id.as_deref(),
                    need.caps.page_rows,
                    need.caps.bytes as usize,
                    need.caps.carrier_bytes,
                    need.caps.cpu_steps,
                    need.caps.probes,
                    need.caps.rows,
                )
                .unwrap();
            page.charged.bytes = page.edges.iter().map(|edge| edge.raw.len() as u64).sum();
            let charged = page.charged;
            (SessionResponseKind::Outgoing(page), charged)
        }
        SessionNeedKind::CurrentPolicy { sha256 } => {
            let record = model
                .nodes
                .values()
                .chain(model.edges.values().flatten())
                .chain(std::iter::once(&model.authority))
                .find(|item| item.sha256 == *sha256)
                .unwrap()
                .clone();
            let mut charged = model.authorize_current(&record).unwrap();
            charged.cpu_steps = 0;
            (
                SessionResponseKind::PolicyApproved { sha256: *sha256 },
                charged,
            )
        }
        SessionNeedKind::AuthorityBoundary => {
            let record = model.authority_boundary(need.caps.bytes as usize).unwrap();
            let charged = Charged {
                probes: 1,
                rows: 1,
                bytes: record.raw.len() as u64,
                ..Charged::default()
            };
            (SessionResponseKind::AuthorityBoundary(record), charged)
        }
        SessionNeedKind::AcquireDisclosure {
            selected,
            selected_digest,
        } => {
            let lease = model.acquire_disclosure(&need.binding, selected).unwrap();
            (
                SessionResponseKind::Disclosure {
                    selected_digest: *selected_digest,
                    lease,
                },
                Charged::default(),
            )
        }
    };
    SessionResponse {
        schema: SOURCE_DESCEND_SESSION_V1,
        meter_profile: SOURCE_DESCEND_D1_METER_V1,
        nonce: need.nonce,
        binding: need.binding.clone(),
        certified_index_root: Some(need.binding.index_root),
        charged,
        kind,
    }
}

fn synthetic_session(
    model: &mut SyntheticReadModel,
    request: SourceDescendRequest,
    limits: Budget,
) -> Result<Vec<u8>, QueryError> {
    let (mut session, mut need) =
        SourceDescendSession::start(model.binding.clone(), request, limits)?;
    loop {
        let header = parse_json(
            &need.wire_header(16 * 1024)?,
            JsonMode::PublishedStrict,
            JsonLimits::default(),
        )
        .unwrap();
        assert_eq!(string(header.root(), "schema"), SOURCE_DESCEND_SESSION_V1);
        assert_eq!(
            string(header.root(), "meter_profile"),
            SOURCE_DESCEND_D1_METER_V1
        );
        if let SessionNeedKind::AcquireDisclosure { selected, .. } = &need.kind {
            assert_eq!(need.selected_records().unwrap().len(), selected.len());
        }
        let response = synthetic_response(model, &need);
        match session.resume(response)? {
            SessionAdvance::Need(next) => need = next,
            SessionAdvance::Ready(mut ready) => {
                ready.recheck()?;
                return Ok(ready.to_vec());
            }
        }
    }
}

#[test]
fn resumable_session_matches_python_and_sync_oracle_without_graph_prefetch() {
    let (mut model, expected, request) = SyntheticReadModel::fixture();
    let packet = synthetic_session(&mut model, request.clone(), budget()).unwrap();
    let actual = parse_json(&packet, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
    assert!(semantic_eq(actual.root(), &expected));
    let (mut synchronous, _, _) = SyntheticReadModel::fixture();
    let direct = source_descend(&mut synchronous, &request, budget()).unwrap();
    assert_eq!(packet, direct.to_vec());
    assert!(model.pin_checks >= 3);
}

#[test]
fn session_header_preserves_wide_integer_identity_and_refuses_one_over() {
    let (model, _, request) = SyntheticReadModel::fixture();
    let mut binding = model.binding;
    binding.through_commit_seq = 9_007_199_254_740_993;
    let mut limits = budget();
    limits.max_probes = 9_007_199_254_740_993;
    let (_, need) = SourceDescendSession::start(binding, request, limits).unwrap();
    let header = need.wire_header(16 * 1024).unwrap();
    let parsed = parse_json(&header, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
    assert_eq!(
        string(field(parsed.root(), "binding"), "through_commit_seq"),
        "9007199254740993"
    );
    assert_eq!(
        string(field(parsed.root(), "caps"), "probes"),
        "9007199254740993"
    );
    assert_eq!(string(parsed.root(), "nonce"), "1");
    assert_eq!(
        need.wire_header(header.len() - 1).unwrap_err().code,
        QueryErrorCode::BudgetExceeded
    );
}

#[test]
fn non_ascii_edge_ids_follow_python_codepoint_order_across_pages() {
    // CPython source_descend_query with the frozen fixture's first two edge IDs
    // changed to these values emits ["edge.z", "edge.Å"].  JS localeCompare
    // reverses them, while the source-navigation index uses binary key order.
    let (mut model, _, mut request) = SyntheticReadModel::fixture();
    let edges = model.edges.get_mut("α").unwrap();
    for (edge, id) in edges.iter_mut().zip(["edge.z", "edge.Å"]) {
        let mut value = parse_json(&edge.raw, JsonMode::PublishedStrict, JsonLimits::default())
            .unwrap()
            .into_root();
        let JsonValue::Object(entries) = &mut value else {
            panic!("fixture edge is not an object")
        };
        let (_, edge_id) = entries
            .iter_mut()
            .find(|(key, _)| key.as_str() == Some("edge_id"))
            .unwrap();
        *edge_id = JsonValue::String(JsonString::from_utf8(id));
        *edge = raw(&value, &model.binding);
    }
    edges.sort_by_key(SyntheticReadModel::edge_id);
    request.max_depth = 1;
    let mut limits = budget();
    limits.page_rows = 1;
    let packet = synthetic_session(&mut model, request, limits).unwrap();
    let parsed = parse_json(&packet, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
    let ids: Vec<_> = field(parsed.root(), "edges")
        .as_array()
        .unwrap()
        .iter()
        .map(|edge| string(edge, "edge_id"))
        .collect();
    assert_eq!(ids, ["edge.z", "edge.Å"]);
}

#[test]
fn resumable_session_rejects_replayed_nonce_and_wrong_selected_binding() {
    let (mut model, _, request) = SyntheticReadModel::fixture();
    let (mut session, need) =
        SourceDescendSession::start(model.binding.clone(), request, budget()).unwrap();
    let mut reply = synthetic_response(&mut model, &need);
    reply.nonce += 1;
    assert_eq!(
        session.resume(reply).unwrap_err().code,
        QueryErrorCode::StaleSelection
    );
    let (mut session, need) = SourceDescendSession::start(
        model.binding.clone(),
        SyntheticReadModel::fixture().2,
        budget(),
    )
    .unwrap();
    let mut reply = synthetic_response(&mut model, &need);
    reply.binding.index_generation = "other".into();
    assert_eq!(
        session.resume(reply).unwrap_err().code,
        QueryErrorCode::StaleSelection
    );
}

#[test]
fn resumable_session_refuses_incomplete_adjacency_and_over_admission() {
    let (mut model, _, request) = SyntheticReadModel::fixture();
    model.bad_digest = true;
    assert_eq!(
        synthetic_session(&mut model, request.clone(), budget())
            .unwrap_err()
            .code,
        QueryErrorCode::IndexIncomplete
    );
    let (mut model, _, request) = SyntheticReadModel::fixture();
    let (mut session, need) =
        SourceDescendSession::start(model.binding.clone(), request, budget()).unwrap();
    let mut reply = synthetic_response(&mut model, &need);
    reply.charged.probes = need.caps.probes + 1;
    assert_eq!(
        session.resume(reply).unwrap_err().code,
        QueryErrorCode::BudgetExceeded
    );
    let (mut model, _, request) = SyntheticReadModel::fixture();
    let (mut session, mut need) =
        SourceDescendSession::start(model.binding.clone(), request, budget()).unwrap();
    loop {
        let mut reply = synthetic_response(&mut model, &need);
        if matches!(need.kind, SessionNeedKind::Outgoing { .. }) {
            reply.certified_index_root = None;
            assert_eq!(
                session.resume(reply).unwrap_err().code,
                QueryErrorCode::IndexIncomplete
            );
            break;
        }
        need = match session.resume(reply).unwrap() {
            SessionAdvance::Need(next) => next,
            SessionAdvance::Ready(_) => panic!("ready before index-root check"),
        };
    }
}

#[test]
fn resumable_session_requires_owner_lease_and_current_policy() {
    let (mut model, _, request) = SyntheticReadModel::fixture();
    let (mut session, mut need) =
        SourceDescendSession::start(model.binding.clone(), request.clone(), budget()).unwrap();
    loop {
        if let SessionNeedKind::AcquireDisclosure { .. } = &need.kind {
            let wrong = SessionResponse {
                schema: SOURCE_DESCEND_SESSION_V1,
                meter_profile: SOURCE_DESCEND_D1_METER_V1,
                nonce: need.nonce,
                binding: need.binding.clone(),
                certified_index_root: None,
                charged: Charged::default(),
                kind: SessionResponseKind::PolicyApproved {
                    sha256: Digest256::of_bytes(b"wrong"),
                },
            };
            assert_eq!(
                session.resume(wrong).unwrap_err().code,
                QueryErrorCode::InvalidRequest
            );
            break;
        }
        let response = synthetic_response(&mut model, &need);
        need = match session.resume(response).unwrap() {
            SessionAdvance::Need(next) => next,
            SessionAdvance::Ready(_) => panic!("ready before disclosure"),
        };
    }
    let (mut model, _, request) = SyntheticReadModel::fixture();
    model.denied_id = Some("δ".into());
    let (mut session, mut need) =
        SourceDescendSession::start(model.binding.clone(), request, budget()).unwrap();
    loop {
        if let SessionNeedKind::CurrentPolicy { sha256 } = &need.kind {
            let denied = model.nodes.get("δ").unwrap();
            if sha256 == &denied.sha256 {
                let response = SessionResponse {
                    schema: SOURCE_DESCEND_SESSION_V1,
                    meter_profile: SOURCE_DESCEND_D1_METER_V1,
                    nonce: need.nonce,
                    binding: need.binding.clone(),
                    certified_index_root: None,
                    charged: Charged::default(),
                    kind: SessionResponseKind::Refused {
                        code: QueryErrorCode::PolicyDenied,
                    },
                };
                assert_eq!(
                    session.resume(response).unwrap_err().code,
                    QueryErrorCode::PolicyDenied
                );
                break;
            }
        }
        let response = synthetic_response(&mut model, &need);
        need = match session.resume(response).unwrap() {
            SessionAdvance::Need(next) => next,
            SessionAdvance::Ready(_) => panic!("ready before policy refusal"),
        };
    }
}

#[test]
fn source_descend_preserves_retired_worker_bounded_route_control() {
    let (mut model, _, mut request) = SyntheticReadModel::fixture();
    let fixture = parse_json(br#"{"nodes":[{"node_id":"era","node_kind":"era","source_ref":"era.json"},{"node_id":"planting","node_kind":"source_planting","source_ref":"planting.json"},{"node_id":"work","node_kind":"work","source_ref":"work.json"},{"node_id":"expression","node_kind":"expression","source_ref":"expression.json"},{"node_id":"link","node_kind":"link","source_ref":"link.json","properties":{"access_status":"open_download"}}],"edges":[{"edge_id":"e1","from_id":"era","to_id":"planting","predicate_id":"contains","edge_kind":"authored_branch_hierarchy","source_refs":["era.json"]},{"edge_id":"e2","from_id":"planting","to_id":"work","predicate_id":"references_source_witness","edge_kind":"authored_source_planting","source_refs":["planting.json"]},{"edge_id":"e3","from_id":"work","to_id":"expression","predicate_id":"has_expression","edge_kind":"evidence_claim","source_refs":["claims.jsonl"]},{"edge_id":"e4","from_id":"work","to_id":"link","predicate_id":"downloadable_at","edge_kind":"evidence_claim","source_refs":["links.jsonl"]}]}"#, JsonMode::PublishedStrict, JsonLimits::default()).unwrap().into_root();
    model.nodes = field(&fixture, "nodes")
        .as_array()
        .unwrap()
        .iter()
        .map(|v| (string(v, "node_id").to_owned(), raw(v, &model.binding)))
        .collect();
    model.edges.clear();
    for e in field(&fixture, "edges").as_array().unwrap() {
        model
            .edges
            .entry(string(e, "from_id").into())
            .or_default()
            .push(raw(e, &model.binding));
    }
    request.node_id = "era".into();
    request.max_depth = 8;
    request.limit = 3;
    let packet = source_descend(&mut model, &request, budget()).unwrap();
    let packet = parse_json(&packet, JsonMode::PublishedStrict, JsonLimits::default())
        .unwrap()
        .into_root();
    assert_eq!(field(field(&packet, "counts"), "nodes").as_u64(), Some(3));
    assert_eq!(field(field(&packet, "counts"), "edges").as_u64(), Some(2));
    assert_eq!(field(&packet, "truncated"), &JsonValue::Bool(true));
    assert_eq!(
        field(&packet, "nodes")
            .as_array()
            .unwrap()
            .iter()
            .map(|n| string(n, "node_id"))
            .collect::<Vec<_>>(),
        vec!["era", "planting", "work"]
    );
    assert!(model.pin_checks >= 3);
}
