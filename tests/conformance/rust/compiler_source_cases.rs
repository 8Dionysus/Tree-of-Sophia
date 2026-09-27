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
use tos_validation::source_cut::{CutWorkerLimits, CutWorkerSchemaExecutor};

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
fn sources() -> (BTreeMap<String, Vec<u8>>, BTreeMap<String, Vec<u8>>) {
    let repository = super::validation_cut_cases::repository();
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
            max_page_rows: 128,
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
