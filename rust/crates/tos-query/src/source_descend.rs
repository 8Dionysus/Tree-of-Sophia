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
    const fn new(code: QueryErrorCode, message: &'static str) -> Self {
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

/// The adapter accounts physical work, including rows skipped by visibility,
/// dangling targets, and verification reads. Returned rows are only a subset.
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
    fn selected_binding(&mut self) -> Result<Binding, QueryError>;
    /// `max_bytes` must be enforced before transferring a wide row from the
    /// physical backend, including an oversized-row refusal.
    fn exact_visible_node(
        &mut self,
        id: &str,
        max_bytes: usize,
        max_vm_steps: u64,
    ) -> Result<ExactNode, QueryError>;
    fn visible_outgoing(
        &mut self,
        from_id: &str,
        after_edge_id: Option<&str>,
        max_rows: usize,
        max_bytes: usize,
        max_vm_steps: u64,
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

/// An owner hold, not merely a one-time policy observation. The transport
/// rechecks immediately before framing and retains the lease until flush.
pub trait DisclosureLease: Send {
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
        self.probes = self.probes.saturating_add(cost.probes);
        self.rows = self.rows.saturating_add(cost.rows);
        self.bytes = self.bytes.saturating_add(cost.bytes);
        self.cpu_steps = self.cpu_steps.saturating_add(cost.cpu_steps);
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
    let max_bytes = remaining.min(budget.json.max_bytes as u64) as usize;
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
    let got = model.exact_visible_node(id, max_bytes, max_vm_steps)?;
    work.charge(got.charged, budget)?;
    work.charge(
        Charged {
            probes: 1,
            rows: u64::from(got.record.is_some()),
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
        work.step(budget)?;
        if depth >= request.max_depth {
            continue;
        }
        let mut after: Option<String> = None;
        let mut expected: Option<(u64, Digest256)> = None;
        let mut count = 0u64;
        let mut hasher = Digest256Hasher::new();
        loop {
            model.check_pin(&binding)?;
            let remaining = budget.max_bytes.saturating_sub(work.bytes);
            let max_bytes = remaining
                .min(budget.json.max_bytes.saturating_mul(budget.page_rows) as u64)
                as usize;
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
                max_vm_steps,
            )?;
            work.charge(page.charged, budget)?;
            work.charge(
                Charged {
                    probes: 1,
                    rows: page.edges.len() as u64,
                    bytes: page
                        .edges
                        .iter()
                        .map(|record| record.raw.len() as u64)
                        .sum(),
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
        work.charge(model.authorize_current(record)?, budget)?;
    }
    for record in &edges {
        work.charge(model.authorize_current(record)?, budget)?;
    }
    let remaining = budget.max_bytes.saturating_sub(work.bytes);
    let max_bytes = remaining.min(budget.json.max_bytes as u64) as usize;
    if max_bytes == 0 {
        return Err(QueryError::new(
            QueryErrorCode::BudgetExceeded,
            "source descent read budget exhausted",
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
    model.check_pin(&prepared.binding)?;
    let mut lease = model.acquire_disclosure(&prepared.binding, &prepared.selected)?;
    lease.recheck()?;
    Ok(DisclosableSourceDescend {
        body: prepared.body,
        lease,
    })
}
