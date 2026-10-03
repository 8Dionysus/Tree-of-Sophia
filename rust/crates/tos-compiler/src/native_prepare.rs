//! Explicit native source bootstrap into the maintained prepared directory ABI.
//! The internal disposable stage is computational custody, never selected authority.
use crate::{
    Error, NATIVE_KNOWLEDGE_ADAPTER_PROFILES, NativeFamilyInputs, NavigationHeaderClaim,
    QueryVocabulary, Result,
    d1_public_build::portable_public_d1_limits,
    d1_public_capture::{PublicCapture, compact, json},
    d1_public_graph::{
        PublicRepositoryRoot, PublicStageOwner, exact_receipt, ingest_family_rows,
        prepare_family_rows,
    },
    d1_public_header::build_public_header,
    d1_public_knowledge::portabilize_public_stage,
    d1_public_lens_specs::saved_lenses,
    d1_public_semantics::{validate_public_current_registries, validate_public_semantics},
    knowledge_stage::{KnowledgeStage, WritePhase},
    local_prepared::{self, BootstrapSearch, PreparedRows, PublicationLimits},
    local_prepared_bulk::BulkBootstrapLimits,
    prepared_catalog_index::CatalogMaintenanceLimits,
    prepared_catalog_semantics::{CatalogInputs, SourceOrderProfile},
    prepared_semantic_index::{SemanticMaintenanceLimits, SemanticRows},
};
use serde_json::{Value, json as packet};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::Path,
    time::{Duration, Instant},
};
use tos_foundation::{Digest256, Digest256Hasher, JsonValue};
pub const SCHEMA: &str = "tos_offline_prepared_bootstrap_receipt_v1";
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaintenanceAttachmentLimits {
    pub max_mutations: u64,
    pub catalog_limits: CatalogMaintenanceLimits,
    pub semantic_limits: SemanticMaintenanceLimits,
}
impl Default for MaintenanceAttachmentLimits {
    fn default() -> Self {
        Self {
            max_mutations: 2_000_000,
            catalog_limits: CatalogMaintenanceLimits::default(),
            semantic_limits: SemanticMaintenanceLimits::default(),
        }
    }
}
/// Explicit computational file envelopes, independent of publication caps.
/// Absence retains existing production compiler defaults; these grant no space.
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceBootstrapLimits {
    pub max_input_bytes: u64,
    pub max_capture_bytes: u64,
    pub max_stage_bytes: u64,
    pub max_temp_bytes: u64,
}
impl SourceBootstrapLimits {
    fn apply(self, limits: &mut crate::d1_public_build::PublicD1BuildLimits) -> Result<()> {
        if self.max_input_bytes == 0
            || self.max_capture_bytes < 65536
            || self.max_stage_bytes < 65536
            || self.max_temp_bytes < 65536
            || self.max_input_bytes > limits.capture.max_input_bytes
            || self.max_capture_bytes > limits.capture.max_staging_bytes
            || self.max_stage_bytes > limits.stage.sqlite.max_output_bytes
            || self.max_temp_bytes > limits.stage.max_temp_bytes
        {
            return Err(Error::Budget("prepare explicit computational envelopes"));
        }
        limits.capture.max_input_bytes = self.max_input_bytes;
        limits.capture.max_staging_bytes = self.max_capture_bytes;
        limits.stage.sqlite.max_output_bytes = self.max_stage_bytes;
        limits.stage.max_temp_bytes = self.max_temp_bytes;
        Ok(())
    }
}
// Native prepare alone binds normalization to the actual executing ELF. The
// D1 publisher's accepted processor identity and public receipt are unchanged.
// Match the selected CI software-image envelope; this bounds one streaming
// identity read, not source, staging, publication, or allocated image memory.
const MAX_EXECUTING_IMAGE_BYTES: u64 = 1024 * 1024 * 1024;
struct ExecutingProcessor {
    file: File,
    path: std::path::PathBuf,
    stamp: (u64, u64, u64, i64, i64, i64, i64),
    digest: Digest256,
    deadline: Instant,
}
fn executable_stamp(m: &fs::Metadata) -> Result<(u64, u64, u64, i64, i64, i64, i64)> {
    if !m.is_file() || m.len() == 0 || m.len() > MAX_EXECUTING_IMAGE_BYTES {
        return Err(Error::Budget("prepare executing ELF file envelope"));
    }
    Ok((
        m.dev(),
        m.ino(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    ))
}
impl ExecutingProcessor {
    fn measure(deadline: Instant) -> Result<Self> {
        let path = std::env::current_exe()?;
        // /proc/self/exe is the running image, rather than a PATH selection or
        // a caller-provided string. Keep that inode open for the whole build.
        let file = File::open("/proc/self/exe")?;
        let stamp = executable_stamp(&file.metadata()?)?;
        let mut result = Self {
            file,
            path,
            stamp,
            digest: Digest256::of_bytes(&[]),
            deadline,
        };
        result.digest = result.hash()?;
        result.verify_stamp()?;
        Ok(result)
    }
    fn verify_stamp(&self) -> Result<()> {
        deadline_check(self.deadline)?;
        if executable_stamp(&self.file.metadata()?)? != self.stamp
            || executable_stamp(&fs::metadata(&self.path)?)? != self.stamp
        {
            return Err(Error::Invalid(
                "prepare executing ELF physical identity changed",
            ));
        }
        Ok(())
    }
    fn hash(&mut self) -> Result<Digest256> {
        self.verify_stamp()?;
        self.file.seek(SeekFrom::Start(0))?;
        let mut head = [0u8; 4];
        self.file.read_exact(&mut head)?;
        if head != *b"\x7fELF" {
            return Err(Error::Invalid("prepare executing image is not ELF"));
        }
        let mut hash = Digest256Hasher::new();
        hash.update(&head);
        let mut total = 4u64;
        let mut block = [0u8; 65536];
        loop {
            deadline_check(self.deadline)?;
            let count = self.file.read(&mut block)?;
            if count == 0 {
                break;
            }
            total = total
                .checked_add(count as u64)
                .filter(|n| *n <= self.stamp.2)
                .ok_or(Error::Budget("prepare ELF read bytes"))?;
            hash.update(&block[..count]);
        }
        if total != self.stamp.2 {
            return Err(Error::Invalid("prepare ELF changed during read"));
        }
        self.verify_stamp()?;
        Ok(hash.finalize())
    }
}
fn configuration_binding(descriptor: &[u8], entity: &[u8], relation: &[u8]) -> Digest256 {
    let mut hash = Digest256Hasher::new();
    for raw in [descriptor, entity, relation] {
        hash.update(&(raw.len() as u64).to_be_bytes());
        hash.update(raw);
    }
    hash.finalize()
}

pub struct PrepareRequest<'a> {
    pub source_root: &'a Path,
    pub output_dir: &'a Path,
    pub publication: PublicationLimits,
    pub search_scratch: Option<BulkBootstrapLimits>,
    pub maintenance: Option<MaintenanceAttachmentLimits>,
    pub max_seconds: u64,
    pub source_limits: Option<SourceBootstrapLimits>,
}
fn deadline_check(deadline: Instant) -> Result<()> {
    if Instant::now() >= deadline {
        Err(Error::Budget("native prepare whole deadline"))
    } else {
        Ok(())
    }
}
fn operand(path: &Path) -> Result<()> {
    if !path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(Error::Invalid("native prepare absolute explicit operand"));
    }
    // Reject symlink ancestors before creation, preserving the selected namespace.
    let mut prefix = std::path::PathBuf::new();
    for component in path.components() {
        prefix.push(component.as_os_str());
        match fs::symlink_metadata(&prefix) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(Error::Invalid("native prepare symlink operand"));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && prefix == path => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
fn sync_dir(path: &Path) -> Result<()> {
    File::open(path)?.sync_all()?;
    Ok(())
}
fn exclusive_json(path: &Path, value: &JsonValue, cap: usize) -> Result<()> {
    let mut raw = compact(value, cap)?;
    raw.push(b'\n');
    let temporary = path.with_file_name(format!(
        ".{}.partial",
        path.file_name()
            .and_then(|s| s.to_str())
            .ok_or(Error::Invalid("prepare marker name"))?
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)?;
    let original = file.metadata()?;
    file.write_all(&raw)?;
    file.sync_all()?;
    drop(file);
    // Link never replaces any completion marker, even after interrupted attempts.
    fs::hard_link(&temporary, path)?;
    let result = (|| {
        fs::remove_file(&temporary)?;
        sync_dir(
            path.parent()
                .ok_or(Error::Invalid("prepare marker parent"))?,
        )
    })();
    if result.is_err() {
        if let Ok(current) = fs::symlink_metadata(path) {
            if current.dev() == original.dev() && current.ino() == original.ino() {
                let _ = fs::remove_file(path);
            }
        }
    }
    result
}
fn value(packet: &Value, cap: usize) -> Result<JsonValue> {
    let raw = serde_json::to_vec(packet).map_err(|e| Error::Source(e.to_string()))?;
    json(&raw, cap)
}
// Reuse the existing portable walker at its foundation JSON boundary. The
// catalog renderer still consumes serde values, with the same metadata cap.
fn portable_serde(packet: &mut Value, root: &str, cap: usize) -> Result<()> {
    let mut owner = value(packet, cap)?;
    crate::d1_public_rows::portable(&mut owner, root);
    *packet = value_to_serde(&owner, cap)?;
    Ok(())
}
struct StageRows<'s, 'a> {
    stage: &'s mut KnowledgeStage<'a>,
    capture: &'s PublicCapture,
    cap: usize,
}
impl StageRows<'_, '_> {
    fn visit_rows(
        &mut self,
        kind: &str,
        sink: &mut dyn FnMut(&JsonValue) -> Result<()>,
    ) -> Result<()> {
        let table = match kind {
            "node" => "knowledge_nodes",
            "relation" => "knowledge_relations",
            _ => return Err(Error::Invalid("prepare row kind")),
        };
        self.stage.with_connection(WritePhase::Catalog, |db| {
            let mut statement = db.prepare(&format!("SELECT payload_len,payload_sha256,CASE WHEN payload_len<=?1 AND length(payload)=payload_len THEN payload ELSE NULL END FROM {table} ORDER BY source_order"))?;
            let mut rows = statement.query([self.cap as i64])?;
            while let Some(row) = rows.next()? {
                let length: i64 = row.get(0)?;
                if length < 0 || length as u64 > self.cap as u64 { return Err(Error::Budget("prepare selected row bytes")); }
                self.capture.charge_work((length as u64).checked_mul(4).ok_or(Error::Budget("prepare row copy work"))?)?;
                let digest: Vec<u8> = row.get(1)?;
                let raw: Option<Vec<u8>> = row.get(2)?;
                let raw = raw.ok_or(Error::Invalid("prepare selected row bytes"))?;
                if raw.len() != length as usize || digest.as_slice() != Digest256::of_bytes(&raw).as_bytes() { return Err(Error::Invalid("prepare stage row digest")); }
                sink(&json(&raw, self.cap)?)?;
            }
            Ok(())
        })
    }
}
impl PreparedRows for StageRows<'_, '_> {
    fn visit(&mut self, kind: &str, sink: &mut dyn FnMut(&JsonValue) -> Result<()>) -> Result<()> {
        self.visit_rows(kind, sink)
    }
}
impl SemanticRows for StageRows<'_, '_> {
    fn visit(&mut self, kind: &str, sink: &mut dyn FnMut(&JsonValue) -> Result<()>) -> Result<()> {
        self.visit_rows(kind, sink)
    }
}
/// Fresh private directory, completion marker last. Failed attempts remain
/// visible and cannot be selected as completed; no currentness promise follows.
pub fn prepare(request: PrepareRequest<'_>) -> Result<Value> {
    let PrepareRequest {
        source_root,
        output_dir,
        publication,
        search_scratch,
        maintenance,
        max_seconds,
        source_limits,
    } = request;
    publication.validate()?;
    if let Some(scratch) = search_scratch {
        scratch.validate()?;
    }
    if let Some(m) = maintenance {
        m.catalog_limits.validate()?;
        m.semantic_limits.validate()?;
        PublicationLimits {
            max_mutations: m.max_mutations,
            ..publication
        }
        .validate()?;
    }
    if max_seconds == 0 {
        return Err(Error::Budget("prepare explicit whole deadline"));
    }
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(max_seconds))
        .ok_or(Error::Budget("prepare deadline arithmetic"))?;
    let mut limits = portable_public_d1_limits(max_seconds)?;
    if let Some(profile) = source_limits {
        profile.apply(&mut limits)?;
    }
    let mut processor_owner = ExecutingProcessor::measure(deadline)?;
    operand(source_root)?;
    operand(output_dir)?;
    let root = fs::canonicalize(source_root)?;
    if !root.is_dir() {
        return Err(Error::Invalid("prepare source directory"));
    }
    let parent = output_dir
        .parent()
        .ok_or(Error::Invalid("prepare output parent"))?;
    if !parent.is_dir() {
        return Err(Error::Invalid("prepare separate fresh output namespace"));
    }
    // No recursive parent creation, replacement, cleanup or retry.
    fs::DirBuilder::new().mode(0o700).create(output_dir)?;
    sync_dir(parent)?;
    let scratch_dir = output_dir.join(".native-source-bootstrap");
    fs::DirBuilder::new().mode(0o700).create(&scratch_dir)?;
    let capture = PublicCapture::create_prepared(
        &root,
        &scratch_dir.join("capture.sqlite3"),
        limits.capture,
        deadline,
    )?;
    let source_revision = if capture.partitioned() {
        capture.partitioned_source_revision()?
    } else {
        capture.legacy_source_revision()?
    };
    let read = |label: &str| {
        capture
            .read_input(label, 4 * 1024 * 1024)?
            .ok_or(Error::Invalid("prepare required companion"))
    };
    let entity = read("ToS/doctrine/semantic-interchange/entity-types.v1.json")?;
    let relation = read("ToS/doctrine/semantic-interchange/relation-types.v1.json")?;
    let descriptor = read("ToS/doctrine/semantic-interchange/query-vocabulary.v1.json")?;
    let registry = validate_public_current_registries(&capture, &entity, &relation)?;
    let vocabulary = QueryVocabulary::parse(&descriptor, NATIVE_KNOWLEDGE_ADAPTER_PROFILES)?;
    prepare_family_rows(&capture, limits.capture)?;
    let receipt = exact_receipt(&capture, &vocabulary, &source_revision)?;
    let owner = PublicStageOwner {
        capture: &capture,
        receipt: receipt.clone(),
    };
    let mut stage = KnowledgeStage::create_public_build(
        &scratch_dir.join("normalized.sqlite3"),
        limits.stage,
        receipt,
        &owner,
        capture.vm_counter(),
        capture.work_counter(),
        capture.max_work_bytes(),
        deadline,
    )?;
    ingest_family_rows(&mut stage, &capture, limits.max_stage_transfer_work_bytes)?;
    // Maintained prepare consumes navigation collections without requiring a
    // detached owner header. Adapt their captured counts to the native
    // computational claim; do not relabel or modify captured source metadata.
    let navigation_counts = {
        let db = capture.read_db()?;
        let mut count =
            db.prepare("SELECT count(*) FROM capture_rows WHERE role='corpus' AND collection=?1")?;
        let mut counts = serde_json::Map::new();
        for name in ["nodes", "edges", "rights"] {
            let n: u64 = count.query_row([format!("source_navigation/{name}")], |r| r.get(0))?;
            counts.insert(name.to_owned(), Value::from(n));
        }
        counts
    };
    let navigation = packet!({
        "schema_version": "tos_source_navigation_v1",
        "authority_boundary": "captured prepare collections; computational normalization only",
        "counts": navigation_counts,
    });
    let nav_raw = serde_json::to_vec(&navigation).map_err(|e| Error::Source(e.to_string()))?;
    let nav = NavigationHeaderClaim {
        expected_sha256: Digest256::of_bytes(&nav_raw).to_hex(),
        raw_json: nav_raw,
    };
    let public_root = PublicRepositoryRoot::new(&stage, &source_revision)?;
    let mut additional = NativeFamilyInputs::bounded_from(limits.native);
    additional.repository_root = Some(public_root.input());
    additional.prepared_philosophy_projection = true;
    additional.prepared_canon_projection = true;
    processor_owner.verify_stamp()?;
    // The measured executing image encloses the actual row producer call, not
    // a post-hoc replacement of somebody else's normalization header.
    crate::materialize_native_sources_with_inputs(
        &mut stage,
        &registry,
        &entity,
        &relation,
        &vocabulary,
        &descriptor,
        &nav,
        limits.native,
        additional,
    )?;
    processor_owner.verify_stamp()?;
    portabilize_public_stage(&mut stage, &capture, &root)?;
    let semantics = validate_public_semantics(&mut stage, &capture, &registry, &entity, &relation)?;
    let processor = processor_owner.digest;
    let configuration = configuration_binding(&descriptor, &entity, &relation);
    let mut header = build_public_header(
        &mut stage,
        &capture,
        &registry,
        &entity,
        &source_revision,
        processor,
        configuration,
        &semantics,
    )?;
    stage.close_inputs_for_full_components()?;
    let mut lenses = saved_lenses(&capture)?;
    let root_text = root.to_str().ok_or(Error::Invalid("prepare root UTF8"))?;
    portable_serde(&mut header, root_text, publication.max_metadata_bytes)?;
    for lens in &mut lenses {
        portable_serde(lens, root_text, publication.max_metadata_bytes)?;
    }
    let mut header_owner = value(&header, publication.max_metadata_bytes)?;
    // The maintained semantic attachment hashes compact report bytes in
    // Python insertion order. Preserve that existing renderer at the
    // foundation boundary instead of serde's alphabetical object ordering.
    let semantic_raw = crate::prepared_semantic_index::report_raw(
        &header["counts"]["semantic_validation"],
        publication.max_metadata_bytes,
    )?;
    let semantic_owner = json(semantic_raw.as_bytes(), publication.max_metadata_bytes)?;
    let JsonValue::Object(fields) = &mut header_owner else {
        return Err(Error::Invalid("prepare header object"));
    };
    let counts = fields
        .iter_mut()
        .find(|(k, _)| k.as_str() == Some("counts"))
        .map(|(_, v)| v)
        .ok_or(Error::Invalid("prepare header counts"))?;
    let JsonValue::Object(counts) = counts else {
        return Err(Error::Invalid("prepare header counts object"));
    };
    let report = counts
        .iter_mut()
        .find(|(k, _)| k.as_str() == Some("semantic_validation"))
        .map(|(_, v)| v)
        .ok_or(Error::Invalid("prepare header semantic report"))?;
    *report = semantic_owner;
    let lenses_owner = lenses
        .iter()
        .map(|v| value(v, publication.max_metadata_bytes))
        .collect::<Result<Vec<_>>>()?;
    let inputs = CatalogInputs {
        header: header_owner.clone(),
        entity_registry: json(&entity, 4 * 1024 * 1024)?,
        relation_registry: json(&relation, 4 * 1024 * 1024)?,
        lenses: lenses_owner,
        source_order_profile: SourceOrderProfile::OwnerSequence,
    };
    // The maintained exact contributor renderer owns prepared catalog semantics;
    // public D1's emitter/manifest authority is never invoked.
    let mut catalog = stage.with_connection(WritePhase::Catalog, |db| {
        let entity_value: Value =
            serde_json::from_slice(&entity).map_err(|e| Error::Source(e.to_string()))?;
        let relation_value: Value =
            serde_json::from_slice(&relation).map_err(|e| Error::Source(e.to_string()))?;
        crate::catalog::compile_catalog(
            db,
            &header,
            &entity_value,
            &relation_value,
            &lenses,
            &vocabulary,
            &descriptor,
            limits.catalog,
        )
    })?;
    portable_serde(
        &mut catalog.catalog,
        root_text,
        publication.max_metadata_bytes,
    )?;
    let catalog_owner = value(&catalog.catalog, publication.max_metadata_bytes)?;
    capture.verify_inputs(limits.capture)?;
    processor_owner.verify_stamp()?;
    let path = output_dir.join("snapshot.sqlite");
    let search = match search_scratch {
        Some(limits) => BootstrapSearch::Bulk {
            scratch_path: output_dir.join(".search-sort.sqlite"),
            limits,
        },
        None => BootstrapSearch::Buffered,
    };
    let mut rows = StageRows {
        stage: &mut stage,
        capture: &capture,
        cap: publication.max_row_bytes,
    };
    let binding = local_prepared::publish_prepared_rows_with_search_until(
        &path,
        &header_owner,
        &catalog_owner,
        &mut rows,
        publication,
        search,
        deadline,
    )?;
    capture.verify_inputs(limits.capture)?;
    let attached = if let Some(m) = maintenance {
        let whole = publication
            .max_bytes
            .min(m.catalog_limits.max_index_bytes)
            .min(m.semantic_limits.max_bytes);
        let effective = PublicationLimits {
            max_bytes: whole,
            max_mutations: m.max_mutations,
            ..publication
        };
        let cat = CatalogMaintenanceLimits {
            max_index_bytes: whole,
            ..m.catalog_limits
        };
        let sem = SemanticMaintenanceLimits {
            max_bytes: whole,
            max_writes: m.semantic_limits.max_writes.min(m.max_mutations),
            ..m.semantic_limits
        };
        let mut precommit = || {
            processor_owner.verify_stamp()?;
            capture.verify_inputs(limits.capture)
        };
        let result = crate::prepared_maintenance_file::bootstrap_prepared_maintenance_file(
            &path,
            &binding,
            &inputs,
            effective,
            cat,
            sem,
            Some(&mut rows),
            &processor.to_hex(),
            deadline,
            &mut precommit,
        )?;
        if result.binding != binding
            || result.publication_changed
            || result.consumer_switched
            || result.sql_mutations > effective.max_mutations
        {
            return Err(Error::Invalid("prepare maintenance selected binding"));
        }
        capture.verify_inputs(limits.capture)?;
        Some(
            packet!({"schema":"tos_prepared_maintenance_attachment_receipt_v1","status":"attached","mode":"catalog_semantic_indexes",
            "binding":value_to_serde(&binding,publication.max_metadata_bytes)?,"catalog_digest":result.catalog_digest,
            "semantic_report_sha256":result.semantic_report_sha256.ok_or(Error::Invalid("prepare maintenance report"))?,
            "sql_mutations":result.sql_mutations,"declared_limits":m,
            "effective_limits":{"publication":effective,"catalog":cat,"semantic":sem},
            "mutation_budget_upper_bound":publication.max_mutations.checked_add(m.max_mutations).ok_or(Error::Budget("prepare mutation arithmetic"))?,
            "publication_changed":false,"consumer_switched":false,"source_transition_verified":false,"semantic_acceptance":false}),
        )
    } else {
        None
    };
    // The protected running ELF is held open; its exact FD/path identity
    // brackets the initial stream and all work. Recheck that custody here,
    // without paying a second whole-image hash inside the same deadline.
    processor_owner.verify_stamp()?;
    deadline_check(deadline)?;
    let mut result = packet!({"schema":SCHEMA,"status":"completed","mode":"full_bootstrap",
        "source_root":root.to_str().ok_or(Error::Invalid("prepare root UTF8"))?,"output_dir":output_dir.to_str().ok_or(Error::Invalid("prepare output UTF8"))?,
        "source_revision":source_revision,"normalization_binding":header["normalization_binding"],
        "source_state_checked":true,"ongoing_currentness_granted":false,"normalization_cache":"disabled","consumer_switched":false,
        "snapshot":"snapshot.sqlite","binding_file":"binding.json","binding":value_to_serde(&binding,publication.max_metadata_bytes)?,
        "publication_limits":publication,"search_bootstrap":if search_scratch.is_some(){"bulk"}else{"buffered"},"search_scratch_limits":search_scratch,
        "build_counts":{"nodes":header["counts"]["nodes"],"relations":header["counts"]["relations"],"snapshot_bytes":fs::metadata(&path)?.len()}});
    if let Some(attached) = attached {
        result["maintenance"] = attached;
    }
    // Drop private SQLite holders before removing only this attempt's own
    // ephemeral files. Failed attempts keep them for diagnosis.
    drop(rows);
    drop(stage);
    drop(owner);
    drop(capture);
    // KnowledgeStage drops its own exact candidate inode and lease.
    if scratch_dir.join("normalized.sqlite3").exists() {
        return Err(Error::Invalid("prepare stage cleanup failed"));
    }
    fs::remove_file(scratch_dir.join("capture.sqlite3"))?;
    fs::remove_dir(&scratch_dir)?;
    exclusive_json(
        &output_dir.join("binding.json"),
        &binding,
        publication.max_metadata_bytes,
    )?;
    deadline_check(deadline)?;
    exclusive_json(
        &output_dir.join("completed.json"),
        &value(&result, publication.max_metadata_bytes)?,
        publication.max_metadata_bytes,
    )?;
    Ok(result)
}
fn value_to_serde(value: &JsonValue, cap: usize) -> Result<Value> {
    serde_json::from_slice(&compact(value, cap)?).map_err(|e| Error::Source(e.to_string()))
}
