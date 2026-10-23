//! Actual local Item commit followed by the maintained source producer,
//! offline prepared publisher and transaction-bound query consumer.
use super::*;
use std::collections::BTreeMap;
use tos_compiler::knowledge_stage::{
    ExactInputReceipt, KnowledgeStage, StageIsolation, StageLimits, StageOwner, WritePhase,
};
use tos_compiler::source_bibliographic::{
    BibliographicLimits, BibliographicSink, BibliographicSourceCut, render_bibliographic_graph,
};
use tos_compiler::source_witness_catalog::{SourceCatalogLimits, SourceCatalogValidator};
use tos_compiler::{SourceBinding, SourceCatalogInputLimits};
use tos_validation::executor::{BatchStreamBudget, ExactWorkerIdentity, ExecutorBudget};
use tos_validation::source_cut::CutWorkerLimits;

struct SelectedCutOwner<'a> {
    cut: &'a CorpusCutReader,
    receipt: ExactInputReceipt,
}
impl StageOwner for SelectedCutOwner<'_> {
    fn verify_receipt(&self, receipt: &ExactInputReceipt) -> tos_compiler::Result<()> {
        if serde_json::to_vec(&receipt.binding).unwrap()
            != serde_json::to_vec(&self.receipt.binding).unwrap()
            || serde_json::to_vec(&receipt.collections).unwrap()
                != serde_json::to_vec(&self.receipt.collections).unwrap()
        {
            return Err(tos_compiler::Error::Invalid(
                "Item fixture exact producer receipt",
            ));
        }
        self.recheck_sealed_cut(receipt)
    }
    fn recheck_sealed_cut(&self, receipt: &ExactInputReceipt) -> tos_compiler::Result<()> {
        let revision = self.cut.current().revision();
        let membership = self.cut.stream(revision).unwrap().expectation();
        if receipt.binding.index_generation != revision.0.to_hex()
            || receipt.binding.membership_root != membership.digest.to_hex()
        {
            return Err(tos_compiler::Error::Invalid(
                "Item fixture source selection changed",
            ));
        }
        Ok(())
    }
}

// Finite software-fixture custody only, as in compiler_source_cases. The
// enclosing CI case owns host admission; this is no production quota grant.
struct FixtureStage<'a> {
    root: &'a Path,
    deadline: Instant,
    cancelled: &'a AtomicBool,
}
impl StageIsolation for FixtureStage<'_> {
    fn verify(&self, path: &Path, _: StageLimits, _: WritePhase) -> tos_compiler::Result<()> {
        if path.parent() != Some(self.root)
            || Instant::now() >= self.deadline
            || self.cancelled.load(std::sync::atomic::Ordering::Relaxed)
        {
            return Err(tos_compiler::Error::Budget(
                "Item fixture private stage/deadline",
            ));
        }
        Ok(())
    }
}

#[derive(Default)]
struct RawBibliographicRows(BTreeMap<String, Vec<Value>>);
impl BibliographicSink for RawBibliographicRows {
    fn row(&mut self, collection: &str, _: &str, raw: &[u8]) -> tos_compiler::Result<()> {
        self.0.entry(collection.to_owned()).or_default().push(
            serde_json::from_slice(raw)
                .map_err(|_| tos_compiler::Error::Invalid("Item producer raw row JSON"))?,
        );
        Ok(())
    }
}

fn write_json(root: &Path, relative: &str, value: &Value) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}

#[test]
fn local_inventory_bridge_tei_prepared_snapshot_consumer() {
    let mut f = Fixture::new("source.tei.xml", "application/tei+xml", TEI);
    // Select the existing content-addressed File namespace required by the
    // navigation producer, through a fresh genuine protected owner selection.
    let file_id = format!("tos.file.sha256.{}", Digest256::of_bytes(TEI).to_hex());
    f.config["file_id"] = json!(file_id);
    f.context.configuration_raw = serde_json::to_vec(&f.config).unwrap();
    let mut proposal: Value = serde_json::from_slice(&f.context.request_raw).unwrap();
    proposal["rights"]["scope_refs"] = json!([f.config["item_id"], file_id]);
    f.context.request_raw = serde_json::to_vec(&proposal).unwrap();
    let owner_path = f.root().join("item-owner.json");
    fs::write(&owner_path, &f.context.configuration_raw).unwrap();
    f.filesystem =
        CreationFilesystem::select_item_owner(&owner_path, f.deadline, &f.cancelled).unwrap();
    let preparation = f.prepare();
    let request = prepared_request(&preparation);
    assert_eq!(request["inventory"]["profile"], "tei_structure_v1");
    let committed = f.execute(&request).unwrap();
    assert!(committed.metadata_committed());
    let terminal = f.publication();
    let mut files = support::authored(f.root());
    let committed_members = files.clone();
    // The frozen operation fixture carries only the operation's schemas.
    // Add missing current compiler definitions in a separate selected cut;
    // every committed member remains byte-for-byte identical.
    let repository = support::repository();
    for entry in fs::read_dir(repository.join("ToS/contracts")).unwrap() {
        let path = entry.unwrap().path();
        if path.is_file()
            && path
                .extension()
                .is_some_and(|extension| extension == "json")
        {
            let relative = path.strip_prefix(&repository).unwrap().to_str().unwrap();
            files
                .entry(relative.to_owned())
                .or_insert_with(|| fs::read(path).unwrap());
        }
    }
    for (path, raw) in &committed_members {
        assert_eq!(files.get(path), Some(raw));
    }
    let (revision, cut) = support::cut(
        &files,
        &f._temporary.path().join("prepared-source-cut"),
        f.deadline,
        &f.cancelled,
    );
    let claims_path = format!(
        "{}/source-claims.jsonl",
        f.config["item_source_path"]
            .as_str()
            .unwrap()
            .rsplit_once('/')
            .unwrap()
            .0
    );
    let claims: Value = serde_json::from_slice(&files[&claims_path]).unwrap();
    let mut compound_worker = support::worker(&cut, f.deadline, &f.cancelled);
    let compound = tos_validation::native_compound::verify_edition_item_from_cut(
        &cut,
        &mut compound_worker,
        &claims_path,
        &claims,
        f.limits(),
        &f.cancelled,
    )
    .unwrap();
    compound_worker.finish(f.deadline, &f.cancelled).unwrap();
    assert_eq!(compound.transaction_id, committed.transaction_id());
    drop(compound_worker);

    let membership = cut.stream(revision).unwrap().expectation();
    let binding = SourceBinding {
        owner_profile: "finite-local-item-software-fixture".into(),
        source_cut: revision.0.to_hex(),
        through_commit_seq: 0,
        membership_root: membership.digest.to_hex(),
        index_generation: revision.0.to_hex(),
        route_map_version: "fixture-v1".into(),
        reader_abi: "fixture-v1".into(),
        projection_root_sha256: Digest256::of_bytes(
            &cmd::canonical(committed.receipt().unwrap()).unwrap(),
        )
        .to_hex(),
        complete: true,
    };
    let limits = BibliographicLimits {
        catalog: SourceCatalogLimits {
            max_files: 512,
            max_rows: 4096,
            max_file_bytes: 2 * 1024 * 1024,
            max_row_bytes: 1024 * 1024,
            max_contract_bytes: 16 * 1024 * 1024,
            max_output_row_bytes: 1024 * 1024,
        },
        max_claim_cohort_rows: 16,
        max_claim_cohort_bytes: 16 * 1024 * 1024,
        max_output_rows: 4096,
        max_output_bytes: 16 * 1024 * 1024,
        deadline: f.deadline,
    };
    let plan = tos_compiler::plan_source_catalog_inputs(
        &cut,
        revision,
        membership,
        &binding,
        SourceCatalogInputLimits {
            max_manifest_members: 512,
            max_selected_members: 512,
            max_plan_bytes: 16 * 1024 * 1024,
            max_work_bytes: 64 * 1024 * 1024,
        },
        limits,
        &f.cancelled,
    )
    .unwrap();
    let owner = SelectedCutOwner {
        cut: &cut,
        receipt: plan.input_receipt(),
    };
    let isolation = FixtureStage {
        root: f._temporary.path(),
        deadline: f.deadline,
        cancelled: &f.cancelled,
    };
    let mut stage = KnowledgeStage::create(
        &f._temporary.path().join("prepared-catalog.sqlite"),
        StageLimits {
            sqlite: tos_compiler::Limits {
                max_rows: 8192,
                max_row_bytes: 2 * 1024 * 1024,
                max_output_bytes: 64 * 1024 * 1024,
                max_work_bytes: 128 * 1024 * 1024,
                sqlite_cache_kib: 512,
                max_sql_vm_steps: 20_000_000,
            },
            max_temp_bytes: 64 * 1024 * 1024,
            max_seek_rows: 16,
            max_seek_bytes: 32 * 1024 * 1024,
        },
        plan.input_receipt(),
        &owner,
        &isolation,
    )
    .unwrap();
    let worker_path = std::env::var_os("TOS_SCHEMA_WORKER_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("CARGO_TARGET_DIR").unwrap())
                .join("debug/tos-schema-worker")
        });
    let worker = ExactWorkerIdentity {
        sha256: Digest256::of_bytes(&fs::read(&worker_path).unwrap()),
        absolute_path: worker_path,
    };
    let remaining = f.deadline.saturating_duration_since(Instant::now());
    let worker_budget = ExecutorBudget {
        execution_wall: remaining,
        ..ExecutorBudget::laboratory()
    };
    let mut operation = BatchStreamBudget::laboratory();
    operation.max_chunks = limits.max_output_rows;
    operation.max_total_units = limits.max_output_rows;
    operation.total_execution_wall = remaining;
    operation.operation_cpu_seconds = remaining.as_secs().saturating_add(1);
    operation.operation_address_space_bytes = worker_budget.address_space_bytes;
    let validator = SourceCatalogValidator::from_cut(
        &cut,
        &worker,
        worker_budget,
        CutWorkerLimits {
            max_receipts: 4096,
            max_receipt_bytes: 16 * 1024 * 1024,
        },
        operation,
        f.deadline,
        &f.cancelled,
    )
    .unwrap();
    let mut forms = crate::source_forms_compiler::NativeBibliographicForms;
    let candidate = tos_compiler::render_source_bibliographic_plan(
        &plan,
        &cut,
        revision,
        membership,
        &mut stage,
        &validator,
        &mut forms,
        limits,
        512,
        16 * 1024 * 1024,
    )
    .unwrap();
    let entities: Value =
        serde_json::from_slice(&files["ToS/doctrine/semantic-interchange/entity-types.v1.json"])
            .unwrap();
    let navigation = tos_compiler::source_navigation_source::project_source_navigation_from_cut(
        &mut stage,
        &candidate.catalog,
        &BibliographicSourceCut {
            cut: &cut,
            expected_revision: revision,
            expected_membership: membership,
            stage_source_cut: &binding.source_cut,
            max_read_files: 512,
            max_read_bytes: 16 * 1024 * 1024,
        },
        &validator,
        &entities,
        &mut forms,
        limits,
    )
    .unwrap();
    assert!(
        !navigation.value()["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["level"] == "error")
    );
    assert!(
        navigation.value()["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n["node_id"] == file_id)
    );
    let mut bibliographic = RawBibliographicRows::default();
    render_bibliographic_graph(
        &mut stage,
        &candidate.catalog,
        &candidate.bibliographic,
        limits,
        &mut bibliographic,
    )
    .unwrap();
    drop(validator);

    // These are raw producer outputs. Empty unrelated families are explicit
    // finite fixture selection; this does not claim whole-corpus parity.
    let source = f._temporary.path().join("prepared-inputs");
    write_json(
        &source,
        "ToS/derived-exports/tos_corpus_index.min.json",
        &json!({
            "schema_version":"tos_corpus_index_v1", "nodes":[], "resources":[], "manifests":[],
            "branches":[], "relation_edges":[], "relation_packs":[], "graph_views":[],
            "source_navigation":navigation.value(),
        }),
    );
    write_json(
        &source,
        "ToS/derived-exports/philosophy_graph_projection.min.json",
        &json!({
            "schema_version":"tos_philosophy_graph_projection_v2", "nodes":[], "edges":[], "layers":[], "views":[], "clusters":[],
        }),
    );
    write_json(
        &source,
        "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json",
        &json!({
            "schema_version":"tos_source_witness_bibliographic_graph_v1", "nodes":bibliographic.0["nodes"],
            "edges":bibliographic.0["edges"], "claim_traces":bibliographic.0["claim_traces"],
        }),
    );
    for relative in [
        "ToS/doctrine/semantic-interchange/entity-types.v1.json",
        "ToS/doctrine/semantic-interchange/relation-types.v1.json",
    ] {
        let to = source.join(relative);
        fs::create_dir_all(to.parent().unwrap()).unwrap();
        fs::write(to, &files[relative]).unwrap();
    }
    let output = f._temporary.path().join("prepared-output");
    let receipt =
        tos_compiler::native_prepare::prepare(tos_compiler::native_prepare::PrepareRequest {
            source_root: &source,
            output_dir: &output,
            publication: tos_compiler::local_prepared::PublicationLimits::default(),
            search_scratch: None,
            maintenance: None,
            max_seconds: f
                .deadline
                .saturating_duration_since(Instant::now())
                .as_secs()
                .min(120),
            source_limits: None,
        })
        .unwrap();
    assert_eq!(receipt["status"], "completed");
    assert_eq!(receipt["source_state_checked"], true);
    assert_eq!(receipt["ongoing_currentness_granted"], false);
    let selected = cmd::parse(&fs::read(output.join("binding.json")).unwrap()).unwrap();
    let db = rusqlite::Connection::open_with_flags(
        output.join("snapshot.sqlite"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    db.execute_batch("BEGIN").unwrap();
    let mut consumer = tos_query::compressed_search::PreparedSearchSession::new(
        &db,
        tos_compiler::local_prepared::PreparedReadLimits::default(),
    )
    .unwrap();
    let item = consumer
        .inspect(
            &selected,
            tos_query::search_v2::SearchKind::Nodes,
            &format!(
                "source-navigation:{}",
                f.config["item_id"].as_str().unwrap()
            ),
            16,
        )
        .unwrap();
    let item = serde_json::from_slice::<Value>(&cmd::canonical(&item).unwrap()).unwrap();
    assert_eq!(item["matches"].as_array().unwrap().len(), 1);
    assert_eq!(item["matches"][0]["native_id"], f.config["item_id"]);
    let file = consumer
        .inspect(
            &selected,
            tos_query::search_v2::SearchKind::Nodes,
            &format!("source-navigation:{file_id}"),
            16,
        )
        .unwrap();
    let file_raw = cmd::canonical(&file).unwrap();
    let file: Value = serde_json::from_slice(&file_raw).unwrap();
    assert_eq!(file["matches"].as_array().unwrap().len(), 1);
    assert_eq!(
        file["matches"][0]["attributes"]["sha256"],
        f.config["sha256"]
    );
    assert_eq!(
        file["matches"][0]["attributes"]["media_type"],
        "application/tei+xml"
    );
    assert!(!String::from_utf8(file_raw).unwrap().contains(PRIVATE_TEXT));
    let mut stale = receipt["binding"].clone();
    stale["publication_epoch"] = json!(stale["publication_epoch"].as_u64().unwrap() + 1);
    let stale = cmd::parse(&serde_json::to_vec(&stale).unwrap()).unwrap();
    assert_eq!(
        consumer
            .inspect(
                &stale,
                tos_query::search_v2::SearchKind::Nodes,
                &format!("source-navigation:{file_id}"),
                16
            )
            .unwrap_err()
            .code,
        tos_query::search_v2::SearchV2ErrorCode::StaleSelection
    );
    consumer.recheck_binding(&selected).unwrap();
    drop(consumer);
    db.execute_batch("COMMIT; BEGIN").unwrap();
    tos_compiler::local_prepared::PreparedReadTransaction::recheck_binding(
        &db,
        &selected,
        tos_compiler::local_prepared::PreparedReadLimits::default(),
    )
    .unwrap();
    db.execute_batch("COMMIT").unwrap();
    assert_eq!(f.publication(), terminal);
    assert_eq!(
        fs::read(deposit::destination(&f.config).unwrap()).unwrap(),
        TEI
    );
    let rights: Value =
        serde_json::from_slice(&fs::read(f.item().parent().unwrap().join("rights.json")).unwrap())
            .unwrap();
    assert_eq!(rights, request["rights"]);
    assert_eq!(rights["visibility"], "local_only");
}
