//! Resumable bounded selected-knowledge BFS. Checkpoints contain IDs and
//! query state only; the adapter owns disposable storage, expiry and replay.
use crate::{
    exploration_plan::{ExplorationProfile, Rows},
    inspect_plan::InspectBudget,
    knowledge_lens_spec::{LensVocabulary, array, boolean, get, number, set, string, uint},
    knowledge_presentation::{knowledge_scene, lens_carrier},
    search_v2::{SearchKind, SearchV2Error, SearchV2ErrorCode},
    source_read_projection::{object, text},
};
#[cfg(not(target_arch = "wasm32"))]
use crate::{
    knowledge_binding::BoundCmpKnowledge,
    knowledge_inspect::{
        DisclosableInspect, InspectCurrentAuthority, Reader, execute_selected_carrier_packet,
    },
};
use std::collections::{BTreeSet, VecDeque};
#[cfg(not(target_arch = "wasm32"))]
use tos_compiler::VerifiedKnowledgeModel;
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonMode, JsonValue, canonical_bytes_v1, parse_json,
    python_strip_unicode16_v1,
};

pub const EXPLORATION_OPERATION: &str = "tos.knowledge.explore";
pub const EXPLORATION_INTENDED_USE: &str = "read_only_public_knowledge_exploration_v1";
pub const PUBLISHED_EXPLORATION_EXECUTION_VERSION: &str = "tos-exploration-d1-execution-v6";
pub const PUBLISHED_EXPLORATION_CACHE_VERSION: &str =
    "tos-exploration-d1-execution-v6/rust-state-v1";
pub const EXPLORATION_EXECUTION_VERSION: &str = "tos-exploration-execution-v6";
#[cfg(not(target_arch = "wasm32"))]
const SNAPSHOT_SCHEMA: &str = "tos_rust_selected_exploration_snapshot_v1";
fn error(code: SearchV2ErrorCode, message: &'static str) -> SearchV2Error {
    SearchV2Error { code, message }
}
fn invalid(message: &'static str) -> SearchV2Error {
    error(SearchV2ErrorCode::InvalidRequest, message)
}
fn corrupt(message: &'static str) -> SearchV2Error {
    error(SearchV2ErrorCode::CorruptSelectedCarrier, message)
}
pub(crate) fn bare(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn exact_id(value: &JsonValue) -> bool {
    value.as_str().is_some_and(|id| {
        !id.is_empty() && python_strip_unicode16_v1(id, 1024).is_ok_and(|trimmed| trimmed == id)
    })
}
fn int(value: &JsonValue) -> Option<usize> {
    let JsonValue::Number(n) = value else {
        return None;
    };
    if n.kind != tos_foundation::JsonNumberKind::Int {
        return None;
    }
    n.lexeme.parse().ok()
}
fn names(value: &JsonValue) -> Result<Vec<String>, SearchV2Error> {
    let values = value
        .as_array()
        .ok_or_else(|| invalid("exploration selector must be array"))?;
    values
        .iter()
        .map(|v| {
            v.as_str()
                .filter(|v| !v.is_empty() && v.chars().count() <= 1024)
                .map(str::to_owned)
                .ok_or_else(|| invalid("invalid exploration selector"))
        })
        .collect()
}
pub fn normalize_exploration(
    request: &JsonValue,
    vocabulary: &LensVocabulary,
) -> Result<JsonValue, SearchV2Error> {
    let fields = request
        .as_object()
        .ok_or_else(|| invalid("exploration request must be object"))?;
    let v2 = get(request, "schema_version").as_str() == Some("tos_exploration_request_v2");
    let allowed = if v2 {
        vec![
            "schema_version",
            "source_revision",
            "origin",
            "sources",
            "direction",
            "predicate_ids",
            "profile",
            "max_depth",
            "page_nodes",
            "page_relations",
        ]
    } else {
        vec![
            "focus_node_id",
            "sources",
            "direction",
            "predicate_ids",
            "profile",
            "max_depth",
            "page_nodes",
            "page_relations",
        ]
    };
    if fields
        .iter()
        .any(|(k, _)| !allowed.contains(&k.as_str().unwrap_or("")))
    {
        return Err(invalid("unknown exploration fields"));
    }
    let mut result = if v2 {
        let origin = get(request, "origin");
        if !get(request, "source_revision").as_str().is_some_and(bare)
            || !origin.as_object().is_some_and(|f| f.len() == 3)
            || !matches!(get(origin, "kind").as_str(), Some("node" | "relation"))
            || !exact_id(get(origin, "id"))
            || !get(origin, "content_revision").as_str().is_some_and(bare)
        {
            return Err(invalid("invalid exact exploration origin"));
        }
        object(vec![
            ("schema_version", text("tos_exploration_request_v2")),
            ("source_revision", get(request, "source_revision").clone()),
            (
                "origin",
                object(vec![
                    ("kind", get(origin, "kind").clone()),
                    ("id", get(origin, "id").clone()),
                    ("content_revision", get(origin, "content_revision").clone()),
                ]),
            ),
        ])
    } else {
        let focus = get(request, "focus_node_id")
            .as_str()
            .filter(|id| {
                id.chars().count() <= 1024
                    && python_strip_unicode16_v1(id, 1024).is_ok_and(|v| !v.is_empty())
            })
            .ok_or_else(|| invalid("invalid exploration focus"))?;
        object(vec![("focus_node_id", text(focus))])
    };
    for (key, default, max, min) in [
        ("max_depth", 3, 10, 0),
        ("page_nodes", 40, 100, 1),
        ("page_relations", 80, 100, 1),
    ] {
        let n = match request.object_get(key) {
            None => default,
            Some(v) => int(v).ok_or_else(|| invalid("invalid exploration integer limit"))?,
        };
        if n < min || n > max {
            return Err(invalid("exploration limit out of range"));
        }
        set(&mut result, key, number(n));
    }
    for (key, default, allowed) in [
        (
            "direction",
            "either",
            vec!["either", "outgoing", "incoming"],
        ),
        ("profile", "overview", vec!["overview", "all"]),
    ] {
        let value = match request.object_get(key) {
            None => default,
            Some(v) => v
                .as_str()
                .ok_or_else(|| invalid("invalid exploration option"))?,
        };
        if !allowed.contains(&value) {
            return Err(invalid("unknown exploration option"));
        }
        set(&mut result, key, text(value));
    }
    for (key, default, max) in [
        (
            "sources",
            vocabulary.sources.clone(),
            vocabulary.sources.len(),
        ),
        ("predicate_ids", vec![], 100),
    ] {
        let values = match request.object_get(key) {
            None => default,
            Some(v) => names(v)?,
        };
        if values.len() > max
            || (key == "sources"
                && (values.is_empty() || values.iter().any(|s| !vocabulary.sources.contains(s))))
        {
            return Err(invalid("invalid exploration source/predicate scope"));
        }
        let unique: BTreeSet<_> = values.into_iter().collect();
        set(
            &mut result,
            key,
            JsonValue::Array(unique.into_iter().map(|v| text(&v)).collect()),
        );
    }
    Ok(result)
}

pub(crate) fn number64(value: u64) -> JsonValue {
    JsonValue::Number(tos_foundation::JsonNumber {
        kind: tos_foundation::JsonNumberKind::Int,
        lexeme: value.to_string(),
    })
}
/// Existing published snapshot identity, independent of native policy scopes.
pub fn published_exploration_snapshot(
    data_revision: &str,
    epoch: u64,
    limits: JsonLimits,
) -> Result<String, SearchV2Error> {
    if !bare(data_revision) || epoch > 9_007_199_254_740_991 {
        return Err(corrupt("exploration snapshot identity invalid"));
    }
    let binding = JsonValue::Array(vec![
        text(PUBLISHED_EXPLORATION_EXECUTION_VERSION),
        text(data_revision),
        number64(epoch),
    ]);
    Ok(Digest256::of_bytes(
        &canonical_bytes_v1(&binding, CanonicalProfile::SourceRecordDigestV1, limits).map_err(
            |_| {
                error(
                    SearchV2ErrorCode::BudgetExceeded,
                    "exploration snapshot byte budget exceeded",
                )
            },
        )?,
    )
    .to_hex())
}
// Move fields, including retained payload arrays, rather than copying carriers
// to change published packet framing. Native canonical emission is unchanged.
fn ordered(value: JsonValue, keys: &[&str]) -> JsonValue {
    let JsonValue::Object(mut fields) = value else {
        return value;
    };
    let mut result = vec![];
    for key in keys {
        if let Some(index) = fields
            .iter()
            .position(|(name, _)| name.as_str() == Some(*key))
        {
            result.push(fields.remove(index));
        }
    }
    result.extend(fields);
    JsonValue::Object(result)
}
pub(crate) fn normalize_for_profile(
    request: &JsonValue,
    vocabulary: &LensVocabulary,
    profile: ExplorationProfile,
) -> Result<JsonValue, SearchV2Error> {
    let normalized = normalize_exploration(request, vocabulary)?;
    Ok(if profile == ExplorationProfile::PublishedD1 {
        ordered(
            normalized,
            &[
                "schema_version",
                "source_revision",
                "origin",
                "focus_node_id",
                "sources",
                "direction",
                "profile",
                "predicate_ids",
                "max_depth",
                "page_nodes",
                "page_relations",
            ],
        )
    } else {
        normalized
    })
}
/// Shape/counter validation before any publication or checkpoint I/O. A cursor
/// is opaque and selects only disposable host state, never a source grant.
pub fn validate_published_exploration_request(
    request: &JsonValue,
) -> Result<Option<String>, SearchV2Error> {
    if request.object_get("cursor").is_some() {
        if !request.as_object().is_some_and(|fields| fields.len() == 1)
            || !get(request, "cursor").as_str().is_some_and(bare)
        {
            return Err(invalid("continue exploration with opaque cursor only"));
        }
        return Ok(Some(string(get(request, "cursor")).to_owned()));
    }
    normalize_for_profile(
        request,
        &LensVocabulary::published_shape(),
        ExplorationProfile::PublishedD1,
    )?;
    Ok(None)
}
pub(crate) fn validate_budget(budget: ExplorationBudget) -> Result<(), SearchV2Error> {
    if budget.max_work_units == 0
        || budget.max_work_units > 512
        || budget.max_session_nodes == 0
        || budget.max_session_nodes > 10000
        || budget.max_session_relations == 0
        || budget.max_session_relations > 20000
        || budget.max_state_bytes == 0
        || budget.max_checkpoint_bytes == 0
        || budget.max_checkpoint_bytes > 32 * 1024 * 1024
        || budget.max_state_bytes > budget.max_checkpoint_bytes
        || budget.max_checkpoints == 0
        || budget.max_checkpoints > 128
    {
        return Err(invalid("exploration admission limits out of range"));
    }
    Ok(())
}
pub fn set_exploration_cursor(
    packet: &mut JsonValue,
    cursor: Option<&str>,
) -> Result<(), SearchV2Error> {
    if cursor.is_some_and(|id| !bare(id))
        || cursor.is_some() != (string(get(packet, "status")) == "paused")
    {
        return Err(corrupt("exploration cursor disagrees with terminal status"));
    }
    let mut page = get(packet, "page").clone();
    if page.as_object().is_none() {
        return Err(corrupt("exploration packet page missing"));
    }
    set(
        &mut page,
        "next_cursor",
        cursor.map_or(JsonValue::Null, text),
    );
    set(packet, "page", page);
    Ok(())
}

#[derive(Clone, Copy, Debug)]
pub struct ExplorationBudget {
    pub read: InspectBudget,
    pub max_work_units: usize,
    pub max_session_nodes: usize,
    pub max_session_relations: usize,
    pub max_state_bytes: usize,
    pub max_checkpoint_bytes: usize,
    pub max_checkpoints: usize,
}
/// Profile/snapshot-bound ID/query state, separate from the request future.
/// Hosts may clone it, never treat it as source or authorisation data.
#[derive(Clone)]
pub struct ExplorationState {
    snapshot_revision: String,
    query: JsonValue,
    origin: Option<JsonValue>,
    roots: Vec<String>,
    seed_relations: Vec<String>,
    queue: Vec<(String, usize)>,
    head: usize,
    after: String,
    identity_after: String,
    identity_added: usize,
    expanded_entities: BTreeSet<String>,
    seen_nodes: BTreeSet<String>,
    seen_relations: BTreeSet<String>,
    page_number: u64,
    reader_profile: ExplorationProfile,
    identity_complete: bool,
    identity_entity: Option<String>,
}
impl ExplorationState {
    pub(crate) fn requested_revision(&self) -> Option<&str> {
        self.query
            .object_get("source_revision")
            .and_then(JsonValue::as_str)
    }
    pub(crate) fn profile(&self) -> ExplorationProfile {
        self.reader_profile
    }
    pub fn snapshot_revision(&self) -> &str {
        &self.snapshot_revision
    }
    pub fn encoded_state(&self, limits: JsonLimits) -> Result<Vec<u8>, SearchV2Error> {
        let value = object(vec![
            ("schema", text("tos_rust_exploration_state_v1")),
            ("snapshot_revision", text(&self.snapshot_revision)),
            ("query", self.query.clone()),
            ("origin", self.origin.clone().unwrap_or(JsonValue::Null)),
            ("roots", strings(&self.roots)),
            ("seed_relations", strings(&self.seed_relations)),
            (
                "queue",
                JsonValue::Array(
                    self.queue
                        .iter()
                        .map(|(id, depth)| JsonValue::Array(vec![text(id), number(*depth)]))
                        .collect(),
                ),
            ),
            ("head", number(self.head)),
            ("after", text(&self.after)),
            ("identity_after", text(&self.identity_after)),
            ("identity_added", number(self.identity_added)),
            (
                "expanded_entities",
                strings(&self.expanded_entities.iter().cloned().collect::<Vec<_>>()),
            ),
            (
                "seen_nodes",
                strings(&self.seen_nodes.iter().cloned().collect::<Vec<_>>()),
            ),
            (
                "seen_relations",
                strings(&self.seen_relations.iter().cloned().collect::<Vec<_>>()),
            ),
            ("page_number", number64(self.page_number)),
            (
                "reader_profile",
                text(if self.reader_profile == ExplorationProfile::PublishedD1 {
                    "published_d1"
                } else {
                    "native_selected"
                }),
            ),
            ("identity_complete", JsonValue::Bool(self.identity_complete)),
            (
                "identity_entity",
                self.identity_entity
                    .as_deref()
                    .map_or(JsonValue::Null, text),
            ),
        ]);
        canonical_bytes_v1(&value, CanonicalProfile::SourceRecordDigestV1, limits).map_err(|_| {
            error(
                SearchV2ErrorCode::BudgetExceeded,
                "exploration state byte cap exceeded",
            )
        })
    }
}

/// Decode this implementation's disposable published checkpoint only. The
/// host first checks cache version, epoch, expiry and atomic storage framing.
/// Old TS state is intentionally refused by the new private cache version.
pub fn decode_published_exploration_state(
    raw: &[u8],
    snapshot_revision: &str,
    budget: ExplorationBudget,
) -> Result<ExplorationState, SearchV2Error> {
    validate_budget(budget)?;
    let mut limits = budget.read.json;
    limits.max_bytes = limits.max_bytes.min(budget.max_state_bytes);
    let document = parse_json(raw, JsonMode::PublishedStrict, limits)
        .map_err(|_| corrupt("invalid exploration checkpoint JSON"))?;
    let value = document.root();
    validate_checkpoint_scalars(value)?;
    let keys = [
        "schema",
        "snapshot_revision",
        "query",
        "origin",
        "roots",
        "seed_relations",
        "queue",
        "head",
        "after",
        "identity_after",
        "identity_added",
        "expanded_entities",
        "seen_nodes",
        "seen_relations",
        "page_number",
        "reader_profile",
        "identity_complete",
        "identity_entity",
    ];
    if !value.as_object().is_some_and(|fields| {
        fields.len() == keys.len()
            && fields
                .iter()
                .all(|(k, _)| k.as_str().is_some_and(|k| keys.contains(&k)))
    }) || string(get(value, "schema")) != "tos_rust_exploration_state_v1"
        || string(get(value, "reader_profile")) != "published_d1"
        || string(get(value, "snapshot_revision")) != snapshot_revision
    {
        return Err(corrupt("invalid exploration checkpoint framing"));
    }
    let id_list = |key: &str, max: usize| -> Result<Vec<String>, SearchV2Error> {
        let list = get(value, key)
            .as_array()
            .ok_or_else(|| corrupt("invalid checkpoint ID array"))?;
        if list.len() > max {
            return Err(corrupt("checkpoint ID array exceeds bound"));
        }
        let mut seen = BTreeSet::new();
        let mut result = vec![];
        for id in list {
            let id = id
                .as_str()
                .filter(|id| !id.is_empty() && id.len() <= budget.read.max_field_bytes)
                .ok_or_else(|| corrupt("invalid checkpoint ID"))?;
            if !seen.insert(id) {
                return Err(corrupt("duplicate checkpoint ID"));
            }
            result.push(id.to_owned());
        }
        Ok(result)
    };
    let query = get(value, "query").clone();
    if normalize_for_profile(
        &query,
        &LensVocabulary::published_shape(),
        ExplorationProfile::PublishedD1,
    )
    .map_err(|_| corrupt("checkpoint query invalid"))?
        != query
    {
        return Err(corrupt("checkpoint query is not normalized"));
    }
    let pairs = get(value, "queue")
        .as_array()
        .filter(|q| !q.is_empty() && q.len() <= budget.max_session_nodes)
        .ok_or_else(|| corrupt("checkpoint queue invalid"))?;
    let mut queue = vec![];
    let mut seen_queue = BTreeSet::new();
    for pair in pairs {
        let pair = pair
            .as_array()
            .filter(|p| p.len() == 2)
            .ok_or_else(|| corrupt("checkpoint queue entry invalid"))?;
        let id = pair[0]
            .as_str()
            .filter(|id| !id.is_empty() && id.len() <= budget.read.max_field_bytes)
            .ok_or_else(|| corrupt("checkpoint queue ID invalid"))?;
        let depth = int(&pair[1])
            .filter(|d| *d <= 10)
            .ok_or_else(|| corrupt("checkpoint queue depth invalid"))?;
        if !seen_queue.insert(id.to_owned()) {
            return Err(corrupt("checkpoint queue duplicate"));
        }
        queue.push((id.to_owned(), depth));
    }
    let counter = |key: &str, max: usize| {
        int(get(value, key))
            .filter(|n| *n <= max)
            .ok_or_else(|| corrupt("checkpoint counter invalid"))
    };
    let head = counter("head", queue.len())?;
    let identity_added = counter("identity_added", queue.len())?;
    let text_field = |key: &str| {
        get(value, key)
            .as_str()
            .filter(|s| s.len() <= budget.read.max_field_bytes)
            .map(str::to_owned)
            .ok_or_else(|| corrupt("checkpoint keyset invalid"))
    };
    let after = text_field("after")?;
    let identity_after = text_field("identity_after")?;
    let identity_complete = match get(value, "identity_complete") {
        JsonValue::Bool(v) => *v,
        _ => return Err(corrupt("checkpoint identity state invalid")),
    };
    let identity_entity = match get(value, "identity_entity") {
        JsonValue::Null => None,
        v => Some(
            v.as_str()
                .filter(|s| !s.is_empty() && s.len() <= budget.read.max_field_bytes)
                .ok_or_else(|| corrupt("checkpoint identity entity invalid"))?
                .to_owned(),
        ),
    };
    let page_number = match get(value, "page_number") {
        JsonValue::Number(n) if n.kind == tos_foundation::JsonNumberKind::Int => n
            .lexeme
            .parse::<u64>()
            .ok()
            .filter(|n| *n >= 1 && *n <= 9_007_199_254_740_991),
        _ => None,
    }
    .ok_or_else(|| corrupt("checkpoint page counter invalid"))?;
    let roots = id_list("roots", 2)?;
    let seed_relations = id_list("seed_relations", 1)?;
    let seen_nodes = id_list("seen_nodes", budget.max_session_nodes)?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let seen_relations = id_list("seen_relations", budget.max_session_relations)?
        .into_iter()
        .collect::<BTreeSet<_>>();
    if seen_nodes != seen_queue
        || roots.iter().any(|id| !seen_nodes.contains(id))
        || seed_relations.iter().any(|id| !seen_relations.contains(id))
    {
        return Err(corrupt("checkpoint closure invalid"));
    }
    let origin = if get(value, "origin") == &JsonValue::Null {
        None
    } else {
        Some(get(value, "origin").clone())
    };
    if let Some(origin) = &origin {
        let requested = get(&query, "origin");
        for key in ["kind", "id", "content_revision"] {
            if get(origin, key) != get(requested, key) {
                return Err(corrupt("checkpoint origin differs"));
            }
        }
        if string(get(origin, "kind")) == "node" {
            if !origin.as_object().is_some_and(|f| f.len() == 3)
                || roots != vec![string(get(origin, "id")).to_owned()]
                || !seed_relations.is_empty()
            {
                return Err(corrupt("checkpoint node origin invalid"));
            }
        } else {
            let endpoints = get(origin, "endpoints");
            if !origin.as_object().is_some_and(|f| f.len() == 4)
                || !endpoints.as_object().is_some_and(|f| f.len() == 2)
                || seed_relations != vec![string(get(origin, "id")).to_owned()]
            {
                return Err(corrupt("checkpoint relation origin invalid"));
            }
            let mut expected = vec![];
            for role in ["from", "to"] {
                let node = get(endpoints, role);
                if !node.as_object().is_some_and(|f| f.len() == 3)
                    || !get(node, "content_revision").as_str().is_some_and(bare)
                    || !get(node, "entity_id")
                        .as_str()
                        .is_some_and(|id| !id.is_empty() && id.len() <= budget.read.max_field_bytes)
                {
                    return Err(corrupt("checkpoint origin endpoint invalid"));
                }
                let id = string(get(node, "node_id"));
                if id.is_empty() {
                    return Err(corrupt("checkpoint endpoint ID invalid"));
                }
                if !expected.iter().any(|n| n == id) {
                    expected.push(id.to_owned());
                }
            }
            if expected != roots {
                return Err(corrupt("checkpoint origin roots differ"));
            }
        }
    } else if get(&query, "origin") != &JsonValue::Null
        || roots != vec![string(get(&query, "focus_node_id")).to_owned()]
        || !seed_relations.is_empty()
    {
        return Err(corrupt("checkpoint focus closure invalid"));
    }
    Ok(ExplorationState {
        snapshot_revision: snapshot_revision.into(),
        query,
        origin,
        roots,
        seed_relations,
        queue,
        head,
        after,
        identity_after,
        identity_added,
        expanded_entities: id_list("expanded_entities", budget.max_session_nodes)?
            .into_iter()
            .collect(),
        seen_nodes,
        seen_relations,
        page_number,
        reader_profile: ExplorationProfile::PublishedD1,
        identity_complete,
        identity_entity,
    })
}
fn validate_checkpoint_scalars(value: &JsonValue) -> Result<(), SearchV2Error> {
    match value {
        JsonValue::Number(n)
            if n.kind != tos_foundation::JsonNumberKind::Int
                || n.lexeme.parse::<i64>().ok().is_none_or(|n| {
                    !(-9_007_199_254_740_991..=9_007_199_254_740_991).contains(&n)
                }) =>
        {
            return Err(corrupt("checkpoint counters must be safe integers"));
        }
        JsonValue::String(s) if s.as_str().is_none() => {
            return Err(corrupt("checkpoint Unicode invalid"));
        }
        JsonValue::Array(items) => {
            for item in items {
                validate_checkpoint_scalars(item)?;
            }
        }
        JsonValue::Object(fields) => {
            for (key, value) in fields {
                if key.as_str().is_none() {
                    return Err(corrupt("checkpoint member Unicode invalid"));
                }
                validate_checkpoint_scalars(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}
/// Validate exact cached bytes without reserialization. Publication epoch and
/// cache framing remain the host's responsibility, not native policy issuance.
pub fn validate_published_exploration_replay(
    raw: &[u8],
    limits: JsonLimits,
) -> Result<(), SearchV2Error> {
    let document = parse_json(raw, JsonMode::PublishedStrict, limits)
        .map_err(|_| corrupt("exploration replay JSON invalid"))?;
    let value = document.root();
    if !matches!(
        get(value, "schema").as_str(),
        Some("tos_exploration_result_v1" | "tos_exploration_result_v2")
    ) || string(get(value, "execution_version")) != PUBLISHED_EXPLORATION_EXECUTION_VERSION
    {
        return Err(corrupt("exploration replay packet invalid"));
    }
    fn unicode(value: &JsonValue) -> Result<(), SearchV2Error> {
        match value {
            JsonValue::String(s) if s.as_str().is_none() => {
                return Err(corrupt("exploration replay Unicode invalid"));
            }
            JsonValue::Array(items) => {
                for item in items {
                    unicode(item)?;
                }
            }
            JsonValue::Object(fields) => {
                for (key, value) in fields {
                    if key.as_str().is_none() {
                        return Err(corrupt("exploration replay member Unicode invalid"));
                    }
                    unicode(value)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    unicode(value)
}
pub enum ExplorationCheckpoint {
    State(ExplorationState),
    Replay {
        packet: JsonValue,
        packet_sha256: Digest256,
    },
}
/// Storage must stage the replay packet and successor atomically, protect both
/// from eviction during admission, and discard an uncommitted preparation.
pub trait PreparedExplorationCheckpoint: Send {
    fn next_cursor(&self) -> Option<&str>;
    fn commit(&mut self) -> Result<(), SearchV2Error>;
}
/// Clock, expiry, opaque-token generation, replay and bounded checkpoint storage
/// belong to this host adapter, separate from immutable selected corpus bytes.
pub trait ExplorationCheckpoints {
    fn load(
        &mut self,
        cursor: &str,
        snapshot_revision: &str,
    ) -> Result<ExplorationCheckpoint, SearchV2Error>;
    fn prepare(
        &mut self,
        input_cursor: Option<&str>,
        snapshot_revision: &str,
        successor: Option<&ExplorationState>,
        packet_without_cursor: &JsonValue,
        budget: ExplorationBudget,
    ) -> Result<Box<dyn PreparedExplorationCheckpoint>, SearchV2Error>;
}
fn strings(values: &[String]) -> JsonValue {
    JsonValue::Array(values.iter().map(|s| text(s)).collect())
}
fn strict_carrier(value: &JsonValue, kind: SearchKind) -> Result<(), SearchV2Error> {
    let mut keys = vec!["id", "native_id", "source_graph"];
    if kind == SearchKind::Nodes {
        keys.extend(["entity_id", "kind_id", "type_id"]);
    } else {
        keys.extend(["from_id", "to_id", "predicate_id", "relation_type_id"]);
    }
    if keys
        .iter()
        .any(|key| !get(value, key).as_str().is_some_and(|s| !s.is_empty()))
        || !get(value, "content_revision").as_str().is_some_and(bare)
    {
        return Err(corrupt("exploration origin identity invalid"));
    }
    for key in [
        "attributes",
        "semantics",
        "display",
        "epistemic",
        if kind == SearchKind::Nodes {
            "type_mapping"
        } else {
            "predicate_mapping"
        },
    ] {
        if get(value, key).as_object().is_none() {
            return Err(corrupt("exploration origin container invalid"));
        }
    }
    for key in ["source_refs", "graph_layers", "view_ids"] {
        let values = get(value, key)
            .as_array()
            .ok_or_else(|| corrupt("exploration origin array invalid"))?;
        if values
            .iter()
            .any(|v| !v.as_str().is_some_and(|v| !v.is_empty()))
            || (key == "source_refs" && values.is_empty())
        {
            return Err(corrupt("exploration origin references invalid"));
        }
    }
    for key in [
        "claim",
        "time",
        "space",
        "responsibility",
        "annotation",
        "language_context",
        "record_version",
    ] {
        if let Some(value) = get(value, "semantics").object_get(key) {
            if value.as_object().is_none() {
                return Err(corrupt("exploration origin semantic container invalid"));
            }
        }
    }
    Ok(())
}
pub(crate) async fn start(
    query: JsonValue,
    revision: &str,
    snapshot_revision: &str,
    rows: &mut Rows,
    vocabulary: &LensVocabulary,
    budget: ExplorationBudget,
) -> Result<ExplorationState, SearchV2Error> {
    let mut query = query;
    let sources = names(get(&query, "sources"))?;
    let mut origin = None;
    let mut seed_relations = vec![];
    let roots = if string(get(&query, "schema_version")) == "tos_exploration_request_v2" {
        if string(get(&query, "source_revision")) != revision {
            return Err(error(
                SearchV2ErrorCode::StaleSelection,
                "exploration source revision changed",
            ));
        }
        let requested = get(&query, "origin");
        let kind = if string(get(requested, "kind")) == "node" {
            SearchKind::Nodes
        } else {
            SearchKind::Relations
        };
        let item = rows
            .item_origin(kind, string(get(requested, "id")))
            .await?
            .ok_or_else(|| {
                error(
                    SearchV2ErrorCode::UnknownIdentifier,
                    "unknown exact exploration origin",
                )
            })?;
        strict_carrier(&item, kind)?;
        if get(&item, "content_revision") != get(requested, "content_revision") {
            return Err(error(
                SearchV2ErrorCode::StaleSelection,
                "exploration origin content changed",
            ));
        }
        if !sources
            .iter()
            .any(|s| s == string(get(&item, "source_graph")))
        {
            return Err(invalid("exploration sources exclude origin"));
        }
        let mut resolved = requested.clone();
        let mut roots = vec![];
        if kind == SearchKind::Nodes {
            roots.push(string(get(&item, "id")).to_owned());
        } else {
            let mut endpoints = object(vec![]);
            for (role, key) in [("from", "from_id"), ("to", "to_id")] {
                let id = string(get(&item, key));
                let node = rows.node(id).await?;
                strict_carrier(&node, SearchKind::Nodes)?;
                if !sources
                    .iter()
                    .any(|s| s == string(get(&node, "source_graph")))
                {
                    return Err(invalid("exploration sources exclude origin endpoint"));
                }
                set(
                    &mut endpoints,
                    role,
                    object(vec![
                        ("node_id", text(id)),
                        ("entity_id", get(&node, "entity_id").clone()),
                        ("content_revision", get(&node, "content_revision").clone()),
                    ]),
                );
                if !roots.iter().any(|v| v == id) {
                    roots.push(id.to_owned());
                }
            }
            set(&mut resolved, "endpoints", endpoints);
            seed_relations.push(string(get(&item, "id")).to_owned());
        }
        origin = Some(resolved);
        roots
    } else {
        let focus = rows
            .focus(
                string(get(&query, "focus_node_id")),
                &sources,
                vocabulary,
                budget.read.max_matches,
            )
            .await?;
        let id = string(get(&focus, "id")).to_owned();
        set(&mut query, "focus_node_id", text(&id));
        vec![id]
    };
    if roots.len() > budget.max_session_nodes || seed_relations.len() > budget.max_session_relations
    {
        return Err(invalid("exploration origin closure exceeds session limits"));
    }
    Ok(ExplorationState {
        snapshot_revision: snapshot_revision.into(),
        query,
        origin,
        queue: roots.iter().map(|id| (id.clone(), 0)).collect(),
        head: 0,
        after: String::new(),
        identity_after: String::new(),
        identity_added: 0,
        expanded_entities: BTreeSet::new(),
        seen_nodes: roots.iter().cloned().collect(),
        seen_relations: seed_relations.iter().cloned().collect(),
        roots,
        seed_relations,
        page_number: 0,
        reader_profile: rows.profile,
        identity_complete: false,
        identity_entity: None,
    })
}
pub(crate) async fn advance(
    state: &mut ExplorationState,
    rows: &mut Rows,
    vocab: &LensVocabulary,
    revision: &str,
    authority_boundary: &JsonValue,
    budget: ExplorationBudget,
) -> Result<JsonValue, SearchV2Error> {
    let query = state.query.clone();
    let sources = names(get(&query, "sources"))?;
    let predicates = names(get(&query, "predicate_ids"))?;
    let max_depth = uint(get(&query, "max_depth"));
    let page_nodes = uint(get(&query, "page_nodes"));
    let page_relations = uint(get(&query, "page_relations"));
    let focus = if let Some(origin) = &state.origin {
        if string(get(origin, "kind")) == "node" {
            Some(string(get(origin, "id")).to_owned())
        } else {
            None
        }
    } else {
        Some(string(get(&query, "focus_node_id")).to_owned())
    };
    let mut primary = if state.origin.is_none() && state.page_number == 0 {
        vec![focus.clone().unwrap()]
    } else {
        vec![]
    };
    let mut emitted = vec![];
    let mut promoted = BTreeSet::new();
    let mut node_reasons = object(vec![]);
    let mut edge_reasons = object(vec![]);
    for id in &state.roots {
        set(
            &mut node_reasons,
            id,
            object(vec![(
                "kind",
                text(if let Some(origin) = &state.origin {
                    if string(get(origin, "kind")) == "node" {
                        "origin"
                    } else {
                        "origin-endpoint"
                    }
                } else {
                    "focus"
                }),
            )]),
        );
    }
    for id in &state.seed_relations {
        set(
            &mut edge_reasons,
            id,
            object(vec![("kind", text("origin"))]),
        );
    }
    let published = rows.profile == ExplorationProfile::PublishedD1;
    let mut identity_cache = VecDeque::new();
    let mut identity_node = String::new();
    let mut adjacency_cache = VecDeque::new();
    let mut adjacency_node = String::new();
    let mut reads = 0usize;
    let mut work = 0;
    let mut limit_reason = None;
    while state.head < state.queue.len() && work < budget.max_work_units {
        rows.check()?;
        let (current, depth) = state.queue[state.head].clone();
        let edge = if depth < max_depth {
            let native_node = if published {
                None
            } else {
                Some(rows.node(&current).await?)
            };
            let entity = native_node
                .as_ref()
                .map(|n| string(get(n, "entity_id")))
                .unwrap_or("");
            let aliases = string(get(&query, "profile")) == "overview"
                && if published {
                    !state.identity_complete
                } else {
                    vocab.declared_entity(entity) && !state.expanded_entities.contains(entity)
                };
            let alias = if aliases && published {
                if identity_node != current || identity_cache.is_empty() {
                    if reads >= 24 {
                        break;
                    }
                    let expanded = state.expanded_entities.iter().cloned().collect::<Vec<_>>();
                    let ids = rows
                        .identities(
                            &current,
                            None,
                            &expanded,
                            &sources,
                            &state.identity_after,
                            32,
                        )
                        .await?;
                    reads += 1;
                    identity_cache = rows.selected(SearchKind::Nodes, &ids).await?.into();
                    identity_node = current.clone();
                    if identity_cache.is_empty() {
                        state.identity_complete = true;
                        if let Some(entity) = &state.identity_entity {
                            state.expanded_entities.insert(entity.clone());
                        }
                        work += 1;
                        continue;
                    }
                }
                let row = identity_cache.pop_front().unwrap();
                let entity = string(get(&row, "entity_id"));
                if !vocab.declared_entity(entity)
                    || state.expanded_entities.contains(entity)
                    || !sources
                        .iter()
                        .any(|s| s == string(get(&row, "source_graph")))
                    || state
                        .identity_entity
                        .as_ref()
                        .is_some_and(|prior| prior != entity)
                {
                    return Err(corrupt("exploration identity page differs from payload"));
                }
                state.identity_entity = Some(entity.to_owned());
                Some((
                    string(get(&row, "id")).to_owned(),
                    entity.to_owned(),
                    Some(row),
                ))
            } else if aliases {
                rows.identities(
                    &current,
                    Some(entity),
                    &[],
                    &vocab.sources,
                    &state.identity_after,
                    1,
                )
                .await?
                .into_iter()
                .next()
                .map(|id| (id, entity.to_owned(), None))
            } else {
                None
            };
            if let Some((alias, entity, retained)) = alias {
                work += 1;
                let alias_node = match retained {
                    Some(node) => node,
                    None => rows.node(&alias).await?,
                };
                if state.seen_nodes.contains(&alias)
                    || !sources
                        .iter()
                        .any(|s| s == string(get(&alias_node, "source_graph")))
                {
                    if state.seen_nodes.contains(&alias) {
                        let position = state
                            .queue
                            .iter()
                            .position(|(id, _)| id == &alias)
                            .ok_or_else(|| corrupt("exploration seen node has no queue entry"))?;
                        if state.queue[position].1 > depth {
                            if !promoted.contains(&alias)
                                && primary.len() + promoted.len() >= page_nodes
                            {
                                break;
                            }
                            state.queue.remove(position);
                            state.queue.insert(
                                state.head + 1 + state.identity_added,
                                (alias.clone(), depth),
                            );
                            state.identity_added += 1;
                            promoted.insert(alias.clone());
                            set(
                                &mut node_reasons,
                                &alias,
                                object(vec![
                                    ("kind", text("identity-carrier")),
                                    ("via_node_id", text(&current)),
                                    ("entity_id", text(&entity)),
                                    ("depth", number(depth)),
                                ]),
                            );
                        }
                    }
                    state.identity_after = alias;
                    continue;
                }
                if state.seen_nodes.len() >= budget.max_session_nodes {
                    limit_reason = Some("session_nodes");
                    break;
                }
                if primary.len() + promoted.len() >= page_nodes {
                    break;
                }
                state.identity_after = alias.clone();
                state.seen_nodes.insert(alias.clone());
                state.queue.insert(
                    state.head + 1 + state.identity_added,
                    (alias.clone(), depth),
                );
                state.identity_added += 1;
                primary.push(alias.clone());
                set(
                    &mut node_reasons,
                    &alias,
                    object(vec![
                        ("kind", text("identity-carrier")),
                        ("via_node_id", text(&current)),
                        ("entity_id", text(&entity)),
                        ("depth", number(depth)),
                    ]),
                );
                continue;
            }
            if aliases && !published {
                state.expanded_entities.insert(entity.to_owned());
            }
            if published {
                if adjacency_node != current || adjacency_cache.is_empty() {
                    if reads >= 24 {
                        break;
                    }
                    let ids = rows.adjacent(&current, &state.after, 32).await?;
                    reads += 1;
                    let edges = rows.selected(SearchKind::Relations, &ids).await?;
                    if edges.iter().any(|edge| {
                        string(get(edge, "from_id")) != current
                            && string(get(edge, "to_id")) != current
                    }) {
                        return Err(corrupt("exploration adjacency endpoint mirror differs"));
                    }
                    let endpoints = edges
                        .iter()
                        .flat_map(|edge| {
                            [
                                string(get(edge, "from_id")).to_owned(),
                                string(get(edge, "to_id")).to_owned(),
                            ]
                        })
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect::<Vec<_>>();
                    let endpoints = rows
                        .optional(SearchKind::Nodes, &endpoints)
                        .await?
                        .into_iter()
                        .map(|node| (string(get(&node, "id")).to_owned(), node))
                        .collect::<std::collections::BTreeMap<_, _>>();
                    for edge in edges {
                        let from = endpoints.get(string(get(&edge, "from_id"))).cloned();
                        let to = endpoints.get(string(get(&edge, "to_id"))).cloned();
                        adjacency_cache.push_back((edge, from, to));
                    }
                    adjacency_node = current.clone();
                }
                adjacency_cache.pop_front()
            } else {
                match rows
                    .adjacent(&current, &state.after, 1)
                    .await?
                    .into_iter()
                    .next()
                {
                    Some(id) => Some((rows.relation(&id).await?, None, None)),
                    None => None,
                }
            }
        } else {
            None
        };
        let Some((edge, from_node, to_node)) = edge else {
            state.head += 1;
            state.after.clear();
            state.identity_after.clear();
            state.identity_added = 0;
            state.identity_complete = false;
            state.identity_entity = None;
            work += 1;
            continue;
        };
        work += 1;
        let id = string(get(&edge, "id"));
        let from = string(get(&edge, "from_id"));
        let to = string(get(&edge, "to_id"));
        if from != current && to != current {
            return Err(corrupt("exploration adjacency endpoint mirror differs"));
        }
        let target = if from == current { to } else { from };
        let target_node = if published {
            if from == current {
                to_node.clone()
            } else {
                from_node.clone()
            }
        } else {
            rows.item(SearchKind::Nodes, target).await?
        };
        let endpoint_scope = !published
            || from_node
                .as_ref()
                .zip(to_node.as_ref())
                .is_some_and(|(from, to)| {
                    sources
                        .iter()
                        .any(|s| s == string(get(from, "source_graph")))
                        && sources.iter().any(|s| s == string(get(to, "source_graph")))
                });
        let eligible = endpoint_scope
            && !state.seen_relations.contains(id)
            && sources
                .iter()
                .any(|s| s == string(get(&edge, "source_graph")))
            && target_node
                .as_ref()
                .is_some_and(|n| sources.iter().any(|s| s == string(get(n, "source_graph"))))
            && (string(get(&query, "direction")) != "outgoing" || from == current)
            && (string(get(&query, "direction")) != "incoming" || to == current)
            && (predicates.is_empty()
                || predicates
                    .iter()
                    .any(|s| s == string(get(&edge, "predicate_id"))))
            && (string(get(&query, "profile")) != "overview"
                || (!vocab
                    .overview_excluded_predicates
                    .iter()
                    .any(|s| s == string(get(&edge, "predicate_id")))
                    && !vocab
                        .overview_excluded_relation_types
                        .iter()
                        .any(|s| s == string(get(&edge, "relation_type_id")))));
        if !eligible {
            state.after = id.to_owned();
            continue;
        }
        let new = !state.seen_nodes.contains(target);
        if state.seen_relations.len() >= budget.max_session_relations
            || (new && state.seen_nodes.len() >= budget.max_session_nodes)
        {
            limit_reason = Some(
                if state.seen_relations.len() >= budget.max_session_relations {
                    "session_relations"
                } else {
                    "session_nodes"
                },
            );
            break;
        }
        if emitted.len() >= page_relations || (new && primary.len() + promoted.len() >= page_nodes)
        {
            break;
        }
        state.after = id.to_owned();
        state.seen_relations.insert(id.to_owned());
        emitted.push(id.to_owned());
        set(
            &mut edge_reasons,
            id,
            object(vec![
                ("kind", text("traversal")),
                ("via_node_id", text(&current)),
                ("depth", number(depth + 1)),
            ]),
        );
        if new {
            state.seen_nodes.insert(target.to_owned());
            state.queue.push((target.to_owned(), depth + 1));
            primary.push(target.to_owned());
            set(
                &mut node_reasons,
                target,
                object(vec![
                    ("kind", text("traversal")),
                    ("via_node_id", text(&current)),
                    ("via_relation_id", text(id)),
                    ("depth", number(depth + 1)),
                ]),
            );
        }
    }
    state.page_number = state.page_number.checked_add(1).ok_or_else(|| {
        error(
            SearchV2ErrorCode::BudgetExceeded,
            "exploration page ordinal overflow",
        )
    })?;
    let mut selected: BTreeSet<String> = primary
        .iter()
        .chain(&state.roots)
        .chain(&promoted)
        .cloned()
        .collect();
    for id in &emitted {
        let edge = rows.relation(id).await?;
        for key in ["from_id", "to_id"] {
            selected.insert(string(get(&edge, key)).to_owned());
        }
    }
    for id in &selected {
        if node_reasons.object_get(id).is_none() {
            set(
                &mut node_reasons,
                id,
                object(vec![("kind", text("context-endpoint"))]),
            );
        }
    }
    let mut nodes = vec![];
    let selected_ids = selected.iter().cloned().collect::<Vec<_>>();
    for row in rows.selected(SearchKind::Nodes, &selected_ids).await? {
        nodes.push(lens_carrier(&row, "compact", Some("auto"))?);
    }
    let mut relations = vec![];
    let delivered_ids = state
        .seed_relations
        .iter()
        .chain(&emitted)
        .cloned()
        .collect::<Vec<_>>();
    for row in rows.selected(SearchKind::Relations, &delivered_ids).await? {
        relations.push(lens_carrier(&row, "compact", Some("auto"))?);
    }
    let status = if limit_reason.is_some() {
        "limit_reached"
    } else if state.head == state.queue.len() {
        "complete"
    } else {
        "paused"
    };
    let context: Vec<_> = selected
        .iter()
        .filter(|id| !primary.contains(id))
        .cloned()
        .collect();
    let mut page = object(vec![
        ("number", number64(state.page_number)),
        ("primary_node_ids", strings(&primary)),
        ("context_node_ids", strings(&context)),
        ("next_cursor", JsonValue::Null),
        ("returned_nodes", number(selected.len())),
        ("returned_relations", number(relations.len())),
        ("work_units", number(work)),
        ("scope", text("resumable-neighborhood")),
    ]);
    let focus_relation = state
        .origin
        .as_ref()
        .filter(|o| string(get(o, "kind")) == "relation")
        .map(|o| string(get(o, "id")));
    let scene = knowledge_scene(&nodes, &relations, focus.as_deref(), focus_relation, vocab)?;
    let mut result = object(vec![
        (
            "schema",
            text(if state.origin.is_some() {
                "tos_exploration_result_v2"
            } else {
                "tos_exploration_result_v1"
            }),
        ),
        (
            "execution_version",
            text(if published {
                PUBLISHED_EXPLORATION_EXECUTION_VERSION
            } else {
                EXPLORATION_EXECUTION_VERSION
            }),
        ),
        ("snapshot_revision", text(&state.snapshot_revision)),
        ("source_revision", text(revision)),
        ("query", query),
        ("status", text(status)),
        ("limit_reason", limit_reason.map_or(JsonValue::Null, text)),
        ("nodes", JsonValue::Array(nodes)),
        ("relations", JsonValue::Array(relations)),
        ("scene", scene),
        (
            "counts",
            object(vec![
                ("discovered_nodes", number(state.seen_nodes.len())),
                (
                    "emitted_relations",
                    number(state.seen_relations.len() - state.seed_relations.len()),
                ),
                ("scope", text("cumulative-discovered-not-global-total")),
            ]),
        ),
        (
            "inclusion",
            object(vec![
                ("nodes", node_reasons),
                ("relations", edge_reasons),
                ("authority", text("query-execution-not-semantic-proof")),
            ]),
        ),
        ("authority_boundary", authority_boundary.clone()),
        ("writes_to_tree", JsonValue::Bool(false)),
    ]);
    if let Some(origin) = &state.origin {
        set(&mut result, "origin", origin.clone());
        set(&mut page, "primary_relation_ids", strings(&emitted));
        set(
            &mut page,
            "context_relation_ids",
            strings(&state.seed_relations),
        );
    } else {
        set(
            &mut result,
            "focus",
            object(vec![(
                "node_id",
                focus.as_ref().map_or(JsonValue::Null, |id| text(id)),
            )]),
        );
    }
    set(&mut result, "page", page);
    if published {
        let JsonValue::Object(fields) = &mut result else {
            unreachable!()
        };
        let page = fields
            .iter_mut()
            .find(|(key, _)| key.as_str() == Some("page"))
            .map(|(_, value)| std::mem::replace(value, JsonValue::Null))
            .unwrap();
        let page = ordered(
            page,
            &[
                "number",
                "primary_node_ids",
                "context_node_ids",
                "primary_relation_ids",
                "context_relation_ids",
                "next_cursor",
                "returned_nodes",
                "returned_relations",
                "work_units",
                "scope",
            ],
        );
        set(&mut result, "page", page);
        result = ordered(
            result,
            &[
                "schema",
                "execution_version",
                "snapshot_revision",
                "source_revision",
                "query",
                "origin",
                "focus",
                "status",
                "limit_reason",
                "nodes",
                "relations",
                "scene",
                "page",
                "counts",
                "inclusion",
                "authority_boundary",
                "writes_to_tree",
            ],
        );
    }
    Ok(result)
}
#[cfg(not(target_arch = "wasm32"))]
fn snapshot(
    bound: &BoundCmpKnowledge<'_>,
    scope: &crate::knowledge_packet::IndexedDisclosureScope,
    limits: JsonLimits,
    budget: ExplorationBudget,
) -> Result<String, SearchV2Error> {
    let value = object(vec![
        ("schema", text(SNAPSHOT_SCHEMA)),
        ("execution_version", text(EXPLORATION_EXECUTION_VERSION)),
        ("work_limit", number(budget.max_work_units)),
        ("session_nodes", number(budget.max_session_nodes)),
        ("session_relations", number(budget.max_session_relations)),
        ("model_sha256", text(&scope.selected_index_sha256.to_hex())),
        ("descriptor_sha256", text(&scope.descriptor_sha256.to_hex())),
        ("owner_receipt", text(&scope.selected_model_receipt_id)),
        ("source_cut", text(&scope.source_cut)),
        ("source_revision", text(bound.source_revision())),
        (
            "through_commit_seq",
            JsonValue::Number(tos_foundation::JsonNumber {
                kind: tos_foundation::JsonNumberKind::Int,
                lexeme: scope.through_commit_seq.to_string(),
            }),
        ),
        (
            "membership_root",
            text(&scope.source_membership_root.to_hex()),
        ),
        (
            "index_generation",
            text(&bound.selection().index_generation),
        ),
        (
            "route_map_version",
            text(&bound.selection().route_map_version),
        ),
        ("reader_abi", text(&bound.selection().reader_abi)),
        ("policy_issuer", text(&scope.policy_issuer_ref)),
        ("policy_receipt", text(&scope.policy_receipt_id)),
        ("policy_scope", text(&scope.policy_scope)),
        ("policy_epoch", text(&scope.policy_epoch)),
        ("withdrawal_generation", text(&scope.withdrawal_generation)),
    ]);
    Ok(Digest256::of_bytes(
        &canonical_bytes_v1(&value, CanonicalProfile::SourceRecordDigestV1, limits).map_err(
            |_| {
                error(
                    SearchV2ErrorCode::BudgetExceeded,
                    "exploration binding cap exceeded",
                )
            },
        )?,
    )
    .to_hex())
}

#[cfg(not(target_arch = "wasm32"))]
fn drive_selected<A: InspectCurrentAuthority + ?Sized>(
    read: &mut Reader<'_, '_, A>,
    input: crate::exploration_plan::ExplorationInput,
    revision: &str,
    snapshot: &str,
    boundary: JsonValue,
    vocabulary: LensVocabulary,
    budget: ExplorationBudget,
) -> Result<crate::exploration_plan::ExplorationOutput, SearchV2Error> {
    use crate::exploration_plan::{ExplorationNeed, ExplorationPlan, ExplorationReply};
    struct Probe(Option<std::sync::Arc<dyn crate::AbortProbe>>);
    impl crate::AbortProbe for Probe {
        fn reason(&self) -> Option<crate::AbortReason> {
            self.0.as_ref().and_then(|p| p.reason())
        }
    }
    let probe = std::rc::Rc::new(Probe(read.abort_probe()));
    let mut plan = ExplorationPlan::native(
        input, revision, snapshot, boundary, vocabulary, budget, probe,
    )?;
    while !plan.advance()? {
        read.check_interrupt()?;
        let need = plan
            .need()
            .ok_or_else(|| corrupt("native exploration continuation lacks read"))?;
        let reply = match &*need {
            ExplorationNeed::Rows { kind, ids, .. } => {
                let mut rows = vec![];
                let mut raw_bytes = vec![];
                for id in ids {
                    for (value, size) in read.items_with_sizes(*kind, "id", id, 1, false)? {
                        rows.push(value);
                        raw_bytes.push(size);
                    }
                }
                ExplorationReply::Rows {
                    rows,
                    raw_bytes,
                    ambiguous_ids: vec![],
                }
            }
            ExplorationNeed::Focus {
                field,
                identifier,
                sources,
                source_priority,
                limit,
            } => {
                let values =
                    read.items_with_sizes(SearchKind::Nodes, field, identifier, *limit, false)?;
                let matched = values.len();
                let mut values = values
                    .into_iter()
                    .filter(|(node, _)| {
                        sources
                            .iter()
                            .any(|source| source == string(get(node, "source_graph")))
                    })
                    .collect::<Vec<_>>();
                if *field == "entity_id" {
                    values.sort_by_key(|(node, _)| {
                        (
                            source_priority
                                .iter()
                                .find(|(source, _)| source == string(get(node, "source_graph")))
                                .map_or(99, |(_, priority)| *priority),
                            string(get(node, "id")).to_owned(),
                        )
                    });
                }
                let (rows, raw_bytes) = values.into_iter().unzip();
                ExplorationReply::Focus {
                    matched,
                    rows,
                    raw_bytes,
                }
            }
            ExplorationNeed::IdentityPage {
                entity_id,
                sources,
                after,
                limit,
                ..
            } => ExplorationReply::Ids(
                read.identity_ids(
                    entity_id
                        .as_deref()
                        .ok_or_else(|| corrupt("native exploration entity selector missing"))?,
                    sources,
                    after,
                    *limit,
                )?,
            ),
            ExplorationNeed::AdjacencyPage {
                node_id,
                after,
                limit,
            } => ExplorationReply::Ids(read.incident_ids(node_id, after, *limit)?),
        };
        read.check_interrupt()?;
        plan.resume(reply)?;
    }
    read.check_interrupt()?;
    plan.finish()
}
/// Execute and stage one page. A checkpoint preparation is committed only
/// after packet size, selected snapshot, current policy and disclosure hold
/// all pass. A failed query leaves the input cursor/state unchanged.
#[cfg(not(target_arch = "wasm32"))]
pub fn execute_selected_exploration<
    A: InspectCurrentAuthority + ?Sized,
    C: ExplorationCheckpoints + ?Sized,
>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    checkpoints: &mut C,
    request: &JsonValue,
    budget: ExplorationBudget,
) -> Result<DisclosableInspect, SearchV2Error> {
    validate_budget(budget)?;
    let mut prepared: Option<Box<dyn PreparedExplorationCheckpoint>> = None;
    let packet = execute_selected_carrier_packet(
        model,
        bound,
        authority,
        EXPLORATION_OPERATION,
        EXPLORATION_INTENDED_USE,
        budget.read,
        |read| {
            let revision = snapshot(bound, read.disclosure_scope(), budget.read.json, budget)?;
            let header = read.header()?;
            let vocabulary = LensVocabulary::from_selected(bound, &header)?;
            let cursor = if request.object_get("cursor").is_some() {
                if !request.as_object().is_some_and(|fields| fields.len() == 1)
                    || !get(request, "cursor").as_str().is_some_and(bare)
                {
                    return Err(invalid("continue exploration with opaque cursor only"));
                }
                Some(string(get(request, "cursor")))
            } else {
                None
            };
            let checkpoint = match cursor {
                Some(cursor) => Some(checkpoints.load(cursor, &revision)?),
                None => None,
            };
            if let Some(ExplorationCheckpoint::Replay {
                packet,
                packet_sha256,
            }) = &checkpoint
            {
                let mut limits = budget.read.json;
                limits.max_bytes = limits.max_bytes.min(budget.read.max_response_bytes);
                let bytes =
                    canonical_bytes_v1(packet, CanonicalProfile::SourceRecordDigestV1, limits)
                        .map_err(|_| {
                            error(
                                SearchV2ErrorCode::BudgetExceeded,
                                "exploration replay packet cap exceeded",
                            )
                        })?;
                if Digest256::of_bytes(&bytes) != *packet_sha256 {
                    return Err(corrupt("exploration replay packet integrity changed"));
                }
                if string(get(packet, "snapshot_revision")) != revision
                    || string(get(packet, "source_revision")) != bound.source_revision()
                {
                    return Err(error(
                        SearchV2ErrorCode::StaleContinuation,
                        "exploration replay binding changed",
                    ));
                }
                // Replay reauthorizes exact source carriers under the current hold,
                // never blindly discloses a cached prior-policy response.
                for (key, kind) in [
                    ("nodes", SearchKind::Nodes),
                    ("relations", SearchKind::Relations),
                ] {
                    for carrier in array(get(packet, key)) {
                        let selected = read
                            .items(kind, "id", string(get(carrier, "id")), 1, false)?
                            .into_iter()
                            .next()
                            .ok_or_else(|| corrupt("exploration replay carrier missing"))?;
                        if get(&selected, "content_revision") != get(carrier, "content_revision") {
                            return Err(error(
                                SearchV2ErrorCode::StaleContinuation,
                                "exploration replay carrier changed",
                            ));
                        }
                    }
                }
                return Ok(packet.clone());
            }
            let input = match checkpoint {
                Some(ExplorationCheckpoint::State(state)) => {
                    crate::exploration_plan::ExplorationInput::Continue(state)
                }
                None => crate::exploration_plan::ExplorationInput::Start(request.clone()),
                Some(ExplorationCheckpoint::Replay { .. }) => unreachable!(),
            };
            let boundary = parse_json(
                bound.authority_boundary().as_bytes(),
                JsonMode::PublishedStrict,
                budget.read.json,
            )
            .map_err(|_| corrupt("exploration selected boundary invalid"))?
            .into_root();
            let output = drive_selected(
                read,
                input,
                bound.source_revision(),
                &revision,
                boundary,
                vocabulary,
                budget,
            )?;
            let state = output.state;
            let mut packet = output.packet;
            let mut limits = budget.read.json;
            limits.max_bytes = limits.max_bytes.min(budget.max_state_bytes);
            state.encoded_state(limits)?;
            let successor = (string(get(&packet, "status")) == "paused").then_some(&state);
            let staged = checkpoints.prepare(cursor, &revision, successor, &packet, budget)?;
            if staged.next_cursor().is_some() != successor.is_some()
                || staged.next_cursor().is_some_and(|token| !bare(token))
            {
                return Err(corrupt("exploration checkpoint token invalid"));
            }
            let mut page = get(&packet, "page").clone();
            set(
                &mut page,
                "next_cursor",
                staged.next_cursor().map_or(JsonValue::Null, text),
            );
            set(&mut packet, "page", page);
            prepared = Some(staged);
            Ok(packet)
        },
    )?;
    if let Some(mut staged) = prepared {
        staged.commit()?;
    }
    Ok(packet)
}
