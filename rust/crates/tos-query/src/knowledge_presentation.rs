//! Packet-local display and scene mapping. These carriers retain source
//! materializations and never synthesize semantic admission or translation.
use crate::{
    knowledge_lens_spec::{
        LensVocabulary, array, boolean, corrupt, field, get, language, lower, number, remove, set,
        string, strip,
    },
    search_v2::SearchV2Error,
    source_read_projection::{object, text},
};
use std::collections::{BTreeMap, BTreeSet};
use tos_foundation::{CanonicalProfile, JsonLimits, JsonValue, canonical_bytes_v1};

fn forms(value: &JsonValue) -> BTreeMap<String, String> {
    value
        .as_object()
        .unwrap_or(&[])
        .iter()
        .filter_map(|(key, value)| {
            let key = key.as_str()?;
            if !matches!(key, "default" | "original") && !language(key) {
                return None;
            }
            let value = strip(value.as_str()?);
            (!value.is_empty()).then(|| (key.to_owned(), value.to_owned()))
        })
        .collect()
}
pub fn select_display_form(
    value: &JsonValue,
    requested: &str,
    original: Option<&str>,
) -> JsonValue {
    let forms = forms(value);
    let available: Vec<_> = forms.keys().cloned().collect();
    let mut selected = None;
    let mut reason = "missing";
    if !matches!(requested, "auto" | "original") {
        let mut candidate = requested.to_owned();
        while !candidate.is_empty() {
            let matches: Vec<_> = available
                .iter()
                .filter(|k| lower(k) == lower(&candidate))
                .collect();
            if matches.len() > 1 {
                return object(vec![
                    ("requested_language", text(requested)),
                    ("selected_key", JsonValue::Null),
                    ("actual_language", JsonValue::Null),
                    ("text", JsonValue::Null),
                    ("reason", text("ambiguous-language-key")),
                    (
                        "available_keys",
                        JsonValue::Array(available.iter().map(|k| text(k)).collect()),
                    ),
                ]);
            }
            if let Some(key) = matches.first() {
                selected = Some((*key).to_owned());
                reason = if candidate == requested {
                    "exact-language"
                } else {
                    "less-specific-language"
                };
                break;
            }
            candidate = less_specific(&candidate);
        }
    } else if requested == "original" && forms.contains_key("original") {
        selected = Some("original".to_owned());
        reason = "original-role";
    }
    if selected.is_none() {
        selected = ["default", "ru", "en", "original"]
            .iter()
            .find(|k| forms.contains_key(**k))
            .map(|k| (*k).to_owned())
            .or_else(|| available.first().cloned());
        if selected.is_some() {
            reason = if requested == "auto" {
                "automatic"
            } else {
                "fallback"
            };
        }
    }
    let actual = selected.as_deref().and_then(|s| {
        if s == "original" {
            original
        } else if s == "default" {
            None
        } else {
            Some(s)
        }
    });
    object(vec![
        ("requested_language", text(requested)),
        (
            "selected_key",
            selected.as_deref().map(text).unwrap_or(JsonValue::Null),
        ),
        (
            "actual_language",
            actual.map(text).unwrap_or(JsonValue::Null),
        ),
        (
            "text",
            selected
                .as_ref()
                .and_then(|k| forms.get(k))
                .map(|v| text(v))
                .unwrap_or(JsonValue::Null),
        ),
        ("reason", text(reason)),
        (
            "available_keys",
            JsonValue::Array(available.iter().map(|k| text(k)).collect()),
        ),
    ])
}
fn less_specific(candidate: &str) -> String {
    let mut s = candidate.rsplit_once('-').map(|(s, _)| s).unwrap_or("");
    if s.rsplit('-').next().is_some_and(|p| p.len() == 1) {
        s = s.rsplit_once('-').map(|(s, _)| s).unwrap_or("")
    }
    s.to_owned()
}
fn display_selection(item: &JsonValue, lang: &str) -> JsonValue {
    let display = get(item, "display");
    let semantics = get(item, "semantics");
    let original = field(semantics, "language_context.language.original_language")
        .as_str()
        .filter(|s| language(s));
    let relation = item.object_get("from_id").is_some();
    let fields = if relation {
        &["label", "inverse_label", "statement", "explanation"][..]
    } else {
        &["title", "kind_label", "summary"][..]
    };
    let mut selected = object(vec![]);
    for name in fields {
        let mut selection = select_display_form(
            get(display, name),
            lang,
            if *name == "title" { original } else { None },
        );
        let provenance = get(display, "provenance");
        let quotation = get(provenance, "summary_source_language")
            .as_str()
            .filter(|s| !matches!(*s, "default" | "original" | "auto") && language(s));
        if *name == "summary"
            && matches!(
                string(get(&selection, "selected_key")),
                "default" | "original"
            )
            && string(field(semantics, "record_version.status")) == "available"
            && string(get(provenance, "summary")) == "exact-record-quotation"
        {
            if let Some(q) = quotation {
                set(&mut selection, "actual_language", text(q));
            }
        }
        let missing = match *name {
            "title" => matches!(
                get(provenance, "source_title_available"),
                JsonValue::Bool(false)
            ),
            "summary" => matches!(
                get(provenance, "source_summary_available"),
                JsonValue::Bool(false)
            ),
            "explanation" => matches!(
                get(provenance, "source_explanation_available"),
                JsonValue::Bool(false)
            ),
            _ => false,
        };
        let key = get(&selection, "selected_key").as_str().map(str::to_owned);
        let content = !matches!(get(&selection, "text"), JsonValue::Null) && !missing;
        set(
            &mut selection,
            "content_available",
            JsonValue::Bool(content),
        );
        set(
            &mut selection,
            "source_form_pointer",
            key.map(|k| text(&format!("/display/{name}/{k}")))
                .unwrap_or(JsonValue::Null),
        );
        set(&mut selected, name, selection);
    }
    object(vec![
        ("schema_version", text("tos_display_selection_v1")),
        ("content_revision", get(item, "content_revision").clone()),
        ("fields", selected),
        (
            "essential_context_pointers",
            JsonValue::Array(
                array(get(semantics, "assertion_contexts"))
                    .iter()
                    .enumerate()
                    .map(|(i, _)| text(&format!("/semantics/assertion_contexts/{i}")))
                    .collect(),
            ),
        ),
        ("performs_translation", JsonValue::Bool(false)),
        ("is_semantic_assessment", JsonValue::Bool(false)),
    ])
}
pub fn lens_carrier(
    item: &JsonValue,
    detail: &str,
    lang: Option<&str>,
) -> Result<JsonValue, SearchV2Error> {
    let mut out = item.clone();
    if detail != "full" {
        remove(&mut out, "source_record");
        remove(&mut out, "readable_context");
        set(&mut out, "attributes", object(vec![]));
        if let Some(semantics) = item.object_get("semantics") {
            if let Some(claim) = semantics.object_get("claim") {
                if claim.object_get("source_canonical_json").is_some() {
                    let mut semantics = semantics.clone();
                    let mut claim = claim.clone();
                    remove(&mut claim, "source_canonical_json");
                    set(&mut semantics, "claim", claim);
                    set(&mut out, "semantics", semantics);
                }
            }
        }
    }
    if let Some(lang) = lang {
        set(&mut out, "display_selection", display_selection(item, lang));
        if field(item, "attributes.human_forms").as_array().is_some()
            || get(item, "attributes").object_get("human_forms").is_some()
        {
            set(
                &mut out,
                "human_form_selection",
                select_human_forms(item, lang)?,
            );
        }
    }
    Ok(out)
}
pub fn knowledge_scene(
    nodes: &[JsonValue],
    relations: &[JsonValue],
    focus_node: Option<&str>,
    focus_relation: Option<&str>,
    vocabulary: &LensVocabulary,
) -> Result<JsonValue, SearchV2Error> {
    let mut groups: BTreeMap<String, (Option<String>, Vec<&JsonValue>)> = BTreeMap::new();
    let mut by_node = BTreeMap::new();
    for n in nodes {
        let id = string(get(n, "id"));
        let entity = get(n, "entity_id")
            .as_str()
            .filter(|s| vocabulary.declared_entity(s));
        let vertex = if let Some(entity) = entity {
            format!("tos-scene:entity:{entity}")
        } else {
            format!("tos-scene:carrier:{id}")
        };
        by_node.insert(id.to_owned(), vertex.clone());
        groups
            .entry(vertex)
            .or_insert_with(|| (entity.map(str::to_owned), vec![]))
            .1
            .push(n);
    }
    let mut vertices = vec![];
    for (id, (entity, nodes)) in groups {
        let representative = nodes
            .iter()
            .min_by_key(|n| (vocabulary.priority(n), string(get(n, "id"))))
            .ok_or_else(|| corrupt("empty scene vertex"))?;
        let mut ids: Vec<_> = nodes.iter().map(|n| string(get(n, "id"))).collect();
        ids.sort();
        vertices.push(object(vec![
            ("id", text(&id)),
            (
                "entity_id",
                entity.as_deref().map(text).unwrap_or(JsonValue::Null),
            ),
            (
                "node_ids",
                JsonValue::Array(ids.into_iter().map(text).collect()),
            ),
            ("representative_node_id", get(representative, "id").clone()),
        ]));
    }
    let mut sorted: Vec<_> = relations.iter().collect();
    sorted.sort_by_key(|r| string(get(r, "id")));
    let mut arcs = vec![];
    let mut collapsed = vec![];
    for r in sorted {
        let id = string(get(r, "id"));
        let left = by_node
            .get(string(get(r, "from_id")))
            .ok_or_else(|| corrupt("scene endpoint missing"))?;
        let right = by_node
            .get(string(get(r, "to_id")))
            .ok_or_else(|| corrupt("scene endpoint missing"))?;
        if left == right
            && string(get(r, "relation_type_id")) == "tos.relation.projects"
            && Some(id) != focus_relation
        {
            collapsed.push(text(id))
        } else {
            arcs.push(object(vec![
                ("relation_id", text(id)),
                ("from_id", text(left)),
                ("to_id", text(right)),
            ]));
        }
    }
    let compact = compact_claim_scene(
        nodes,
        relations,
        &vertices,
        &arcs,
        &by_node,
        focus_node,
        focus_relation,
    )?;
    Ok(object(vec![
        ("schema_version", text("tos_knowledge_scene_v1")),
        ("vertices", JsonValue::Array(vertices)),
        ("arcs", JsonValue::Array(arcs)),
        ("collapsed_relation_ids", JsonValue::Array(collapsed)),
        (
            "focus_vertex_id",
            focus_node
                .and_then(|id| by_node.get(id))
                .map(|s| text(s))
                .unwrap_or(JsonValue::Null),
        ),
        ("compact", compact),
        ("scope", text("returned-packet-only")),
        ("identity_rule", text("declared-tos-entity-id")),
        (
            "authority",
            text("presentation-mapping-not-semantic-admission"),
        ),
    ]))
}
fn compact_claim_scene(
    nodes: &[JsonValue],
    relations: &[JsonValue],
    vertices: &[JsonValue],
    arcs: &[JsonValue],
    by_node: &BTreeMap<String, String>,
    focus_node: Option<&str>,
    focus_relation: Option<&str>,
) -> Result<JsonValue, SearchV2Error> {
    let by_relation: BTreeMap<_, _> = relations
        .iter()
        .map(|r| (string(get(r, "id")), r))
        .collect();
    let mut outgoing: BTreeMap<&str, Vec<&JsonValue>> = BTreeMap::new();
    let mut incident: BTreeMap<&str, Vec<&JsonValue>> = BTreeMap::new();
    for r in relations {
        outgoing
            .entry(string(get(r, "from_id")))
            .or_default()
            .push(r);
    }
    for a in arcs {
        incident
            .entry(string(get(a, "from_id")))
            .or_default()
            .push(a);
        if get(a, "from_id") != get(a, "to_id") {
            incident.entry(string(get(a, "to_id"))).or_default().push(a);
        }
    }
    let claims: BTreeMap<_, _> = nodes
        .iter()
        .filter(|n| {
            string(get(n, "type_id")) == "tos.entity.claim"
                || array(field(n, "semantics.type_ancestors"))
                    .iter()
                    .any(|t| string(t) == "tos.entity.claim")
        })
        .map(|n| (string(get(n, "id")), n))
        .collect();
    let mut candidates: BTreeMap<&str, (&JsonValue, &JsonValue, Vec<String>)> = BTreeMap::new();
    let mut reasons: BTreeMap<String, String> = BTreeMap::new();
    for (id, n) in &claims {
        let claim = field(n, "semantics.claim");
        let subject = string(get(claim, "subject_node_id"));
        let object_id = string(get(claim, "object_node_id"));
        let reason = if !by_node.contains_key(subject) || !by_node.contains_key(object_id) {
            Some("incomplete-claim-contract")
        } else if string(get(claim, "predicate_mapping_status")) != "mapped"
            || string(get(claim, "relation_type_id")).is_empty()
        {
            Some("unmapped-claim-predicate")
        } else if by_node.get(*id) == by_node.get(subject)
            || by_node.get(*id) == by_node.get(object_id)
        {
            Some("claim-endpoint-identity-collision")
        } else {
            None
        };
        if let Some(reason) = reason {
            reasons.insert((*id).to_owned(), reason.into());
            continue;
        }
        let edges = outgoing.get(id).map(Vec::as_slice).unwrap_or(&[]);
        let legs: Vec<Vec<_>> = ["tos.relation.has-subject", "tos.relation.has-object"]
            .iter()
            .map(|kind| {
                edges
                    .iter()
                    .filter(|r| string(get(r, "relation_type_id")) == *kind)
                    .copied()
                    .collect()
            })
            .collect();
        if legs.iter().any(|l| l.len() != 1)
            || string(get(legs[0][0], "to_id")) != subject
            || string(get(legs[1][0], "to_id")) != object_id
        {
            reasons.insert((*id).to_owned(), "incomplete-or-ambiguous-path".into());
            continue;
        }
        let members = get(claim, "value_member_node_ids");
        let member_edges: Vec<_> = edges
            .iter()
            .filter(|r| string(get(r, "relation_type_id")) == "tos.relation.claim-value-member")
            .copied()
            .collect();
        if claim.object_get("value_member_node_ids").is_some() || !member_edges.is_empty() {
            let a = array(members);
            let ids: BTreeSet<_> = a.iter().map(string).collect();
            let targets: BTreeSet<_> = member_edges
                .iter()
                .map(|r| string(get(r, "to_id")))
                .collect();
            if members.as_array().is_none()
                || a.is_empty()
                || a.iter()
                    .any(|id| id.as_str().is_none() || !by_node.contains_key(string(id)))
                || ids.len() != a.len()
                || member_edges.len() != a.len()
                || ids != targets
            {
                reasons.insert((*id).to_owned(), "incomplete-value-member-context".into());
                continue;
            }
        }
        candidates.insert(
            id,
            (
                n,
                claim,
                legs.iter()
                    .map(|l| string(get(l[0], "id")).to_owned())
                    .collect(),
            ),
        );
    }
    let focus_vertex = focus_node.and_then(|id| by_node.get(id));
    let mut folded = BTreeSet::new();
    let mut removed = BTreeSet::new();
    let mut paths = vec![];
    let mut details_vertices = BTreeSet::new();
    for vertex in vertices {
        let vertex_id = string(get(vertex, "id"));
        let ids: Vec<_> = array(get(vertex, "node_ids")).iter().map(string).collect();
        let local: Vec<_> = ids
            .iter()
            .filter(|id| claims.contains_key(**id))
            .copied()
            .collect();
        if local.is_empty() {
            continue;
        }
        let incident = incident.get(vertex_id).map(Vec::as_slice).unwrap_or(&[]);
        let mut reason = if focus_relation
            .is_some_and(|id| incident.iter().any(|a| string(get(a, "relation_id")) == id))
        {
            Some("focus-relation")
        } else if focus_vertex.map(String::as_str) == Some(vertex_id) {
            Some("focus-claim")
        } else if ids.iter().any(|id| !candidates.contains_key(id)) {
            Some("mixed-or-incomplete-claim-carriers")
        } else {
            None
        };
        if reason.is_none() {
            let legs: BTreeSet<_> = ids
                .iter()
                .flat_map(|id| candidates[id].2.iter().map(String::as_str))
                .collect();
            for arc in incident {
                let relation_id = string(get(arc, "relation_id"));
                if legs.contains(relation_id) {
                    continue;
                }
                let r = by_relation[relation_id];
                if !ids.contains(&string(get(r, "from_id")))
                    || !matches!(
                        string(get(r, "relation_type_id")),
                        "tos.relation.claim-supported-by" | "tos.relation.claim-value-member"
                    )
                    || string(get(arc, "to_id")) == vertex_id
                {
                    reason = Some("nonfoldable-incident-relation");
                    break;
                }
                if focus_vertex.map(String::as_str) == Some(string(get(arc, "to_id"))) {
                    reason = Some("focus-detail");
                    break;
                }
            }
        }
        if reason.is_some_and(|r| r != "focus-claim")
            || (reason == Some("focus-claim")
                && local.iter().any(|id| !candidates.contains_key(id)))
        {
            for id in local {
                reasons
                    .entry(id.to_owned())
                    .or_insert(reason.unwrap().to_owned());
            }
            continue;
        }
        if reason.is_none() {
            folded.insert(vertex_id.to_owned());
        }
        for id in ids {
            let Some((node, claim, legs)) = candidates.get(id) else {
                continue;
            };
            let mut details: Vec<_> = outgoing
                .get(id)
                .into_iter()
                .flatten()
                .filter(|r| {
                    matches!(
                        string(get(r, "relation_type_id")),
                        "tos.relation.claim-supported-by" | "tos.relation.claim-value-member"
                    )
                })
                .map(|r| string(get(r, "id")))
                .collect();
            details.sort();
            for r in legs
                .iter()
                .map(String::as_str)
                .chain(details.iter().copied())
            {
                removed.insert(r.to_owned());
            }
            for r in &details {
                details_vertices.insert(by_node[string(get(by_relation[r], "to_id"))].clone());
            }
            let mut wording = None;
            let mut mode = "claim-with-mandatory-context";
            let selected = get(node, "human_form_selection");
            for role in ["caption", "statement", "hover"] {
                if string(field(selected, &format!("roles.{role}.state"))) == "ready" {
                    let shared =
                        string(get(selected, "schema_version")) == "tos_human_form_selection_v2";
                    wording = Some(format!(
                        "/human_form_selection/roles/{role}{}",
                        if shared { "" } else { "/packet" }
                    ));
                    if shared {
                        mode = "claim-with-shared-form-context-v2"
                    }
                    break;
                }
            }
            if wording.is_none() {
                for name in ["summary", "title"] {
                    if boolean(field(
                        node,
                        &format!("display_selection.fields.{name}.content_available"),
                    )) {
                        wording = Some(format!("/display_selection/fields/{name}"));
                        break;
                    }
                }
            }
            let mut relation_context: Vec<_> = legs.iter().map(|s| text(s)).collect();
            relation_context.extend(details.iter().map(|s| text(s)));
            paths.push(object(vec![
                ("id", text(&format!("tos-scene:claim-path:{id}"))),
                (
                    "from_id",
                    text(&by_node[string(get(claim, "subject_node_id"))]),
                ),
                (
                    "to_id",
                    text(&by_node[string(get(claim, "object_node_id"))]),
                ),
                ("claim_node_id", text(id)),
                ("relation_type_id", get(claim, "relation_type_id").clone()),
                (
                    "node_ids",
                    JsonValue::Array(vec![
                        get(claim, "subject_node_id").clone(),
                        text(id),
                        get(claim, "object_node_id").clone(),
                    ]),
                ),
                (
                    "relation_ids",
                    JsonValue::Array(legs.iter().map(|s| text(s)).collect()),
                ),
                (
                    "detail_relation_ids",
                    JsonValue::Array(details.iter().map(|s| text(s)).collect()),
                ),
                (
                    "reading",
                    object(vec![
                        ("mode", text(mode)),
                        ("node_id", text(id)),
                        ("content_revision", get(node, "content_revision").clone()),
                        (
                            "wording_pointer",
                            wording.as_deref().map(text).unwrap_or(JsonValue::Null),
                        ),
                        (
                            "wording_state",
                            text(if wording.is_some() {
                                "available"
                            } else {
                                "missing"
                            }),
                        ),
                        (
                            "context_pointers",
                            JsonValue::Array(["/semantics", "/epistemic"].map(text).to_vec()),
                        ),
                        ("relation_context_ids", JsonValue::Array(relation_context)),
                        ("standalone", JsonValue::Bool(false)),
                    ]),
                ),
            ]));
        }
    }
    let retained: Vec<_> = arcs
        .iter()
        .filter(|a| !removed.contains(string(get(a, "relation_id"))))
        .collect();
    let endpoints: BTreeSet<_> = retained
        .iter()
        .copied()
        .chain(paths.iter())
        .flat_map(|a| [string(get(a, "from_id")), string(get(a, "to_id"))])
        .collect();
    let claim_vertices: BTreeSet<_> = claims.keys().map(|id| by_node[*id].as_str()).collect();
    for id in details_vertices {
        if !endpoints.contains(id.as_str())
            && focus_vertex != Some(&id)
            && !claim_vertices.contains(id.as_str())
        {
            folded.insert(id);
        }
    }
    paths.sort_by(|a, b| string(get(a, "id")).cmp(string(get(b, "id"))));
    Ok(object(vec![
        ("rule", text("explicit-claim-paths-v1")),
        (
            "vertex_ids",
            JsonValue::Array(
                vertices
                    .iter()
                    .filter(|v| !folded.contains(string(get(v, "id"))))
                    .map(|v| get(v, "id").clone())
                    .collect(),
            ),
        ),
        (
            "relation_ids",
            JsonValue::Array(
                retained
                    .iter()
                    .map(|a| get(a, "relation_id").clone())
                    .collect(),
            ),
        ),
        ("claim_paths", JsonValue::Array(paths)),
        (
            "folded_vertex_ids",
            JsonValue::Array(folded.iter().map(|s| text(s)).collect()),
        ),
        (
            "retained_claims",
            JsonValue::Array(
                reasons
                    .iter()
                    .map(|(id, r)| object(vec![("node_id", text(id)), ("reason", text(r))]))
                    .collect(),
            ),
        ),
        ("authority", text("presentation-only-no-new-assertion")),
    ]))
}

pub const HUMAN_FORM_ROLES: [&str; 7] = [
    "name",
    "caption",
    "hover",
    "statement",
    "grounds",
    "history",
    "technical",
];
fn exact_ref(v: &JsonValue) -> bool {
    v.as_object().is_some_and(|o| o.len() == 3)
        && v.object_get("id")
            .and_then(JsonValue::as_str)
            .is_some_and(|s| !s.is_empty())
        && get(v, "version")
            .as_u64()
            .is_some_and(|n| n > 0 && n <= 9_007_199_254_740_991)
        && get(v, "digest")
            .as_str()
            .is_some_and(|s| tos_foundation::Digest256::from_prefixed(s).is_ok())
}
fn json_same(a: &JsonValue, b: &JsonValue) -> bool {
    canonical_bytes_v1(
        a,
        CanonicalProfile::SourceRecordDigestV1,
        JsonLimits::default(),
    )
    .ok()
        == canonical_bytes_v1(
            b,
            CanonicalProfile::SourceRecordDigestV1,
            JsonLimits::default(),
        )
        .ok()
}
fn member_count(v: &JsonValue) -> usize {
    match v {
        JsonValue::Array(a) => 1 + a.iter().map(member_count).sum::<usize>(),
        JsonValue::Object(o) => 1 + o.iter().map(|(_, v)| 1 + member_count(v)).sum::<usize>(),
        _ => 1,
    }
}
fn form_cost(v: &JsonValue, limit: usize) -> Result<usize, SearchV2Error> {
    fn walk(
        v: &JsonValue,
        depth: usize,
        cost: &mut usize,
        members: &mut usize,
        limit: usize,
    ) -> Result<(), SearchV2Error> {
        *members += 1;
        if depth > 64 || *members > 30000 {
            return Err(corrupt("human form JSON structural bounds exceeded"));
        }
        let n = match v {
            JsonValue::String(_) => canonical_bytes_v1(
                v,
                CanonicalProfile::SourceRecordDigestV1,
                JsonLimits::default(),
            )
            .map_err(|_| corrupt("human form string invalid"))?
            .len(),
            JsonValue::Number(n) => 32.max(n.lexeme.len()),
            JsonValue::Null | JsonValue::Bool(_) => 5,
            JsonValue::Array(a) => {
                *cost += 2 + a.len();
                for child in a {
                    walk(child, depth + 1, cost, members, limit)?
                }
                0
            }
            JsonValue::Object(o) => {
                *cost += 2 + 2 * o.len();
                for (k, v) in o {
                    if matches!(k.as_str(), Some("__proto__" | "prototype" | "constructor")) {
                        return Err(corrupt("human form unsafe key"));
                    }
                    walk(
                        &text(
                            k.as_str()
                                .ok_or_else(|| corrupt("human form key invalid"))?,
                        ),
                        depth + 1,
                        cost,
                        members,
                        limit,
                    )?;
                    walk(v, depth + 1, cost, members, limit)?;
                }
                0
            }
        };
        *cost = cost
            .checked_add(n)
            .ok_or_else(|| corrupt("human form cost overflow"))?;
        if *cost > limit {
            return Err(corrupt("human form byte budget exceeded"));
        }
        Ok(())
    }
    let mut cost = 0;
    walk(v, 0, &mut cost, &mut 0, limit)?;
    Ok(cost)
}
fn common(values: &[JsonValue]) -> JsonValue {
    let mut out = object(vec![]);
    if let Some(first) = values.first() {
        for (k, v) in first.as_object().unwrap_or(&[]) {
            let k = k.as_str().unwrap_or("");
            if values.iter().skip(1).any(|v| v.object_get(k).is_none()) {
                continue;
            }
            let members: Vec<_> = values.iter().map(|v| get(v, k).clone()).collect();
            if members.iter().skip(1).all(|m| json_same(v, m)) {
                set(&mut out, k, v.clone())
            } else if members.iter().all(|m| m.as_object().is_some()) {
                let nested = common(&members);
                if nested.as_object().is_some_and(|o| !o.is_empty()) {
                    set(&mut out, k, nested)
                }
            }
        }
    }
    out
}
fn subtract(value: &JsonValue, base: &JsonValue) -> JsonValue {
    let mut out = object(vec![]);
    for (k, v) in value.as_object().unwrap_or(&[]) {
        let k = k.as_str().unwrap_or("");
        if let Some(b) = base.object_get(k) {
            if !json_same(v, b) {
                set(&mut out, k, subtract(v, b))
            }
        } else {
            set(&mut out, k, v.clone())
        }
    }
    out
}
fn encode_selection(logical: &JsonValue, enforce: bool) -> Result<JsonValue, SearchV2Error> {
    form_cost(logical, 524288)?;
    let mut packets = BTreeMap::new();
    let mut shared_limits: Vec<JsonValue> = vec![];
    for role in HUMAN_FORM_ROLES {
        let chosen = field(logical, &format!("roles.{role}"));
        if !matches!(get(chosen, "packet"), JsonValue::Null) {
            let mut packet = get(chosen, "packet").clone();
            form_cost(&packet, 65536)?;
            if !exact_ref(get(chosen, "form"))
                || !json_same(get(chosen, "form"), get(&packet, "form"))
            {
                return Err(corrupt("human form exact binding differs"));
            }
            remove(&mut packet, "form");
            if get(&packet, "admission").as_object().is_some() {
                let mut admission = get(&packet, "admission").clone();
                if admission.object_get("limit_refs").is_some() {
                    return Err(corrupt("reserved human form limit_refs"));
                }
                if let Some(limits) = admission.object_get("limits") {
                    let mut refs = vec![];
                    for limit in limits
                        .as_array()
                        .ok_or_else(|| corrupt("invalid human form limits"))?
                    {
                        if limit.as_str().is_none() {
                            return Err(corrupt("invalid human form limit"));
                        }
                        let index = shared_limits
                            .iter()
                            .position(|v| v == limit)
                            .unwrap_or_else(|| {
                                shared_limits.push(limit.clone());
                                shared_limits.len() - 1
                            });
                        refs.push(number(index));
                    }
                    remove(&mut admission, "limits");
                    set(&mut admission, "limit_refs", JsonValue::Array(refs));
                    set(&mut packet, "admission", admission);
                }
            }
            packets.insert(role, packet);
        }
    }
    if shared_limits.len() > 512 {
        return Err(corrupt("excessive shared human form limits"));
    }
    let base = common(&packets.values().cloned().collect::<Vec<_>>());
    let mut out = logical.clone();
    set(
        &mut out,
        "schema_version",
        text("tos_human_form_selection_v2"),
    );
    set(&mut out, "packet_base", base.clone());
    set(&mut out, "shared_limits", JsonValue::Array(shared_limits));
    let mut roles = get(&out, "roles").clone();
    for role in HUMAN_FORM_ROLES {
        let mut selected = get(&roles, role).clone();
        remove(&mut selected, "packet");
        set(
            &mut selected,
            "packet_delta",
            packets
                .get(role)
                .map(|p| subtract(p, &base))
                .unwrap_or(JsonValue::Null),
        );
        set(&mut roles, role, selected);
    }
    set(&mut out, "roles", roles);
    form_cost(&out, if enforce { 16384 } else { 524288 })?;
    Ok(out)
}
fn empty_role() -> JsonValue {
    object(vec![
        ("state", text("missing")),
        ("reason", text("no-ready-form")),
        ("form", JsonValue::Null),
        ("packet", JsonValue::Null),
    ])
}
fn empty_roles() -> JsonValue {
    object(
        HUMAN_FORM_ROLES
            .iter()
            .map(|role| (*role, empty_role()))
            .collect(),
    )
}
fn stop_selection(
    result: &JsonValue,
    state: &str,
    issue: &str,
) -> Result<JsonValue, SearchV2Error> {
    let mut out = result.clone();
    set(&mut out, "state", text(state));
    set(&mut out, "roles", empty_roles());
    if state == "over-budget" {
        set(&mut out, "source_ref", JsonValue::Null)
    }
    set(&mut out, "candidates", JsonValue::Array(vec![]));
    set(&mut out, "issues", JsonValue::Array(vec![text(issue)]));
    if encode_selection(&out, false).is_err() {
        set(&mut out, "source_ref", JsonValue::Null)
    }
    encode_selection(&out, true)
}
fn valid_pointer(p: &str) -> bool {
    if !p.is_empty() && !p.starts_with('/') {
        return false;
    }
    let mut chars = p.chars();
    while let Some(c) = chars.next() {
        if c == '~' && !matches!(chars.next(), Some('0' | '1')) {
            return false;
        }
    }
    true
}
fn valid_binding(v: &JsonValue) -> bool {
    v.as_object().is_some_and(|o| o.len() == 2)
        && exact_ref(get(v, "record"))
        && get(v, "pointer").as_str().is_some_and(valid_pointer)
}
fn snapshot_valid(p: &JsonValue) -> bool {
    let Some(s) = p.object_get("assessment_snapshot") else {
        return true;
    };
    if s.as_object().is_none()
        || get(s, "owner_snapshot")
            .as_str()
            .is_none_or(|s| tos_foundation::Digest256::from_prefixed(s).is_err())
        || !matches!(get(s, "publication_authorized"), JsonValue::Bool(false))
        || !matches!(get(s, "current_runtime_grant"), JsonValue::Bool(false))
    {
        return false;
    }
    let Some(count) = get(s, "journal_batches")
        .as_u64()
        .filter(|n| *n <= 9_007_199_254_740_991)
    else {
        return false;
    };
    if s.object_get("journal_revision").is_none()
        || matches!(get(s, "journal_revision"), JsonValue::Null) != (count == 0)
        || (count > 0
            && get(s, "journal_revision")
                .as_str()
                .is_none_or(|s| tos_foundation::Digest256::from_hex(s).is_err()))
    {
        return false;
    }
    if (s.object_get("subject_assessment_required").is_some()
        || p.object_get("subject_assessment").is_some())
        && (!boolean(get(s, "subject_assessment_required"))
            || p.object_get("subject_assessment").is_none())
    {
        return false;
    }
    if string(get(p, "state")) != "ready" {
        return true;
    }
    let a = get(p, "admission");
    count > 0
        && matches!(string(get(p, "derivation")), "freeform" | "source-copy")
        && string(get(a, "schema_version")) == "tos_knowledge_admission_v1"
        && exact_ref(get(a, "subject"))
        && json_same(get(a, "subject"), get(p, "form"))
        && exact_ref(get(a, "policy"))
        && matches!(
            string(get(a, "status")),
            "admitted" | "admitted-with-limits"
        )
        && boolean(get(a, "can_use"))
        && matches!(get(a, "is_semantic_evaluation"), JsonValue::Bool(false))
        && !string(get(a, "use")).is_empty()
}
fn subject_assessment_valid(p: &JsonValue) -> bool {
    let Some(s) = p.object_get("subject_assessment") else {
        return true;
    };
    let keys = [
        "schema_version",
        "subject",
        "admission",
        "journal_revision",
        "journal_batches",
        "historical_withdrawals",
        "form_admission_is_parent_endorsement",
    ];
    if s.as_object().is_none_or(|o| {
        o.len() != keys.len()
            || o.iter()
                .any(|(k, _)| !keys.contains(&k.as_str().unwrap_or("")))
    }) || string(get(s, "schema_version")) != "tos_human_form_subject_assessment_v1"
        || !exact_ref(get(s, "subject"))
        || !json_same(get(s, "subject"), get(p, "subject"))
        || !matches!(
            get(s, "form_admission_is_parent_endorsement"),
            JsonValue::Bool(false)
        )
    {
        return false;
    }
    let Some(count) = get(s, "journal_batches")
        .as_u64()
        .filter(|n| *n <= 9_007_199_254_740_991)
    else {
        return false;
    };
    if matches!(get(s, "journal_revision"), JsonValue::Null) != (count == 0)
        || (count > 0
            && get(s, "journal_revision")
                .as_str()
                .is_none_or(|s| tos_foundation::Digest256::from_hex(s).is_err()))
    {
        return false;
    }
    let Some(w) = get(s, "historical_withdrawals")
        .as_array()
        .filter(|a| a.len() <= 256 && a.iter().all(exact_ref) && (count > 0 || a.is_empty()))
    else {
        return false;
    };
    let _ = w;
    let a = get(s, "admission");
    if string(get(a, "schema_version")) != "tos_knowledge_admission_v1"
        || !exact_ref(get(a, "subject"))
        || !json_same(get(a, "subject"), get(s, "subject"))
        || !exact_ref(get(a, "policy"))
        || !matches!(
            string(get(a, "status")),
            "admitted"
                | "admitted-with-limits"
                | "disputed"
                | "rejected"
                | "deferred"
                | "unreviewed"
        )
        || !matches!(get(a, "can_use"), JsonValue::Bool(_))
        || !matches!(get(a, "is_semantic_evaluation"), JsonValue::Bool(false))
        || string(get(a, "use")).is_empty()
        || get(a, "limits")
            .as_array()
            .is_none_or(|a| a.iter().any(|v| v.as_str().is_none()))
    {
        return false;
    }
    if let Some(f) = p.object_get("admission") {
        if f.as_object().is_none()
            || get(f, "use") != get(a, "use")
            || !exact_ref(get(f, "policy"))
            || !json_same(get(f, "policy"), get(a, "policy"))
        {
            return false;
        }
    }
    string(get(p, "state")) != "ready"
        || (matches!(get(p, "standalone_reading"), JsonValue::Bool(false))
            && matches!(string(get(p, "derivation")), "source-copy" | "freeform")
            && get(p, "admission").as_object().is_some())
}
fn language_context_valid(p: &JsonValue) -> bool {
    let Some(c) = p.object_get("language_context") else {
        return true;
    };
    let binding = get(c, "binding");
    let value = get(c, "value");
    let deps = array(get(p, "dependencies"));
    let entries = array(get(p, "context"));
    let same_entry = |b: &JsonValue, v: &JsonValue| {
        entries
            .iter()
            .any(|e| json_same(get(e, "binding"), b) && json_same(get(e, "value"), v))
    };
    let dependency = |r: &JsonValue| deps.iter().any(|d| exact_ref(d) && json_same(d, r));
    if c.as_object().is_none_or(|o| o.len() != 2)
        || !valid_binding(binding)
        || !dependency(get(binding, "record"))
        || value.as_object().is_none()
        || ["language", "script", "relation", "source"]
            .iter()
            .any(|k| value.object_get(k).is_none())
        || p.object_get("script").is_none()
        || !json_same(get(value, "language"), get(p, "language"))
        || !json_same(get(value, "script"), get(p, "script"))
        || (!matches!(get(value, "script"), JsonValue::Null)
            && get(value, "script")
                .as_str()
                .is_none_or(|s| s.len() != 4 || !s.bytes().all(|c| c.is_ascii_alphabetic())))
        || !matches!(
            string(get(value, "relation")),
            "unknown" | "original" | "translation" | "transliteration" | "adaptation"
        )
        || !same_entry(binding, value)
    {
        return false;
    }
    let source = get(value, "source");
    if matches!(string(get(value, "relation")), "original" | "unknown") {
        matches!(source, JsonValue::Null)
    } else {
        valid_binding(source)
            && dependency(get(source, "record"))
            && entries.iter().any(|e| {
                json_same(get(e, "binding"), source)
                    && get(e, "value")
                        .as_str()
                        .is_some_and(|s| !strip(s).is_empty())
            })
    }
}
fn native_record_field(record: &JsonValue) -> Result<&'static str, ()> {
    let schema = string(get(record, "schema_version"));
    let (kind, key) = match schema {
        "tos_canonical_node_v1" => {
            let kind = string(get(record, "node_type"));
            if ![
                "source",
                "concept",
                "principle",
                "lineage",
                "event",
                "state",
                "support",
                "context",
                "analogy",
                "synthesis",
            ]
            .contains(&kind)
                || get(record, "record_version")
                    .as_u64()
                    .is_none_or(|n| n == 0 || n > 9_007_199_254_740_991)
            {
                return Err(());
            }
            (kind, "node_id")
        }
        "tos_scholarly_composite_witness_v1" => ("composite", "composite_id"),
        "tos_artifact_source_witness_v1" | "tos_artifact_source_witness_v2" => {
            ("artifact", "artifact_id")
        }
        _ => return Ok("record_id"),
    };
    let id = string(get(record, key));
    if record.object_get("record_id").is_some() || !id.starts_with(&format!("tos.{kind}.")) {
        return Err(());
    }
    if key == "node_id"
        && !id.strip_prefix(&format!("tos.{kind}.")).is_some_and(|b| {
            !b.is_empty()
                && b.split(['.', '-']).all(|s| {
                    !s.is_empty()
                        && s.bytes()
                            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
                })
        })
    {
        return Err(());
    }
    Ok(key)
}
fn select_human_forms(item: &JsonValue, lang: &str) -> Result<JsonValue, SearchV2Error> {
    let attrs = get(item, "attributes");
    let forms = get(attrs, "human_forms");
    let source_ref = get(attrs, "human_forms_source_ref")
        .as_str()
        .filter(|s| s.encode_utf16().count() <= 2048)
        .map(text)
        .unwrap_or(JsonValue::Null);
    let mut result = object(vec![
        ("schema_version", text("tos_human_form_selection_v1")),
        ("content_revision", get(item, "content_revision").clone()),
        ("requested_language", text(lang)),
        ("source_ref", source_ref),
        ("state", text("available")),
        ("roles", empty_roles()),
        ("candidates", JsonValue::Array(vec![])),
        ("issues", JsonValue::Array(vec![])),
        ("performs_translation", JsonValue::Bool(false)),
        ("performs_assessment", JsonValue::Bool(false)),
    ]);
    let Some(forms) = forms.as_array().filter(|a| a.len() <= 32) else {
        return stop_selection(&result, "invalid", "forms.invalid-or-excessive-collection");
    };
    let collection = JsonValue::Array(forms.to_vec());
    if form_cost(&collection, 32 * 65536).is_err()
        || forms
            .iter()
            .any(|p| get(p, "admission").object_get("limit_refs").is_some())
    {
        return stop_selection(&result, "invalid", "forms.invalid-shared-codec-input");
    }
    if forms.is_empty() {
        return encode_selection(&result, true).or_else(|_| {
            stop_selection(
                &result,
                "over-budget",
                "forms.inspect-collection-separately",
            )
        });
    }
    let record = get(attrs, "source_record");
    let claim = get(attrs, "source_claim");
    if !matches!(record, JsonValue::Null) && !matches!(claim, JsonValue::Null) {
        return stop_selection(&result, "invalid", "forms.ambiguous-source-record-binding");
    }
    let is_claim = matches!(record, JsonValue::Null) && claim.as_object().is_some();
    let record = if is_claim { claim } else { record };
    if record.as_object().is_none() {
        return stop_selection(&result, "invalid", "forms.missing-source-record-binding");
    }
    let key = if is_claim {
        "claim_id"
    } else {
        match native_record_field(record) {
            Ok(k) => k,
            Err(_) => {
                return stop_selection(&result, "invalid", "forms.invalid-source-record-binding");
            }
        }
    };
    let subject = object(vec![
        ("id", get(record, key).clone()),
        (
            "version",
            get(
                record,
                if is_claim {
                    "claim_version"
                } else {
                    "record_version"
                },
            )
            .clone(),
        ),
        (
            "digest",
            text(&format!("sha256:{}", string(get(attrs, "source_sha256")))),
        ),
    ]);
    if !exact_ref(&subject)
        || ((is_claim || key != "record_id") && get(item, "entity_id") != get(&subject, "id"))
    {
        return stop_selection(&result, "invalid", "forms.invalid-source-record-binding");
    }
    let mut ready = vec![];
    let mut seen = BTreeSet::new();
    let mut candidates = vec![];
    for (i, p) in forms.iter().enumerate() {
        if p.as_object().is_none()
            || string(get(p, "schema_version")) != "tos_human_form_materialization_v1"
            || !exact_ref(get(p, "form"))
            || !exact_ref(get(p, "subject"))
            || !json_same(get(p, "subject"), &subject)
            || !matches!(
                get(p, "performs_semantic_assessment"),
                JsonValue::Bool(false)
            )
        {
            return stop_selection(&result, "invalid", "forms.invalid-packet-or-source-binding");
        }
        if !seen.insert(string(field(p, "form.id"))) {
            return stop_selection(&result, "invalid", "forms.duplicate-current-identity");
        }
        let state = string(get(p, "state"));
        let role = get(p, "role");
        let actual = get(p, "language");
        if !matches!(
            state,
            "ready"
                | "invalid"
                | "unavailable"
                | "stale"
                | "restricted"
                | "needs-assessment"
                | "over-budget"
        ) {
            return stop_selection(&result, "invalid", "forms.unknown-materialization-state");
        }
        if !snapshot_valid(p) {
            return stop_selection(&result, "invalid", "forms.invalid-assessment-snapshot");
        }
        if !subject_assessment_valid(p) {
            return stop_selection(&result, "invalid", "forms.invalid-subject-assessment");
        }
        if (!matches!(role, JsonValue::Null) && !HUMAN_FORM_ROLES.contains(&string(role)))
            || (!matches!(actual, JsonValue::Null) && !actual.as_str().is_some_and(language))
        {
            return stop_selection(&result, "invalid", "forms.invalid-role-or-language");
        }
        candidates.push(object(vec![
            ("form", get(p, "form").clone()),
            ("role", role.clone()),
            ("language", actual.clone()),
            ("state", get(p, "state").clone()),
            (
                "source_pointer",
                text(&format!("/attributes/human_forms/{i}")),
            ),
        ]));
        if state != "ready" {
            if p.object_get("display_text").is_none()
                || !matches!(get(p, "display_text"), JsonValue::Null)
                || get(p, "context") != &JsonValue::Array(vec![])
            {
                return stop_selection(&result, "invalid", "forms.nonready-packet-has-wording");
            }
            continue;
        }
        let context = get(p, "context");
        if p.object_get("language").is_none()
            || !HUMAN_FORM_ROLES.contains(&string(role))
            || get(p, "display_text")
                .as_str()
                .is_none_or(|s| strip(s).is_empty())
            || context.as_array().is_none_or(|a| a.len() > 256)
            || !matches!(get(p, "standalone_reading"), JsonValue::Bool(_))
            || (!array(context).is_empty()
                && !matches!(get(p, "standalone_reading"), JsonValue::Bool(false)))
            || array(context).iter().any(|e| {
                e.as_object().is_none()
                    || ["slot", "binding", "value"]
                        .iter()
                        .any(|k| e.object_get(k).is_none())
                    || !exact_ref(field(e, "binding.record"))
                    || field(e, "binding.pointer").as_str().is_none()
            })
        {
            return stop_selection(&result, "invalid", "forms.incomplete-ready-packet");
        }
        if !language_context_valid(p) {
            return stop_selection(&result, "invalid", "forms.invalid-language-context");
        }
        ready.push(p);
    }
    set(&mut result, "candidates", JsonValue::Array(candidates));
    if encode_selection(&result, false).is_err()
        || encode_selection(&result, false)
            .ok()
            .and_then(|v| form_cost(&v, 16384).ok())
            .is_none()
    {
        return stop_selection(
            &result,
            "over-budget",
            "forms.inspect-collection-separately",
        );
    }
    let mut roles = get(&result, "roles").clone();
    let mut choices = vec![];
    for role in HUMAN_FORM_ROLES {
        let candidates: Vec<_> = ready
            .iter()
            .copied()
            .filter(|p| string(get(p, "role")) == role)
            .collect();
        let mut selected = candidates.clone();
        let mut reason = if lang == "auto" {
            "automatic"
        } else {
            "fallback"
        };
        if lang == "original" {
            selected = candidates
                .iter()
                .copied()
                .filter(|p| string(field(p, "language_context.value.relation")) == "original")
                .collect();
            if selected.is_empty() {
                let mut e = empty_role();
                set(&mut e, "state", text("unavailable"));
                set(&mut e, "reason", text("original-role-not-declared"));
                set(&mut roles, role, e);
                continue;
            }
            reason = "original";
        } else if lang != "auto" {
            let mut candidate = lang.to_owned();
            while !candidate.is_empty() {
                let matching: Vec<_> = candidates
                    .iter()
                    .copied()
                    .filter(|p| {
                        get(p, "language")
                            .as_str()
                            .is_some_and(|l| lower(l) == lower(&candidate))
                    })
                    .collect();
                if !matching.is_empty() {
                    selected = matching;
                    reason = if candidate == lang {
                        "exact-language"
                    } else {
                        "less-specific-language"
                    };
                    break;
                }
                candidate = less_specific(&candidate);
            }
        }
        let chosen = if selected.len() > 1 {
            object(vec![
                ("state", text("ambiguous")),
                ("reason", text("multiple-forms")),
                ("form", JsonValue::Null),
                ("packet", JsonValue::Null),
            ])
        } else if let Some(packet) = selected.first() {
            choices.push((role, reason, *packet));
            object(vec![
                ("state", text("over-budget")),
                ("reason", text("inspect-exact-form")),
                ("form", get(packet, "form").clone()),
                ("packet", JsonValue::Null),
            ])
        } else {
            let mut e = empty_role();
            if forms.iter().any(|p| string(get(p, "state")) != "ready") {
                set(&mut e, "state", text("unavailable"));
            }
            e
        };
        set(&mut roles, role, chosen);
    }
    set(&mut result, "roles", roles.clone());
    if encode_selection(&result, true).is_err() {
        return stop_selection(
            &result,
            "over-budget",
            "forms.inspect-collection-separately",
        );
    }
    choices.sort_by_key(|(_, reason, _)| match *reason {
        "less-specific-language" => 1,
        "fallback" => 2,
        _ => 0,
    });
    for (role, reason, packet) in choices {
        let reference = get(&roles, role).clone();
        set(
            &mut roles,
            role,
            object(vec![
                ("state", text("ready")),
                ("reason", text(reason)),
                ("form", get(packet, "form").clone()),
                ("packet", packet.clone()),
            ]),
        );
        set(&mut result, "roles", roles.clone());
        if encode_selection(&result, true).is_err() {
            set(&mut roles, role, reference);
            set(&mut result, "roles", roles.clone());
        }
    }
    encode_selection(&result, true).or_else(|_| {
        stop_selection(
            &result,
            "over-budget",
            "forms.inspect-collection-separately",
        )
    })
}
