//! Exact raw bibliographic rendering. Source resolution and schema validation
//! belong to the catalog-backed producer, never to this pure renderer.
use crate::{Error, Result};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use tos_foundation::{CanonicalProfile, Digest256, JsonLimits, canonical_raw_bytes_v1};

pub(crate) fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or(Error::Invalid("bibliographic required string"))
}
pub(crate) fn array<'a>(v: &'a Value, key: &str) -> Result<&'a [Value]> {
    v.get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or(Error::Invalid("bibliographic required array"))
}
pub(crate) fn encode(v: &Value, cap: usize) -> Result<Vec<u8>> {
    struct Count {
        n: usize,
        cap: usize,
    }
    impl std::io::Write for Count {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.n = self
                .n
                .checked_add(b.len())
                .filter(|n| *n <= self.cap)
                .ok_or_else(|| std::io::Error::other("bibliographic row cap"))?;
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer(&mut Count { n: 0, cap }, v)
        .map_err(|_| Error::Budget("bibliographic output bytes"))?;
    let raw = serde_json::to_vec(v).map_err(|_| Error::Invalid("bibliographic encode"))?;
    canonical_raw_bytes_v1(
        &raw,
        CanonicalProfile::SourceRecordDigestV1,
        JsonLimits::new(cap, 96, 1_000_000, 4096)
            .map_err(|_| Error::Budget("bibliographic JSON limits"))?,
    )
    .map_err(|e| Error::Source(e.to_string()))
}
pub(crate) fn digest(v: &Value, cap: usize) -> Result<String> {
    Ok(Digest256::of_bytes(&encode(v, cap)?).to_hex())
}
pub(crate) fn node_id(kind: &str, reference: &str) -> String {
    if matches!(kind, "identity" | "claim" | "provenance_event" | "review") {
        format!("{kind}:{reference}")
    } else {
        format!(
            "{kind}:sha256:{}",
            Digest256::of_bytes(reference.as_bytes()).to_hex()
        )
    }
}
pub(crate) fn identity(
    entry: &Value,
    source: &Value,
    forms: Option<(&str, &Value)>,
) -> Result<Value> {
    let mut properties = source
        .as_object()
        .ok_or(Error::Invalid("bibliographic identity source"))?
        .clone();
    for (target, field) in [
        ("identity_ref", "record_id"),
        ("identity_kind", "record_type"),
        ("preferred_label", "preferred_label"),
        ("identity_status", "identity_status"),
    ] {
        properties.insert(target.into(), entry[field].clone());
    }
    if let Some((reference, value)) = forms {
        properties.insert("human_forms".into(), value.clone());
        properties.insert("human_forms_source_ref".into(), json!(reference));
    }
    if let Some(pointer) = entry.get("label_source_pointer") {
        properties.insert("label_source_pointer".into(), pointer.clone());
    }
    if entry["record_type"] == "artifact" {
        for (field, pointer) in [
            ("description", "/path_identity/note"),
            ("review_status", "/authority/review_status"),
            ("visibility", "/authority/visibility"),
        ] {
            properties.insert(
                field.into(),
                source
                    .pointer(pointer)
                    .cloned()
                    .ok_or(Error::Invalid("bibliographic artifact display"))?,
            );
        }
        properties.insert(
            "label_source_pointer".into(),
            json!("/custody/inventory_numbers/0"),
        );
        properties.insert("metadata_field_sources".into(),json!({"preferred_label":"/custody/inventory_numbers/0","description":"/path_identity/note","review_status":"/authority/review_status","visibility":"/authority/visibility"}));
    }
    if entry["source_schema_ref"] == "ToS/contracts/scholarly-composite-witness.schema.json" {
        for (field, pointer) in [
            ("description", "/editorial_object/description"),
            ("review_status", "/authority/review_status"),
            ("visibility", "/authority/visibility"),
        ] {
            properties.insert(
                field.into(),
                source
                    .pointer(pointer)
                    .cloned()
                    .ok_or(Error::Invalid("bibliographic composite display"))?,
            );
        }
        properties.insert("metadata_field_sources".into(),json!({"preferred_label":"/preferred_label","description":"/editorial_object/description","identity_status":"/identity_status","review_status":"/authority/review_status","visibility":"/authority/visibility"}));
    }
    properties.insert("source_record".into(), source.clone());
    Ok(
        json!({"node_id":node_id("identity",text(entry,"record_id")?),"node_kind":"identity","source_ref":entry["source_record_ref"],"source_sha256":entry["record_sha256"],"properties":properties}),
    )
}
pub(crate) fn literal(entry: &Value, claim: &Value, cap: usize) -> Result<Value> {
    let value = &claim["object"];
    let kind = match value {
        Value::Object(_) => "object",
        Value::Array(_) => "array",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        _ => "string",
    };
    Ok(
        json!({"node_id":format!("literal:sha256:{}",digest(&json!({"claim_ref":claim["claim_id"],"value":value}),cap)?),"node_kind":"literal",
        "source_ref":entry["source_claim_file_ref"],"source_line":entry["source_claim_line"],"source_sha256":entry["claim_sha256"],
        "properties":{"value":value,"value_sha256":digest(value,cap)?,"value_type":kind,"claim_ref":claim["claim_id"]}}),
    )
}
fn source_endpoint(node: &Value, reference: &Value, cap: usize) -> Result<Option<Value>> {
    let Some(source) = node
        .pointer("/properties/source_record")
        .filter(|v| v.is_object())
    else {
        return Ok(None);
    };
    let Some(id) = reference.as_str().filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let schema = source["schema_version"].as_str().unwrap_or("");
    if schema.trim().is_empty() {
        return Ok(None);
    }
    let (field, pointer) = match schema {
        "tos_artifact_source_witness_v1" | "tos_artifact_source_witness_v2" => {
            ("artifact_id", "/custody/inventory_numbers/0")
        }
        "tos_scholarly_composite_witness_v1" => ("composite_id", "/preferred_label"),
        _ => ("record_id", "/preferred_label"),
    };
    let label = source
        .pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or("");
    let source_ref = node["source_ref"].as_str().unwrap_or("");
    if node["node_kind"] != "identity"
        || node["node_id"] != node_id("identity", id)
        || node.pointer("/properties/identity_ref") != Some(reference)
        || source[field] != *reference
        || source["record_version"].as_u64().unwrap_or(0) == 0
        || source_ref.trim().is_empty()
        || label.trim().is_empty()
        || label == id
        || label == source_ref
        || node
            .pointer("/properties/preferred_label")
            .and_then(Value::as_str)
            != Some(label)
        || node["source_sha256"] != digest(source, cap)?
    {
        return Ok(None);
    }
    if let Some(p) = node.pointer("/properties/label_source_pointer") {
        if p != pointer {
            return Ok(None);
        }
    }
    if field == "artifact_id"
        && node.pointer("/properties/label_source_pointer") != Some(&json!(pointer))
    {
        return Ok(None);
    }
    Ok(Some(
        json!({"identity_ref":id,"node_id":node["node_id"],"source_ref":source_ref,"record_version":source["record_version"],"sha256":node["source_sha256"],"label_pointer":pointer,"label":label}),
    ))
}
fn endpoint_types(
    subject: &Value,
    object: &Value,
    relation: &Value,
    entities: &Value,
    temporal: bool,
) -> Result<bool> {
    let entries = array(entities, "types")?;
    let mut types = BTreeMap::new();
    for entry in entries {
        if types.insert(text(entry, "type_id")?, entry).is_some() {
            return Ok(false);
        }
    }
    for (node, field, override_kind) in [
        (subject, "domain_type_ids", None),
        (
            object,
            "range_type_ids",
            if temporal {
                Some("temporal-assertion")
            } else {
                None
            },
        ),
    ] {
        let Some(kind) = override_kind.or_else(|| {
            node.pointer("/properties/identity_kind")
                .and_then(Value::as_str)
        }) else {
            return Ok(false);
        };
        let mapped = entries
            .iter()
            .filter(|e| {
                e.get("source_mappings")
                    .and_then(Value::as_array)
                    .is_some_and(|m| {
                        m.iter().any(|m| {
                            m["source_graph"] == "source-claims" && m["source_kind_id"] == kind
                        })
                    })
            })
            .collect::<Vec<_>>();
        if mapped.len() != 1 || mapped[0]["abstract"] != false {
            return Ok(false);
        }
        let mut pending = vec![text(mapped[0], "type_id")?];
        let mut visited = BTreeSet::new();
        while let Some(id) = pending.pop() {
            if !visited.insert(id) {
                continue;
            }
            if visited.len() > 4096 {
                return Err(Error::Budget("bibliographic type ancestry"));
            }
            let Some(entry) = types.get(id) else {
                return Ok(false);
            };
            for parent in array(entry, "parent_type_ids")? {
                let Some(parent) = parent.as_str() else {
                    return Ok(false);
                };
                pending.push(parent);
            }
        }
        let allowed = array(relation, field)?;
        if allowed
            .iter()
            .any(|v| v.as_str().is_none_or(|id| !types.contains_key(id)))
            || !allowed
                .iter()
                .any(|v| v.as_str().is_some_and(|id| visited.contains(id)))
        {
            return Ok(false);
        }
    }
    Ok(true)
}
pub(crate) fn descriptor(
    claim: &Value,
    subject: &Value,
    object: &Value,
    registry: &Value,
    entities: &Value,
    limits: super::source_bibliographic::BibliographicDocumentLimits,
) -> Result<Option<Value>> {
    let cap = limits.input_document_bytes;
    let Some(template) = registry.get("claim_navigation_template") else {
        return Ok(None);
    };
    let mut d = json!({"schema_version":"tos_claim_navigation_descriptor_v1","purpose":"claim-navigation-only","standalone":false,"state":"unavailable","reason":null,
        "template":{"id":template["template_id"],"version":template["template_version"],"sha256":digest(template,cap)?},
        "claim":{"id":claim["claim_id"],"version":claim["claim_version"],"sha256":digest(claim,cap)?}});
    macro_rules! unavailable {
        ($reason:expr) => {{
            d["reason"] = json!($reason);
            return Ok(Some(d));
        }};
    }
    let mut candidates = Vec::new();
    for relation in array(registry, "relations")? {
        for mapping in array(relation, "source_mappings")? {
            if mapping["source_graph"] == "source-claims"
                && mapping["scope"] == "claim-predicate"
                && mapping["source_predicate_id"] == claim["predicate"]
            {
                candidates.push((relation, mapping))
            }
        }
    }
    if candidates.len() != 1
        || candidates[0].0["abstract"] != false
        || candidates[0].0["assertion_mode"] != "reified-claim"
    {
        unavailable!("predicate-not-understood")
    }
    let (relation, mapping) = candidates[0];
    let value = &claim["object"];
    let adapter = match relation
        .pointer("/source_claim_profile/reader")
        .and_then(Value::as_str)
    {
        Some("historical-temporal-v1") => {
            Some(("historical-time-source-wording-v1", "historical-time"))
        }
        Some("document-catalogue-temporal-v1") => Some((
            "document-catalogue-time-source-wording-v1",
            "catalogue-assigned-document-date",
        )),
        _ => None,
    };
    let temporal = adapter.is_some_and(|(adapter, role)| {
        template["object_label_adapters"]
            .as_array()
            .is_some_and(|v| v.contains(&json!(adapter)))
            && value["role"] == role
            && matches!(
                value["kind"].as_str(),
                Some("date-assertion" | "interval-assertion" | "relative-order" | "unknown-date")
            )
            && object["node_kind"] == "literal"
    });
    if !temporal && (!value.is_string() || object["node_kind"] != "identity") {
        unavailable!("object-not-identity")
    }
    if !endpoint_types(subject, object, relation, entities, temporal)? {
        unavailable!("endpoint-type-not-understood")
    }
    let subject = source_endpoint(subject, &claim["subject_ref"], cap)?;
    let target = if temporal {
        let wording = &value["source_wording"];
        let label = wording["text"].as_str().unwrap_or("");
        if label.trim().is_empty()
            || wording.get("language").is_none()
            || (!wording["language"].is_null() && !wording["language"].is_string())
        {
            None
        } else {
            Some(
                json!({"claim_ref":claim["claim_id"],"node_id":object["node_id"],"source_ref":object["source_ref"],"source_line":object["source_line"],"claim_version":claim["claim_version"],"sha256":digest(claim,cap)?,"value_sha256":digest(value,cap)?,"label_pointer":"/object/source_wording/text","label":label,"language":wording["language"]}),
            )
        }
    } else {
        source_endpoint(object, value, cap)?
    };
    let (Some(subject), Some(target)) = (subject, target) else {
        unavailable!("source-name-unavailable")
    };
    let mut statuses = Map::new();
    for field in ["epistemic_status", "review_status"] {
        let Some(status) = claim[field].as_str() else {
            unavailable!("source-status-unavailable")
        };
        if template["status_labels"][field].get(status).is_none() {
            unavailable!("source-status-unavailable")
        }
        statuses.insert(field.into(), json!({"present":true,"value":status}));
    }
    let labels = mapping.get("labels").or_else(|| {
        let count = relation["source_mappings"]
            .as_array()
            .map(|m| {
                m.iter()
                    .filter(|m| {
                        m["source_graph"] == "source-claims" && m["scope"] == "claim-predicate"
                    })
                    .count()
            })
            .unwrap_or(0);
        if count == 1 {
            relation.get("labels")
        } else {
            None
        }
    });
    let Some(labels) = labels.filter(|v| v.is_object()) else {
        unavailable!("predicate-label-unavailable")
    };
    let renderings = template["renderings"]
        .as_object()
        .ok_or(Error::Invalid("bibliographic descriptor renderings"))?;
    let mut title = Map::new();
    for (language, parts) in renderings {
        if labels[language]
            .as_str()
            .is_none_or(|s| s.trim().is_empty())
        {
            unavailable!("predicate-label-unavailable")
        }
        let mut rendered = String::new();
        for part in parts
            .as_array()
            .ok_or(Error::Invalid("bibliographic descriptor parts"))?
        {
            let piece = if let Some(literal) = part.get("literal") {
                literal
            } else {
                match text(part, "slot")? {
                    "claim-marker" => &template["marker"][language],
                    "subject-label" => &subject["label"],
                    "predicate-label" => &labels[language],
                    "object-label" => &target["label"],
                    "declared-epistemic-status" => {
                        &template["status_labels"]["epistemic_status"]
                            [text(claim, "epistemic_status")?][language]
                    }
                    "declared-review-status" => {
                        &template["status_labels"]["review_status"][text(claim, "review_status")?]
                            [language]
                    }
                    _ => return Err(Error::Invalid("bibliographic descriptor slot")),
                }
            };
            rendered.push_str(
                piece
                    .as_str()
                    .ok_or(Error::Invalid("bibliographic descriptor language"))?,
            );
            if rendered.len() > limits.output_row_bytes {
                unavailable!("over-budget")
            }
        }
        title.insert(language.clone(), json!(rendered));
    }
    let default = text(template, "default_language")?;
    title.insert(
        "default".into(),
        title
            .get(default)
            .cloned()
            .ok_or(Error::Invalid("bibliographic descriptor default"))?,
    );
    if encode(&json!(title), limits.output_row_bytes)?.len() as u64
        > template["max_output_bytes"]
            .as_u64()
            .ok_or(Error::Invalid("bibliographic descriptor cap"))?
    {
        unavailable!("over-budget")
    }
    d["state"] = json!("ready");
    d["predicate"] = json!({"id":claim["predicate"],"relation_type_id":relation["relation_type_id"],"sha256":digest(relation,cap)?,"mapping_sha256":digest(mapping,cap)?});
    d["subject"] = subject;
    d["object"] = target;
    d["statuses"] = json!(statuses);
    d["title"] = json!(title);
    Ok(Some(d))
}

pub struct Cohort {
    pub nodes: Vec<Value>,
    pub edges: Vec<Value>,
    pub trace: Value,
}
pub struct ClaimInputs<'a> {
    pub entry: &'a Value,
    pub claim: &'a Value,
    pub subject: Value,
    pub object: Value,
    pub event: Value,
    pub maker: Value,
    pub maker_identity: Option<Value>,
    pub evidence: Vec<Value>,
    pub counterevidence: Vec<Value>,
    pub members: Vec<Value>,
    pub normalized: Vec<(String, Value)>,
    pub descriptor: Option<Value>,
    pub forms: Option<(String, Value)>,
    pub collection_order_basis: Option<Value>,
    pub legacy_context: Option<Value>,
}
pub(crate) fn project(inputs: ClaimInputs<'_>, cap: usize) -> Result<Cohort> {
    let entry = inputs.entry;
    let claim = inputs.claim;
    let id = text(claim, "claim_id")?;
    let mut nodes = BTreeMap::new();
    let mut add = |node: Value| -> Result<()> {
        let id = text(&node, "node_id")?.to_owned();
        if nodes.get(&id).is_some_and(|old| old != &node) {
            return Err(Error::Invalid("bibliographic conflicting node"));
        }
        nodes.insert(id, node);
        Ok(())
    };
    let mut properties = claim
        .as_object()
        .ok_or(Error::Invalid("bibliographic Claim object"))?
        .clone();
    for (field, source) in [
        ("claim_ref", "claim_id"),
        ("claim_type", "claim_type"),
        ("assertion_layer", "assertion_layer"),
        ("predicate", "predicate"),
        ("epistemic_status", "epistemic_status"),
        ("review_status", "review_status"),
        ("visibility", "visibility"),
        ("claim_version", "claim_version"),
    ] {
        properties.insert(field.into(), entry[source].clone());
    }
    for field in ["confidence", "qualifiers"] {
        properties.insert(field.into(), claim[field].clone());
    }
    properties.insert("source_claim".into(), claim.clone());
    if let Some(basis) = inputs.collection_order_basis {
        properties.insert("collection_order_basis".into(), basis);
    }
    if let Some(context) = inputs.legacy_context {
        for (key, value) in context
            .as_object()
            .ok_or(Error::Invalid("bibliographic legacy context"))?
        {
            properties.insert(key.clone(), value.clone());
        }
    }
    if let Some(d) = inputs.descriptor {
        properties.insert("navigation_descriptor".into(), d);
    }
    if let Some((reference, forms)) = inputs.forms {
        properties.insert("human_forms".into(), forms);
        properties.insert("human_forms_source_ref".into(), json!(reference));
    }
    let claim_node_id = node_id("claim", id);
    add(
        json!({"node_id":claim_node_id,"node_kind":"claim","source_ref":entry["source_claim_file_ref"],"source_line":entry["source_claim_line"],"source_sha256":entry["claim_sha256"],"properties":properties}),
    )?;
    add(inputs.subject.clone())?;
    add(inputs.object.clone())?;
    add(inputs.event.clone())?;
    add(inputs.maker.clone())?;
    if let Some(node) = inputs.maker_identity {
        add(node)?;
    }
    let mut evidence_ids = BTreeSet::new();
    let mut counter_ids = BTreeSet::new();
    for (field, resolved, ids) in [
        ("evidence_refs", &inputs.evidence, &mut evidence_ids),
        (
            "counterevidence_refs",
            &inputs.counterevidence,
            &mut counter_ids,
        ),
    ] {
        let refs = claim
            .get(field)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if refs.len() != resolved.len() {
            return Err(Error::Invalid("bibliographic ordered evidence closure"));
        }
        for (reference, node) in refs.iter().zip(resolved) {
            if node.pointer("/properties/evidence_ref") != Some(reference) {
                return Err(Error::Invalid("bibliographic evidence source order"));
            }
            ids.insert(text(node, "node_id")?.to_owned());
            add(node.clone())?;
        }
    }
    if evidence_ids.is_empty() {
        return Err(Error::Invalid("bibliographic Claim requires evidence"));
    }
    let mut review_ids = Vec::new();
    for review in claim
        .get("reviews")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let review_id = node_id("review", text(review, "review_id")?);
        review_ids.push(review_id.clone());
        let mut p = review
            .as_object()
            .ok_or(Error::Invalid("bibliographic review object"))?
            .clone();
        p.insert("review_ref".into(), review["review_id"].clone());
        p.insert("source_review".into(), review.clone());
        add(
            json!({"node_id":review_id,"node_kind":"review","source_ref":entry["source_claim_file_ref"],"source_line":entry["source_claim_line"],"source_sha256":entry["claim_sha256"],"properties":p}),
        )?;
    }
    review_ids.sort();
    let maker_id = text(&inputs.maker, "node_id")?;
    let event_id = text(&inputs.event, "node_id")?;
    let mut specs = vec![
        (
            "has_subject".to_owned(),
            text(&inputs.subject, "node_id")?.to_owned(),
        ),
        ("has_object".into(), text(&inputs.object, "node_id")?.into()),
        ("made_by".into(), maker_id.into()),
        ("generated_by".into(), event_id.into()),
    ];
    specs.extend(
        evidence_ids
            .iter()
            .cloned()
            .map(|id| ("supported_by".into(), id)),
    );
    specs.extend(
        counter_ids
            .iter()
            .cloned()
            .map(|id| ("counterevidenced_by".into(), id)),
    );
    specs.extend(
        review_ids
            .iter()
            .cloned()
            .map(|id| ("reviewed_by".into(), id)),
    );
    let mut member_ids = Vec::new();
    let mut normalized_ids = Vec::new();
    for node in inputs.members {
        let id = text(&node, "node_id")?.to_owned();
        member_ids.push(id.clone());
        specs.push(("has_value_member".into(), id));
        add(node)?;
    }
    for (kind, node) in inputs.normalized {
        let id = text(&node, "node_id")?.to_owned();
        normalized_ids.push(id.clone());
        specs.push((kind, id));
        add(node)?;
    }
    let mut alternatives = claim
        .get("alternative_claim_refs")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    alternatives.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
    for reference in &alternatives {
        specs.push((
            "alternative_to".into(),
            node_id(
                "claim",
                reference
                    .as_str()
                    .ok_or(Error::Invalid("bibliographic alternative reference"))?,
            ),
        ));
    }
    if let Some(reference) = claim.get("supersedes_claim_ref").filter(|v| !v.is_null()) {
        specs.push((
            "supersedes".into(),
            node_id(
                "claim",
                reference
                    .as_str()
                    .ok_or(Error::Invalid("bibliographic supersedes reference"))?,
            ),
        ));
    }
    let edges=specs.into_iter().enumerate().map(|(index,(kind,target))|json!({"edge_id":format!("edge:{id}:{kind}:{:03}",index+1),"edge_kind":kind,"from_id":claim_node_id,"to_id":target,"claim_ref":id,"claim_sha256":entry["claim_sha256"],"evidence_node_ids":evidence_ids,"maker_node_id":maker_id,"provenance_event_node_id":event_id,"review_status":entry["review_status"],"source_claim_file_ref":entry["source_claim_file_ref"],"source_claim_line":entry["source_claim_line"]})).collect::<Vec<_>>();
    member_ids.sort();
    normalized_ids.sort();
    let mut edge_ids = edges
        .iter()
        .map(|e| e["edge_id"].clone())
        .collect::<Vec<_>>();
    edge_ids.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
    let mut counter_refs = claim
        .get("counterevidence_refs")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    counter_refs.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
    let mut trace = json!({"claim_ref":id,"claim_sha256":entry["claim_sha256"],"claim_node_id":claim_node_id,"subject_node_id":inputs.subject["node_id"],"object_node_id":inputs.object["node_id"],"predicate":claim["predicate"],"assertion_layer":claim["assertion_layer"],"epistemic_status":claim["epistemic_status"],"confidence":claim["confidence"],"qualifiers":claim["qualifiers"],"maker_node_id":maker_id,"provenance_event_node_id":event_id,"evidence_node_ids":evidence_ids,"counterevidence_node_ids":counter_ids,"review_node_ids":review_ids,"normalized_identity_node_ids":normalized_ids,"review_status":claim["review_status"],"visibility":claim["visibility"],"alternative_claim_refs":alternatives,"counterevidence_refs":counter_refs,"supersedes_claim_ref":claim["supersedes_claim_ref"],"source_claim_file_ref":entry["source_claim_file_ref"],"source_claim_line":entry["source_claim_line"],"source_claim_sha256":entry["claim_sha256"],"edge_ids":edge_ids});
    if !member_ids.is_empty() {
        trace["value_member_node_ids"] = json!(member_ids);
    }
    encode(&trace, cap)?;
    Ok(Cohort {
        nodes: nodes.into_values().collect(),
        edges,
        trace,
    })
}

#[cfg(test)]
mod oracle {
    use super::*;
    #[test]
    fn retained_python_raw_claim_cohort_preserves_fields_and_edge_ordinals() {
        // Maintained project_bibliographic_claim independently reproduced this
        // existing public transport cut (6 nodes,5 edges,1 trace). The separately
        // derived navigation descriptor is outside this cohort renderer test.
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../access/tests/fixtures/knowledge-contract/temporal-jenseits-date.json"
        ))
        .unwrap();
        let nodes = fixture["nodes"].as_array().unwrap();
        let trace = &fixture["claim_traces"][0];
        let node = |id: &Value| nodes.iter().find(|n| n["node_id"] == *id).unwrap().clone();
        let claim_node = nodes.iter().find(|n| n["node_kind"] == "claim").unwrap();
        let claim = &claim_node["properties"]["source_claim"];
        let mut entry = json!({"source_claim_file_ref":claim_node["source_ref"],"source_claim_line":claim_node["source_line"],"claim_sha256":claim_node["source_sha256"]});
        for field in [
            "claim_id",
            "claim_type",
            "assertion_layer",
            "subject_ref",
            "predicate",
            "object",
            "evidence_refs",
            "maker",
            "provenance_event_ref",
            "epistemic_status",
            "review_status",
            "visibility",
            "claim_version",
        ] {
            entry[field] = claim[field].clone();
        }
        let result = project(
            ClaimInputs {
                entry: &entry,
                claim,
                subject: node(&trace["subject_node_id"]),
                object: node(&trace["object_node_id"]),
                event: node(&trace["provenance_event_node_id"]),
                maker: node(&trace["maker_node_id"]),
                maker_identity: None,
                evidence: trace["evidence_node_ids"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(node)
                    .collect(),
                counterevidence: Vec::new(),
                members: Vec::new(),
                normalized: Vec::new(),
                descriptor: None,
                forms: Some((
                    claim_node["properties"]["human_forms_source_ref"]
                        .as_str()
                        .unwrap()
                        .into(),
                    claim_node["properties"]["human_forms"].clone(),
                )),
                collection_order_basis: None,
                legacy_context: None,
            },
            1024 * 1024,
        )
        .unwrap();
        let mut expected = nodes.clone();
        expected
            .iter_mut()
            .find(|n| n["node_kind"] == "claim")
            .unwrap()["properties"]
            .as_object_mut()
            .unwrap()
            .remove("navigation_descriptor");
        expected.sort_by(|a, b| a["node_id"].as_str().cmp(&b["node_id"].as_str()));
        assert_eq!(result.nodes, expected);
        let mut edges = result.edges;
        edges.sort_by(|a, b| a["edge_id"].as_str().cmp(&b["edge_id"].as_str()));
        assert_eq!(json!(edges), fixture["edges"]);
        assert_eq!(result.trace, *trace);
    }
}
