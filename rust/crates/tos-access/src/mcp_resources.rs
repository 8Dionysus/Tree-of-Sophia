//! Maintained MCP resource association; packet semantics stay in QRY.
use crate::common::{json_string, json_string_len};
use crate::{
    AccessError, AccessErrorCode, AccessProfile, KnowledgeOperation as O, KnowledgeRequest as R,
};
use tos_foundation::{JsonMode, JsonValue, parse_json};

const RESOURCES: &[(&str, &str)] = &[
    ("tos-corpus://status", "status_resource"),
    ("tos-corpus://summary", "summary_resource"),
    ("tos-corpus://graph-views", "graph_views_resource"),
    ("tos-philosophy://status", "philosophy_status_resource"),
    ("tos-philosophy://views", "philosophy_views_resource"),
    ("tos-philosophy://layers", "philosophy_layers_resource"),
    (
        "tos-philosophy://contracts",
        "philosophy_contracts_resource",
    ),
    (
        "tos-philosophy://scale-manifest",
        "philosophy_scale_manifest_resource",
    ),
    ("tos-philosophy://snapshot", "philosophy_snapshot_resource"),
    ("tos-philosophy://audit", "philosophy_audit_resource"),
    ("tos-philosophy://clusters", "philosophy_clusters_resource"),
    (
        "tos-philosophy://unresolved",
        "philosophy_unresolved_resource",
    ),
];
const TEMPLATES: &[(&str, &str)] = &[
    ("tos-corpus://graph-view/{view_id}", "graph_view_resource"),
    (
        "tos-philosophy://view/{view_id}",
        "philosophy_view_resource",
    ),
    (
        "tos-philosophy://review-packet/{view_id}",
        "philosophy_review_packet_resource",
    ),
    (
        "tos-philosophy://edge/{edge_id}",
        "philosophy_edge_resource",
    ),
    (
        "tos-philosophy://lens/{view_id}",
        "philosophy_lens_resource",
    ),
];
pub(crate) fn list(templates: bool) -> Vec<u8> {
    let rows = if templates { TEMPLATES } else { RESOURCES };
    let mut out = if templates {
        b"{\"resourceTemplates\":[".to_vec()
    } else {
        b"{\"resources\":[".to_vec()
    };
    for (i, (uri, name)) in rows.iter().enumerate() {
        if i != 0 {
            out.push(b',');
        }
        out.extend_from_slice(if templates {
            b"{\"uriTemplate\":"
        } else {
            b"{\"uri\":"
        });
        out.extend(json_string(uri));
        out.extend_from_slice(b",\"name\":");
        out.extend(json_string(name));
        // Maintained FastMCP functions return strings, whose MIME is text/plain.
        out.extend_from_slice(b",\"description\":\"\",\"mimeType\":\"text/plain\"}");
    }
    out.extend_from_slice(b"]}");
    out
}
fn invalid(message: &'static str) -> AccessError {
    AccessError::new(AccessErrorCode::InvalidRequest, message)
}
pub(crate) fn request(uri: &str, profile: AccessProfile) -> Result<R, AccessError> {
    if uri.len() > 4096 {
        return Err(invalid("resource URI exceeds byte budget"));
    }
    let fixed = match uri {
        "tos-corpus://status" => Some(O::CorpusStatus),
        "tos-corpus://summary" => Some(O::CorpusSummary),
        "tos-corpus://graph-views" => Some(O::CorpusGraphViews),
        "tos-philosophy://status" => Some(O::PhilosophyStatus),
        "tos-philosophy://views" => Some(O::PhilosophyViews),
        "tos-philosophy://layers" => Some(O::PhilosophyLayers),
        "tos-philosophy://contracts" => Some(O::PhilosophyContracts),
        "tos-philosophy://scale-manifest" => Some(O::PhilosophyScaleManifest),
        "tos-philosophy://snapshot" => Some(O::PhilosophySnapshot),
        "tos-philosophy://audit" => Some(O::PhilosophyAudit),
        "tos-philosophy://clusters" => Some(O::PhilosophyClusters),
        "tos-philosophy://unresolved" => Some(O::PhilosophyUnresolved),
        _ => None,
    };
    let (operation, field, value) = if let Some(operation) = fixed {
        (operation, None, None)
    } else {
        let templates = [
            ("tos-corpus://graph-view/", O::CorpusGraphView, "view_id"),
            ("tos-philosophy://view/", O::PhilosophyView, "view_id"),
            (
                "tos-philosophy://review-packet/",
                O::PhilosophyReview,
                "view_id",
            ),
            ("tos-philosophy://edge/", O::PhilosophyEdge, "edge_id"),
            ("tos-philosophy://lens/", O::PhilosophyLens, "view_id"),
        ];
        let (prefix, operation, field) = templates
            .into_iter()
            .find(|(prefix, _, _)| uri.starts_with(prefix))
            .ok_or_else(|| invalid("unknown ToS resource URI"))?;
        let value = &uri[prefix.len()..];
        // The maintained FastMCP template matches one raw nonempty path
        // segment; it does not percent-decode the selected opaque identifier.
        if value.is_empty() || value.contains('/') {
            return Err(invalid("resource template identifier is invalid"));
        }
        (operation, Some(field), Some(value))
    };
    let mut args = b"{".to_vec();
    if let (Some(field), Some(value)) = (field, value) {
        args.extend(json_string(field));
        args.push(b':');
        args.extend(json_string(value));
    }
    args.push(b'}');
    let args = parse_json(&args, JsonMode::RequestLastWins, profile.json_limits())
        .map_err(|_| invalid("resource parameters are invalid"))?;
    R::from_arguments(operation, args.root())
}
pub(crate) fn uri(params: Option<&JsonValue>) -> Result<&str, AccessError> {
    params
        .and_then(|p| p.object_get("uri"))
        .and_then(JsonValue::as_str)
        .ok_or_else(|| invalid("resource URI is required"))
}
pub(crate) fn contents(
    uri: &str,
    text: &str,
    id_bytes: usize,
    profile: AccessProfile,
) -> Result<Vec<u8>, AccessError> {
    let size = json_string_len(uri)
        .and_then(|a| json_string_len(text).and_then(|b| a.checked_add(b)))
        .and_then(|v| v.checked_add(id_bytes))
        .and_then(|v| v.checked_add(128));
    if !size.is_some_and(|v| v <= profile.max_mcp_frame_bytes) {
        return Err(AccessError::new(
            AccessErrorCode::BudgetExceeded,
            "resource response frame exceeds byte budget",
        ));
    }
    let mut out = b"{\"contents\":[{\"uri\":".to_vec();
    out.extend(json_string(uri));
    out.extend_from_slice(b",\"mimeType\":\"text/plain\",\"text\":");
    out.extend(json_string(text));
    out.extend_from_slice(b"}]}");
    Ok(out)
}
