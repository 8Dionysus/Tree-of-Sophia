//! The maintained per-record SourceNavigation renderer and its catalog/cut
//! composition. External incidence and global conflict ordering stay with the
//! navigation producer. These rows confer no assessment or current-use grant.
use crate::knowledge_stage::KnowledgeStage;
use crate::source_bibliographic::{self as graph, BibliographicForms, BibliographicLimits};
use crate::source_bibliographic_render::{array, digest, encode, text};
use crate::source_bibliographic_versions::{Version, Versions};
use crate::source_witness_catalog::{
    self as catalog, SourceCatalogReceipt, SourceCatalogValidator,
};
use crate::{Error, Result};
use serde_json::{Value, json};
use std::collections::BTreeSet;

/// Borrowed inputs are not a source receipt. Pure rendering retains the exact
/// ordered history resolution set supplied by the checked caller.
pub(crate) struct NavigationRecordInput<'a> {
    pub entry: &'a Value,
    pub source_record: &'a Value,
    pub forms: Option<(&'a str, &'a Value)>,
    pub history: Option<&'a Value>,
    pub versions: &'a [(Value, Value)],
    pub native_composite: bool,
}
pub(crate) struct NavigationRecordProjection {
    pub nodes: Vec<Value>,
    pub edges: Vec<Value>,
    pub diagnostics: Vec<Value>,
}
impl NavigationRecordProjection {
    fn check(&self, l: BibliographicLimits) -> Result<()> {
        let count = self
            .nodes
            .len()
            .checked_add(self.edges.len())
            .and_then(|n| n.checked_add(self.diagnostics.len()))
            .ok_or(Error::Budget("navigation record cohort count"))?;
        if count > l.max_claim_cohort_rows {
            return Err(Error::Budget("navigation record cohort rows"));
        }
        let mut total = 0usize;
        for row in self
            .nodes
            .iter()
            .chain(&self.edges)
            .chain(&self.diagnostics)
        {
            total = total
                .checked_add(encode(row, l.catalog.max_output_row_bytes)?.len())
                .filter(|n| *n <= l.max_claim_cohort_bytes)
                .ok_or(Error::Budget("navigation record cohort bytes"))?;
        }
        Ok(())
    }
}
/// Pure rendering only: the caller owns the exact resolver envelope and its
/// status. Unsupported source profiles must not become invented unavailable rows.
pub(crate) fn project_record_version(
    reference: &Value,
    resolved: &Value,
    kind: &str,
    l: BibliographicLimits,
) -> Result<Value> {
    if !["claim", "metadata"].contains(&kind) {
        return Err(Error::Invalid("navigation record version kind"));
    }
    let available = resolved["status"] == "available";
    let status = text(resolved, "status")?;
    let reason = text(resolved, "reason")?;
    let view = json!({"schema_version":"tos_record_version_view_v1","record_ref":reference,"record_kind":kind,
        "status":status,"reason":reason,"version_status":resolved["version_status"],"record":resolved["record"],
        "provenance":if available {resolved["provenance"].clone()}else{json!({})},"grants_current_use":false,"performs_assessment":false});
    let id = format!(
        "record-version:{}",
        digest(reference, l.catalog.max_row_bytes)?
    );
    let mut source =
        "ToS/contracts/record-version-view.schema.json#/properties/record_ref".to_owned();
    if available {
        let locator = &resolved["provenance"]["source"];
        source = locator["archive_blob_ref"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or(text(locator, "source_ref")?)
            .into();
        if kind == "claim" {
            let line = locator["line"]
                .as_u64()
                .filter(|n| *n > 0)
                .ok_or(Error::Invalid("navigation exact Claim version source line"))?;
            source.push_str(&format!("#L{line}"));
        }
    }
    let node = json!({"node_id":id,"node_kind":"record-version","label":"Exact record version","source_ref":source,
        "identity_status":"not_applicable","properties":{"record_version_view":view}});
    encode(&node, l.catalog.max_output_row_bytes)?;
    Ok(node)
}
fn edge(id: String, from: &str, predicate: &str, node: &Value, refs: &[&str]) -> Result<Value> {
    let refs = refs
        .iter()
        .map(|s| (*s).to_owned())
        .collect::<BTreeSet<_>>();
    Ok(
        json!({"edge_id":id,"from_id":from,"predicate_id":predicate,"to_id":text(node,"node_id")?,
        "edge_kind":"exact_historical_record_reference","review_status":"not_applicable","source_refs":refs}),
    )
}
pub(crate) fn project_source_navigation_record(
    input: NavigationRecordInput<'_>,
    l: BibliographicLimits,
) -> Result<NavigationRecordProjection> {
    l.validate()?;
    let entry = input.entry;
    let record = input.source_record;
    encode(entry, l.catalog.max_output_row_bytes)?;
    encode(record, l.catalog.max_row_bytes)?;
    if let Some(history) = input.history {
        encode(history, l.catalog.max_output_row_bytes)?;
    }
    let mut retained_bytes = 0usize;
    for (reference, resolution) in input.versions {
        let bytes = encode(reference, l.catalog.max_row_bytes)?
            .len()
            .checked_add(encode(resolution, l.catalog.max_output_row_bytes)?.len())
            .ok_or(Error::Budget(
                "navigation record retained resolution arithmetic",
            ))?;
        retained_bytes = retained_bytes
            .checked_add(bytes)
            .filter(|n| *n <= l.max_claim_cohort_bytes)
            .ok_or(Error::Budget("navigation record retained resolution bytes"))?;
    }

    let kind = text(entry, "record_type")?;
    let id = text(entry, "record_id")?;
    let source = text(entry, "source_record_ref")?;
    let expected = if let Some(history) = input.history {
        array(history, "refs")?
    } else {
        &[]
    };
    if input.versions.len() != expected.len()
        || input.versions.len() > 129
        || input
            .versions
            .iter()
            .zip(expected)
            .any(|((reference, _), expected)| reference != expected)
    {
        return Err(Error::Invalid(
            "navigation exact ordered history resolution set",
        ));
    }
    let mut properties = record
        .as_object()
        .ok_or(Error::Invalid("navigation metadata source object"))?
        .clone();
    properties.insert("source_record".into(), record.clone());
    if kind == "artifact" || input.native_composite {
        let (description, fields) = if kind == "artifact" {
            (
                &record["path_identity"]["note"],
                json!({"preferred_label":"/custody/inventory_numbers/0","description":"/path_identity/note","review_status":"/authority/review_status","visibility":"/authority/visibility"}),
            )
        } else {
            (
                &record["editorial_object"]["description"],
                json!({"preferred_label":"/preferred_label","description":"/editorial_object/description","identity_status":"/identity_status","review_status":"/authority/review_status","visibility":"/authority/visibility"}),
            )
        };
        properties.insert("description".into(), description.clone());
        properties.insert(
            "review_status".into(),
            record["authority"]["review_status"].clone(),
        );
        properties.insert(
            "visibility".into(),
            record["authority"]["visibility"].clone(),
        );
        properties.insert("metadata_field_sources".into(), fields);
        if kind == "artifact" {
            properties.insert(
                "label_source_pointer".into(),
                json!("/custody/inventory_numbers/0"),
            );
        }
    }
    if let Some((reference, forms)) = input.forms {
        properties.insert("human_forms".into(), forms.clone());
        properties.insert("human_forms_source_ref".into(), json!(reference));
        properties.insert("source_sha256".into(), entry["record_sha256"].clone());
    }
    if let Some(links) = entry.get("links").and_then(Value::as_object) {
        properties.extend(links.clone());
    }
    if let Some(labels) = record.get("variant_labels").filter(|v| v.is_array()) {
        properties.insert("variant_labels".into(), labels.clone());
    }
    if let Some(notes) = record["notes"].as_str().filter(|s| {
        !s.trim_matches(crate::source_bibliographic_unicode::is_python_whitespace)
            .is_empty()
    }) {
        properties.insert(
            "description".into(),
            json!(notes.trim_matches(crate::source_bibliographic_unicode::is_python_whitespace)),
        );
    }
    if let Some(identifiers) = record.get("external_identifiers").filter(|v| v.is_array()) {
        properties.insert("external_identifiers".into(), identifiers.clone());
    }
    if kind == "link" {
        for key in [
            "uri",
            "link_kind",
            "provider_label",
            "interface_type",
            "access_status",
            "observed_at",
            "observation_ref",
            "mutable",
            "provenance_event_ref",
        ] {
            properties.insert(key.into(), record[key].clone());
        }
    }
    if let Some(history) = input.history {
        let mut value = history
            .as_object()
            .ok_or(Error::Invalid("navigation metadata history envelope"))?
            .clone();
        value.insert(
            "schema_version".into(),
            json!("tos_metadata_record_history_v1"),
        );
        properties.insert("record_history".into(), Value::Object(value));
    }
    let label = entry["preferred_label"]
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or(id);
    let identity = entry["identity_status"]
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or("unknown");
    let mut output = NavigationRecordProjection {
        nodes: vec![
            json!({"node_id":id,"node_kind":kind,"label":label,"source_ref":source,"identity_status":identity,"properties":properties}),
        ],
        edges: Vec::new(),
        diagnostics: Vec::new(),
    };
    for (reference, resolved) in input.versions {
        let node = project_record_version(reference, resolved, "metadata", l)?;
        output.edges.push(edge(
            format!(
                "source-navigation:record-history:{}",
                text(&node, "node_id")?
            ),
            id,
            "has_record_version",
            &node,
            &[source, text(&node, "source_ref")?],
        )?);
        output.nodes.push(node);
    }
    if let Some(history) = input.history.filter(|v| v["status"] != "available") {
        output.diagnostics.push(json!({"level":"warning","path":source,"message":format!("exact metadata history unavailable: {}/{}",text(history,"status")?,text(history,"reason")?)}));
    }
    output.check(l)?;
    Ok(output)
}
fn resolved(reference: &Value, version: Version) -> Value {
    json!({"status":"available","reason":format!("exact-{}-version",version.version_status),"exact_ref":reference,
        "version_status":version.version_status,"record":version.record,"record_digest":reference["digest"],"provenance":version.provenance,
        "grants_current_use":false,"performs_assessment":false,"writes_to_source":false})
}
/// Checked per-record producer. Catalog rows and original source schemas have
/// already been sealed by the catalog owner; Versions additionally proves the
/// independent current cut and every selected retained source read. Global
/// navigation incidence, publication and admission remain caller-owned.
pub(crate) fn prepare_navigation_record_from_catalog(
    stage: &mut KnowledgeStage<'_>,
    receipt: &SourceCatalogReceipt,
    versions: &mut Versions<'_, '_>,
    id: &str,
    entities: &Value,
    validator: &SourceCatalogValidator<'_>,
    forms: &mut dyn BibliographicForms,
    l: BibliographicLimits,
) -> Result<NavigationRecordProjection> {
    l.validate()?;
    versions.verify_catalog_binding(stage, receipt, l)?;
    let row = catalog::catalog_row(stage, "records", id, l.catalog)?
        .ok_or(Error::Invalid("navigation catalog record absent"))?;
    let entry = &row["entry"];
    let source = text(entry, "source_record_ref")?;
    let current = versions.exact_record_refs(stage, id, validator, entities, l)?;
    let record = current.record.clone();
    let mut provenance = current.provenance.clone();
    let p = provenance
        .as_object_mut()
        .ok_or(Error::Invalid("navigation history provenance object"))?;
    p.remove("source");
    p.remove("transition");
    let history = json!({"status":"available","reason":"verified-record-references","record_id":id,"current_ref":current.current_ref,
        "refs":current.refs,"provenance":provenance,"grants_current_use":false,"performs_assessment":false,"writes_to_source":false});
    let references = array(&history, "refs")?;
    if references
        .len()
        .checked_mul(2)
        .and_then(|n| n.checked_add(3))
        .is_none_or(|n| n > l.max_claim_cohort_rows)
    {
        return Err(Error::Budget("navigation record history row cap"));
    }
    let mut resolved_versions = Vec::with_capacity(references.len());
    let mut retained_bytes = 0usize;
    for reference in references {
        let value = resolved(
            reference,
            versions.resolve_record(stage, reference, validator, entities, l)?,
        );
        retained_bytes = retained_bytes
            .checked_add(encode(&value, l.catalog.max_output_row_bytes)?.len())
            .filter(|n| *n <= l.max_claim_cohort_bytes)
            .ok_or(Error::Budget("navigation retained record versions bytes"))?;
        resolved_versions.push((reference.clone(), value));
    }
    let form_ref = format!(
        "{}.human-forms.json",
        source
            .strip_suffix(".json")
            .ok_or(Error::Invalid("navigation source metadata basename"))?
    );
    let materialized = graph::forms(stage, &form_ref, &record, validator, forms, l)?;
    let mut output = project_source_navigation_record(
        NavigationRecordInput {
            entry,
            source_record: &record,
            forms: materialized.as_ref().map(|(r, v)| (r.as_str(), v)),
            history: Some(&history),
            versions: &resolved_versions,
            native_composite: source.ends_with("/composite-witness.json"),
        },
        l,
    )?;
    if entry["record_type"] == "sign" {
        let owner = array(entities, "types")?
            .iter()
            .filter(|e| e["type_id"] == "tos.entity.sign")
            .collect::<Vec<_>>();
        if owner.len() != 1
            || owner[0]["source_record_profile"]["reader"] != "semantic-metadata-v1"
            || owner[0]["source_record_profile"]["creation_gate"] != "sign-promotion-v1"
        {
            return Err(Error::Invalid(
                "navigation Sign source-owned promotion profile",
            ));
        }
        let reference = &record["promotion_basis"]["candidate"];
        let value = versions.resolve_claim(stage, reference, validator, forms, l)?;
        let node = project_record_version(reference, &resolved(reference, value), "claim", l)?;
        let basis = format!("{source}#/promotion_basis/candidate");
        output.edges.push(edge(
            format!("source-navigation:promotion-basis:{id}"),
            id,
            "promotion_basis_version",
            &node,
            &[basis.as_str(), text(&node, "source_ref")?],
        )?);
        output.nodes.push(node);
        output.check(l)?;
    }
    versions.verify_catalog_binding(stage, receipt, l)?;
    Ok(output)
}

#[cfg(test)]
mod oracle {
    use super::*;
    use std::time::{Duration, Instant};
    #[test]
    fn retained_python_record_projection_preserves_complete_semantic_source() {
        let raw = include_bytes!(
            "../../../../access/tests/fixtures/knowledge-contract/ToS/source-witnesses/semantic-descriptions/crosscutting-concept-freedom/crosscutting-concept.json"
        );
        let source: Value = serde_json::from_slice(raw).unwrap();
        let reference = "ToS/source-witnesses/semantic-descriptions/crosscutting-concept-freedom/crosscutting-concept.json";
        let entry = catalog::render_catalog_record(
            &source,
            reference,
            Some("ToS/contracts/semantic-description-record.schema.json"),
            262144,
        )
        .unwrap();
        let limits = BibliographicLimits {
            catalog: catalog::SourceCatalogLimits {
                max_files: 128,
                max_rows: 4096,
                max_file_bytes: 2097152,
                max_row_bytes: 1048576,
                max_contract_bytes: 2097152,
                max_output_row_bytes: 262144,
            },
            max_claim_cohort_rows: 256,
            max_claim_cohort_bytes: 67108864,
            max_output_rows: 1000,
            max_output_bytes: 67108864,
            deadline: Instant::now() + Duration::from_secs(5),
        };
        let out = project_source_navigation_record(
            NavigationRecordInput {
                entry: &entry,
                source_record: &source,
                forms: None,
                history: None,
                versions: &[],
                native_composite: false,
            },
            limits,
        )
        .unwrap();
        assert_eq!(out.nodes.len(), 1);
        assert!(out.edges.is_empty() && out.diagnostics.is_empty());
        assert_eq!(
            digest(&out.nodes[0], 262144).unwrap(),
            "6be28d148a8d56fb0b2eed328a2324fa1993d3a4c281e07018d69cce3b42bb19"
        );
    }
}
