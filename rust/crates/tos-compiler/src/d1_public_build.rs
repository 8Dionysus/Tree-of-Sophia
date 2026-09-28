//! Full disposable public D1 bootstrap. This path publishes only SQL/static
//! carriers and a full-row baseline; it cannot mint a selected Stage receipt.

use crate::{
    Error, Limits, NATIVE_KNOWLEDGE_ADAPTER_PROFILES, NativeFamilyInputs, NativeProducerLimits,
    NavigationHeaderClaim, QueryVocabulary, Result,
    catalog::{CatalogLimits, compile_catalog},
    d1_public_capture::{PublicCapture, PublicCaptureLimits},
    d1_public_graph::{
        PublicRepositoryRoot, PublicStageOwner, exact_receipt, ingest_family_rows,
        prepare_family_rows,
    },
    d1_public_header::build_public_header,
    d1_public_knowledge::{KnowledgeSqlCounts, emit_knowledge, portabilize_public_stage},
    d1_public_lens::LensCounts,
    d1_public_lens_specs::saved_lenses,
    d1_public_metadata,
    d1_public_rows::{SourceSqlCounts, emit_corpus, emit_navigation, emit_philosophy},
    d1_public_schema,
    d1_public_semantics::{validate_public_current_registries, validate_public_semantics},
    d1_public_sql::SqlSink,
    knowledge_scope::{ScopeLimits, write_source_scope},
    knowledge_search::{SearchBuildLimits, build_search_index},
    knowledge_stage::{KnowledgeStage, StageLimits, WritePhase},
};
use fs2::FileExt;
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tos_foundation::{Digest256, Digest256Hasher};

#[derive(Clone, Copy, Debug)]
pub struct PublicD1BuildLimits {
    pub capture: PublicCaptureLimits,
    pub stage: StageLimits,
    pub native: NativeProducerLimits,
    pub scope: ScopeLimits,
    pub catalog: CatalogLimits,
    pub search: SearchBuildLimits,
    pub max_sql_bytes: u64,
    pub max_baseline_bytes: u64,
    pub max_static_bytes: u64,
    pub max_lens_auxiliary_bytes: u64,
    pub max_lens_memberships: u64,
    pub max_stage_transfer_work_bytes: u64,
    pub max_build_seconds: u64,
}

impl PublicD1BuildLimits {
    fn validate(self) -> Result<()> {
        self.capture.validate()?;
        self.stage.validate()?;
        let n = self.native;
        n.navigation_prepare.validate()?;
        n.navigation_nodes.validate()?;
        n.navigation_materialize.validate()?;
        n.navigation_dependencies.validate()?;
        n.navigation_relations.validate()?;
        n.claims_prepare.validate()?;
        n.claims.validate()?;
        n.philosophy_prepare.validate()?;
        n.philosophy.validate()?;
        n.titles.validate()?;
        n.inherited.validate()?;
        n.finalize.validate()?;
        self.search.validate()?;
        if self.scope.max_sources == 0
            || self.scope.max_rows == 0
            || self.scope.max_index_work_bytes == 0
            || self.catalog.max_rows == 0
            || self.catalog.max_row_bytes == 0
            || self.catalog.max_catalog_bytes == 0
            || self.catalog.max_catalog_entries == 0
            || self.catalog.max_staging_pages == 0
            || self.max_build_seconds == 0
            || self.max_sql_bytes == 0
            || self.max_baseline_bytes == 0
            || self.max_static_bytes == 0
            || self.max_lens_auxiliary_bytes == 0
            || self.max_lens_memberships == 0
            || self.max_stage_transfer_work_bytes == 0
        {
            return Err(Error::Budget("public D1 build limits"));
        }
        Ok(())
    }
}

/// One local caller profile derived from existing compiler defaults and the
/// maintained Python full-producer caps. Its SQLite page limits are per DB;
/// no aggregate host disk or sorter-spill quota is inferred from them.
pub fn portable_public_d1_limits(max_build_seconds: u64) -> Result<PublicD1BuildLimits> {
    let base = Limits::default();
    let row = base.max_row_bytes;
    let rows = base.max_rows;
    let work = base.max_work_bytes;
    let page_rows = 8usize;
    let page_bytes = page_rows
        .checked_mul(row)
        .ok_or(Error::Budget("public D1 page arithmetic"))?;
    let endpoint_rows = rows
        .checked_mul(2)
        .ok_or(Error::Budget("public D1 endpoint arithmetic"))?;
    let native = NativeProducerLimits {
        navigation_prepare: crate::NavigationPrepareLimits {
            max_nodes: rows,
            max_edges: rows,
            max_endpoint_refs: endpoint_rows,
            max_page_rows: page_rows,
            max_row_bytes: row,
            max_header_bytes: 1024 * 1024,
            max_work_bytes: work,
        },
        navigation_nodes: crate::NavigationNodeLimits {
            max_raw_bytes: row,
            max_output_bytes: row,
            max_ancestor_cache_bytes: 4 * 1024 * 1024,
        },
        navigation_materialize: crate::NavigationMaterializeLimits {
            max_nodes: rows,
            max_edges: rows,
            max_placeholders: rows,
            max_page_rows: page_rows,
            max_raw_bytes: row,
            max_output_bytes: row,
            max_page_bytes: page_bytes,
            max_work_bytes: work,
        },
        navigation_dependencies: crate::NavigationRelationLimits {
            max_edges: rows,
            max_page_rows: page_rows,
            max_raw_bytes: row,
            max_context_bytes: row,
            max_page_bytes: page_bytes,
            max_work_bytes: work,
        },
        navigation_relations: crate::NavigationRelationNormalizeLimits {
            max_raw_bytes: row,
            max_output_bytes: row,
            max_registry_bytes: 4 * 1024 * 1024,
            max_claim_contexts: 4096,
            max_global_input_bytes: row,
        },
        claims_prepare: crate::ClaimPrepareLimits {
            max_nodes: rows,
            max_edges: rows,
            max_claims: rows,
            max_page_rows: page_rows,
            max_row_bytes: row,
            max_work_bytes: work,
        },
        claims: crate::knowledge_source_claims::ClaimNormalizeLimits {
            max_raw_bytes: row,
            max_output_bytes: row,
            max_page_rows: page_rows,
            max_contexts: 4096,
            max_work_bytes: work,
        },
        philosophy_prepare: crate::PhilosophyPrepareLimits {
            max_nodes: rows,
            max_edges: rows,
            max_edge_view_bindings: rows,
            max_page_rows: page_rows,
            max_row_bytes: row,
            max_work_bytes: work,
        },
        philosophy: crate::PhilosophyMaterializeLimits {
            max_raw_bytes: row,
            max_output_bytes: row,
            max_registry_bytes: 4 * 1024 * 1024,
            max_page_rows: page_rows,
            max_rows: rows,
            max_work_bytes: work,
        },
        titles: crate::GlobalTitleLimits {
            max_nodes: rows,
            max_page_rows: page_rows,
            max_page_bytes: page_bytes,
            max_node_bytes: row,
            max_title_bytes: 64 * 1024,
            max_work_bytes: work,
        },
        inherited: crate::InheritedViewLimits {
            max_relations: rows,
            max_endpoint_evidence_rows: endpoint_rows,
            max_view_tokens: rows,
            max_page_rows: page_rows,
            max_page_bytes: page_bytes,
            max_row_bytes: row,
            max_work_bytes: work,
        },
        finalize: crate::NativeFinalizeLimits {
            max_rows: rows,
            max_page_rows: page_rows,
            max_page_bytes: page_bytes,
            max_row_bytes: row,
            max_view_ids_per_node: 64,
            max_context_sources: 64,
            max_work_bytes: work,
        },
    };
    let limits = PublicD1BuildLimits {
        capture: PublicCaptureLimits {
            max_input_bytes: base.max_output_bytes,
            max_rows: rows,
            max_staging_bytes: base.max_output_bytes,
            max_work_bytes: work,
            max_sql_vm_steps: base.max_sql_vm_steps,
            sqlite_cache_kib: base.sqlite_cache_kib,
        },
        stage: StageLimits {
            sqlite: base,
            max_temp_bytes: base.max_output_bytes,
            max_seek_rows: 1024,
            max_seek_bytes: page_bytes as u64,
        },
        native,
        scope: ScopeLimits {
            max_sources: NATIVE_KNOWLEDGE_ADAPTER_PROFILES.len(),
            max_rows: rows,
            max_index_work_bytes: work,
        },
        catalog: CatalogLimits::default(),
        search: SearchBuildLimits {
            max_payload_bytes: row,
            max_document_chars: row,
            max_document_bytes: page_bytes,
            max_rank_field_bytes: row,
            max_postings: 10_000_000,
            max_work_bytes: work,
            gram_batch_rows: 64,
        },
        max_sql_bytes: work,
        max_baseline_bytes: work,
        max_static_bytes: work,
        max_lens_auxiliary_bytes: 1024 * 1024 * 1024,
        max_lens_memberships: 2_000_000,
        max_stage_transfer_work_bytes: work,
        max_build_seconds,
    };
    limits.validate()?;
    Ok(limits)
}

pub struct PublicD1Build<'a> {
    pub source_root: &'a Path,
    pub output: &'a Path,
    pub runtime: &'a Path,
    pub limits: PublicD1BuildLimits,
}

fn fresh_name(prefix: &str) -> Result<String> {
    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Error::Invalid("public D1 build clock"))?
        .as_nanos();
    Ok(format!(".{prefix}-{}-{tick}", std::process::id()))
}

struct PrivateDirectory {
    path: PathBuf,
}
impl Drop for PrivateDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
fn private_directory(parent: &Path, prefix: &str) -> Result<PrivateDirectory> {
    let path = parent.join(fresh_name(prefix)?);
    fs::create_dir(&path)?;
    Ok(PrivateDirectory { path })
}

fn canonical_endpoint(path: &Path) -> Result<bool> {
    let canonical = if path.exists() {
        fs::canonicalize(path)?
    } else {
        let parent = path
            .parent()
            .ok_or(Error::Invalid("public D1 path parent"))?;
        let name = path
            .file_name()
            .ok_or(Error::Invalid("public D1 path name"))?;
        fs::canonicalize(parent)?.join(name)
    };
    Ok(canonical == path)
}

fn checked_paths(root: &Path, output: &Path, runtime: &Path) -> Result<()> {
    if !root.is_absolute()
        || !output.is_absolute()
        || !runtime.is_absolute()
        || !root.is_dir()
        || root.is_symlink()
        || output == runtime
        || output.starts_with(runtime)
        || runtime.starts_with(output)
        || root.starts_with(output)
        || root.starts_with(runtime)
        || output == Path::new("/")
        || runtime == Path::new("/")
        || output.is_symlink()
        || runtime.is_symlink()
        || !canonical_endpoint(root)?
        || !canonical_endpoint(output)?
        || !canonical_endpoint(runtime)?
    {
        return Err(Error::Invalid("public D1 unsafe output/runtime paths"));
    }
    let worker = root.join("access/deploy/cloudflare-worker");
    if output.starts_with(root) && output != worker.join("dist") {
        return Err(Error::Invalid("public D1 output inside source"));
    }
    if runtime.starts_with(root) && runtime != worker.join("runtime") {
        return Err(Error::Invalid("public D1 runtime inside source"));
    }
    Ok(())
}

fn input(capture: &PublicCapture, path: &str, cap: usize) -> Result<Vec<u8>> {
    capture
        .read_input(path, cap)?
        .ok_or(Error::Invalid("public D1 required source absent"))
}

fn processor_binding(
    root: &PublicRepositoryRoot,
    descriptor: &[u8],
    entity: &[u8],
    relation: &[u8],
) -> Result<(Digest256, Digest256)> {
    let processor = Digest256::from_hex(root.software_binding())
        .map_err(|_| Error::Invalid("public D1 processor software binding"))?;
    let mut configuration = Digest256Hasher::new();
    for raw in [descriptor, entity, relation] {
        configuration.update(&(raw.len() as u64).to_be_bytes());
        configuration.update(raw);
    }
    Ok((processor, configuration.finalize()))
}

/// Build a complete full-only publication from one captured public snapshot.
/// Existing completion markers are invalidated before work and recreated last.
pub fn build_public_d1(request: PublicD1Build<'_>) -> Result<Value> {
    let PublicD1Build {
        source_root,
        output,
        runtime,
        limits,
    } = request;
    checked_paths(source_root, output, runtime)?;
    limits.validate()?;
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(limits.max_build_seconds))
        .ok_or(Error::Budget("public D1 deadline"))?;
    fs::create_dir_all(runtime)?;
    let lock = OpenOptions::new()
        .create(true)
        .append(true)
        .open(runtime.join("build.lock"))?;
    lock.try_lock_exclusive()
        .map_err(|_| Error::Invalid("another edge build owns runtime"))?;
    for marker in [
        runtime.join("manifest.json"),
        output.join("__edge/build-manifest.json"),
    ] {
        if marker.is_symlink() {
            return Err(Error::Invalid("public D1 completion marker symlink"));
        }
        match fs::remove_file(&marker) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    let private = private_directory(runtime, "tos-public-d1")?;
    let capture = PublicCapture::create(
        source_root,
        &private.path.join("capture.sqlite3"),
        limits.capture,
        deadline,
    )?;
    let source_revision = if capture.partitioned() {
        capture.partitioned_source_revision()?
    } else {
        capture.legacy_source_revision()?
    };
    let entity = input(
        &capture,
        "ToS/doctrine/semantic-interchange/entity-types.v1.json",
        4 * 1024 * 1024,
    )?;
    let relation = input(
        &capture,
        "ToS/doctrine/semantic-interchange/relation-types.v1.json",
        4 * 1024 * 1024,
    )?;
    let descriptor = input(
        &capture,
        "ToS/doctrine/semantic-interchange/query-vocabulary.v1.json",
        4 * 1024 * 1024,
    )?;
    let registry = validate_public_current_registries(&capture, &entity, &relation)?;
    let vocabulary = QueryVocabulary::parse(&descriptor, NATIVE_KNOWLEDGE_ADAPTER_PROFILES)?;
    prepare_family_rows(&capture, limits.capture)?;
    let receipt = exact_receipt(&capture, &vocabulary, &source_revision)?;
    let owner = PublicStageOwner {
        capture: &capture,
        receipt: receipt.clone(),
    };
    let mut stage = KnowledgeStage::create_public_build(
        &private.path.join("public-stage.sqlite3"),
        limits.stage,
        receipt,
        &owner,
        capture.vm_counter(),
        capture.work_counter(),
        capture.max_work_bytes(),
        capture.deadline(),
    )?;
    ingest_family_rows(&mut stage, &capture, limits.max_stage_transfer_work_bytes)?;
    let navigation = capture.header_object("corpus", "source_navigation", 1024 * 1024)?;
    let nav_raw = serde_json::to_vec(&navigation).map_err(|e| Error::Source(e.to_string()))?;
    let nav_claim = NavigationHeaderClaim {
        expected_sha256: Digest256::of_bytes(&nav_raw).to_hex(),
        raw_json: nav_raw,
    };
    let public_root = PublicRepositoryRoot::new(&stage, &source_revision)?;
    let mut additional = NativeFamilyInputs::bounded_from(limits.native);
    additional.repository_root = Some(public_root.input());
    let native = crate::materialize_native_sources_with_inputs(
        &mut stage,
        &registry,
        &entity,
        &relation,
        &vocabulary,
        &descriptor,
        &nav_claim,
        limits.native,
        additional,
    )?;
    portabilize_public_stage(&mut stage, &capture, source_root)?;
    let semantics = validate_public_semantics(&mut stage, &capture, &registry, &entity, &relation)?;
    let (processor, configuration) =
        processor_binding(&public_root, &descriptor, &entity, &relation)?;
    let header = build_public_header(
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
    let scope = write_source_scope(&mut stage, &vocabulary, limits.scope)?;
    let lenses = saved_lenses(&capture)?;
    let entity_value: Value =
        serde_json::from_slice(&entity).map_err(|e| Error::Source(e.to_string()))?;
    let relation_value: Value =
        serde_json::from_slice(&relation).map_err(|e| Error::Source(e.to_string()))?;
    let catalog = stage.with_connection(WritePhase::Catalog, |db| {
        compile_catalog(
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
    let remaining_work = capture
        .max_work_bytes()
        .checked_sub(capture.work_bytes())
        .ok_or(Error::Budget("public D1 build work bytes"))?;
    let mut search_limits = limits.search;
    search_limits.max_work_bytes = search_limits.max_work_bytes.min(remaining_work);
    if search_limits.max_work_bytes == 0 {
        return Err(Error::Budget("public D1 Search work bytes"));
    }
    let search = build_search_index(&mut stage, search_limits)?;
    capture.charge_work(search.work_bytes)?;
    let metadata = d1_public_metadata::prepare(
        &mut stage,
        &capture,
        source_root,
        &header,
        &catalog.catalog,
        &source_revision,
    )?;
    let mut sink = SqlSink::create(
        &private.path.join("read-model.sql.next"),
        &private.path.join("read-model.rows.index.sqlite"),
        limits.max_sql_bytes,
        &capture,
        limits.capture,
    )?;
    d1_public_schema::begin(&mut sink)?;
    let values = d1_public_metadata::values(
        &capture,
        source_root,
        &header,
        &catalog.catalog,
        &metadata,
        &navigation,
    )?;
    d1_public_metadata::emit_edge_meta(&mut sink, &capture, &values)?;
    let mut source_counts = SourceSqlCounts::default();
    emit_philosophy(&capture, &mut sink, source_root, &mut source_counts)?;
    emit_corpus(&capture, &mut sink, source_root, &mut source_counts)?;
    emit_navigation(&capture, &mut sink, source_root, &mut source_counts)?;
    let mut knowledge_counts = KnowledgeSqlCounts::default();
    let mut lens_counts = LensCounts::default();
    emit_knowledge(
        &mut stage,
        &capture,
        &mut sink,
        source_root,
        limits.search,
        &mut knowledge_counts,
        &mut lens_counts,
        limits.max_lens_auxiliary_bytes,
        limits.max_lens_memberships,
    )?;
    if knowledge_counts.nodes != scope.node_count
        || knowledge_counts.relations != scope.relation_count
        || knowledge_counts.postings != search.postings
    {
        return Err(Error::Invalid("public D1 source/search coverage"));
    }
    d1_public_schema::finish(
        &mut sink,
        &metadata.binding_prefix,
        &metadata.binding_suffix,
    )?;
    let (sql, baseline, sql_bytes, statements, baseline_bytes) = sink.finish(
        &private.path.join("read-model.rows.json.next"),
        &metadata.revision,
        &metadata.reader_top,
        limits.max_baseline_bytes,
    )?;
    stage.complete_public_build(scope.node_count, scope.relation_count)?;
    // The normalized Stage is disposable. Release its DB, journal and TEMP
    // lifetime before copying web assets and assembling static companions.
    drop(stage);
    let static_dir = private_directory(
        output
            .parent()
            .ok_or(Error::Invalid("public D1 output parent"))?,
        "tos-public-static",
    )?;
    let static_summary = crate::d1_public_static::build(
        &capture,
        &static_dir.path,
        source_root,
        &header,
        &catalog.catalog,
        limits.max_static_bytes,
    )?;
    capture.verify_inputs(limits.capture)?;
    static_summary.verify_web_inputs(&capture)?;
    let counts = json!({
        "philosophy_nodes":source_counts.philosophy_nodes,"philosophy_edges":source_counts.philosophy_edges,
        "philosophy_clusters":source_counts.cluster_node_memberships,
        "corpus_items":source_counts.corpus_items,"corpus_edges":source_counts.corpus_edges,
        "corpus_packs":source_counts.corpus_packs,
        "knowledge_nodes":knowledge_counts.nodes,"knowledge_relations":knowledge_counts.relations,
        "knowledge_search_postings":knowledge_counts.postings,
        "knowledge_search_schema":"tos_knowledge_search_read_model_v3",
        "knowledge_compact_rows":lens_counts.compact_rows,
        "knowledge_lens_memberships":lens_counts.membership_rows,
        "knowledge_lens_auxiliary_bytes":lens_counts.auxiliary_bytes,
        "source_navigation_nodes":source_counts.navigation_nodes,
        "source_navigation_edges":source_counts.navigation_edges,
        "source_navigation_rights":source_counts.navigation_rights,
        "sql_statements":statements,"delta":null
    });
    let native_derived_rows = native
        .final_rows
        .nodes
        .checked_add(native.final_rows.relations)
        .ok_or(Error::Budget("public D1 native row count"))?;
    let mut manifest = json!({
        "schema":"tos_cloudflare_edge_build_v1",
        "read_model_schema":"tos_cloudflare_edge_read_model_v9",
        "data_revision":metadata.revision,
        "processing":{"status":"not-run","reason":"rust-offline-public-compiler-owned",
            "executed":0,"reused":0,"is_semantic_acceptance":false},
        "build_stages":{"read-model":"computed","static-responses":"computed"},
        "source_owner":"Tree-of-Sophia",
        "source_paths":capture.source_labels(),
        "producer_paths":["rust/crates/tos-compiler/src/d1_public_build.rs",
            "rust/crates/tos-compiler/src/d1_public_capture.rs",
            "rust/crates/tos-compiler/src/d1_public_rows.rs",
            "rust/crates/tos-compiler/src/d1_public_knowledge.rs"],
        "contract_refs":["access/contracts/knowledge-api.v1.json",
            "access/contracts/knowledge-graph.v1.schema.json",
            "access/contracts/lens-spec.v1.schema.json",
            "access/contracts/lens-result.v1.schema.json"],
        "revision_policy":{"imports_when":"source data, normalized item content, capability data, published catalog, or read-model schema changes",
            "does_not_import_when":"only documentation or Worker-only code changes"},
        "counts":counts,
        "normalization_cache":{"reused_steps":0,"computed_steps":0},
        "default_corpus_view":static_summary.default_corpus_view,
        "default_philosophy_view":static_summary.default_philosophy_view,
        "authority_limit":"Cloudflare carries a generated read model; ToS authored and reviewed surfaces remain authoritative.",
        "public_source_scope_sha256":scope.source_scope_root_sha256,
        "native_derived_rows":native_derived_rows,
        "sql_bytes":sql_bytes,"baseline_bytes":baseline_bytes,
    });
    // The existing 2 MiB completion envelope includes both the fixed
    // manifest fields and the measured input binding. Admit that combined
    // serialized size before constructing the potentially long binding.
    let prior = serde_json::to_vec(&manifest).map_err(|e| Error::Source(e.to_string()))?;
    let binding_key_bytes = b",\"public_input_binding\":".len();
    let binding_bytes = crate::d1_public_capture::MAX_HEADER_BYTES
        .checked_sub(prior.len())
        .and_then(|remaining| remaining.checked_sub(binding_key_bytes))
        .ok_or(Error::Budget("public D1 manifest bytes"))?;
    capture.charge_work(prior.len() as u64)?;
    manifest
        .as_object_mut()
        .ok_or(Error::Invalid("public D1 manifest object"))?
        .insert(
            "public_input_binding".to_owned(),
            capture.manifest_input_binding(binding_bytes)?,
        );
    crate::d1_public_publication::publish(
        output,
        runtime,
        &static_dir.path,
        &sql,
        &baseline,
        &manifest,
        &capture,
    )?;
    Ok(manifest)
}
