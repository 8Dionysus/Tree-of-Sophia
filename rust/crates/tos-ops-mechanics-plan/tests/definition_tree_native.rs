#[path = "../src/definition_tree.rs"]
mod tree;
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    root: PathBuf,
    tree: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "tos-definition-tree-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let tree = root.join("tree");
        let baseline = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../ToS/doctrine/semantic-interchange/definitions/baseline");
        tree::split(
            &baseline.join("entity-types.v1.json"),
            &baseline.join("relation-types.v1.json"),
            &tree,
        )
        .unwrap();
        Self { root, tree }
    }
    fn manifest(&self) -> Value {
        read(&self.tree.join("manifest.json"))
    }
    fn set_manifest(&self, m: &Value) {
        write(&self.tree.join("manifest.json"), m)
    }
    fn home(&self, id: &str) -> PathBuf {
        let m = self.manifest();
        self.tree.join(
            m["entries"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["id"] == id)
                .unwrap()["path"]
                .as_str()
                .unwrap(),
        )
    }
    fn rejection(&self, expected: &str) {
        match tree::assemble(&self.tree) {
            Ok(_) => panic!("expected {expected}"),
            Err(e) => assert_eq!(e.code, expected, "{e}"),
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn read(p: &Path) -> Value {
    serde_json::from_slice(&fs::read(p).unwrap()).unwrap()
}
fn write(p: &Path, v: &Value) {
    fs::write(p, serde_json::to_vec_pretty(v).unwrap()).unwrap()
}
fn semantic_without_labels(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .filter(|(k, _)| !["labels", "inverse_labels"].contains(&k.as_str()))
                .map(|(k, v)| (k.clone(), semantic_without_labels(v)))
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(semantic_without_labels).collect()),
        _ => v.clone(),
    }
}
#[test]
fn t0_001_full_inventory_split_assemble() {
    let f = Fixture::new();
    // Same historical inventory in both postures; selection must not alter output.
    for posture in ["candidate-not-selected", "selected-authored"] {
        if posture == "selected-authored" {
            select_authored(&f);
        }
        let a = tree::assemble(&f.tree).unwrap();
        a.check_migration().unwrap();
        assert_eq!(a.registries[0]["types"].as_array().unwrap().len(), 160);
        assert_eq!(
            a.registries[0]["property_definitions"]
                .as_array()
                .unwrap()
                .len(),
            168
        );
        assert_eq!(a.registries[1]["relations"].as_array().unwrap().len(), 196);
        let evidence = a.evidence().unwrap();
        assert_eq!(evidence["posture"], posture);
        if posture == "selected-authored" {
            assert_eq!(
                evidence["selection_review_ref"],
                "ToS/review-ledger/2026-10-19-definition-topology-review.md"
            );
        } else {
            assert!(evidence["selection_review_ref"].is_null());
        }
        for r in evidence["registries"].as_array().unwrap() {
            assert_eq!(r["parsed_value_equal"], true);
            assert_eq!(
                r["byte_equal"], false,
                "pinned baseline uses mixed inline/pretty formatting; emitted uniform pretty JSON is a separate format"
            );
        }
        let dir = f.root.join(format!("{posture}-candidate"));
        a.write_candidate(&dir, false).unwrap();
        a.check_generated(&dir).unwrap();
        let dir2 = f.root.join(format!("{posture}-candidate2"));
        a.write_candidate(&dir2, false).unwrap();
        for name in [
            "entity-types.v1.json",
            "relation-types.v1.json",
            "evidence.json",
        ] {
            assert_eq!(
                fs::read(dir.join(name)).unwrap(),
                fs::read(dir2.join(name)).unwrap()
            );
        }
    }
}

#[test]
fn t0_002_duplicate_home() {
    let f = Fixture::new();
    let mut m = f.manifest();
    let e = m["entries"][0].clone();
    m["entries"].as_array_mut().unwrap().push(e);
    f.set_manifest(&m);
    f.rejection("DuplicateCanonicalDefinition");
}
#[test]
fn t0_003_unresolved_semantic_parent() {
    let f = Fixture::new();
    let p = f.home("tos.entity.place");
    let mut d = read(&p);
    d["parent_type_ids"] = json!(["tos.entity.missing"]);
    write(&p, &d);
    f.rejection("UnresolvedParent");
}
#[test]
fn t0_004_navigation_cycle() {
    let f = Fixture::new();
    let mut m = f.manifest();
    let a = m["entries"][0]["id"].clone();
    let b = m["entries"][1]["id"].clone();
    m["entries"][0]["navigation_parent"] = b;
    m["entries"][1]["navigation_parent"] = a;
    f.set_manifest(&m);
    f.rejection("NavigationCycle");
}
#[test]
fn t0_005_semantic_edge_loss() {
    let f = Fixture::new();
    let p = f.home("tos.entity.place");
    let mut d = read(&p);
    d["parent_type_ids"] = json!(["tos.entity.spatial-object"]);
    write(&p, &d);
    f.rejection("SemanticEdgeLoss");
}
#[test]
fn t0_006_move_home_preserves_answers_and_history() {
    let f = Fixture::new();
    let before = tree::assemble(&f.tree).unwrap().registries;
    let mut m = f.manifest();
    let e = m["entries"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|e| e["id"] == "tos.entity.place")
        .unwrap();
    let old = e["path"].as_str().unwrap().to_string();
    let new = "canonical/entities/moved-place.json";
    fs::rename(f.tree.join(&old), f.tree.join(new)).unwrap();
    e["path"] = json!(new);
    e["previous_paths"] = json!([old]);
    f.set_manifest(&m);
    assert_eq!(before, tree::assemble(&f.tree).unwrap().registries);
    assert_eq!(
        f.manifest()["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["id"] == "tos.entity.place")
            .unwrap()["previous_paths"][0],
        old
    );
}
#[test]
fn t0_007_separate_navigation_root_preserves_relation_forest() {
    let f = Fixture::new();
    let before = tree::assemble(&f.tree).unwrap().registries;
    let mut m = f.manifest();
    m["navigation_roots"]
        .as_array_mut()
        .unwrap()
        .push(json!("navigation.relation-browser"));
    m["navigation_groups"]["navigation.relations"] = json!("navigation.relation-browser");
    m["navigation_roots"]
        .as_array_mut()
        .unwrap()
        .retain(|x| x != "navigation.relations");
    f.set_manifest(&m);
    assert_eq!(before, tree::assemble(&f.tree).unwrap().registries);
}
#[test]
fn t0_009_unplaced_home() {
    let f = Fixture::new();
    let mut m = f.manifest();
    m["entries"][0]["navigation_parent"] = json!("navigation.missing");
    f.set_manifest(&m);
    f.rejection("UnplacedDefinition");
}
#[test]
fn t0_010_label_changes_do_not_change_semantics() {
    let f = Fixture::new();
    let before = tree::assemble(&f.tree).unwrap().registries;
    let p = f.home("tos.entity.place");
    let mut d = read(&p);
    d["labels"] = json!({"default":"Renamed display", "en":"Somewhere"});
    write(&p, &d);
    let after = tree::assemble(&f.tree).unwrap().registries;
    for i in 0..2 {
        assert_eq!(
            semantic_without_labels(&before[i]),
            semantic_without_labels(&after[i])
        );
    }
}
#[test]
fn t0_012_interrupted_write_does_not_publish() {
    let f = Fixture::new();
    let a = tree::assemble(&f.tree).unwrap();
    let old = f.root.join("accepted");
    a.write_candidate(&old, false).unwrap();
    let before = fs::read(old.join("entity-types.v1.json")).unwrap();
    let candidate = f.root.join("candidate");
    assert_eq!(
        a.write_candidate(&candidate, true).unwrap_err().code,
        "NotPublished"
    );
    assert!(!candidate.exists());
    assert_eq!(before, fs::read(old.join("entity-types.v1.json")).unwrap());
    a.check_generated(&old).unwrap();
}
#[test]
fn extra_dangling_endpoint_and_baseline_tamper() {
    let f = Fixture::new();
    let p = f.home("tos.relation.related");
    let mut d = read(&p);
    d["range_type_ids"] = json!(["tos.entity.missing"]);
    write(&p, &d);
    f.rejection("DanglingReference");
    let m = f.manifest();
    fs::write(
        f.tree.join(m["baseline"][0]["path"].as_str().unwrap()),
        b"{}",
    )
    .unwrap();
    f.rejection("BaselineDigestMismatch");
}
#[test]
fn extra_shadow_copy_and_path_escape() {
    let f = Fixture::new();
    fs::copy(
        f.home("tos.entity.place"),
        f.tree.join("canonical/entities/shadow.json"),
    )
    .unwrap();
    f.rejection("UnlistedDefinition");
    fs::remove_file(f.tree.join("canonical/entities/shadow.json")).unwrap();
    let mut m = f.manifest();
    m["entries"][0]["path"] = json!("canonical/../../outside.json");
    f.set_manifest(&m);
    f.rejection("UnsafePath");
}

#[cfg(feature = "mechanics")]
#[test]
fn existing_schema_and_registry_consumers_accept_full_candidate_and_new_instance() {
    use std::sync::atomic::AtomicI32;
    use std::time::{Duration, Instant};
    use tos_validation::item_rules::ItemLimits;
    let f = Fixture::new();
    let a = tree::assemble(&f.tree).unwrap();
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    for (i, name) in [
        "semantic-entity-type-registry.schema.json",
        "semantic-relation-type-registry.schema.json",
    ]
    .iter()
    .enumerate()
    {
        let schema = read(&repo.join("ToS/contracts").join(name));
        let validator = jsonschema::validator_for(&schema).unwrap();
        let errors: Vec<_> = validator
            .iter_errors(&a.registries[i])
            .map(|e| e.to_string())
            .collect();
        assert!(errors.is_empty(), "{name}: {errors:?}");
    }
    let limits = ItemLimits {
        max_member_bytes: 2 * 1024 * 1024,
        max_total_bytes: 16 * 1024 * 1024,
        max_state_bytes: 64 * 1024 * 1024,
        max_issues: 4096,
        deadline: Instant::now() + Duration::from_secs(30),
    };
    let report = tos_validation::semantic_registry_rules::validate_semantic_registries(
        &a.registries[0],
        &a.registries[1],
        Some((&a.registries[0], &a.registries[1])),
        limits,
        &AtomicI32::new(0),
    )
    .unwrap();
    assert_eq!(report["valid"], json!(true), "{report}");
    let schema = read(&repo.join("ToS/contracts/corpus-record.schema.json"));
    let validator = jsonschema::validator_for(&schema).unwrap();
    for (suffix, label) in [
        ("unseen-alpha", "A new place"),
        ("different-beta", "Другое место"),
    ] {
        let instance = json!({"schema_version":"tos_corpus_record_v1","record_type":"place","record_id":format!("tos.place.{suffix}"),"preferred_label":label,"identity_status":"provisional","source_refs":["synthetic/evidence.json"],"external_identifiers":[],"same_as_posture":"not_assessed","record_version":1});
        assert!(validator.is_valid(&instance), "{instance}");
    }
    let invalid = json!({"schema_version":"tos_corpus_record_v1","record_type":"place"});
    assert!(!validator.is_valid(&invalid));
}

fn select_authored(f: &Fixture) {
    let mut m = f.manifest();
    m["posture"] = json!("selected-authored");
    m["selection_review_ref"] = json!("ToS/review-ledger/2026-10-19-definition-topology-review.md");
    f.set_manifest(&m);
}

#[test]
fn selection_requires_explicit_source_review_locator_and_known_posture() {
    let f = Fixture::new();
    let mut m = f.manifest();
    m["posture"] = json!("selected-authored");
    f.set_manifest(&m);
    f.rejection("InvalidManifest");
    m["selection_review_ref"] = json!("/tmp/review.md");
    f.set_manifest(&m);
    f.rejection("InvalidManifest");
    m["selection_review_ref"] = json!("ToS/review-ledger/../invented.md");
    f.set_manifest(&m);
    f.rejection("InvalidManifest");
    m["selection_review_ref"] = json!("ToS/review-ledger/2026-10-19-definition-topology-review.md");
    m["posture"] = json!("unreviewed-auto-selection");
    f.set_manifest(&m);
    f.rejection("InvalidManifest");
}

#[test]
fn generated_compatibility_pair_keeps_t0_008_alias_and_plain_directory_contract() {
    let f = Fixture::new();
    select_authored(&f);
    let a = tree::assemble(&f.tree).unwrap();
    let out = f.root.join("flat-registry-directory");
    a.write_candidate(&out, false).unwrap();
    fs::remove_file(out.join("evidence.json")).unwrap();
    // Unrelated target contents, even an invalid assembly manifest, are not inputs.
    fs::write(out.join("manifest.json"), b"not an assembly manifest").unwrap();
    a.check_compatibility(&out).unwrap();
    let relation = fs::read(out.join("relation-types.v1.json")).unwrap();
    fs::remove_file(out.join("relation-types.v1.json")).unwrap();
    assert!(a.check_compatibility(&out).is_err());
    fs::write(out.join("relation-types.v1.json"), &relation).unwrap();
    let entity = fs::read(out.join("entity-types.v1.json")).unwrap();
    let mut whitespace_drift = entity.clone();
    whitespace_drift.push(b'\n');
    fs::write(out.join("entity-types.v1.json"), whitespace_drift).unwrap();
    assert_eq!(
        a.check_compatibility(&out).unwrap_err().code,
        "GeneratedOutputDivergence"
    );
    fs::write(out.join("entity-types.v1.json"), entity).unwrap();
    fs::write(out.join("relation-types.v1.json"), b"{}").unwrap();
    assert_eq!(
        a.check_compatibility(&out).unwrap_err().code,
        "GeneratedOutputDivergence"
    );
    assert_eq!(
        a.check_generated(&out).unwrap_err().code,
        "GeneratedOutputDivergence"
    );
}

#[test]
fn selected_parent_evolution_is_not_a_permanent_migration_freeze() {
    let f = Fixture::new();
    select_authored(&f);
    let p = f.home("tos.entity.place");
    let mut d = read(&p);
    d["parent_type_ids"]
        .as_array_mut()
        .unwrap()
        .push(json!("tos.entity.semantic-object"));
    write(&p, &d);
    let a = tree::assemble(&f.tree).unwrap();
    assert_eq!(a.check_migration().unwrap_err().code, "SemanticEdgeLoss");
    // The historical document is still exact, while current definitions can evolve.
    assert_eq!(
        a.evidence().unwrap()["registries"][0]["parsed_value_equal"],
        false
    );
    d["parent_type_ids"] = json!(["tos.entity.missing"]);
    write(&p, &d);
    f.rejection("UnresolvedParent");
}

#[test]
fn selected_new_type_and_predicate_extend_data_without_generic_handler_changes() {
    let f = Fixture::new();
    select_authored(&f);
    let mut m = f.manifest();
    let mut entity = read(&f.home("tos.entity.place"));
    entity["type_id"] = json!("tos.entity.navigation-extension-example");
    entity["source_mappings"] = json!([]);
    entity["definition"] = json!("Synthetic type extension used only by this structural test.");
    let entity_path = "canonical/entities/extension-example.json";
    write(&f.tree.join(entity_path), &entity);
    m["entries"].as_array_mut().unwrap().push(json!({"id":entity["type_id"],"collection":"types","path":entity_path,"navigation_parent":"tos.entity.identity","previous_paths":[]}));
    let mut relation = read(&f.home("tos.relation.related"));
    relation["relation_type_id"] = json!("tos.relation.navigation-extension-example");
    relation["abstract"] = json!(false);
    relation["parent_relation_type_ids"] = json!(["tos.relation.related"]);
    relation["domain_type_ids"] = json!([entity["type_id"]]);
    relation["range_type_ids"] = json!(["tos.entity.place"]);
    relation["source_mappings"] = json!([]);
    relation["definition"] =
        json!("Synthetic predicate extension used only by this structural test.");
    let relation_path = "canonical/relations/extension-example.json";
    write(&f.tree.join(relation_path), &relation);
    m["entries"].as_array_mut().unwrap().push(json!({"id":relation["relation_type_id"],"collection":"relations","path":relation_path,"navigation_parent":"tos.relation.related","previous_paths":[]}));
    f.set_manifest(&m);
    let a = tree::assemble(&f.tree).unwrap();
    assert_eq!(
        a.check_migration().unwrap_err().code,
        "MigrationValueDivergence"
    );
    assert_eq!(a.registries[0]["types"].as_array().unwrap().len(), 161);
    assert_eq!(a.registries[1]["relations"].as_array().unwrap().len(), 197);
    assert_eq!(a.manifest.entries.len(), 526);
    // A definition or ancestry does not grant a source writer/profile capability.
    assert!(entity.get("source_record_profile").is_none());
    assert!(relation.get("source_claim_profile").is_none());
}

#[test]
fn selected_preserves_historical_ids_and_strict_initial_full_value_gate() {
    let f = Fixture::new();
    select_authored(&f);
    let p = f.home("tos.entity.place");
    let mut d = read(&p);
    d["labels"]["default"] = json!("A reviewed later display label");
    write(&p, &d);
    let a = tree::assemble(&f.tree).unwrap();
    assert_eq!(
        a.check_migration().unwrap_err().code,
        "MigrationValueDivergence"
    );
    let mut m = f.manifest();
    let index = m["entries"]
        .as_array()
        .unwrap()
        .iter()
        .position(|e| e["collection"] == "property_definitions")
        .unwrap();
    let entry = m["entries"].as_array_mut().unwrap().remove(index);
    fs::remove_file(f.tree.join(entry["path"].as_str().unwrap())).unwrap();
    f.set_manifest(&m);
    f.rejection("MissingBaselineDefinition");
}
