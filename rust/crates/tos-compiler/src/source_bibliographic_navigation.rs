//! The maintained per-record SourceNavigation renderer and its catalog/cut
//! composition. External incidence and global conflict ordering stay with the
//! navigation producer. These rows confer no assessment or current-use grant.
use crate::knowledge_stage::KnowledgeStage;
use crate::source_bibliographic::{self as graph, BibliographicForms, BibliographicLimits};
use crate::source_bibliographic_render::{array, digest, encode, text};
use crate::source_bibliographic_versions::{Version, Versions};
use crate::source_witness_catalog::{self as catalog, SourceCatalogValidator};
use crate::{Error, Result};
use serde_json::{Value, json};
use std::collections::BTreeSet;

/// Adapt reified bibliographic rows without discarding source fields or changing
/// their identities. Transport metadata is retained inside the six-field
/// navigation envelope; authored properties remain exactly as supplied.
pub(crate) fn navigation_claim_carriers(
    mut cohort: graph::SuppliedBibliographicClaimCohort,
    limits: BibliographicLimits,
) -> Result<graph::SuppliedBibliographicClaimCohort> {
    for row in &mut cohort.nodes {
        let original = row
            .as_object()
            .ok_or(Error::Invalid("navigation Claim node object"))?;
        let mut properties = row["properties"]
            .as_object()
            .ok_or(Error::Invalid("navigation Claim node properties"))?
            .clone();
        let carrier = original
            .iter()
            .filter(|(key, _)| {
                !matches!(
                    key.as_str(),
                    "node_id" | "node_kind" | "source_ref" | "properties"
                )
            })
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<serde_json::Map<_, _>>();
        if properties
            .insert("bibliographic_carrier".into(), Value::Object(carrier))
            .is_some()
        {
            return Err(Error::Invalid("navigation Claim reserved carrier property"));
        }
        let kind = if row["node_kind"] == "identity" {
            text(&row["properties"], "identity_kind")?
        } else {
            text(row, "node_kind")?
        };
        let label = row
            .pointer("/display/title/default")
            .and_then(Value::as_str)
            .or_else(|| {
                row.pointer("/properties/preferred_label")
                    .and_then(Value::as_str)
            })
            .filter(|value| !value.is_empty())
            .unwrap_or(text(row, "node_id")?);
        let status = row
            .pointer("/properties/identity_status")
            .and_then(Value::as_str)
            .unwrap_or("not_applicable");
        *row = json!({"node_id":row["node_id"], "node_kind":kind, "label":label,
            "source_ref":row["source_ref"], "identity_status":status, "properties":properties});
        encode(row, limits.catalog.max_output_row_bytes)?;
    }
    for row in &mut cohort.edges {
        row["predicate_id"] = row["edge_kind"].clone();
        row["source_ref"] = row["source_claim_file_ref"].clone();
        row["source_refs"] = json!([row["source_claim_file_ref"]]);
        encode(row, limits.catalog.max_output_row_bytes)?;
    }
    // The metadata record and its reified identity carrier are distinct views
    // of the same source ID. Make the Claim discoverable from the metadata
    // record without replacing that record or assigning it to this Claim.
    let mut subject_edges = Vec::new();
    for edge in &cohort.edges {
        if edge["edge_kind"] != "has_subject" {
            continue;
        }
        let Some(subject) = cohort.nodes.iter().find(|node| {
            node["node_id"] == edge["to_id"]
                && node
                    .pointer("/properties/identity_ref")
                    .and_then(Value::as_str)
                    .is_some()
        }) else {
            continue;
        };
        let identity = text(&subject["properties"], "identity_ref")?;
        let claim = text(edge, "claim_ref")?;
        let mut inverse = edge.clone();
        inverse["edge_id"] = json!(format!("source-navigation:claim-subject:{claim}"));
        inverse["from_id"] = json!(identity);
        inverse["to_id"] = edge["from_id"].clone();
        inverse["edge_kind"] = json!("claim_subject_navigation");
        inverse["predicate_id"] = json!("has_source_claim");
        navigation_claim_subject_identity(&inverse, &cohort.nodes, &cohort.edges)?
            .ok_or(Error::Invalid("navigation Claim subject anchor absent"))?;
        subject_edges.push(inverse);
    }
    if !subject_edges.is_empty() {
        let ids = cohort.trace["edge_ids"]
            .as_array_mut()
            .ok_or(Error::Invalid("navigation Claim trace edge IDs"))?;
        ids.extend(subject_edges.iter().map(|edge| edge["edge_id"].clone()));
        ids.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
        cohort.edges.extend(subject_edges);
        cohort
            .edges
            .sort_by(|a, b| a["edge_id"].as_str().cmp(&b["edge_id"].as_str()));
    }
    if cohort
        .nodes
        .len()
        .checked_add(cohort.edges.len())
        .and_then(|n| n.checked_add(1))
        .is_none_or(|n| n > limits.max_claim_cohort_rows)
    {
        return Err(Error::Budget("navigation Claim carrier rows"));
    }
    let mut bytes = 0usize;
    for row in cohort
        .nodes
        .iter()
        .chain(&cohort.edges)
        .chain(std::iter::once(&cohort.trace))
    {
        bytes = bytes
            .checked_add(encode(row, limits.catalog.max_output_row_bytes)?.len())
            .filter(|n| *n <= limits.max_claim_cohort_bytes)
            .ok_or(Error::Budget("navigation Claim carrier bytes"))?;
    }
    Ok(cohort)
}

/// Check the one external metadata endpoint introduced by Claim navigation.
/// Its identity and directed subject edge must both be present in this exact
/// cohort. This grants neither ownership of the metadata nor its visibility.
pub(crate) fn navigation_claim_subject_identity<'a>(
    edge: &Value,
    nodes: &'a [Value],
    edges: &[Value],
) -> Result<Option<&'a Value>> {
    if edge["edge_kind"] != "claim_subject_navigation" {
        return Ok(None);
    }
    let claim = text(edge, "claim_ref")?;
    let from = text(edge, "from_id")?;
    let root = format!("claim:{claim}");
    let identity_id = format!("identity:{from}");
    let identity = nodes
        .iter()
        .find(|node| node["node_id"] == identity_id)
        .ok_or(Error::Invalid("navigation Claim subject identity absent"))?;
    if edge["predicate_id"] != "has_source_claim"
        || edge["edge_id"] != format!("source-navigation:claim-subject:{claim}")
        || edge["to_id"] != root
        || identity["properties"]["identity_ref"] != from
        || !nodes.iter().any(|node| {
            node["node_id"] == root
                && node["properties"]["source_claim"]["claim_id"] == claim
                && node["properties"]["source_claim"]["subject_ref"] == from
        })
        || !edges.iter().any(|subject| {
            subject["edge_kind"] == "has_subject"
                && subject["from_id"] == root
                && subject["to_id"] == identity_id
                && subject["claim_ref"] == claim
                && subject["source_claim_file_ref"] == edge["source_claim_file_ref"]
        })
    {
        return Err(Error::Invalid("navigation Claim subject anchor differs"));
    }
    Ok(Some(identity))
}

/// Borrowed inputs are not a source receipt. Pure rendering retains the exact
/// ordered history resolution set supplied by the checked caller.
pub struct NavigationRecordInput<'a> {
    pub entry: &'a Value,
    pub source_record: &'a Value,
    pub forms: Option<(&'a str, &'a Value)>,
    pub history: Option<&'a Value>,
    pub versions: &'a [(Value, Value)],
    pub native_composite: bool,
}
pub struct NavigationRecordProjection {
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
fn metadata_history(current: &Version, id: &str) -> Result<Value> {
    let mut provenance = current.provenance.clone();
    let p = provenance
        .as_object_mut()
        .ok_or(Error::Invalid("navigation history provenance object"))?;
    p.remove("source");
    p.remove("transition");
    Ok(
        json!({"status":"available","reason":"verified-record-references","record_id":id,"current_ref":current.current_ref,
        "refs":current.refs,"provenance":provenance,"grants_current_use":false,"performs_assessment":false,"writes_to_source":false}),
    )
}
/// The used initial-Agent branch shares Versions and the record renderer.
/// Caller proves committed initial creation and schema/form/current custody.
pub(crate) fn project_managed_initial_agent(
    entry: &Value,
    source: &Value,
    raw: &[u8],
    entities: &Value,
    catalog: (u64, String),
    forms: Option<(&str, &Value)>,
    l: BibliographicLimits,
) -> Result<NavigationRecordProjection> {
    let version = crate::source_bibliographic_versions::initial_managed_agent_version(
        entry, source, raw, entities, catalog, l,
    )?;
    let history = metadata_history(&version, text(entry, "record_id")?)?;
    let reference = version.current_ref.clone();
    let versions = vec![(reference.clone(), resolved(&reference, version))];
    project_source_navigation_record(
        NavigationRecordInput {
            entry,
            source_record: source,
            forms,
            history: Some(&history),
            versions: &versions,
            native_composite: false,
        },
        l,
    )
}

pub(crate) fn project_managed_initial_agent_with_epoch_and_work(
    entry: &Value,
    source: &Value,
    raw: &[u8],
    entities: &Value,
    epoch: &crate::VersionsCatalogEpochV2,
    tree_limits: tos_segment_store::AuthenticatedTreeLimitsV1,
    deadline: std::time::Instant,
    cancelled: &std::sync::atomic::AtomicBool,
    forms: Option<(&str, &Value)>,
    l: BibliographicLimits,
) -> Result<(
    NavigationRecordProjection,
    tos_segment_store::AuthenticatedTreeWorkV1,
)> {
    let (version, work) =
        crate::source_bibliographic_versions::initial_managed_agent_version_with_epoch_and_work(
            entry,
            source,
            raw,
            entities,
            epoch,
            tree_limits,
            deadline,
            cancelled,
            l,
        )?;
    let history = metadata_history(&version, text(entry, "record_id")?)?;
    let reference = version.current_ref.clone();
    let versions = vec![(reference.clone(), resolved(&reference, version))];
    project_source_navigation_record(
        NavigationRecordInput {
            entry,
            source_record: source,
            forms,
            history: Some(&history),
            versions: &versions,
            native_composite: false,
        },
        l,
    )
    .map(|projection| (projection, work))
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
/// The enclosing navigation operation verifies the complete private catalog
/// before and after projection. Each addressed seek still checks its digest;
/// repeating the entire input census around every record adds corpus-wide I/O.
pub(crate) fn prepare_navigation_record_from_catalog(
    stage: &mut KnowledgeStage<'_>,
    versions: &mut Versions<'_, '_>,
    id: &str,
    entities: &Value,
    validator: &SourceCatalogValidator<'_>,
    forms: &mut dyn BibliographicForms,
    l: BibliographicLimits,
) -> Result<NavigationRecordProjection> {
    l.validate()?;
    let row = catalog::catalog_row(stage, "records", id, l.catalog)?
        .ok_or(Error::Invalid("navigation catalog record absent"))?;
    let entry = &row["entry"];
    let source = text(entry, "source_record_ref")?;
    let record = versions.current_record(stage, id, validator, l)?;
    let history = if versions.supports_record(stage, id, entities, l)? {
        let current = versions.exact_record_refs(stage, id, validator, entities, l)?;
        if current.record != record {
            return Err(Error::Invalid("navigation current/history source equality"));
        }
        Some(metadata_history(&current, id)?)
    } else {
        None
    };
    let references = if let Some(history) = &history {
        array(history, "refs")?
    } else {
        &[]
    };
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
    let contracts = versions.contracts();
    let materialized =
        graph::forms_with_contracts(stage, &form_ref, &record, &contracts, validator, forms, l)?;
    let mut output = project_source_navigation_record(
        NavigationRecordInput {
            entry,
            source_record: &record,
            forms: materialized.as_ref().map(|(r, v)| (r.as_str(), v)),
            history: history.as_ref(),
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
        let value = versions.resolve_claim_view(stage, reference, validator, forms, l)?;
        let available = value["status"] == "available";
        let node = project_record_version(reference, &value, "claim", l)?;
        let basis = format!("{source}#/promotion_basis/candidate");
        output.edges.push(edge(
            format!("source-navigation:promotion-basis:{id}"),
            id,
            "promotion_basis_version",
            &node,
            &[basis.as_str(), text(&node, "source_ref")?],
        )?);
        output.nodes.push(node);
        if !available {
            output.diagnostics.push(json!({"level":"warning","path":source,"message":format!("exact Sign promotion basis unavailable: {}/{}",text(&value,"status")?,text(&value,"reason")?)}));
        }
        output.check(l)?;
    }
    Ok(output)
}

#[cfg(test)]
mod oracle {
    use super::*;
    use std::time::{Duration, Instant};
    #[test]
    fn navigation_claim_carrier_preserves_source_and_exact_evidence() {
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
        let claim =
            json!({"claim_id":"tos.claim.a", "object":"Точное чтение", "visibility":"public"});
        let original = json!({"node_id":"claim:tos.claim.a","node_kind":"claim",
            "source_ref":"ToS/source-witnesses/claims/a/source-claims.jsonl", "source_line":7,
            "source_sha256":"ab".repeat(32), "properties":{"source_claim":claim,"visibility":"public"}});
        let edge = json!({"edge_id":"edge:a","edge_kind":"supported_by","from_id":"claim:tos.claim.a",
            "to_id":"evidence:a","source_claim_file_ref":original["source_ref"],
            "claim_ref":"tos.claim.a","claim_sha256":original["source_sha256"],
            "source_claim_line":7,"review_status":"unreviewed","evidence_node_ids":["evidence:a"],
            "maker_node_id":"maker:a","provenance_event_node_id":"provenance_event:a"});
        let output = navigation_claim_carriers(
            graph::SuppliedBibliographicClaimCohort {
                nodes: vec![original.clone()],
                edges: vec![edge],
                trace: json!({"claim_ref":"tos.claim.a"}),
            },
            limits,
        )
        .unwrap();
        let node = &output.nodes[0];
        assert_eq!(node.as_object().unwrap().len(), 6);
        assert_eq!(node["properties"]["source_claim"], claim);
        assert_eq!(node["source_ref"], original["source_ref"]);
        assert_eq!(
            node["properties"]["bibliographic_carrier"]["source_line"],
            7
        );
        assert_eq!(
            node["properties"]["bibliographic_carrier"]["source_sha256"],
            original["source_sha256"]
        );
        assert_eq!(output.edges[0]["predicate_id"], "supported_by");
        assert_eq!(
            output.edges[0]["source_refs"],
            json!([original["source_ref"]])
        );
        assert_eq!(output.trace, json!({"claim_ref":"tos.claim.a"}));
        let schema_uri = "https://tree-of-sophia.local/ToS/contracts/tos-corpus-index.schema.json";
        let probe = tos_validation::SchemaBackendProbe::new(
            [tos_validation::SchemaResource {
                uri: schema_uri.into(),
                raw: include_bytes!("../../../../ToS/contracts/tos-corpus-index.schema.json")
                    .to_vec(),
            }],
            tos_validation::FormatProfile::AssertedSourceCandidateV1,
        )
        .unwrap();
        let edge_uri = format!("{schema_uri}#/$defs/sourceNavigationEdge");
        let validates = |row: &Value| {
            probe
                .is_valid_raw(&edge_uri, &serde_json::to_vec(row).unwrap())
                .unwrap()
        };
        assert!(validates(&output.edges[0]));
        for field in [
            "claim_ref",
            "claim_sha256",
            "evidence_node_ids",
            "maker_node_id",
            "provenance_event_node_id",
            "source_claim_file_ref",
            "source_claim_line",
            "source_ref",
        ] {
            let mut incomplete = output.edges[0].clone();
            incomplete.as_object_mut().unwrap().remove(field);
            assert!(!validates(&incomplete), "missing provenance field {field}");
        }
        let mut malformed = output.edges[0].clone();
        malformed["claim_sha256"] = json!("not-a-digest");
        assert!(!validates(&malformed));
        let mut unknown = output.edges[0].clone();
        unknown["unexpected"] = json!(true);
        assert!(!validates(&unknown));

        let mut subject_claim = original.clone();
        subject_claim["properties"]["source_claim"]["subject_ref"] = json!("tos.work.a");
        let subject = json!({"node_id":"identity:tos.work.a", "node_kind":"identity",
            "source_ref":"ToS/source-witnesses/works/a/work.json", "source_sha256":"cd".repeat(32),
            "properties":{"identity_ref":"tos.work.a","identity_kind":"work","preferred_label":"Work A"}});
        let subject_edge = json!({"edge_id":"edge:subject:a","edge_kind":"has_subject",
            "from_id":"claim:tos.claim.a","to_id":"identity:tos.work.a","claim_ref":"tos.claim.a",
            "source_claim_file_ref":original["source_ref"]});
        let output = navigation_claim_carriers(
            graph::SuppliedBibliographicClaimCohort {
                nodes: vec![subject_claim.clone(), subject.clone()],
                edges: vec![subject_edge.clone()],
                trace: json!({"claim_ref":"tos.claim.a","edge_ids":["edge:subject:a"]}),
            },
            limits,
        )
        .unwrap();
        let inverse = output
            .edges
            .iter()
            .find(|edge| edge["predicate_id"] == "has_source_claim")
            .unwrap();
        assert_eq!(inverse["from_id"], "tos.work.a");
        assert_eq!(inverse["to_id"], "claim:tos.claim.a");
        assert!(
            output
                .nodes
                .iter()
                .all(|node| node["node_id"] != "tos.work.a")
        );
        assert!(
            navigation_claim_subject_identity(inverse, &output.nodes, &output.edges)
                .unwrap()
                .is_some()
        );
        let mut altered = inverse.clone();
        altered["from_id"] = json!("tos.work.unrelated");
        assert!(navigation_claim_subject_identity(&altered, &output.nodes, &output.edges).is_err());
        assert!(navigation_claim_subject_identity(inverse, &output.nodes, &[]).is_err());
        let mut bounded = limits;
        bounded.max_claim_cohort_rows = 4;
        assert!(
            navigation_claim_carriers(
                graph::SuppliedBibliographicClaimCohort {
                    nodes: vec![subject_claim, subject],
                    edges: vec![subject_edge],
                    trace: json!({"claim_ref":"tos.claim.a","edge_ids":["edge:subject:a"]}),
                },
                bounded
            )
            .is_err()
        );
    }
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
