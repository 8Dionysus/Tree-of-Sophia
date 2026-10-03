//! Complete original carriers from the completed producer's borrowed capture.
//! This reader does not recover originals from normalized knowledge records.
use crate::native_snapshot::CompletedCaptureCarriers;
use crate::{Error, Result};
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::{
    CanonicalProfile, JsonLimits, JsonMode, JsonNumber, JsonNumberKind, JsonString, JsonValue,
    canonical_bytes_v1_with_visits, parse_json,
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

struct Read<'a> {
    view: &'a CompletedCaptureCarriers<'a>,
    budget: CapturedCarrierReadBudget,
    input_bytes: u64,
    rows: u64,
    visits: usize,
    deadline: Instant,
    cancelled: &'a AtomicBool,
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
    fn parse(&mut self, raw: &[u8]) -> Result<JsonValue> {
        self.checkpoint()?;
        self.input_bytes = self
            .input_bytes
            .checked_add(raw.len() as u64)
            .filter(|bytes| *bytes <= self.budget.max_input_bytes)
            .ok_or(Error::Budget("complete carrier input bytes"))?;
        let mut limits = self.budget.json;
        limits.max_visits = limits.max_visits.min(self.visits);
        let parsed = parse_json(raw, JsonMode::PublishedStrict, limits)
            .map_err(|_| Error::Invalid("complete carrier original JSON"))?;
        self.visits = self
            .visits
            .checked_sub(parsed.visits())
            .ok_or(Error::Budget("complete carrier JSON visits"))?;
        Ok(parsed.into_root())
    }
    fn header(&mut self, role: &str, prefix: &str) -> Result<JsonValue> {
        self.checkpoint()?;
        let available = self.budget.max_input_bytes.saturating_sub(self.input_bytes);
        let cap = self
            .budget
            .json
            .max_bytes
            .min(usize::try_from(available).unwrap_or(usize::MAX))
            .min(2 * 1024 * 1024);
        if cap == 0 {
            return Err(Error::Budget("complete carrier header bytes"));
        }
        let header = self.view.header_object(role, prefix, cap)?;
        let raw = serde_json::to_vec(&header)
            .map_err(|_| Error::Invalid("complete carrier header encoding"))?;
        if raw.len() > cap {
            return Err(Error::Budget("complete carrier header bytes"));
        }
        self.parse(&raw)
    }
    fn collection(&mut self, role: &str, name: &str) -> Result<Option<JsonValue>> {
        self.checkpoint()?;
        let mapping = match self.view.captured_collection_kind(role, name)?.as_deref() {
            None => return Ok(None),
            Some("array") => false,
            Some("mapping") if matches!(role, "bibliographic" | "philosophy") => true,
            _ => return Err(Error::Invalid("complete carrier original collection kind")),
        };
        let mut values = Vec::new();
        let view = self.view;
        let visited = view.visit_rows(role, name, |ordinal, raw| {
            self.checkpoint()?;
            if ordinal != values.len() as u64 {
                return Err(Error::Invalid("complete carrier original row order"));
            }
            self.rows = self
                .rows
                .checked_add(1)
                .filter(|rows| *rows <= self.budget.max_rows)
                .ok_or(Error::Budget("complete carrier rows"))?;
            values.push(self.parse(raw)?);
            Ok(())
        })?;
        if visited != values.len() as u64 {
            return Err(Error::Invalid("complete carrier original EOF count"));
        }
        if mapping {
            let mut fields = Vec::with_capacity(values.len());
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
                previous = Some(name.to_owned());
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
                replace(header, name, value)?;
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
            let collection = format!("source_navigation/{name}");
            if let Some(rows) = self.collection("corpus", &collection)? {
                replace(&mut header, name, rows)?;
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
                    replace(&mut header, "source_navigation", navigation)?;
                }
                Ok(header)
            }
            CapturedCarrierRequest::PhilosophyProjection => {
                let mut projection = self.header("philosophy", "")?;
                for name in self.view.captured_collection_names("philosophy")? {
                    if let Some(value) = self.collection("philosophy", &name)? {
                        replace(&mut projection, &name, value)?;
                    }
                }
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
        let mut ids = BTreeSet::new();
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
            ids.insert(id);
        }
        let comparisons = usize::BITS - ids.len().max(1).leading_zeros();
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
            if ids.contains(from) && ids.contains(to) {
                kept.push(edge);
            }
        }
        drop(ids);
        let node_count = nodes.len();
        let edge_count = kept.len();
        replace(&mut navigation, "nodes", JsonValue::Array(nodes))?;
        replace(&mut navigation, "edges", JsonValue::Array(kept))?;
        let mut counts = take(&mut navigation, "counts").unwrap_or(JsonValue::Object(Vec::new()));
        replace(&mut counts, "nodes", number(node_count))?;
        replace(&mut counts, "edges", number(edge_count))?;
        replace(&mut navigation, "counts", counts)?;
        Ok(navigation)
    }
}
fn replace(object: &mut JsonValue, name: &str, value: JsonValue) -> Result<()> {
    let JsonValue::Object(fields) = object else {
        return Err(Error::Invalid("complete carrier original object"));
    };
    if let Some((_, old)) = fields
        .iter_mut()
        .find(|(key, _)| key.as_str() == Some(name))
    {
        *old = value;
    } else {
        fields.push((JsonString::from_utf8(name), value));
    }
    Ok(())
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
    if budget.max_rows == 0
        || budget.max_input_bytes == 0
        || budget.max_output_bytes == 0
        || budget.json.max_visits == 0
    {
        return Err(Error::Budget("complete carrier budget"));
    }
    let mut read = Read {
        view,
        budget,
        input_bytes: 0,
        rows: 0,
        visits: budget.json.max_visits,
        deadline,
        cancelled,
    };
    read.checkpoint()?;
    let value = read.read(request)?;
    let mut output_limits = budget.json;
    output_limits.max_bytes = output_limits.max_bytes.min(budget.max_output_bytes);
    output_limits.max_visits = output_limits.max_visits.min(read.visits);
    let (raw, _, _) = canonical_bytes_v1_with_visits(
        &value,
        CanonicalProfile::SourceRecordDigestV1,
        output_limits,
    )
    .map_err(|_| Error::Budget("complete carrier output bytes or visits"))?;
    read.checkpoint()?;
    Ok(raw)
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
