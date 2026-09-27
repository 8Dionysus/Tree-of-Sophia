//! Software-owned exploration contracts. Data roots cannot replace these bytes;
//! runtime metadata describes the selected executor, not a disclosure grant.
use crate::{AccessError, AccessErrorCode, AccessExecutor, DisclosureFence, PreparedPacket};
use tos_foundation::{
    CanonicalProfile, JsonLimits, JsonMode, JsonNumber, JsonNumberKind, JsonString, JsonValue,
    canonical_bytes_v1, parse_json,
};

pub const OPERATION: &str = "tos_knowledge_exploration_contracts";
pub const CONTRACTS: &[(&str, &[u8])] = &[
    (
        "request",
        include_bytes!("../../../../access/contracts/exploration-request.v1.schema.json"),
    ),
    (
        "result",
        include_bytes!("../../../../access/contracts/exploration-result.v1.schema.json"),
    ),
    (
        "request_v2",
        include_bytes!("../../../../access/contracts/exploration-request.v2.schema.json"),
    ),
    (
        "result_v2",
        include_bytes!("../../../../access/contracts/exploration-result.v2.schema.json"),
    ),
];
fn text(value: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(value))
}
fn number(value: u64) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: value.to_string(),
    })
}
fn object(fields: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        fields
            .into_iter()
            .map(|(key, value)| (JsonString::from_utf8(key), value))
            .collect(),
    )
}
fn texts(values: &[&str]) -> JsonValue {
    JsonValue::Array(values.iter().map(|value| text(value)).collect())
}

/// None means there is no actual selected exploration backend. The available
/// profile is read from its existing store and query budget, never schema presence.
pub fn runtime_capabilities(
    selected: Option<(
        crate::exploration_checkpoints::CheckpointLimits,
        tos_query::knowledge_exploration::ExplorationBudget,
    )>,
) -> JsonValue {
    let available = selected.is_some();
    let (ttl, checkpoints, bytes, work, nodes, relations) = selected
        .map(|(store, query)| {
            (
                store.ttl.as_secs(),
                store.max_entries as u64,
                store.max_encoded_bytes as u64,
                query.max_work_units as u64,
                query.max_session_nodes as u64,
                query.max_session_relations as u64,
            )
        })
        .unwrap_or((0, 0, 0, 0, 0, 0));
    object(vec![
        ("schema", text("tos_exploration_capabilities_v1")),
        ("available", JsonValue::Bool(available)),
        (
            "execution_version",
            text(tos_query::knowledge_exploration::EXPLORATION_EXECUTION_VERSION),
        ),
        (
            "http",
            object(vec![
                ("method", text("POST")),
                ("path", text("/api/knowledge/explore")),
            ]),
        ),
        (
            "storage",
            text(if available {
                "bounded-process-memory"
            } else {
                "unavailable"
            }),
        ),
        ("ttl_seconds", number(ttl)),
        ("max_checkpoints", number(checkpoints)),
        ("max_checkpoint_bytes", number(bytes)),
        (
            "limits",
            object(vec![
                ("depth", number(if available { 10 } else { 0 })),
                ("page_nodes", number(if available { 100 } else { 0 })),
                ("page_relations", number(if available { 100 } else { 0 })),
                ("work_per_page", number(work)),
                ("session_nodes", number(nodes)),
                ("session_relations", number(relations)),
            ]),
        ),
        ("restart_survival", JsonValue::Bool(false)),
        ("writes_to_tree", JsonValue::Bool(false)),
        (
            "continuation",
            text("opaque-cursor-only; fixed query and page sizes"),
        ),
        (
            "ordering",
            text(
                "zero-distance declared identity carriers before relation-id ordered edges; all profile uses carrier BFS",
            ),
        ),
        (
            "identity_expansion",
            text(
                "overview only; source-filtered declared tos.* IDs; page and session node budgets apply",
            ),
        ),
        ("runtime", text("local")),
        ("other_runtimes", text("discover-on-target")),
        (
            "request_versions",
            texts(&["tos_exploration_request_v1", "tos_exploration_request_v2"]),
        ),
        (
            "result_versions",
            texts(&["tos_exploration_result_v1", "tos_exploration_result_v2"]),
        ),
        ("v2_origin_kinds", texts(&["node", "relation"])),
        (
            "v2_origin_context",
            object(vec![
                ("max_nodes", number(2)),
                ("max_relations", number(1)),
                (
                    "page_budgets",
                    text("incremental; mandatory origin closure is additional"),
                ),
            ]),
        ),
    ])
}

pub fn execute(
    executor: &dyn AccessExecutor,
    max_bytes: usize,
) -> Result<PreparedPacket<'static>, AccessError> {
    let limits = JsonLimits {
        max_bytes,
        ..JsonLimits::default()
    };
    let mut fields = vec![(
        JsonString::from_utf8("capabilities"),
        executor.exploration_runtime_capabilities(),
    )];
    let mut admitted = 0usize;
    for (name, raw) in CONTRACTS {
        admitted = admitted.checked_add(raw.len()).ok_or_else(|| {
            AccessError::new(
                AccessErrorCode::BudgetExceeded,
                "software contract byte budget exceeded",
            )
        })?;
        if admitted > max_bytes {
            return Err(AccessError::new(
                AccessErrorCode::BudgetExceeded,
                "software contract byte budget exceeded",
            ));
        }
        let schema = parse_json(raw, JsonMode::PublishedStrict, limits)
            .map_err(|_| {
                AccessError::new(
                    AccessErrorCode::Unavailable,
                    "packaged exploration contract invalid",
                )
            })?
            .into_root();
        fields.push((JsonString::from_utf8(name), schema));
    }
    let body = canonical_bytes_v1(
        &JsonValue::Object(fields),
        CanonicalProfile::SourceRecordDigestV1,
        limits,
    )
    .map_err(|_| {
        AccessError::new(
            AccessErrorCode::BudgetExceeded,
            "software contract packet byte budget exceeded",
        )
    })?;
    // Only software bytes and a capability snapshot are disclosed. There is no
    // source-policy lease here; transport supplies its existing AbortFence.
    Ok(PreparedPacket {
        body,
        fence: Box::new(SoftwareFence),
    })
}
struct SoftwareFence;
impl DisclosureFence for SoftwareFence {
    fn recheck(&mut self) -> Result<(), AccessError> {
        Ok(())
    }
}
