//! Actual cold repository plan/render through the existing selected cut and
//! captured-software fixture. Scope is the finite declared fixture Git tree.
use super::*;
use serde_json::json;
use std::collections::BTreeMap;
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_compiler::knowledge_repository_source::{
    RepositorySourceLimits, plan_repository_source_inputs, render_repository_source_plan,
};
use tos_compiler::knowledge_stage::{
    ExactInputReceipt, KnowledgeStage, StageIsolation, StageLimits, StageOwner, WritePhase,
};
use tos_compiler::{
    Limits, QueryVocabulary, RepositoryRootInput, SourceBinding, TopologyLimits,
    prepare_repository_topology,
};
use tos_validation::executor::{ExactWorkerIdentity, ExecutorBudget};
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerLimits, CutWorkerSchemaExecutor};

include!("native_corpus_query.rs");
#[path = "native_managed_corpus_consumer.rs"]
mod native_managed_corpus_consumer;

struct FixtureOwner;
impl StageOwner for FixtureOwner {
    fn verify_receipt(&self, _: &ExactInputReceipt) -> tos_compiler::Result<()> {
        Ok(())
    }
    fn recheck_sealed_cut(&self, _: &ExactInputReceipt) -> tos_compiler::Result<()> {
        Ok(())
    }
}
struct FixtureIsolation;
impl StageIsolation for FixtureIsolation {
    fn verify(&self, _: &Path, _: StageLimits, _: WritePhase) -> tos_compiler::Result<()> {
        Ok(())
    }
}
fn git(root: &Path, args: &[&str]) -> Vec<u8> {
    let mut command = Command::new("git");
    command.arg("-C").arg(root).args(args);
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            command.env_remove(key);
        }
    }
    let out = command
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "bounded fixture Git failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    out.stdout
}
fn schema_sources(repository: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut authored = BTreeMap::new();
    for entry in fs::read_dir(repository.join("ToS/contracts")).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().into_string().unwrap();
        if name.ends_with(".schema.json") {
            authored.insert(
                format!("ToS/contracts/{name}"),
                fs::read(entry.path()).unwrap(),
            );
        }
    }
    authored
}
fn sources() -> (BTreeMap<String, Vec<u8>>, BTreeMap<String, Vec<u8>>) {
    let repository = super::validation_cut_cases::repository();
    let mut authored = schema_sources(&repository);
    let home = fs::read(repository.join("ToS/source_home.manifest.json")).unwrap();
    let home_value: Value = serde_json::from_slice(&home).unwrap();
    authored.insert("ToS/source_home.manifest.json".into(), home);
    for branch in home_value["branches"].as_array().unwrap() {
        let surface = branch["owner_surface"].as_str().unwrap();
        authored.insert(surface.into(), fs::read(repository.join(surface)).unwrap());
    }
    let mut captured = authored.clone();
    // Genuine maintained compatibility mirror remains weaker than canon.
    // A separate original manifest also exercises the captured-component
    // manifest read path; neither is a member of the declared source cut.
    for companion in [
        "ToS/public-compatibility/source_node.example.json",
        "ToS/philosophy/philosophy.manifest.json",
    ] {
        assert!(!authored.contains_key(companion));
        captured.insert(
            companion.into(),
            fs::read(repository.join(companion)).unwrap(),
        );
    }
    captured.insert(
        "scripts/corpus_archive.py".into(),
        fs::read(repository.join("scripts/corpus_archive.py")).unwrap(),
    );
    assert!(captured.len() <= 512);
    assert!(captured.values().map(Vec::len).sum::<usize>() <= 16 * 1024 * 1024);
    (authored, captured)
}
#[test]
fn actual_selected_capture_repository_plan_render_matches_maintained_python() {
    let (authored, captured) = sources();
    let fixture = tempfile::tempdir().unwrap();
    let git_root = fixture.path().join("selected-git");
    fs::create_dir(&git_root).unwrap();
    git(&git_root, &["init", "-q"]);
    for (name, raw) in &captured {
        let path = git_root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, raw).unwrap();
    }
    git(
        &git_root,
        &["add", "--", "ToS", "scripts/corpus_archive.py"],
    );
    git(
        &git_root,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "Selected maintained fixture bytes",
        ],
    );
    let commit = String::from_utf8(git(&git_root, &["rev-parse", "HEAD^{commit}"]))
        .unwrap()
        .trim()
        .to_owned();
    let capture = super::source_cut_cases::captured_software_fixture(&git_root, &commit, &["ToS"]);
    let cancelled = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(60);
    let read_limits = ReadLimits {
        max_manifest_bytes: 2 * 1024 * 1024,
        max_manifest_entries: 512,
        max_selected_object_bytes: 2 * 1024 * 1024,
        json: JsonLimits::default(),
    };
    let inventory = tos_source_store::SoftwareCaptureReader::open(
        &capture.capture,
        &capture.restored,
        capture.selection.clone(),
        read_limits,
        deadline,
        &cancelled,
    )
    .unwrap();
    let store = fixture.path().join("source-store");
    let revision = super::validation_cut_cases::write_cut_store(&authored, &store);
    let reader = CorpusReader::open_existing(&store, read_limits).unwrap();
    let cut = reader
        .open_source_cut(
            revision,
            tos_source_store::CutReadLimits {
                max_revisions: 1,
                max_members: 512,
                max_total_bytes: 16 * 1024 * 1024,
                max_member_bytes: 2 * 1024 * 1024,
            },
            deadline,
            &cancelled,
        )
        .unwrap();
    let membership = cut.stream(revision).unwrap().expectation();
    let worker_path = super::validation_cut_cases::selected_worker_path();
    let mut schemas = CutWorkerSchemaExecutor::from_cut(
        &cut,
        tos_validation::FormatProfile::LegacyPythonObserved20260923,
        ExactWorkerIdentity {
            sha256: Digest256::of_bytes(&fs::read(&worker_path).unwrap()),
            absolute_path: worker_path,
        },
        ExecutorBudget::laboratory(),
        CutWorkerLimits {
            max_receipts: 128,
            max_receipt_bytes: 131_072,
        },
        deadline,
        &cancelled,
    )
    .unwrap();
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    // Preserve the complete maintained descriptor: its dossier identity binds
    // another registered source even for this repository-only producer case.
    let descriptor_bytes = fs::read(
        repository.join("rust/crates/tos-compiler/tests/fixtures/query-vocabulary.v1.json"),
    )
    .unwrap();
    let vocabulary = QueryVocabulary::parse(
        &descriptor_bytes,
        &[
            "philosophy-node-edge-v1",
            "canon-node-relation-v1",
            "candidate-relation-v1",
            "source-navigation-node-edge-v1",
            "reified-bibliographic-claims-v1",
            "declared-identity-and-source-ref-joins-v1",
            "repository-topology-v1",
            "indexed-node-edge-v1",
        ],
    )
    .unwrap();
    let root_raw=serde_json::to_vec(&json!({"node_id":"fixture-source-root","node_type":"repository-root","label":"Selected fixture repository","source_ref":"ToS/source_home.manifest.json"})).unwrap();
    let root_sha = Digest256::of_bytes(&root_raw).to_hex();
    let limits = RepositorySourceLimits {
        max_inventory_members: 512,
        max_source_bytes: 2 * 1024 * 1024,
        max_row_bytes: 2 * 1024 * 1024,
        max_plan_bytes: 16 * 1024 * 1024,
        max_work_bytes: 32 * 1024 * 1024,
    };
    let job = "selected-fixture-repository-source";
    let plan = plan_repository_source_inputs(
        &cut,
        revision,
        membership,
        &inventory,
        &capture.selection,
        job,
        &vocabulary,
        RepositoryRootInput {
            source_cut: job,
            material: &root_raw,
            material_sha256: &root_sha,
            identity_id: "root:fixture-source-root",
        },
        &mut schemas,
        limits,
        deadline,
        &cancelled,
    )
    .unwrap();
    let mut wrong_membership = membership;
    wrong_membership.count += 1;
    assert!(
        plan_repository_source_inputs(
            &cut,
            revision,
            wrong_membership,
            &inventory,
            &capture.selection,
            job,
            &vocabulary,
            plan.root_input(),
            &mut schemas,
            limits,
            deadline,
            &cancelled
        )
        .is_err()
    );
    schemas.finish(deadline, &cancelled).unwrap();
    let receipt = ExactInputReceipt {
        binding: SourceBinding {
            owner_profile: "private-selected-fixture".into(),
            source_cut: job.into(),
            through_commit_seq: 0,
            membership_root: membership.digest.to_hex(),
            index_generation: revision.0.to_hex(),
            route_map_version: "fixture-v1".into(),
            reader_abi: "fixture-v1".into(),
            projection_root_sha256: Digest256::of_bytes(b"independent-private-target").to_hex(),
            complete: true,
        },
        collections: plan.receipt().collections.clone(),
    };
    let stage_limits = StageLimits {
        sqlite: Limits {
            max_rows: 4096,
            max_row_bytes: 2 * 1024 * 1024,
            max_output_bytes: 64 * 1024 * 1024,
            max_work_bytes: 128 * 1024 * 1024,
            sqlite_cache_kib: 512,
            max_sql_vm_steps: 10_000_000,
        },
        max_temp_bytes: 64 * 1024 * 1024,
        max_seek_rows: 128,
        max_seek_bytes: 16 * 1024 * 1024,
    };
    let owner = FixtureOwner;
    let isolation = FixtureIsolation;
    let mut stage = KnowledgeStage::create(
        &fixture.path().join("repository-candidate.sqlite"),
        stage_limits,
        receipt,
        &owner,
        &isolation,
    )
    .unwrap();
    render_repository_source_plan(&mut stage, &plan, &vocabulary, deadline, &cancelled).unwrap();
    let python = r#"import json,pathlib,sys
sys.path[:0]=[sys.argv[1]+'/scripts',sys.argv[1]+'/access/src']
import tos_corpus_index_common as owner
root=pathlib.Path(sys.argv[2]);owner.REPO_ROOT=root;owner.TOS_ROOT=root/'ToS'
paths=tuple(sorted(p for p in (root/'ToS').rglob('*') if p.is_file()))
diagnostics=[]
result={'branches':owner.build_branches(owner.load_json(root/'ToS/source_home.manifest.json'),diagnostics),'manifests':owner.build_manifests(diagnostics,paths),'resources':owner.build_resources(paths)}
assert not diagnostics,diagnostics
print(json.dumps(result,ensure_ascii=False,sort_keys=True,separators=(',',':')))
"#;
    let output = Command::new("python3")
        .args(["-c", python])
        .arg(&repository)
        .arg(&capture.restored)
        .env_remove("PYTHONPATH")
        .env_remove("PYTHONHOME")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "maintained Python repository oracle: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected: Value = serde_json::from_slice(&output.stdout).unwrap();
    let source = &vocabulary
        .sources
        .iter()
        .find(|source| source.adapter_profile == "repository-topology-v1")
        .unwrap()
        .source_graph_id;
    let mut orders = Vec::new();
    for collection in ["branches", "manifests", "resources"] {
        let mut actual = Vec::new();
        let mut after = None;
        loop {
            let page = stage
                .scan_input(source, collection, after.as_deref(), 128)
                .unwrap();
            for row in page.rows {
                actual.push(serde_json::from_slice::<Value>(&row.payload).unwrap());
            }
            after = page.next_id;
            if after.is_none() {
                break;
            }
        }
        let mut wanted = expected[collection].as_array().unwrap().clone();
        for (ordinal, row) in wanted.iter().enumerate() {
            let id = row.get("id").or_else(|| row.get("path")).unwrap();
            orders.push(json!({"collection":collection,"id":id,"ordinal":ordinal}));
        }
        // Raw roots sort by identity, independently from owner array ordinal.
        wanted.sort_by_key(|v| {
            v.get("id")
                .or_else(|| v.get("path"))
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned()
        });
        assert_eq!(
            actual, wanted,
            "whole selected maintained {collection} rows"
        );
    }
    let mut actual_orders = Vec::new();
    let mut after = None;
    loop {
        let page = stage
            .scan_input(source, "source_order", after.as_deref(), 128)
            .unwrap();
        for row in page.rows {
            actual_orders.push(serde_json::from_slice::<Value>(&row.payload).unwrap());
        }
        after = page.next_id;
        if after.is_none() {
            break;
        }
    }
    orders.sort_by_key(|v| {
        format!(
            "{}:{}",
            v["collection"].as_str().unwrap(),
            v["id"].as_str().unwrap()
        )
    });
    assert_eq!(actual_orders, orders, "exact dense original owner ordering");
    let prepared = prepare_repository_topology(
        &mut stage,
        &vocabulary,
        plan.root_input(),
        TopologyLimits {
            max_rows: 4096,
            max_page_rows: 32,
            max_row_bytes: 2 * 1024 * 1024,
            max_work_bytes: 64 * 1024 * 1024,
        },
    )
    .unwrap();
    let rows = plan
        .receipt()
        .collections
        .iter()
        .filter(|r| r.collection != "source_order")
        .map(|r| r.expected_count)
        .sum::<u64>();
    assert_eq!(prepared.nodes, rows + 1);
    assert!(prepared.relations > 0);
}

#[derive(Default)]
struct CatalogOutput {
    files: BTreeMap<String, Vec<u8>>,
    current: Option<String>,
    manifest: Value,
}
impl tos_compiler::source_witness_catalog::SourceCatalogSink for CatalogOutput {
    fn begin_file(&mut self, source: &str) -> tos_compiler::Result<()> {
        assert!(self.current.is_none());
        assert!(self.files.insert(source.into(), Vec::new()).is_none());
        self.current = Some(source.into());
        Ok(())
    }
    fn file_bytes(&mut self, raw: &[u8]) -> tos_compiler::Result<()> {
        let output = self.files.get_mut(self.current.as_ref().unwrap()).unwrap();
        assert!(output.len() + raw.len() <= 2 * 1024 * 1024);
        output.extend_from_slice(raw);
        Ok(())
    }
    fn end_file(&mut self, source: &str, sha256: &str) -> tos_compiler::Result<()> {
        assert_eq!(self.current.take().as_deref(), Some(source));
        assert_eq!(Digest256::of_bytes(&self.files[source]).to_hex(), sha256);
        Ok(())
    }
    fn addressed_row(&mut self, _: &str, _: &[u8]) -> tos_compiler::Result<()> {
        Ok(())
    }
    fn manifest(&mut self, value: &Value) -> tos_compiler::Result<()> {
        self.manifest = value.clone();
        Ok(())
    }
}
#[derive(Default)]
struct BibliographicOutput(BTreeMap<String, Vec<Value>>);
impl tos_compiler::source_bibliographic::BibliographicSink for BibliographicOutput {
    fn row(&mut self, collection: &str, _: &str, raw: &[u8]) -> tos_compiler::Result<()> {
        assert!(raw.len() <= 1024 * 1024);
        let rows = self.0.entry(collection.into()).or_default();
        assert!(rows.len() < 1024);
        rows.push(serde_json::from_slice(raw).unwrap());
        Ok(())
    }
}

#[test]
fn actual_selected_catalog_and_native_forms_match_maintained_python() {
    use tos_compiler::source_bibliographic::{BibliographicLimits, render_bibliographic_graph};
    use tos_compiler::source_witness_catalog::{
        SourceCatalogLimits, SourceCatalogValidator, render_source_witness_catalog,
    };
    use tos_compiler::{
        SourceCatalogInputLimits, plan_source_catalog_inputs, render_source_bibliographic_plan,
    };
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let mut files = schema_sources(&repository);
    // The declared source carrier contains schemas and two genuine maintained
    // fixture records; repository branch inputs are outside this family scope.
    for name in ["entity-types.v1.json", "relation-types.v1.json"] {
        let path = format!("ToS/doctrine/semantic-interchange/{name}");
        files.insert(path.clone(), fs::read(repository.join(path)).unwrap());
    }
    let fixture_root = repository.join("access/tests/fixtures/knowledge-contract");
    let record = "ToS/source-witnesses/semantic-descriptions/crosscutting-concept-freedom/crosscutting-concept.json";
    let forms = "ToS/source-witnesses/semantic-descriptions/crosscutting-concept-freedom/crosscutting-concept.human-forms.json";
    for path in [record, forms] {
        files.insert(path.into(), fs::read(fixture_root.join(path)).unwrap());
    }
    assert!(files.len() <= 512);
    assert!(files.values().map(Vec::len).sum::<usize>() <= 16 * 1024 * 1024);
    let fixture = tempfile::tempdir().unwrap();
    let store = fixture.path().join("source-store");
    let revision = super::validation_cut_cases::write_cut_store(&files, &store);
    let cancelled = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(60);
    let read_limits = ReadLimits {
        max_manifest_bytes: 2 * 1024 * 1024,
        max_manifest_entries: 512,
        max_selected_object_bytes: 2 * 1024 * 1024,
        json: JsonLimits::default(),
    };
    let reader = CorpusReader::open_existing(&store, read_limits).unwrap();
    let cut = reader
        .open_source_cut(
            revision,
            tos_source_store::CutReadLimits {
                max_revisions: 1,
                max_members: 512,
                max_total_bytes: 16 * 1024 * 1024,
                max_member_bytes: 2 * 1024 * 1024,
            },
            deadline,
            &cancelled,
        )
        .unwrap();
    let membership = cut.stream(revision).unwrap().expectation();
    let binding = SourceBinding {
        owner_profile: "private-selected-fixture".into(),
        source_cut: "selected-fixture-bibliographic-source".into(),
        through_commit_seq: 0,
        membership_root: membership.digest.to_hex(),
        index_generation: revision.0.to_hex(),
        route_map_version: "fixture-v1".into(),
        reader_abi: "fixture-v1".into(),
        projection_root_sha256: Digest256::of_bytes(b"independent-catalog-plan").to_hex(),
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
        deadline,
    };
    let plan = plan_source_catalog_inputs(
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
        &cancelled,
    )
    .unwrap();
    let mut receipt = plan.input_receipt();
    assert_eq!(receipt.collections.len(), 5);
    receipt.binding.projection_root_sha256 =
        Digest256::of_bytes(b"independent-catalog-target").to_hex();
    let owner = FixtureOwner;
    let isolation = FixtureIsolation;
    let mut stage = KnowledgeStage::create(
        &fixture.path().join("bibliographic-candidate.sqlite"),
        StageLimits {
            sqlite: Limits {
                max_rows: 8192,
                max_row_bytes: 2 * 1024 * 1024,
                max_output_bytes: 64 * 1024 * 1024,
                max_work_bytes: 128 * 1024 * 1024,
                sqlite_cache_kib: 512,
                max_sql_vm_steps: 20_000_000,
            },
            max_temp_bytes: 64 * 1024 * 1024,
            max_seek_rows: 128,
            max_seek_bytes: 16 * 1024 * 1024,
        },
        receipt,
        &owner,
        &isolation,
    )
    .unwrap();
    let worker_path = super::validation_cut_cases::selected_worker_path();
    let worker = ExactWorkerIdentity {
        sha256: Digest256::of_bytes(&fs::read(&worker_path).unwrap()),
        absolute_path: worker_path,
    };
    // One explicit finite envelope for this complete catalog/schema operation.
    // Limits derive from the declared output/receipt budget and common deadline.
    let mut operation = tos_validation::executor::BatchStreamBudget::laboratory();
    operation.max_chunks = limits.max_output_rows;
    operation.max_total_units = limits.max_output_rows;
    operation.total_execution_wall = deadline.saturating_duration_since(Instant::now());
    operation.operation_cpu_seconds = operation.total_execution_wall.as_secs().saturating_add(1);
    operation.operation_address_space_bytes = ExecutorBudget::laboratory().address_space_bytes;
    let validator = SourceCatalogValidator::from_cut(
        &cut,
        &worker,
        ExecutorBudget::laboratory(),
        tos_validation::source_cut::CutWorkerLimits {
            max_receipts: usize::try_from(limits.max_output_rows).unwrap(),
            max_receipt_bytes: usize::try_from(limits.max_output_bytes).unwrap(),
        },
        operation,
        deadline,
        &cancelled,
    )
    .unwrap();
    let candidate = render_source_bibliographic_plan(
        &plan,
        &cut,
        revision,
        membership,
        &mut stage,
        &validator,
        &mut tos_command::source_forms_compiler::NativeBibliographicForms,
        limits,
        512,
        16 * 1024 * 1024,
    )
    .unwrap();
    validator.finish().unwrap();
    let mut actual_catalog = CatalogOutput::default();
    render_source_witness_catalog(
        &mut stage,
        &candidate.catalog,
        limits.catalog,
        &mut actual_catalog,
    )
    .unwrap();
    let mut actual_graph = BibliographicOutput::default();
    render_bibliographic_graph(
        &mut stage,
        &candidate.catalog,
        &candidate.bibliographic,
        limits,
        &mut actual_graph,
    )
    .unwrap();
    // Python may write only private generated companions for its full oracle.
    // The original cut and native stage already bind the independent source.
    let oracle_root = fixture.path().join("maintained-oracle");
    for (path, raw) in &files {
        let target = oracle_root.join(path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, raw).unwrap();
    }
    let python = r#"
import json,pathlib,sys
sys.path.insert(0,str(pathlib.Path(sys.argv[1])/'scripts'))
import build_source_witness_catalog as catalog
import source_witness_bibliographic_graph_common as graph
root=pathlib.Path(sys.argv[2]);outputs=catalog.render_outputs(root)
for path,text in outputs.items():
    target=root/path;target.parent.mkdir(parents=True,exist_ok=True);target.write_text(text,encoding='utf-8')
payload=graph.build_payload(root)
manifest=json.loads(outputs[catalog.MANIFEST_PATH])
files={str(path):text for path,text in outputs.items() if path!=catalog.MANIFEST_PATH}
print(json.dumps({'files':files,'manifest':manifest,'graph':{key:payload[key] for key in ('nodes','edges','claim_traces')}},ensure_ascii=False,sort_keys=True,separators=(',',':')))
"#;
    let output = Command::new("python3")
        .args(["-c", python])
        .arg(&repository)
        .arg(&oracle_root)
        .env_remove("PYTHONPATH")
        .env_remove("PYTHONHOME")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "maintained catalog/bibliographic oracle: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected: Value = serde_json::from_slice(&output.stdout).unwrap();
    let actual_files: BTreeMap<String, String> = actual_catalog
        .files
        .into_iter()
        .map(|(path, raw)| (path, String::from_utf8(raw).unwrap()))
        .collect();
    assert_eq!(
        serde_json::to_value(actual_files).unwrap(),
        expected["files"]
    );
    assert_eq!(actual_catalog.manifest, expected["manifest"]);
    for collection in ["nodes", "edges", "claim_traces"] {
        let rows = actual_graph.0.remove(collection).unwrap_or_default();
        assert_eq!(
            serde_json::to_value(rows).unwrap(),
            expected["graph"][collection]
        );
    }
    assert_eq!(candidate.catalog.record_count, 1);
    assert_eq!(candidate.bibliographic.node_count, 1);
    assert_eq!(candidate.bibliographic.edge_count, 0);
}

/// One finite maintained authored recipe exercises real canon/candidate rows
/// and Claim/endpoint retained history. Capture and source-cut identities stay
/// distinct; neither the owner stubs nor this oracle issue source admission.
#[test]
fn actual_native_corpus_composition_with_retained_claim_matches_maintained_python() {
    native_corpus_composition_case(false);
}

/// Required installed-native stage of the same authored recipe. OPS admits
/// this Linux kernel/product boundary independently from pure selected reads.
#[test]
#[ignore = "requires admitted Linux fs-verity and installed native consumer binary"]
fn actual_native_corpus_managed_installed_consumer() {
    native_corpus_composition_case(true);
}

fn native_corpus_composition_case(installed: bool) {
    use std::io::Write;
    use tos_compiler::knowledge_canon_source::*;
    use tos_compiler::knowledge_stage::{InputCollectionReceipt, InputRow};
    use tos_compiler::source_bibliographic::{BibliographicLimits, BibliographicSourceCut};
    use tos_compiler::source_corpus::*;
    use tos_compiler::source_navigation_source::project_source_navigation_from_cut;
    use tos_compiler::source_witness_catalog::{SourceCatalogLimits, SourceCatalogValidator};
    use tos_compiler::{
        SourceCatalogInputLimits, plan_source_catalog_inputs, render_source_bibliographic_plan,
    };
    fn phase(started: Instant, deadline: Instant, name: &str) {
        // Write to the actual stderr handle: libtest's captured eprintln!
        // output is lost when the outer whole-case deadline kills the process.
        let _ = writeln!(
            std::io::stderr().lock(),
            "native corpus phase={name} elapsed_seconds={}",
            started.elapsed().as_secs()
        );
        assert!(
            Instant::now() < deadline,
            "native corpus deadline after {name}"
        );
    }
    fn add_tree(repository: &Path, relative: &str, files: &mut BTreeMap<String, Vec<u8>>) {
        let mut entries = fs::read_dir(repository.join(relative))
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect::<Vec<_>>();
        entries.sort();
        for path in entries {
            let name = path.strip_prefix(repository).unwrap().to_str().unwrap();
            assert!(!path.is_symlink());
            if path.is_dir() {
                add_tree(repository, name, files);
            } else {
                files.insert(name.into(), fs::read(path).unwrap());
            }
        }
    }
    fn raw_receipt(
        source: &str,
        collection: &str,
        role: &str,
        profile: &str,
        rows: &BTreeMap<String, Vec<u8>>,
    ) -> InputCollectionReceipt {
        let mut hash = tos_foundation::Digest256Hasher::new();
        for (id, raw) in rows {
            hash.update(&(id.len() as u64).to_be_bytes());
            hash.update(id.as_bytes());
            hash.update(Digest256::of_bytes(raw).as_bytes());
        }
        InputCollectionReceipt {
            source_graph: source.into(),
            collection: collection.into(),
            input_role: role.into(),
            adapter_profile: profile.into(),
            expected_count: rows.len() as u64,
            expected_root_sha256: hash.finalize().to_hex(),
        }
    }
    let started = Instant::now();
    let deadline = started + Duration::from_secs(240);
    let cancelled = AtomicBool::new(false);
    phase(started, deadline, "source-preparation");
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let (mut files, _) = sources();
    for name in ["entity-types.v1.json", "relation-types.v1.json"] {
        let path = format!("ToS/doctrine/semantic-interchange/{name}");
        files.insert(path.clone(), fs::read(repository.join(path)).unwrap());
    }
    // Complete current canon-node membership for this selected recipe. No row
    // is synthesized to fit a producer schema, and no post-derivation truncation.
    let mut canon_tree = BTreeMap::new();
    add_tree(&repository, "ToS/canon", &mut canon_tree);
    for (path, raw) in canon_tree {
        if path.ends_with("/node.json") || path.ends_with("/node.human-forms.json") {
            files.insert(path, raw);
        }
    }
    for path in [
        "ToS/canon/relations/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/edges.csv",
        "ToS/candidate-intake/thus-spoke-zarathustra/prologue-1/mode-b/edges.csv",
        "ToS/research-packets/foundation-laboratory-2026-07/JENSEITS_1886_LETTER_705_SOURCE_READING_V1.md",
    ] {
        files.insert(path.into(), fs::read(repository.join(path)).unwrap());
    }
    for path in [
        "ToS/source-witnesses/relations/nietzsche-letter-705-addressee",
        "ToS/source-witnesses/agents/constantin-georg-naumann",
        "ToS/source-witnesses/documents/friedrich-nietzsche/naumann-letter-705",
        "ToS/source-witnesses/.record-revisions/2c4c3a4f5cb2cbf1713ebdaa0b27dfcb0729cf980f33591a6e1e2ea6296b8d25-f63f2f0562a6a662be9c5340ddde5686a6de35a8991ad2ad7b53e3a8fd134eba",
        "ToS/source-witnesses/.record-revisions/709df7fb307a1331d25fa253f7159a3ee27b3862898ddf8db74c6cbfc6965438-75afe571bb0254a738c20c0d5d09fac8b11ce0f299068b534ef652ef09575422",
        "ToS/source-witnesses/.record-revisions/3b6ca195bb9bb9fb57cc1e0d9bece8b18011ef12c3d99d614aac5fa3760ad712-8d38fda8bf756906f8ed3543a8cc069082d39b04b188db5050d76a2ad663a497",
    ] {
        add_tree(&repository, path, &mut files);
    }
    assert!(files.len() <= 512);
    assert!(files.values().map(Vec::len).sum::<usize>() <= 16 * 1024 * 1024);
    phase(started, deadline, "selected-inputs-ready");
    let fixture = tempfile::tempdir().unwrap();
    let git_root = fixture.path().join("selected-git");
    fs::create_dir(&git_root).unwrap();
    git(&git_root, &["init", "-q"]);
    let mut captured = files.clone();
    // The existing capture helper executes this exact commit's archive program;
    // both actual software owners remain outside the authored source cut.
    for path in [
        "scripts/corpus_archive.py",
        "scripts/tos_corpus_index_common.py",
    ] {
        captured.insert(path.into(), fs::read(repository.join(path)).unwrap());
    }
    let declaration_raw =
        fs::read(repository.join("access/contracts/runtime-data.v1.json")).unwrap();
    let declaration: Value = serde_json::from_slice(&declaration_raw).unwrap();
    let outputs = declaration["subjects"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|subject| subject["subject_id"] == "tos-corpus-index")
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), 1);
    let output_path = outputs[0]["source_path"].as_str().unwrap().to_owned();
    captured.insert(
        "access/contracts/runtime-data.v1.json".into(),
        declaration_raw,
    );
    assert!(captured.len() <= 512);
    assert!(captured.values().map(Vec::len).sum::<usize>() <= 16 * 1024 * 1024);
    phase(started, deadline, "capture-inputs-ready");
    for (path, raw) in &captured {
        let target = git_root.join(path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, raw).unwrap();
    }
    git(
        &git_root,
        &[
            "add",
            "--",
            "ToS",
            "scripts",
            "access/contracts/runtime-data.v1.json",
        ],
    );
    git(
        &git_root,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "Finite authored corpus recipe",
        ],
    );
    let commit = String::from_utf8(git(&git_root, &["rev-parse", "HEAD^{commit}"]))
        .unwrap()
        .trim()
        .to_owned();
    phase(started, deadline, "fixture-commit-ready");
    let capture = super::source_cut_cases::captured_software_fixture(
        &git_root,
        &commit,
        &["ToS", "scripts", "access/contracts"],
    );
    phase(started, deadline, "software-capture-ready");
    let read_limits = ReadLimits {
        max_manifest_bytes: 2 * 1024 * 1024,
        max_manifest_entries: 512,
        max_selected_object_bytes: 2 * 1024 * 1024,
        json: JsonLimits::default(),
    };
    let software = tos_source_store::SoftwareCaptureReader::open(
        &capture.capture,
        &capture.restored,
        capture.selection.clone(),
        read_limits,
        deadline,
        &cancelled,
    )
    .unwrap();
    let store = fixture.path().join("source-store");
    let revision = super::validation_cut_cases::write_cut_store(&files, &store);
    let reader = CorpusReader::open_existing(&store, read_limits).unwrap();
    let cut = reader
        .open_source_cut(
            revision,
            tos_source_store::CutReadLimits {
                max_revisions: 1,
                max_members: 512,
                max_total_bytes: 16 * 1024 * 1024,
                max_member_bytes: 2 * 1024 * 1024,
            },
            deadline,
            &cancelled,
        )
        .unwrap();
    phase(started, deadline, "source-cut-ready");
    let membership = cut.stream(revision).unwrap().expectation();
    let descriptor = fs::read(
        repository.join("rust/crates/tos-compiler/tests/fixtures/query-vocabulary.v1.json"),
    )
    .unwrap();
    let vocabulary =
        QueryVocabulary::parse(&descriptor, tos_compiler::NATIVE_KNOWLEDGE_ADAPTER_PROFILES)
            .unwrap();
    let binding = SourceBinding {
        owner_profile: "private-selected-fixture".into(),
        source_cut: "finite-native-corpus-source".into(),
        through_commit_seq: 0,
        membership_root: membership.digest.to_hex(),
        index_generation: revision.0.to_hex(),
        route_map_version: "fixture-v1".into(),
        reader_abi: "fixture-v1".into(),
        projection_root_sha256: Digest256::of_bytes(b"independent-native-corpus-plan").to_hex(),
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
        deadline,
    };
    let catalog_plan = plan_source_catalog_inputs(
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
        &cancelled,
    )
    .unwrap();
    let owner = FixtureOwner;
    let isolation = FixtureIsolation;
    let stage_limits = StageLimits {
        sqlite: Limits {
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
    };
    let mut catalog_stage = KnowledgeStage::create(
        &fixture.path().join("catalog.sqlite"),
        stage_limits,
        catalog_plan.input_receipt(),
        &owner,
        &isolation,
    )
    .unwrap();
    let mut operation = tos_validation::executor::BatchStreamBudget::laboratory();
    operation.max_chunks = limits.max_output_rows;
    operation.max_total_units = limits.max_output_rows;
    operation.total_execution_wall = deadline.saturating_duration_since(Instant::now());
    operation.operation_cpu_seconds = operation.total_execution_wall.as_secs().saturating_add(1);
    operation.operation_address_space_bytes = ExecutorBudget::laboratory().address_space_bytes;
    let worker_path = super::validation_cut_cases::selected_worker_path();
    let worker = ExactWorkerIdentity {
        sha256: Digest256::of_bytes(&fs::read(&worker_path).unwrap()),
        absolute_path: worker_path,
    };
    let validator = SourceCatalogValidator::from_cut(
        &cut,
        &worker,
        ExecutorBudget::laboratory(),
        CutWorkerLimits {
            max_receipts: 4096,
            max_receipt_bytes: 16 * 1024 * 1024,
        },
        operation,
        deadline,
        &cancelled,
    )
    .unwrap();
    let mut forms = tos_command::source_forms_compiler::NativeBibliographicForms;
    phase(started, deadline, "bibliographic-render-start");
    let candidate = render_source_bibliographic_plan(
        &catalog_plan,
        &cut,
        revision,
        membership,
        &mut catalog_stage,
        &validator,
        &mut forms,
        limits,
        512,
        16 * 1024 * 1024,
    )
    .unwrap();
    phase(started, deadline, "bibliographic-candidate-ready");
    assert!(candidate.catalog.record_count > 1);
    assert!(candidate.bibliographic.edge_count > 0);
    let canon_files = files
        .iter()
        .filter(|(p, _)| {
            (p.starts_with("ToS/canon/")
                && (p.ends_with("/node.json") || p.ends_with("/node.human-forms.json")))
                || ((p.starts_with("ToS/canon/") || p.starts_with("ToS/candidate-intake/"))
                    && p.ends_with("/edges.csv"))
        })
        .map(|(p, b)| (p.clone(), b.clone()))
        .collect::<BTreeMap<_, _>>();
    let contracts = [
        "ToS/contracts/tos-node-contract.schema.json",
        "ToS/contracts/human-form-set.schema.json",
    ]
    .into_iter()
    .map(|p| (p.into(), files[p].clone()))
    .collect::<BTreeMap<_, _>>();
    let canon_inputs = vec![
        raw_receipt(
            CANON_SOURCE_CUSTODY,
            "source-files",
            CANON_SOURCE_FILES_ROLE,
            CANON_SOURCE_FILES_PROFILE,
            &canon_files,
        ),
        raw_receipt(
            CANON_SOURCE_CUSTODY,
            "contracts",
            CANON_SOURCE_CONTRACTS_ROLE,
            CANON_SOURCE_CONTRACTS_PROFILE,
            &contracts,
        ),
    ];
    let mut canon_planner = KnowledgeStage::create(
        &fixture.path().join("canon.sqlite"),
        stage_limits,
        ExactInputReceipt {
            binding: binding.clone(),
            collections: canon_inputs,
        },
        &owner,
        &isolation,
    )
    .unwrap();
    for (collection, rows) in [("source-files", &canon_files), ("contracts", &contracts)] {
        let mut entries = rows.iter();
        loop {
            let borrowed = entries
                .by_ref()
                .take(stage_limits.max_seek_rows)
                .map(|(id, raw)| InputRow {
                    source_graph: CANON_SOURCE_CUSTODY,
                    collection,
                    id,
                    payload: raw,
                })
                .collect::<Vec<_>>();
            if borrowed.is_empty() {
                break;
            }
            canon_planner.ingest_input_batch(&borrowed).unwrap();
        }
    }
    let canon_limits = CanonSourceLimits {
        max_manifest_members: 512,
        max_selected_members: 512,
        max_nodes: 128,
        max_packs: 16,
        max_edges: 1024,
        max_source_bytes: 2 * 1024 * 1024,
        max_raw_row_bytes: 2 * 1024 * 1024,
        max_csv_fields: 128,
        max_csv_record_bytes: 2 * 1024 * 1024,
        max_forms: 256,
        max_forms_output_bytes: 262144,
        max_page_rows: 8,
        max_page_bytes: 16 * 1024 * 1024,
        max_work_bytes: 64 * 1024 * 1024,
    };
    let root_raw = serde_json::to_vec(
        &json!({"node_id":"fixture-source-root","node_type":"repository-root",
        "label":"Selected fixture repository","source_ref":"ToS/source_home.manifest.json"}),
    )
    .unwrap();
    let root_sha = Digest256::of_bytes(&root_raw).to_hex();
    let (repository_plan, canon_plan) = plan_native_corpus_source_families(
        &mut canon_planner,
        &cut,
        &software,
        &binding,
        &vocabulary,
        RepositoryRootInput {
            source_cut: &binding.source_cut,
            material: &root_raw,
            material_sha256: &root_sha,
            identity_id: "root:fixture-source-root",
        },
        &validator,
        RepositorySourceLimits {
            max_inventory_members: 512,
            max_source_bytes: 2 * 1024 * 1024,
            max_row_bytes: 2 * 1024 * 1024,
            max_plan_bytes: 16 * 1024 * 1024,
            max_work_bytes: 32 * 1024 * 1024,
        },
        canon_limits,
        deadline,
        &cancelled,
        tos_command::source_forms_compiler::materialize_compiler_forms,
    )
    .unwrap();
    assert!(canon_plan.receipt().nodes > 0);
    assert!(canon_plan.receipt().packs >= 2);
    assert!(canon_plan.receipt().edges > 0);
    let entities: Value =
        serde_json::from_slice(&files["ToS/doctrine/semantic-interchange/entity-types.v1.json"])
            .unwrap();
    let source = BibliographicSourceCut {
        cut: &cut,
        expected_revision: revision,
        expected_membership: membership,
        stage_source_cut: &binding.source_cut,
        max_read_files: 512,
        max_read_bytes: 16 * 1024 * 1024,
    };
    let navigation = project_source_navigation_from_cut(
        &mut catalog_stage,
        &candidate.catalog,
        &source,
        &validator,
        &entities,
        &mut forms,
        limits,
    )
    .unwrap();
    assert!(!navigation.value()["edges"].as_array().unwrap().is_empty());
    let mut collections = repository_plan.receipt().collections.clone();
    collections.extend(
        canon_plan
            .receipt()
            .collections
            .iter()
            .map(|c| InputCollectionReceipt {
                source_graph: c.source_graph.clone(),
                collection: c.collection.clone(),
                input_role: c.input_role.clone(),
                adapter_profile: c.adapter_profile.clone(),
                expected_count: c.count,
                expected_root_sha256: c.root_sha256.clone(),
            }),
    );
    collections.extend(catalog_plan.input_receipt().collections);
    let mut target = KnowledgeStage::create(
        &fixture.path().join("corpus.sqlite"),
        stage_limits,
        ExactInputReceipt {
            binding: binding.clone(),
            collections,
        },
        &owner,
        &isolation,
    )
    .unwrap();
    render_repository_source_plan(
        &mut target,
        &repository_plan,
        &vocabulary,
        deadline,
        &cancelled,
    )
    .unwrap();
    render_canon_source_plan(
        &mut canon_planner,
        &canon_plan,
        &cut,
        revision,
        membership,
        &mut target,
        canon_limits,
        deadline,
        &cancelled,
    )
    .unwrap();
    for input in &catalog_plan.input_receipt().collections {
        let mut after = None;
        loop {
            let page = catalog_stage
                .scan_input(
                    &input.source_graph,
                    &input.collection,
                    after.as_deref(),
                    stage_limits.max_seek_rows,
                )
                .unwrap();
            if !page.rows.is_empty() {
                let borrowed = page
                    .rows
                    .iter()
                    .map(|row| InputRow {
                        source_graph: &input.source_graph,
                        collection: &input.collection,
                        id: &row.id,
                        payload: &row.payload,
                    })
                    .collect::<Vec<_>>();
                target.ingest_input_batch(&borrowed).unwrap();
            }
            after = page.next_id;
            if after.is_none() {
                break;
            }
        }
    }
    let originals = tos_compiler::NavigationOriginalLimits {
        max_rows: 4096,
        max_row_bytes: limits.catalog.max_output_row_bytes,
        max_total_bytes: 16 * 1024 * 1024,
    };
    phase(started, deadline, "native-projection-start");
    let projection = project_native_corpus_from_sources(
        &mut target,
        &vocabulary,
        &cut,
        &software,
        &repository_plan,
        &canon_plan,
        &navigation,
        &validator,
        NativeCorpusLimits {
            originals,
            max_work_bytes: 128 * 1024 * 1024,
            schema_work: operation,
        },
        deadline,
        &cancelled,
    )
    .unwrap();
    phase(started, deadline, "native-projection-ready");
    // The maintained whole oracle receives exactly the selected authored paths.
    // Generated catalogue companions are private oracle outputs, not new inputs.
    let oracle_root = fixture.path().join("oracle");
    for (path, raw) in &files {
        let to = oracle_root.join(path);
        fs::create_dir_all(to.parent().unwrap()).unwrap();
        fs::write(to, raw).unwrap();
    }
    let paths = fixture.path().join("source-paths.json");
    fs::write(
        &paths,
        serde_json::to_vec(&files.keys().collect::<Vec<_>>()).unwrap(),
    )
    .unwrap();
    let python = r#"
import json,pathlib,sys
sys.path[:0]=[sys.argv[1]+'/scripts',sys.argv[1]+'/access/src']
import build_source_witness_catalog as catalog
import tos_corpus_index_common as owner
root=pathlib.Path(sys.argv[2]);owner.REPO_ROOT=root;owner.TOS_ROOT=root/'ToS'
for path,text in catalog.render_outputs(root).items():
    to=root/path;to.parent.mkdir(parents=True,exist_ok=True);to.write_text(text,encoding='utf-8')
payload=owner.build_payload(source_paths=json.loads(pathlib.Path(sys.argv[3]).read_text()))
sys.stdout.write(owner.render_payload(payload))
"#;
    let python = format!("{python}\n{NATIVE_CORPUS_QUERY_ORACLE}");
    phase(started, deadline, "maintained-oracle-start");
    let output = Command::new("python3")
        .args(["-c", &python])
        .arg(&repository)
        .arg(&oracle_root)
        .arg(&paths)
        .arg(&output_path)
        .env_remove("PYTHONPATH")
        .env_remove("PYTHONHOME")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "whole maintained corpus oracle: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(projection.value(), &expected);
    assert_eq!(projection.output_bytes(), output.stdout.as_slice());
    phase(started, deadline, "maintained-oracle-matched");
    // A fresh independently declared selected stage receives actual family
    // outputs. Catalogue custody and private planner tables stay out of its DDL.
    let mut graph = BibliographicOutput::default();
    tos_compiler::source_bibliographic::render_bibliographic_graph(
        &mut catalog_stage,
        &candidate.catalog,
        &candidate.bibliographic,
        limits,
        &mut graph,
    )
    .unwrap();
    let mut selected_rows = BTreeMap::<(String, String), BTreeMap<String, Vec<u8>>>::new();
    let mut selected_collections = repository_plan.receipt().collections.clone();
    selected_collections.extend(canon_plan.receipt().collections.iter().map(|c| {
        InputCollectionReceipt {
            source_graph: c.source_graph.clone(),
            collection: c.collection.clone(),
            input_role: c.input_role.clone(),
            adapter_profile: c.adapter_profile.clone(),
            expected_count: c.count,
            expected_root_sha256: c.root_sha256.clone(),
        }
    }));
    for source in &vocabulary.sources {
        if [
            "repository-topology-v1",
            "canon-node-relation-v1",
            "candidate-relation-v1",
        ]
        .contains(&source.adapter_profile.as_str())
        {
            continue;
        }
        if source.adapter_profile == "declared-identity-and-source-ref-joins-v1" {
            // This derived family declares scope, not source node/edge rows.
            // Its actual relations consume the complete same-cut base seal.
            selected_collections.push(raw_receipt(
                &source.source_graph_id,
                "join_scope",
                &source.input_role,
                &source.adapter_profile,
                &BTreeMap::new(),
            ));
            continue;
        }
        let names: &[&str] = if source.adapter_profile == "reified-bibliographic-claims-v1" {
            &["nodes", "edges", "claim_traces"]
        } else {
            &["nodes", "edges"]
        };
        for &collection in names {
            let mut rows = BTreeMap::new();
            let values = match source.adapter_profile.as_str() {
                "source-navigation-node-edge-v1" => navigation.value()[collection]
                    .as_array()
                    .unwrap()
                    .as_slice(),
                "reified-bibliographic-claims-v1" => {
                    graph.0.get(collection).map_or(&[][..], Vec::as_slice)
                }
                // The selected authored recipe has no inputs for these raw
                // families. Zero receipts are explicit; no philosophy original
                // component or whole-source philosophy parity is claimed.
                "philosophy-node-edge-v1" | "indexed-node-edge-v1" => &[],
                _ => panic!("unsupported selected source family"),
            };
            let key = match collection {
                "nodes" => "node_id",
                "edges" => "edge_id",
                "claim_traces" => "claim_ref",
                _ => unreachable!(),
            };
            for value in values {
                let id = value[key].as_str().unwrap().to_owned();
                let raw = serde_json::to_vec(value).unwrap();
                assert!(raw.len() <= limits.catalog.max_output_row_bytes);
                assert!(rows.insert(id, raw).is_none());
            }
            selected_collections.push(raw_receipt(
                &source.source_graph_id,
                collection,
                &source.input_role,
                &source.adapter_profile,
                &rows,
            ));
            assert!(
                selected_rows
                    .insert((source.source_graph_id.clone(), collection.into()), rows)
                    .is_none()
            );
        }
    }
    let mut selected_binding = binding.clone();
    selected_binding.projection_root_sha256 =
        Digest256::of_bytes(&serde_json::to_vec(&selected_collections).unwrap()).to_hex();
    let selected_dir = fixture.path().join("native-selected");
    fs::create_dir(&selected_dir).unwrap();
    let selected_path = selected_dir.join("knowledge.sqlite3");
    // Keep the actual SQLite allocation compatible with the existing final
    // VACUUM's two-database temp reserve. This narrows output, not host quota.
    let mut selected_stage_limits = stage_limits;
    selected_stage_limits.sqlite.max_output_bytes = stage_limits
        .sqlite
        .max_output_bytes
        .min(stage_limits.max_temp_bytes / 2);
    // Every attempted trigram charges at least three UTF-8 bytes before SQL;
    // per-document dedup can only reduce the resulting posting count. Derive
    // this guard from existing work, not the current corpus's observed count.
    let search_work_bytes = (100 * 1024 * 1024u64).min(stage_limits.sqlite.max_work_bytes);
    let search_postings = search_work_bytes / 3;
    let mut selected_stage = KnowledgeStage::create(
        &selected_path,
        selected_stage_limits,
        ExactInputReceipt {
            binding: selected_binding.clone(),
            collections: selected_collections,
        },
        &owner,
        &isolation,
    )
    .unwrap();
    render_repository_source_plan(
        &mut selected_stage,
        &repository_plan,
        &vocabulary,
        deadline,
        &cancelled,
    )
    .unwrap();
    // The source planner is consumed by its first successful render above.
    // Continue from that actual frozen output, never reopen an erased plan or
    // recreate private tables in the independent selected stage.
    let mut transfer_work = 0u64;
    for collection in &canon_plan.receipt().collections {
        let mut after = None;
        let mut count = 0u64;
        let mut root = tos_foundation::Digest256Hasher::new();
        loop {
            assert!(Instant::now() < deadline);
            assert!(!cancelled.load(std::sync::atomic::Ordering::Relaxed));
            let page = target
                .scan_input(
                    &collection.source_graph,
                    &collection.collection,
                    after.as_deref(),
                    16,
                )
                .unwrap();
            let mut borrowed = Vec::with_capacity(page.rows.len());
            for row in &page.rows {
                transfer_work = transfer_work
                    .checked_add(u64::try_from(row.id.len() + row.payload.len()).unwrap())
                    .filter(|n| *n <= stage_limits.sqlite.max_work_bytes)
                    .expect("native selected transfer work budget");
                count += 1;
                assert!(count <= collection.count);
                root.update(&(row.id.len() as u64).to_be_bytes());
                root.update(row.id.as_bytes());
                root.update(Digest256::of_bytes(&row.payload).as_bytes());
                borrowed.push(InputRow {
                    source_graph: &collection.source_graph,
                    collection: &collection.collection,
                    id: &row.id,
                    payload: &row.payload,
                });
            }
            if !borrowed.is_empty() {
                selected_stage.ingest_input_batch(&borrowed).unwrap();
            }
            after = page.next_id;
            if after.is_none() {
                break;
            }
        }
        assert_eq!(count, collection.count);
        assert_eq!(root.finalize().to_hex(), collection.root_sha256);
    }
    for ((source, collection), rows) in &selected_rows {
        let mut entries = rows.iter();
        loop {
            let borrowed = entries
                .by_ref()
                .take(stage_limits.max_seek_rows)
                .map(|(id, raw)| InputRow {
                    source_graph: source,
                    collection,
                    id,
                    payload: raw,
                })
                .collect::<Vec<_>>();
            if borrowed.is_empty() {
                break;
            }
            selected_stage.ingest_input_batch(&borrowed).unwrap();
        }
    }
    // Bind this original plan to the final selected transport, not the prior
    // corpus planner's projection root. The producer validates every other
    // source-selection field and preserves its independent output proof.
    let original = tos_compiler::prepare_native_corpus_original(
        &projection,
        &tos_foundation::RelativePath::parse(&output_path).unwrap(),
        &selected_binding,
        &vocabulary,
        tos_compiler::CorpusOriginalSourceLimits {
            originals,
            max_members: 1,
            max_work_bytes: 64 * 1024 * 1024,
        },
        deadline,
        &cancelled,
    )
    .unwrap();
    let mut navigation_header = navigation.value().clone();
    for field in ["nodes", "edges", "rights"] {
        navigation_header.as_object_mut().unwrap().remove(field);
    }
    let navigation_header_raw = serde_json::to_vec(&navigation_header).unwrap();
    let navigation_claim = tos_compiler::NavigationHeaderClaim {
        expected_sha256: Digest256::of_bytes(&navigation_header_raw).to_hex(),
        raw_json: navigation_header_raw,
    };
    let rights = navigation.value()["rights"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| serde_json::to_vec(r).unwrap())
        .collect::<Vec<_>>();
    let rights_refs = rights.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let rights_root = tos_compiler::navigation_original_rights_root(&rights_refs);
    let additional = tos_compiler::NativeFamilyInputs {
        repository_root: Some(repository_plan.root_input()),
        navigation_original: Some(tos_compiler::NavigationOriginalInput {
            rights: &rights_refs,
            expected_rights_root_sha256: &rights_root,
            limits: originals,
        }),
        philosophy_original: None,
        corpus_original: Some(&original),
        topology: tos_compiler::TopologyLimits {
            max_rows: stage_limits.sqlite.max_rows,
            max_page_rows: 16,
            max_row_bytes: stage_limits.sqlite.max_row_bytes,
            max_work_bytes: stage_limits.sqlite.max_work_bytes,
        },
        canon_prepare: tos_compiler::knowledge_canon_prepare::CanonPrepareLimits {
            max_nodes: stage_limits.sqlite.max_rows,
            max_packs: stage_limits.sqlite.max_rows,
            max_edges: stage_limits.sqlite.max_rows,
            max_node_relations: stage_limits.sqlite.max_rows,
            max_page_rows: 16,
            max_row_bytes: limits.catalog.max_output_row_bytes,
            max_work_bytes: stage_limits.sqlite.max_work_bytes,
        },
        canon: tos_compiler::knowledge_canon_materialize::CanonMaterializeLimits {
            max_raw_bytes: limits.catalog.max_output_row_bytes,
            max_output_bytes: limits.catalog.max_output_row_bytes,
            max_registry_bytes: 4 * 1024 * 1024,
            max_page_rows: 16,
            max_page_bytes: 16 * 1024 * 1024,
            max_rows: stage_limits.sqlite.max_rows,
            max_work_bytes: stage_limits.sqlite.max_work_bytes,
        },
        indexed: tos_compiler::IndexedLimits {
            max_row_bytes: limits.catalog.max_output_row_bytes,
            max_page_rows: 16,
        },
    };
    phase(started, deadline, "selected-model-start");
    let selected = tos_compiler::knowledge_full_fixture::finish_native_source_fixture(
        selected_stage,
        selected_path,
        &files["ToS/doctrine/semantic-interchange/entity-types.v1.json"],
        &files["ToS/doctrine/semantic-interchange/relation-types.v1.json"],
        vocabulary.clone(),
        descriptor,
        &navigation_claim,
        additional,
        tos_compiler::FullKnowledgeLimits {
            scope: tos_compiler::ScopeLimits {
                max_sources: vocabulary.sources.len(),
                max_rows: stage_limits.sqlite.max_rows,
                max_index_work_bytes: 8 * 1024 * 1024,
            },
            catalog: tos_compiler::catalog::CatalogLimits::default(),
            catalog_index: tos_compiler::CatalogIndexLimits::default(),
            search: tos_compiler::SearchBuildLimits {
                max_payload_bytes: limits.catalog.max_output_row_bytes,
                max_document_chars: limits.catalog.max_output_row_bytes,
                max_document_bytes: 4 * 1024 * 1024,
                max_rank_field_bytes: limits.catalog.max_output_row_bytes,
                max_postings: search_postings,
                max_work_bytes: search_work_bytes,
                gram_batch_rows: 64,
            },
            seal: tos_compiler::SealLimits {
                max_header_bytes: limits.catalog.max_output_row_bytes,
            },
            max_registry_bytes: 4 * 1024 * 1024,
        },
    );
    phase(started, deadline, "selected-model-sealed");
    let cold = selected.open().unwrap();
    let receipt = cold.corpus_original_receipt().unwrap();
    assert_eq!(
        receipt.origin.native_producer.as_ref(),
        Some(projection.receipt())
    );
    assert!(receipt.origin.source_git_commit.is_none());
    assert_eq!(receipt.origin.members.len(), 1);
    assert_eq!(
        selected
            .corpus_original
            .as_ref()
            .unwrap()
            .component_root_sha256,
        selected
            .expectation
            .corpus_original_root_sha256
            .as_ref()
            .unwrap()
            .as_str()
    );
    assert_eq!(selected.expectation.source_cut, binding.source_cut);
    assert_eq!(
        selected.expectation.membership_root,
        membership.digest.to_hex()
    );
    assert_eq!(
        selected.expectation.through_commit_seq,
        binding.through_commit_seq
    );
    drop(cold);
    let inspect = tos_query::InspectBudget {
        max_open_vm_steps: selected.cold_limits().max_vm_steps,
        max_read_vm_steps: stage_limits.sqlite.max_sql_vm_steps,
        max_matches: usize::try_from(stage_limits.sqlite.max_rows).unwrap(),
        max_rows: stage_limits.sqlite.max_rows,
        max_field_bytes: 8192,
        max_payload_bytes: originals.max_row_bytes,
        max_decoded_bytes: stage_limits.sqlite.max_work_bytes,
        max_response_bytes: 8 * 1024 * 1024,
        json: tos_foundation::JsonLimits::default(),
    };
    phase(started, deadline, "cold-query-start");
    let packets = assert_native_corpus_query_packets(
        &selected,
        &projection,
        &expected,
        &oracle_root,
        &output_path,
        tos_query::corpus_read::CorpusReadBudget {
            inspect,
            // Existing API caller law: logical JSON/string work has its own
            // bound, distinct from row count and SQLite VM accounting.
            max_work_steps: inspect.max_read_vm_steps,
        },
    );
    phase(started, deadline, "cold-query-matched");
    if installed {
        native_managed_corpus_consumer::exercise_managed_native_corpus(
            &selected,
            &projection,
            &repository,
            &output_path,
            &files,
            &packets,
        );
    }
}

/// Full authored projection contract. OPS runs this ignored functional case
/// only after finite batch protocol checks and aggregate resource admission.
#[test]
#[ignore = "requires admitted aggregate full authored philosophy batch window"]
fn actual_whole_authored_philosophy_batch_plan_render_matches_maintained_python() {
    use std::io::Read;
    use tos_compiler::knowledge_stage::{InputCollectionReceipt, InputRow};
    use tos_compiler::source_philosophy::{
        PHILOSOPHY_CONTRACTS_PROFILE, PHILOSOPHY_CONTRACTS_ROLE, PHILOSOPHY_MEMBERS_PROFILE,
        PHILOSOPHY_MEMBERS_ROLE, PHILOSOPHY_SOURCE_CUSTODY, PhilosophySourceLimits,
        philosophy_source_member_packet, plan_philosophy_source_inputs,
        render_philosophy_projection, render_philosophy_source_plan,
    };
    use tos_validation::executor::{BatchBudget, BatchStreamBudget};
    let window = std::env::var("TOS_PHI_FUNCTIONAL_WINDOW_SECONDS")
        .expect("OPS must supply the admitted aggregate functional window")
        .parse::<u64>()
        .unwrap();
    assert!(
        (1..=3600).contains(&window),
        "finite admitted aggregate window"
    );
    let deadline = Instant::now() + Duration::from_secs(window);
    let cancelled = AtomicBool::new(false);
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let mut authored = schema_sources(&repository);
    // Preserve every tracked eligible authored phi member; no post-derivation
    // filtering, candidate truncation or separately invented product selection.
    let names = git(&repository, &["ls-files", "-z", "--", "ToS/philosophy"]);
    for name in names.split(|b| *b == 0).filter(|v| !v.is_empty()) {
        let name = std::str::from_utf8(name).unwrap();
        if !name.split('/').any(|p| p == "payload")
            && [".json", ".jsonl", ".md"]
                .iter()
                .any(|ext| name.ends_with(ext))
        {
            let raw = fs::read(repository.join(name)).unwrap();
            assert!(raw.len() <= 32 * 1024 * 1024);
            assert!(authored.insert(name.into(), raw).is_none());
        }
    }
    assert!(authored.len() <= 2048);
    let source_bytes = authored.values().map(Vec::len).sum::<usize>();
    assert!(source_bytes <= 64 * 1024 * 1024);
    let fixture = tempfile::tempdir().unwrap();
    let oracle_root = fixture.path().join("authored-oracle");
    for (name, raw) in &authored {
        let path = oracle_root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, raw).unwrap();
    }
    let store = fixture.path().join("source-store");
    let revision = super::validation_cut_cases::write_cut_store(&authored, &store);
    // Drop duplicate fixture bytes before the two Value-based pure derivations.
    drop(authored);
    let read_limits = ReadLimits {
        max_manifest_bytes: 4 * 1024 * 1024,
        max_manifest_entries: 2048,
        max_selected_object_bytes: 32 * 1024 * 1024,
        json: JsonLimits::default(),
    };
    let reader = CorpusReader::open_existing(&store, read_limits).unwrap();
    let cut = reader
        .open_source_cut(
            revision,
            tos_source_store::CutReadLimits {
                max_revisions: 1,
                max_members: 2048,
                max_total_bytes: 64 * 1024 * 1024,
                max_member_bytes: 32 * 1024 * 1024,
            },
            deadline,
            &cancelled,
        )
        .unwrap();
    let membership = cut.stream(revision).unwrap().expectation();
    let worker_path = super::validation_cut_cases::selected_worker_path();
    let mut schemas = CutWorkerSchemaExecutor::from_cut(
        &cut,
        tos_validation::FormatProfile::LegacyPythonObserved20260923,
        ExactWorkerIdentity {
            sha256: Digest256::of_bytes(&fs::read(&worker_path).unwrap()),
            absolute_path: worker_path,
        },
        ExecutorBudget::laboratory(),
        CutWorkerLimits {
            max_receipts: 65_536,
            max_receipt_bytes: 64 * 1024 * 1024,
        },
        deadline,
        &cancelled,
    )
    .unwrap();
    let descriptor = fs::read(
        repository.join("rust/crates/tos-compiler/tests/fixtures/query-vocabulary.v1.json"),
    )
    .unwrap();
    let vocabulary = QueryVocabulary::parse(
        &descriptor,
        &[
            "philosophy-node-edge-v1",
            "canon-node-relation-v1",
            "candidate-relation-v1",
            "source-navigation-node-edge-v1",
            "reified-bibliographic-claims-v1",
            "declared-identity-and-source-ref-joins-v1",
            "repository-topology-v1",
            "indexed-node-edge-v1",
        ],
    )
    .unwrap();
    let mut inputs = BTreeMap::<String, BTreeMap<String, Vec<u8>>>::new();
    for member in cut
        .current()
        .members()
        .filter(|m| m.path.as_str().starts_with("ToS/philosophy/"))
    {
        inputs.entry("current-members".into()).or_default().insert(
            member.path.as_str().into(),
            serde_json::to_vec(&philosophy_source_member_packet(
                revision, membership, member,
            ))
            .unwrap(),
        );
    }
    for contract in [
        tos_compiler::source_philosophy_atlas::ATLAS_SCHEMA,
        tos_compiler::source_philosophy_views::VIEWS_SCHEMA,
        tos_compiler::source_philosophy_graph::GRAPH_SCHEMA,
    ] {
        inputs.entry("contracts".into()).or_default().insert(
            contract.into(),
            fs::read(oracle_root.join(contract)).unwrap(),
        );
    }
    let collections = inputs
        .iter()
        .map(|(name, rows)| {
            let mut root = tos_foundation::Digest256Hasher::new();
            for (id, raw) in rows {
                root.update(&(id.len() as u64).to_be_bytes());
                root.update(id.as_bytes());
                root.update(Digest256::of_bytes(raw).as_bytes());
            }
            let (role, profile) = if name == "current-members" {
                (PHILOSOPHY_MEMBERS_ROLE, PHILOSOPHY_MEMBERS_PROFILE)
            } else {
                (PHILOSOPHY_CONTRACTS_ROLE, PHILOSOPHY_CONTRACTS_PROFILE)
            };
            InputCollectionReceipt {
                source_graph: PHILOSOPHY_SOURCE_CUSTODY.into(),
                collection: name.clone(),
                input_role: role.into(),
                adapter_profile: profile.into(),
                expected_count: rows.len() as u64,
                expected_root_sha256: root.finalize().to_hex(),
            }
        })
        .collect();
    let binding = SourceBinding {
        owner_profile: "private-authored-phi-fixture".into(),
        source_cut: "whole-authored-phi-fixture".into(),
        through_commit_seq: 0,
        membership_root: membership.digest.to_hex(),
        index_generation: revision.0.to_hex(),
        route_map_version: "fixture-v1".into(),
        reader_abi: "fixture-v1".into(),
        projection_root_sha256: Digest256::of_bytes(b"independent-phi-custody").to_hex(),
        complete: true,
    };
    let stage_limits = StageLimits {
        sqlite: Limits {
            max_rows: 65_536,
            max_row_bytes: 8 * 1024 * 1024,
            max_output_bytes: 256 * 1024 * 1024,
            max_work_bytes: 512 * 1024 * 1024,
            sqlite_cache_kib: 1024,
            max_sql_vm_steps: 100_000_000,
        },
        max_temp_bytes: 256 * 1024 * 1024,
        max_seek_rows: 8,
        max_seek_bytes: 64 * 1024 * 1024,
    };
    let owner = FixtureOwner;
    let isolation = FixtureIsolation;
    let mut planner = KnowledgeStage::create(
        &fixture.path().join("phi-planner.sqlite"),
        stage_limits,
        ExactInputReceipt {
            binding: binding.clone(),
            collections,
        },
        &owner,
        &isolation,
    )
    .unwrap();
    for (name, rows) in &inputs {
        let borrowed = rows
            .iter()
            .map(|(id, raw)| InputRow {
                source_graph: PHILOSOPHY_SOURCE_CUSTODY,
                collection: name,
                id,
                payload: raw,
            })
            .collect::<Vec<_>>();
        for chunk in borrowed.chunks(8) {
            planner.ingest_input_batch(chunk).unwrap();
        }
    }
    drop(inputs);
    // Existing maintained builders derive the full atlas, view catalog and
    // graph from this same original cut. Only their generated inputs are bound
    // to newly derived values; source/schema bytes and all instances stay whole.
    let python = r#"import hashlib,json,pathlib,sys
sys.path.insert(0,sys.argv[1]+'/scripts')
import philosophy_atlas_projection_common as a,philosophy_graph_views_common as v,philosophy_graph_projection_common as g,philosophy_multilingual_common as m
root=pathlib.Path(sys.argv[2]);out=pathlib.Path(sys.argv[3])
for owner in (a,v,g):owner.REPO_ROOT=root;owner.TOS_ROOT=root/'ToS'
m.REPO_ROOT=root;m.LEDGER_PATH=root/m.LEDGER_REF
atlas=a.build_payload();load=v.load_json
v.load_json=lambda p:atlas if p==root/v.ATLAS_PROJECTION_REF else load(p)
views=v.build_payload();loadg=g.load_json
g.load_json=lambda p:atlas if p==root/g.ATLAS_PROJECTION_REF else views if p==root/g.GRAPH_VIEW_CATALOG_REF else loadg(p)
graph=g.build_payload()
def raw(value):return json.dumps(value,ensure_ascii=False,sort_keys=True,separators=(',',':'),allow_nan=False).encode()
units=total=batches=context=receipt_strings=0;maximum=0
resources=[json.loads(p.read_text()) for p in sorted((root/'ToS/contracts').glob('*.schema.json'))]
resource_frame=sum(8+len(x['$id'].encode())+len(p.read_bytes()) for x,p in zip(resources,sorted((root/'ToS/contracts').glob('*.schema.json'))))
for value,ref,contract in [(atlas,v.ATLAS_PROJECTION_REF,'ToS/contracts/philosophy-atlas-projection.schema.json'),(views,g.GRAPH_VIEW_CATALOG_REF,'ToS/contracts/philosophy-graph-views.schema.json'),(graph,'ToS/derived-exports/philosophy_graph_projection.min.json','ToS/contracts/philosophy-graph-projection.schema.json')]:
    pending=count=0;schema_id=json.loads((root/contract).read_text())['$id']
    for key in sorted(value):
        values=value[key] if isinstance(value[key],list) else [value[key]]
        for index,instance in enumerate(values):
            size=len(raw(instance));maximum=max(maximum,size);units+=1;total+=size
            if count==64 or pending+size>32*1024*1024:batches+=1;pending=count=0
            pointer='/properties/'+key.replace('~','~0').replace('/','~1')+('/items' if isinstance(value[key],list) else '')
            path=ref+'#/'+key+('/'+str(index) if isinstance(value[key],list) else '')
            uri=schema_id+'#'+pointer
            context+=24+len(str(count).encode())+len(path.encode())+len(uri.encode())
            receipt_strings+=len(path.encode())+len((contract+'#'+pointer).encode())
            pending+=size;count+=1
    if count:batches+=1
collections={}
for name,key in [('nodes','node_id'),('edges','edge_id'),('views','view_id'),('clusters','cluster_id'),('review_packets','packet_id'),('unresolved_review_surfaces','surface_id')]:
    h=hashlib.sha256()
    for identity,row in sorted((x[key],raw(x)) for x in graph[name]):
        identity=identity.encode();h.update(len(identity).to_bytes(8,'big'));h.update(identity);h.update(hashlib.sha256(row).digest())
    collections[name]={'count':len(graph[name]),'root':h.hexdigest()}
with out.open('wb') as stream:
    for part in json.JSONEncoder(ensure_ascii=False,sort_keys=True,separators=(',',':'),allow_nan=False).iterencode(graph):stream.write(part.encode())
    stream.write(b'\n')
print(json.dumps({'atlas_counts':atlas['counts'],'graph_counts':graph['counts'],'collections':collections,'schema_units_estimate':units,'schema_batches_estimate':batches,'schema_raw_bytes_estimate':total,'schema_context_bytes_estimate':context,'schema_resource_frame_estimate':resource_frame,'schema_receipt_strings_estimate':receipt_strings,'schema_max_instance_estimate':maximum,'graph_bytes':out.stat().st_size},sort_keys=True))
"#;
    let expected_path = fixture.path().join("expected-whole-graph.json");
    let output = Command::new("python3")
        .args(["-c", python])
        .arg(&repository)
        .arg(&oracle_root)
        .arg(&expected_path)
        .env_remove("PYTHONPATH")
        .env_remove("PYTHONHOME")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "maintained whole phi oracle: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected: Value = serde_json::from_slice(&output.stdout).unwrap();
    // Python transport accounting is an admission estimate, not a maintained
    // output contract. Actual coverage/caps are enforced by native execution.
    assert!(expected["schema_units_estimate"].as_u64().unwrap() <= 65_536);
    assert!(expected["schema_batches_estimate"].as_u64().unwrap() <= 1024);
    assert!(expected["schema_raw_bytes_estimate"].as_u64().unwrap() <= 256 * 1024 * 1024);
    assert!(
        expected["schema_max_instance_estimate"].as_u64().unwrap()
            <= tos_validation::SchemaBackendProbe::MAX_INSTANCE_BYTES as u64
    );
    let receipt_bytes = expected["schema_units_estimate"].as_u64().unwrap()
        * std::mem::size_of::<tos_validation::source_cut::CutSchemaReceipt>() as u64
        + expected["schema_receipt_strings_estimate"]
            .as_u64()
            .unwrap();
    assert!(receipt_bytes <= 64 * 1024 * 1024);
    let encoded_input = expected["schema_batches_estimate"].as_u64().unwrap()
        * (33 + expected["schema_resource_frame_estimate"].as_u64().unwrap())
        + expected["schema_raw_bytes_estimate"].as_u64().unwrap()
        + expected["schema_context_bytes_estimate"].as_u64().unwrap();
    assert!(encoded_input <= 2 * 1024 * 1024 * 1024u64);
    let limits = PhilosophySourceLimits {
        max_manifest_members: 2048,
        max_custody_members: 2048,
        max_page_rows: 8,
        max_page_bytes: 64 * 1024 * 1024,
        schema_batches: BatchStreamBudget {
            batch: BatchBudget::laboratory(),
            max_chunks: 1024,
            max_total_units: 65_536,
            max_total_raw_bytes: 256 * 1024 * 1024,
            total_execution_wall: Duration::from_secs(window),
            operation_cpu_seconds: window,
            operation_address_space_bytes: BatchBudget::laboratory().address_space_bytes,
            max_total_wire_bytes: 2 * 1024 * 1024 * 1024,
            max_distinct_selectors: BatchBudget::MAX_UNITS,
        },
        ..PhilosophySourceLimits::default()
    };
    let plan = plan_philosophy_source_inputs(
        &mut planner,
        &cut,
        revision,
        membership,
        &vocabulary,
        &mut schemas,
        limits,
        deadline,
        &cancelled,
    )
    .unwrap();
    assert_eq!(plan.receipt().atlas_counts, expected["atlas_counts"]);
    assert_eq!(plan.receipt().graph_counts, expected["graph_counts"]);
    assert_eq!(schemas.receipts().len() as u64, plan.receipt().schema_units);
    assert!(
        schemas
            .receipts()
            .iter()
            .all(|r| r.valid && r.batch.is_some() && r.source_revision == revision)
    );
    for collection in plan
        .receipt()
        .raw_collections
        .iter()
        .chain(&plan.receipt().material_collections)
    {
        if let Some(expected) = expected["collections"].get(&collection.collection) {
            assert_eq!(collection.count, expected["count"].as_u64().unwrap());
            assert_eq!(collection.root_sha256, expected["root"].as_str().unwrap());
        }
    }
    let mut expected_stream = fs::File::open(&expected_path).unwrap();
    let mut compared = 0u64;
    let mut expected_hash = tos_foundation::Digest256Hasher::new();
    let actual_sha = render_philosophy_projection(
        &mut planner,
        &plan,
        &cut,
        revision,
        membership,
        &vocabulary,
        limits,
        deadline,
        &cancelled,
        |actual| {
            let mut expected = vec![0; actual.len()];
            expected_stream.read_exact(&mut expected)?;
            assert_eq!(
                actual,
                expected.as_slice(),
                "maintained whole phi stream at byte {compared}"
            );
            expected_hash.update(&expected);
            compared += actual.len() as u64;
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(actual_sha, expected_hash.finalize().to_hex());
    assert_eq!(compared, expected["graph_bytes"].as_u64().unwrap());
    assert_eq!(expected_stream.read(&mut [0]).unwrap(), 0);
    let target_collections = plan
        .receipt()
        .raw_collections
        .iter()
        .map(|c| InputCollectionReceipt {
            source_graph: c.source_graph.clone(),
            collection: c.collection.clone(),
            input_role: c.input_role.clone(),
            adapter_profile: c.adapter_profile.clone(),
            expected_count: c.count,
            expected_root_sha256: c.root_sha256.clone(),
        })
        .collect();
    let mut target_binding = binding;
    target_binding.projection_root_sha256 = actual_sha;
    let mut target = KnowledgeStage::create(
        &fixture.path().join("phi-target.sqlite"),
        stage_limits,
        ExactInputReceipt {
            binding: target_binding,
            collections: target_collections,
        },
        &owner,
        &isolation,
    )
    .unwrap();
    render_philosophy_source_plan(
        &mut planner,
        &plan,
        &cut,
        revision,
        membership,
        &vocabulary,
        &mut target,
        limits,
        deadline,
        &cancelled,
    )
    .unwrap();
    for c in &plan.receipt().raw_collections {
        let mut after = None;
        let mut count = 0u64;
        let mut root = tos_foundation::Digest256Hasher::new();
        loop {
            let page = target
                .scan_input(&c.source_graph, &c.collection, after.as_deref(), 8)
                .unwrap();
            for row in page.rows {
                count += 1;
                root.update(&(row.id.len() as u64).to_be_bytes());
                root.update(row.id.as_bytes());
                root.update(Digest256::of_bytes(&row.payload).as_bytes());
            }
            after = page.next_id;
            if after.is_none() {
                break;
            }
        }
        assert_eq!(count, c.count);
        assert_eq!(root.finalize().to_hex(), c.root_sha256);
    }
    eprintln!(
        "whole_phi source_bytes={source_bytes} encoded_input={encoded_input} receipt_bytes={receipt_bytes} schema_units={} batches={} raw_bytes={} graph_bytes={compared} work_bytes={}",
        plan.receipt().schema_units,
        plan.receipt().schema_batches,
        plan.receipt().schema_raw_bytes,
        plan.receipt().work_bytes
    );
}
