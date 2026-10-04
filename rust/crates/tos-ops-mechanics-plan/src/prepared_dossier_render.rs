//! Draft for `tos-ops-mechanics-plan::prepared_dossier_planting`.
//! Deterministic prepared-dossier planting projection mechanics.
//!
//! This module is a pure compiler-side renderer. It consumes the exact bytes
//! and source baselines already observed by the caller's single bounded
//! ResearchExecution; it neither opens paths nor writes files. The OPS adapter
//! must call the all-package readiness gate before parsing/extraction, pass the
//! resulting parsed dossiers and their matching source preimages here, then
//! publish the complete path/bytes plan through its guarded output role.
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use tos_compiler::source_philosophy_dossier_extract::PreparedDossier;
use tos_compiler::source_philosophy_multilingual::Multilingual;
use tos_foundation::{Digest256, python_casefold_unicode16_v1};

pub const ROUTE_MAP_REF: &str = "ToS/philosophy/atlas/dossiers/prepared-dossier-routes.json";
pub const ATLAS_MANIFEST_REF: &str = "ToS/philosophy/atlas/atlas.manifest.json";
pub const DOSSIER_BRANCH_REF: &str = "ToS/philosophy/atlas/dossiers/branch.manifest.json";
pub const PHILOSOPHY_MANIFEST_REF: &str = "ToS/philosophy/philosophy.manifest.json";
pub const DOSSIER_INDEX_REF: &str = "ToS/philosophy/atlas/dossiers/index.jsonl";
pub const DOSSIER_SUMMARY_REF: &str = "ToS/philosophy/atlas/dossiers/graph-shape-summary.json";
pub const SOURCE_ANCHOR_REF: &str = "ToS/philosophy/atlas/dossiers/source-anchor-backlog.jsonl";
pub const TERM_INDEX_REF: &str = "ToS/philosophy/atlas/dossiers/term-index.jsonl";
pub const TRANSMISSION_REF: &str = "ToS/philosophy/atlas/dossiers/transmission-backlog.jsonl";
pub const LANGUAGE_PACKETS_ROOT: &str = "ToS/philosophy/graph-workbench/language-packets";
pub const LANGUAGE_PACKET_CONTRACT_REF: &str =
    "ToS/philosophy/atlas/multilingual/text-bearing-nodes.contract.json";
pub const LANGUAGE_REGISTRY_REF: &str = "ToS/philosophy/atlas/multilingual/language-registry.json";
pub const OBSOLETE_GENERATED_BRANCH: &str =
    "ToS/philosophy/eras/bronze-age/regions/ancient-near-east";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlantingSourcePreimage {
    pub reference: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Clone, Debug)]
pub struct PreparedDossierPackageRefs {
    pub master_rows: String,
    pub master_manifest: String,
    pub intake_manifest: String,
    pub extraction_coverage: String,
    pub proposed_nodes: String,
    pub proposed_relations: String,
    pub language_packets: String,
    pub branch_fragments: String,
    pub promotion_ledger: String,
}

#[derive(Clone, Debug)]
pub struct PreparedDossierPackageInput {
    pub table_id: String,
    /// Exact `packages[table_id]` route-map object read by readiness.
    pub package: Value,
    pub routes: BTreeMap<String, Value>,
    /// Route-list order from the source JSON; branch ancestry role folding is
    /// order-sensitive in the maintained Python mapping.
    pub route_order: Vec<String>,
    pub blocked: BTreeMap<String, Value>,
    pub master_rows: Vec<Value>,
    pub master_manifest: Value,
    pub refs: PreparedDossierPackageRefs,
}

#[derive(Clone, Debug)]
pub struct PreparedDossierPlantingInputs {
    /// Preserve source package insertion order: it controls section-ref arrays.
    pub supported_table_ids: Vec<String>,
    pub packages: Vec<PreparedDossierPackageInput>,
    pub atlas_manifest: Value,
    pub dossier_branch_manifest: Value,
    pub philosophy_manifest: Value,
    /// Keyed by branch path without `/branch.manifest.json`.
    pub branch_manifests: BTreeMap<String, Value>,
    pub source_planting_refs: BTreeMap<String, Vec<String>>,
    /// Existing paths (files and directories) consulted by conditional README
    /// rows and the one guarded obsolete-branch deletion rule.
    pub existing_paths: BTreeSet<String>,
    /// Complete recursive file set for the obsolete branch, or `None` if the
    /// branch root was absent. Every file must already have a pinned preimage.
    pub existing_obsolete_branch_files: Option<Vec<PlantingSourcePreimage>>,
    /// Complete existing directory set, including the root and empty
    /// directories. `None` means the branch root was absent.
    pub existing_obsolete_branch_directories: Option<Vec<PlantingDirectoryPreimage>>,
    /// Output-root identities observed for every removable leaf at preflight.
    pub existing_obsolete_branch_output_preimages: Option<Vec<PlantingOutputPreimage>>,
    /// Reviewed Russian display labels produced from the existing label ledger.
    pub reviewed_russian_titles: BTreeMap<String, String>,
    pub source_preimages: Vec<PlantingSourcePreimage>,
}

#[derive(Clone, Debug)]
pub struct PreparedDossierOutput {
    pub reference: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlantingOutputLeafState {
    Absent,
    Present {
        sha256: String,
        size_bytes: u64,
        device: u64,
        inode: u64,
        mode: u32,
        mtime: i64,
        mtime_nsec: i64,
        ctime: i64,
        ctime_nsec: i64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlantingOutputPreimage {
    pub reference: String,
    pub state: PlantingOutputLeafState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlantingDirectoryPreimage {
    pub reference: String,
    pub device: u64,
    pub inode: u64,
    pub mode: u32,
    pub mtime: i64,
    pub mtime_nsec: i64,
    pub ctime: i64,
    pub ctime_nsec: i64,
}

#[derive(Clone, Debug)]
pub struct PreparedDossierPlantingPlan {
    /// Complete deterministic output set. The OPS publisher retains the legacy
    /// sequential per-file commit semantics after whole-plan preflight.
    pub outputs: Vec<PreparedDossierOutput>,
    /// Read-only source fixity evidence; publication must revalidate these and
    /// every extant output preimage through the same execution before writes.
    pub source_preimages: Vec<PlantingSourcePreimage>,
    /// Filled by OPS after rendering, from descriptor-guarded observations of
    /// every planned output leaf. Publication must refuse an unbound plan.
    pub output_preimages: Option<Vec<PlantingOutputPreimage>>,
    /// `None` means the obsolete generated branch was absent at preflight.
    /// `Some(paths)` contains its complete, source-owned file inventory; only
    /// `branch.manifest.json` leaves are eligible for the legacy removal.
    pub obsolete_generated_branch: Option<ObsoleteBranchIntent>,
    /// If cleanup is required, perform it immediately before this output index
    /// to retain the Python operation's branch-phase ordering.
    pub obsolete_generated_branch_before_output_index: Option<usize>,
}

impl PreparedDossierPlantingPlan {
    pub fn bind_output_preimages(
        &mut self,
        mut preimages: Vec<PlantingOutputPreimage>,
    ) -> Result<(), String> {
        preimages.sort_by(|a, b| a.reference.cmp(&b.reference));
        if preimages
            .windows(2)
            .any(|pair| pair[0].reference == pair[1].reference)
        {
            return Err("duplicate prepared-dossier output preimage".into());
        }
        let expected = self
            .outputs
            .iter()
            .map(|output| output.reference.as_str())
            .collect::<BTreeSet<_>>();
        let observed = preimages
            .iter()
            .map(|item| item.reference.as_str())
            .collect::<BTreeSet<_>>();
        if expected != observed {
            return Err(
                "prepared-dossier output preimages do not cover the complete output plan".into(),
            );
        }
        self.output_preimages = Some(preimages);
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct ObsoleteBranchIntent {
    pub root: String,
    pub files: Vec<String>,
    pub directories: Vec<PlantingDirectoryPreimage>,
    pub file_preimages: Vec<PlantingSourcePreimage>,
    pub output_preimages: Vec<PlantingOutputPreimage>,
}

fn text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Bool(true)) => "True".into(),
        Some(Value::Bool(false) | Value::Null) | None => String::new(),
        Some(Value::Number(number)) if number.as_f64() == Some(0.0) => String::new(),
        Some(value) => value.to_string(),
    }
}

fn truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) | Some(Value::Bool(false)) => false,
        Some(Value::String(value)) => !value.is_empty(),
        Some(Value::Number(number)) => number.as_f64().is_some_and(|value| value != 0.0),
        Some(Value::Array(values)) => !values.is_empty(),
        Some(Value::Object(values)) => !values.is_empty(),
        _ => true,
    }
}

fn strings(values: Option<&Value>) -> Vec<String> {
    values
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|value| text(Some(value)))
        .collect()
}

fn json_string(value: &str, output: &mut String) -> Result<(), String> {
    output.push_str(&serde_json::to_string(value).map_err(|error| error.to_string())?);
    Ok(())
}

fn python_number(number: &serde_json::Number) -> Result<String, String> {
    let raw = serde_json::to_string(number).map_err(|error| error.to_string())?;
    if !number.is_f64() {
        return Ok(raw);
    }
    let value = number
        .as_f64()
        .ok_or_else(|| "floating JSON number cannot be represented as f64".to_owned())?;
    if value == 0.0 {
        return Ok(raw);
    }
    let (mantissa, exponent) = match raw.split_once('e') {
        Some((mantissa, exponent)) => (
            mantissa,
            exponent.parse::<i32>().map_err(|error| error.to_string())?,
        ),
        None => (raw.as_str(), 0),
    };
    let negative = mantissa.starts_with('-');
    let unsigned = mantissa.strip_prefix('-').unwrap_or(mantissa);
    let digits = unsigned
        .bytes()
        .filter(u8::is_ascii_digit)
        .map(char::from)
        .collect::<String>();
    let before_decimal = unsigned.find('.').unwrap_or(unsigned.len()) as i32;
    let leading_zeroes = digits.bytes().take_while(|byte| *byte == b'0').count();
    if leading_zeroes == digits.len() {
        return Ok(raw);
    }
    let power = before_decimal
        .checked_sub(1)
        .and_then(|power| power.checked_add(exponent))
        .and_then(|power| power.checked_sub(leading_zeroes as i32))
        .ok_or_else(|| "floating JSON exponent overflow".to_owned())?;
    let mut significant = digits[leading_zeroes..].to_owned();
    while significant.len() > 1 && significant.ends_with('0') {
        significant.pop();
    }
    let sign = if negative { "-" } else { "" };
    if value.abs() < 1e-4 || value.abs() >= 1e16 {
        let fraction = &significant[1..];
        let mantissa = if fraction.is_empty() {
            significant[..1].to_owned()
        } else {
            format!("{}.{}", &significant[..1], fraction)
        };
        let exponent_sign = if power < 0 { '-' } else { '+' };
        return Ok(format!(
            "{sign}{mantissa}e{exponent_sign}{:02}",
            power.unsigned_abs()
        ));
    }
    let decimal = power + 1;
    let body = if decimal <= 0 {
        format!(
            "0.{}{}",
            "0".repeat(decimal.unsigned_abs() as usize),
            significant
        )
    } else if decimal as usize >= significant.len() {
        format!(
            "{}{}.0",
            significant,
            "0".repeat(decimal as usize - significant.len())
        )
    } else {
        format!(
            "{}.{}",
            &significant[..decimal as usize],
            &significant[decimal as usize..]
        )
    };
    Ok(format!("{sign}{body}"))
}

fn emit_python_json(
    value: &Value,
    output: &mut String,
    depth: usize,
    pretty: bool,
) -> Result<(), String> {
    match value {
        Value::Object(object) => {
            output.push('{');
            if !object.is_empty() {
                let ordered = object.iter().collect::<BTreeMap<_, _>>();
                for (index, (key, item)) in ordered.into_iter().enumerate() {
                    if pretty {
                        output.push_str(if index == 0 { "\n" } else { ",\n" });
                        output.push_str(&"  ".repeat(depth + 1));
                    } else if index != 0 {
                        output.push_str(", ");
                    }
                    json_string(key, output)?;
                    output.push_str(": ");
                    emit_python_json(item, output, depth + 1, pretty)?;
                }
                if pretty {
                    output.push('\n');
                    output.push_str(&"  ".repeat(depth));
                }
            }
            output.push('}');
        }
        Value::Array(items) => {
            output.push('[');
            for (index, item) in items.iter().enumerate() {
                if pretty {
                    output.push_str(if index == 0 { "\n" } else { ",\n" });
                    output.push_str(&"  ".repeat(depth + 1));
                } else if index != 0 {
                    output.push_str(", ");
                }
                emit_python_json(item, output, depth + 1, pretty)?;
            }
            if pretty && !items.is_empty() {
                output.push('\n');
                output.push_str(&"  ".repeat(depth));
            }
            output.push(']');
        }
        Value::String(value) => json_string(value, output)?,
        Value::Number(number) => output.push_str(&python_number(number)?),
        Value::Null | Value::Bool(_) => {
            output.push_str(&serde_json::to_string(value).map_err(|error| error.to_string())?);
        }
    }
    Ok(())
}

/// Match the maintained Python `json.dumps(value, ensure_ascii=False,
/// sort_keys=True, indent=2) + "\\n"` byte surface, including recursive
/// key ordering and default separators.
fn json_bytes(value: &Value) -> Result<Vec<u8>, String> {
    let mut output = String::new();
    emit_python_json(value, &mut output, 0, true)?;
    output.push('\n');
    Ok(output.into_bytes())
}

/// Readiness stdout uses the same Python-compatible pretty/sorted JSON byte
/// surface as the maintained observatory script.
pub fn readiness_report_bytes(value: &Value) -> Result<Vec<u8>, String> {
    json_bytes(value)
}

/// Match one Python `json.dumps(row, ensure_ascii=False, sort_keys=True)` line
/// per row; Python's default compact separators retain one space after `,`.
fn jsonl_bytes(rows: &[Value]) -> Result<Vec<u8>, String> {
    let mut output = String::new();
    for row in rows {
        emit_python_json(row, &mut output, 0, false)?;
        output.push('\n');
    }
    Ok(output.into_bytes())
}

fn add_output(
    plan: &mut Vec<PreparedDossierOutput>,
    reference: String,
    bytes: Vec<u8>,
    check: &mut dyn FnMut(u64) -> Result<(), String>,
) -> Result<(), String> {
    validate_reference(&reference)?;
    check(
        u64::try_from(bytes.len())
            .map_err(|_| "prepared-dossier output length exceeds work counter".to_owned())?,
    )?;
    if plan.iter().any(|output| output.reference == reference) {
        return Err(format!(
            "prepared-dossier output path is duplicated: {reference}"
        ));
    }
    plan.push(PreparedDossierOutput { reference, bytes });
    Ok(())
}

fn validate_reference(reference: &str) -> Result<(), String> {
    if !reference.starts_with("ToS/")
        || reference.contains('\\')
        || reference.contains('\0')
        || reference
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(format!("prepared-dossier output escaped ToS: {reference}"));
    }
    Ok(())
}

fn is_under_reference(reference: &str, root: &str) -> bool {
    reference
        .strip_prefix(root)
        .is_some_and(|suffix| suffix.starts_with('/'))
}

fn package<'a>(
    inputs: &'a PreparedDossierPlantingInputs,
    table_id: &str,
) -> Result<&'a PreparedDossierPackageInput, String> {
    inputs
        .packages
        .iter()
        .find(|package| package.table_id == table_id)
        .ok_or_else(|| format!("missing prepared-dossier package snapshot: {table_id}"))
}

fn dossier_sort_key(dossier: &PreparedDossier) -> (String, u64) {
    let number = dossier
        .dossier_id
        .chars()
        .rev()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>()
        .parse::<u64>()
        .unwrap_or(0);
    (dossier.table_id.clone(), number)
}

fn sorted_dossiers<'a>(dossiers: &'a [PreparedDossier]) -> Vec<&'a PreparedDossier> {
    let mut sorted = dossiers.iter().collect::<Vec<_>>();
    sorted.sort_by_key(|dossier| dossier_sort_key(dossier));
    sorted
}

fn route_fields(
    package: &PreparedDossierPackageInput,
    dossier: &PreparedDossier,
) -> Result<Map<String, Value>, String> {
    let route = package
        .routes
        .get(&dossier.dossier_id)
        .ok_or_else(|| format!("admitted dossier route missing: {}", dossier.dossier_id))?;
    let defaults = package
        .package
        .get("route_defaults")
        .and_then(Value::as_object)
        .ok_or("package route_defaults is not an object")?;
    let policy = package
        .package
        .get("review_policy")
        .and_then(Value::as_object);
    let manual_status = policy
        .and_then(|policy| policy.get("manual_review_statuses"))
        .and_then(Value::as_array)
        .is_some_and(|values| {
            values
                .iter()
                .any(|value| text(Some(value)) == dossier.master_status)
        });
    let confidence = dossier.master_confidence.trim().parse::<i64>().ok();
    let confidence_limit = policy
        .and_then(|policy| policy.get("manual_review_max_confidence"))
        .and_then(Value::as_i64);
    let manual_by_policy = manual_status
        || confidence
            .zip(confidence_limit)
            .is_some_and(|(value, limit)| value <= limit);
    let manual_review =
        text(route.get("review_posture")) == "manual_review_required" || manual_by_policy;
    let route_kind = text(route.get("route_kind"));
    if !manual_review && route_kind == text(defaults.get("route_kind")) {
        return Ok(Map::new());
    }
    let mut fields = Map::new();
    fields.insert(
        "review_posture".into(),
        json!(if manual_review {
            "manual_review_required".to_owned()
        } else {
            text(route.get("review_posture"))
        }),
    );
    fields.insert("route_kind".into(), json!(route_kind));
    if manual_review {
        fields.insert("master_confidence".into(), json!(dossier.master_confidence));
        fields.insert("master_status".into(), json!(dossier.master_status));
    }
    let review_reason = if truthy(route.get("review_reason")) {
        text(route.get("review_reason"))
    } else if manual_by_policy {
        format!(
            "{} master status {} and confidence {} require manual review under the package review policy",
            dossier.table_id,
            if dossier.master_status.is_empty() {
                "unknown"
            } else {
                &dossier.master_status
            },
            if dossier.master_confidence.is_empty() {
                "unknown"
            } else {
                &dossier.master_confidence
            }
        )
    } else {
        String::new()
    };
    if !review_reason.is_empty() {
        fields.insert("review_reason".into(), json!(review_reason));
    }
    if let Some(constraints) = route.get("route_constraints").and_then(Value::as_array) {
        fields.insert(
            "route_constraints".into(),
            Value::Array(
                constraints
                    .iter()
                    .map(|value| json!(text(Some(value))))
                    .collect(),
            ),
        );
    }
    Ok(fields)
}

fn source_refs(inputs: &PreparedDossierPlantingInputs, field: &str) -> Result<Vec<String>, String> {
    inputs
        .supported_table_ids
        .iter()
        .map(|table_id| {
            let package = package(inputs, table_id)?;
            Ok(match field {
                "intake_manifest" => package.refs.intake_manifest.clone(),
                "extraction_coverage" => package.refs.extraction_coverage.clone(),
                _ => return Err(format!("unknown package reference field: {field}")),
            })
        })
        .collect()
}

fn repo_ref(reference: &str) -> String {
    reference.to_owned()
}

fn intake_fingerprint(rows: &[Value]) -> Result<String, String> {
    let mut sorted = rows.iter().collect::<Vec<_>>();
    sorted.sort_by_key(|row| text(row.get("relative_path")));
    let mut body = String::new();
    for row in sorted {
        body.push_str(&text(row.get("relative_path")));
        body.push('\t');
        body.push_str(&text(row.get("size_bytes")));
        body.push('\t');
        body.push_str(&text(row.get("sha256")));
        body.push('\n');
    }
    Ok(Digest256::of_bytes(body.as_bytes()).to_hex())
}

fn build_intake_manifest(
    inputs: &PreparedDossierPlantingInputs,
    dossiers: &[&PreparedDossier],
) -> Result<Value, String> {
    let table_ids = dossiers
        .iter()
        .map(|dossier| dossier.table_id.as_str())
        .collect::<BTreeSet<_>>();
    if table_ids.len() != 1 {
        return Err("intake manifest requires exactly one table package".into());
    }
    let table_id = *table_ids.first().ok_or("intake manifest has no table")?;
    let package = package(inputs, table_id)?;
    let mut files = Vec::new();
    for dossier in dossiers {
        let metadata = &dossier.intake_metadata;
        let mut row = json!({
            "creator": metadata.get("creator").cloned().unwrap_or(Value::Null),
            "custom_generator": metadata.get("custom_generator").cloned().unwrap_or(Value::Null),
            "dossier_id": dossier.dossier_id,
            "last_modified_by": metadata.get("last_modified_by").cloned().unwrap_or(Value::Null),
            "relative_path": format!("{}/{}", dossier.docx_section, dossier.source_document),
            "section": dossier.docx_section,
            "sha256": metadata.get("sha256").cloned().unwrap_or(Value::Null),
            "signature_part_count": metadata.get("signature_part_count").cloned().unwrap_or(json!(0)),
            "size_bytes": metadata.get("size_bytes").cloned().unwrap_or(json!(0))
        });
        if table_id != "table-i" {
            row["admission_status"] = json!(dossier.admission_status);
        }
        files.push(row);
    }
    let sections = strings(package.package.get("docx_sections"));
    let section_records = sections
        .iter()
        .map(|section| {
            (
                section.clone(),
                files
                    .iter()
                    .filter(|row| text(row.get("section")) == *section)
                    .cloned()
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let generators = files
        .iter()
        .map(|row| text(row.get("custom_generator")))
        .filter(|v| !v.is_empty())
        .collect::<BTreeSet<_>>();
    let has_signatures = files.iter().any(|row| {
        row.get("signature_part_count")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            > 0
    });
    let mut result = json!({
        "schema_version": "tos_philosophy_docx_intake_manifest_v1",
        "path": package.refs.intake_manifest,
        "owner_repo": "Tree-of-Sophia",
        "owner_surface": "ToS/research-packets/deep-research/philosophy/packet-contract.md",
        "recorded_on": match table_id { "table-i" => "2026-08-27", "table-ii"|"table-iii" => "2026-09-01", _ => return Err(format!("unsupported table id {table_id}")) },
        "artifact_role": if table_id == "table-i" { "operator-local prepared Table I dossier extraction input".to_owned() } else { format!("operator-local prepared {table_id} dossier extraction input") },
        "capture_posture": {
            "custody": "operator_local_untracked_payload",
            "origin_verification": "unverified",
            "author_identity": null,
            "session_or_export_id": null,
            "generator_property_observed": generators,
            "signature_posture": if has_signatures { "signature_parts_present" } else { "no_ooxml_signature_parts_observed" }
        },
        "fingerprint_contract": {"algorithm":"sha256","record_format":"relative_path<TAB>size_bytes<TAB>sha256<LF>","ordering":"relative_path_utf8_ascending"},
        "bundle_fingerprint": intake_fingerprint(&files)?,
        "section_fingerprints": section_records.iter().map(|(section,rows)| Ok((section.clone(), json!(intake_fingerprint(rows)?)))).collect::<Result<BTreeMap<_,_>,String>>()?,
        "section_counts": section_records.iter().map(|(section,rows)| (section.clone(), json!(rows.len()))).collect::<BTreeMap<_,_>>(),
        "file_count": files.len(),
        "files": files,
        "claim_limit": "This manifest records exact bytes, sizes, logical section paths and OOXML metadata observed in the operator-local capture at planting time. Authorship, export-session identity, origin, signature trust, source-witness status, claim truth, rights, review, doctrine and canon each require evidence from their corresponding owner route."
    });
    if table_id != "table-i" {
        result["table_id"] = json!(table_id);
        result["artifact_trust_posture"] = json!({"artifact_class":"operator_local_research_docx_bundle","registered_trust_class":false,"verdict":"unknown","reason":"No registered research-DOCX trust class, authenticated producer, signature, or verified origin is available; exact local research intake remains claim-limited."});
        result["admitted_file_count"] = json!(
            dossiers
                .iter()
                .filter(|d| d.admission_status == "admitted")
                .count()
        );
        result["quarantined_file_count"] = json!(
            dossiers
                .iter()
                .filter(|d| d.admission_status != "admitted")
                .count()
        );
    }
    Ok(result)
}

fn coverage_summary(dossiers: &[&PreparedDossier]) -> Value {
    let mut classes = BTreeMap::<String, u64>::new();
    let mut families = BTreeMap::<String, u64>::new();
    let mut underlying = BTreeMap::<String, u64>::new();
    let mut headers = BTreeMap::<Vec<String>, u64>::new();
    for dossier in dossiers {
        for table in &dossier.coverage_tables {
            let count = table.get("row_count").and_then(Value::as_u64).unwrap_or(0);
            *classes
                .entry(text(table.get("coverage_class")))
                .or_default() += count;
            *families.entry(text(table.get("family"))).or_default() += count;
            if truthy(table.get("underlying_family")) {
                *underlying
                    .entry(text(table.get("underlying_family")))
                    .or_default() += count;
            }
            let header = table
                .get("header")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|v| text(Some(v)))
                .collect::<Vec<_>>();
            *headers.entry(header).or_default() += count;
        }
    }
    let row_total = classes.values().copied().sum::<u64>();
    let mut result = json!({
        "dossier_count": dossiers.len(),
        "table_body_row_count": row_total,
        "coverage_class_counts": classes,
        "family_row_counts": families,
        "headers": headers.iter().map(|(header,count)| json!({"header":header,"row_count":count})).collect::<Vec<_>>()
    });
    if !underlying.is_empty() {
        result["underlying_family_row_counts"] = json!(underlying);
    }
    result
}

fn extraction_coverage(
    inputs: &PreparedDossierPlantingInputs,
    dossiers: &[&PreparedDossier],
) -> Result<Value, String> {
    let table_ids = dossiers
        .iter()
        .map(|d| d.table_id.as_str())
        .collect::<BTreeSet<_>>();
    if table_ids.len() != 1 {
        return Err("extraction coverage requires exactly one table package".into());
    }
    let table_id = *table_ids
        .first()
        .ok_or("extraction coverage has no table")?;
    let package = package(inputs, table_id)?;
    let mut diagnostics = Vec::new();
    let mut dossier_rows = Vec::new();
    for dossier in dossiers {
        let summary = coverage_summary(&[*dossier]);
        let risks = summary["family_row_counts"]["risk_control_source_needs"]
            .as_u64()
            .unwrap_or(0);
        let mut dossier_diagnostics = dossier
            .identity_diagnostics
            .iter()
            .map(|v| json!(v))
            .collect::<Vec<_>>();
        if dossier.admission_status != "admitted" {
            diagnostics.push(json!({"code":dossier.admission_status,"dossier_id":dossier.dossier_id,"posture":"artifact_accounted_but_no_structured_semantic_output","message":"The supplied artifact conflicts with its master identity and is quarantined from planting."}));
        } else if dossier.table_count == 0 {
            dossier_diagnostics.extend([
                json!("structured_tables_absent"),
                json!("structured_risk_table_absent"),
            ]);
            diagnostics.push(json!({"code":"structured_tables_absent","dossier_id":dossier.dossier_id,"posture":"prose_only_artifact_no_structured_semantic_projection","message":"The admitted DOCX contains no tables; its bytes and paragraph count are captured, but no nodes, relations, source anchors, terms, or transmissions are synthesized from prose."}));
        } else if risks == 0 {
            dossier_diagnostics.push(json!("structured_risk_table_absent"));
            diagnostics.push(json!({"code":"structured_risk_table_absent","dossier_id":dossier.dossier_id,"posture":"prose_only_not_synthesized","message":"No structured risk rows were extracted; prose risk language remains only in the local DOCX."}));
        }
        if dossier.metadata_identity_posture != "docx_metadata_cross_checked" {
            dossier_diagnostics.push(json!(dossier.metadata_identity_posture));
        }
        let route = package.routes.get(&dossier.dossier_id);
        let route_fields = route
            .map(|_| route_fields(package, dossier))
            .transpose()?
            .unwrap_or_default();
        let review = if route.is_some() {
            route_fields
                .get("review_posture")
                .map(|v| text(Some(v)))
                .unwrap_or_else(|| text(route.and_then(|r| r.get("review_posture"))))
        } else {
            "not_applicable_quarantined".into()
        };
        dossier_rows.push(json!({
            "dossier_id":dossier.dossier_id,"docx_section":dossier.docx_section,
            "metadata_headers":dossier.metadata_headers,"metadata_identity_posture":dossier.metadata_identity_posture,
            "review_posture":review,"route_kind":route.map(|r|text(r.get("route_kind"))).unwrap_or_else(||"blocked_identity_mismatch".into()),
            "structured_risk_row_count":risks,"coverage":summary,"diagnostics":dossier_diagnostics
        }));
        if table_id != "table-i" {
            dossier_rows.last_mut().unwrap()["admission_status"] = json!(dossier.admission_status);
        }
    }
    let sections = strings(package.package.get("docx_sections"))
        .into_iter()
        .map(|section| {
            let selected = dossiers
                .iter()
                .copied()
                .filter(|d| d.docx_section == section)
                .collect::<Vec<_>>();
            (section, coverage_summary(&selected))
        })
        .collect::<BTreeMap<_, _>>();
    Ok(json!({
        "schema_version":"tos_philosophy_docx_extraction_coverage_v1","path":package.refs.extraction_coverage,
        "owner_repo":"Tree-of-Sophia","owner_surface":"ToS/philosophy/graph-workbench/PLANTING_INTERFACE.md",
        "intake_manifest_ref":package.refs.intake_manifest,"route_map_ref":ROUTE_MAP_REF,
        "coverage_posture":"bounded_structured_planting_not_full_dossier_transfer",
        "structured_primary_families":["control_or_review_anchors","corpus_or_edition_anchors","incoming_transmissions","outgoing_transmissions","proposed_nodes","proposed_relations","risk_control_source_needs","terms"],
        "identity_metadata_rule":if table_id=="table-i" {"Поле|Значение rows are examined only to cross-check Table I and ROW_TO_EXPAND; other metadata values are not represented as full dossier transfer. The A44 Параметр|Значение alias is also inspected for those two identity fields, but the table remains counted as deferred context."} else {"Поле|Значение, Поле|Идентификация, and Параметр|Значение rows are examined only to cross-check the package table and ROW_TO_EXPAND identity; other metadata values are not represented as full dossier transfer."},
        "deferred_context_rule":"Context table rows remain in the operator-local DOCX and contribute to the coverage count. Transfer into authored tree or source records follows the selected owner route.",
        "summary":coverage_summary(dossiers),"sections":sections,"dossiers":dossier_rows,"diagnostics":diagnostics,
        "claim_limit":"Coverage counts non-empty DOCX table-body rows recognized by the current parser. Prose interpretation, citation verification, risk assessment and complete dossier transfer each require their own review."
    }))
}

fn put_route(row: &mut Value, fields: &Map<String, Value>) -> Result<(), String> {
    let object = row
        .as_object_mut()
        .ok_or("planting row is not a JSON object")?;
    object.extend(fields.clone());
    Ok(())
}

fn update_atlas(
    inputs: &PreparedDossierPlantingInputs,
    dossiers: &[&PreparedDossier],
    plan: &mut Vec<PreparedDossierOutput>,
    check: &mut dyn FnMut(u64) -> Result<(), String>,
) -> Result<(Value, Value), String> {
    let admitted = dossiers
        .iter()
        .copied()
        .filter(|d| d.admission_status == "admitted")
        .collect::<Vec<_>>();
    for table_id in &inputs.supported_table_ids {
        let package = package(inputs, table_id)?;
        let rows_by_id = admitted
            .iter()
            .copied()
            .filter(|d| d.table_id == *table_id)
            .map(|d| (d.dossier_id.clone(), d))
            .collect::<BTreeMap<_, _>>();
        let blocked_by_id = dossiers
            .iter()
            .copied()
            .filter(|d| d.table_id == *table_id && d.admission_status != "admitted")
            .map(|d| (d.dossier_id.clone(), d))
            .collect::<BTreeMap<_, _>>();
        let missing = strings(package.package.get("missing_master_dossier_ids"));
        let mut master_rows = package.master_rows.clone();
        for row in &mut master_rows {
            if row.get("normalized").is_none() {
                row["normalized"] = json!({});
            }
            if !row.get("normalized").is_some_and(Value::is_object) {
                return Err(format!(
                    "master row normalized metadata is not an object: {}",
                    text(row.get("row_id"))
                ));
            }
            let row_id = text(row.get("row_id"));
            if let Some(dossier) = rows_by_id.get(&row_id) {
                row["dossier_available"] = json!(true);
                row["dossier_id"] = json!(row_id);
                let normalized = row
                    .get_mut("normalized")
                    .and_then(Value::as_object_mut)
                    .ok_or_else(|| {
                        format!("master row normalized metadata is not an object: {row_id}")
                    })?;
                normalized.insert("prepared_branch_path".into(), json!(dossier.branch_path));
                if table_id != "table-i" {
                    normalized.insert("dossier_intake_status".into(), json!("admitted"));
                } else {
                    normalized.remove("dossier_intake_status");
                }
            } else {
                row["dossier_available"] = json!(false);
                row["dossier_id"] = Value::Null;
                let normalized = row
                    .get_mut("normalized")
                    .and_then(Value::as_object_mut)
                    .ok_or_else(|| {
                        format!("master row normalized metadata is not an object: {row_id}")
                    })?;
                normalized.remove("prepared_branch_path");
                if let Some(blocked) = blocked_by_id.get(&row_id) {
                    normalized.insert(
                        "dossier_intake_status".into(),
                        json!(blocked.admission_status),
                    );
                } else if missing.iter().any(|value| value == &row_id) {
                    normalized.insert("dossier_intake_status".into(), json!("input_not_supplied"));
                } else {
                    normalized.remove("dossier_intake_status");
                }
            }
        }
        add_output(
            plan,
            package.refs.master_rows.clone(),
            jsonl_bytes(&master_rows)?,
            check,
        )?;
        let mut manifest = package.master_manifest.clone();
        manifest["available_dossiers"] = json!(rows_by_id.keys().collect::<Vec<_>>());
        if table_id != "table-i" {
            manifest["available_dossier_count"] = json!(rows_by_id.len());
            manifest["observed_input_count"] =
                json!(dossiers.iter().filter(|d| d.table_id == *table_id).count());
            manifest["quarantined_dossiers"] = json!(blocked_by_id.keys().collect::<Vec<_>>());
            manifest["missing_master_dossier_ids"] = json!(missing);
        } else if let Some(object) = manifest.as_object_mut() {
            for key in [
                "available_dossier_count",
                "observed_input_count",
                "quarantined_dossiers",
                "missing_master_dossier_ids",
            ] {
                object.remove(key);
            }
        }
        add_output(
            plan,
            package.refs.master_manifest.clone(),
            json_bytes(&manifest)?,
            check,
        )?;
    }
    let intake_refs = source_refs(inputs, "intake_manifest")?;
    let coverage_refs = source_refs(inputs, "extraction_coverage")?;
    let mut atlas = inputs.atlas_manifest.clone();
    let dossier_meta = atlas
        .get_mut("dossiers")
        .and_then(Value::as_object_mut)
        .ok_or("atlas manifest dossiers object missing")?;
    dossier_meta.insert("available_count".into(), json!(admitted.len()));
    dossier_meta.insert("route_map".into(), json!(ROUTE_MAP_REF));
    dossier_meta.insert("intake_manifests".into(), json!(intake_refs));
    dossier_meta.insert("extraction_coverages".into(), json!(coverage_refs));
    let table_i = package(inputs, "table-i")?;
    dossier_meta.insert(
        "intake_manifest".into(),
        json!(table_i.refs.intake_manifest),
    );
    dossier_meta.insert(
        "extraction_coverage".into(),
        json!(table_i.refs.extraction_coverage),
    );
    atlas["role"] = json!(
        "prepared atlas for ToS philosophy growth from master tables and admitted prepared dossiers"
    );
    let mut dossier_branch = inputs.dossier_branch_manifest.clone();
    dossier_branch["dossier_count"] = json!(admitted.len());
    dossier_branch["source_anchor_backlog"] = json!(SOURCE_ANCHOR_REF);
    dossier_branch["term_index"] = json!(TERM_INDEX_REF);
    dossier_branch["transmission_backlog"] = json!(TRANSMISSION_REF);
    dossier_branch["prepared_dossier_routes"] = json!(ROUTE_MAP_REF);
    dossier_branch["intake_manifests"] = json!(source_refs(inputs, "intake_manifest")?);
    dossier_branch["extraction_coverages"] = json!(source_refs(inputs, "extraction_coverage")?);
    dossier_branch["intake_manifest"] = json!(table_i.refs.intake_manifest);
    dossier_branch["extraction_coverage"] = json!(table_i.refs.extraction_coverage);
    dossier_branch["role"] =
        json!("index of admitted prepared Deep Research dossiers and their graph-shape tables");
    add_output(plan, ATLAS_MANIFEST_REF.into(), json_bytes(&atlas)?, check)?;
    add_output(
        plan,
        DOSSIER_BRANCH_REF.into(),
        json_bytes(&dossier_branch)?,
        check,
    )?;
    Ok((atlas, dossier_branch))
}

fn dossier_indexes(
    inputs: &PreparedDossierPlantingInputs,
    dossiers: &[&PreparedDossier],
    plan: &mut Vec<PreparedDossierOutput>,
    check: &mut dyn FnMut(u64) -> Result<(), String>,
) -> Result<(), String> {
    let admitted = dossiers
        .iter()
        .copied()
        .filter(|d| d.admission_status == "admitted")
        .collect::<Vec<_>>();
    let mut node_counts = BTreeMap::<String, u64>::new();
    let mut relation_counts = BTreeMap::<String, u64>::new();
    let mut index_rows = Vec::new();
    let mut sources = Vec::new();
    let mut terms = Vec::new();
    let mut transmissions = Vec::new();
    for dossier in &admitted {
        let package = package(inputs, &dossier.table_id)?;
        let route = package
            .routes
            .get(&dossier.dossier_id)
            .ok_or("indexed dossier route missing")?;
        let fields = route_fields(package, dossier)?;
        let mut local_nodes = BTreeMap::<String, u64>::new();
        let mut local_relations = BTreeMap::<String, u64>::new();
        for row in &dossier.node_rows {
            *local_nodes
                .entry(text(row.get("node_kind")).if_empty("unspecified"))
                .or_default() += 1;
        }
        for row in &dossier.relation_rows {
            *local_relations
                .entry(text(row.get("relation_kind")).if_empty("related_to"))
                .or_default() += 1;
        }
        for (key, count) in &local_nodes {
            *node_counts.entry(key.clone()).or_default() += count;
        }
        for (key, count) in &local_relations {
            *relation_counts.entry(key.clone()).or_default() += count;
        }
        let route_constraints = route
            .get("route_constraints")
            .filter(|v| v.is_array())
            .cloned();
        let mut row = json!({
            "atlas_status":"prepared_dossier_indexed","branch_path":dossier.branch_path,"dossier_id":dossier.dossier_id,
            "docx_section":dossier.docx_section,"extraction_coverage_ref":package.refs.extraction_coverage,
            "intake_manifest_ref":package.refs.intake_manifest,"master_table":dossier.master_table,"table_id":dossier.table_id,
            "master_confidence":dossier.master_confidence,"master_status":dossier.master_status,
            "metadata_identity_posture":dossier.metadata_identity_posture,"node_row_count":dossier.node_rows.len(),
            "node_type_counts":local_nodes,"paragraph_count":dossier.paragraph_count,"relation_counts":local_relations,
            "relation_row_count":dossier.relation_rows.len(),"source_anchor_count":dossier.source_rows.len(),
            "source_document":dossier.source_document,"table_count":dossier.table_count,"table_row":dossier.table_row,
            "term_count":dossier.term_rows.len(),"title":dossier.title,"transmission_count":dossier.transmission_rows.len(),
            "review_posture":fields.get("review_posture").map(|v|text(Some(v))).unwrap_or_else(||text(route.get("review_posture"))),
            "route_kind":text(route.get("route_kind"))
        });
        if let Some(value) = fields
            .get("review_reason")
            .filter(|v| truthy(Some(v)))
            .or_else(|| route.get("review_reason").filter(|v| truthy(Some(v))))
        {
            row["review_reason"] = value.clone();
        }
        if let Some(value) = route_constraints {
            row["route_constraints"] = value;
        }
        index_rows.push(row);
        sources.extend(dossier.source_rows.clone());
        terms.extend(dossier.term_rows.clone());
        transmissions.extend(dossier.transmission_rows.clone());
    }
    index_rows.sort_by_key(|row| (text(row.get("table_id")), text(row.get("dossier_id"))));
    add_output(
        plan,
        DOSSIER_INDEX_REF.into(),
        jsonl_bytes(&index_rows)?,
        check,
    )?;
    add_output(
        plan,
        DOSSIER_SUMMARY_REF.into(),
        json_bytes(&json!({
            "schema_version":"tos_philosophy_atlas_dossier_graph_shape_v1","path":DOSSIER_SUMMARY_REF,
            "source":"supported prepared Deep Research dossier DOCX packages",
            "source_posture":"bounded structured extraction from operator-local non-authoritative research packets",
            "intake_manifest_ref":package(inputs,"table-i")?.refs.intake_manifest,
            "intake_manifest_refs":source_refs(inputs,"intake_manifest")?,
            "extraction_coverage_ref":package(inputs,"table-i")?.refs.extraction_coverage,
            "extraction_coverage_refs":source_refs(inputs,"extraction_coverage")?,
            "dossier_count":admitted.len(),"node_row_count":admitted.iter().map(|d|d.node_rows.len()).sum::<usize>(),
            "relation_row_count":admitted.iter().map(|d|d.relation_rows.len()).sum::<usize>(),
            "node_type_counts":node_counts,"relation_counts":relation_counts,
            "source_anchor_count":sources.len(),"term_count":terms.len(),"transmission_count":transmissions.len()
        }))?,
        check,
    )?;
    add_output(
        plan,
        SOURCE_ANCHOR_REF.into(),
        jsonl_bytes(&sources)?,
        check,
    )?;
    add_output(plan, TERM_INDEX_REF.into(), jsonl_bytes(&terms)?, check)?;
    add_output(
        plan,
        TRANSMISSION_REF.into(),
        jsonl_bytes(&transmissions)?,
        check,
    )?;
    Ok(())
}

trait IfEmpty {
    fn if_empty(self, fallback: &'static str) -> String;
}
impl IfEmpty for String {
    fn if_empty(self, fallback: &'static str) -> String {
        if self.is_empty() {
            fallback.into()
        } else {
            self
        }
    }
}

fn local_alias(dossier_id: &str, original: &str) -> Option<String> {
    let value = original.trim();
    for prefix in [dossier_id.to_owned(), dossier_id.replace('-', "")] {
        if let Some(rest) = value.strip_prefix(&prefix) {
            let mut chars = rest.chars();
            if chars.next().is_some_and(|ch| matches!(ch, '.' | '-' | ':')) {
                let alias = chars.as_str().trim();
                if !alias.is_empty() {
                    return Some(alias.to_owned());
                }
            }
        }
    }
    None
}

fn casefold(value: &str) -> Result<String, String> {
    let points = value.chars().count();
    python_casefold_unicode16_v1(
        value,
        points,
        points.saturating_mul(3),
        value.len().saturating_mul(3),
    )
    .map_err(|error| error.to_string())
}

fn resolve_endpoint(
    label: &str,
    exact: &BTreeMap<String, String>,
    folded: &BTreeMap<String, String>,
    aliases: &BTreeMap<String, String>,
) -> Result<Option<String>, String> {
    let value = label.trim();
    if let Some(candidate) = exact.get(value) {
        return Ok(Some(candidate.clone()));
    }
    if let Some(candidate) = folded.get(&casefold(value)?) {
        return Ok(Some(candidate.clone()));
    }
    let mut ordered = aliases.iter().collect::<Vec<_>>();
    ordered.sort_by(|(a, _), (b, _)| {
        b.chars()
            .count()
            .cmp(&a.chars().count())
            .then_with(|| a.cmp(b))
    });
    for (alias, candidate) in ordered {
        if let Some(rest) = value.strip_prefix(alias) {
            if !rest.is_empty()
                && rest
                    .chars()
                    .next()
                    .is_some_and(|ch| ch.is_whitespace() || ":;,.—–-".contains(ch))
            {
                return Ok(Some(candidate.clone()));
            }
        }
    }
    Ok(None)
}

fn resolve_relations(
    dossiers: &[&PreparedDossier],
    check: &mut dyn FnMut(u64) -> Result<(), String>,
) -> Result<(Vec<Value>, Vec<Value>), String> {
    let mut nodes = Vec::new();
    let mut relations = Vec::new();
    let mut exact_by_dossier = BTreeMap::<String, BTreeMap<String, String>>::new();
    let mut fold_by_dossier = BTreeMap::<String, BTreeMap<String, String>>::new();
    let mut aliases_by_dossier = BTreeMap::<String, BTreeMap<String, String>>::new();
    for dossier in dossiers {
        let mut exact = BTreeMap::new();
        let mut fold_claims = BTreeMap::<String, BTreeSet<String>>::new();
        let mut alias_claims = BTreeMap::<String, BTreeSet<String>>::new();
        for row in &dossier.node_rows {
            check(1)?;
            nodes.push(row.clone());
            let candidate = text(row.get("candidate_id"));
            for key in ["original_node_id", "label"] {
                let value = text(row.get(key)).trim().to_owned();
                if value.is_empty() {
                    continue;
                }
                exact.insert(value.clone(), candidate.clone());
                fold_claims
                    .entry(casefold(&value)?)
                    .or_default()
                    .insert(candidate.clone());
                if key == "original_node_id" {
                    if let Some(alias) = local_alias(&dossier.dossier_id, &value) {
                        alias_claims
                            .entry(alias)
                            .or_default()
                            .insert(candidate.clone());
                    }
                }
            }
        }
        let mut unique_folds = BTreeMap::new();
        for (value, claims) in fold_claims {
            if claims.len() == 1 {
                unique_folds.insert(value, claims.into_iter().next().unwrap());
            }
        }
        let mut unique_aliases = BTreeMap::new();
        for (alias, claims) in alias_claims {
            if claims.len() == 1 && !exact.contains_key(&alias) {
                exact.insert(alias.clone(), claims.iter().next().unwrap().clone());
                unique_aliases.insert(alias, claims.into_iter().next().unwrap());
            }
        }
        exact_by_dossier.insert(dossier.dossier_id.clone(), exact);
        fold_by_dossier.insert(dossier.dossier_id.clone(), unique_folds);
        aliases_by_dossier.insert(dossier.dossier_id.clone(), unique_aliases);
    }
    for dossier in dossiers {
        for original in &dossier.relation_rows {
            check(1)?;
            let mut row = original.clone();
            let exact = exact_by_dossier
                .get(&dossier.dossier_id)
                .ok_or("relation dossier lookup absent")?;
            let folded = fold_by_dossier
                .get(&dossier.dossier_id)
                .ok_or("casefold dossier lookup absent")?;
            let aliases = aliases_by_dossier
                .get(&dossier.dossier_id)
                .ok_or("local alias dossier lookup absent")?;
            let source = resolve_endpoint(
                &text(row.get("source_endpoint_label")),
                exact,
                folded,
                aliases,
            )?;
            let target = resolve_endpoint(
                &text(row.get("target_endpoint_label")),
                exact,
                folded,
                aliases,
            )?;
            row["source_candidate_id"] = source
                .clone()
                .map(|value| json!(value))
                .unwrap_or(Value::Null);
            row["target_candidate_id"] = target
                .clone()
                .map(|value| json!(value))
                .unwrap_or(Value::Null);
            row["endpoint_resolution"] = json!(if source.is_some() && target.is_some() {
                "matched_nodes"
            } else {
                "label_endpoint"
            });
            relations.push(row);
        }
    }
    Ok((nodes, relations))
}

fn table_for_row(row: &Value) -> String {
    if let Some(value) = row
        .get("table_id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        return value.to_owned();
    }
    if text(row.get("dossier_id")).starts_with("T2-") {
        "table-ii".into()
    } else {
        "table-i".into()
    }
}

fn language_packet(row: &Value, multilingual: &Multilingual) -> Result<Value, String> {
    let label = text(row.get("label")).trim().to_owned();
    let table_id = table_for_row(row);
    let packet_path = format!(
        "{LANGUAGE_PACKETS_ROOT}/{}-text-bearing-nodes.jsonl",
        table_id
    );
    let source_node_ref = text(row.get("source_ref"));
    let source_node_ref = if source_node_ref.is_empty() {
        format!("ToS/philosophy/graph-workbench/proposed-nodes/{table_id}-prepared-dossiers.jsonl")
    } else {
        source_node_ref
    };
    let source_refs = [
        packet_path.clone(),
        source_node_ref.clone(),
        LANGUAGE_PACKET_CONTRACT_REF.into(),
        LANGUAGE_REGISTRY_REF.into(),
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    let props = json!({"node_type":"prepared-dossier","dossier_id":row.get("dossier_id").cloned().unwrap_or(Value::Null)});
    let label_record = multilingual
        .label(&label, &source_node_ref, &props)
        .map_err(|error| error.to_string())?;
    let review_metadata = [
        "review_posture",
        "review_reason",
        "master_status",
        "master_confidence",
    ]
    .into_iter()
    .filter_map(|field| {
        row.get(field)
            .map(|value| (field.to_owned(), value.clone()))
    })
    .collect::<Map<_, _>>();
    let mut packet = json!({
        "schema_version":"tos_philosophy_text_bearing_language_packet_v1",
        "packet_id":format!("language-packet:{}",text(row.get("candidate_id"))),
        "node_ref":{"id":row.get("candidate_id"),"id_kind":"candidate_id","source_ref":source_node_ref},
        "node_kind":row.get("node_kind"),"branch_path":row.get("branch_path"),"authority_posture":row.get("authority_posture"),
        "canon_status":row.get("canon_status"),"atlas_row_id":row.get("atlas_row_id"),"dossier_id":row.get("dossier_id"),
        "source_document":row.get("source_document"),"source_row_index":row.get("source_row_index"),"source_table_index":row.get("source_table_index"),
        "source_label":label,"source_ref":packet_path,"source_refs":source_refs,
        "language_registry_ref":LANGUAGE_REGISTRY_REF,"text_bearing_nodes_contract_ref":LANGUAGE_PACKET_CONTRACT_REF,
        "title_block":{"original":{"value":null,"language":"und","script":"Zzzz","transliteration":null,"attestation_status":"unknown","source_ref":source_node_ref,"review_status":"pending_original_witness"},
            "ru":{"value":label_record["label"]["ru"],"translation_status":label_record["translation_status"]["ru"],"source_ref":source_node_ref},
            "en":{"value":label_record["label"]["en"],"translation_status":label_record["translation_status"]["en"],"source_ref":source_node_ref}},
        "witness_block":{"source_witnesses":[],"witness_status":"pending_source_witness_anchor","required_next_fields":["witness_id","witness_kind","language","script","repository_or_corpus","source_ref"]},
        "relation_pressure":[{"predicate":"has_original_language","target_status":"unresolved","review_route":LANGUAGE_PACKET_CONTRACT_REF},{"predicate":"uses_script","target_status":"unresolved","review_route":LANGUAGE_PACKET_CONTRACT_REF},{"predicate":"has_witness","target_status":"unresolved","review_route":SOURCE_ANCHOR_REF}]
    });
    if table_id != "table-i" {
        packet["table_id"] = json!(table_id);
    }
    for (key, value) in review_metadata {
        packet[key] = value;
    }
    Ok(packet)
}

fn graph_workbench(
    inputs: &PreparedDossierPlantingInputs,
    dossiers: &[&PreparedDossier],
    multilingual: &Multilingual,
    plan: &mut Vec<PreparedDossierOutput>,
    check: &mut dyn FnMut(u64) -> Result<(), String>,
) -> Result<Vec<Value>, String> {
    let admitted = dossiers
        .iter()
        .copied()
        .filter(|d| d.admission_status == "admitted")
        .collect::<Vec<_>>();
    let (nodes, relations) = resolve_relations(&admitted, check)?;
    let packets = nodes
        .iter()
        .filter(|row| text(row.get("node_kind")) == "text_corpus")
        .map(|row| language_packet(row, multilingual))
        .collect::<Result<Vec<_>, _>>()?;
    let mut packet_refs = Vec::new();
    for table_id in &inputs.supported_table_ids {
        let package = package(inputs, table_id)?;
        let table_packets = packets
            .iter()
            .filter(|row| table_for_row(row) == *table_id)
            .cloned()
            .collect::<Vec<_>>();
        add_output(
            plan,
            package.refs.language_packets.clone(),
            jsonl_bytes(&table_packets)?,
            check,
        )?;
        packet_refs.push(package.refs.language_packets.clone());
    }
    add_output(
        plan,
        format!("{LANGUAGE_PACKETS_ROOT}/branch.manifest.json"),
        json_bytes(&json!({
            "branch_id":"philosophy.graph-workbench.language-packets","path":LANGUAGE_PACKETS_ROOT,
            "role":"text-bearing language packets emitted by prepared dossier planting before canon promotion",
            "contract_ref":LANGUAGE_PACKET_CONTRACT_REF,"language_registry_ref":LANGUAGE_REGISTRY_REF,
            "packet_count":packets.len(),"source_ref":package(inputs,"table-i")?.refs.language_packets,"source_refs":packet_refs
        }))?,
        check,
    )?;
    let table_rows=inputs.supported_table_ids.iter().map(|table_id|format!("| `{table_id}-text-bearing-nodes.jsonl` | generated packets for admitted {table_id} text-corpus candidates |\n")).collect::<String>();
    let readme = format!(
        "# Language Packets\n\n`language-packets/` contains pre-canon language packets for text-bearing philosophy nodes.\n\nThe package files are generated from supported prepared-dossier proposed nodes. It records original-language uncertainty, Russian and English display slots, witness posture, and language/script relation pressure without promoting any source claim to canon.\n\n| Surface | Role |\n| --- | --- |\n{table_rows}| `{LANGUAGE_PACKET_CONTRACT_REF}` | packet contract |\n| `{LANGUAGE_REGISTRY_REF}` | language and script registry |\n"
    );
    add_output(
        plan,
        format!("{LANGUAGE_PACKETS_ROOT}/README.md"),
        readme.into_bytes(),
        check,
    )?;
    for table_id in &inputs.supported_table_ids {
        let package = package(inputs, table_id)?;
        let mut ds = admitted
            .iter()
            .copied()
            .filter(|d| d.table_id == *table_id)
            .collect::<Vec<_>>();
        ds.sort_by_key(|dossier| dossier_sort_key(dossier));
        let table_nodes = nodes
            .iter()
            .filter(|row| table_for_row(row) == *table_id)
            .cloned()
            .collect::<Vec<_>>();
        let table_relations = relations
            .iter()
            .filter(|row| table_for_row(row) == *table_id)
            .cloned()
            .collect::<Vec<_>>();
        let table_packets = packets
            .iter()
            .filter(|row| table_for_row(row) == *table_id)
            .count();
        add_output(
            plan,
            package.refs.proposed_nodes.clone(),
            jsonl_bytes(&table_nodes)?,
            check,
        )?;
        add_output(
            plan,
            package.refs.proposed_relations.clone(),
            jsonl_bytes(&table_relations)?,
            check,
        )?;
        let mut branches = Vec::new();
        for dossier in &ds {
            let route = package
                .routes
                .get(&dossier.dossier_id)
                .ok_or("fragment dossier route missing")?;
            let fields = route_fields(package, dossier)?;
            branches.push(json!({
                "branch_path":dossier.branch_path,"dossier_id":dossier.dossier_id,"docx_section":dossier.docx_section,
                "title":dossier.title,"node_row_count":dossier.node_rows.len(),"relation_row_count":dossier.relation_rows.len(),
                "source_anchor_count":dossier.source_rows.len(),"term_count":dossier.term_rows.len(),"transmission_count":dossier.transmission_rows.len(),
                "review_posture":fields.get("review_posture").map(|v|text(Some(v))).unwrap_or_else(||text(route.get("review_posture"))),
                "route_kind":route.get("route_kind")
            }));
        }
        let mut fragments = json!({"schema_version":"tos_philosophy_branch_fragments_v1","path":package.refs.branch_fragments,"source_ref":DOSSIER_INDEX_REF,"canon_status":"pre-canon","branch_count":ds.len(),"language_packet_count":table_packets,"language_packets_ref":package.refs.language_packets,"branches":branches});
        if table_id != "table-i" {
            fragments["table_id"] = json!(table_id);
        }
        add_output(
            plan,
            package.refs.branch_fragments.clone(),
            json_bytes(&fragments)?,
            check,
        )?;
        let title = match table_id.as_str() {
            "table-i" => "Table I",
            "table-ii" => "Table II",
            "table-iii" => "Table III",
            _ => return Err(format!("unsupported table {table_id}")),
        };
        let ledger = format!(
            "# {title} Prepared Dossiers\n\nThis ledger records the bounded planting of the admitted {title} dossier package.\n\n| Surface | Count | Status |\n| --- | ---: | --- |\n| prepared dossiers | {} | atlas indexed |\n| proposed nodes | {} | pre-canon graph workbench |\n| proposed relations | {} | pre-canon graph workbench |\n| text-bearing language packets | {table_packets} | pre-canon multilingual review |\n| branch fragments | {} | era/region/tradition or explicit frontier branch bodies |\n\nPromotion remains a later authored review step through ToS canon route cards.\n",
            ds.len(),
            table_nodes.len(),
            table_relations.len(),
            ds.len()
        );
        add_output(
            plan,
            package.refs.promotion_ledger.clone(),
            ledger.into_bytes(),
            check,
        )?;
    }
    Ok(packets)
}

fn branch_id(path: &str) -> String {
    format!(
        "philosophy.{}",
        path.strip_prefix("ToS/philosophy/")
            .unwrap_or(path)
            .replace('/', ".")
    )
}

fn top_counts(rows: &[Value], key: &str, limit: usize) -> Vec<String> {
    let mut counts = BTreeMap::<String, (usize, usize)>::new();
    for (index, row) in rows.iter().enumerate() {
        let name = text(row.get(key));
        let name = if name.is_empty() {
            "unspecified".into()
        } else {
            name
        };
        let entry = counts.entry(name).or_insert((0, index));
        entry.0 += 1;
    }
    let mut values = counts.into_iter().collect::<Vec<_>>();
    // Python Counter.most_common is stable for ties: first encounter wins.
    values.sort_by(|a, b| b.1.0.cmp(&a.1.0).then_with(|| a.1.1.cmp(&b.1.1)));
    values
        .into_iter()
        .take(limit)
        .map(|(name, (count, _))| format!("{name}: {count}"))
        .collect()
}

fn branch_title(
    inputs: &PreparedDossierPlantingInputs,
    dossier: &PreparedDossier,
) -> Result<String, String> {
    if dossier.table_id == "table-i" {
        return Ok(dossier
            .title
            .replace("ToS Deep Research:", "")
            .trim()
            .to_owned());
    }
    let reviewed = inputs
        .reviewed_russian_titles
        .get(&dossier.dossier_id)
        .ok_or_else(|| format!("reviewed Russian title missing for {}", dossier.dossier_id))?;
    Ok(reviewed
        .strip_prefix(&format!("{} — ", dossier.dossier_id))
        .unwrap_or(reviewed)
        .to_owned())
}

fn route_review(
    inputs: &PreparedDossierPlantingInputs,
    dossier: &PreparedDossier,
) -> Result<(Value, Map<String, Value>), String> {
    let package = package(inputs, &dossier.table_id)?;
    let route = package
        .routes
        .get(&dossier.dossier_id)
        .ok_or("branch dossier route missing")?;
    let fields = route_fields(package, dossier)?;
    let posture = fields
        .get("review_posture")
        .map(|v| text(Some(v)))
        .unwrap_or_else(|| text(route.get("review_posture")));
    let reason = fields
        .get("review_reason")
        .map(|v| text(Some(v)))
        .or_else(|| route.get("review_reason").map(|v| text(Some(v))))
        .unwrap_or_default();
    Ok((json!({"posture":posture,"reason":reason}), fields))
}

fn render_branch_readme(
    inputs: &PreparedDossierPlantingInputs,
    dossier: &PreparedDossier,
) -> Result<String, String> {
    let title = branch_title(inputs, dossier)?;
    let node_pressure = top_counts(&dossier.node_rows, "node_kind", 8).join(", ");
    let relation_pressure = top_counts(&dossier.relation_rows, "relation_kind", 8).join(", ");
    let path = dossier
        .branch_path
        .as_deref()
        .ok_or("admitted dossier has no branch path")?;
    let route = package(inputs, &dossier.table_id)?
        .routes
        .get(&dossier.dossier_id)
        .ok_or("README route missing")?;
    let (review, fields) = route_review(inputs, dossier)?;
    let plants = inputs
        .source_planting_refs
        .get(&dossier.dossier_id)
        .cloned()
        .unwrap_or_default();
    let planting_row = if plants.is_empty() {
        String::new()
    } else {
        "| `sources/plantings/` | exact source-witness routes already planted from backlog anchors |\n".into()
    };
    let source_readme = format!("{path}/sources/README.md");
    let source_index_row = if inputs.existing_paths.contains(&source_readme) {
        "| [Sources and local texts](sources/README.md) | named works, exact versions, and local file routes |\n".into()
    } else {
        String::new()
    };
    let posture = text(review.get("posture"));
    let reason = text(review.get("reason"));
    let review_block = if posture == "manual_review_required" {
        format!(
            "## Manual Review Gate\n\n- Posture: `{posture}`\n- Master-table status/confidence: `{}/{}`\n- Reason: {reason}\n\n",
            dossier.master_status, dossier.master_confidence
        )
    } else {
        String::new()
    };
    let constraints = route
        .get("route_constraints")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|v| format!("- `{}`\n", text(Some(v))))
        .collect::<String>();
    let constraint_block = if constraints.is_empty() {
        String::new()
    } else {
        format!("## Route Constraints\n\n{constraints}\n")
    };
    Ok(format!(
        "# {title}\n\nAtlas row: `{}`. Prepared dossier: `{}`.\n\nThis branch is the ToS philosophy home for the prepared dossier's first tree-shaped growth. It keeps the dossier material in the philosophy tree while leaving canon promotion to a later authored review.\n\n## Branch Pressure\n\n- Candidate node rows: {}\n- Candidate relation rows: {}\n- Source-anchor backlog rows: {}\n- Term rows: {}\n- Transmission rows: {}\n- Node pressure: {}\n- Relation pressure: {}\n\n{}{}## Local Surfaces\n\n| Surface | Role |\n| --- | --- |\n{}| `sources/source-anchor-backlog.jsonl` | future real source witness and edition anchors for this branch |\n{}| `graph-workbench/pre-canon-summary.json` | local summary of proposed graph rows before canon review |\n\nGlobal proposed node and relation rows for this branch are aggregated in `{}` and `{}`.\n",
        dossier.dossier_id,
        dossier.source_document,
        dossier.node_rows.len(),
        dossier.relation_rows.len(),
        dossier.source_rows.len(),
        dossier.term_rows.len(),
        dossier.transmission_rows.len(),
        if node_pressure.is_empty() {
            "none"
        } else {
            &node_pressure
        },
        if relation_pressure.is_empty() {
            "none"
        } else {
            &relation_pressure
        },
        review_block,
        constraint_block,
        source_index_row,
        planting_row,
        package(inputs, &dossier.table_id)?.refs.proposed_nodes,
        package(inputs, &dossier.table_id)?.refs.proposed_relations
    ))
}

fn branch_surfaces(
    inputs: &PreparedDossierPlantingInputs,
    dossiers: &[&PreparedDossier],
    plan: &mut Vec<PreparedDossierOutput>,
    check: &mut dyn FnMut(u64) -> Result<(), String>,
) -> Result<Option<ObsoleteBranchIntent>, String> {
    let admitted = dossiers
        .iter()
        .copied()
        .filter(|d| d.admission_status == "admitted")
        .collect::<Vec<_>>();
    let mut parent_children = BTreeMap::<String, BTreeSet<String>>::new();
    let mut parent_roles = BTreeMap::<String, String>::new();
    for package in &inputs.packages {
        if let Some(roles) = package
            .package
            .get("ancestor_roles")
            .and_then(Value::as_object)
        {
            for (path, role) in roles {
                parent_roles.insert(path.clone(), text(Some(role)));
            }
        }
    }
    for package in &inputs.packages {
        for dossier_id in &package.route_order {
            let route = package
                .routes
                .get(dossier_id)
                .ok_or_else(|| format!("source route order names a missing route: {dossier_id}"))?;
            if let Some(roles) = route.get("ancestor_roles").and_then(Value::as_object) {
                for (path, role) in roles {
                    let value = text(Some(role));
                    if parent_roles.get(path).is_some_and(|prior| prior != &value) {
                        return Err(format!("conflicting ancestor role for {path}"));
                    }
                    parent_roles.insert(path.clone(), value);
                }
            }
            let branch = text(route.get("branch_path"));
            if branch.is_empty() {
                return Err("route branch_path is empty".into());
            }
            validate_reference(&format!("{branch}/branch.manifest.json"))?;
            let parts = branch.split('/').collect::<Vec<_>>();
            for index in 3..parts.len() {
                let parent = parts[..index].join("/");
                let child = parts[index].to_owned();
                parent_children.entry(parent).or_default().insert(child);
            }
        }
    }
    for (path, children) in &parent_children {
        let existing = inputs.branch_manifests.get(path);
        let existing_children = existing
            .and_then(|v| v.get("children"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter(|child| *child != "regions/ancient-near-east")
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        let mut all = existing_children;
        all.extend(children.iter().cloned());
        let role = existing
            .and_then(|v| v.get("role"))
            .filter(|v| truthy(Some(v)))
            .map(|v| text(Some(v)))
            .or_else(|| parent_roles.get(path).cloned())
            .unwrap_or_else(|| {
                format!(
                    "{} philosophy branch",
                    path.rsplit('/').next().unwrap_or("branch")
                )
            });
        let refs = format!("{path}/branch.manifest.json");
        add_output(
            plan,
            refs,
            json_bytes(
                &json!({"branch_id":branch_id(path),"children":all,"path":path,"role":role}),
            )?,
            check,
        )?;
    }
    for dossier in &admitted {
        let path = dossier
            .branch_path
            .as_deref()
            .ok_or("admitted branch has no path")?;
        let role = dossier
            .branch_role
            .as_deref()
            .ok_or("admitted branch has no role")?;
        let plants = inputs
            .source_planting_refs
            .get(&dossier.dossier_id)
            .cloned()
            .unwrap_or_default();
        let mut plants = plants;
        plants.sort();
        let (_, fields) = route_review(inputs, dossier)?;
        let mut root_manifest = json!({
            "branch_id":branch_id(path),"path":path,"role":role,"atlas_rows":[dossier.dossier_id],
            "prepared_dossiers":[dossier.dossier_id],"evidence_status":"prepared_dossier_branch",
            "expected_local_children":["sources","graph-workbench"],
            "source_anchor_backlog":format!("{path}/sources/source-anchor-backlog.jsonl"),
            "local_graph_summary":format!("{path}/graph-workbench/pre-canon-summary.json")
        });
        put_route(&mut root_manifest, &fields)?;
        if !plants.is_empty() {
            root_manifest["source_planting_count"] = json!(plants.len());
            root_manifest["source_planting_refs"] = json!(plants);
        }
        add_output(
            plan,
            format!("{path}/branch.manifest.json"),
            json_bytes(&root_manifest)?,
            check,
        )?;
        add_output(
            plan,
            format!("{path}/README.md"),
            render_branch_readme(inputs, dossier)?.into_bytes(),
            check,
        )?;
        let mut source_manifest = json!({"branch_id":branch_id(&format!("{path}/sources")),"path":format!("{path}/sources"),"role":if inputs.source_planting_refs.get(&dossier.dossier_id).is_some_and(|v|!v.is_empty()){format!("source-anchor backlog and planted source routes for {}",dossier.dossier_id)}else{format!("source-anchor backlog for {}",dossier.dossier_id)},"anchor_count":dossier.source_rows.len()});
        put_route(&mut source_manifest, &fields)?;
        if !plants.is_empty() {
            source_manifest["planting_count"] = json!(plants.len());
            source_manifest["planting_refs"] = json!(plants);
        }
        add_output(
            plan,
            format!("{path}/sources/branch.manifest.json"),
            json_bytes(&source_manifest)?,
            check,
        )?;
        add_output(
            plan,
            format!("{path}/sources/source-anchor-backlog.jsonl"),
            jsonl_bytes(&dossier.source_rows)?,
            check,
        )?;
        let mut graph_manifest = json!({"branch_id":branch_id(&format!("{path}/graph-workbench")),"path":format!("{path}/graph-workbench"),"role":format!("local pre-canon graph summary for {}",dossier.dossier_id),"proposed_node_count":dossier.node_rows.len(),"proposed_relation_count":dossier.relation_rows.len()});
        put_route(&mut graph_manifest, &fields)?;
        add_output(
            plan,
            format!("{path}/graph-workbench/branch.manifest.json"),
            json_bytes(&graph_manifest)?,
            check,
        )?;
        let nodes = dossier
            .node_rows
            .iter()
            .map(|r| text(r.get("node_kind")))
            .collect::<Vec<_>>();
        let relations = dossier
            .relation_rows
            .iter()
            .map(|r| text(r.get("relation_kind")))
            .collect::<Vec<_>>();
        let counts = |values: Vec<String>| {
            let mut out = BTreeMap::<String, u64>::new();
            for v in values {
                *out.entry(v).or_default() += 1;
            }
            out
        };
        let mut graph_summary = json!({
            "schema_version":"tos_philosophy_local_graph_summary_v1","path":format!("{path}/graph-workbench/pre-canon-summary.json"),
            "atlas_row_id":dossier.dossier_id,"dossier_id":dossier.dossier_id,"branch_path":path,"canon_status":"pre-canon",
            "proposed_nodes_ref":package(inputs,&dossier.table_id)?.refs.proposed_nodes,"proposed_relations_ref":package(inputs,&dossier.table_id)?.refs.proposed_relations,
            "node_row_count":dossier.node_rows.len(),"relation_row_count":dossier.relation_rows.len(),"node_type_counts":counts(nodes),"relation_counts":counts(relations)
        });
        put_route(&mut graph_summary, &fields)?;
        add_output(
            plan,
            format!("{path}/graph-workbench/pre-canon-summary.json"),
            json_bytes(&graph_summary)?,
            check,
        )?;
    }
    let obsolete = if let Some(files) = &inputs.existing_obsolete_branch_files {
        let directories = inputs
            .existing_obsolete_branch_directories
            .as_ref()
            .ok_or("obsolete branch directory inventory is missing")?;
        let output_preimages = inputs
            .existing_obsolete_branch_output_preimages
            .as_ref()
            .ok_or("obsolete branch output preimages are missing")?;
        let file_refs = files
            .iter()
            .map(|p| p.reference.clone())
            .collect::<Vec<_>>();
        if files.iter().any(|file| {
            !is_under_reference(&file.reference, OBSOLETE_GENERATED_BRANCH)
                || std::path::Path::new(&file.reference)
                    .file_name()
                    .and_then(|v| v.to_str())
                    != Some("branch.manifest.json")
        }) {
            return Err(format!(
                "{OBSOLETE_GENERATED_BRANCH} contains non-generated files; refusing to remove it"
            ));
        }
        if directories.is_empty()
            || directories
                .iter()
                .all(|directory| directory.reference != OBSOLETE_GENERATED_BRANCH)
            || output_preimages
                .iter()
                .map(|preimage| preimage.reference.as_str())
                .collect::<BTreeSet<_>>()
                != file_refs
                    .iter()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>()
        {
            return Err("obsolete branch cleanup preimages do not cover its exact tree".into());
        }
        for preimage in output_preimages {
            if !matches!(&preimage.state, PlantingOutputLeafState::Present { .. }) {
                return Err(format!(
                    "obsolete branch leaf lacks an existing output preimage: {}",
                    preimage.reference
                ));
            }
        }
        for file in files {
            if !inputs.source_preimages.iter().any(|pin| {
                pin.reference == file.reference
                    && pin.sha256 == file.sha256
                    && pin.size_bytes == file.size_bytes
            }) {
                return Err(format!(
                    "obsolete branch leaf lacks matching source preimage: {}",
                    file.reference
                ));
            }
        }
        Some(ObsoleteBranchIntent {
            root: OBSOLETE_GENERATED_BRANCH.into(),
            files: file_refs,
            directories: directories.clone(),
            file_preimages: files.clone(),
            output_preimages: output_preimages.clone(),
        })
    } else {
        None
    };
    Ok(obsolete)
}

fn refresh_philosophy_manifest(
    inputs: &PreparedDossierPlantingInputs,
    plan: &mut Vec<PreparedDossierOutput>,
    check: &mut dyn FnMut(u64) -> Result<(), String>,
) -> Result<(), String> {
    let mut manifest = inputs.philosophy_manifest.clone();
    let mut branches = inputs
        .branch_manifests
        .keys()
        .filter(|path| {
            *path != OBSOLETE_GENERATED_BRANCH
                && !is_under_reference(path, OBSOLETE_GENERATED_BRANCH)
        })
        .map(|path| format!("{path}/branch.manifest.json"))
        .collect::<BTreeSet<_>>();
    branches.extend(
        plan.iter()
            .filter(|output| {
                output.reference.ends_with("/branch.manifest.json")
                    && output.reference != OBSOLETE_GENERATED_BRANCH
                    && !is_under_reference(&output.reference, OBSOLETE_GENERATED_BRANCH)
            })
            .map(|output| output.reference.clone()),
    );
    manifest["branch_manifests"] = json!(branches);
    let mut atlas_routes = manifest
        .get("atlas_routes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|v| text(Some(v)))
        .collect::<BTreeSet<_>>();
    atlas_routes.extend([
        SOURCE_ANCHOR_REF.into(),
        TERM_INDEX_REF.into(),
        TRANSMISSION_REF.into(),
        ROUTE_MAP_REF.into(),
        LANGUAGE_REGISTRY_REF.into(),
        LANGUAGE_PACKET_CONTRACT_REF.into(),
    ]);
    manifest["atlas_routes"] = json!(atlas_routes);
    let mut contracts = manifest
        .get("research_packet_contracts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|v| text(Some(v)))
        .collect::<BTreeSet<_>>();
    for table_id in &inputs.supported_table_ids {
        let package = package(inputs, table_id)?;
        contracts.insert(package.refs.intake_manifest.clone());
        contracts.insert(package.refs.extraction_coverage.clone());
    }
    manifest["research_packet_contracts"] = json!(contracts);
    add_output(
        plan,
        PHILOSOPHY_MANIFEST_REF.into(),
        json_bytes(&manifest)?,
        check,
    )
}

fn readmes(
    plan: &mut Vec<PreparedDossierOutput>,
    inputs: &PreparedDossierPlantingInputs,
    check: &mut dyn FnMut(u64) -> Result<(), String>,
) -> Result<(), String> {
    let mut lines = String::new();
    for table_id in ["table-i", "table-ii", "table-iii"] {
        let package = package(inputs, table_id)?;
        let (intake_role, coverage_role) = match table_id {
            "table-i" => (
                "tracked fixity and capture-posture manifest for the untracked local DOCX bytes",
                "explicit structured extraction and deferred-context coverage",
            ),
            "table-ii" => (
                "Table II fixity and admission record",
                "Table II structured and deferred row accounting",
            ),
            "table-iii" => (
                "Table III fixity and admission record",
                "Table III structured and deferred row accounting",
            ),
            _ => return Err(format!("unsupported README package {table_id}")),
        };
        lines.push_str(&format!(
            "| `{}` | {intake_role} |\n| `{}` | {coverage_role} |\n",
            package.refs.intake_manifest, package.refs.extraction_coverage
        ));
    }
    let dossier_readme = format!(
        "# Dossiers\n\n`dossiers/` indexes admitted prepared Deep Research documents for the philosophy atlas.\n\nThe complete Table I, Table II, and Table III plantings record dossier identity, branch route, graph-row pressure, source-anchor backlog, terms, and transmission rows while keeping canon promotion separate.\n\n| Surface | Role |\n| --- | --- |\n| `index.jsonl` | one entry per admitted prepared dossier |\n| `graph-shape-summary.json` | aggregate node, relation, source-anchor, term, and transmission pressure |\n| `prepared-dossier-routes.json` | source-owned route map from prepared dossier ids to philosophy branch homes |\n{}| `source-anchor-backlog.jsonl` | future source witness, edition, corpus, and risk-control anchors |\n| `term-index.jsonl` | prepared term rows extracted from dossier terminology tables |\n| `transmission-backlog.jsonl` | incoming and outgoing transmission rows extracted from dossier tables |\n\nText-bearing language packets for graph review live in `ToS/philosophy/graph-workbench/language-packets/` and follow `ToS/philosophy/atlas/multilingual/text-bearing-nodes.contract.json`.\n\nBranch bodies live under `ToS/philosophy/eras/...` or an explicit `ToS/philosophy/frontiers/...` route, and pre-canon graph rows live under `ToS/philosophy/graph-workbench/`.\n",
        lines
    );
    let atlas_readme = format!(
        "# Philosophy Atlas\n\n`atlas/` is the prepared navigation body for the whole ToS philosophy tree.\n\nIt holds the master-table row spine, admitted prepared-dossier index, and aggregate pressure maps that tell the philosophy tree what must grow next.\n\n## Shape\n\n```text\natlas/\n  master-tables/\n    table-i/\n    table-ii/\n    table-iii/\n  dossiers/\n    index.jsonl\n    graph-shape-summary.json\n    source-anchor-backlog.jsonl\n    term-index.jsonl\n    transmission-backlog.jsonl\n  multilingual/\n    content-labels.json\n    language-registry.json\n    text-bearing-nodes.contract.json\n```\n\nThe atlas is prepared navigation and growth pressure. Branch bodies live in `ToS/philosophy/eras/...` or the explicit non-era `ToS/philosophy/frontiers/...` route; pre-canon graph material lives in `ToS/philosophy/graph-workbench/`; authored canon relation packs live in the canon route.\n\n`multilingual/` is a source-owned display companion for the atlas and generated graph projections. It preserves the route rule that planted works must carry their attested original form when available, plus Russian and English display labels for review and downstream visualization.\n\nFor works, corpora, inscriptions, source witnesses, translations, versions, and commentaries, `multilingual/text-bearing-nodes.contract.json` is the planting contract. It keeps original-language title posture, transliteration, Russian review text, English review/runtime text, witness posture, and graph relation pressure in one source-owned packet shape.\n"
    );
    add_output(
        plan,
        "ToS/philosophy/atlas/dossiers/README.md".into(),
        dossier_readme.into_bytes(),
        check,
    )?;
    add_output(
        plan,
        "ToS/philosophy/atlas/README.md".into(),
        atlas_readme.into_bytes(),
        check,
    )?;
    Ok(())
}

/// Purely constructs every output and preservation/deletion intent. Readiness
/// must already have passed for all supported packages; the adapter must not
/// invoke this with an observational `--table` subset.
pub fn prepare_planting_plan(
    inputs: &PreparedDossierPlantingInputs,
    dossiers: &[PreparedDossier],
    multilingual: &Multilingual,
    check: &mut dyn FnMut(u64) -> Result<(), String>,
) -> Result<PreparedDossierPlantingPlan, String> {
    if inputs
        .supported_table_ids
        .iter()
        .collect::<BTreeSet<_>>()
        .len()
        != inputs.supported_table_ids.len()
        || !inputs.supported_table_ids.iter().any(|id| id == "table-i")
    {
        return Err("prepared-dossier package set is duplicate or lacks required Table I compatibility package".into());
    }
    let package_ids = inputs
        .packages
        .iter()
        .map(|package| package.table_id.as_str())
        .collect::<BTreeSet<_>>();
    if package_ids.len() != inputs.packages.len()
        || inputs
            .supported_table_ids
            .iter()
            .any(|table_id| !package_ids.contains(table_id.as_str()))
    {
        return Err("supported table IDs lack unique package snapshots".into());
    }
    let mut seen = BTreeSet::new();
    for dossier in dossiers {
        check(1)?;
        if !inputs.supported_table_ids.contains(&dossier.table_id) {
            return Err(format!(
                "dossier table is unsupported: {}",
                dossier.table_id
            ));
        }
        if !seen.insert(dossier.dossier_id.clone()) {
            return Err(format!(
                "dossier IDs collide across supported packages: {}",
                dossier.dossier_id
            ));
        }
        let package = package(inputs, &dossier.table_id)?;
        if dossier.admission_status == "admitted"
            && !package.routes.contains_key(&dossier.dossier_id)
        {
            return Err(format!(
                "admitted dossier has no route: {}",
                dossier.dossier_id
            ));
        }
        if dossier.admission_status != "admitted"
            && !package.blocked.contains_key(&dossier.dossier_id)
        {
            return Err(format!(
                "non-admitted dossier has no explicit blocked disposition: {}",
                dossier.dossier_id
            ));
        }
    }
    let sorted = sorted_dossiers(dossiers);
    let mut outputs = Vec::new();
    for table_id in &inputs.supported_table_ids {
        let rows = sorted
            .iter()
            .copied()
            .filter(|d| d.table_id == *table_id)
            .collect::<Vec<_>>();
        if rows.is_empty() {
            return Err(format!(
                "supported package has no discovered dossier records: {table_id}"
            ));
        }
        let package = package(inputs, table_id)?;
        add_output(
            &mut outputs,
            package.refs.intake_manifest.clone(),
            json_bytes(&build_intake_manifest(inputs, &rows)?)?,
            check,
        )?;
        add_output(
            &mut outputs,
            package.refs.extraction_coverage.clone(),
            json_bytes(&extraction_coverage(inputs, &rows)?)?,
            check,
        )?;
    }
    let _ = (
        update_atlas(inputs, &sorted, &mut outputs, check)?,
        dossier_indexes(inputs, &sorted, &mut outputs, check)?,
    );
    graph_workbench(inputs, &sorted, multilingual, &mut outputs, check)?;
    let branch_output_start = outputs.len();
    let obsolete = branch_surfaces(inputs, &sorted, &mut outputs, check)?;
    refresh_philosophy_manifest(inputs, &mut outputs, check)?;
    readmes(&mut outputs, inputs, check)?;
    let mut output_preimages = inputs.source_preimages.clone();
    if let Some(intent) = &obsolete {
        output_preimages.extend(intent.file_preimages.clone());
    }
    output_preimages.sort_by(|a, b| a.reference.cmp(&b.reference));
    let mut unique_preimages = Vec::with_capacity(output_preimages.len());
    for preimage in output_preimages {
        validate_reference(&preimage.reference)?;
        if let Some(previous) = unique_preimages.last() {
            let previous: &PlantingSourcePreimage = previous;
            if previous.reference == preimage.reference {
                if previous != &preimage {
                    return Err(format!(
                        "conflicting prepared-dossier source preimages: {}",
                        preimage.reference
                    ));
                }
                continue;
            }
        }
        unique_preimages.push(preimage);
    }
    let obsolete_generated_branch_before_output_index =
        obsolete.as_ref().map(|_| branch_output_start);
    Ok(PreparedDossierPlantingPlan {
        outputs,
        source_preimages: unique_preimages,
        output_preimages: None,
        obsolete_generated_branch: obsolete,
        obsolete_generated_branch_before_output_index,
    })
}
