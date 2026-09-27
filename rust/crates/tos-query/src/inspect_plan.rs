//! The shared complete inspect domain plan; adapters retain carrier custody.
//! Lookup producers must return the whole admitted alias set after a bounded
//! ID lookahead, before loading bodies. Each supplied row has an authenticated
//! original ID/digest and is parsed under the caller's payload/JSON limits.
//! Incident totals depend on the admitted complete incidence index. This plan
//! checks selected closure; it does not turn arbitrary rows into a snapshot.
//! Raw byte charges come from the adapter, which caps transfer before parsing;
//! re-emitting JSON here would change lexical numeric sizes. Logical row/value
//! work is bounded separately from the native VM or published D1 accounting.
use crate::{
    search_v2::{SearchKind, SearchV2Error, SearchV2ErrorCode},
    source_read_projection::{object, source_read_targets, text},
};
use std::collections::BTreeSet;
use tos_foundation::{
    JsonLimits, JsonNumber, JsonNumberKind, JsonValue, python_strip_unicode16_v1,
};
#[derive(Clone, Copy, Debug)]
pub struct InspectBudget {
    pub max_open_vm_steps: u64,
    pub max_read_vm_steps: u64,
    pub max_matches: usize,
    pub max_rows: u64,
    pub max_field_bytes: usize,
    pub max_payload_bytes: usize,
    pub max_decoded_bytes: u64,
    pub max_response_bytes: usize,
    pub json: JsonLimits,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AbortReason {
    Cancelled,
    DeadlineExceeded,
}

/// A cheap owner/transport cancellation probe. Deadline clocks and client
/// disconnect signals stay with the caller; QRY checks at domain boundaries
/// and inside every SQLite VM progress callback during a seek.
pub trait AbortProbe: Send + Sync {
    fn reason(&self) -> Option<AbortReason>;
}

fn error(code: SearchV2ErrorCode, message: &'static str) -> SearchV2Error {
    SearchV2Error { code, message }
}
fn corrupt() -> SearchV2Error {
    error(
        SearchV2ErrorCode::CorruptSelectedCarrier,
        "inspect producer closure invalid",
    )
}
fn budget_error() -> SearchV2Error {
    error(SearchV2ErrorCode::BudgetExceeded, "inspect budget exceeded")
}
fn check(probe: Option<&dyn AbortProbe>) -> Result<(), SearchV2Error> {
    match probe.and_then(AbortProbe::reason) {
        Some(AbortReason::Cancelled) => {
            Err(error(SearchV2ErrorCode::Cancelled, "inspect cancelled"))
        }
        Some(AbortReason::DeadlineExceeded) => Err(error(
            SearchV2ErrorCode::DeadlineExceeded,
            "inspect deadline exceeded",
        )),
        None => Ok(()),
    }
}
fn number(value: u64) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: value.to_string(),
    })
}
#[derive(Clone, Debug)]
pub struct InspectRequest {
    kind: SearchKind,
    identifier: String,
    relation_limit: usize,
}
impl InspectRequest {
    pub fn new(
        kind: SearchKind,
        identifier: &str,
        relation_limit: usize,
        budget: InspectBudget,
    ) -> Result<Self, SearchV2Error> {
        if identifier.len() as u64 > budget.max_decoded_bytes {
            return Err(budget_error());
        }
        let input_cap = usize::try_from(budget.max_decoded_bytes).unwrap_or(usize::MAX);
        let identifier = python_strip_unicode16_v1(identifier, input_cap).map_err(|_| {
            error(
                SearchV2ErrorCode::InvalidRequest,
                "inspect identifier exceeds character cap",
            )
        })?;
        if identifier.is_empty()
            || identifier.chars().count() > 4096
            || identifier.len() > budget.max_field_bytes
            || relation_limit > 1000
        {
            return Err(error(
                SearchV2ErrorCode::InvalidRequest,
                "invalid inspect identifier or relation limit",
            ));
        }
        Ok(Self {
            kind,
            identifier: identifier.to_owned(),
            relation_limit,
        })
    }
    pub fn from_json(value: &JsonValue, budget: InspectBudget) -> Result<Self, SearchV2Error> {
        let invalid = || error(SearchV2ErrorCode::InvalidRequest, "invalid inspect request");
        let kind = match value.object_get("kind").and_then(JsonValue::as_str) {
            Some("node") => SearchKind::Nodes,
            Some("relation") => SearchKind::Relations,
            _ => return Err(invalid()),
        };
        let Some(JsonValue::String(identifier)) = value.object_get("identifier") else {
            return Err(invalid());
        };
        let Some(JsonValue::Number(limit)) = value.object_get("relation_limit") else {
            return Err(invalid());
        };
        if limit.kind != JsonNumberKind::Int {
            return Err(invalid());
        }
        let relation_limit = usize::try_from(limit.lexeme.parse::<i64>().map_err(|_| invalid())?)
            .map_err(|_| invalid())?;
        if relation_limit > 1000 {
            return Err(invalid());
        }
        if let Some(identifier) = identifier.as_str() {
            return Self::new(kind, identifier, relation_limit, budget);
        }
        // Match strip-before-length even for retained escaped lone surrogates.
        // Whitespace scalars use the same owner-pinned Unicode primitive.
        let whitespace = |unit: u16| {
            let Some(point) = char::from_u32(u32::from(unit)) else {
                return false;
            };
            let mut bytes = [0; 4];
            python_strip_unicode16_v1(point.encode_utf8(&mut bytes), 1).is_ok_and(str::is_empty)
        };
        if identifier.units().len() as u64 > budget.max_decoded_bytes {
            return Err(budget_error());
        }
        let mut units = identifier.units();
        while units.first().is_some_and(|u| whitespace(*u)) {
            units = &units[1..];
        }
        while units.last().is_some_and(|u| whitespace(*u)) {
            units = &units[..units.len() - 1];
        }
        let length = std::char::decode_utf16(units.iter().copied()).count();
        if length == 0 || length > 4096 {
            return Err(invalid());
        }
        // SQLite cannot address WTF-16: never select a replacement identifier.
        Err(corrupt())
    }
}
pub fn validate_inspect_request(
    value: &JsonValue,
    budget: InspectBudget,
) -> Result<(), SearchV2Error> {
    InspectRequest::from_json(value, budget).map(|_| ())
}
#[derive(Clone, Debug)]
pub enum InspectNeed {
    Lookup {
        kind: SearchKind,
        selector: &'static str,
        identifier: String,
        limit: usize,
    },
    NodeIncident {
        ids: Vec<String>,
        relation_limit: usize,
    },
    RelationEndpoints {
        ids: Vec<String>,
    },
}
pub struct InspectPlan {
    request: InspectRequest,
    revision: String,
    authority: JsonValue,
    budget: InspectBudget,
    need: Option<InspectNeed>,
    matches: Vec<JsonValue>,
    context: Vec<JsonValue>,
    field: &'static str,
    total: u64,
    rows: u64,
    decoded: u64,
    work: u64,
}
impl InspectPlan {
    pub fn new(
        request: InspectRequest,
        revision: String,
        authority_boundary: JsonValue,
        budget: InspectBudget,
    ) -> Result<Self, SearchV2Error> {
        if budget.max_matches == 0
            || budget.max_rows == 0
            || budget.max_read_vm_steps == 0
            || budget.max_payload_bytes == 0
            || budget.max_decoded_bytes == 0
            || budget.max_response_bytes == 0
        {
            return Err(budget_error());
        }
        let request = InspectRequest::new(
            request.kind,
            &request.identifier,
            request.relation_limit,
            budget,
        )?;
        let need = Some(InspectNeed::Lookup {
            kind: request.kind,
            selector: "id",
            identifier: request.identifier.clone(),
            limit: 1,
        });
        Ok(Self {
            request,
            revision,
            authority: authority_boundary,
            budget,
            need,
            matches: vec![],
            context: vec![],
            field: "id",
            total: 0,
            rows: 0,
            decoded: 0,
            work: 0,
        })
    }
    pub fn need(&self) -> Option<&InspectNeed> {
        self.need.as_ref()
    }
    fn charge(
        &mut self,
        values: &[JsonValue],
        bytes: u64,
        probe: Option<&dyn AbortProbe>,
    ) -> Result<(), SearchV2Error> {
        check(probe)?;
        self.rows = self
            .rows
            .checked_add(values.len() as u64)
            .ok_or_else(budget_error)?;
        self.decoded = self.decoded.checked_add(bytes).ok_or_else(budget_error)?;
        if self.rows > self.budget.max_rows || self.decoded > self.budget.max_decoded_bytes {
            return Err(budget_error());
        }
        let mut pending: Vec<&JsonValue> = values.iter().collect();
        while let Some(value) = pending.pop() {
            self.work = self.work.checked_add(1).ok_or_else(budget_error)?;
            if self.work > self.budget.max_read_vm_steps {
                return Err(budget_error());
            }
            check(probe)?;
            match value {
                JsonValue::Array(values) => pending.extend(values),
                JsonValue::Object(values) => pending.extend(values.iter().map(|(_, v)| v)),
                _ => {}
            }
        }
        Ok(())
    }
    fn ids(values: &[JsonValue]) -> Result<Vec<String>, SearchV2Error> {
        let mut seen = BTreeSet::new();
        values
            .iter()
            .map(|v| {
                let id = v
                    .object_get("id")
                    .and_then(JsonValue::as_str)
                    .ok_or_else(corrupt)?
                    .to_owned();
                if !seen.insert(id.clone()) {
                    return Err(corrupt());
                }
                Ok(id)
            })
            .collect()
    }
    pub fn resume_lookup(
        &mut self,
        rows: Vec<JsonValue>,
        decoded_bytes: u64,
        probe: Option<&dyn AbortProbe>,
    ) -> Result<(), SearchV2Error> {
        let Some(InspectNeed::Lookup {
            kind,
            selector,
            identifier,
            limit,
        }) = self.need.clone()
        else {
            return Err(corrupt());
        };
        if rows.len() > limit {
            return Err(if selector == "id" {
                corrupt()
            } else {
                budget_error()
            });
        }
        self.charge(&rows, decoded_bytes, probe)?;
        Self::ids(&rows)?;
        if rows.iter().any(|v| {
            v.object_get(selector).and_then(JsonValue::as_str) != Some(identifier.as_str())
        }) {
            return Err(corrupt());
        }
        if rows.is_empty() {
            let next = match selector {
                "id" if kind == SearchKind::Nodes => "entity_id",
                "id" | "entity_id" => "native_id",
                _ => {
                    return Err(error(
                        SearchV2ErrorCode::UnknownIdentifier,
                        "unknown ToS knowledge identifier",
                    ));
                }
            };
            self.need = Some(InspectNeed::Lookup {
                kind,
                selector: next,
                identifier,
                limit: self.budget.max_matches,
            });
            return Ok(());
        }
        self.field = selector;
        let ids = Self::ids(&rows)?;
        self.matches = rows;
        self.need = Some(if kind == SearchKind::Nodes {
            InspectNeed::NodeIncident {
                ids,
                relation_limit: self.request.relation_limit,
            }
        } else {
            let mut endpoints = BTreeSet::new();
            for row in &self.matches {
                for key in ["from_id", "to_id"] {
                    endpoints.insert(
                        row.object_get(key)
                            .and_then(JsonValue::as_str)
                            .ok_or_else(corrupt)?
                            .to_owned(),
                    );
                }
            }
            InspectNeed::RelationEndpoints {
                ids: endpoints.into_iter().collect(),
            }
        });
        Ok(())
    }
    pub fn resume_incident(
        &mut self,
        total: u64,
        rows: Vec<JsonValue>,
        decoded_bytes: u64,
        probe: Option<&dyn AbortProbe>,
    ) -> Result<(), SearchV2Error> {
        let Some(InspectNeed::NodeIncident {
            ids,
            relation_limit,
        }) = &self.need
        else {
            return Err(corrupt());
        };
        let ids: BTreeSet<_> = ids.iter().cloned().collect();
        if rows.len() as u64 != total.min(*relation_limit as u64) {
            return Err(corrupt());
        }
        let selected = Self::ids(&rows)?;
        if selected.windows(2).any(|w| w[0] >= w[1])
            || rows.iter().any(|row| {
                let from = row.object_get("from_id").and_then(JsonValue::as_str);
                let to = row.object_get("to_id").and_then(JsonValue::as_str);
                from.is_none()
                    || to.is_none()
                    || !(ids.contains(from.unwrap()) || ids.contains(to.unwrap()))
            })
        {
            return Err(corrupt());
        }
        self.charge(&rows, decoded_bytes, probe)?;
        self.total = total;
        self.context = rows;
        self.need = None;
        Ok(())
    }
    pub fn resume_endpoints(
        &mut self,
        rows: Vec<JsonValue>,
        decoded_bytes: u64,
        probe: Option<&dyn AbortProbe>,
    ) -> Result<(), SearchV2Error> {
        let Some(InspectNeed::RelationEndpoints { ids }) = &self.need else {
            return Err(corrupt());
        };
        if Self::ids(&rows)? != *ids {
            return Err(corrupt());
        }
        self.charge(&rows, decoded_bytes, probe)?;
        self.total = rows.len() as u64;
        self.context = rows;
        self.need = None;
        Ok(())
    }
    pub fn into_packet(self) -> Result<JsonValue, SearchV2Error> {
        if self.need.is_some() {
            return Err(corrupt());
        }
        let items: Vec<_> = self.matches.iter().chain(&self.context).cloned().collect();
        let mut refs = BTreeSet::new();
        for item in &items {
            if let Some(JsonValue::Array(values)) = item.object_get("source_refs") {
                for value in values {
                    if let Some(value) = value.as_str().filter(|v| !v.is_empty()) {
                        refs.insert(value.to_owned());
                    }
                }
            }
        }
        let node = self.request.kind == SearchKind::Nodes;
        let mut fields = vec![
            (
                "schema",
                text(if node {
                    "tos_knowledge_node_packet_v1"
                } else {
                    "tos_knowledge_relation_packet_v1"
                }),
            ),
            ("source_revision", text(&self.revision)),
            ("requested_id", text(&self.request.identifier)),
            (
                "ambiguous_native_id",
                JsonValue::Bool(self.field == "native_id" && self.matches.len() > 1),
            ),
        ];
        if node {
            fields.push((
                "shared_entity_id",
                JsonValue::Bool(self.field == "entity_id" && self.matches.len() > 1),
            ));
        }
        let count = self.matches.len() as u64;
        fields.push(("matches", JsonValue::Array(self.matches)));
        let returned = self.context.len() as u64;
        fields.push((
            if node {
                "related_relations"
            } else {
                "endpoints"
            },
            JsonValue::Array(self.context),
        ));
        fields.push((
            "counts",
            object(if node {
                vec![
                    ("matches", number(count)),
                    ("related_relations", number(self.total)),
                    ("returned_relations", number(returned)),
                ]
            } else {
                vec![
                    ("matches", number(count)),
                    ("endpoints", number(self.total)),
                ]
            }),
        ));
        fields.push((
            "source_refs",
            JsonValue::Array(refs.into_iter().map(|v| text(&v)).collect()),
        ));
        fields.push(("authority_boundary", self.authority));
        fields.push((
            "source_read_targets",
            source_read_targets(&items, &self.revision, self.budget.json),
        ));
        Ok(object(fields))
    }
}
