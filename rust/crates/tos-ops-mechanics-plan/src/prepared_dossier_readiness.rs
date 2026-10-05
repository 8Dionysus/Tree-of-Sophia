//! Source-first readiness for aggregate prepared-dossier planting.
//!
//! This module is a role draft for `tos-ops-mechanics-plan`. It reads the route map and master
//! rows as source inputs, inventories local DOCX files, and delegates binary
//! DOCX parsing plus identity/header checks to the native planting owner.
//! It never writes planting products and never grants semantic, rights, review,
//! or canon admission.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};
use tos_foundation::{
    Digest256, JsonLimits, JsonMode, parse_json, python_decimal_unicode16_v1,
    python_strip_unicode16_v1,
};

pub const ROUTE_MAP_REF: &str = "ToS/philosophy/atlas/dossiers/prepared-dossier-routes.json";
pub const DEFAULT_DOC_ROOT: &str = "/home/dionysus/Загрузки/ToS";
pub const MAX_ROUTE_MAP_BYTES: u64 = 16_777_216;
pub const MAX_MASTER_JSONL_BYTES: u64 = 268_435_456;
pub const MAX_JSONL_RECORD_BYTES: usize = 1_048_576;
pub const MAX_DOCX_BYTES: u64 = 268_435_456;
/// The Python observatory exposes this fixed CLI choice list even though the
/// aggregate readiness set is derived dynamically from the route map.
pub const OBSERVATORY_TABLE_CHOICES: [&str; 3] = ["table-i", "table-ii", "table-iii"];

/// Source parsing must be selected by the owner/caller. Legacy is never a
/// fallback after strict parsing fails.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparedSourceProfile {
    PublishedStrict,
    LegacyPythonObserved,
}

impl PreparedSourceProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PublishedStrict => "tos_published_json_v1",
            Self::LegacyPythonObserved => "tos_legacy_python_observed_json_v1",
        }
    }

    const fn mode(self) -> JsonMode {
        match self {
            Self::PublishedStrict => JsonMode::PublishedStrict,
            Self::LegacyPythonObserved => JsonMode::LegacyPythonObserved,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocxContentIssue {
    pub code: String,
    pub message: String,
    pub blocking: bool,
}

/// Implemented by the native planting owner using its real DOCX parser and
/// dossier identity/header validator. A no-op implementation is not valid.
pub trait PreparedDossierContentValidator {
    fn validate(
        &self,
        table_id: &str,
        dossier_id: &str,
        raw_docx: &[u8],
        master_row: &Value,
        route: Option<&Value>,
        blocked: Option<&Value>,
        work_tick: &mut dyn FnMut(u64) -> Result<(), String>,
    ) -> Result<(), Vec<DocxContentIssue>>;

    /// Native aggregate planting retains the document parsed by validation.
    /// Non-native readiness validators keep their existing contract; the
    /// native planting adapter must supply a retained document explicitly.
    fn validate_and_retain(
        &self,
        table_id: &str,
        dossier_id: &str,
        raw_docx: &[u8],
        master_row: &Value,
        route: Option<&Value>,
        blocked: Option<&Value>,
        work_tick: &mut dyn FnMut(u64) -> Result<(), String>,
    ) -> Result<
        Option<tos_compiler::source_philosophy_dossier_docx::DocxDocument>,
        Vec<DocxContentIssue>,
    > {
        self.validate(
            table_id, dossier_id, raw_docx, master_row, route, blocked, work_tick,
        )?;
        Ok(None)
    }
}

/// Execution operations stay separate because ResearchExecution's read budget
/// is measured in IO bytes while its tick ledger measures parser/work units.
/// The caller maps repository inputs through its custody-checked `read_file`
/// path and DOCX inputs through the selected operator-owned directory reader.
/// That reader inventories one entry at a time with a no-follow type check,
/// charging a work tick for each entry.
pub trait PreparedDossierReadinessExecution {
    /// Read through the retained repository or selected DOCX root, enforcing
    /// its existing path custody, file cap, and read-byte ledger. DOCX opens
    /// must recheck regular-file status without following symlinks.
    fn read_file(&mut self, path: &Path, max_bytes: u64) -> Result<Vec<u8>, String>;
    fn tick(&mut self, work_units: u64) -> Result<(), String>;

    /// Enumerate one selected directory without following symlinks. The
    /// implementation calls `tick(1)` as each entry is visited, reports
    /// non-following entry kinds, and does not materialize the directory first.
    /// `Missing` means the directory does not exist, matching an empty glob.
    fn visit_directory(
        &mut self,
        path: &Path,
        visit: &mut dyn FnMut(PreparedDossierDirectoryEntry) -> Result<(), String>,
    ) -> Result<PreparedDossierDirectoryStatus, String>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparedDossierDirectoryStatus {
    Present,
    Missing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparedDossierEntryKind {
    RegularFile,
    Directory,
    Symlink,
    Special,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedDossierDirectoryEntry {
    pub name: OsString,
    pub kind: PreparedDossierEntryKind,
}

#[derive(Clone, Debug)]
pub struct ReadinessInputs {
    pub repository_root: PathBuf,
    pub doc_root: PathBuf,
    pub source_profile: PreparedSourceProfile,
    pub selected_table_id: Option<String>,
}

impl ReadinessInputs {
    pub fn with_defaults(
        repository_root: PathBuf,
        source_profile: PreparedSourceProfile,
        selected_table_id: Option<String>,
    ) -> Self {
        Self {
            repository_root,
            doc_root: PathBuf::from(DEFAULT_DOC_ROOT),
            source_profile,
            selected_table_id,
        }
    }
}

#[derive(Clone, Debug)]
struct Package {
    value: Value,
    routes: BTreeMap<String, Value>,
    route_order: Vec<String>,
}

#[derive(Clone, Debug)]
struct Catalog {
    packages: BTreeMap<String, Package>,
    package_order: Vec<String>,
    supported_tables: Vec<String>,
    route_map_pin: Option<PreparedDossierInputPin>,
}

#[derive(Clone, Debug)]
struct LocalDocx {
    by_section: BTreeMap<String, Vec<String>>,
    ids: Vec<String>,
    paths: Vec<(String, String, PathBuf)>, // section, dossier id, absolute path
}

#[derive(Clone, Debug)]
struct TableAssessment {
    value: Value,
    ready: bool,
    local_sections: Value,
    package: Option<PreparedDossierPackageSnapshot>,
    input_pins: Vec<PreparedDossierInputPin>,
}

/// Read-only source state carried from aggregate assessment to rendering.
/// It contains only files readiness actually read; the command owner attaches
/// manifests and output baselines collected by its wider operation.
#[derive(Clone, Debug)]
pub struct ReadinessAssessment {
    pub payload: Value,
    pub packages: BTreeMap<String, PreparedDossierPackageSnapshot>,
    /// Package IDs in the route-map object's source insertion order, including
    /// packages with no routes. Consumers use this order for source-faithful
    /// package metadata folding; it does not widen aggregate readiness.
    pub package_order: Vec<String>,
    /// Dynamically supported packages in route-map insertion order. This is
    /// the required aggregate readiness set and planting scope.
    pub supported_table_ids: Vec<String>,
    /// Exact package values from the already-read route map, including empty
    /// packages not assessed for master rows or DOCX readiness.
    pub package_configs: BTreeMap<String, Value>,
    pub input_pins: Vec<PreparedDossierInputPin>,
}

#[derive(Clone, Debug)]
pub struct PreparedDossierPackageSnapshot {
    pub table_id: String,
    pub package: Value,
    pub routes: BTreeMap<String, Value>,
    /// Route IDs in the source `routes` array order. `routes` remains keyed
    /// for identity lookup; consumers fold routes using this vector.
    pub route_order: Vec<String>,
    pub blocked: BTreeMap<String, Value>,
    pub master_rows: Vec<Value>,
    pub artifacts: Vec<PreparedDossierArtifactSnapshot>,
}

#[derive(Clone, Debug)]
pub struct PreparedDossierArtifactSnapshot {
    pub table_id: String,
    pub dossier_id: String,
    pub section: String,
    pub filename: String,
    pub raw_docx: Vec<u8>,
    /// Parsed from these same retained archive bytes during native readiness.
    /// Extraction rechecks archive SHA/size and borrows this exact document.
    pub parsed_docx: Option<tos_compiler::source_philosophy_dossier_docx::DocxDocument>,
    pub master_row: Value,
    pub route: Option<Value>,
    pub blocked: Option<Value>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedDossierInputPin {
    pub reference: String,
    pub sha256: String,
    pub size_bytes: u64,
}

/// Build the same all-supported-packages aggregate as `--plant` while
/// displaying only `selected_table_id` when a selector is supplied.
///
/// The caller owns the execution ledger. `read_file` accounts for bounded IO
/// bytes, while `tick` is the distinct work-unit channel used for source text
/// traversal and the native DOCX parser/identity validator. Raw archive bytes
/// are never passed to `tick` merely because they were read.
pub fn readiness_payload(
    inputs: &ReadinessInputs,
    validator: &dyn PreparedDossierContentValidator,
    execution: &mut dyn PreparedDossierReadinessExecution,
) -> Result<Value, String> {
    Ok(assess_readiness_inner(inputs, validator, execution, false)?.payload)
}

/// Assess all supported packages and retain the exact parsed source state and
/// DOCX bytes consumed by the assessment. The plant path must gate this
/// payload, then pass these retained snapshots to the renderer; it must not
/// re-read an unbound copy of the same readiness inputs.
pub fn assess_readiness(
    inputs: &ReadinessInputs,
    validator: &dyn PreparedDossierContentValidator,
    execution: &mut dyn PreparedDossierReadinessExecution,
) -> Result<ReadinessAssessment, String> {
    assess_readiness_inner(inputs, validator, execution, true)
}

fn assess_readiness_inner(
    inputs: &ReadinessInputs,
    validator: &dyn PreparedDossierContentValidator,
    execution: &mut dyn PreparedDossierReadinessExecution,
    retain_render_inputs: bool,
) -> Result<ReadinessAssessment, String> {
    validate_table_choice(inputs.selected_table_id.as_deref())?;
    execution.tick(0)?;
    let catalog = load_catalog(inputs, execution, retain_render_inputs)?;
    if let Some(selected) = inputs.selected_table_id.as_deref() {
        if !catalog.packages.contains_key(selected) {
            return Err(format!("unknown prepared-dossier table: {selected}"));
        }
    }

    let mut assessments = BTreeMap::new();
    for table_id in &catalog.supported_tables {
        let package = &catalog.packages[table_id];
        let assessment = table_readiness(
            inputs,
            table_id,
            package,
            &catalog,
            validator,
            execution,
            retain_render_inputs,
        )?;
        assessments.insert(table_id.clone(), assessment);
    }

    let displayed_table_ids: Vec<String> = match inputs.selected_table_id.as_deref() {
        Some(table_id) => vec![table_id.to_owned()],
        None => OBSERVATORY_TABLE_CHOICES
            .iter()
            .map(|table_id| (*table_id).to_owned())
            .collect(),
    };
    for table_id in &displayed_table_ids {
        if !catalog.packages.contains_key(table_id) {
            return Err(format!(
                "prepared-dossier-routes.json must expose a {table_id} package"
            ));
        }
        if assessments.contains_key(table_id) {
            continue;
        }
        let assessment = table_readiness(
            inputs,
            table_id,
            &catalog.packages[table_id],
            &catalog,
            validator,
            execution,
            retain_render_inputs,
        )?;
        assessments.insert(table_id.clone(), assessment);
    }

    let required_supported_package_readiness: Map<String, Value> = catalog
        .supported_tables
        .iter()
        .map(|table_id| (table_id.clone(), json!(assessments[table_id].ready)))
        .collect();
    let ready_to_plant = catalog
        .supported_tables
        .iter()
        .all(|table_id| assessments[table_id].ready);

    let tables: Map<String, Value> = displayed_table_ids
        .iter()
        .map(|table_id| (table_id.clone(), assessments[table_id].value.clone()))
        .collect();
    let local_docx_sections: Map<String, Value> = displayed_table_ids
        .iter()
        .map(|table_id| {
            (
                table_id.clone(),
                assessments[table_id].local_sections.clone(),
            )
        })
        .collect();

    let payload = json!({
        "schema_version": "tos_prepared_dossier_planting_readiness_v1",
        "owner_repo": "Tree-of-Sophia",
        "owner_surface": "ToS/philosophy/atlas/README.md",
        "doc_root": inputs.doc_root.display().to_string(),
        "planting_scope": "all_supported_packages",
        "selected_table_id": inputs.selected_table_id.clone(),
        "required_supported_package_readiness": required_supported_package_readiness,
        "ready_to_plant": ready_to_plant,
        "local_docx_sections": local_docx_sections,
        "tables": tables,
    });
    let mut packages = BTreeMap::new();
    let mut input_pins = BTreeMap::<String, PreparedDossierInputPin>::new();
    if let Some(route_map_pin) = catalog.route_map_pin {
        input_pins.insert(route_map_pin.reference.clone(), route_map_pin);
    }
    for (table_id, assessment) in assessments {
        if let Some(package) = assessment.package {
            packages.insert(table_id.clone(), package);
        }
        for pin in assessment.input_pins {
            match input_pins.get(&pin.reference) {
                Some(existing) if existing != &pin => {
                    return Err(format!(
                        "prepared-dossier input changed during assessment: {}",
                        pin.reference
                    ));
                }
                Some(_) => {}
                None => {
                    input_pins.insert(pin.reference.clone(), pin);
                }
            }
        }
    }
    let package_configs = catalog
        .packages
        .iter()
        .map(|(table_id, package)| (table_id.clone(), package.value.clone()))
        .collect();
    Ok(ReadinessAssessment {
        payload,
        packages,
        package_order: catalog.package_order,
        supported_table_ids: catalog.supported_tables,
        package_configs,
        input_pins: input_pins.into_values().collect(),
    })
}

/// Preserve the owner CLI's readiness short-circuit and aggregate-only plant
/// rule. The caller invokes planting only after `require_aggregate_readiness`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PreparedDossierAction {
    Readiness { table_id: Option<String> },
    PlantAggregate,
}

pub fn select_action(
    readiness: bool,
    plant: bool,
    table_id: Option<String>,
) -> Result<PreparedDossierAction, String> {
    validate_table_choice(table_id.as_deref())?;
    if readiness || !plant {
        return Ok(PreparedDossierAction::Readiness { table_id });
    }
    if table_id.is_some() {
        return Err(
            "prepared-dossier planting is aggregate-only because shared atlas and graph outputs cover all supported packages; use --plant without --table"
                .to_owned(),
        );
    }
    Ok(PreparedDossierAction::PlantAggregate)
}

fn validate_table_choice(table_id: Option<&str>) -> Result<(), String> {
    if let Some(table_id) = table_id
        && !OBSERVATORY_TABLE_CHOICES.contains(&table_id)
    {
        return Err(format!(
            "argument --table: invalid choice: '{table_id}' (choose from 'table-i', 'table-ii', 'table-iii')"
        ));
    }
    Ok(())
}

pub fn require_aggregate_readiness(payload: &Value) -> Result<(), String> {
    if payload
        .get("ready_to_plant")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Ok(());
    }
    let mut failed: Vec<String> = payload
        .get("required_supported_package_readiness")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|packages| packages.iter())
        .filter_map(|(table_id, ready)| {
            (!ready.as_bool().unwrap_or(false)).then(|| table_id.clone())
        })
        .collect();
    failed.sort();
    Err(format!(
        "prepared dossier planting not ready for supported packages: {}; run --readiness for exact blockers",
        failed.join(", ")
    ))
}

fn load_catalog(
    inputs: &ReadinessInputs,
    execution: &mut dyn PreparedDossierReadinessExecution,
    retain_input_pins: bool,
) -> Result<Catalog, String> {
    let route_path = inputs.repository_root.join(ROUTE_MAP_REF);
    let raw = read_bounded(&route_path, MAX_ROUTE_MAP_BYTES, execution)?;
    let route_map_pin = if retain_input_pins {
        Some(input_pin(ROUTE_MAP_REF, &raw, execution)?)
    } else {
        None
    };
    execution
        .tick(raw.len() as u64)
        .map_err(|error| format!("{}: {error}", route_path.display()))?;
    let route_map = parse_source_value(&raw, inputs.source_profile, MAX_ROUTE_MAP_BYTES as usize)?;
    let packages_value = route_map
        .get("packages")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "prepared-dossier-routes.json must expose packages as an object".to_owned()
        })?;

    let mut packages = BTreeMap::new();
    let mut package_order = Vec::new();
    let mut supported_tables = Vec::new();
    let mut all_route_ids = BTreeSet::new();

    for (table_id, package_value) in packages_value {
        execution.tick(1)?;
        package_order.push(table_id.clone());
        if !package_value.is_object() {
            return Err(format!(
                "prepared-dossier-routes.json package {table_id} must be an object"
            ));
        }
        let routes_value = package_value
            .get("routes")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                format!("prepared-dossier-routes.json {table_id}.routes must be a list")
            })?;
        let mut routes = BTreeMap::new();
        let mut route_order = Vec::new();
        if !routes_value.is_empty() {
            let defaults = package_value
                .get("route_defaults")
                .and_then(Value::as_object)
                .ok_or_else(|| {
                    format!(
                        "prepared-dossier-routes.json {table_id}.route_defaults must be an object"
                    )
                })?;
            for route_value in routes_value {
                execution.tick(1)?;
                let route = route_value
                    .as_object()
                    .ok_or_else(|| "prepared dossier routes must be objects".to_owned())?;
                let mut merged = defaults.clone();
                merged.extend(route.clone());
                // The source loader writes the owning package id after route
                // defaults and route-local values, so an input `table_id`
                // cannot redirect the route to a different package.
                merged.insert("table_id".to_owned(), json!(table_id));
                let merged_value = Value::Object(merged);
                let dossier_id = truthy_string(merged_value.get("dossier_id"));
                let branch_path = truthy_string(merged_value.get("branch_path"));
                let branch_role = truthy_string(merged_value.get("branch_role"));
                let review_posture = truthy_string(merged_value.get("review_posture"));
                let route_kind = truthy_string(merged_value.get("route_kind"));
                if dossier_id.is_empty() || branch_path.is_empty() || branch_role.is_empty() {
                    return Err(
                        "prepared dossier routes must carry dossier_id, branch_path, and branch_role"
                            .to_owned(),
                    );
                }
                if review_posture.is_empty() || route_kind.is_empty() {
                    return Err(
                        "prepared dossier routes must resolve review_posture and route_kind"
                            .to_owned(),
                    );
                }
                if routes.insert(dossier_id.clone(), merged_value).is_some() {
                    return Err(format!("duplicate prepared dossier route: {dossier_id}"));
                }
                route_order.push(dossier_id.clone());
                if !all_route_ids.insert(dossier_id.clone()) {
                    return Err(format!(
                        "prepared dossier ids must be unique across supported packages: {dossier_id}"
                    ));
                }
            }
        }
        if !routes.is_empty() {
            supported_tables.push(table_id.clone());
        }
        packages.insert(
            table_id.clone(),
            Package {
                value: package_value.clone(),
                routes,
                route_order,
            },
        );
    }

    if !supported_tables
        .iter()
        .any(|table_id| table_id == "table-i")
    {
        return Err("Table I compatibility package must remain supported".to_owned());
    }
    Ok(Catalog {
        packages,
        package_order,
        supported_tables,
        route_map_pin,
    })
}

fn table_readiness(
    inputs: &ReadinessInputs,
    table_id: &str,
    package: &Package,
    catalog: &Catalog,
    validator: &dyn PreparedDossierContentValidator,
    execution: &mut dyn PreparedDossierReadinessExecution,
    retain_render_inputs: bool,
) -> Result<TableAssessment, String> {
    let rows_path = inputs
        .repository_root
        .join("ToS/philosophy/atlas/master-tables")
        .join(table_id)
        .join("rows.jsonl");
    let mut table_input_pins = Vec::new();
    let rows = load_jsonl(
        &rows_path,
        &inputs.repository_root,
        inputs.source_profile,
        execution,
        retain_render_inputs,
        &mut table_input_pins,
    )?;
    let sections =
        package_string_array_checked(package.value.get("docx_sections"), "docx_sections")?;
    let docx_root = truthy_string(package.value.get("docx_root"));
    let local = discover_local_docx(&inputs.doc_root, &docx_root, &sections, execution)?;
    let section_json = json!(local.by_section.clone());

    if !catalog.supported_tables.iter().any(|item| item == table_id) {
        return Ok(TableAssessment {
            value: json!({
                "table_id": table_id,
                "row_count": rows.len(),
                "supported": false,
                "planting_entrypoint": Value::Null,
                "package_implementation": Value::Null,
                "route_map_ref": ROUTE_MAP_REF,
                "expected_dossier_ids": [],
                "local_docx_ids": [],
                "local_docx_ids_unique": true,
                "duplicate_local_docx_ids": [],
                "matched_local_docx_ids": [],
                "missing_expected_docx_ids": [],
                "extra_local_docx_ids": [],
                "readiness_scope": "package_local",
                "package_ready_to_plant": false,
                "next_route": "add a source-owned dossier id and branch route map before planting this table"
            }),
            ready: false,
            local_sections: section_json,
            package: retain_render_inputs.then(|| PreparedDossierPackageSnapshot {
                table_id: table_id.to_owned(),
                package: package.value.clone(),
                routes: package.routes.clone(),
                route_order: package.route_order.clone(),
                blocked: BTreeMap::new(),
                master_rows: rows,
                artifacts: Vec::new(),
            }),
            input_pins: if retain_render_inputs {
                table_input_pins
            } else {
                Vec::new()
            },
        });
    }

    let blocked = blocked_dossiers(table_id, package, execution)?;
    let expected: BTreeSet<String> = package
        .routes
        .keys()
        .chain(blocked.keys())
        .cloned()
        .collect();
    let missing_master = package_string_array_checked(
        package.value.get("missing_master_dossier_ids"),
        "missing_master_dossier_ids",
    )?;
    let expected_master: BTreeSet<String> = expected
        .iter()
        .cloned()
        .chain(missing_master.iter().cloned())
        .collect();

    let mut master_row_ids = Vec::with_capacity(rows.len());
    let mut master_row_counts = BTreeMap::<String, usize>::new();
    for row in &rows {
        execution.tick(1)?;
        let dossier_id = truthy_string(row.get("row_id"));
        *master_row_counts.entry(dossier_id.clone()).or_default() += 1;
        master_row_ids.push(dossier_id);
    }
    let unexpected_master_ids: Vec<String> = master_row_counts
        .keys()
        .filter(|dossier_id| !expected_master.contains(*dossier_id))
        .cloned()
        .collect();
    let missing_expected_master_ids: Vec<String> = expected_master
        .iter()
        .filter(|dossier_id| master_row_counts.get(*dossier_id).copied().unwrap_or(0) == 0)
        .cloned()
        .collect();
    let duplicate_expected_master_ids: Vec<String> = expected_master
        .iter()
        .filter(|dossier_id| master_row_counts.get(*dossier_id).copied().unwrap_or(0) > 1)
        .cloned()
        .collect();
    let matched_expected_master_ids: Vec<String> = expected_master
        .iter()
        .filter(|dossier_id| master_row_counts.get(*dossier_id).copied() == Some(1))
        .cloned()
        .collect();

    let unique_rows: BTreeMap<String, &Value> = rows
        .iter()
        .zip(&master_row_ids)
        .filter(|(row, dossier_id)| {
            row.is_object() && master_row_counts.get(*dossier_id).copied() == Some(1)
        })
        .map(|(row, dossier_id)| ((*dossier_id).clone(), row))
        .collect();
    let mut invalid_expected_master_rows = Vec::new();
    for dossier_id in &matched_expected_master_ids {
        execution.tick(1)?;
        let row = unique_rows[dossier_id];
        let mut errors = Vec::new();
        if row.get("table_id").and_then(Value::as_str) != Some(table_id) {
            errors.push("table_id_mismatch");
        }
        match row.get("normalized").and_then(Value::as_object) {
            None => errors.push("normalized_metadata_missing"),
            Some(normalized)
                if normalized.get("row_id").and_then(Value::as_str)
                    != Some(dossier_id.as_str()) =>
            {
                errors.push("normalized_row_id_mismatch")
            }
            Some(_) => {}
        }
        if !errors.is_empty() {
            invalid_expected_master_rows.push(json!({
                "dossier_id": dossier_id,
                "errors": errors,
            }));
        }
    }

    let master_expected_rows_valid = invalid_expected_master_rows.is_empty();
    let master_expected_ids_unique = missing_expected_master_ids.is_empty()
        && duplicate_expected_master_ids.is_empty()
        && unexpected_master_ids.is_empty();
    let local_counts = counts(&local.ids);
    let local_docx_ids_unique = local_counts.values().all(|count| *count == 1);
    let unique_local_docx_ids: BTreeSet<String> = local_counts.keys().cloned().collect();
    let expected_ids_match = unique_local_docx_ids == expected;
    let structural_preflight_ready = local_docx_ids_unique
        && expected_ids_match
        && master_expected_ids_unique
        && master_expected_rows_valid;

    let mut docx_validation_errors = Vec::new();
    let mut artifacts = Vec::new();
    let mut docx_content_validation_performed = false;
    if structural_preflight_ready {
        docx_content_validation_performed = true;
        for (section, dossier_id, path) in &local.paths {
            let raw = match read_bounded(path, MAX_DOCX_BYTES, execution) {
                Ok(raw) => raw,
                Err(error) => {
                    docx_validation_errors.push(json!({
                        "dossier_id": dossier_id,
                        "path": relative_docx_path(&inputs.doc_root, path),
                        "error_type": "ReadError",
                        "message": error,
                    }));
                    continue;
                }
            };
            let filename = relative_docx_path(&inputs.doc_root, path);
            if retain_render_inputs {
                table_input_pins.push(input_pin(filename.clone(), &raw, execution)?);
            }
            let master_row = unique_rows[dossier_id];
            let mut work_tick_refused = None;
            let mut work_tick = |amount| match execution.tick(amount) {
                Ok(()) => Ok(()),
                Err(error) => {
                    work_tick_refused = Some(error.clone());
                    Err(error)
                }
            };
            let validation = if retain_render_inputs {
                validator.validate_and_retain(
                    table_id,
                    dossier_id,
                    &raw,
                    master_row,
                    package.routes.get(dossier_id),
                    blocked.get(dossier_id),
                    &mut work_tick,
                )
            } else {
                validator
                    .validate(
                        table_id,
                        dossier_id,
                        &raw,
                        master_row,
                        package.routes.get(dossier_id),
                        blocked.get(dossier_id),
                        &mut work_tick,
                    )
                    .map(|()| None)
            };
            drop(work_tick);
            if let Some(error) = work_tick_refused {
                return Err(format!("prepared dossier work tick refused: {error}"));
            }
            let parsed_docx = match validation {
                Ok(document) => document,
                Err(issues) => {
                    for issue in issues.into_iter().filter(|issue| issue.blocking) {
                        docx_validation_errors.push(json!({
                            "dossier_id": dossier_id,
                            "path": relative_docx_path(&inputs.doc_root, path),
                            "error_type": issue.code,
                            "message": issue.message,
                        }));
                    }
                    None
                }
            };
            if retain_render_inputs {
                artifacts.push(PreparedDossierArtifactSnapshot {
                    table_id: table_id.to_owned(),
                    dossier_id: dossier_id.clone(),
                    section: section.clone(),
                    filename,
                    raw_docx: raw,
                    parsed_docx,
                    master_row: (*master_row).clone(),
                    route: package.routes.get(dossier_id).cloned(),
                    blocked: blocked.get(dossier_id).cloned(),
                });
            }
        }
    }

    // `local.paths` is already sorted by source path, matching discover_docx.
    // Preserve that order so per-file errors remain attributable to the same
    // deterministic input sequence.
    let docx_contents_valid =
        docx_content_validation_performed && docx_validation_errors.is_empty();
    let package_ready = structural_preflight_ready && docx_contents_valid;
    let matched_local_docx_ids: Vec<String> = expected
        .intersection(&unique_local_docx_ids)
        .cloned()
        .collect();
    let missing_expected_docx_ids: Vec<String> = expected
        .difference(&unique_local_docx_ids)
        .cloned()
        .collect();
    let extra_local_docx_ids: Vec<String> = unique_local_docx_ids
        .difference(&expected)
        .cloned()
        .collect();
    Ok(TableAssessment {
        value: json!({
            "table_id": table_id,
            "row_count": rows.len(),
            "supported": true,
            "planting_entrypoint": "scripts/plant_prepared_dossiers.py --plant",
            "planting_scope": "all_supported_packages",
            "package_implementation": "scripts/plant_table_i_prepared_dossiers.py",
            "route_map_ref": ROUTE_MAP_REF,
            "docx_sections": sections,
            "expected_dossier_ids": expected.iter().cloned().collect::<Vec<_>>(),
            "routed_dossier_ids": package.routes.keys().cloned().collect::<Vec<_>>(),
            "blocked_dossier_ids": blocked.keys().cloned().collect::<Vec<_>>(),
            "missing_master_dossier_ids": missing_master,
            "expected_master_dossier_ids": expected_master.iter().cloned().collect::<Vec<_>>(),
            "master_row_ids": master_row_ids.clone(),
            "master_expected_ids_unique": master_expected_ids_unique,
            "master_expected_rows_valid": master_expected_rows_valid,
            "invalid_expected_master_rows": invalid_expected_master_rows,
            "matched_expected_master_ids": matched_expected_master_ids.clone(),
            "missing_expected_master_ids": missing_expected_master_ids,
            "duplicate_expected_master_ids": duplicate_expected_master_ids,
            "unexpected_master_ids": unexpected_master_ids,
            "local_docx_ids": local.ids.clone(),
            "local_docx_ids_unique": local_docx_ids_unique,
            "duplicate_local_docx_ids": duplicate_values(&local.ids),
            "matched_local_docx_ids": matched_local_docx_ids,
            "missing_expected_docx_ids": missing_expected_docx_ids,
            "extra_local_docx_ids": extra_local_docx_ids,
            "docx_content_validation_performed": docx_content_validation_performed,
            "docx_contents_valid": docx_contents_valid,
            "docx_validation_errors": docx_validation_errors,
            "readiness_scope": "package_local",
            "package_ready_to_plant": package_ready,
            "planting_mode": truthy_string(package.value.get("planting_mode")).if_empty("complete"),
            "master_alignment": format!("{}/{}", package.routes.len(), rows.len()),
            "master_expected_alignment": format!("{}/{}", matched_expected_master_ids.len(), expected_master.len()),
            "input_admission": format!("{}/{}", package.routes.len(), expected.len()),
        }),
        ready: package_ready,
        local_sections: section_json,
        package: retain_render_inputs.then(|| PreparedDossierPackageSnapshot {
            table_id: table_id.to_owned(),
            package: package.value.clone(),
            routes: package.routes.clone(),
            route_order: package.route_order.clone(),
            blocked,
            master_rows: rows,
            artifacts,
        }),
        input_pins: if retain_render_inputs {
            table_input_pins
        } else {
            Vec::new()
        },
    })
}

fn blocked_dossiers(
    table_id: &str,
    package: &Package,
    execution: &mut dyn PreparedDossierReadinessExecution,
) -> Result<BTreeMap<String, Value>, String> {
    let Some(value) = package.value.get("blocked_dossiers") else {
        return Ok(BTreeMap::new());
    };
    let Some(values) = value.as_array() else {
        return Err(format!("{table_id}.blocked_dossiers must be a list"));
    };
    let mut result = BTreeMap::new();
    for value in values {
        execution.tick(1)?;
        if !value.is_object() || !python_truthy(value.get("dossier_id")) {
            return Err(format!(
                "{table_id}.blocked_dossiers entries must carry dossier_id"
            ));
        }
        let dossier_id = truthy_string(value.get("dossier_id"));
        if result.contains_key(&dossier_id) || package.routes.contains_key(&dossier_id) {
            return Err(format!(
                "duplicate or routed blocked dossier id: {dossier_id}"
            ));
        }
        result.insert(dossier_id, value.clone());
    }
    Ok(result)
}

fn discover_local_docx(
    doc_root: &Path,
    docx_root: &str,
    sections: &[String],
    execution: &mut dyn PreparedDossierReadinessExecution,
) -> Result<LocalDocx, String> {
    let mut by_section = BTreeMap::<String, Vec<String>>::new();
    let mut paths = Vec::new();
    for section in sections {
        let directory = doc_root.join(docx_root).join(section);
        let status = execution
            .visit_directory(&directory, &mut |entry: PreparedDossierDirectoryEntry| {
                let candidate = Path::new(&entry.name);
                if candidate.extension().and_then(|ext| ext.to_str()) != Some("docx") {
                    return Ok(());
                }
                let path = directory.join(&entry.name);
                let file_name = entry.name.to_string_lossy();
                let Some(dossier_id) = dossier_id_search(&file_name) else {
                    return Ok(());
                };
                if entry.kind != PreparedDossierEntryKind::RegularFile {
                    return Err(format!(
                        "{} is not a regular file (no-follow metadata: {:?})",
                        path.display(),
                        entry.kind
                    ));
                }
                by_section
                    .entry(section.clone())
                    .or_default()
                    .push(dossier_id.clone());
                paths.push((section.clone(), dossier_id, path));
                Ok(())
            })
            .map_err(|error| format!("cannot list {}: {error}", directory.display()))?;
        if status == PreparedDossierDirectoryStatus::Missing {
            continue;
        }
    }
    for ids in by_section.values_mut() {
        ids.sort();
    }
    paths.sort_by(|left, right| left.2.cmp(&right.2));
    let mut ids: Vec<String> = by_section.values().flatten().cloned().collect();
    ids.sort();
    Ok(LocalDocx {
        by_section,
        ids,
        paths,
    })
}

/// Equivalent to the source regex's first `(?<![A-Za-z0-9])(?:A\d{2}|T[23]-\d{2})(?!\d)` match.
/// `\d` uses Python's Unicode decimal category, pinned here through tos-foundation.
fn dossier_id_search(value: &str) -> Option<String> {
    let chars: Vec<(usize, char)> = value.char_indices().collect();
    for start in 0..chars.len() {
        if start > 0 && chars[start - 1].1.is_ascii_alphanumeric() {
            continue;
        }
        let (id_start, digit_start, digit_count) = match chars[start].1 {
            'A' if start + 2 < chars.len() => (start, start + 1, 2),
            'T' if start + 4 < chars.len()
                && matches!(chars[start + 1].1, '2' | '3')
                && chars[start + 2].1 == '-' =>
            {
                (start, start + 3, 2)
            }
            _ => continue,
        };
        if chars[digit_start..digit_start + digit_count]
            .iter()
            .any(|(_, ch)| !python_decimal_unicode16_v1(*ch))
        {
            continue;
        }
        let end = digit_start + digit_count;
        if end < chars.len() && python_decimal_unicode16_v1(chars[end].1) {
            continue;
        }
        let byte_start = chars[id_start].0;
        let byte_end = if end < chars.len() {
            chars[end].0
        } else {
            value.len()
        };
        return Some(value[byte_start..byte_end].to_owned());
    }
    None
}

fn load_jsonl(
    path: &Path,
    repository_root: &Path,
    profile: PreparedSourceProfile,
    execution: &mut dyn PreparedDossierReadinessExecution,
    retain_input_pins: bool,
    input_pins: &mut Vec<PreparedDossierInputPin>,
) -> Result<Vec<Value>, String> {
    let raw = read_bounded(path, MAX_MASTER_JSONL_BYTES, execution)?;
    if retain_input_pins {
        let reference = path
            .strip_prefix(repository_root)
            .map_err(|_| format!("{} is outside the selected repository root", path.display()))?;
        input_pins.push(input_pin(portable_path(reference), &raw, execution)?);
    }
    execution
        .tick(raw.len() as u64)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let text = std::str::from_utf8(&raw)
        .map_err(|error| format!("{} is not UTF-8: {error}", path.display()))?;
    let mut rows = Vec::new();
    for (line_number, line) in python_splitlines(text).into_iter().enumerate() {
        if python_strip_unicode16_v1(line, line.chars().count())
            .map_err(|error| format!("{} line {}: {error}", path.display(), line_number + 1))?
            .is_empty()
        {
            continue;
        }
        if line.len() > MAX_JSONL_RECORD_BYTES {
            return Err(format!(
                "{} line {} exceeds the JSONL record byte ceiling",
                path.display(),
                line_number + 1
            ));
        }
        execution
            .tick(1)
            .map_err(|error| format!("{} line {}: {error}", path.display(), line_number + 1))?;
        rows.push(
            parse_source_value(line.as_bytes(), profile, MAX_JSONL_RECORD_BYTES)
                .map_err(|error| format!("{} line {}: {error}", path.display(), line_number + 1))?,
        );
    }
    Ok(rows)
}

fn parse_source_value(
    raw: &[u8],
    profile: PreparedSourceProfile,
    max_bytes: usize,
) -> Result<Value, String> {
    let limits = JsonLimits::new(max_bytes, 64, 300_000, 4_300)
        .map_err(|error| format!("invalid source limits: {error}"))?;
    parse_json(raw, profile.mode(), limits)
        .map_err(|error| format!("foundation {}: {error}", profile.as_str()))?;
    // The foundation profile gates duplicate names, depth, number lexemes, and
    // source-mode choice. serde_json::Value is the owner-facing row API.
    serde_json::from_slice(raw).map_err(|error| {
        format!(
            "serde_json cannot represent source under {}: {error}",
            profile.as_str()
        )
    })
}

fn read_bounded(
    path: &Path,
    max_bytes: u64,
    execution: &mut dyn PreparedDossierReadinessExecution,
) -> Result<Vec<u8>, String> {
    let bytes = execution
        .read_file(path, max_bytes)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    if bytes.len() as u64 > max_bytes {
        return Err(format!(
            "{} exceeds the {} byte input ceiling",
            path.display(),
            max_bytes
        ));
    }
    Ok(bytes)
}

fn python_splitlines(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if !matches!(
            ch,
            '\n' | '\r'
                | '\u{000b}'
                | '\u{000c}'
                | '\u{001c}'
                | '\u{001d}'
                | '\u{001e}'
                | '\u{0085}'
                | '\u{2028}'
                | '\u{2029}'
        ) {
            continue;
        }
        let mut end = index + ch.len_utf8();
        if ch == '\r' && chars.peek().is_some_and(|(_, next)| *next == '\n') {
            let (next_index, next) = chars.next().expect("peeked character exists");
            end = next_index + next.len_utf8();
        }
        lines.push(&text[start..index]);
        start = end;
    }
    if start < text.len() {
        lines.push(&text[start..]);
    }
    lines
}

fn package_string_array_checked(value: Option<&Value>, field: &str) -> Result<Vec<String>, String> {
    match value {
        None => Ok(Vec::new()),
        Some(value) => value
            .as_array()
            .map(|array| {
                array
                    .iter()
                    .map(|value| source_string(Some(value)))
                    .collect()
            })
            .ok_or_else(|| format!("package {field} must be a list")),
    }
}

fn source_string(value: Option<&Value>) -> String {
    match value {
        None => String::new(),
        Some(Value::Null) => "None".to_owned(),
        Some(Value::String(value)) => value.clone(),
        Some(Value::Bool(true)) => "True".to_owned(),
        Some(Value::Bool(false)) => "False".to_owned(),
        Some(Value::Number(value)) => value.to_string(),
        Some(value) => value.to_string(),
    }
}

fn truthy_string(value: Option<&Value>) -> String {
    if python_truthy(value) {
        source_string(value)
    } else {
        String::new()
    }
}

fn python_truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) | Some(Value::Bool(false)) => false,
        Some(Value::String(value)) => !value.is_empty(),
        Some(Value::Array(value)) => !value.is_empty(),
        Some(Value::Object(value)) => !value.is_empty(),
        Some(Value::Number(value)) => {
            value.as_i64() != Some(0) && value.as_u64() != Some(0) && value.as_f64() != Some(0.0)
        }
        Some(Value::Bool(true)) => true,
    }
}

trait EmptyFallback {
    fn if_empty(self, fallback: &str) -> String;
}

impl EmptyFallback for String {
    fn if_empty(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.to_owned()
        } else {
            self
        }
    }
}

fn counts(values: &[String]) -> BTreeMap<String, usize> {
    let mut result = BTreeMap::new();
    for value in values {
        *result.entry(value.clone()).or_default() += 1;
    }
    result
}

fn duplicate_values(values: &[String]) -> Vec<String> {
    counts(values)
        .into_iter()
        .filter_map(|(value, count)| (count > 1).then_some(value))
        .collect()
}

fn relative_docx_path(doc_root: &Path, path: &Path) -> String {
    path.strip_prefix(doc_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn portable_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn input_pin(
    reference: impl Into<String>,
    raw: &[u8],
    execution: &mut dyn PreparedDossierReadinessExecution,
) -> Result<PreparedDossierInputPin, String> {
    let reference = reference.into();
    execution
        .tick(raw.len() as u64)
        .map_err(|error| format!("{reference}: {error}"))?;
    Ok(PreparedDossierInputPin {
        reference,
        sha256: Digest256::of_bytes(raw).to_hex(),
        size_bytes: raw.len() as u64,
    })
}
