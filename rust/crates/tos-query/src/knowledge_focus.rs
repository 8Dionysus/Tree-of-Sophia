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
    let all = requested.iter().all(|s| s.is_empty());
    let sources = vocabulary
        .sources
        .iter()
        .filter(|s| all || requested.contains(s))
        .map(|s| text(s))
        .collect();
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
