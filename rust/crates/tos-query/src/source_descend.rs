use std::collections::{BTreeMap, VecDeque};

use tos_foundation::{Digest256, Digest256Hasher, JsonLimits, JsonMode, JsonValue, parse_json};

/// All fields identify one immutable, complete published read-model cut.
/// Current policy is deliberately outside this binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Binding {
    pub source_cut: String,
    pub through_commit_seq: u64,
    pub membership_root: Digest256,
    pub projection_root: Digest256,
    pub index_root: Digest256,
    pub index_generation: String,
    pub route_map_version: String,
    pub reader_abi: String,
    pub model_abi: String,
    pub selection_profile: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryErrorCode {
    InvalidRequest,
    UnknownExactId,
    StaleSelection,
    PublicationPending,
    BudgetExceeded,
    Cancelled,
    DeadlineExceeded,
    PolicyDenied,
    IndexIncomplete,
    CorruptSelectedCarrier,
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryError {
    pub code: QueryErrorCode,
    pub message: &'static str,
}

impl QueryError {
    pub(crate) const fn new(code: QueryErrorCode, message: &'static str) -> Self {
        Self { code, message }
    }
}

impl std::fmt::Display for QueryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for QueryError {}

/// Exact emitted carrier bytes, checked against the selected index digest.
#[derive(Clone, Debug)]
pub struct RawRecord {
    pub binding: Binding,
    pub raw: Vec<u8>,
    pub sha256: Digest256,
}

/// Adapter work units: `probes` count bounded backend requests, `rows` count
/// backend result rows (including lookahead/hidden rows), `bytes` count decoded
/// result-column payload (i64=8, TEXT=UTF-8 length, BLOB=byte length), and
/// `cpu_steps` count declared backend instruction/fuel units. File/page I/O,
/// allocator footprint and wall time are separate admission dimensions, never
/// inferred from `bytes` or `cpu_steps`. Returned rows are only a subset.
#[derive(Clone, Copy, Debug, Default)]
pub struct Charged {
    pub probes: u64,
    pub rows: u64,
    pub bytes: u64,
    pub cpu_steps: u64,
}

#[derive(Clone, Debug)]
pub struct ExactNode {
    pub binding: Binding,
    pub record: Option<RawRecord>,
    pub complete_unique_lookup: bool,
    pub charged: Charged,
}

/// The expected count/digest must come from the sealed compiler index root.
/// Pages of one adjacency repeat it. QRY recomputes it over every visible row
/// before it accepts an exhausted scope, detecting omission and reordering.
#[derive(Clone, Debug)]
pub struct AdjacencyPage {
    pub binding: Binding,
    pub from_id: String,
    pub after_edge_id: Option<String>,
    pub edges: Vec<RawRecord>,
    pub exhausted: bool,
    pub expected_count: u64,
    pub expected_digest: Digest256,
    pub charged: Charged,
}

/// Trusted local adapter contract. A remote untrusted host must first prove
/// the same sealed range/count/digest and pin semantics at its boundary.
pub trait ReadModel {
    /// Optional owner/transport cancellation or deadline check. It must not
    /// turn a partially staged packet into success.
    fn check_interrupt(&mut self) -> Result<(), QueryError> {
        Ok(())
    }
    fn selected_binding(&mut self) -> Result<Binding, QueryError>;
    /// `max_bytes` bounds all decoded columns in this call; `max_carrier_bytes`
    /// separately bounds any individual raw carrier before BLOB transfer.
    fn exact_visible_node(
        &mut self,
        id: &str,
        max_bytes: usize,
        max_carrier_bytes: usize,
        max_vm_steps: u64,
        max_work_probes: u64,
        max_work_rows: u64,
    ) -> Result<ExactNode, QueryError>;
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
    ) -> Result<AdjacencyPage, QueryError>;
    /// Source owner current decision. Must be rechecked before disclosure.
    fn authorize_current(&mut self, record: &RawRecord) -> Result<Charged, QueryError>;
    /// Exact JSON value from the selected source-navigation header.
    fn authority_boundary(&mut self, max_bytes: usize) -> Result<RawRecord, QueryError>;
    /// Renew/check the fenced pin and selected immutable publication.
    fn check_pin(&mut self, binding: &Binding) -> Result<(), QueryError>;
    /// Acquire a source-owner hold on this exact cut and current policy for all
    /// selected carriers. The hold must remain live through transport write.
    fn acquire_disclosure(
        &mut self,
        binding: &Binding,
        selected: &[RawRecord],
    ) -> Result<Box<dyn DisclosureLease>, QueryError>;
}

/// An owner hold, not merely a one-time policy observation. Native transports
/// may move it across threads, so native leases must be Send. A WASM host
/// lease stays on its creating JS agent and must not claim Send. Both retain
/// the hold until the final transport flush.
#[cfg(not(target_arch = "wasm32"))]
pub trait DisclosureLease: Send {
    fn recheck(&mut self) -> Result<(), QueryError>;
}

#[cfg(target_arch = "wasm32")]
pub trait DisclosureLease {
    fn recheck(&mut self) -> Result<(), QueryError>;
}

pub struct DisclosableSourceDescend {
    body: Vec<u8>,
    lease: Box<dyn DisclosureLease>,
}

impl std::fmt::Debug for DisclosableSourceDescend {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DisclosableSourceDescend")
            .field("body_bytes", &self.body.len())
            .finish_non_exhaustive()
    }
}

impl std::ops::Deref for DisclosableSourceDescend {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        &self.body
    }
}

/// Versioned, one-step host protocol. An adapter may encode the header as JSON
/// and carry RawRecord.raw in separate UTF-8 byte attachments. Only the exact
/// selected publication owner may issue the Binding and disclosure lease.
pub const SOURCE_DESCEND_SESSION_V1: &str = "tos_source_descend_session_v1";
/// D1's queries/rows_read/transfer-byte meter is not a SQLite VM-step meter.
pub const SOURCE_DESCEND_D1_METER_V1: &str = "tos_d1_indexed_rows_v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionCaps {
    pub probes: u64,
    pub rows: u64,
    pub bytes: u64,
    pub cpu_steps: u64,
    pub carrier_bytes: usize,
    pub page_rows: usize,
}

#[derive(Clone, Debug)]
pub enum SessionNeedKind {
    CheckPin,
    ExactNode {
        id: String,
    },
    Outgoing {
        from_id: String,
        after_edge_id: Option<String>,
    },
    CurrentPolicy {
        sha256: Digest256,
    },
    AuthorityBoundary,
    AcquireDisclosure {
        selected: Vec<RawRecord>,
        selected_digest: Digest256,
    },
}

#[derive(Clone, Debug)]
pub struct SessionNeed {
    pub schema: &'static str,
    pub meter_profile: &'static str,
    pub nonce: u64,
    pub binding: Binding,
    pub caps: SessionCaps,
    pub kind: SessionNeedKind,
}

impl SessionNeed {
    /// UTF-8 JSON control header for a WASM/async adapter. Exact carrier
    /// bytes in an AcquireDisclosure need are separate raw attachments from
    /// `selected_records`; the header alone never grants disclosure.
    pub fn wire_header(&self, max_header_bytes: usize) -> Result<Vec<u8>, QueryError> {
        fn quoted(value: &str) -> String {
            String::from_utf8(json_string(value)).expect("JSON string is UTF-8")
        }
        let b = &self.binding;
        let variable_bytes = [
            b.source_cut.as_str(),
            b.index_generation.as_str(),
            b.route_map_version.as_str(),
            b.reader_abi.as_str(),
            b.model_abi.as_str(),
            b.selection_profile.as_str(),
        ]
        .iter()
        .try_fold(0usize, |sum, value| sum.checked_add(value.len()))
        .ok_or_else(|| QueryError::new(QueryErrorCode::BudgetExceeded, "header size overflow"))?;
        let variable_bytes = match &self.kind {
            SessionNeedKind::ExactNode { id } => variable_bytes.checked_add(id.len()),
            SessionNeedKind::Outgoing {
                from_id,
                after_edge_id,
            } => variable_bytes
                .checked_add(from_id.len())
                .and_then(|size| size.checked_add(after_edge_id.as_ref().map_or(0, String::len))),
            _ => Some(variable_bytes),
        };
        if variable_bytes.is_none_or(|size| size > max_header_bytes) {
            return Err(QueryError::new(
                QueryErrorCode::BudgetExceeded,
                "source descent header exceeds admitted bytes",
            ));
        }
        let binding = format!(
            "{{\"source_cut\":{},\"through_commit_seq\":\"{}\",\"membership_root\":{},\"projection_root\":{},\"index_root\":{},\"index_generation\":{},\"route_map_version\":{},\"reader_abi\":{},\"model_abi\":{},\"selection_profile\":{}}}",
            quoted(&b.source_cut),
            b.through_commit_seq,
            quoted(&b.membership_root.to_hex()),
            quoted(&b.projection_root.to_hex()),
            quoted(&b.index_root.to_hex()),
            quoted(&b.index_generation),
            quoted(&b.route_map_version),
            quoted(&b.reader_abi),
            quoted(&b.model_abi),
            quoted(&b.selection_profile)
        );
        let operation = match &self.kind {
            SessionNeedKind::CheckPin => "\"check_pin\"".to_owned(),
            SessionNeedKind::ExactNode { id } => format!("{{\"exact_node\":{}}}", quoted(id)),
            SessionNeedKind::Outgoing {
                from_id,
                after_edge_id,
            } => format!(
                "{{\"outgoing\":{{\"from_id\":{},\"after_edge_id\":{}}}}}",
                quoted(from_id),
                after_edge_id
                    .as_ref()
                    .map_or("null".to_owned(), |id| quoted(id))
            ),
            SessionNeedKind::CurrentPolicy { sha256 } => {
                format!("{{\"current_policy\":{}}}", quoted(&sha256.to_hex()))
            }
            SessionNeedKind::AuthorityBoundary => "\"authority_boundary\"".to_owned(),
            SessionNeedKind::AcquireDisclosure {
                selected,
                selected_digest,
            } => format!(
                "{{\"acquire_disclosure\":{{\"selected_count\":\"{}\",\"selected_digest\":{}}}}}",
                selected.len(),
                quoted(&selected_digest.to_hex())
            ),
        };
        let header = format!(
            "{{\"schema\":{},\"meter_profile\":{},\"nonce\":\"{}\",\"binding\":{},\"caps\":{{\"probes\":\"{}\",\"rows\":\"{}\",\"bytes\":\"{}\",\"cpu_steps\":\"{}\",\"carrier_bytes\":\"{}\",\"page_rows\":\"{}\"}},\"operation\":{}}}",
            quoted(self.schema), quoted(self.meter_profile), self.nonce, binding,
            self.caps.probes, self.caps.rows, self.caps.bytes, self.caps.cpu_steps,
            self.caps.carrier_bytes, self.caps.page_rows, operation
        ).into_bytes();
        if header.len() > max_header_bytes {
            return Err(QueryError::new(
                QueryErrorCode::BudgetExceeded,
                "source descent header exceeds admitted bytes",
            ));
        }
        Ok(header)
    }

    pub fn selected_records(&self) -> Option<&[RawRecord]> {
        match &self.kind {
            SessionNeedKind::AcquireDisclosure { selected, .. } => Some(selected),
            _ => None,
        }
    }
}

pub enum SessionResponseKind {
    Refused {
        code: QueryErrorCode,
    },
    PinHeld,
    ExactNode(ExactNode),
    Outgoing(AdjacencyPage),
    PolicyApproved {
        sha256: Digest256,
    },
    AuthorityBoundary(RawRecord),
    Disclosure {
        selected_digest: Digest256,
        lease: Box<dyn DisclosureLease>,
    },
}

pub struct SessionResponse {
    pub schema: &'static str,
    pub meter_profile: &'static str,
    pub nonce: u64,
    pub binding: Binding,
    /// For exact/adjacency reads, this must be certified against the selected
    /// compiler index root. A D1 table-local count/digest is insufficient.
    pub certified_index_root: Option<Digest256>,
    /// Actual host work, including hidden/lookahead rows and transferred
    /// columns. The host must meter it independently of QRY's local work.
    pub charged: Charged,
    pub kind: SessionResponseKind,
}

#[derive(Debug)]
pub enum SessionAdvance {
    Need(SessionNeed),
    Ready(DisclosableSourceDescend),
}

struct SessionScope {
    from_id: String,
    depth: u8,
    after: Option<String>,
    expected: Option<(u64, Digest256)>,
    count: u64,
    digest: Digest256Hasher,
    page: VecDeque<RawRecord>,
    page_exhausted: bool,
}

#[derive(Clone, Copy)]
enum SessionPhase {
    InitialPin,
    Root,
    RootPolicy,
    Advance,
    PagePin,
    Page,
    EdgePolicy,
    Target,
    TargetPolicy,
    RecheckPolicy,
    Authority,
    AuthorityPolicy,
    FinalPin,
    PostBuildPin,
    Disclosure,
    Finished,
}

/// Bounded state machine for an async/indexed host. It never asks the host to
/// materialize the graph, and it never releases a partial packet. A failed
/// resume consumes the session. The selected binding must have been obtained
/// from an owner-selected immutable cut, independently of visible table rows.
pub struct SourceDescendSession {
    binding: Binding,
    request: SourceDescendRequest,
    budget: Budget,
    work: Work,
    nonce: u64,
    pending: Option<SessionNeed>,
    phase: SessionPhase,
    observed: Vec<RawRecord>,
    nodes: BTreeMap<String, (u8, RawRecord)>,
    frontier: VecDeque<(String, u8)>,
    edges: Vec<RawRecord>,
    scope: Option<SessionScope>,
    edge_pending: Option<(RawRecord, String)>,
    policy_pending: Option<RawRecord>,
    recheck: VecDeque<RawRecord>,
    authority: Option<RawRecord>,
    body: Option<Vec<u8>>,
    truncated: bool,
}

impl SourceDescendSession {
    pub fn start(
        binding: Binding,
        request: SourceDescendRequest,
        budget: Budget,
    ) -> Result<(Self, SessionNeed), QueryError> {
        request.validate()?;
        if !budget.valid() || request.node_id.len() > budget.max_request_bytes {
            return Err(QueryError::new(
                QueryErrorCode::InvalidRequest,
                "invalid source descent session budget or request",
            ));
        }
        if request
            .at_least_commit_seq
            .is_some_and(|minimum| binding.through_commit_seq < minimum)
        {
            return Err(QueryError::new(
                QueryErrorCode::PublicationPending,
                "selected read model has not published the requested commit",
            ));
        }
        if binding.model_abi != "tos_source_navigation_read_model_v1"
            || binding.selection_profile != "tos_source_navigation_visible_v1"
        {
            return Err(QueryError::new(
                QueryErrorCode::Unavailable,
                "source navigation visibility profile unsupported",
            ));
        }
        let mut session = Self {
            binding,
            request,
            budget,
            work: Work::default(),
            nonce: 0,
            pending: None,
            phase: SessionPhase::InitialPin,
            observed: Vec::new(),
            nodes: BTreeMap::new(),
            frontier: VecDeque::new(),
            edges: Vec::new(),
            scope: None,
            edge_pending: None,
            policy_pending: None,
            recheck: VecDeque::new(),
            authority: None,
            body: None,
            truncated: false,
        };
        let need = session.next_need()?;
        Ok((session, need))
    }

    pub fn selected_binding(&self) -> &Binding {
        &self.binding
    }

    fn caps(&self) -> Result<SessionCaps, QueryError> {
        let caps = SessionCaps {
            probes: self.budget.max_probes.saturating_sub(self.work.probes),
            rows: self.budget.max_rows.saturating_sub(self.work.rows),
            bytes: self.budget.max_bytes.saturating_sub(self.work.bytes),
            cpu_steps: self
                .budget
                .max_cpu_steps
                .saturating_sub(self.work.cpu_steps),
            carrier_bytes: self.budget.json.max_bytes,
            page_rows: self.budget.page_rows.min(
                self.budget
                    .max_rows
                    .saturating_sub(self.work.rows)
                    .min(usize::MAX as u64) as usize,
            ),
        };
        if caps.probes == 0 || caps.rows == 0 || caps.bytes == 0 || caps.cpu_steps == 0 {
            return Err(QueryError::new(
                QueryErrorCode::BudgetExceeded,
                "source descent host admission exhausted",
            ));
        }
        Ok(caps)
    }

    fn next_need(&mut self) -> Result<SessionNeed, QueryError> {
        let caps = self.caps()?;
        let kind = match self.phase {
            SessionPhase::InitialPin
            | SessionPhase::PagePin
            | SessionPhase::FinalPin
            | SessionPhase::PostBuildPin => SessionNeedKind::CheckPin,
            SessionPhase::Root => SessionNeedKind::ExactNode {
                id: self.request.node_id.clone(),
            },
            SessionPhase::Target => SessionNeedKind::ExactNode {
                id: self.edge_pending.as_ref().expect("target").1.clone(),
            },
            SessionPhase::Page => {
                let scope = self.scope.as_ref().expect("scope");
                SessionNeedKind::Outgoing {
                    from_id: scope.from_id.clone(),
                    after_edge_id: scope.after.clone(),
                }
            }
            SessionPhase::RootPolicy
            | SessionPhase::EdgePolicy
            | SessionPhase::TargetPolicy
            | SessionPhase::RecheckPolicy
            | SessionPhase::AuthorityPolicy => SessionNeedKind::CurrentPolicy {
                sha256: self.policy_pending.as_ref().expect("policy carrier").sha256,
            },
            SessionPhase::Authority => SessionNeedKind::AuthorityBoundary,
            SessionPhase::Disclosure => SessionNeedKind::AcquireDisclosure {
                selected: self.observed.clone(),
                selected_digest: selected_digest(&self.observed),
            },
            SessionPhase::Advance | SessionPhase::Finished => {
                unreachable!("internal phase cannot issue need")
            }
        };
        self.nonce = self.nonce.checked_add(1).ok_or_else(|| {
            QueryError::new(
                QueryErrorCode::BudgetExceeded,
                "source descent nonce exhausted",
            )
        })?;
        let need = SessionNeed {
            schema: SOURCE_DESCEND_SESSION_V1,
            meter_profile: SOURCE_DESCEND_D1_METER_V1,
            nonce: self.nonce,
            binding: self.binding.clone(),
            caps,
            kind,
        };
        self.pending = Some(need.clone());
        Ok(need)
    }

    /// A mismatched response is terminal. Hosts must echo the nonce, binding,
    /// operation and caps from the current Need, not a prior or future request.
    pub fn resume(&mut self, response: SessionResponse) -> Result<SessionAdvance, QueryError> {
        let need = self.pending.take().ok_or_else(|| {
            QueryError::new(
                QueryErrorCode::InvalidRequest,
                "source descent session has no outstanding need",
            )
        })?;
        if response.schema != SOURCE_DESCEND_SESSION_V1
            || response.meter_profile != SOURCE_DESCEND_D1_METER_V1
            || response.nonce != need.nonce
            || response.binding != self.binding
        {
            self.phase = SessionPhase::Finished;
            return Err(QueryError::new(
                QueryErrorCode::StaleSelection,
                "source descent response nonce or binding differs",
            ));
        }
        if response.charged.probes > need.caps.probes
            || response.charged.rows > need.caps.rows
            || response.charged.bytes > need.caps.bytes
            || response.charged.cpu_steps != 0
        {
            self.phase = SessionPhase::Finished;
            return Err(QueryError::new(
                QueryErrorCode::BudgetExceeded,
                "host charged beyond admitted operation",
            ));
        }
        let outcome = self.accept(&need, response);
        if outcome.is_err() {
            self.phase = SessionPhase::Finished;
        }
        outcome
    }

    fn accept(
        &mut self,
        need: &SessionNeed,
        response: SessionResponse,
    ) -> Result<SessionAdvance, QueryError> {
        let charged = response.charged;
        self.work.charge(charged, self.budget)?;
        match (self.phase, &need.kind, response.kind) {
            (_, _, SessionResponseKind::Refused { code }) => {
                return Err(QueryError::new(
                    code,
                    "source descent host refused operation",
                ));
            }
            (SessionPhase::InitialPin, SessionNeedKind::CheckPin, SessionResponseKind::PinHeld) => {
                self.phase = SessionPhase::Root
            }
            (SessionPhase::PagePin, SessionNeedKind::CheckPin, SessionResponseKind::PinHeld) => {
                self.phase = SessionPhase::Page
            }
            (SessionPhase::FinalPin, SessionNeedKind::CheckPin, SessionResponseKind::PinHeld) => {
                self.build_body()?;
                self.phase = SessionPhase::PostBuildPin;
            }
            (
                SessionPhase::PostBuildPin,
                SessionNeedKind::CheckPin,
                SessionResponseKind::PinHeld,
            ) => {
                self.phase = SessionPhase::Disclosure;
            }
            (
                SessionPhase::Root,
                SessionNeedKind::ExactNode { id },
                SessionResponseKind::ExactNode(got),
            )
            | (
                SessionPhase::Target,
                SessionNeedKind::ExactNode { id },
                SessionResponseKind::ExactNode(got),
            ) => {
                if response.certified_index_root != Some(self.binding.index_root) {
                    return Err(QueryError::new(
                        QueryErrorCode::IndexIncomplete,
                        "exact node lacks selected index-root certificate",
                    ));
                }
                if got.charged.probes != charged.probes
                    || got.charged.rows != charged.rows
                    || got.charged.bytes != charged.bytes
                    || got.charged.cpu_steps != charged.cpu_steps
                {
                    return Err(QueryError::new(
                        QueryErrorCode::InvalidRequest,
                        "exact node charge differs from response",
                    ));
                }
                if charged.probes == 0
                    || charged.rows < u64::from(got.record.is_some())
                    || charged.bytes
                        < got
                            .record
                            .as_ref()
                            .map_or(0, |record| record.raw.len() as u64)
                {
                    return Err(QueryError::new(
                        QueryErrorCode::IndexIncomplete,
                        "exact node host meter omits selected row",
                    ));
                }
                same_binding(&got.binding, &self.binding)?;
                if !got.complete_unique_lookup {
                    return Err(QueryError::new(
                        QueryErrorCode::IndexIncomplete,
                        "exact visible node index is incomplete",
                    ));
                }
                let Some(record) = got.record else {
                    if matches!(self.phase, SessionPhase::Root) {
                        return Err(QueryError::new(
                            QueryErrorCode::UnknownExactId,
                            "unknown source-navigation node",
                        ));
                    }
                    self.finish_edge_target(None)?;
                    return self.advance();
                };
                if record.raw.len() as u64 > need.caps.bytes
                    || record.raw.len() > need.caps.carrier_bytes
                {
                    return Err(QueryError::new(
                        QueryErrorCode::BudgetExceeded,
                        "selected node exceeds admitted bytes",
                    ));
                }
                self.work.charge(
                    Charged {
                        bytes: record.raw.len() as u64,
                        cpu_steps: 1,
                        ..Charged::default()
                    },
                    self.budget,
                )?;
                let value = parse_record(&record, &self.binding, self.budget)?;
                if value.object_get("node_id").and_then(JsonValue::as_str) != Some(id)
                    || value.object_get("depth").is_some()
                {
                    return Err(QueryError::new(
                        QueryErrorCode::CorruptSelectedCarrier,
                        "node identity or reserved depth field invalid",
                    ));
                }
                self.policy_pending = Some(record);
                self.phase = if matches!(self.phase, SessionPhase::Root) {
                    SessionPhase::RootPolicy
                } else {
                    SessionPhase::TargetPolicy
                };
            }
            (
                SessionPhase::Page,
                SessionNeedKind::Outgoing {
                    from_id,
                    after_edge_id,
                },
                SessionResponseKind::Outgoing(page),
            ) => {
                if response.certified_index_root != Some(self.binding.index_root) {
                    return Err(QueryError::new(
                        QueryErrorCode::IndexIncomplete,
                        "outgoing scope lacks selected index-root certificate",
                    ));
                }
                if page.charged.probes != charged.probes
                    || page.charged.rows != charged.rows
                    || page.charged.bytes != charged.bytes
                    || page.charged.cpu_steps != charged.cpu_steps
                {
                    return Err(QueryError::new(
                        QueryErrorCode::InvalidRequest,
                        "adjacency charge differs from response",
                    ));
                }
                let delivered_raw = page
                    .edges
                    .iter()
                    .try_fold(0u64, |sum, edge| sum.checked_add(edge.raw.len() as u64))
                    .ok_or_else(|| {
                        QueryError::new(
                            QueryErrorCode::BudgetExceeded,
                            "adjacency page byte count overflow",
                        )
                    })?;
                if charged.probes == 0
                    || charged.rows < page.edges.len() as u64
                    || charged.bytes < delivered_raw
                {
                    return Err(QueryError::new(
                        QueryErrorCode::IndexIncomplete,
                        "outgoing host meter omits selected rows",
                    ));
                }
                same_binding(&page.binding, &self.binding)?;
                let scope = self.scope.as_mut().expect("scope");
                if page.from_id != *from_id
                    || page.after_edge_id != *after_edge_id
                    || page.edges.len() > need.caps.page_rows
                    || page.edges.is_empty() && !page.exhausted
                {
                    return Err(QueryError::new(
                        QueryErrorCode::IndexIncomplete,
                        "adjacency page scope, size or progress invalid",
                    ));
                }
                let certificate = (page.expected_count, page.expected_digest);
                if scope.expected.is_some_and(|prior| prior != certificate) {
                    return Err(QueryError::new(
                        QueryErrorCode::IndexIncomplete,
                        "adjacency certificate changed across pages",
                    ));
                }
                scope.expected = Some(certificate);
                let page_bytes = page
                    .edges
                    .iter()
                    .try_fold(0u64, |sum, edge| sum.checked_add(edge.raw.len() as u64))
                    .ok_or_else(|| {
                        QueryError::new(
                            QueryErrorCode::BudgetExceeded,
                            "adjacency page bytes overflow",
                        )
                    })?;
                if page_bytes > need.caps.bytes
                    || page
                        .edges
                        .iter()
                        .any(|edge| edge.raw.len() > need.caps.carrier_bytes)
                {
                    return Err(QueryError::new(
                        QueryErrorCode::BudgetExceeded,
                        "adjacency page exceeds admitted bytes",
                    ));
                }
                self.work.charge(
                    Charged {
                        bytes: page_bytes,
                        cpu_steps: page.edges.len() as u64,
                        ..Charged::default()
                    },
                    self.budget,
                )?;
                scope.page = page.edges.into();
                scope.page_exhausted = page.exhausted;
                self.phase = SessionPhase::Advance;
            }
            (
                SessionPhase::RootPolicy
                | SessionPhase::EdgePolicy
                | SessionPhase::TargetPolicy
                | SessionPhase::RecheckPolicy
                | SessionPhase::AuthorityPolicy,
                SessionNeedKind::CurrentPolicy { sha256 },
                SessionResponseKind::PolicyApproved { sha256: approved },
            ) if sha256 == &approved => {
                let record = self.policy_pending.take().expect("policy carrier");
                match self.phase {
                    SessionPhase::RootPolicy => {
                        self.observed.push(record.clone());
                        self.nodes.insert(self.request.node_id.clone(), (0, record));
                        self.frontier.push_back((self.request.node_id.clone(), 0));
                        self.phase = SessionPhase::Advance;
                    }
                    SessionPhase::EdgePolicy => {
                        self.observed.push(record);
                        let target = self.edge_pending.as_ref().expect("edge target").1.clone();
                        if let Some((_, existing)) = self.nodes.get(&target) {
                            self.finish_edge_target(Some(existing.clone()))?;
                            self.phase = SessionPhase::Advance;
                        } else {
                            self.phase = SessionPhase::Target;
                        }
                    }
                    SessionPhase::TargetPolicy => {
                        self.finish_edge_target(Some(record))?;
                        self.phase = SessionPhase::Advance;
                    }
                    SessionPhase::RecheckPolicy => {
                        if let Some(next) = self.recheck.pop_front() {
                            self.policy_pending = Some(next);
                        } else {
                            self.phase = SessionPhase::Authority;
                        }
                    }
                    SessionPhase::AuthorityPolicy => {
                        self.authority = Some(record);
                        self.phase = SessionPhase::FinalPin;
                    }
                    _ => unreachable!(),
                }
            }
            (
                SessionPhase::Authority,
                SessionNeedKind::AuthorityBoundary,
                SessionResponseKind::AuthorityBoundary(record),
            ) => {
                if charged.probes == 0
                    || charged.rows == 0
                    || charged.bytes < record.raw.len() as u64
                {
                    return Err(QueryError::new(
                        QueryErrorCode::IndexIncomplete,
                        "authority host meter omits selected row",
                    ));
                }
                if record.raw.len() as u64 > need.caps.bytes
                    || record.raw.len() > need.caps.carrier_bytes
                {
                    return Err(QueryError::new(
                        QueryErrorCode::BudgetExceeded,
                        "authority boundary exceeds admitted bytes",
                    ));
                }
                self.work.charge(
                    Charged {
                        bytes: record.raw.len() as u64,
                        cpu_steps: 1,
                        ..Charged::default()
                    },
                    self.budget,
                )?;
                let value = parse_record(&record, &self.binding, self.budget)?;
                if value.as_str().is_none_or(str::is_empty) {
                    return Err(QueryError::new(
                        QueryErrorCode::CorruptSelectedCarrier,
                        "source-navigation authority boundary must be a nonempty string",
                    ));
                }
                self.observed.push(record.clone());
                self.policy_pending = Some(record);
                self.phase = SessionPhase::AuthorityPolicy;
            }
            (
                SessionPhase::Disclosure,
                SessionNeedKind::AcquireDisclosure {
                    selected_digest: expected,
                    ..
                },
                SessionResponseKind::Disclosure {
                    selected_digest: actual,
                    mut lease,
                },
            ) if *expected == actual => {
                lease.recheck()?;
                self.phase = SessionPhase::Finished;
                return Ok(SessionAdvance::Ready(DisclosableSourceDescend {
                    body: self.body.take().expect("staged body"),
                    lease,
                }));
            }
            _ => {
                return Err(QueryError::new(
                    QueryErrorCode::InvalidRequest,
                    "source descent host response does not match outstanding need",
                ));
            }
        }
        self.advance()
    }

    fn finish_edge_target(&mut self, target_record: Option<RawRecord>) -> Result<(), QueryError> {
        let (edge, target) = self.edge_pending.take().expect("edge pending");
        let Some(record) = target_record else {
            return Ok(());
        };
        self.observed.push(record.clone());
        if !self.nodes.contains_key(&target) && self.nodes.len() >= self.request.limit {
            self.truncated = true;
            return Ok(());
        }
        if self.edges.len() >= self.budget.max_edges {
            return Err(QueryError::new(
                QueryErrorCode::BudgetExceeded,
                "source descent edge budget exceeded",
            ));
        }
        self.edges.push(edge);
        if let std::collections::btree_map::Entry::Vacant(slot) = self.nodes.entry(target.clone()) {
            let depth = self.scope.as_ref().expect("scope").depth + 1;
            slot.insert((depth, record));
            self.frontier.push_back((target, depth));
        }
        Ok(())
    }

    fn advance(&mut self) -> Result<SessionAdvance, QueryError> {
        loop {
            if !matches!(self.phase, SessionPhase::Advance) {
                return Ok(SessionAdvance::Need(self.next_need()?));
            }
            if self.scope.is_none() {
                let Some((from_id, depth)) = self.frontier.pop_front() else {
                    self.recheck = self
                        .nodes
                        .values()
                        .map(|(_, record)| record.clone())
                        .chain(self.edges.iter().cloned())
                        .collect();
                    if let Some(next) = self.recheck.pop_front() {
                        self.policy_pending = Some(next);
                        self.phase = SessionPhase::RecheckPolicy;
                    } else {
                        self.phase = SessionPhase::Authority;
                    }
                    continue;
                };
                self.work.step(self.budget)?;
                if depth >= self.request.max_depth {
                    continue;
                }
                self.scope = Some(SessionScope {
                    from_id,
                    depth,
                    after: None,
                    expected: None,
                    count: 0,
                    digest: Digest256Hasher::new(),
                    page: VecDeque::new(),
                    page_exhausted: false,
                });
                self.phase = SessionPhase::PagePin;
                continue;
            }
            let scope = self.scope.as_mut().expect("scope");
            if let Some(edge) = scope.page.pop_front() {
                self.work.step(self.budget)?;
                let value = parse_record(&edge, &self.binding, self.budget)?;
                let edge_id = value
                    .object_get("edge_id")
                    .and_then(JsonValue::as_str)
                    .ok_or_else(|| {
                        QueryError::new(
                            QueryErrorCode::CorruptSelectedCarrier,
                            "edge identity missing",
                        )
                    })?;
                if edge_id.is_empty()
                    || scope.after.as_deref().is_some_and(|last| edge_id <= last)
                    || value.object_get("from_id").and_then(JsonValue::as_str)
                        != Some(scope.from_id.as_str())
                {
                    return Err(QueryError::new(
                        QueryErrorCode::IndexIncomplete,
                        "adjacency key order or source invalid",
                    ));
                }
                let target = value
                    .object_get("to_id")
                    .and_then(JsonValue::as_str)
                    .ok_or_else(|| {
                        QueryError::new(
                            QueryErrorCode::CorruptSelectedCarrier,
                            "edge target missing",
                        )
                    })?
                    .to_owned();
                scope.count = scope.count.checked_add(1).ok_or_else(|| {
                    QueryError::new(QueryErrorCode::BudgetExceeded, "adjacency count overflow")
                })?;
                scope.digest.update(&(edge_id.len() as u64).to_be_bytes());
                scope.digest.update(edge_id.as_bytes());
                scope.digest.update(edge.sha256.as_bytes());
                scope.after = Some(edge_id.to_owned());
                self.policy_pending = Some(edge.clone());
                self.edge_pending = Some((edge, target));
                self.phase = SessionPhase::EdgePolicy;
                continue;
            }
            if scope.page_exhausted {
                let scope = self.scope.take().expect("scope");
                let Some((expected_count, expected_digest)) = scope.expected else {
                    return Err(QueryError::new(
                        QueryErrorCode::IndexIncomplete,
                        "adjacency certificate missing",
                    ));
                };
                if scope.count != expected_count || scope.digest.finalize() != expected_digest {
                    return Err(QueryError::new(
                        QueryErrorCode::IndexIncomplete,
                        "sealed adjacency count or digest differs",
                    ));
                }
            } else {
                self.phase = SessionPhase::PagePin;
            }
        }
    }

    fn build_body(&mut self) -> Result<(), QueryError> {
        let authority = self.authority.as_ref().expect("authority");
        let mut ordered_nodes: Vec<_> = self.nodes.iter().collect();
        ordered_nodes.sort_by(|left, right| (left.1.0, left.0).cmp(&(right.1.0, right.0)));
        let mut packet = Vec::new();
        push(
            &mut packet,
            b"{\"schema\":\"tos_source_descent_v1\",\"root_id\":",
            self.budget,
        )?;
        push(
            &mut packet,
            &json_string(&self.request.node_id),
            self.budget,
        )?;
        push(&mut packet, format!(",\"max_depth\":{},\"limit\":{},\"truncated\":{},\"counts\":{{\"nodes\":{},\"edges\":{}}},\"nodes\":[",
            self.request.max_depth, self.request.limit, self.truncated, ordered_nodes.len(), self.edges.len()).as_bytes(), self.budget)?;
        for (index, (_, (depth, record))) in ordered_nodes.iter().enumerate() {
            if index > 0 {
                push(&mut packet, b",", self.budget)?;
            }
            push(&mut packet, &node_with_depth(record, *depth)?, self.budget)?;
        }
        push(&mut packet, b"],\"edges\":[", self.budget)?;
        for (index, edge) in self.edges.iter().enumerate() {
            if index > 0 {
                push(&mut packet, b",", self.budget)?;
            }
            push(&mut packet, edge.raw.trim_ascii(), self.budget)?;
        }
        push(&mut packet, b"],\"authority_note\":", self.budget)?;
        push(&mut packet, authority.raw.trim_ascii(), self.budget)?;
        push(&mut packet, b"}", self.budget)?;
        self.body = Some(packet);
        Ok(())
    }
}

fn selected_digest(records: &[RawRecord]) -> Digest256 {
    let mut digest = Digest256Hasher::new();
    for record in records {
        digest.update(&(record.raw.len() as u64).to_be_bytes());
        digest.update(record.sha256.as_bytes());
    }
    digest.finalize()
}

impl DisclosableSourceDescend {
    pub fn recheck(&mut self) -> Result<(), QueryError> {
        self.lease.recheck()
    }
    pub fn into_parts(self) -> (Vec<u8>, Box<dyn DisclosureLease>) {
        (self.body, self.lease)
    }
}

struct PreparedSourceDescend {
    body: Vec<u8>,
    binding: Binding,
    selected: Vec<RawRecord>,
}

#[derive(Clone, Copy, Debug)]
pub struct Budget {
    pub max_probes: u64,
    pub max_rows: u64,
    pub max_bytes: u64,
    pub max_cpu_steps: u64,
    pub max_edges: usize,
    pub max_response_bytes: usize,
    pub max_request_bytes: usize,
    pub page_rows: usize,
    pub json: JsonLimits,
}

impl Budget {
    fn valid(self) -> bool {
        self.max_probes > 0
            && self.max_rows > 0
            && self.max_bytes > 0
            && self.max_cpu_steps > 0
            && self.max_edges > 0
            && self.max_response_bytes > 0
            && self.max_request_bytes > 0
            && (1..=1024).contains(&self.page_rows)
    }
}

#[derive(Clone, Debug)]
pub struct SourceDescendRequest {
    pub node_id: String,
    pub max_depth: u8,
    pub limit: usize,
    /// Internal session lower bound from a durable command receipt. Transport
    /// grammar remains owned by the public API adapter.
    pub at_least_commit_seq: Option<u64>,
}

impl SourceDescendRequest {
    fn validate(&self) -> Result<(), QueryError> {
        if self.node_id.is_empty()
            || !(1..=8).contains(&self.max_depth)
            || !(1..=300).contains(&self.limit)
        {
            return Err(QueryError::new(
                QueryErrorCode::InvalidRequest,
                "invalid source descent request",
            ));
        }
        Ok(())
    }
}

#[derive(Default)]
struct Work {
    probes: u64,
    rows: u64,
    bytes: u64,
    cpu_steps: u64,
}

impl Work {
    fn charge(&mut self, cost: Charged, budget: Budget) -> Result<(), QueryError> {
        self.probes = self.probes.checked_add(cost.probes).ok_or_else(|| {
            QueryError::new(
                QueryErrorCode::BudgetExceeded,
                "source descent probe count overflow",
            )
        })?;
        self.rows = self.rows.checked_add(cost.rows).ok_or_else(|| {
            QueryError::new(
                QueryErrorCode::BudgetExceeded,
                "source descent row count overflow",
            )
        })?;
        self.bytes = self.bytes.checked_add(cost.bytes).ok_or_else(|| {
            QueryError::new(
                QueryErrorCode::BudgetExceeded,
                "source descent byte count overflow",
            )
        })?;
        self.cpu_steps = self.cpu_steps.checked_add(cost.cpu_steps).ok_or_else(|| {
            QueryError::new(
                QueryErrorCode::BudgetExceeded,
                "source descent CPU count overflow",
            )
        })?;
        if self.probes > budget.max_probes
            || self.rows > budget.max_rows
            || self.bytes > budget.max_bytes
            || self.cpu_steps > budget.max_cpu_steps
        {
            return Err(QueryError::new(
                QueryErrorCode::BudgetExceeded,
                "source descent work budget exceeded",
            ));
        }
        Ok(())
    }

    fn step(&mut self, budget: Budget) -> Result<(), QueryError> {
        self.charge(
            Charged {
                cpu_steps: 1,
                ..Charged::default()
            },
            budget,
        )
    }
}

fn same_binding(actual: &Binding, expected: &Binding) -> Result<(), QueryError> {
    if actual != expected {
        return Err(QueryError::new(
            QueryErrorCode::StaleSelection,
            "read model cut changed during query",
        ));
    }
    Ok(())
}

fn parse_record(
    record: &RawRecord,
    binding: &Binding,
    budget: Budget,
) -> Result<JsonValue, QueryError> {
    same_binding(&record.binding, binding)?;
    if record.raw.len() > budget.json.max_bytes || Digest256::of_bytes(&record.raw) != record.sha256
    {
        return Err(QueryError::new(
            QueryErrorCode::CorruptSelectedCarrier,
            "selected carrier digest or size invalid",
        ));
    }
    parse_json(&record.raw, JsonMode::PublishedStrict, budget.json)
        .map(|document| document.into_root())
        .map_err(|_| {
            QueryError::new(
                QueryErrorCode::CorruptSelectedCarrier,
                "selected carrier is invalid published JSON",
            )
        })
}

fn exact_node<M: ReadModel>(
    model: &mut M,
    id: &str,
    binding: &Binding,
    budget: Budget,
    work: &mut Work,
) -> Result<Option<(RawRecord, JsonValue)>, QueryError> {
    let remaining = budget.max_bytes.saturating_sub(work.bytes);
    let max_bytes = remaining.min(usize::MAX as u64) as usize;
    if max_bytes == 0 {
        return Err(QueryError::new(
            QueryErrorCode::BudgetExceeded,
            "source descent read budget exhausted",
        ));
    }
    let max_vm_steps = budget.max_cpu_steps.saturating_sub(work.cpu_steps);
    if max_vm_steps == 0 {
        return Err(QueryError::new(
            QueryErrorCode::BudgetExceeded,
            "source descent VM budget exhausted",
        ));
    }
    let got = model.exact_visible_node(
        id,
        max_bytes,
        budget.json.max_bytes,
        max_vm_steps,
        budget.max_probes.saturating_sub(work.probes),
        budget.max_rows.saturating_sub(work.rows),
    )?;
    work.charge(got.charged, budget)?;
    work.charge(
        Charged {
            probes: 0,
            rows: 0,
            bytes: got
                .record
                .as_ref()
                .map_or(0, |record| record.raw.len() as u64),
            cpu_steps: 1,
        },
        budget,
    )?;
    same_binding(&got.binding, binding)?;
    if !got.complete_unique_lookup {
        return Err(QueryError::new(
            QueryErrorCode::IndexIncomplete,
            "exact visible node index is incomplete",
        ));
    }
    let Some(record) = got.record else {
        return Ok(None);
    };
    let value = parse_record(&record, binding, budget)?;
    if value.object_get("node_id").and_then(JsonValue::as_str) != Some(id)
        || value.object_get("depth").is_some()
    {
        return Err(QueryError::new(
            QueryErrorCode::CorruptSelectedCarrier,
            "node identity or reserved depth field invalid",
        ));
    }
    // The visible-index profile only excludes dense packet members. It may
    // never disguise a current rights denial as certified absence.
    work.charge(model.authorize_current(&record)?, budget)?;
    Ok(Some((record, value)))
}

fn push(output: &mut Vec<u8>, bytes: &[u8], budget: Budget) -> Result<(), QueryError> {
    if bytes.len() > budget.max_response_bytes.saturating_sub(output.len()) {
        return Err(QueryError::new(
            QueryErrorCode::BudgetExceeded,
            "source descent response budget exceeded",
        ));
    }
    output.extend_from_slice(bytes);
    Ok(())
}

pub(crate) fn json_string(value: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(value.len() + 2);
    out.push(b'"');
    for character in value.chars() {
        match character {
            '"' => out.extend_from_slice(br#"\""#),
            '\\' => out.extend_from_slice(br"\\"),
            '\n' => out.extend_from_slice(br"\n"),
            '\r' => out.extend_from_slice(br"\r"),
            '\t' => out.extend_from_slice(br"\t"),
            c if (c as u32) < 0x20 => {
                out.extend_from_slice(format!("\\u{:04x}", c as u32).as_bytes())
            }
            c => {
                let mut bytes = [0u8; 4];
                out.extend_from_slice(c.encode_utf8(&mut bytes).as_bytes());
            }
        }
    }
    out.push(b'"');
    out
}

fn node_with_depth(record: &RawRecord, depth: u8) -> Result<Vec<u8>, QueryError> {
    let trimmed = record.raw.trim_ascii();
    if trimmed.first() != Some(&b'{') || trimmed.last() != Some(&b'}') {
        return Err(QueryError::new(
            QueryErrorCode::CorruptSelectedCarrier,
            "node carrier is not an object",
        ));
    }
    let mut out = Vec::with_capacity(trimmed.len() + 12);
    out.extend_from_slice(&trimmed[..trimmed.len() - 1]);
    if !trimmed[1..trimmed.len() - 1].trim_ascii().is_empty() {
        out.push(b',');
    }
    out.extend_from_slice(b"\"depth\":");
    out.extend_from_slice(depth.to_string().as_bytes());
    out.push(b'}');
    Ok(out)
}

/// Execute the complete public `tos.source.descend` family over a sealed,
/// bibliographically visible source-navigation read model. Returns one staged
/// packet only after every selected carrier, scope and policy check succeeds.
fn prepare_source_descend<M: ReadModel>(
    model: &mut M,
    request: &SourceDescendRequest,
    budget: Budget,
) -> Result<PreparedSourceDescend, QueryError> {
    request.validate()?;
    model.check_interrupt()?;
    if !budget.valid() {
        return Err(QueryError::new(
            QueryErrorCode::InvalidRequest,
            "invalid source descent budget",
        ));
    }
    if request.node_id.len() > budget.max_request_bytes {
        return Err(QueryError::new(
            QueryErrorCode::BudgetExceeded,
            "source descent request byte budget exceeded",
        ));
    }
    let binding = model.selected_binding()?;
    if request
        .at_least_commit_seq
        .is_some_and(|minimum| binding.through_commit_seq < minimum)
    {
        return Err(QueryError::new(
            QueryErrorCode::PublicationPending,
            "selected read model has not published the requested commit",
        ));
    }
    if binding.model_abi != "tos_source_navigation_read_model_v1"
        || binding.selection_profile != "tos_source_navigation_visible_v1"
    {
        return Err(QueryError::new(
            QueryErrorCode::Unavailable,
            "source navigation visibility profile unsupported",
        ));
    }
    model.check_pin(&binding)?;
    let mut work = Work::default();
    let Some((root, _)) = exact_node(model, &request.node_id, &binding, budget, &mut work)? else {
        return Err(QueryError::new(
            QueryErrorCode::UnknownExactId,
            "unknown source-navigation node",
        ));
    };
    // The disclosure hold covers every carrier that influenced the result,
    // including a target skipped at the node limit and a non-emitted edge.
    let mut observed = vec![root.clone()];
    let mut nodes: BTreeMap<String, (u8, RawRecord)> = BTreeMap::new();
    nodes.insert(request.node_id.clone(), (0, root));
    let mut frontier = VecDeque::from([(request.node_id.clone(), 0u8)]);
    let mut edges: Vec<RawRecord> = Vec::new();
    let mut truncated = false;

    while let Some((current, depth)) = frontier.pop_front() {
        model.check_interrupt()?;
        work.step(budget)?;
        if depth >= request.max_depth {
            continue;
        }
        let mut after: Option<String> = None;
        let mut expected: Option<(u64, Digest256)> = None;
        let mut count = 0u64;
        let mut hasher = Digest256Hasher::new();
        loop {
            model.check_interrupt()?;
            model.check_pin(&binding)?;
            let remaining = budget.max_bytes.saturating_sub(work.bytes);
            let max_bytes = remaining.min(usize::MAX as u64) as usize;
            if max_bytes == 0 {
                return Err(QueryError::new(
                    QueryErrorCode::BudgetExceeded,
                    "source descent read budget exhausted",
                ));
            }
            let max_vm_steps = budget.max_cpu_steps.saturating_sub(work.cpu_steps);
            if max_vm_steps == 0 {
                return Err(QueryError::new(
                    QueryErrorCode::BudgetExceeded,
                    "source descent VM budget exhausted",
                ));
            }
            let page = model.visible_outgoing(
                &current,
                after.as_deref(),
                budget.page_rows,
                max_bytes,
                budget.json.max_bytes,
                max_vm_steps,
                budget.max_probes.saturating_sub(work.probes),
                budget.max_rows.saturating_sub(work.rows),
            )?;
            work.charge(page.charged, budget)?;
            let page_raw_bytes = page.edges.iter().try_fold(0u64, |total, record| {
                total.checked_add(record.raw.len() as u64).ok_or_else(|| {
                    QueryError::new(
                        QueryErrorCode::BudgetExceeded,
                        "source descent page byte count overflow",
                    )
                })
            })?;
            work.charge(
                Charged {
                    probes: 0,
                    rows: 0,
                    bytes: page_raw_bytes,
                    cpu_steps: page.edges.len() as u64,
                },
                budget,
            )?;
            same_binding(&page.binding, &binding)?;
            if page.from_id != current
                || page.after_edge_id != after
                || page.edges.len() > budget.page_rows
            {
                return Err(QueryError::new(
                    QueryErrorCode::IndexIncomplete,
                    "adjacency page scope or size invalid",
                ));
            }
            let certificate = (page.expected_count, page.expected_digest);
            if expected.is_some_and(|value| value != certificate) {
                return Err(QueryError::new(
                    QueryErrorCode::IndexIncomplete,
                    "adjacency certificate changed across pages",
                ));
            }
            expected = Some(certificate);
            if page.edges.is_empty() && !page.exhausted {
                return Err(QueryError::new(
                    QueryErrorCode::IndexIncomplete,
                    "adjacency seek did not progress",
                ));
            }
            for edge in page.edges {
                model.check_interrupt()?;
                work.step(budget)?;
                let value = parse_record(&edge, &binding, budget)?;
                work.charge(model.authorize_current(&edge)?, budget)?;
                observed.push(edge.clone());
                let edge_id = value
                    .object_get("edge_id")
                    .and_then(JsonValue::as_str)
                    .ok_or_else(|| {
                        QueryError::new(
                            QueryErrorCode::CorruptSelectedCarrier,
                            "edge identity missing",
                        )
                    })?;
                if edge_id.is_empty()
                    || after.as_deref().is_some_and(|last| edge_id <= last)
                    || value.object_get("from_id").and_then(JsonValue::as_str)
                        != Some(current.as_str())
                {
                    return Err(QueryError::new(
                        QueryErrorCode::IndexIncomplete,
                        "adjacency key order or source invalid",
                    ));
                }
                let target = value
                    .object_get("to_id")
                    .and_then(JsonValue::as_str)
                    .ok_or_else(|| {
                        QueryError::new(
                            QueryErrorCode::CorruptSelectedCarrier,
                            "edge target missing",
                        )
                    })?
                    .to_owned();
                count += 1;
                hasher.update(&(edge_id.len() as u64).to_be_bytes());
                hasher.update(edge_id.as_bytes());
                hasher.update(edge.sha256.as_bytes());
                after = Some(edge_id.to_owned());
                let target_record = if let Some((_, record)) = nodes.get(&target) {
                    Some(record.clone())
                } else {
                    exact_node(model, &target, &binding, budget, &mut work)?
                        .map(|(record, _)| record)
                };
                let Some(target_record) = target_record else {
                    continue;
                };
                observed.push(target_record.clone());
                if !nodes.contains_key(&target) && nodes.len() >= request.limit {
                    truncated = true;
                    continue;
                }
                if edges.len() >= budget.max_edges {
                    return Err(QueryError::new(
                        QueryErrorCode::BudgetExceeded,
                        "source descent edge budget exceeded",
                    ));
                }
                // The sealed unique edge index means an edge_id cannot repeat;
                // distinct IDs imply distinct source objects under this profile.
                edges.push(edge);
                if let std::collections::btree_map::Entry::Vacant(slot) =
                    nodes.entry(target.clone())
                {
                    slot.insert((depth + 1, target_record));
                    frontier.push_back((target, depth + 1));
                }
            }
            if page.exhausted {
                break;
            }
        }
        let (expected_count, expected_digest) = expected.expect("one page required");
        if count != expected_count || hasher.finalize() != expected_digest {
            return Err(QueryError::new(
                QueryErrorCode::IndexIncomplete,
                "sealed adjacency count or digest differs",
            ));
        }
    }

    // Check current policy only after the complete traversal and all sealed
    // adjacency scopes are verified, then recheck the pin before publication.
    for (_, record) in nodes.values() {
        model.check_interrupt()?;
        work.charge(model.authorize_current(record)?, budget)?;
    }
    for record in &edges {
        model.check_interrupt()?;
        work.charge(model.authorize_current(record)?, budget)?;
    }
    let remaining = budget.max_bytes.saturating_sub(work.bytes);
    let max_bytes = remaining.min(budget.json.max_bytes as u64) as usize;
    if max_bytes == 0
        || budget.max_probes.saturating_sub(work.probes) == 0
        || budget.max_rows.saturating_sub(work.rows) == 0
    {
        return Err(QueryError::new(
            QueryErrorCode::BudgetExceeded,
            "source descent authority read budget exhausted",
        ));
    }
    let authority = model.authority_boundary(max_bytes)?;
    work.charge(
        Charged {
            probes: 1,
            rows: 1,
            bytes: authority.raw.len() as u64,
            cpu_steps: 1,
        },
        budget,
    )?;
    let authority_value = parse_record(&authority, &binding, budget)?;
    if authority_value.as_str().is_none_or(str::is_empty) {
        return Err(QueryError::new(
            QueryErrorCode::CorruptSelectedCarrier,
            "source-navigation authority boundary must be a nonempty string",
        ));
    }
    work.charge(model.authorize_current(&authority)?, budget)?;
    model.check_pin(&binding)?;
    model.check_interrupt()?;

    let mut ordered_nodes: Vec<_> = nodes.into_iter().collect();
    ordered_nodes.sort_by(|left, right| (left.1.0, &left.0).cmp(&(right.1.0, &right.0)));
    let mut packet = Vec::new();
    push(
        &mut packet,
        b"{\"schema\":\"tos_source_descent_v1\",\"root_id\":",
        budget,
    )?;
    push(&mut packet, &json_string(&request.node_id), budget)?;
    push(&mut packet, format!(",\"max_depth\":{},\"limit\":{},\"truncated\":{},\"counts\":{{\"nodes\":{},\"edges\":{}}},\"nodes\":[", request.max_depth, request.limit, truncated, ordered_nodes.len(), edges.len()).as_bytes(), budget)?;
    for (index, (_, (depth, record))) in ordered_nodes.iter().enumerate() {
        if index > 0 {
            push(&mut packet, b",", budget)?;
        }
        push(&mut packet, &node_with_depth(record, *depth)?, budget)?;
    }
    push(&mut packet, b"],\"edges\":[", budget)?;
    for (index, edge) in edges.iter().enumerate() {
        if index > 0 {
            push(&mut packet, b",", budget)?;
        }
        push(&mut packet, edge.raw.trim_ascii(), budget)?;
    }
    push(&mut packet, b"],\"authority_note\":", budget)?;
    push(&mut packet, authority.raw.trim_ascii(), budget)?;
    push(&mut packet, b"}", budget)?;
    model.check_pin(&binding)?;
    observed.push(authority);
    Ok(PreparedSourceDescend {
        body: packet,
        binding,
        selected: observed,
    })
}

/// Produce one staged packet and acquire a live owner disclosure lease before
/// it can leave the private query core. The caller must keep the lease through
/// the final synchronous write/flush and call `recheck` just before framing.
pub fn source_descend<M: ReadModel>(
    model: &mut M,
    request: &SourceDescendRequest,
    budget: Budget,
) -> Result<DisclosableSourceDescend, QueryError> {
    let prepared = prepare_source_descend(model, request, budget)?;
    model.check_interrupt()?;
    model.check_pin(&prepared.binding)?;
    let mut lease = model.acquire_disclosure(&prepared.binding, &prepared.selected)?;
    lease.recheck()?;
    Ok(DisclosableSourceDescend {
        body: prepared.body,
        lease,
    })
}
