//! Software-only standalone contracts and source portability checks.
//! Explicit source root; no corpus selection, runtime grant or data admission.
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{Read, Write},
    os::unix::fs::MetadataExt,
    path::{Component, Path, PathBuf},
    time::{Duration, Instant},
};
use tos_foundation::{JsonLimits, JsonMode, JsonValue, parse_json};
use tos_validation::{FormatProfile, SchemaBackendProbe, SchemaResource};
const REPORT_SCHEMA: &str = "tos_standalone_validation_v1";
const QUERY_OPERATIONS: &[&str] = &[
    "tos.status",
    "tos.snapshot",
    "tos.search",
    "tos.knowledge.search",
    "tos_philosophy_graph_scale_rows",
    "tos.source-gaps.search",
    "tos.source.descend",
    "tos.dossier.inspect",
    "tos.view.open",
    "tos.node.inspect",
    "tos.neighborhood",
    "tos.epistemic.inspect",
    "tos.path.find",
    "tos.zarathustra.word-analysis.prepare",
    "tos_philosophy_graph_lens_packet",
    "tos.zarathustra.word_analysis.public-capability",
    "tos.zarathustra.reading.public-capability",
];
const KNOWLEDGE_OPERATIONS: &[&str] = &[
    "tos.knowledge.catalog",
    "tos.knowledge.contracts",
    "tos.knowledge.search",
    "tos.knowledge.search.capabilities",
    "tos.knowledge.node.inspect",
    "tos.knowledge.relation.inspect",
    "tos.knowledge.temporal.compare",
    "tos.knowledge.focus",
    "tos.lens.open",
    "tos.lens.compile",
    "tos.source.read.contracts",
    "tos.source.read.capabilities",
    "tos.source.handle.discover",
    "tos.source.record.read",
];
const PAGE_COMMANDS: &[&str] = &[
    "tos.page.context",
    "tos.page.open-view",
    "tos.page.search",
    "tos.page.knowledge-search",
    "tos.page.find-source-gaps",
    "tos.page.prepare-word-analysis",
    "tos.page.select",
    "tos.page.inspect-selection",
    "tos.page.show-neighborhood",
    "tos.page.start-path",
    "tos.page.find-path",
    "tos.page.reroute-without-selection",
    "tos.page.inspect-epistemic",
    "tos.page.compare-readings",
    "tos.page.research-workspace",
    "tos.page.add-research-note",
    "tos.page.add-session-hypothesis",
    "tos.page.stage-proposal",
    "tos.page.exclude-selected-edge",
    "tos.page.save-route-comparison",
    "tos.page.workspace-undo",
    "tos.page.workspace-redo",
    "tos.page.workspace-export",
    "tos.page.workspace-import",
    "tos.page.clear-focus",
    "tos.page.cancel",
];
const TEN_ACCESS_SCHEMAS: &[&str] = &[
    "access/contracts/knowledge-graph.v1.schema.json",
    "access/contracts/knowledge-search-indexed.v2.schema.json",
    "access/contracts/lens-spec.v1.schema.json",
    "access/contracts/lens-result.v1.schema.json",
    "access/contracts/temporal-comparison-request.v1.schema.json",
    "access/contracts/temporal-comparison-result.v1.schema.json",
    "access/contracts/exploration-request.v1.schema.json",
    "access/contracts/exploration-result.v1.schema.json",
    "access/contracts/exploration-request.v2.schema.json",
    "access/contracts/exploration-result.v2.schema.json",
];
const ALL_PROGRAM_SCHEMAS: &[&str] = &[
    "access/contracts/knowledge-graph.v1.schema.json",
    "access/contracts/knowledge-search-indexed.v2.schema.json",
    "access/contracts/lens-spec.v1.schema.json",
    "access/contracts/lens-result.v1.schema.json",
    "access/contracts/temporal-comparison-request.v1.schema.json",
    "access/contracts/temporal-comparison-result.v1.schema.json",
    "access/contracts/exploration-request.v1.schema.json",
    "access/contracts/exploration-result.v1.schema.json",
    "access/contracts/exploration-request.v2.schema.json",
    "access/contracts/exploration-result.v2.schema.json",
    "ToS/contracts/semantic-entity-type-registry.schema.json",
    "ToS/contracts/semantic-relation-type-registry.schema.json",
    "access/contracts/readable-context.v1.schema.json",
    "access/contracts/source-read.v1.schema.json",
];
const SOFTWARE_CONTRACT_PATHS: &[&str] = &[
    "access/contracts/runtime-manifest.v1.json",
    "access/profiles/abyssos.v1.json",
    "access/contracts/web-actions.v1.json",
    "access/contracts/query-operations.v1.json",
    "access/contracts/knowledge-api.v1.json",
    "access/contracts/epistemic-packet.v1.schema.json",
    "ToS/contracts/epistemic-evidence-projection.schema.json",
    "access/contracts/evidence-lens-packet.v1.schema.json",
    "access/contracts/page-commands.v1.json",
    "access/contracts/research-workspace.v1.schema.json",
    "access/contracts/runtime-data.v1.json",
];
const REQUIRED_RUNTIME_SUBJECTS: &[&str] = &[
    "ToS/derived-exports/epistemic_evidence_projection.min.json",
    "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json",
    "ToS/doctrine/semantic-interchange/entity-types.v1.json",
    "ToS/doctrine/semantic-interchange/relation-types.v1.json",
    "ToS/contracts/semantic-entity-type-registry.schema.json",
    "ToS/contracts/semantic-relation-type-registry.schema.json",
];
const BLOCKED_CODE_MARKERS: &[&[u8]] = &[
    &[47, 115, 114, 118, 47, 65, 98, 121, 115, 115, 79, 83],
    &[
        47, 115, 114, 118, 47, 97, 98, 121, 115, 115, 45, 109, 97, 99, 104, 105, 110, 101,
    ],
];
const SOFTWARE_BYTES: u64 = 64 * 1024 * 1024;
const SOFTWARE_ROWS: u64 = 100_000;
const SOFTWARE_FILE_BYTES: u64 = 16 * 1024 * 1024;
const SOFTWARE_SECONDS: u64 = 45;
const SOURCE_JSON_DEPTH: usize = 96;
const SOURCE_JSON_VISITS: usize = 1_000_000;
#[derive(Clone, Copy)]
struct Stamp {
    dev: u64,
    ino: u64,
    len: u64,
    mtime: i64,
    mtime_nsec: i64,
    ctime: i64,
    ctime_nsec: i64,
}
impl From<&fs::Metadata> for Stamp {
    fn from(value: &fs::Metadata) -> Self {
        Self {
            dev: value.dev(),
            ino: value.ino(),
            len: value.len(),
            mtime: value.mtime(),
            mtime_nsec: value.mtime_nsec(),
            ctime: value.ctime(),
            ctime_nsec: value.ctime_nsec(),
        }
    }
}
impl PartialEq for Stamp {
    fn eq(&self, other: &Self) -> bool {
        (
            self.dev,
            self.ino,
            self.len,
            self.mtime,
            self.mtime_nsec,
            self.ctime,
            self.ctime_nsec,
        ) == (
            other.dev,
            other.ino,
            other.len,
            other.mtime,
            other.mtime_nsec,
            other.ctime,
            other.ctime_nsec,
        )
    }
}
impl Eq for Stamp {}

struct Meter {
    max_bytes: u64,
    max_rows: u64,
    bytes: u64,
    rows: u64,
    work: u64,
    max_work: u64,
    deadline: Instant,
    root_path: PathBuf,
    root_stamp: Stamp,
    root_fd: File,
    held: BTreeMap<String, Stamp>,
    absent: BTreeSet<String>,
}
impl Meter {
    fn new(root: &Path, max_bytes: u64, max_rows: u64, deadline: Instant) -> Result<Self, String> {
        if max_bytes == 0 || max_rows == 0 || Instant::now() >= deadline {
            return Err("standalone validation limits required".into());
        }
        let root = fs::canonicalize(root).map_err(|_| "selected source root unavailable")?;
        let root_fd = tos_fd_open::open_absolute_directory(&root)
            .map_err(|_| "selected source root is not a no-symlink directory")?;
        let root_stamp = Stamp::from(&root_fd.metadata().map_err(|_| "source root stat failed")?);
        let max_work = max_bytes
            .checked_mul(4)
            .ok_or("standalone work envelope overflow")?;
        Ok(Self {
            max_bytes,
            max_rows,
            bytes: 0,
            rows: 0,
            work: 0,
            max_work,
            deadline,
            root_path: root,
            root_stamp,
            root_fd,
            held: BTreeMap::new(),
            absent: BTreeSet::new(),
        })
    }
    fn tick(&mut self, work: u64) -> Result<(), String> {
        if Instant::now() >= self.deadline {
            return Err("standalone validation deadline exceeded".into());
        }
        self.work = self
            .work
            .checked_add(work)
            .filter(|value| *value <= self.max_work)
            .ok_or("standalone validation work envelope exceeded")?;
        Ok(())
    }
    fn preflight_external(&self, rows: u64, bytes: u64) -> Result<(), String> {
        self.check_deadline()?;
        if rows > self.remaining_rows() {
            return Err("standalone validation row envelope exceeded".into());
        }
        if bytes > self.remaining_bytes() {
            return Err("standalone validation cumulative byte envelope exceeded".into());
        }
        Ok(())
    }
    fn check_deadline(&self) -> Result<(), String> {
        if Instant::now() >= self.deadline {
            return Err("standalone validation deadline exceeded".into());
        }
        Ok(())
    }
    fn hold(&mut self, relative: &str, stamp: Stamp) -> Result<(), String> {
        if self.absent.contains(relative) {
            return Err("optional source member appeared during validation".into());
        }
        if let Some(previous) = self.held.get(relative) {
            if *previous != stamp {
                return Err("source member changed between validation reads".into());
            }
        } else {
            self.held.insert(relative.to_owned(), stamp);
        }
        Ok(())
    }
    fn charge(&mut self, relative: &str, bytes: usize, expected: Stamp) -> Result<(), String> {
        self.tick(bytes as u64 + 1)?;
        self.bytes = self
            .bytes
            .checked_add(bytes as u64)
            .filter(|value| *value <= self.max_bytes)
            .ok_or("standalone validation cumulative byte envelope exceeded")?;
        self.rows = self
            .rows
            .checked_add(1)
            .filter(|value| *value <= self.max_rows)
            .ok_or("standalone validation row envelope exceeded")?;
        let path = self.root_path.join(relative);
        let file = tos_fd_open::open_absolute_regular(&path, bytes as u64)
            .map_err(|_| "source member changed while it was read")?;
        let stamp = Stamp::from(&file.metadata().map_err(|_| "source member stat failed")?);
        if stamp != expected {
            return Err("source member changed after it was read".into());
        }
        self.hold(relative, stamp)
    }
    fn read(&mut self, relative: &str, cap: usize) -> Result<Vec<u8>, String> {
        self.tick(1)?;
        if !safe_relative(relative) {
            return Err("source contract path is unsafe".into());
        }
        let path = self.root_path.join(relative);
        let mut file = tos_fd_open::open_absolute_regular(&path, cap as u64)
            .map_err(|_| "required source member is missing, linked, or oversized")?;
        let before = Stamp::from(&file.metadata().map_err(|_| "source member stat failed")?);
        if before.len > cap as u64 {
            return Err("source member exceeds its selected byte envelope".into());
        }
        // Reserve the observed file length plus the one-byte growth/EOF probe
        // before allocating or reading it. Only the exact bytes read are
        // charged after the post-read identity checks.
        let read_bound = before
            .len
            .checked_add(1)
            .ok_or("source member read bound overflow")?;
        self.preflight_external(1, read_bound)?;
        let mut bytes = Vec::with_capacity(before.len as usize);
        (&mut file)
            .take(read_bound)
            .read_to_end(&mut bytes)
            .map_err(|_| "source member read failed")?;
        let after = Stamp::from(&file.metadata().map_err(|_| "source member stat failed")?);
        let named = fs::symlink_metadata(&path).map_err(|_| "source member name disappeared")?;
        if bytes.len() as u64 != before.len
            || before != after
            || !named.is_file()
            || named.file_type().is_symlink()
            || Stamp::from(&named) != before
        {
            return Err("source member changed while it was read".into());
        }
        self.charge(relative, bytes.len(), after)?;
        Ok(bytes)
    }
    fn account_external(&mut self, rows: u64, bytes: u64) -> Result<(), String> {
        self.tick(
            rows.checked_add(bytes)
                .ok_or("standalone external work envelope overflow")?,
        )?;
        self.rows = self
            .rows
            .checked_add(rows)
            .filter(|value| *value <= self.max_rows)
            .ok_or("standalone validation row envelope exceeded")?;
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .filter(|value| *value <= self.max_bytes)
            .ok_or("standalone validation cumulative byte envelope exceeded")?;
        Ok(())
    }
    fn json(&mut self, relative: &str, cap: usize) -> Result<(Vec<u8>, Value), String> {
        let raw = self.read(relative, cap)?;
        let limits = JsonLimits::new(cap, SOURCE_JSON_DEPTH, SOURCE_JSON_VISITS, 4_300)
            .map_err(|_| "source JSON limits invalid")?;
        parse_json(&raw, JsonMode::PublishedStrict, limits)
            .map_err(|_| format!("invalid strict JSON source: {relative}"))?;
        let value = serde_json::from_slice(&raw)
            .map_err(|_| format!("source JSON representation unsupported: {relative}"))?;
        Ok((raw, value))
    }
    fn verify(&mut self) -> Result<(), String> {
        self.tick(1)?;
        let root_now =
            fs::symlink_metadata(&self.root_path).map_err(|_| "source root disappeared")?;
        if !root_now.is_dir()
            || root_now.file_type().is_symlink()
            || Stamp::from(&root_now) != self.root_stamp
            || Stamp::from(
                &self
                    .root_fd
                    .metadata()
                    .map_err(|_| "source root descriptor lost")?,
            ) != self.root_stamp
        {
            return Err("source root changed during standalone validation".into());
        }
        for (relative, expected) in &self.held {
            self.check_deadline()?;
            let path = self.root_path.join(relative);
            let file = tos_fd_open::open_absolute_regular(&path, expected.len)
                .map_err(|_| "source member changed before validation completed")?;
            if Stamp::from(
                &file
                    .metadata()
                    .map_err(|_| "source member recheck failed")?,
            ) != *expected
            {
                return Err("source member changed before validation completed".into());
            }
        }
        for relative in &self.absent {
            self.check_deadline()?;
            let path = self.root_path.join(relative);
            match fs::symlink_metadata(path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                _ => return Err("optional source member appeared during validation".into()),
            }
        }
        Ok(())
    }
    fn remaining_bytes(&self) -> u64 {
        self.max_bytes.saturating_sub(self.bytes)
    }
    fn remaining_rows(&self) -> u64 {
        self.max_rows.saturating_sub(self.rows)
    }
}

fn strict_json_value(raw: &[u8], cap: usize) -> Result<Value, String> {
    let limits = JsonLimits::new(cap, SOURCE_JSON_DEPTH, SOURCE_JSON_VISITS, 4_300)
        .map_err(|_| "projection JSON limits invalid")?;
    parse_json(raw, JsonMode::PublishedStrict, limits)
        .map_err(|_| "projection contains invalid strict JSON")?;
    serde_json::from_slice(raw).map_err(|_| "projection JSON representation unsupported".into())
}

fn safe_relative(value: &str) -> bool {
    !value.is_empty()
        && !Path::new(value).is_absolute()
        && Path::new(value)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
        && !value.contains(['*', '?', '[', ']'])
}
fn string_set(value: &Value, field: &str) -> Result<BTreeSet<String>, String> {
    let rows = value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("source contract {field} must be an array"))?;
    let mut result = BTreeSet::new();
    for item in rows {
        let text = item
            .as_str()
            .filter(|text| !text.is_empty())
            .ok_or_else(|| format!("source contract {field} must contain non-empty strings"))?;
        if !result.insert(text.to_owned()) {
            return Err(format!("source contract {field} contains duplicates"));
        }
    }
    Ok(result)
}
fn expected_set(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}
fn exact_ids(
    document: &Value,
    array_name: &str,
    key: &str,
    expected: &[&str],
) -> Result<(), String> {
    let rows = document
        .get(array_name)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("source contract {array_name} must be an array"))?;
    let mut ids = BTreeSet::new();
    for row in rows {
        let value = row
            .get(key)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("source contract {array_name} identity is missing"))?;
        if !ids.insert(value.to_owned()) {
            return Err(format!("source contract {array_name} repeats an identity"));
        }
    }
    if ids != expected_set(expected) {
        return Err(format!("source contract {array_name} identity set drift"));
    }
    Ok(())
}
fn parse_schema_id(raw: &[u8], cap: usize) -> Result<String, String> {
    let limits = JsonLimits::new(cap, SOURCE_JSON_DEPTH, SOURCE_JSON_VISITS, 4_300)
        .map_err(|_| "schema JSON limits invalid")?;
    let parsed = parse_json(raw, JsonMode::PublishedStrict, limits)
        .map_err(|_| "software schema source is invalid JSON")?;
    parsed
        .root()
        .object_get("$id")
        .and_then(JsonValue::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| "software schema source has no $id".into())
}
fn json_limits(max_bytes: usize) -> Result<JsonLimits, String> {
    JsonLimits::new(max_bytes, SOURCE_JSON_DEPTH, SOURCE_JSON_VISITS, 4_300)
        .map_err(|_| "standalone JSON limits invalid".into())
}
fn validate_software_contracts(meter: &mut Meter) -> Result<usize, String> {
    let mut documents = BTreeMap::new();
    for path in SOFTWARE_CONTRACT_PATHS {
        let (_, value) = meter.json(path, SOFTWARE_FILE_BYTES as usize)?;
        documents.insert(*path, value);
    }
    let mut resources = Vec::new();
    let mut schema_bytes = 0usize;
    for path in ALL_PROGRAM_SCHEMAS {
        let raw = meter.read(path, SOFTWARE_FILE_BYTES as usize)?;
        let uri = parse_schema_id(&raw, SOFTWARE_FILE_BYTES as usize)?;
        schema_bytes = schema_bytes
            .checked_add(raw.len())
            .ok_or("schema byte overflow")?;
        resources.push(SchemaResource { uri, raw });
    }
    let schemas = SchemaBackendProbe::new(resources, FormatProfile::LegacyPythonObserved20260923)
        .map_err(|e| format!("standalone schema resource preparation: {e:?}"))?;
    schemas
        .compile_all()
        .map_err(|e| format!("standalone schema compilation: {e:?}"))?;

    for path in TEN_ACCESS_SCHEMAS {
        if !documents.contains_key(path) {
            let (_, value) = meter.json(path, SOFTWARE_FILE_BYTES as usize)?;
            documents.insert(*path, value);
        }
    }
    let runtime = &documents["access/contracts/runtime-manifest.v1.json"];
    if runtime.get("authority_owner").and_then(Value::as_str) != Some("Tree-of-Sophia") {
        return Err("runtime authority owner must be Tree-of-Sophia".into());
    }
    let profiles = runtime
        .get("runtime_profiles")
        .and_then(Value::as_array)
        .ok_or("runtime manifest profiles are missing")?;
    if !profiles.iter().any(|item| {
        item.get("profile_id").and_then(Value::as_str) == Some("standalone")
            && item.get("requires_abyssos") == Some(&Value::Bool(false))
    }) {
        return Err("standalone profile must not require AbyssOS".into());
    }
    let components = runtime
        .get("components")
        .and_then(Value::as_array)
        .ok_or("runtime manifest components are missing")?;
    if !components.iter().any(|item| {
        item.get("component_id").and_then(Value::as_str) == Some("knowledge-lens-engine")
            && item.get("required") == Some(&Value::Bool(true))
            && item.get("posture").and_then(Value::as_str) == Some("read-only-derived-composition")
    }) {
        return Err("runtime manifest must require the read-only knowledge lens engine".into());
    }
    if runtime.get("integration_posture")
        != Some(&json!({
            "state":"paused", "scope":["abyssos"], "default_profile":"standalone",
            "external_activation":"disabled", "unfreeze_requires":"explicit ToS operator command"
        }))
    {
        return Err("AbyssOS integration posture must remain explicitly paused".into());
    }
    if documents["access/profiles/abyssos.v1.json"]
        .get("availability")
        .and_then(Value::as_str)
        != Some("paused")
    {
        return Err("AbyssOS access profile must remain paused".into());
    }
    let migration = &documents["access/contracts/web-actions.v1.json"];
    if migration.get("status").and_then(Value::as_str) != Some("superseded")
        || string_set(migration, "superseded_by")?
            != expected_set(&["query-operations.v1.json", "page-commands.v1.json"])
    {
        return Err("web action migration marker must route to split contracts".into());
    }
    exact_ids(
        &documents["access/contracts/query-operations.v1.json"],
        "operations",
        "operation_id",
        QUERY_OPERATIONS,
    )?;
    let api = &documents["access/contracts/knowledge-api.v1.json"];
    let api_rows = api
        .get("operations")
        .and_then(Value::as_array)
        .ok_or("knowledge API operation list is missing")?;
    let mut api_ids = BTreeSet::new();
    for row in api_rows {
        let id = row
            .get("operation_id")
            .and_then(Value::as_str)
            .ok_or("knowledge API operation ID missing")?;
        if !api_ids.insert(id.to_owned()) {
            return Err("knowledge API operation IDs are not unique".into());
        }
    }
    if api_ids != expected_set(KNOWLEDGE_OPERATIONS) {
        return Err("knowledge operation contract drift".into());
    }
    let compile_operation = api_rows
        .iter()
        .find(|row| row.get("operation_id").and_then(Value::as_str) == Some("tos.lens.compile"))
        .ok_or("knowledge lens compile operation is missing")?;
    if compile_operation
        .pointer("/http/method")
        .and_then(Value::as_str)
        != Some("POST")
        || !compile_operation
            .get("post_semantics")
            .and_then(Value::as_str)
            .is_some_and(|value| value.contains("creates no server state"))
    {
        return Err("knowledge lens compile must remain a read-only structured query".into());
    }
    let epistemic = &documents["access/contracts/epistemic-packet.v1.schema.json"];
    if epistemic
        .pointer("/properties/schema/const")
        .and_then(Value::as_str)
        != Some("tos_philosophy_epistemic_packet_v1")
        || epistemic.pointer("/properties/authority_boundary/properties")
            != Some(&json!({
                "is_source":{"const":false}, "is_canon":{"const":false},
                "is_semantic_truth":{"const":false}, "is_rights_clearance":{"const":false}
            }))
    {
        return Err("epistemic packet authority boundary must fail closed".into());
    }
    if documents["ToS/contracts/epistemic-evidence-projection.schema.json"]
        .pointer("/properties/schema_version/const")
        .and_then(Value::as_str)
        != Some("tos_epistemic_evidence_projection_v1")
    {
        return Err("Evidence Lens projection schema identity drift".into());
    }
    if documents["access/contracts/evidence-lens-packet.v1.schema.json"]
        .pointer("/properties/schema/const")
        .and_then(Value::as_str)
        != Some("tos_evidence_lens_packet_v1")
    {
        return Err("Evidence Lens packet schema identity drift".into());
    }
    exact_ids(
        &documents["access/contracts/page-commands.v1.json"],
        "commands",
        "command_id",
        PAGE_COMMANDS,
    )?;
    let workspace = &documents["access/contracts/research-workspace.v1.schema.json"];
    if workspace
        .pointer("/properties/schema/const")
        .and_then(Value::as_str)
        != Some("tos_research_workspace_session_v1")
        || workspace.pointer("/$defs/posture/properties")
            != Some(&json!({
                "session_hypothesis":{"const":true}, "source":{"const":false},
                "reviewed":{"const":false}, "canon":{"const":false}
            }))
    {
        return Err("research hypotheses must remain explicitly outside ToS authority".into());
    }
    let exploration_capabilities = crate::exploration_contracts::runtime_capabilities(None);
    if exploration_capabilities
        .object_get("runtime")
        .and_then(JsonValue::as_str)
        != Some("local")
        || exploration_capabilities.object_get("restart_survival") != Some(&JsonValue::Bool(false))
    {
        return Err("local exploration capability contract drift".into());
    }
    let allowlist: Value = {
        let path = "access/contracts/runtime-data.v1.json";
        let raw = meter.read(path, SOFTWARE_FILE_BYTES as usize)?;
        let limits = json_limits(SOFTWARE_FILE_BYTES as usize)?;
        parse_json(&raw, JsonMode::PublishedStrict, limits)
            .map_err(|_| "runtime data allowlist JSON is invalid")?;
        let value =
            serde_json::from_slice(&raw).map_err(|_| "runtime data allowlist shape invalid")?;
        value
    };
    let subjects = allowlist
        .get("subjects")
        .and_then(Value::as_array)
        .ok_or("runtime allowlist subjects must be a list")?;
    let source_paths: BTreeSet<String> = subjects
        .iter()
        .map(|item| {
            item.get("source_path")
                .and_then(Value::as_str)
                .filter(|path| safe_relative(path))
                .map(ToOwned::to_owned)
                .ok_or("runtime allowlist source path is invalid")
        })
        .collect::<Result<_, _>>()?;
    if !expected_set(REQUIRED_RUNTIME_SUBJECTS).is_subset(&source_paths) {
        return Err("runtime allowlist is missing required constructor inputs".into());
    }
    if source_paths
        .iter()
        .any(|path| path.contains("lexical-search") || path.contains("/payload/"))
    {
        return Err("runtime allowlist admits an explicitly excluded subject".into());
    }
    Ok(schema_bytes)
}

fn scan_source(meter: &mut Meter) -> Result<(), String> {
    const ROOTS: &[&str] = &[
        "access/src/tos_access",
        "access/contracts",
        "access/profiles",
        "access/packaging",
        "access/web/src",
    ];
    let mut paths = vec![
        "access/pyproject.toml".to_owned(),
        "access/web/index.html".to_owned(),
    ];
    let mut visited_entries = 0u64;
    for relative in ROOTS {
        let start = meter.root_path.join(relative);
        if !start.exists() {
            continue;
        }
        let mut pending = vec![start];
        while let Some(directory) = pending.pop() {
            meter.tick(1)?;
            let metadata = fs::symlink_metadata(&directory)
                .map_err(|_| "owned source directory disappeared")?;
            if metadata.file_type().is_symlink() {
                return Err("owned source tree contains a symlink".into());
            }
            if !metadata.is_dir() {
                continue;
            }
            let entries =
                fs::read_dir(&directory).map_err(|_| "owned source directory is unreadable")?;
            let mut children = Vec::new();
            for entry in entries {
                meter.tick(1)?;
                visited_entries = visited_entries
                    .checked_add(1)
                    .filter(|count| *count <= meter.max_rows)
                    .ok_or("owned source entry envelope exceeded")?;
                children.push(entry.map_err(|_| "owned source entry unreadable")?.path());
            }
            children.sort();
            for path in children.into_iter().rev() {
                let meta =
                    fs::symlink_metadata(&path).map_err(|_| "owned source entry disappeared")?;
                if meta.is_dir() && !meta.file_type().is_symlink() {
                    pending.push(path);
                } else if meta.is_file() && !meta.file_type().is_symlink() {
                    paths.push(
                        path.strip_prefix(&meter.root_path)
                            .map_err(|_| "owned source escaped selected root")?
                            .to_string_lossy()
                            .replace('\\', "/"),
                    );
                } else if meta.file_type().is_symlink() {
                    return Err("owned source tree contains a symlink".into());
                }
            }
        }
    }
    paths.sort();
    paths.dedup();
    for relative in paths {
        if relative.split('/').any(|part| part == "runtime_data") {
            continue;
        }
        let path = Path::new(&relative);
        let suffix = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        if !matches!(
            suffix,
            "py" | "json" | "toml" | "ts" | "js" | "mjs" | "html"
        ) {
            continue;
        }
        let payload = meter.read(&relative, SOFTWARE_FILE_BYTES as usize)?;
        if BLOCKED_CODE_MARKERS
            .iter()
            .any(|marker| payload.windows(marker.len()).any(|chunk| chunk == *marker))
        {
            return Err("hard-coded host path in owned access source".into());
        }
    }
    Ok(())
}

fn validate_software(root: &Path) -> Result<Value, String> {
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(SOFTWARE_SECONDS))
        .ok_or("software validation deadline arithmetic overflow")?;
    let mut meter = Meter::new(root, SOFTWARE_BYTES, SOFTWARE_ROWS, deadline)?;
    let facts = validate_software_contracts(&mut meter)?;
    scan_source(&mut meter)?;
    meter.verify()?;
    Ok(json!({
        "schema_version": REPORT_SCHEMA,
        "ok": true,
        "mode": "software",
        "data_validated": false,
        "checks": {
            "validation_meter_rows": meter.rows,
            "validation_meter_bytes": meter.bytes,
            "software_contracts": true,
            "source_marker_scan": true,
            "schema_resources": facts,
        }
    }))
}

pub fn run_if_requested(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Option<i32> {
    if args.first().map(String::as_str) != Some("validate-standalone") {
        return None;
    }
    if args.len() == 2 && matches!(args[1].as_str(), "--help" | "-h") {
        return Some(if writeln!(stdout,"tos validate-standalone --root ABS --software --json\nChecks software contracts and portable access sources without loading data.").is_ok(){0}else{2});
    }
    let result = (|| -> Result<Value, String> {
        let mut root = None;
        let mut software = false;
        let mut json = false;
        let mut it = args.iter().skip(1);
        while let Some(key) = it.next() {
            match key.as_str() {
                "--root" => {
                    let p = PathBuf::from(it.next().ok_or("--root needs a path")?);
                    if root.replace(p).is_some() {
                        return Err("duplicate --root".into());
                    }
                }
                "--software" if !software => software = true,
                "--json" if !json => json = true,
                _ => return Err(format!("unexpected standalone software option: {key}")),
            }
        }
        if !software || !json {
            return Err("select --software --json explicitly".into());
        }
        let root = root.ok_or("--root is required")?;
        if !root.is_absolute()
            || root
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        {
            return Err("absolute normalized --root required".into());
        }
        validate_software(&root)
    })();
    Some(match result {
        Ok(v) => {
            if writeln!(stdout, "{v}").is_ok() {
                0
            } else {
                2
            }
        }
        Err(e) => {
            let _ = writeln!(stderr, "standalone_validation_failed: {e}");
            1
        }
    })
}
