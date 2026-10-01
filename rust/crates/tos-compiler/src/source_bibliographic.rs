//! Bounded catalog→raw bibliographic producer over one sealed source cut.
//! All catalog identities are retained, and Claim cohorts remain reified.
//! Receipts are private candidates: no rights, publication or semantic grant.
use crate::knowledge_normalization::SourceRow;
use crate::knowledge_stage::{KnowledgeStage, WritePhase};
pub use crate::source_bibliographic_navigation::{
    NavigationRecordInput as SuppliedNavigationRecordInput,
    NavigationRecordProjection as SuppliedNavigationRecordProjection,
};
use crate::source_bibliographic_render::{self as render, array, digest, encode, node_id, text};
pub use crate::source_bibliographic_render::{
    ClaimInputs as SuppliedBibliographicClaimInputs, Cohort as SuppliedBibliographicClaimCohort,
};
pub use crate::source_bibliographic_versions::BibliographicSourceCut;

/// Render authenticated ordered metadata/history supplied by the source owner.
/// This is the existing per-record kernel; inputs are not a source receipt,
/// selected-cut proof, assessment, or grant of current use.
pub fn render_supplied_navigation_record(
    input: SuppliedNavigationRecordInput<'_>,
    limits: BibliographicLimits,
) -> Result<SuppliedNavigationRecordProjection> {
    crate::source_bibliographic_navigation::project_source_navigation_record(input, limits)
}

/// Input documents and emitted rows have different cost/size contracts.
#[derive(Clone, Copy)]
pub struct BibliographicDocumentLimits {
    pub input_document_bytes: usize,
    pub output_row_bytes: usize,
}
impl From<SourceCatalogLimits> for BibliographicDocumentLimits {
    fn from(limits: SourceCatalogLimits) -> Self {
        Self {
            input_document_bytes: limits.max_row_bytes,
            output_row_bytes: limits.max_output_row_bytes,
        }
    }
}

/// Existing metadata route descriptor over source-authenticated inputs.
pub fn supplied_metadata_descriptor(
    entry: &Value,
    record: &Value,
    entities: &Value,
    limits: BibliographicDocumentLimits,
) -> Result<Value> {
    for value in [entry, record, entities] {
        encode(value, limits.input_document_bytes)?;
    }
    let descriptor = crate::source_bibliographic_versions::supplied_metadata_descriptor(
        entry, record, entities,
    )?;
    encode(&descriptor, limits.output_row_bytes)?;
    Ok(descriptor)
}

/// The same literal renderer used by the maintained full Claim producer.
pub fn supplied_bibliographic_literal(
    entry: &Value,
    claim: &Value,
    limits: BibliographicDocumentLimits,
) -> Result<Value> {
    encode(entry, limits.input_document_bytes)?;
    encode(claim, limits.input_document_bytes)?;
    let row = render::literal(entry, claim, limits.input_document_bytes)?;
    encode(&row, limits.output_row_bytes)?;
    Ok(row)
}

/// Existing fixed reference/proposal grammar, with original member order.
pub fn supplied_claim_reference_members<'a>(
    claim: &'a Value,
    profile: &Value,
    max_row_bytes: usize,
) -> Result<Vec<&'a str>> {
    encode(claim, max_row_bytes)?;
    encode(profile, max_row_bytes)?;
    crate::source_bibliographic_values::members(claim, profile)
}

/// Same registry inheritance/domain predicate; no source lookup is performed.
pub fn supplied_bibliographic_endpoint_matches(
    node: &Value,
    allowed: &Value,
    entities: &Value,
    limits: BibliographicDocumentLimits,
) -> Result<bool> {
    for value in [node, allowed, entities] {
        encode(value, limits.input_document_bytes)?;
    }
    typed_endpoint(node, allowed, entities)
}

/// Pure bounded rendering only. The caller owns schema, source-slot, endpoint,
/// profile and currentness verification; this function grants no admission.
pub fn render_supplied_claim(
    inputs: SuppliedBibliographicClaimInputs<'_>,
    limits: BibliographicLimits,
) -> Result<SuppliedBibliographicClaimCohort> {
    limits.validate()?;
    let result = render::project(inputs, limits.catalog.max_output_row_bytes)?;
    if result
        .nodes
        .len()
        .checked_add(result.edges.len())
        .and_then(|n| n.checked_add(1))
        .is_none_or(|n| n > limits.max_claim_cohort_rows)
    {
        return Err(Error::Budget("bibliographic Claim cohort rows"));
    }
    let mut bytes = 0usize;
    for row in result
        .nodes
        .iter()
        .chain(&result.edges)
        .chain(std::iter::once(&result.trace))
    {
        bytes = bytes
            .checked_add(encode(row, limits.catalog.max_output_row_bytes)?.len())
            .filter(|n| *n <= limits.max_claim_cohort_bytes)
            .ok_or(Error::Budget("bibliographic Claim cohort bytes"))?;
    }
    Ok(result)
}

/// Rebuild the existing descriptor syntax from explicitly supplied source
/// Claim and endpoints. Source authentication remains with the caller.
pub fn supplied_claim_navigation_descriptor(
    claim: &Value,
    subject: &Value,
    object: &Value,
    registry: &Value,
    entities: &Value,
    limits: BibliographicDocumentLimits,
) -> Result<Option<Value>> {
    if limits.input_document_bytes == 0 || limits.output_row_bytes == 0 {
        return Err(Error::Budget("bibliographic descriptor row cap"));
    }
    for value in [claim, subject, object, registry, entities] {
        encode(value, limits.input_document_bytes)?;
    }
    let descriptor = render::descriptor(claim, subject, object, registry, entities, limits)?;
    if let Some(value) = &descriptor {
        encode(value, limits.output_row_bytes)?;
    }
    Ok(descriptor)
}

/// Project an already authenticated selected metadata identity.
pub fn supplied_bibliographic_identity(
    entry: &Value,
    source: &Value,
    forms: Option<(&str, &Value)>,
    limits: BibliographicDocumentLimits,
) -> Result<Value> {
    if limits.input_document_bytes == 0 || limits.output_row_bytes == 0 {
        return Err(Error::Budget("bibliographic identity row cap"));
    }
    encode(entry, limits.input_document_bytes)?;
    encode(source, limits.input_document_bytes)?;
    if let Some((_, value)) = forms {
        encode(value, limits.input_document_bytes)?;
    }
    let result = render::identity(entry, source, forms)?;
    encode(&result, limits.output_row_bytes)?;
    Ok(result)
}

use crate::source_witness_catalog::{
    self as catalog, BIBLIOGRAPHIC_FILES, CATALOG_SOURCE, CONTRACT_FILES, SOURCE_FILES,
    SourceCatalogLimits, SourceCatalogReceipt, SourceCatalogValidator,
};
use crate::{Error, Result};
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use tos_foundation::{Digest256, Digest256Hasher};
const SCHEMA: &str = "ToS/contracts/source-witness-bibliographic-graph.schema.json";
const ENTITY: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const RELATION: &str = "ToS/doctrine/semantic-interchange/relation-types.v1.json";
const CLAIM_CATALOG: &str = "ToS/source-witnesses/catalog/claims.jsonl";

#[derive(Clone, Copy, Debug)]
pub struct BibliographicLimits {
    pub catalog: SourceCatalogLimits,
    pub max_claim_cohort_rows: usize,
    pub max_claim_cohort_bytes: usize,
    pub max_output_rows: u64,
    pub max_output_bytes: u64,
    pub deadline: std::time::Instant,
}
impl BibliographicLimits {
    pub fn validate(self) -> Result<()> {
        if std::time::Instant::now() >= self.deadline {
            return Err(Error::Budget("bibliographic deadline"));
        }
        if self.max_claim_cohort_rows == 0
            || self.max_claim_cohort_rows > 16384
            || self.max_claim_cohort_bytes == 0
            || self.max_claim_cohort_bytes > 128 * 1024 * 1024
            || self
                .max_claim_cohort_rows
                .checked_mul(self.catalog.max_output_row_bytes)
                .is_none_or(|n| n > self.max_claim_cohort_bytes)
            || self.max_output_rows == 0
            || self.max_output_bytes == 0
        {
            return Err(Error::Budget("bibliographic producer limits"));
        }
        Ok(())
    }
}
#[derive(Debug)]
pub struct BibliographicReceipt {
    pub node_count: u64,
    pub edge_count: u64,
    pub claim_count: u64,
    pub row_root_sha256: String,
    pub source_catalog_root_sha256: String,
    pub source_reference_closure_verified: bool,
    pub selected_version_source: Option<Value>,
    summary_sha256: String,
}
/// Sorted exact raw rows accepted by reified-bibliographic-claims-v1. Any
/// failure invalidates the whole private candidate; selection remains external.
pub trait BibliographicSink {
    fn row(&mut self, collection: &str, id: &str, raw: &[u8]) -> Result<()>;
}
/// Operational adapters invoke the maintained native source-forms engine.
/// It derives wording only. Exact source/set/schema closure remains here;
/// assessment, publication and current-rights authority stay outside compiler.
pub trait BibliographicForms {
    fn materialize(
        &mut self,
        source: &Value,
        set: &Value,
        max_output_bytes: usize,
    ) -> Result<Vec<Value>>;
    /// Reconstruct only the retained source-form calculation. The producer
    /// verifies selectors, source/set/schema/archive closure and receipt refs.
    /// The maintained engine supplies no history, admission or write grant.
    fn reconstruct_revision_forms(
        &mut self,
        revised_source: &Value,
        prior_set: Option<&Value>,
        principal: &str,
        selections: &Value,
        max_output_bytes: usize,
    ) -> Result<Value> {
        let _ = (
            revised_source,
            prior_set,
            principal,
            selections,
            max_output_bytes,
        );
        Err(Error::Invalid(
            "bibliographic retained source-form reconstruction adapter absent",
        ))
    }
}
pub(crate) fn forms(
    stage: &KnowledgeStage<'_>,
    reference: &str,
    source: &Value,
    validator: &SourceCatalogValidator<'_>,
    materializer: &mut dyn BibliographicForms,
    l: BibliographicLimits,
) -> Result<Option<(String, Value)>> {
    let Some(raw) = raw_file(stage, reference, l)? else {
        return Ok(None);
    };
    if raw.len() > 2_097_152 {
        return Err(Error::Budget("bibliographic HumanForm set bytes"));
    }
    catalog::check_catalog_schema(
        stage,
        validator,
        l.catalog,
        "ToS/contracts/human-form-set.schema.json",
        "",
        &raw,
    )?;
    materialize_checked_form_set(&raw, reference, source, materializer, l)
}
/// The same form binding/materialization law after the owner schema check.
/// Managed addressed input uses this exact core, never a second renderer.
pub(crate) fn materialize_checked_form_set(
    raw: &[u8],
    reference: &str,
    source: &Value,
    materializer: &mut dyn BibliographicForms,
    l: BibliographicLimits,
) -> Result<Option<(String, Value)>> {
    owned_ref(reference)?;
    if raw.len() > 2_097_152 {
        return Err(Error::Budget("bibliographic HumanForm set bytes"));
    }
    let set = SourceRow::parse(&raw, l.catalog.max_row_bytes)?
        .value()
        .clone();
    let identity = source
        .get("claim_id")
        .or_else(|| source.get("record_id"))
        .or_else(|| source.get("artifact_id"))
        .or_else(|| source.get("composite_id"))
        .ok_or(Error::Invalid("bibliographic form subject identity"))?;
    if set.pointer("/subject/id") != Some(identity) {
        return Err(Error::Invalid("bibliographic form set subject differs"));
    }
    let values = materializer.materialize(source, &set, 262_144)?;
    let expected_subject = json!({"id":identity,"version":source.get("claim_version").or_else(||source.get("record_version")).ok_or(Error::Invalid("bibliographic form subject version"))?,"digest":format!("sha256:{}",digest(source,l.catalog.max_row_bytes)?)});
    let declared = array(&set, "forms")?;
    if values.len() != declared.len() {
        return Err(Error::Invalid("bibliographic form complete output order"));
    }
    for (value, form) in values.iter().zip(declared) {
        if value["schema_version"] != "tos_human_form_materialization_v1"
            || value["subject"] != expected_subject
            || value["form"]
                != json!({"id":form["form_id"],"version":form["form_version"],"digest":format!("sha256:{}",digest(form,l.catalog.max_row_bytes)?)})
            || value["performs_semantic_assessment"] != false
        {
            return Err(Error::Invalid(
                "bibliographic materializer exact output binding",
            ));
        }
    }
    let value = json!(values);
    encode(&value, 262_144)?;
    Ok(Some((reference.into(), value)))
}
fn owned_ref(reference: &str) -> Result<()> {
    if !reference.starts_with("ToS/")
        || reference.len() > 4096
        || reference.contains(['\\', '\0'])
        || reference
            .split('/')
            .any(|p| p.is_empty() || p.starts_with('.'))
        || reference
            .split('/')
            .any(|p| matches!(p, "payload" | "local-content" | "owner-local" | "private"))
    {
        return Err(Error::Invalid("bibliographic metadata locator"));
    }
    Ok(())
}
fn raw_file(
    stage: &KnowledgeStage<'_>,
    reference: &str,
    l: BibliographicLimits,
) -> Result<Option<Vec<u8>>> {
    l.validate()?;
    owned_ref(reference)?;
    let mut found = None;
    for name in [SOURCE_FILES, CONTRACT_FILES, BIBLIOGRAPHIC_FILES] {
        if !stage
            .input_collections()
            .iter()
            .any(|c| c.source_graph == CATALOG_SOURCE && c.collection == name)
        {
            continue;
        }
        if let Some(row) = stage.raw_by_id(CATALOG_SOURCE, name, reference)? {
            if row.payload.len() > l.catalog.max_file_bytes {
                return Err(Error::Budget("bibliographic source file bytes"));
            }
            if found.is_some() {
                return Err(Error::Invalid(
                    "bibliographic duplicate source file collection",
                ));
            }
            found = Some(row.payload);
        }
    }
    Ok(found)
}
fn json_file(stage: &KnowledgeStage<'_>, reference: &str, l: BibliographicLimits) -> Result<Value> {
    let raw = raw_file(stage, reference, l)?
        .ok_or(Error::Invalid("bibliographic exact source file missing"))?;
    Ok(SourceRow::parse(&raw, l.catalog.max_row_bytes)?
        .value()
        .clone())
}
pub(crate) fn slot(
    stage: &mut KnowledgeStage<'_>,
    kind: &str,
    id: &str,
    l: BibliographicLimits,
) -> Result<Option<(Value, Value)>> {
    let key = String::from_utf8(encode(&json!([kind, id]), l.catalog.max_output_row_bytes)?)
        .map_err(|_| Error::Invalid("bibliographic slot key"))?;
    let Some(slot) = catalog::catalog_row(stage, "slots", &key, l.catalog)? else {
        return Ok(None);
    };
    let location = &slot["source"];
    let reference = text(location, "source_ref")?;
    let raw =
        raw_file(stage, reference, l)?.ok_or(Error::Invalid("bibliographic slot source absent"))?;
    if raw.len() as u64 != location["file_bytes"].as_u64().unwrap_or(u64::MAX)
        || Digest256::of_bytes(&raw).to_hex() != text(location, "file_sha256")?
    {
        return Err(Error::Invalid("bibliographic slot file binding"));
    }
    let offset = location["byte_offset"]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or(Error::Invalid("bibliographic slot offset"))?;
    let size = location["row_bytes"]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .filter(|n| *n <= l.catalog.max_row_bytes)
        .ok_or(Error::Budget("bibliographic slot row"))?;
    let end = offset
        .checked_add(size)
        .ok_or(Error::Budget("bibliographic slot arithmetic"))?;
    let bytes = raw
        .get(offset..end)
        .ok_or(Error::Invalid("bibliographic slot range"))?;
    if Digest256::of_bytes(bytes).to_hex() != text(location, "raw_row_sha256")? {
        return Err(Error::Invalid("bibliographic slot raw digest"));
    }
    let value = SourceRow::parse(bytes, l.catalog.max_row_bytes)?
        .value()
        .clone();
    if digest(&value, l.catalog.max_row_bytes)? != text(location, "canonical_sha256")? {
        return Err(Error::Invalid("bibliographic slot canonical digest"));
    }
    Ok(Some((value, location.clone())))
}
fn identity(
    stage: &mut KnowledgeStage<'_>,
    id: &str,
    validator: &SourceCatalogValidator<'_>,
    materializer: &mut dyn BibliographicForms,
    l: BibliographicLimits,
) -> Result<Option<Value>> {
    let Some(row) = catalog::catalog_row(stage, "records", id, l.catalog)? else {
        return Ok(None);
    };
    let entry = &row["entry"];
    let reference = text(entry, "source_record_ref")?;
    let raw = raw_file(stage, reference, l)?
        .ok_or(Error::Invalid("bibliographic identity file missing"))?;
    if Digest256::of_bytes(&raw).to_hex() != text(&row["source"], "raw_sha256")? {
        return Err(Error::Invalid("bibliographic identity raw binding"));
    }
    let source = SourceRow::parse(&raw, l.catalog.max_row_bytes)?
        .value()
        .clone();
    if digest(&source, l.catalog.max_row_bytes)? != text(entry, "record_sha256")? {
        return Err(Error::Invalid("bibliographic identity canonical binding"));
    }
    let sibling = format!(
        "{}.human-forms.json",
        reference
            .strip_suffix(".json")
            .ok_or(Error::Invalid("bibliographic identity extension"))?
    );
    let bound_forms = forms(stage, &sibling, &source, validator, materializer, l)?;
    Ok(Some(render::identity(
        entry,
        &source,
        bound_forms.as_ref().map(|(r, v)| (r.as_str(), v)),
    )?))
}
fn event(
    stage: &mut KnowledgeStage<'_>,
    id: &str,
    validator: &SourceCatalogValidator<'_>,
    l: BibliographicLimits,
) -> Result<Value> {
    let (source, location) = slot(stage, "provenance_event", id, l)?
        .ok_or(Error::Invalid("bibliographic provenance missing"))?;
    let version = text(&source, "schema_version")?;
    let route = match version {
        "tos_provenance_event_v1" => "ToS/contracts/provenance-event.schema.json",
        "tos_provenance_event_v2" => "ToS/contracts/provenance-event-v2.schema.json",
        _ => return Err(Error::Invalid("bibliographic provenance schema")),
    };
    catalog::check_catalog_schema(
        stage,
        validator,
        l.catalog,
        route,
        "",
        &encode(&source, l.catalog.max_row_bytes)?,
    )?;
    let (_activity, _agents) = if version == "tos_provenance_event_v2" {
        if !matches!(
            source
                .pointer("/rights_and_visibility/content_visibility")
                .and_then(Value::as_str),
            Some("tracked_public_metadata" | "public_content" | "public_synthetic")
        ) {
            return Err(Error::Invalid("bibliographic private provenance"));
        }
        let issues = tos_validation::provenance_rules::semantic_issues(&source, 4096, l.deadline)
            .map_err(|_| Error::Budget("bibliographic provenance semantic execution"))?;
        if !issues.is_empty() {
            return Err(Error::Invalid(
                "bibliographic provenance v2 semantic consistency",
            ));
        }
        let mut agents = Vec::new();
        let mut seen = BTreeSet::new();
        for responsibility in array(&source, "responsibility")? {
            let agent = text(responsibility, "agent_ref")?;
            if seen.insert(agent) {
                agents.push(json!(agent));
            }
        }
        (&source["activity"], json!(agents))
    } else {
        (&source, source["agent_refs"].clone())
    };
    supplied_bibliographic_event(&source, &location, l.catalog.max_output_row_bytes)
}

/// Pure existing event projection from an independently validated event slot.
pub fn supplied_bibliographic_event(
    source: &Value,
    location: &Value,
    max_row_bytes: usize,
) -> Result<Value> {
    if max_row_bytes == 0 || max_row_bytes > 4 * 1024 * 1024 {
        return Err(Error::Budget("bibliographic event row cap"));
    }
    encode(source, max_row_bytes)?;
    encode(location, max_row_bytes)?;
    let id = text(source, "event_id")?;
    let (activity, agents) = if source["schema_version"] == "tos_provenance_event_v2" {
        let mut agents = Vec::new();
        let mut seen = BTreeSet::new();
        for responsibility in array(source, "responsibility")? {
            let agent = text(responsibility, "agent_ref")?;
            if seen.insert(agent) {
                agents.push(json!(agent));
            }
        }
        (&source["activity"], json!(agents))
    } else if source["schema_version"] == "tos_provenance_event_v1" {
        (source, source["agent_refs"].clone())
    } else {
        return Err(Error::Invalid("bibliographic provenance schema"));
    };
    let mut p = source
        .as_object()
        .ok_or(Error::Invalid("bibliographic provenance object"))?
        .clone();
    p.remove("inputs");
    p.remove("outputs");
    p.insert("event_ref".into(), source["event_id"].clone());
    p.insert("agent_refs".into(), agents);
    p.insert("source_event".into(), source.clone());
    for field in ["event_type", "started_at", "ended_at", "status"] {
        p.insert(field.into(), activity[field].clone());
    }
    Ok(
        json!({"node_id":node_id("provenance_event",id),"node_kind":"provenance_event","source_ref":location["source_ref"],"source_line":location["source_line"],"source_sha256":location["canonical_sha256"],"properties":p}),
    )
}
fn bounded(s: &str, cap: usize) -> String {
    if s.chars().count() <= cap {
        s.into()
    } else {
        s.chars().take(cap - 1).collect::<String>() + "…"
    }
}
fn basename(reference: &str) -> &str {
    reference.rsplit('/').next().unwrap_or(reference)
}
fn evidence_display(
    reference: &str,
    kind: &str,
    source_ref: &str,
    line: Option<u64>,
    label: Option<&str>,
    origin: &str,
) -> Value {
    let supplied = label.is_some_and(|s| !s.trim().is_empty() && s.chars().count() <= 240);
    let (title, origin) = if supplied {
        (label.unwrap().to_owned(), origin)
    } else if kind == "repo_path" {
        (
            bounded(basename(reference), 240),
            "repository-filename-fallback",
        )
    } else if kind == "external_citation" {
        (bounded(reference, 240), "citation-address-fallback")
    } else {
        (
            bounded(
                &format!(
                    "{} · {}{}",
                    kind.replace('_', " "),
                    basename(source_ref),
                    line.map(|n| format!(":{n}")).unwrap_or_default()
                ),
                240,
            ),
            "source-slot-fallback",
        )
    };
    let summary = if kind == "external_citation" {
        format!(
            "External citation; remote content not observed. Address declared by the citing Claim: {reference}."
        )
    } else {
        let mut name = kind.replace('_', " ");
        if let Some(first) = name.get_mut(..1) {
            first.make_ascii_uppercase()
        }
        format!(
            "{name} evidence reference: {reference}. Return to {source_ref}{}.",
            line.map(|n| format!(":{n}")).unwrap_or_default()
        )
    };
    json!({"title":{"default":title},"summary":{"default":bounded(&summary,1024)},"summary_state":"metadata-synthesis","provenance":{"title":origin,"summary":"evidence-reference-navigation","source_title_available":supplied,"source_summary_available":false,"human_form_authority":"none","source_ref":source_ref}})
}
/// Pure repo-path evidence projection. The caller verifies the public path bytes.
pub fn supplied_bibliographic_path_evidence(
    reference: &str,
    raw: &[u8],
    max_row_bytes: usize,
) -> Result<Value> {
    if !reference.starts_with("ToS/")
        || max_row_bytes == 0
        || max_row_bytes > 4 * 1024 * 1024
        || raw.len() > 16 * 1024 * 1024
    {
        return Err(Error::Budget("bibliographic path evidence bounds"));
    }
    let id = node_id("evidence", reference);
    let family = reference.rsplit_once('/').map(|v| v.0).unwrap_or("");
    let origin = match family {
        "ToS/review-ledger" => Some("catalogued-review-note-h1"),
        "ToS/research-packets/foundation-laboratory-2026-07" => Some("catalogued-research-lead-h1"),
        _ => None,
    };
    let title = if origin.is_some() && reference.ends_with(".md") && raw.len() <= 1_048_576 {
        let first = raw.split(|b| *b == b'\n').next().unwrap_or(&[]);
        let first = first.strip_suffix(b"\r").unwrap_or(first);
        std::str::from_utf8(first)
            .ok()
            .and_then(|s| s.strip_prefix("# "))
            .filter(|s| {
                !s.trim().is_empty()
                    && s.chars().count() <= 240
                    && !s
                        .chars()
                        .any(crate::source_bibliographic_unicode::is_category_c)
            })
    } else {
        None
    };
    return Ok(
        json!({"node_id":id,"node_kind":"evidence","source_ref":reference,"source_sha256":Digest256::of_bytes(&raw).to_hex(),"display":evidence_display(reference,"repo_path",reference,None,title,origin.unwrap_or("source-metadata-label")),"properties":{"evidence_ref":reference,"evidence_kind":"repo_path","resolved":true}}),
    );
}

/// Pure evidence rendering after the owner has resolved the exact source.
/// Variant selection is not a lookup or a source admission proof.
pub enum SuppliedEvidenceResolution<'a> {
    Anchor {
        anchor: &'a Value,
        location: &'a Value,
    },
    Identity(&'a Value),
    ProvenanceEvent(&'a Value),
    ExternalCitation,
}
pub fn supplied_bibliographic_evidence(
    reference: &str,
    claim: &Value,
    entry: &Value,
    resolution: SuppliedEvidenceResolution<'_>,
    max_row_bytes: usize,
) -> Result<Value> {
    if max_row_bytes == 0 || max_row_bytes > 4 * 1024 * 1024 {
        return Err(Error::Budget("bibliographic evidence row cap"));
    }
    encode(claim, max_row_bytes)?;
    encode(entry, max_row_bytes)?;
    match &resolution {
        SuppliedEvidenceResolution::Anchor { anchor, location } => {
            encode(anchor, max_row_bytes)?;
            encode(location, max_row_bytes)?;
        }
        SuppliedEvidenceResolution::Identity(identity) => {
            encode(identity, max_row_bytes)?;
        }
        SuppliedEvidenceResolution::ProvenanceEvent(location) => {
            encode(location, max_row_bytes)?;
        }
        SuppliedEvidenceResolution::ExternalCitation => {}
    }
    let result = render_bibliographic_evidence(reference, claim, entry, resolution, max_row_bytes)?;
    encode(&result, max_row_bytes)?;
    Ok(result)
}

fn render_bibliographic_evidence(
    reference: &str,
    claim: &Value,
    entry: &Value,
    resolution: SuppliedEvidenceResolution<'_>,
    max_row_bytes: usize,
) -> Result<Value> {
    let id = node_id("evidence", reference);
    match resolution {
        SuppliedEvidenceResolution::Anchor { anchor, location } => {
            let mut p = anchor
                .as_object()
                .ok_or(Error::Invalid("bibliographic anchor object"))?
                .clone();
            for (field, value) in [
                ("evidence_ref", json!(reference)),
                ("evidence_kind", json!("anchor")),
                ("resolved", json!(true)),
                ("anchor_status", anchor["status"].clone()),
                ("item_ref", anchor["item_id"].clone()),
                ("file_ref", anchor["file_id"].clone()),
                ("source_anchor", anchor.clone()),
            ] {
                p.insert(field.into(), value);
            }
            return Ok(
                json!({"node_id":id,"node_kind":"evidence","source_ref":location["source_ref"],"source_line":location["source_line"],"source_sha256":location["canonical_sha256"],"display":evidence_display(reference,"anchor",text(&location,"source_ref")?,location["source_line"].as_u64(),None,"source-metadata-label"),"properties":p}),
            );
        }
        SuppliedEvidenceResolution::Identity(identity) => {
            return Ok(
                json!({"node_id":id,"node_kind":"evidence","source_ref":identity["source_ref"],"source_sha256":identity["source_sha256"],"display":evidence_display(reference,"identity",text(&identity,"source_ref")?,None,identity.pointer("/properties/preferred_label").and_then(Value::as_str),"source-metadata-label"),"properties":{"evidence_ref":reference,"evidence_kind":"identity","resolved":true,"identity_node_id":identity["node_id"]}}),
            );
        }
        SuppliedEvidenceResolution::ProvenanceEvent(location) => {
            return Ok(
                json!({"node_id":id,"node_kind":"evidence","source_ref":location["source_ref"],"source_line":location["source_line"],"source_sha256":location["canonical_sha256"],"display":evidence_display(reference,"provenance_event",text(&location,"source_ref")?,location["source_line"].as_u64(),None,"source-metadata-label"),"properties":{"evidence_ref":reference,"evidence_kind":"provenance_event","resolved":true,"provenance_event_node_id":node_id("provenance_event",reference)}}),
            );
        }
        SuppliedEvidenceResolution::ExternalCitation => {}
    }
    // External addresses are preserved as citing-Claim declarations, never fetched.
    let address = reference;
    crate::source_bibliographic_unicode::validate_external_citation_address(address)?;
    let occurrence =
        String::from_utf8(encode(&json!([claim["claim_id"], address]), max_row_bytes)?)
            .map_err(|_| Error::Invalid("bibliographic citation occurrence"))?;
    Ok(
        json!({"node_id":node_id("evidence",&occurrence),"node_kind":"evidence","source_ref":entry["source_claim_file_ref"],"source_line":entry["source_claim_line"],"source_sha256":entry["claim_sha256"],"display":evidence_display(address,"external_citation",text(entry,"source_claim_file_ref")?,None,None,"source-metadata-label"),"properties":{"evidence_ref":address,"evidence_kind":"external_citation","citing_claim_ref":claim["claim_id"],"citation_status":"tracked_claim","resolved":false,"remote_content_sha256":null,"observation_posture":"address_only_not_observed","source_hash_scope":"citing_claim_declaration_not_remote_content"}}),
    )
}

fn evidence(
    stage: &mut KnowledgeStage<'_>,
    reference: &str,
    claim: &Value,
    entry: &Value,
    validator: &SourceCatalogValidator<'_>,
    materializer: &mut dyn BibliographicForms,
    l: BibliographicLimits,
) -> Result<Value> {
    if reference.starts_with("ToS/") {
        let raw = raw_file(stage, reference, l)?
            .ok_or(Error::Invalid("bibliographic evidence file missing"))?;
        return supplied_bibliographic_path_evidence(
            reference,
            &raw,
            l.catalog.max_output_row_bytes,
        );
    }
    if let Some((anchor, location)) = slot(stage, "anchor", reference, l)? {
        return render_bibliographic_evidence(
            reference,
            claim,
            entry,
            SuppliedEvidenceResolution::Anchor {
                anchor: &anchor,
                location: &location,
            },
            l.catalog.max_output_row_bytes,
        );
    }
    if let Some(identity) = identity(stage, reference, validator, materializer, l)? {
        return render_bibliographic_evidence(
            reference,
            claim,
            entry,
            SuppliedEvidenceResolution::Identity(&identity),
            l.catalog.max_output_row_bytes,
        );
    }
    if let Some((_, location)) = slot(stage, "provenance_event", reference, l)? {
        return render_bibliographic_evidence(
            reference,
            claim,
            entry,
            SuppliedEvidenceResolution::ProvenanceEvent(&location),
            l.catalog.max_output_row_bytes,
        );
    }
    render_bibliographic_evidence(
        reference,
        claim,
        entry,
        SuppliedEvidenceResolution::ExternalCitation,
        l.catalog.max_output_row_bytes,
    )
}

fn maker<B: catalog::CatalogInputBinding>(
    stage: &mut KnowledgeStage<'_>,
    claim: &Value,
    receipt: &SourceCatalogReceipt<B>,
    validator: &SourceCatalogValidator<'_>,
    materializer: &mut dyn BibliographicForms,
    l: BibliographicLimits,
) -> Result<(Value, Option<Value>)> {
    let maker = &claim["maker"];
    let reference = text(maker, "agent_ref")?;
    let identity = identity(stage, reference, validator, materializer, l)?;
    let digest = if identity.is_none() {
        receipt
            .file_sha256
            .get(CLAIM_CATALOG)
            .ok_or(Error::Invalid("bibliographic Claim catalog digest missing"))?
            .as_str()
    } else {
        ""
    };
    supplied_bibliographic_maker(claim, identity, digest, l.catalog.max_output_row_bytes)
}

/// Pure maker carrier using its verified identity association or immutable
/// predecessor Claim-catalog fallback, never a fabricated metadata identity.
pub fn supplied_bibliographic_maker(
    claim: &Value,
    identity: Option<Value>,
    claim_catalog_sha256: &str,
    max_row_bytes: usize,
) -> Result<(Value, Option<Value>)> {
    if max_row_bytes == 0 || max_row_bytes > 4 * 1024 * 1024 {
        return Err(Error::Budget("bibliographic maker row cap"));
    }
    encode(claim, max_row_bytes)?;
    if let Some(value) = &identity {
        encode(value, max_row_bytes)?;
    }
    let maker = &claim["maker"];
    let reference = text(maker, "agent_ref")?;
    let (source_ref, sha) = if let Some(node) = &identity {
        (node["source_ref"].clone(), node["source_sha256"].clone())
    } else {
        (json!(CLAIM_CATALOG), json!(claim_catalog_sha256))
    };
    Ok((
        json!({"node_id":node_id("maker",reference),"node_kind":"maker","source_ref":source_ref,"source_sha256":sha,
        "properties":{"agent_ref":reference,"maker_type":maker["maker_type"],"source_maker":maker,"identity_node_id":identity.as_ref().map(|n|n["node_id"].clone()).unwrap_or(Value::Null)}}),
        identity,
    ))
}
fn typed_endpoint(node: &Value, allowed: &Value, entities: &Value) -> Result<bool> {
    let kind = node
        .pointer("/properties/identity_kind")
        .and_then(Value::as_str)
        .ok_or(Error::Invalid("bibliographic endpoint kind"))?;
    let entries = array(entities, "types")?;
    let mut types = BTreeMap::new();
    let mut mapped = None;
    for entry in entries {
        let id = text(entry, "type_id")?;
        if types.insert(id, entry).is_some() {
            return Err(Error::Invalid("bibliographic duplicate type"));
        }
        for mapping in array(entry, "source_mappings")? {
            if mapping["source_graph"] == "source-claims" && mapping["source_kind_id"] == kind {
                if mapped.replace(id).is_some() {
                    return Err(Error::Invalid(
                        "bibliographic duplicate source type mapping",
                    ));
                }
            }
        }
    }
    let mut pending = vec![mapped.ok_or(Error::Invalid("bibliographic unmapped endpoint kind"))?];
    let mut visited = BTreeSet::new();
    while let Some(next) = pending.pop() {
        if !visited.insert(next) {
            continue;
        }
        if visited.len() > 4096 {
            return Err(Error::Budget("bibliographic endpoint ancestry"));
        }
        let entry = types
            .get(next)
            .ok_or(Error::Invalid("bibliographic type parent absent"))?;
        for parent in array(entry, "parent_type_ids")? {
            pending.push(
                parent
                    .as_str()
                    .ok_or(Error::Invalid("bibliographic type parent"))?,
            )
        }
    }
    Ok(allowed
        .as_array()
        .ok_or(Error::Invalid("bibliographic endpoint allowed types"))?
        .iter()
        .any(|v| v.as_str().is_some_and(|id| visited.contains(id))))
}
fn relation<'a>(registry: &'a Value, predicate: &str) -> Result<&'a Value> {
    let candidates = array(registry, "relations")?
        .iter()
        .filter(|r| {
            r["source_mappings"].as_array().is_some_and(|m| {
                m.iter().any(|m| {
                    m["source_graph"] == "source-claims"
                        && m["scope"] == "claim-predicate"
                        && m["source_predicate_id"] == predicate
                })
            })
        })
        .collect::<Vec<_>>();
    if candidates.len() != 1 {
        return Err(Error::Invalid("bibliographic relation mapping closure"));
    }
    Ok(candidates[0])
}
/// Same exact catalogue field attribution calculation as the full producer.
pub fn validate_supplied_catalogue_attribution(claim: &Value) -> Result<()> {
    let predicate = text(claim, "predicate")?;
    if matches!(
        predicate,
        "document_catalogue_date" | "document_catalogue_origin" | "document_catalogue_destination"
    ) {
        let expected = match predicate {
            "document_catalogue_date" => "assigned-date",
            "document_catalogue_origin" => "origin",
            _ => "destination",
        };
        let attribution = &claim["qualifiers"]["catalogue_attribution"];
        if attribution["field_role"] != expected
            || !array(&claim, "evidence_refs")?.contains(&attribution["evidence_ref"])
            || predicate == "document_catalogue_date"
                && attribution["source_wording"] != claim["object"]["source_wording"]
        {
            return Err(Error::Invalid(
                "bibliographic Document catalogue field attribution",
            ));
        }
    }
    Ok(())
}

fn claim_cohort<B: catalog::CatalogInputBinding>(
    stage: &mut KnowledgeStage<'_>,
    id: &str,
    receipt: &SourceCatalogReceipt<B>,
    validator: &SourceCatalogValidator<'_>,
    entities: &Value,
    registry: &Value,
    materializer: &mut dyn BibliographicForms,
    l: BibliographicLimits,
    versions: Option<&mut crate::source_bibliographic_versions::Versions<'_, '_>>,
) -> Result<render::Cohort> {
    let row = catalog::catalog_row(stage, "claims", id, l.catalog)?
        .ok_or(Error::Invalid("bibliographic retained Claim row missing"))?;
    let entry = &row["entry"];
    let (claim, location) = slot(stage, "claim", id, l)?
        .ok_or(Error::Invalid("bibliographic source Claim slot missing"))?;
    if entry["claim_sha256"] != location["canonical_sha256"]
        || entry["source_claim_file_ref"] != location["source_ref"]
        || entry["source_claim_line"] != location["source_line"]
        || claim["claim_id"] != id
    {
        return Err(Error::Invalid("bibliographic Claim entry/slot binding"));
    }
    let source_ref = text(entry, "source_claim_file_ref")?;
    let predicate = text(&claim, "predicate")?;
    if !matches!(
        claim["claim_type"].as_str(),
        Some("bibliographic" | "relation")
    ) {
        return Err(Error::Invalid("bibliographic Claim type"));
    }
    let profiled = basename(source_ref) == "source-claims.jsonl";
    let historical = matches!(
        predicate,
        "historical_participant" | "historical_place" | "historical_work" | "historical_dating"
    );
    let legacy_link =
        source_ref == "ToS/source-witnesses/relations/object-link/object-link-claims.jsonl";
    if !profiled
        && !legacy_link
        && claim["claim_type"] == "relation"
        && predicate != "is_derivative_of"
        && !historical
    {
        return Err(Error::Invalid(
            "bibliographic legacy relation outside owner profile",
        ));
    }
    if !profiled
        && !legacy_link
        && !matches!(
            claim["assertion_layer"].as_str(),
            Some("bibliographic_assertion" | "scholarly_report")
        )
    {
        return Err(Error::Invalid("bibliographic legacy assertion layer"));
    }
    let bound_forms = if matches!(
        basename(source_ref),
        "source-claims.jsonl" | "historical-claims.jsonl"
    ) {
        let forms_ref = format!(
            "{}.{}.human-forms.json",
            source_ref
                .strip_suffix(".jsonl")
                .ok_or(Error::Invalid("bibliographic Claim stream extension"))?,
            Digest256::of_bytes(id.as_bytes()).to_hex()
        );
        forms(stage, &forms_ref, &claim, validator, materializer, l)?
    } else {
        None
    };
    let subject = identity(
        stage,
        text(&claim, "subject_ref")?,
        validator,
        materializer,
        l,
    )?
    .ok_or(Error::Invalid("bibliographic subject identity missing"))?;
    let object = if let Some(reference) = claim["object"].as_str() {
        if let Some(node) = identity(stage, reference, validator, materializer, l)? {
            node
        } else if reference.starts_with("tos.") {
            return Err(Error::Invalid(
                "bibliographic identity-like object unresolved",
            ));
        } else {
            render::literal(entry, &claim, l.catalog.max_output_row_bytes)?
        }
    } else {
        render::literal(entry, &claim, l.catalog.max_output_row_bytes)?
    };
    let profile = if profiled {
        relation(registry, predicate)?.get("source_claim_profile")
    } else {
        None
    };
    let transition = profile
        .and_then(|p| p.get("reader"))
        .and_then(Value::as_str)
        .is_some_and(|r| matches!(r, "identity-transition-v1" | "identity-transition-v2"));
    if (profiled || historical) && !transition {
        let relation = relation(registry, predicate)?;
        if !typed_endpoint(&subject, &relation["domain_type_ids"], entities)? {
            return Err(Error::Invalid("bibliographic subject domain violation"));
        }
        if object["node_kind"] == "identity"
            && !typed_endpoint(&object, &relation["range_type_ids"], entities)?
        {
            return Err(Error::Invalid("bibliographic object range violation"));
        }
    }
    if claim
        .pointer("/qualifiers/display_fields/schema_version")
        .and_then(Value::as_str)
        == Some("tos_claim_display_fields_v1")
    {
        catalog::check_catalog_schema(
            stage,
            validator,
            l.catalog,
            "ToS/contracts/claim-display-fields.schema.json",
            "",
            &encode(&claim["qualifiers"], l.catalog.max_row_bytes)?,
        )?;
    }
    let legacy_context = if legacy_link {
        if claim["schema_version"] != "tos_object_link_claim_v1"
            || !matches!(
                predicate,
                "described_by" | "metadata_at" | "downloadable_at" | "rights_statement_at"
            )
            || !matches!(
                subject
                    .pointer("/properties/identity_kind")
                    .and_then(Value::as_str),
                Some("work" | "expression" | "edition" | "collection" | "item")
            )
            || object
                .pointer("/properties/identity_kind")
                .and_then(Value::as_str)
                != Some("link")
        {
            return Err(Error::Invalid(
                "bibliographic retained object-Link exact domains",
            ));
        }
        Some(
            json!({"source_claim":claim,"source_claim_file_ref":source_ref,"source_claim_line":entry["source_claim_line"],"source_sha256":entry["claim_sha256"],"source_schema_ref":"ToS/contracts/object-link-claim.schema.json","source_adapter":"retained-object-link-v1"}),
        )
    } else {
        None
    };
    let mut collection_order_basis = None;
    let mut members = Vec::new();
    if let Some(profile) = profile {
        if profile
            .pointer("/object_reference_set/basis_adapter")
            .and_then(Value::as_str)
            == Some("collection-membership-versions-v1")
        {
            collection_order_basis = Some(versions.ok_or(Error::Invalid(
                "bibliographic exact-version Collection order requires independently selected source cut",
            ))?.ground(stage, &claim, validator, entities, materializer, l)?);
        }
        for reference in crate::source_bibliographic_values::members(&claim, profile)? {
            let node = identity(stage, reference, validator, materializer, l)?
                .ok_or(Error::Invalid("bibliographic value member identity absent"))?;
            if !transition
                && !typed_endpoint(
                    &node,
                    &profile["object_reference_set"]["member_type_ids"],
                    entities,
                )?
            {
                return Err(Error::Invalid("bibliographic value member domain"));
            }
            if transition {
                let kind = node
                    .pointer("/properties/identity_kind")
                    .and_then(Value::as_str)
                    .ok_or(Error::Invalid("bibliographic proposal participant kind"))?;
                let mut eligible = false;
                for entity in array(entities, "types")? {
                    if entity["abstract"] == false
                        && entity["source_mappings"].as_array().is_some_and(|m| {
                            m.iter().any(|m| {
                                m["source_graph"] == "source-claims" && m["source_kind_id"] == kind
                            })
                        })
                    {
                        eligible = entity["object_role"] == "identity"
                            || profile["reader"] == "identity-transition-v2"
                                && entity["object_role"] == "semantic"
                                && entity
                                    .pointer("/source_record_profile/identity_proposal_adapter")
                                    .and_then(Value::as_str)
                                    == Some("exact-semantic-metadata-v1");
                    }
                }
                if !eligible {
                    return Err(Error::Invalid(
                        "bibliographic proposal participant concrete source role",
                    ));
                }
            }
            if members.len() >= l.max_claim_cohort_rows {
                return Err(Error::Budget("bibliographic value members"));
            }
            members.push(node);
        }
    }
    validate_supplied_catalogue_attribution(&claim)?;
    let event = event(stage, text(&claim, "provenance_event_ref")?, validator, l)?;
    let (maker, maker_identity) = maker(stage, &claim, receipt, validator, materializer, l)?;
    let mut evidence_nodes = Vec::new();
    let mut counter_nodes = Vec::new();
    for (field, resolved) in [
        ("evidence_refs", &mut evidence_nodes),
        ("counterevidence_refs", &mut counter_nodes),
    ] {
        for reference in claim
            .get(field)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if resolved.len() >= l.max_claim_cohort_rows {
                return Err(Error::Budget("bibliographic Claim evidence count"));
            }
            resolved.push(evidence(
                stage,
                reference
                    .as_str()
                    .ok_or(Error::Invalid("bibliographic evidence string"))?,
                &claim,
                entry,
                validator,
                materializer,
                l,
            )?);
        }
    }
    let mut normalized = Vec::new();
    if predicate == "provision_activity" {
        let mut refs = BTreeSet::new();
        for (field, key, edge, allowed) in [
            (
                "places",
                "normalized_place_ref",
                "has_normalized_place",
                &["place"][..],
            ),
            (
                "agents",
                "normalized_agent_ref",
                "has_normalized_agent",
                &["agent", "organization"][..],
            ),
        ] {
            for value in claim["object"][field].as_array().into_iter().flatten() {
                if let Some(reference) = value.get(key).filter(|v| !v.is_null()) {
                    let reference = reference.as_str().ok_or(Error::Invalid(
                        "bibliographic normalized provision reference",
                    ))?;
                    let node = identity(stage, reference, validator, materializer, l)?
                        .ok_or(Error::Invalid("bibliographic provision identity absent"))?;
                    if !allowed.contains(
                        &node
                            .pointer("/properties/identity_kind")
                            .and_then(Value::as_str)
                            .unwrap_or(""),
                    ) {
                        return Err(Error::Invalid("bibliographic provision identity kind"));
                    }
                    refs.insert((edge.to_owned(), reference.to_owned()));
                }
            }
        }
        for (edge, reference) in refs {
            normalized.push((
                edge,
                identity(stage, &reference, validator, materializer, l)?
                    .ok_or(Error::Invalid("bibliographic provision identity absent"))?,
            ));
        }
    }
    if (predicate == "historical_dating"
        || profiled
            && relation(registry, predicate)?
                .pointer("/source_claim_profile/reader")
                .and_then(Value::as_str)
                .is_some_and(|r| {
                    matches!(
                        r,
                        "historical-temporal-v1" | "document-catalogue-temporal-v1"
                    )
                }))
        && claim["object"]
            .get("relative")
            .is_some_and(|v| !v.is_null())
    {
        let reference = claim
            .pointer("/object/relative/anchor_ref")
            .and_then(Value::as_str)
            .ok_or(Error::Invalid("bibliographic relative date anchor"))?;
        let node = identity(stage, reference, validator, materializer, l)?.ok_or(
            Error::Invalid("bibliographic relative date identity missing"),
        )?;
        if !typed_endpoint(&node, &json!(["tos.entity.historical-situation"]), entities)? {
            return Err(Error::Invalid("bibliographic relative date anchor domain"));
        }
        normalized.push(("has_historical_date_anchor".into(), node));
    }
    for reference in claim
        .get("alternative_claim_refs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .chain(claim.get("supersedes_claim_ref").filter(|v| !v.is_null()))
    {
        if catalog::catalog_row(
            stage,
            "claims",
            reference
                .as_str()
                .ok_or(Error::Invalid("bibliographic alternative identity"))?,
            l.catalog,
        )?
        .is_none()
        {
            return Err(Error::Invalid(
                "bibliographic alternative outside exact Claim cut",
            ));
        }
    }
    let descriptor = render::descriptor(
        &claim,
        &subject,
        &object,
        registry,
        entities,
        l.catalog.into(),
    )?;
    let result = render_supplied_claim(
        render::ClaimInputs {
            entry,
            claim: &claim,
            subject,
            object,
            event,
            maker,
            maker_identity,
            evidence: evidence_nodes,
            counterevidence: counter_nodes,
            members,
            normalized,
            descriptor,
            forms: bound_forms,
            collection_order_basis,
            legacy_context,
        },
        l,
    )?;
    if result.nodes.len() + result.edges.len() + 1 > l.max_claim_cohort_rows {
        return Err(Error::Budget("bibliographic Claim cohort rows"));
    }
    Ok(result)
}
fn insert(
    stage: &mut KnowledgeStage<'_>,
    collection: &str,
    id: &str,
    value: &Value,
    l: BibliographicLimits,
) -> Result<()> {
    l.validate()?;
    let raw = encode(value, l.catalog.max_output_row_bytes)?;
    stage.with_connection(WritePhase::Catalog,|db|{
        let previous=db.query_row("SELECT CASE WHEN length(payload)<=?3 AND payload_len=length(payload) THEN payload ELSE NULL END FROM source_bibliographic_rows WHERE collection=?1 AND id=?2",params![collection,id,l.catalog.max_output_row_bytes as i64],|r|r.get::<_,Option<Vec<u8>>>(0)).optional()?;
        if let Some(previous)=previous {if previous.as_deref()!=Some(raw.as_slice()){return Err(Error::Invalid("bibliographic conflicting retained node"))}return Ok(())}
        let (count,bytes)=db.query_row("SELECT row_count,byte_count FROM source_bibliographic_totals WHERE singleton=1",[],|r|Ok((r.get::<_,u64>(0)?,r.get::<_,u64>(1)?)))?;
        if count>=l.max_output_rows||bytes.checked_add(raw.len() as u64).is_none_or(|n|n>l.max_output_bytes){return Err(Error::Budget("bibliographic retained output"))}
        db.execute("UPDATE source_bibliographic_totals SET row_count=row_count+1,byte_count=byte_count+?1 WHERE singleton=1",[raw.len() as i64])?;
        db.execute("INSERT INTO source_bibliographic_rows VALUES(?1,?2,?3,?4,?5)",params![collection,id,raw.len() as i64,Digest256::of_bytes(&raw).as_bytes().as_slice(),raw])?;Ok(())
    })
}
fn root(stage: &mut KnowledgeStage<'_>, l: BibliographicLimits) -> Result<(u64, u64, u64, String)> {
    stage.with_connection(WritePhase::Catalog,|db|{
        let mut statement=db.prepare("SELECT collection,id,CASE WHEN length(payload)<=?1 AND payload_len=length(payload) THEN payload ELSE NULL END,payload_sha256 FROM source_bibliographic_rows ORDER BY collection,id")?;
        let mut rows=statement.query([l.catalog.max_output_row_bytes as i64])?;let mut hash=Digest256Hasher::new();let mut counts=[0u64;3];let mut bytes=0u64;
        while let Some(row)=rows.next()?{
            let collection:String=row.get(0)?;let id:String=row.get(1)?;let raw:Option<Vec<u8>>=row.get(2)?;let sha:Vec<u8>=row.get(3)?;
            let raw=raw.ok_or(Error::Budget("bibliographic retained output row"))?;
            if id.len()>8192||Digest256::of_bytes(&raw).as_bytes().as_slice()!=sha{return Err(Error::Invalid("bibliographic retained output digest"))}
            bytes=bytes.checked_add(raw.len() as u64).filter(|n|*n<=l.max_output_bytes).ok_or(Error::Budget("bibliographic retained total bytes"))?;
            let index=match collection.as_str(){"nodes"=>0,"edges"=>1,"claim_traces"=>2,_=>return Err(Error::Invalid("bibliographic retained collection"))};counts[index]+=1;
            if counts.iter().sum::<u64>()>l.max_output_rows{return Err(Error::Budget("bibliographic retained output rows"))}
            hash.update(collection.as_bytes());hash.update(b"\0");hash.update(id.as_bytes());hash.update(b"\0");hash.update(&sha);hash.update(b"\n");
        }Ok((counts[0],counts[1],counts[2],hash.finalize().to_hex()))
    })
}
fn summary(receipt: &BibliographicReceipt, l: BibliographicLimits) -> Result<String> {
    digest(
        &json!({"nodes":receipt.node_count,"edges":receipt.edge_count,"claims":receipt.claim_count,"root":receipt.row_root_sha256,"catalog":receipt.source_catalog_root_sha256,"closure":receipt.source_reference_closure_verified,"selected_version_source":receipt.selected_version_source}),
        l.catalog.max_output_row_bytes,
    )
}

/// Prepare sorted raw bibliographic rows from the verified catalog/source slots.
/// Schema and explicit endpoint closure run before any candidate is returned.
pub fn prepare_bibliographic_graph(
    stage: &mut KnowledgeStage<'_>,
    catalog_receipt: &SourceCatalogReceipt,
    validator: &SourceCatalogValidator<'_>,
    materializer: &mut dyn BibliographicForms,
    l: BibliographicLimits,
) -> Result<BibliographicReceipt> {
    prepare_impl(stage, catalog_receipt, validator, materializer, l, None)
}
/// Resolve exact-version Collection order from one independently selected
/// immutable source cut. The cut proves current membership/absence; it grants
/// no permission to use or publish retained historical bytes.
pub fn prepare_bibliographic_graph_from_cut(
    stage: &mut KnowledgeStage<'_>,
    catalog_receipt: &SourceCatalogReceipt,
    validator: &SourceCatalogValidator<'_>,
    materializer: &mut dyn BibliographicForms,
    l: BibliographicLimits,
    source: &crate::source_bibliographic_versions::BibliographicSourceCut<'_>,
) -> Result<BibliographicReceipt> {
    prepare_impl(
        stage,
        catalog_receipt,
        validator,
        materializer,
        l,
        Some(source),
    )
}
pub fn prepare_cold_bibliographic_graph_from_cut(
    stage: &mut KnowledgeStage<'_>,
    catalog_receipt: &catalog::ColdSourceCatalogReceipt,
    validator: &SourceCatalogValidator<'_>,
    materializer: &mut dyn BibliographicForms,
    l: BibliographicLimits,
    source: &crate::source_bibliographic_versions::BibliographicSourceCut<'_>,
) -> Result<BibliographicReceipt> {
    prepare_impl(
        stage,
        catalog_receipt,
        validator,
        materializer,
        l,
        Some(source),
    )
}
fn prepare_impl<B: catalog::CatalogInputBinding>(
    stage: &mut KnowledgeStage<'_>,
    catalog_receipt: &SourceCatalogReceipt<B>,
    validator: &SourceCatalogValidator<'_>,
    materializer: &mut dyn BibliographicForms,
    l: BibliographicLimits,
    source: Option<&crate::source_bibliographic_versions::BibliographicSourceCut<'_>>,
) -> Result<BibliographicReceipt> {
    let result = (|| {
        l.validate()?;
        catalog::verify_catalog(stage, catalog_receipt, l.catalog)?;
        if !stage
            .input_collections()
            .iter()
            .any(|c| c.source_graph == CATALOG_SOURCE && c.collection == BIBLIOGRAPHIC_FILES)
        {
            return Err(Error::Invalid(
                "bibliographic exact dependency collection required",
            ));
        }
        let mut versions = source
            .map(|source| {
                crate::source_bibliographic_versions::Versions::new(
                    source,
                    stage,
                    validator,
                    catalog_receipt,
                    l,
                )
            })
            .transpose()?;
        let entities = json_file(stage, ENTITY, l)?;
        let registry = json_file(stage, RELATION, l)?;
        stage.with_connection(WritePhase::Schema,|db|{db.execute_batch("CREATE TABLE source_bibliographic_rows(collection TEXT NOT NULL,id TEXT NOT NULL,payload_len INTEGER NOT NULL,payload_sha256 BLOB NOT NULL,payload BLOB NOT NULL,PRIMARY KEY(collection,id)) WITHOUT ROWID;
CREATE TABLE source_bibliographic_totals(singleton INTEGER PRIMARY KEY CHECK(singleton=1),row_count INTEGER NOT NULL,byte_count INTEGER NOT NULL);
INSERT INTO source_bibliographic_totals VALUES(1,0,0);")?;Ok(())})?;
        let mut after = None;
        while let Some(id) = catalog::catalog_next(stage, "records", after.as_deref())? {
            let node = identity(stage, &id, validator, materializer, l)?
                .ok_or(Error::Invalid("bibliographic catalog identity disappeared"))?;
            catalog::check_catalog_schema(
                stage,
                validator,
                l.catalog,
                SCHEMA,
                "#/$defs/node",
                &encode(&node, l.catalog.max_output_row_bytes)?,
            )?;
            insert(stage, "nodes", text(&node, "node_id")?, &node, l)?;
            after = Some(id);
        }
        after = None;
        while let Some(id) = catalog::catalog_next(stage, "claims", after.as_deref())? {
            let cohort = claim_cohort(
                stage,
                &id,
                catalog_receipt,
                validator,
                &entities,
                &registry,
                materializer,
                l,
                versions.as_mut(),
            )?;
            for (collection, definition, field, rows) in [
                ("nodes", "node", "node_id", cohort.nodes),
                ("edges", "edge", "edge_id", cohort.edges),
                (
                    "claim_traces",
                    "claimTrace",
                    "claim_ref",
                    vec![cohort.trace],
                ),
            ] {
                for value in rows {
                    catalog::check_catalog_schema(
                        stage,
                        validator,
                        l.catalog,
                        SCHEMA,
                        &format!("#/$defs/{definition}"),
                        &encode(&value, l.catalog.max_output_row_bytes)?,
                    )?;
                    insert(stage, collection, text(&value, field)?, &value, l)?;
                }
            }
            after = Some(id);
        }
        // Every edge endpoint, including alternatives, closes over emitted nodes.
        stage.with_connection(WritePhase::Catalog,|db|{
            let mut statement=db.prepare("SELECT CASE WHEN length(payload)<=?1 AND payload_len=length(payload) THEN payload ELSE NULL END FROM source_bibliographic_rows WHERE collection='edges' ORDER BY id")?;
            let mut rows=statement.query([l.catalog.max_output_row_bytes as i64])?;
            while let Some(row)=rows.next()?{
                let raw:Vec<u8>=row.get(0)?;if raw.len()>l.catalog.max_output_row_bytes{return Err(Error::Budget("bibliographic edge closure bytes"))}
                let v=SourceRow::parse(&raw,l.catalog.max_output_row_bytes)?;
                for field in ["from_id","to_id"] {
                    let exists:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM source_bibliographic_rows WHERE collection='nodes' AND id=?1)",[text(v.value(),field)?],|r|r.get(0))?;
                    if !exists{return Err(Error::Invalid("bibliographic edge endpoint outside exact cut"))}
                }
            }Ok(())
        })?;
        catalog::verify_catalog(stage, catalog_receipt, l.catalog)?;
        let (node_count, edge_count, claim_count, row_root_sha256) = root(stage, l)?;
        if claim_count != catalog_receipt.claim_count {
            return Err(Error::Invalid("bibliographic all Claims consumed"));
        }
        let mut receipt = BibliographicReceipt {
            node_count,
            edge_count,
            claim_count,
            row_root_sha256,
            source_catalog_root_sha256: catalog_receipt.row_root_sha256.clone(),
            source_reference_closure_verified: true,
            selected_version_source: versions.as_ref().map(|versions| versions.binding()),
            summary_sha256: String::new(),
        };
        receipt.summary_sha256 = summary(&receipt, l)?;
        Ok(receipt)
    })();
    if result.is_err() {
        stage.poison()
    }
    result
}
/// Stream a privately sealed raw candidate in exact collection/ID order.
pub fn render_bibliographic_graph(
    stage: &mut KnowledgeStage<'_>,
    catalog_receipt: &SourceCatalogReceipt,
    receipt: &BibliographicReceipt,
    l: BibliographicLimits,
    sink: &mut impl BibliographicSink,
) -> Result<()> {
    render_impl(stage, catalog_receipt, receipt, l, sink)
}
pub fn render_cold_bibliographic_graph(
    stage: &mut KnowledgeStage<'_>,
    catalog_receipt: &catalog::ColdSourceCatalogReceipt,
    receipt: &BibliographicReceipt,
    l: BibliographicLimits,
    sink: &mut impl BibliographicSink,
) -> Result<()> {
    render_impl(stage, catalog_receipt, receipt, l, sink)
}
fn render_impl<B: catalog::CatalogInputBinding>(
    stage: &mut KnowledgeStage<'_>,
    catalog_receipt: &SourceCatalogReceipt<B>,
    receipt: &BibliographicReceipt,
    l: BibliographicLimits,
    sink: &mut impl BibliographicSink,
) -> Result<()> {
    let result = (|| {
        l.validate()?;
        catalog::verify_catalog(stage, catalog_receipt, l.catalog)?;
        if receipt.source_catalog_root_sha256 != catalog_receipt.row_root_sha256
            || summary(receipt, l)? != receipt.summary_sha256
            || root(stage, l)?
                != (
                    receipt.node_count,
                    receipt.edge_count,
                    receipt.claim_count,
                    receipt.row_root_sha256.clone(),
                )
        {
            return Err(Error::Invalid(
                "bibliographic candidate exact seal mismatch",
            ));
        }
        stage.with_connection(WritePhase::Catalog,|db|{
            let mut statement=db.prepare("SELECT collection,id,payload FROM source_bibliographic_rows ORDER BY collection,id")?;let mut rows=statement.query([])?;
            while let Some(row)=rows.next()?{let collection:String=row.get(0)?;let id:String=row.get(1)?;let raw:Vec<u8>=row.get(2)?;sink.row(&collection,&id,&raw)?;}Ok(())
        })
    })();
    if result.is_err() {
        stage.poison()
    }
    result
}

pub fn clear_bibliographic_graph(
    stage: &mut KnowledgeStage<'_>,
    catalog_receipt: &SourceCatalogReceipt,
    receipt: &BibliographicReceipt,
    l: BibliographicLimits,
) -> Result<()> {
    let result = (|| {
        l.validate()?;
        catalog::verify_catalog(stage, catalog_receipt, l.catalog)?;
        if summary(receipt, l)? != receipt.summary_sha256
            || receipt.source_catalog_root_sha256 != catalog_receipt.row_root_sha256
            || root(stage, l)?
                != (
                    receipt.node_count,
                    receipt.edge_count,
                    receipt.claim_count,
                    receipt.row_root_sha256.clone(),
                )
        {
            return Err(Error::Invalid("bibliographic cleanup exact seal"));
        }
        stage.with_connection(WritePhase::Finalize, |db| {
            db.execute_batch(
                "DROP TABLE source_bibliographic_rows;DROP TABLE source_bibliographic_totals;",
            )?;
            Ok(())
        })
    })();
    if result.is_err() {
        stage.poison()
    }
    result
}

#[cfg(test)]
mod descriptor_limit_tests {
    use super::*;

    #[test]
    fn literal_bounds_source_separately_from_emitted_row() {
        let claim = json!({"claim_id":"claim:test","object":"small","retained_extension":"x".repeat(40_000)});
        let entry = json!({});
        let render = |input_document_bytes, output_row_bytes| {
            supplied_bibliographic_literal(
                &entry,
                &claim,
                BibliographicDocumentLimits {
                    input_document_bytes,
                    output_row_bytes,
                },
            )
        };
        assert_eq!(
            render(1_048_576, 4096).unwrap()["properties"]["value"],
            "small"
        );
        assert!(render(4096, 4096).is_err());
        assert!(render(1_048_576, 16).is_err());
    }

    #[test]
    fn descriptor_uses_caller_input_and_output_bounds_independently() {
        let claim = json!({"claim_id":"claim:test","claim_version":1,"predicate":"unmapped"});
        let registry = json!({"claim_navigation_template":{"template_id":"test","template_version":1},"relations":[],"retained_extension":"x".repeat(40_000)});
        let empty = json!({});
        let render = |input_document_bytes, output_row_bytes| {
            supplied_claim_navigation_descriptor(
                &claim,
                &empty,
                &empty,
                &registry,
                &empty,
                BibliographicDocumentLimits {
                    input_document_bytes,
                    output_row_bytes,
                },
            )
        };
        let descriptor = render(8 * 1024 * 1024, 4096).unwrap().unwrap();
        assert_eq!(descriptor["reason"], "predicate-not-understood");
        assert!(render(4096, 4096).is_err());
        assert!(render(8 * 1024 * 1024, 16).is_err());
        assert!(render(0, 4096).is_err());
    }
}
