//! Exact stand-off metadata projection. Original payload text is never read;
//! source declarations remain weaker than assessment/current rights authority.
use crate::knowledge_stage::KnowledgeStage;
use crate::source_witness_catalog::{SourceCatalogLimits, SourceCatalogValidator};
use crate::{Error, Result};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Instant;
use tos_foundation::Digest256;

pub(crate) struct PacketProjection {
    pub nodes: Vec<Value>,
    pub edges: Vec<Value>,
}
struct Projector<'a> {
    source: &'a str,
    packet: &'a Value,
    packet_id: &'a str,
    version: &'a Value,
    namespace: String,
    content: bool,
    identities: BTreeMap<String, String>,
    output: PacketProjection,
    bytes: usize,
    cap: usize,
    max_rows: usize,
    deadline: Instant,
    cancelled: &'a AtomicBool,
}
fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .ok_or(Error::Invalid("navigation packet source field"))
}
fn array<'a>(v: &'a Value, key: &str) -> Result<&'a [Value]> {
    match v.get(key) {
        None => Ok(&[]),
        Some(Value::Array(a)) => Ok(a),
        _ => Err(Error::Invalid("navigation packet source array")),
    }
}
fn optional_string(v: Option<&Value>) -> Result<String> {
    match v {
        None | Some(Value::Null) => Ok("None".into()),
        Some(Value::String(s)) => Ok(s.clone()),
        Some(Value::Number(n)) => Ok(n.to_string()),
        _ => Err(Error::Invalid("navigation packet scalar representation")),
    }
}
fn selected(v: &Value, keys: &[&str]) -> Value {
    let mut object = serde_json::Map::new();
    for key in keys {
        if let Some(value) = v.get(key) {
            object.insert((*key).into(), value.clone());
        }
    }
    Value::Object(object)
}
fn metadata_visible(rights: &Value) -> bool {
    matches!(
        rights
            .get("packet_visibility")
            .or_else(|| rights.get("record_visibility"))
            .and_then(Value::as_str),
        Some("public" | "public_metadata_only")
    )
}
fn content_available(rights: &Value) -> bool {
    let visibility = rights
        .get("packet_visibility")
        .or_else(|| rights.get("record_visibility"))
        .and_then(Value::as_str);
    visibility == Some("public")
        && rights.get("publication_authorized") == Some(&Value::Bool(true))
        && !rights
            .get("private_source_used")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        && matches!(
            rights
                .get("effective_visibility")
                .or_else(|| rights.get("source_content_visibility"))
                .and_then(Value::as_str),
            Some("public" | "public_synthetic")
        )
}
fn visible_record(record: &Value, content: bool, fields: &[&str]) -> Value {
    if content {
        record.clone()
    } else {
        selected(record, fields)
    }
}
impl Projector<'_> {
    fn charge(&mut self, v: &Value) -> Result<()> {
        if self.cancelled.load(Ordering::Relaxed) {
            return Err(Error::Invalid("navigation packet cancelled"));
        }
        if Instant::now() >= self.deadline {
            return Err(Error::Budget("navigation packet deadline"));
        }
        let size = serde_json::to_vec(v)
            .map_err(|_| Error::Invalid("navigation packet projection JSON"))?
            .len();
        self.bytes = self
            .bytes
            .checked_add(size)
            .filter(|n| *n <= self.cap)
            .ok_or(Error::Budget("navigation packet projection bytes"))?;
        if self.output.nodes.len() + self.output.edges.len() >= self.max_rows {
            return Err(Error::Budget("navigation packet projection rows"));
        }
        Ok(())
    }
    fn add(
        &mut self,
        identity: &str,
        kind: &str,
        record: Value,
        label: Option<String>,
    ) -> Result<String> {
        let identifier = format!("{identity}@{}", self.namespace);
        let mut properties = record
            .as_object()
            .cloned()
            .ok_or(Error::Invalid("navigation packet source record"))?;
        properties.insert("record_id".into(), json!(identity));
        properties.insert("packet_id".into(), json!(self.packet_id));
        properties.insert("packet_version".into(), self.version.clone());
        properties.insert("content_available".into(), json!(self.content));
        properties.insert(
            "publication_posture".into(),
            json!(if self.content {
                "public"
            } else {
                "public_metadata_only"
            }),
        );
        properties.insert(
            "content_posture".into(),
            self.packet
                .get("content_posture")
                .cloned()
                .unwrap_or(Value::Null),
        );
        properties.insert(
            "review_status".into(),
            record
                .get("admission_status")
                .or_else(|| record.get("boundary_posture"))
                .cloned()
                .unwrap_or(json!("not-recorded")),
        );
        let node = json!({"node_id":identifier,"node_kind":kind,"label":label.filter(|s|!s.is_empty()).unwrap_or_else(||identity.into()),
            "source_ref":self.source,"identity_status":"source-declared-versioned-record","properties":properties});
        self.charge(&node)?;
        self.identities.insert(identity.into(), identifier.clone());
        self.output.nodes.push(node);
        Ok(identifier)
    }
    fn edge(&mut self, left: &str, predicate: &str, right: &str) -> Result<()> {
        let key = Digest256::of_bytes(
            format!("{}:{left}:{predicate}:{right}", self.namespace).as_bytes(),
        )
        .to_hex();
        let edge = json!({"edge_id":format!("text-spine:{key}"),"from_id":left,"to_id":right,"predicate_id":predicate,
            "edge_kind":"stand-off-source-structure","source_refs":[self.source],"review_status":"source-declared-not-semantic-acceptance"});
        self.charge(&edge)?;
        self.output.edges.push(edge);
        Ok(())
    }
    fn resolved(&self, identity: &str) -> Result<String> {
        self.identities
            .get(identity)
            .cloned()
            .ok_or(Error::Invalid("navigation unresolved stand-off identity"))
    }
    fn anchors(&mut self, node: &str, record: &Value, key: &str) -> Result<()> {
        for anchor in array(record, key)? {
            let id = self.resolved(
                anchor
                    .as_str()
                    .ok_or(Error::Invalid("navigation packet anchor reference"))?,
            )?;
            self.edge(node, "has_anchor", &id)?;
        }
        Ok(())
    }
}
/// Caller has already read the original packet from its selected current cut.
/// Validation uses that same cut's exact contracts, never an ambient checkout.
#[allow(clippy::too_many_arguments)]
pub(crate) fn project_packet(
    stage: &mut KnowledgeStage<'_>,
    raw: &[u8],
    source: &str,
    validator: &SourceCatalogValidator<'_>,
    catalog_limits: SourceCatalogLimits,
    max_raw: usize,
    max_output: usize,
    max_rows: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<PacketProjection> {
    if max_raw == 0
        || max_raw > 8 * 1024 * 1024
        || max_output == 0
        || max_output > 64 * 1024 * 1024
        || max_rows == 0
        || max_rows > 65_536
    {
        return Err(Error::Budget("navigation packet bounds"));
    }
    let parsed = crate::knowledge_normalization::SourceRow::parse(raw, max_raw)?;
    let packet = parsed.value();
    let schema = packet
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or("");
    let empty = || PacketProjection {
        nodes: Vec::new(),
        edges: Vec::new(),
    };
    if !matches!(
        schema,
        "tos_source_text_unit_packet_v1" | "tos_semantic_annotation_packet_v2"
    ) {
        return Ok(empty());
    }
    let null = Value::Null;
    let rights = packet.get("rights_and_visibility").unwrap_or(&null);
    if !metadata_visible(rights) {
        return Ok(empty());
    }
    let content = content_available(rights);
    if schema == "tos_semantic_annotation_packet_v2" && !content {
        return Ok(empty());
    }
    let contract = if schema == "tos_source_text_unit_packet_v1" {
        "ToS/contracts/source-text-unit-packet-v1.schema.json"
    } else {
        "ToS/contracts/semantic-annotation-packet-v2.schema.json"
    };
    crate::source_witness_catalog::check_catalog_schema(
        stage,
        validator,
        catalog_limits,
        contract,
        "",
        raw,
    )?;
    if schema == "tos_source_text_unit_packet_v1" {
        let report = tos_validation::text_metadata_rules::inspect_source_text_unit_v1_metadata(
            raw,
            source,
            tos_validation::text_metadata_rules::TextMetadataLimits {
                max_packet_bytes: max_raw,
                max_state_bytes: max_output,
                max_issues: 1024,
                deadline,
            },
            cancelled,
        )
        .map_err(|e| Error::Source(format!("navigation source-unit metadata:{e:?}")))?;
        if report.state != tos_validation::text_metadata_rules::TextMetadataState::CheckedMetadata
            || !report.issues.is_empty()
        {
            return Err(Error::Invalid(
                "navigation source-unit owner metadata predicate",
            ));
        }
    }
    let scope = packet
        .get("source_scope")
        .ok_or(Error::Invalid("navigation packet scope"))?;
    let packet_id = packet
        .get("packet_id")
        .or_else(|| packet.get("annotation_id"))
        .and_then(Value::as_str)
        .ok_or(Error::Invalid("navigation packet identity"))?;
    let version = packet
        .get("packet_version")
        .or_else(|| packet.get("annotation_version"))
        .ok_or(Error::Invalid("navigation packet version"))?;
    let namespace = Digest256::of_bytes(
        format!("{source}:{packet_id}:{}", optional_string(Some(version))?).as_bytes(),
    )
    .to_hex()[..20]
        .to_owned();
    let mut p = Projector {
        source,
        packet,
        packet_id,
        version,
        namespace,
        content,
        identities: BTreeMap::new(),
        output: empty(),
        bytes: 0,
        cap: max_output,
        max_rows,
        deadline,
        cancelled,
    };
    let layer = packet.get("source_layer").cloned().unwrap_or(json!({}));
    let layer_ref = layer
        .get("text_layer_ref")
        .or_else(|| scope.get("source_text_layer_ref"));
    let identity = format!(
        "text-layer:{}",
        Digest256::of_bytes(
            format!(
                "{}:{}",
                optional_string(layer_ref)?,
                optional_string(layer.get("text_layer_sha256"))?
            )
            .as_bytes()
        )
        .to_hex()
    );
    let layer_record = visible_record(
        &layer,
        content,
        &[
            "text_layer_ref",
            "language",
            "immutable",
            "position_unit",
            "interval",
            "visibility",
        ],
    );
    let layer_id = p.add(
        &identity,
        "text-layer",
        layer_record,
        Some(format!(
            "Text layer · {} · {}",
            layer
                .get("language")
                .and_then(Value::as_str)
                .unwrap_or("source"),
            source.rsplit('/').next().unwrap_or(source)
        )),
    )?;
    p.edge(text(scope, "work_ref")?, "has_text_layer", &layer_id)?;
    let annotation = p.add(
        packet_id,
        "annotation",
        json!({"rights_and_visibility":rights,"source_scope":scope}),
        Some(format!(
            "Stand-off packet · version {}",
            optional_string(Some(version))?
        )),
    )?;
    p.edge(&layer_id, "has_annotation", &annotation)?;
    let anchors = packet
        .get("anchors")
        .or_else(|| scope.get("source_anchors"))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    for anchor in anchors {
        let selector = &anchor["selector"];
        if selector["start"]
            .as_u64()
            .zip(selector["end"].as_u64())
            .is_none_or(|(a, b)| a > b)
        {
            return Err(Error::Invalid(
                "navigation reversed/unsupported anchor selector",
            ));
        }
        let record = visible_record(
            anchor,
            content,
            &[
                "anchor_ref",
                "ordinal",
                "selector",
                "anchor_role",
                "text_layer_ref",
            ],
        );
        let id = p.add(
            text(anchor, "anchor_ref")?,
            "anchor",
            record,
            Some(format!(
                "Anchor · {}",
                optional_string(anchor.get("ordinal").or_else(|| anchor.get("anchor_ref")))?
            )),
        )?;
        p.edge(&id, "anchored_in", &layer_id)?;
    }
    for unit in array(packet, "units")? {
        let record = visible_record(
            unit,
            content,
            &[
                "unit_id",
                "unit_version",
                "unit_kind",
                "surface_posture",
                "continuity",
                "ordered_anchor_refs",
                "parent_unit_refs",
                "ordered_child_unit_refs",
                "boundary_posture",
                "semantic_promotion",
            ],
        );
        let id = p.add(
            text(unit, "unit_id")?,
            "text-unit",
            record,
            Some(format!(
                "{} · {}",
                text(unit, "unit_kind")?,
                text(unit, "unit_id")?
            )),
        )?;
        p.edge(&layer_id, "has_text_unit", &id)?;
        p.anchors(&id, unit, "ordered_anchor_refs")?;
    }
    for entity in array(packet, "entities")? {
        let labels = array(entity, "display_labels")?;
        let mut record = entity.clone();
        record["variant_labels"] = json!(labels);
        let original = text(entity, "entity_kind")?;
        let kind = original.replace('_', "-");
        let kind = if matches!(original, "occurrence" | "lexeme" | "lexical_sense" | "sign") {
            format!("annotation-{kind}")
        } else {
            kind
        };
        let label = labels
            .first()
            .map(|v| text(v, "value").map(str::to_owned))
            .transpose()?;
        let id = p.add(text(entity, "entity_id")?, &kind, record, label)?;
        p.edge(&annotation, "annotation_member", &id)?;
        p.anchors(&id, &entity["identity_basis"], "anchor_refs")?;
    }
    for claim in array(packet, "claims")? {
        let claim_id = text(claim, "claim_id")?;
        let proposition = &claim["proposition"];
        let id = p.add(
            claim_id,
            "annotation-claim",
            claim.clone(),
            Some(text(proposition, "predicate")?.into()),
        )?;
        p.edge(&annotation, "annotation_member", &id)?;
        let subject = p.resolved(text(proposition, "subject_ref")?)?;
        p.edge(&id, "assertion_subject", &subject)?;
        let object = &proposition["object"];
        let object_id = if object.get("kind").and_then(Value::as_str) == Some("entity_ref") {
            p.resolved(text(object, "entity_ref")?)?
        } else {
            p.add(
                &format!("{claim_id}:object"),
                "literal",
                object.clone(),
                Some(format!("Claim object · {}", text(object, "kind")?)),
            )?
        };
        p.edge(&id, "assertion_object", &object_id)?;
        p.anchors(&id, claim, "target_anchor_refs")?;
        for (index, evidence) in array(claim, "evidence")?.iter().enumerate() {
            let evidence_id = p.add(
                &format!("{claim_id}:evidence:{index}"),
                "annotation-evidence",
                evidence.clone(),
                Some(text(evidence, "description")?.into()),
            )?;
            p.edge(&id, "assertion_evidence", &evidence_id)?;
            p.anchors(&evidence_id, evidence, "anchor_refs")?;
        }
    }
    for review in array(packet, "reviews")? {
        let id = p.add(
            text(review, "review_id")?,
            "annotation-review",
            review.clone(),
            Some(format!(
                "Review · {}",
                optional_string(review.get("decision").or_else(|| review.get("outcome")))?
            )),
        )?;
        p.edge(&annotation, "annotation_member", &id)?;
        for claim in array(packet, "claims")? {
            if array(claim, "review_refs")?
                .iter()
                .any(|r| r.as_str() == review["review_id"].as_str())
            {
                let claim_id = p.resolved(text(claim, "claim_id")?)?;
                p.edge(&claim_id, "assertion_review", &id)?;
            }
        }
    }
    for relation in array(packet, "relations")? {
        let claim = p.resolved(text(relation, "claim_ref")?)?;
        let id = p.add(
            text(relation, "relation_id")?,
            "annotation-relation",
            relation.clone(),
            Some(text(relation, "relation_type")?.into()),
        )?;
        let subject = p.resolved(text(relation, "subject_ref")?)?;
        let object = p.resolved(text(relation, "object_ref")?)?;
        p.edge(&id, "assertion_subject", &subject)?;
        p.edge(&id, "assertion_object", &object)?;
        p.edge(&id, "asserted_by", &claim)?;
    }
    Ok(p.output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_only_text_projection_withholds_words_and_hashes() {
        let metadata_rights = json!({
            "packet_visibility":"public_metadata_only",
            "publication_authorized":true,
            "private_source_used":false,
            "effective_visibility":"public"
        });
        assert!(metadata_visible(&metadata_rights));
        assert!(!content_available(&metadata_rights));

        let legacy_public_rights = json!({
            "record_visibility":"public",
            "publication_authorized":true,
            "private_source_used":false,
            "effective_visibility":"public_synthetic"
        });
        assert!(content_available(&legacy_public_rights));

        let private_rights = json!({
            "packet_visibility":"public",
            "publication_authorized":true,
            "private_source_used":true,
            "effective_visibility":"public"
        });
        assert!(metadata_visible(&private_rights));
        assert!(!content_available(&private_rights));
        assert!(!metadata_visible(
            &json!({"packet_visibility":"restricted"})
        ));

        let unit = json!({
            "unit_id":"unit-1",
            "unit_version":1,
            "unit_kind":"paragraph",
            "ordered_anchor_refs":["anchor-1"],
            "semantic_promotion":false,
            "exact_text":"private witness wording",
            "exact_sha256":"word-digest",
        });
        let metadata = visible_record(
            &unit,
            false,
            &[
                "unit_id",
                "unit_version",
                "unit_kind",
                "surface_posture",
                "continuity",
                "ordered_anchor_refs",
                "parent_unit_refs",
                "ordered_child_unit_refs",
                "boundary_posture",
                "semantic_promotion",
            ],
        );
        assert_eq!(metadata["unit_id"], "unit-1");
        assert!(!metadata.as_object().unwrap().contains_key("exact_text"));
        assert!(!metadata.as_object().unwrap().contains_key("exact_sha256"));
        assert_eq!(visible_record(&unit, true, &[]), unit);
    }
}
