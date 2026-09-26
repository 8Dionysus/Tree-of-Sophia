//! Complete bounded inspect packets on a cold-admitted CMP selection.
//! Alias sets are complete or refused; no prefix stands in for all matches.
use crate::{
    knowledge_binding::BoundCmpKnowledge,
    knowledge_packet::IndexedDisclosureScope,
    search_v2::{CurrentPolicyBinding, SearchKind, SearchV2Error, SearchV2ErrorCode},
    source_read_projection::{object, source_read_targets, text},
};
use rusqlite::{ErrorCode, params};
use std::{
    collections::BTreeSet,
    ops::Deref,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use tos_compiler::VerifiedKnowledgeModel;
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonMode, JsonNumber, JsonNumberKind, JsonValue,
    canonical_bytes_v1, parse_json,
};

pub const NODE_INSPECT_OPERATION: &str = "tos.knowledge.node";
pub const RELATION_INSPECT_OPERATION: &str = "tos.knowledge.relation";
pub const INSPECT_INTENDED_USE: &str = "read_only_public_knowledge_inspect_v1";
fn error(code: SearchV2ErrorCode, message: &'static str) -> SearchV2Error {
    SearchV2Error { code, message }
}
fn corrupt(message: &'static str) -> SearchV2Error {
    error(SearchV2ErrorCode::CorruptSelectedCarrier, message)
}
fn budget_error() -> SearchV2Error {
    error(SearchV2ErrorCode::BudgetExceeded, "inspect budget exceeded")
}
fn sql_error(reason: rusqlite::Error) -> SearchV2Error {
    if matches!(reason,rusqlite::Error::SqliteFailure(failure,_) if failure.code==ErrorCode::OperationInterrupted)
    {
        budget_error()
    } else {
        corrupt("selected inspect query failed")
    }
}
fn number(value: u64) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: value.to_string(),
    })
}

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
/// A retained row's authenticated full projection. Authority includes every
/// consulted carrier, including endpoint/context carriers and every alias.
pub struct InspectedCarrier {
    pub kind: SearchKind,
    pub id: String,
    pub position: u64,
    pub payload_sha256: Digest256,
    pub payload: JsonValue,
}
pub trait InspectDisclosureLease: Send {
    fn recheck(&mut self) -> Result<(), SearchV2Error>;
}
pub trait InspectCurrentAuthority {
    fn policy_binding(&self) -> CurrentPolicyBinding;
    fn disclosure_scope(&self) -> IndexedDisclosureScope;
    fn check_selected(&mut self) -> Result<(), SearchV2Error>;
    fn authorize_current(&mut self, carrier: &InspectedCarrier) -> Result<(), SearchV2Error>;
    fn acquire_disclosure(
        &mut self,
        scope: &IndexedDisclosureScope,
        consulted: &[InspectedCarrier],
    ) -> Result<Box<dyn InspectDisclosureLease>, SearchV2Error>;
}
pub struct DisclosableInspect {
    body: Vec<u8>,
    lease: Box<dyn InspectDisclosureLease>,
}
impl Deref for DisclosableInspect {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.body
    }
}
impl DisclosableInspect {
    pub fn recheck(&mut self) -> Result<(), SearchV2Error> {
        self.lease.recheck()
    }
}

struct Reader<'a, 'b, A: ?Sized> {
    model: &'a mut VerifiedKnowledgeModel<'b>,
    authority: &'a mut A,
    budget: InspectBudget,
    decoded: u64,
    rows: u64,
    consulted: Vec<InspectedCarrier>,
}
impl<A: InspectCurrentAuthority + ?Sized> Reader<'_, '_, A> {
    /// SQL CASE enforces field/payload transfer caps before row allocation.
    fn items(
        &mut self,
        kind: SearchKind,
        field: &str,
        id: &str,
        limit: usize,
        ordered_id: bool,
    ) -> Result<Vec<JsonValue>, SearchV2Error> {
        if limit == 0 {
            return Ok(vec![]);
        }
        self.authority.check_selected()?;
        self.model.check_pin().map_err(|_| {
            error(
                SearchV2ErrorCode::StaleSelection,
                "inspect selection changed",
            )
        })?;
        let table = if kind == SearchKind::Nodes {
            "knowledge_nodes"
        } else {
            "knowledge_relations"
        };
        let index = match (kind, field) {
            (_, "id") => "",
            (SearchKind::Nodes, "entity_id") => " INDEXED BY knowledge_nodes_entity",
            (SearchKind::Nodes, "native_id") => " INDEXED BY knowledge_nodes_native",
            (SearchKind::Relations, "native_id") => " INDEXED BY knowledge_relations_native",
            _ => return Err(corrupt("invalid internal inspect selector")),
        };
        let order = if ordered_id { "id" } else { "source_order" };
        let sql=format!("SELECT CASE WHEN typeof(id)='text' AND length(CAST(id AS BLOB))<=?2 THEN id END,source_order,
            CASE WHEN typeof(payload_len)='integer' AND payload_len BETWEEN 0 AND ?3 THEN payload_len END,
            CASE WHEN typeof(payload_sha256)='blob' AND length(payload_sha256)=32 THEN payload_sha256 END,
            CASE WHEN typeof(payload)='blob' AND payload_len BETWEEN 0 AND ?3 AND length(payload)=payload_len THEN payload END
            FROM {table}{index} WHERE {field}=?1 ORDER BY {order} LIMIT ?4");
        let mut statement = self
            .model
            .connection()
            .prepare_cached(&sql)
            .map_err(sql_error)?;
        let mut rows = statement
            .query(params![
                id,
                self.budget.max_field_bytes as i64,
                self.budget.max_payload_bytes as i64,
                limit as i64
            ])
            .map_err(sql_error)?;
        let mut values = vec![];
        while let Some(row) = rows.next().map_err(sql_error)? {
            self.rows = self.rows.checked_add(1).ok_or_else(budget_error)?;
            if self.rows > self.budget.max_rows {
                return Err(budget_error());
            }
            let Some(row_id) = row.get::<_, Option<String>>(0).map_err(sql_error)? else {
                return Err(budget_error());
            };
            let position = row.get::<_, i64>(1).map_err(sql_error)?;
            if position < 0 {
                return Err(corrupt("inspect source order invalid"));
            }
            let Some(length) = row.get::<_, Option<i64>>(2).map_err(sql_error)? else {
                return Err(budget_error());
            };
            let Some(sha) = row.get::<_, Option<Vec<u8>>>(3).map_err(sql_error)? else {
                return Err(corrupt("inspect carrier digest width invalid"));
            };
            // Prevent aggregate transfer allocation, not merely post-read cap.
            self.decoded = self
                .decoded
                .checked_add(row_id.len() as u64 + 48 + length as u64)
                .ok_or_else(budget_error)?;
            if self.decoded > self.budget.max_decoded_bytes {
                return Err(budget_error());
            }
            let Some(payload) = row.get::<_, Option<Vec<u8>>>(4).map_err(sql_error)? else {
                return Err(corrupt("inspect carrier length/type differs"));
            };
            let payload_sha = Digest256::of_bytes(&payload);
            if payload.len() != length as usize || payload_sha.as_bytes() != sha.as_slice() {
                return Err(corrupt("inspect carrier digest differs"));
            }
            let mut limits = self.budget.json;
            limits.max_bytes = limits.max_bytes.min(payload.len());
            let parsed = parse_json(&payload, JsonMode::PublishedStrict, limits)
                .map_err(|_| corrupt("inspect carrier JSON invalid"))?;
            let value = parsed.root().clone();
            if value.object_get("id").and_then(JsonValue::as_str) != Some(row_id.as_str()) {
                return Err(corrupt("inspect carrier ID mirror differs"));
            }
            let carrier = InspectedCarrier {
                kind,
                id: row_id,
                position: position as u64,
                payload_sha256: payload_sha,
                payload: value.clone(),
            };
            self.authority.authorize_current(&carrier)?;
            self.consulted.push(carrier);
            values.push(value);
        }
        Ok(values)
    }
    fn resolve(
        &mut self,
        kind: SearchKind,
        id: &str,
    ) -> Result<(&'static str, Vec<JsonValue>), SearchV2Error> {
        let exact = self.items(kind, "id", id, 1, false)?;
        if !exact.is_empty() {
            return Ok(("id", exact));
        }
        if kind == SearchKind::Nodes {
            let entity = self.items(kind, "entity_id", id, self.budget.max_matches + 1, false)?;
            if entity.len() > self.budget.max_matches {
                return Err(budget_error());
            }
            if !entity.is_empty() {
                return Ok(("entity_id", entity));
            }
        }
        let native = self.items(kind, "native_id", id, self.budget.max_matches + 1, false)?;
        if native.len() > self.budget.max_matches {
            return Err(budget_error());
        }
        if native.is_empty() {
            return Err(error(
                SearchV2ErrorCode::InvalidRequest,
                "unknown ToS knowledge identifier",
            ));
        }
        Ok(("native_id", native))
    }
    fn incident(
        &mut self,
        matches: &[JsonValue],
        limit: usize,
    ) -> Result<(u64, Vec<JsonValue>), SearchV2Error> {
        let ids = JsonValue::Array(
            matches
                .iter()
                .map(|value| value.object_get("id").expect("verified id").clone())
                .collect(),
        );
        let encoded = canonical_bytes_v1(
            &ids,
            CanonicalProfile::SourceRecordDigestV1,
            self.budget.json,
        )
        .map_err(|_| budget_error())?;
        let encoded = std::str::from_utf8(&encoded).map_err(|_| corrupt("inspect IDs invalid"))?;
        let union = "SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_from WHERE from_id IN (SELECT value FROM json_each(?1)) UNION SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_to WHERE to_id IN (SELECT value FROM json_each(?1))";
        let total: i64 = self
            .model
            .connection()
            .query_row(
                &format!("SELECT count(*) FROM ({union})"),
                [encoded],
                |row| row.get(0),
            )
            .map_err(sql_error)?;
        if total < 0 {
            return Err(corrupt("inspect incident count invalid"));
        }
        let selected = {
            let mut statement=self.model.connection().prepare_cached(&format!("SELECT CASE WHEN length(CAST(id AS BLOB))<=?3 THEN id END FROM ({union}) ORDER BY id LIMIT ?2")).map_err(sql_error)?;
            statement
                .query_map(
                    params![encoded, limit as i64, self.budget.max_field_bytes as i64],
                    |row| row.get::<_, Option<String>>(0),
                )
                .map_err(sql_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(sql_error)?
        };
        let mut related = vec![];
        for id in selected {
            let id = id.ok_or_else(budget_error)?;
            let rows = self.items(SearchKind::Relations, "id", &id, 1, true)?;
            if rows.len() != 1 {
                return Err(corrupt("inspect incident carrier missing"));
            }
            related.extend(rows);
        }
        Ok((total as u64, related))
    }
}

/// Exact legacy packet semantics, with selected-row closure and current held
/// disclosure. Caller budgets may refuse an expensive complete packet.
pub fn execute_selected_inspect<A: InspectCurrentAuthority + ?Sized>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    kind: SearchKind,
    identifier: &str,
    relation_limit: usize,
    budget: InspectBudget,
) -> Result<DisclosableInspect, SearchV2Error> {
    let identifier = identifier.trim();
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
    if budget.max_open_vm_steps == 0
        || model.open_vm_steps() > budget.max_open_vm_steps
        || budget.max_read_vm_steps == 0
        || budget.max_matches == 0
        || budget.max_matches >= i64::MAX as usize
        || budget.max_rows == 0
        || budget.max_payload_bytes == 0
        || budget.max_payload_bytes > i64::MAX as usize
        || budget.max_field_bytes == 0
        || budget.max_field_bytes > i64::MAX as usize
        || budget.max_response_bytes == 0
        || budget.max_decoded_bytes == 0
    {
        return Err(budget_error());
    }
    bound.check_model(model)?;
    let policy = authority.policy_binding();
    let scope = authority.disclosure_scope();
    let operation = if kind == SearchKind::Nodes {
        NODE_INSPECT_OPERATION
    } else {
        RELATION_INSPECT_OPERATION
    };
    scope.validate_for(bound, &policy, operation, INSPECT_INTENDED_USE)?;
    authority.check_selected()?;
    let steps = Arc::new(AtomicU64::new(0));
    let observed = Arc::clone(&steps);
    let cap = budget.max_read_vm_steps;
    model.connection().progress_handler(
        1,
        Some(move || observed.fetch_add(1, Ordering::Relaxed) >= cap),
    );
    let result = (|| {
        let mut read = Reader {
            model,
            authority,
            budget,
            decoded: 0,
            rows: 0,
            consulted: vec![],
        };
        let (field, matches) = read.resolve(kind, identifier)?;
        let (context, count) = if kind == SearchKind::Nodes {
            let (total, related) = read.incident(&matches, relation_limit)?;
            (related, total)
        } else {
            let mut ids = BTreeSet::new();
            for value in &matches {
                for field in ["from_id", "to_id"] {
                    ids.insert(
                        value
                            .object_get(field)
                            .and_then(JsonValue::as_str)
                            .ok_or_else(|| corrupt("inspect relation endpoint invalid"))?
                            .to_owned(),
                    );
                }
            }
            let mut endpoints = vec![];
            for id in ids {
                let values = read.items(SearchKind::Nodes, "id", &id, 1, true)?;
                if values.len() != 1 {
                    return Err(corrupt("inspect relation endpoint closure incomplete"));
                }
                endpoints.extend(values);
            }
            let count = endpoints.len() as u64;
            (endpoints, count)
        };
        let items: Vec<_> = matches.iter().chain(&context).cloned().collect();
        let mut refs = BTreeSet::new();
        for item in &items {
            if let Some(JsonValue::Array(values)) = item.object_get("source_refs") {
                for value in values {
                    if let Some(value) = value.as_str().filter(|value| !value.is_empty()) {
                        refs.insert(value.to_owned());
                    }
                }
            }
        }
        let authority_boundary = parse_json(
            bound.authority_boundary().as_bytes(),
            JsonMode::PublishedStrict,
            budget.json,
        )
        .map_err(|_| corrupt("inspect authority boundary invalid"))?
        .root()
        .clone();
        let mut fields = vec![
            (
                "schema",
                text(if kind == SearchKind::Nodes {
                    "tos_knowledge_node_packet_v1"
                } else {
                    "tos_knowledge_relation_packet_v1"
                }),
            ),
            ("source_revision", text(bound.source_revision())),
            ("requested_id", text(identifier)),
            (
                "ambiguous_native_id",
                JsonValue::Bool(field == "native_id" && matches.len() > 1),
            ),
            ("matches", JsonValue::Array(matches.clone())),
            (
                "source_refs",
                JsonValue::Array(refs.into_iter().map(|value| text(&value)).collect()),
            ),
            (
                "source_read_targets",
                source_read_targets(&items, bound.source_revision(), budget.json),
            ),
            ("authority_boundary", authority_boundary),
        ];
        if kind == SearchKind::Nodes {
            fields.extend([
                (
                    "shared_entity_id",
                    JsonValue::Bool(field == "entity_id" && matches.len() > 1),
                ),
                (
                    "counts",
                    object(vec![
                        ("matches", number(matches.len() as u64)),
                        ("related_relations", number(count)),
                        ("returned_relations", number(context.len() as u64)),
                    ]),
                ),
                ("related_relations", JsonValue::Array(context)),
            ]);
        } else {
            fields.extend([
                (
                    "counts",
                    object(vec![
                        ("matches", number(matches.len() as u64)),
                        ("endpoints", number(count)),
                    ]),
                ),
                ("endpoints", JsonValue::Array(context)),
            ]);
        }
        let mut limits = budget.json;
        limits.max_bytes = limits.max_bytes.min(budget.max_response_bytes);
        let body = canonical_bytes_v1(
            &object(fields),
            CanonicalProfile::SourceRecordDigestV1,
            limits,
        )
        .map_err(|_| budget_error())?;
        read.authority.check_selected()?;
        bound.check_model(read.model)?;
        let mut lease = read.authority.acquire_disclosure(&scope, &read.consulted)?;
        lease.recheck()?;
        Ok(DisclosableInspect { body, lease })
    })();
    model.connection().progress_handler(0, None::<fn() -> bool>);
    if steps.load(Ordering::Relaxed) > budget.max_read_vm_steps {
        return Err(budget_error());
    }
    result
}
