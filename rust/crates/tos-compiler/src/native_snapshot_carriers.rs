//! Complete original carriers from the completed producer's borrowed capture.
//! This reader does not recover originals from normalized knowledge records.
use crate::native_snapshot::CompletedCaptureCarriers;
use crate::{Error, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::{
    CanonicalProfile, JsonLimits, JsonMode, JsonNumber, JsonNumberKind, JsonString, JsonValue,
    canonical_bytes_v1_with_state_budget_and_visits_and_check, parse_json,
    parse_json_with_state_budget_and_check,
};

#[derive(Clone, Copy, Debug)]
pub enum CapturedCarrierRequest {
    CorpusIndex,
    CorpusHeader,
    PhilosophyProjection,
    PhilosophyAuditPayload,
    SourceNavigation { bibliographic_only: bool },
    BibliographicGraph,
    IndexExists,
}

#[derive(Clone, Copy, Debug)]
pub struct CapturedCarrierReadBudget {
    pub max_rows: u64,
    pub max_input_bytes: u64,
    pub max_output_bytes: usize,
    pub json: JsonLimits,
}

/// Same read's cumulative logical admissions. On failure JSON visits retain the
/// admitted ceiling, because the parser does not expose partial failure visits.
#[derive(Default, Clone, Copy, Debug)]
pub struct CapturedCarrierUsage {
    pub rows: u64,
    pub input_bytes: u64,
    pub json_visits: usize,
}
pub struct CapturedCarrierDelivery {
    pub bytes: Vec<u8>,
    pub usage: CapturedCarrierUsage,
}
struct Read<'a> {
    view: &'a CompletedCaptureCarriers<'a>,
    budget: CapturedCarrierReadBudget,
    input_bytes: u64,
    rows: u64,
    visits: usize,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    remaining: &'a dyn Fn(usize) -> Result<usize>,
    usage: &'a mut CapturedCarrierUsage,
    retained: usize,
    owner_workspace: usize,
}
impl Read<'_> {
    fn checkpoint(&self) -> Result<()> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err(Error::Invalid("complete carrier read cancelled"));
        }
        if Instant::now() >= self.deadline {
            return Err(Error::Budget("complete carrier read deadline"));
        }
        Ok(())
    }
    fn remaining(&self, extra: usize) -> Result<usize> {
        self.checkpoint()?;
        let held = self
            .retained
            .checked_add(self.owner_workspace)
            .and_then(|n| n.checked_add(extra))
            .ok_or(Error::Budget("complete carrier retained state"))?;
        (self.remaining)(held)
    }
    fn reserve<T>(&mut self, values: &mut Vec<T>, additional: usize) -> Result<()> {
        let count = values
            .len()
            .checked_add(additional)
            .ok_or(Error::Budget("complete carrier container"))?;
        if count <= values.capacity() {
            return Ok(());
        }
        let old = values
            .capacity()
            .checked_mul(std::mem::size_of::<T>())
            .ok_or(Error::Budget("complete carrier container"))?;
        let new = count
            .checked_mul(std::mem::size_of::<T>())
            .ok_or(Error::Budget("complete carrier container"))?;
        // Existing storage and replacement overlap until reserve succeeds.
        let available = self.remaining(new)?;
        values
            .try_reserve_exact(count - values.len())
            .map_err(|_| Error::Budget("complete carrier container"))?;
        let actual = values
            .capacity()
            .checked_mul(std::mem::size_of::<T>())
            .ok_or(Error::Budget("complete carrier container"))?;
        // Compare only allocator over-allocation to the unspent remainder.
        // An unconstrained caller uses usize::MAX; adding the already admitted
        // request to that sentinel must not reject even a one-element carrier.
        if actual.saturating_sub(new) > available {
            return Err(Error::Budget("complete carrier container capacity"));
        }
        self.retained = self
            .retained
            .checked_add(actual)
            .and_then(|n| n.checked_sub(old))
            .ok_or(Error::Budget("complete carrier retained state"))?;
        Ok(())
    }
    fn key(&mut self, name: &str) -> Result<JsonString> {
        let units = name.encode_utf16().count();
        let bytes = units
            .checked_mul(2)
            .map(|n| n.max(4)) // Vec<u16>::collect minimum nonzero capacity.
            .and_then(|n| n.checked_mul(std::mem::size_of::<u16>()))
            .and_then(|n| n.checked_add(name.len()))
            .ok_or(Error::Budget("complete carrier key"))?;
        self.remaining(bytes)?;
        let key = JsonString::from_utf8(name);
        let actual = key
            .retained_storage_bytes()
            .map_err(|_| Error::Budget("complete carrier key"))?;
        self.remaining(actual)?;
        self.retained = self
            .retained
            .checked_add(actual)
            .ok_or(Error::Budget("complete carrier retained state"))?;
        Ok(key)
    }
    fn replace(&mut self, object: &mut JsonValue, name: &str, value: JsonValue) -> Result<()> {
        let JsonValue::Object(fields) = object else {
            return Err(Error::Invalid("complete carrier original object"));
        };
        if let Some((_, old)) = fields
            .iter_mut()
            .find(|(key, _)| key.as_str() == Some(name))
        {
            // Retain the conservative prior allocation admission until read returns.
            *old = value;
        } else {
            self.reserve(fields, 1)?;
            let key = self.key(name)?;
            fields.push((key, value));
        }
        Ok(())
    }
    fn observe_input(&mut self, raw: &[u8]) -> Result<()> {
        self.observe_bytes(raw.len() as u64)
    }
    fn observe_bytes(&mut self, bytes: u64) -> Result<()> {
        let observed = self.input_bytes.checked_add(bytes);
        // This raw row was already authenticated by the held capture visitor.
        // Keep its observation even when this read rejects the finite cap.
        self.usage.input_bytes = observed.unwrap_or(u64::MAX);
        self.input_bytes =
            observed.ok_or(Error::Budget("complete carrier input bytes overflow"))?;
        if self.input_bytes > self.budget.max_input_bytes {
            return Err(Error::Budget("complete carrier input bytes"));
        }
        Ok(())
    }
    fn parse(&mut self, raw: &[u8]) -> Result<JsonValue> {
        self.observe_input(raw)?;
        self.parse_observed(raw)
    }
    fn parse_observed(&mut self, raw: &[u8]) -> Result<JsonValue> {
        self.checkpoint()?;
        let mut limits = self.budget.json;
        limits.max_visits = limits.max_visits.min(self.visits);
        // Ceiling is retained on every failure; successful accounting uses actual visits.
        let before = self.usage.json_visits;
        self.usage.json_visits = before
            .checked_add(limits.max_visits)
            .ok_or(Error::Budget("complete carrier JSON visits"))?;
        self.view.charge_work(
            (raw.len() as u64)
                .checked_add(limits.max_visits as u64)
                .ok_or(Error::Budget("complete carrier parse work"))?,
        )?;
        let available = self.remaining(raw.len())?;
        let mut check = || {
            self.checkpoint().map_err(|_| {
                tos_foundation::FoundationError::new(
                    tos_foundation::FoundationErrorCode::BudgetExceeded,
                    "complete carrier cutoff/cancellation",
                )
            })
        };
        let parsed = parse_json_with_state_budget_and_check(
            raw,
            JsonMode::PublishedStrict,
            limits,
            available,
            &mut check,
        )
        .map_err(|_| Error::Invalid("complete carrier original JSON"))?;
        let used = parsed.visits();
        self.visits = self
            .visits
            .checked_sub(used)
            .ok_or(Error::Budget("complete carrier JSON visits"))?;
        self.usage.json_visits = before
            .checked_add(used)
            .ok_or(Error::Budget("complete carrier JSON visits"))?;
        let root = parsed.into_root();
        let actual = root
            .retained_storage_bytes()
            .map_err(|_| Error::Budget("complete carrier tree state"))?;
        self.remaining(
            raw.len()
                .checked_add(actual)
                .ok_or(Error::Budget("complete carrier tree state"))?,
        )?;
        self.retained = self
            .retained
            .checked_add(actual)
            .ok_or(Error::Budget("complete carrier retained state"))?;
        Ok(root)
    }
    fn header(&mut self, role: &str, prefix: &str) -> Result<JsonValue> {
        self.checkpoint()?;
        let cap = self
            .budget
            .json
            .max_bytes
            .min(
                usize::try_from(self.budget.max_input_bytes.saturating_sub(self.input_bytes))
                    .unwrap_or(usize::MAX),
            )
            .min(2 * 1024 * 1024);
        if cap == 0 {
            return Err(Error::Budget("complete carrier header bytes"));
        }
        self.observe_bytes(2)?; // Original enclosing object braces.
        let mut fields = Vec::new();
        let view = self.view;
        view.visit_header_fields(role, prefix, cap, |name, raw| {
            // Count the existing serde header key representation without a buffer.
            let mut count = HeaderKeyCount(0);
            serde_json::to_writer(&mut count, name)
                .map_err(|_| Error::Invalid("complete carrier header key encoding"))?;
            let overhead = count
                .0
                .checked_add(1)
                .and_then(|n| n.checked_add(usize::from(!fields.is_empty())))
                .ok_or(Error::Budget("complete carrier header bytes"))?;
            self.observe_bytes(overhead as u64)?;
            self.observe_input(raw)?;
            let comparison_work = fields.iter().try_fold(0usize, |n, (key, _)| {
                n.checked_add(name.len())
                    .and_then(|n| n.checked_add(JsonString::as_str(key).map_or(0, str::len)))
                    .ok_or(Error::Budget("complete carrier header comparison work"))
            })?;
            self.view.charge_work(comparison_work as u64)?;
            if fields
                .iter()
                .any(|(key, _)| JsonString::as_str(key) == Some(name))
            {
                return Err(Error::Invalid("complete carrier duplicate header"));
            }
            self.reserve(&mut fields, 1)?;
            let key = self.key(name)?;
            let value = self.parse_observed(raw)?;
            fields.push((key, value));
            Ok(())
        })?;
        Ok(JsonValue::Object(fields))
    }
    fn collection(&mut self, role: &str, name: &str) -> Result<Option<JsonValue>> {
        self.checkpoint()?;
        self.remaining(64)?;
        self.retained = self
            .retained
            .checked_add(64)
            .ok_or(Error::Budget("complete carrier kind state"))?;
        let mapping = match self.view.captured_collection_kind(role, name)?.as_deref() {
            None => return Ok(None),
            Some("array") => false,
            Some("mapping") if matches!(role, "bibliographic" | "philosophy") => true,
            _ => return Err(Error::Invalid("complete carrier original collection kind")),
        };
        let mut values = Vec::new();
        let view = self.view;
        let visited = view.visit_rows(role, name, |ordinal, raw| {
            let observed = self.rows.checked_add(1);
            self.usage.rows = observed.unwrap_or(u64::MAX);
            self.rows = observed.ok_or(Error::Budget("complete carrier rows overflow"))?;
            self.observe_input(raw)?;
            self.checkpoint()?;
            if ordinal != values.len() as u64 {
                return Err(Error::Invalid("complete carrier original row order"));
            }
            if self.rows > self.budget.max_rows {
                return Err(Error::Budget("complete carrier rows"));
            }
            self.reserve(&mut values, 1)?;
            values.push(self.parse_observed(raw)?);
            Ok(())
        })?;
        if visited != values.len() as u64 {
            return Err(Error::Invalid("complete carrier original EOF count"));
        }
        if mapping {
            let mut fields = Vec::new();
            self.reserve(&mut fields, values.len())?;
            let mut previous: Option<String> = None;
            for mut record in values {
                let key = take(&mut record, "key")
                    .and_then(|value| match value {
                        JsonValue::String(key) => Some(key),
                        _ => None,
                    })
                    .ok_or(Error::Invalid("complete carrier mapping key"))?;
                let name = key
                    .as_str()
                    .ok_or(Error::Invalid("complete carrier mapping key"))?;
                if previous.as_deref().is_some_and(|prior| name <= prior) {
                    return Err(Error::Invalid("complete carrier mapping order"));
                }
                self.remaining(name.len())?;
                previous = Some(name.to_owned());
                self.retained = self
                    .retained
                    .checked_add(previous.as_ref().unwrap().capacity())
                    .ok_or(Error::Budget("complete carrier mapping key"))?;
                let value = take(&mut record, "value")
                    .ok_or(Error::Invalid("complete carrier mapping value"))?;
                fields.push((key, value));
            }
            Ok(Some(JsonValue::Object(fields)))
        } else {
            Ok(Some(JsonValue::Array(values)))
        }
    }
    fn attach(&mut self, header: &mut JsonValue, role: &str, names: &[&str]) -> Result<()> {
        for name in names {
            if let Some(value) = self.collection(role, name)? {
                self.replace(header, name, value)?;
            }
        }
        Ok(())
    }
    fn navigation(&mut self) -> Result<Option<JsonValue>> {
        match self
            .view
            .captured_collection_kind("corpus", "source_navigation")?
            .as_deref()
        {
            None => return Ok(None),
            Some("object") => {}
            _ => return Err(Error::Invalid("complete carrier navigation object")),
        }
        let mut header = self.header("corpus", "source_navigation")?;
        for name in ["nodes", "edges", "rights"] {
            self.remaining(64)?;
            self.retained = self
                .retained
                .checked_add(64)
                .ok_or(Error::Budget("complete carrier collection name"))?;
            let collection = format!("source_navigation/{name}");
            if let Some(rows) = self.collection("corpus", &collection)? {
                self.replace(&mut header, name, rows)?;
            }
        }
        Ok(Some(header))
    }
    fn read(&mut self, request: CapturedCarrierRequest) -> Result<JsonValue> {
        match request {
            CapturedCarrierRequest::IndexExists => {
                // The completed capture pins the actual required corpus input.
                // No result is produced if its pre/post currentness check fails.
                self.header("corpus", "")?;
                Ok(JsonValue::Bool(true))
            }
            CapturedCarrierRequest::CorpusHeader => {
                let mut header = self.header("corpus", "")?;
                self.attach(&mut header, "corpus", &["graph_views"])?;
                Ok(header)
            }
            CapturedCarrierRequest::CorpusIndex => {
                let mut header = self.header("corpus", "")?;
                self.attach(
                    &mut header,
                    "corpus",
                    &[
                        "nodes",
                        "resources",
                        "manifests",
                        "branches",
                        "graph_views",
                        "relation_packs",
                        "relation_edges",
                        "diagnostics",
                    ],
                )?;
                if let Some(navigation) = self.navigation()? {
                    self.replace(&mut header, "source_navigation", navigation)?;
                }
                Ok(header)
            }
            CapturedCarrierRequest::PhilosophyProjection => {
                let mut projection = self.header("philosophy", "")?;
                let view = self.view;
                view.visit_collection_names("philosophy", |name| {
                    if let Some(value) = self.collection("philosophy", name)? {
                        self.replace(&mut projection, name, value)?;
                    }
                    Ok(())
                })?;
                if !matches!(
                    projection
                        .object_get("schema_version")
                        .and_then(JsonValue::as_str),
                    Some(
                        "tos_philosophy_graph_projection_v1" | "tos_philosophy_graph_projection_v2"
                    )
                ) {
                    return Err(Error::Invalid("philosophy projection schema_version"));
                }
                Ok(projection)
            }
            CapturedCarrierRequest::PhilosophyAuditPayload => {
                let available = self.budget.max_input_bytes.saturating_sub(self.input_bytes);
                let cap = self
                    .budget
                    .json
                    .max_bytes
                    .min(usize::try_from(available).unwrap_or(usize::MAX));
                if cap == 0 {
                    return Err(Error::Budget("philosophy audit input bytes"));
                }
                self.remaining(cap)?;
                let raw = self.view.read_philosophy_audit(cap)?;
                let payload = self.parse(&raw)?;
                if payload
                    .object_get("schema_version")
                    .and_then(JsonValue::as_str)
                    != Some("tos_philosophy_post_planting_audit_v1")
                {
                    return Err(Error::Invalid("philosophy audit schema_version"));
                }
                Ok(payload)
            }
            CapturedCarrierRequest::SourceNavigation { bibliographic_only } => {
                let navigation = self.navigation()?.ok_or(Error::Invalid(
                    "corpus index has no source_navigation surface",
                ))?;
                if navigation
                    .object_get("schema_version")
                    .and_then(JsonValue::as_str)
                    != Some("tos_source_navigation_v1")
                {
                    return Err(Error::Invalid("source_navigation schema_version"));
                }
                if bibliographic_only {
                    self.bibliographic_navigation(navigation)
                } else {
                    Ok(navigation)
                }
            }
            CapturedCarrierRequest::BibliographicGraph => {
                let mut header = self.header("bibliographic", "")?;
                if header
                    .object_get("schema_version")
                    .and_then(JsonValue::as_str)
                    != Some("tos_source_witness_bibliographic_graph_v1")
                {
                    return Err(Error::Invalid("bibliographic graph schema_version"));
                }
                self.attach(
                    &mut header,
                    "bibliographic",
                    &["nodes", "edges", "claim_traces", "input_digests"],
                )?;
                Ok(header)
            }
        }
    }
    fn bibliographic_navigation(&mut self, mut navigation: JsonValue) -> Result<JsonValue> {
        let mut nodes = take_array(&mut navigation, "nodes")?;
        let mut edges = take_array(&mut navigation, "edges")?;
        // Keep the original node/edge payloads and order. Membership uses the
        // same retained original nodes, without cloning a second ID inventory.
        nodes.retain(|node| {
            !truthy(
                node.object_get("properties")
                    .and_then(|properties| properties.object_get("packet_id")),
            )
        });
        let mut ids = Vec::new();
        self.reserve(&mut ids, nodes.len())?;
        let mut max_id_bytes = 0usize;
        for node in &nodes {
            self.checkpoint()?;
            let id = node
                .object_get("node_id")
                .and_then(JsonValue::as_str)
                .ok_or(Error::Invalid(
                    "bibliographic navigation selected string ID absent",
                ))?;
            self.view.charge_work(id.len() as u64)?;
            max_id_bytes = max_id_bytes.max(id.len());
            ids.push(id);
        }
        let comparisons = usize::BITS - ids.len().max(1).leading_zeros();
        self.view.charge_work(
            (ids.len() as u64)
                .checked_mul(comparisons as u64)
                .and_then(|n| n.checked_mul(max_id_bytes as u64))
                .ok_or(Error::Budget("bibliographic navigation sort work"))?,
        )?;
        ids.sort_unstable();
        ids.dedup();
        let mut kept = Vec::new();
        for edge in edges.drain(..) {
            self.checkpoint()?;
            let from = edge
                .object_get("from_id")
                .and_then(JsonValue::as_str)
                .ok_or(Error::Invalid(
                    "bibliographic navigation selected from ID absent",
                ))?;
            let to = edge
                .object_get("to_id")
                .and_then(JsonValue::as_str)
                .ok_or(Error::Invalid(
                    "bibliographic navigation selected to ID absent",
                ))?;
            // A B-tree node may compare several keys. Price a conservative
            // twelve comparisons per level against the longest actual ID.
            let work = max_id_bytes
                .checked_add(from.len())
                .and_then(|n| n.checked_add(to.len()))
                .and_then(|n| n.checked_mul(comparisons as usize))
                .and_then(|n| n.checked_mul(24))
                .ok_or(Error::Budget("bibliographic navigation membership work"))?;
            self.view.charge_work(work as u64)?;
            if ids.binary_search(&from).is_ok() && ids.binary_search(&to).is_ok() {
                self.reserve(&mut kept, 1)?;
                kept.push(edge);
            }
        }
        drop(ids);
        let node_count = nodes.len();
        let edge_count = kept.len();
        self.replace(&mut navigation, "nodes", JsonValue::Array(nodes))?;
        self.replace(&mut navigation, "edges", JsonValue::Array(kept))?;
        let mut counts = take(&mut navigation, "counts").unwrap_or(JsonValue::Object(Vec::new()));
        self.remaining(40)?;
        self.retained = self
            .retained
            .checked_add(40)
            .ok_or(Error::Budget("complete carrier counts"))?;
        self.replace(&mut counts, "nodes", number(node_count))?;
        self.remaining(40)?;
        self.retained = self
            .retained
            .checked_add(40)
            .ok_or(Error::Budget("complete carrier counts"))?;
        self.replace(&mut counts, "edges", number(edge_count))?;
        self.replace(&mut navigation, "counts", counts)?;
        Ok(navigation)
    }
}
struct HeaderKeyCount(usize);
impl std::io::Write for HeaderKeyCount {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 = self
            .0
            .checked_add(bytes.len())
            .ok_or_else(|| std::io::Error::other("header key count overflow"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn take(object: &mut JsonValue, name: &str) -> Option<JsonValue> {
    let JsonValue::Object(fields) = object else {
        return None;
    };
    fields
        .iter_mut()
        .find(|(key, _)| key.as_str() == Some(name))
        .map(|(_, value)| std::mem::replace(value, JsonValue::Null))
}
fn take_array(object: &mut JsonValue, name: &str) -> Result<Vec<JsonValue>> {
    match take(object, name) {
        None => Ok(Vec::new()),
        Some(JsonValue::Array(values)) => Ok(values),
        _ => Err(Error::Invalid("bibliographic navigation original array")),
    }
}
fn number(count: usize) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: count.to_string(),
    })
}
fn truthy(value: Option<&JsonValue>) -> bool {
    match value {
        None | Some(JsonValue::Null) | Some(JsonValue::Bool(false)) => false,
        Some(JsonValue::String(value)) => !value.units().is_empty(),
        Some(JsonValue::Array(value)) => !value.is_empty(),
        Some(JsonValue::Object(value)) => !value.is_empty(),
        Some(JsonValue::Number(value)) => value
            .lexeme
            .parse::<f64>()
            .map_or(true, |value| value != 0.0),
        _ => true,
    }
}

/// Call inside `CompletedNativeSnapshot::with_capture_carriers`. The producer
/// verifies its complete source/custody before and after this bounded read.
/// The Access caller still retains the real selected final disclosure fence.
pub fn read_complete_captured_carrier(
    view: &CompletedCaptureCarriers<'_>,
    request: CapturedCarrierRequest,
    budget: CapturedCarrierReadBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>> {
    let mut usage = CapturedCarrierUsage::default();
    read_complete_captured_carrier_with_state(
        view,
        request,
        budget,
        deadline,
        cancelled,
        &|_| Ok(usize::MAX),
        &mut usage,
    )
    .map(|delivery| delivery.bytes)
}

/// Connected callers supply the genuine original remainder and retain `usage`
/// on success AND failure. Any error terminates the session; no admission refund
/// is available after parser/output failure or a final capture fence refusal.
pub fn read_complete_captured_carrier_with_state(
    view: &CompletedCaptureCarriers<'_>,
    request: CapturedCarrierRequest,
    budget: CapturedCarrierReadBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
    remaining_after_retained: &dyn Fn(usize) -> Result<usize>,
    usage: &mut CapturedCarrierUsage,
) -> Result<CapturedCarrierDelivery> {
    if budget.max_rows == 0
        || budget.max_input_bytes == 0
        || budget.max_output_bytes == 0
        || budget.json.max_visits == 0
    {
        return Err(Error::Budget("complete carrier budget"));
    }
    if usage.rows != 0 || usage.input_bytes != 0 || usage.json_visits != 0 {
        return Err(Error::Invalid("complete carrier nonempty usage report"));
    }
    let mut read = Read {
        view,
        budget,
        input_bytes: 0,
        rows: 0,
        visits: budget.json.max_visits,
        deadline,
        cancelled,
        remaining: remaining_after_retained,
        usage,
        retained: std::mem::size_of::<Read<'_>>(),
        owner_workspace: view
            .carrier_reader_workspace()?
            .checked_mul(2)
            .ok_or(Error::Budget("complete carrier nested owner workspace"))?,
    };
    read.remaining(0)?;
    let value = read.read(request)?;
    let mut limits = budget.json;
    limits.max_bytes = limits.max_bytes.min(budget.max_output_bytes);
    limits.max_visits = limits.max_visits.min(read.visits);
    let before = read.usage.json_visits;
    read.usage.json_visits = before
        .checked_add(limits.max_visits)
        .ok_or(Error::Budget("complete carrier JSON visits"))?;
    read.view.charge_work(
        (limits.max_bytes as u64)
            .checked_mul(2)
            .and_then(|n| n.checked_add(limits.max_visits as u64))
            .ok_or(Error::Budget("complete carrier output work"))?,
    )?;
    let available = read.remaining(0)?;
    let mut check = || {
        read.checkpoint().map_err(|_| {
            tos_foundation::FoundationError::new(
                tos_foundation::FoundationErrorCode::BudgetExceeded,
                "complete carrier output cutoff/cancellation",
            )
        })
    };
    let (raw, used) = canonical_bytes_v1_with_state_budget_and_visits_and_check(
        &value,
        CanonicalProfile::SourceRecordDigestV1,
        limits,
        available,
        &mut check,
    )
    .map_err(|_| Error::Budget("complete carrier output bytes/visits/state"))?;
    read.usage.json_visits = before
        .checked_add(used)
        .ok_or(Error::Budget("complete carrier JSON visits"))?;
    read.remaining(raw.capacity())?;
    read.checkpoint()?;
    let usage = *read.usage;
    Ok(CapturedCarrierDelivery { bytes: raw, usage })
}

/// Exact finished Evidence Lens projection lent by its actual owner holder.
/// Call only inside `CompletedNativeSnapshot::with_evidence_projection` so the
/// completed selection and evidence source/output closure are checked pre/post.
pub fn read_complete_evidence_projection(
    view: &crate::native_snapshot::CompletedEvidenceProjectionView<'_>,
    budget: CapturedCarrierReadBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>> {
    let checkpoint = || {
        if cancelled.load(Ordering::Acquire) {
            return Err(Error::Invalid("complete evidence read cancelled"));
        }
        if Instant::now() >= deadline {
            return Err(Error::Budget("complete evidence read deadline"));
        }
        Ok(())
    };
    checkpoint()?;
    let raw = view.raw();
    if raw.len() as u64 > budget.max_input_bytes
        || raw.len() > budget.max_output_bytes
        || budget.json.max_visits == 0
    {
        return Err(Error::Budget("complete evidence read bytes or visits"));
    }
    view.charge_work(raw.len() as u64)?;
    let parsed = parse_json(raw, JsonMode::PublishedStrict, budget.json)
        .map_err(|_| Error::Invalid("complete evidence original JSON"))?;
    if parsed
        .root()
        .object_get("schema_version")
        .and_then(JsonValue::as_str)
        != Some("tos_epistemic_evidence_projection_v1")
    {
        return Err(Error::Invalid("complete evidence schema_version"));
    }
    drop(parsed);
    checkpoint()?;
    view.charge_work(raw.len() as u64)?;
    let output = raw.to_vec();
    checkpoint()?;
    Ok(output)
}
