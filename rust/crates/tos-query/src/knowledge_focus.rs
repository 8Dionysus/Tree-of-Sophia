//! The declared radial focus request, compiled through the shared LensSpec ABI.
use crate::{
    knowledge_lens_spec::*,
    search_v2::SearchV2Error,
    source_read_projection::{object, text},
};
use tos_foundation::JsonValue;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FocusDirection {
    Outgoing,
    Incoming,
    Either,
}
impl FocusDirection {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Outgoing => "outgoing",
            Self::Incoming => "incoming",
            Self::Either => "either",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FocusProfile {
    All,
    Overview,
}
impl FocusProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Overview => "overview",
        }
    }
}
#[derive(Clone, Debug)]
pub struct KnowledgeFocusRequest {
    pub node_id: String,
    pub sources: Option<Vec<String>>,
    pub depth: usize,
    pub direction: FocusDirection,
    pub predicate_ids: Vec<String>,
    pub node_limit: usize,
    pub relation_limit: usize,
    pub profile: FocusProfile,
}
impl tos_foundation::OwnedState for KnowledgeFocusRequest {
    fn owned_heap_bytes(&self) -> tos_foundation::Result<usize> {
        use tos_foundation::{checked_state_add, OwnedState};
        checked_state_add(self.node_id.owned_heap_bytes()?,
            checked_state_add(self.sources.owned_heap_bytes()?,
                self.predicate_ids.owned_heap_bytes()?)?)
    }
}
impl KnowledgeFocusRequest {
    pub fn new(node_id: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
            sources: None,
            depth: 1,
            direction: FocusDirection::Either,
            predicate_ids: vec![],
            node_limit: 200,
            relation_limit: 400,
            profile: FocusProfile::Overview,
        }
    }
}
pub fn focus_lens_spec(
    request: &KnowledgeFocusRequest,
    vocabulary: &LensVocabulary,
) -> Result<JsonValue, SearchV2Error> {
    let identifier = strip(&request.node_id);
    if identifier.is_empty() {
        return Err(invalid("knowledge focus identifier is required"));
    }
    let requested = request.sources.as_deref().unwrap_or(&[]);
    if requested
        .iter()
        .filter(|s| !s.is_empty())
        .any(|s| !vocabulary.sources.contains(s))
    {
        return Err(invalid("unknown focus source"));
    }
    let all = if vocabulary.is_published() {
        request.sources.is_none()
    } else {
        requested.iter().all(|s| s.is_empty())
    };
    let sources = if vocabulary.is_published() && request.sources.is_some() {
        // Published focus preserves the requested sequence so the shared
        // normalizer can reject duplicates and empty/unknown entries.
        requested.iter().map(|s| text(s)).collect()
    } else {
        vocabulary
            .sources
            .iter()
            .filter(|s| all || requested.contains(s))
            .map(|s| text(s))
            .collect()
    };
    Ok(object(vec![
        ("schema_version", text("tos_lens_spec_v1")),
        ("lens_id", text("focus-neighborhood")),
        (
            "title",
            object(vec![("default", text(&format!("Focus: {identifier}")))]),
        ),
        (
            "description",
            object(vec![(
                "default",
                text(&format!(
                    "Bounded knowledge neighborhood centered on {identifier}."
                )),
            )]),
        ),
        ("sources", JsonValue::Array(sources)),
        ("seed", object(vec![("focus_node_id", text(identifier))])),
        (
            "node_query",
            object(vec![("enabled", JsonValue::Bool(false))]),
        ),
        (
            "relation_query",
            object(vec![("enabled", JsonValue::Bool(true))]),
        ),
        (
            "traversal",
            object(vec![
                ("depth", number(request.depth)),
                ("direction", text(request.direction.as_str())),
                (
                    "predicate_ids",
                    JsonValue::Array(request.predicate_ids.iter().map(|s| text(s)).collect()),
                ),
                ("profile", text(request.profile.as_str())),
            ]),
        ),
        (
            "composition",
            object(vec![
                ("endpoint_policy", text("both")),
                ("group_by", JsonValue::Array(vec![])),
                (
                    "sort_nodes",
                    JsonValue::Array(vec![object(vec![
                        ("field", text("id")),
                        ("direction", text("asc")),
                    ])]),
                ),
                (
                    "sort_relations",
                    JsonValue::Array(vec![object(vec![
                        ("field", text("id")),
                        ("direction", text("asc")),
                    ])]),
                ),
            ]),
        ),
        (
            "presentation",
            object(vec![
                ("layout", text("radial")),
                ("color_by", text("kind_id")),
                ("lane_by", text("epistemic.authority_layer")),
                ("size_by", JsonValue::Null),
                (
                    "inspector_fields",
                    JsonValue::Array(
                        ["display", "epistemic", "source_refs", "attributes"]
                            .map(text)
                            .to_vec(),
                    ),
                ),
            ]),
        ),
        (
            "limits",
            object(vec![
                ("nodes", number(request.node_limit)),
                ("relations", number(request.relation_limit)),
                ("groups", number(100)),
            ]),
        ),
    ]))
}

/// Parse the actual focus transport argument vocabulary, then reuse the
/// declared focus-to-LensSpec constructor and normalizer. No I/O is needed.
pub fn focus_lens_spec_from_json(
    value: &JsonValue,
    vocabulary: &LensVocabulary,
) -> Result<JsonValue, SearchV2Error> {
    let fields = value
        .as_object()
        .ok_or_else(|| invalid("focus request must be an object"))?;
    if fields.iter().any(|(key, _)| {
        !matches!(
            key.as_str(),
            Some(
                "node_id"
                    | "sources"
                    | "depth"
                    | "direction"
                    | "profile"
                    | "node_limit"
                    | "relation_limit"
                    | "predicate_ids"
            )
        )
    }) {
        return Err(invalid("unknown focus request field"));
    }
    let node_id = get(value, "node_id")
        .as_str()
        .ok_or_else(|| invalid("knowledge focus identifier is required"))?;
    let mut request = KnowledgeFocusRequest::new(node_id);
    fn integer(value: &JsonValue, default: usize) -> Result<usize, SearchV2Error> {
        if matches!(value, JsonValue::Null) {
            return Ok(default);
        }
        value
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or_else(|| invalid("invalid focus integer"))
    }
    fn strings(value: &JsonValue) -> Result<Vec<String>, SearchV2Error> {
        value
            .as_array()
            .ok_or_else(|| invalid("invalid focus string list"))?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| invalid("invalid focus string list"))
            })
            .collect()
    }
    if value.object_get("sources").is_some() && !matches!(get(value, "sources"), JsonValue::Null) {
        request.sources = Some(strings(get(value, "sources"))?);
    }
    if value.object_get("predicate_ids").is_some()
        && !matches!(get(value, "predicate_ids"), JsonValue::Null)
    {
        request.predicate_ids = strings(get(value, "predicate_ids"))?;
    }
    request.depth = integer(get(value, "depth"), request.depth)?;
    request.node_limit = integer(get(value, "node_limit"), request.node_limit)?;
    request.relation_limit = integer(get(value, "relation_limit"), request.relation_limit)?;
    request.direction = match get(value, "direction") {
        JsonValue::Null => request.direction,
        value => match value.as_str() {
            Some("outgoing") => FocusDirection::Outgoing,
            Some("incoming") => FocusDirection::Incoming,
            Some("either") => FocusDirection::Either,
            _ => return Err(invalid("invalid focus direction")),
        },
    };
    request.profile = match get(value, "profile") {
        JsonValue::Null => request.profile,
        value => match value.as_str() {
            Some("all") => FocusProfile::All,
            Some("overview") => FocusProfile::Overview,
            _ => return Err(invalid("invalid focus profile")),
        },
    };
    normalize_lens_spec(&focus_lens_spec(&request, vocabulary)?, vocabulary)
}

pub fn normalize_published_focus_request(value: &JsonValue) -> Result<JsonValue, SearchV2Error> {
    focus_lens_spec_from_json(value, &LensVocabulary::published_shape())
}
