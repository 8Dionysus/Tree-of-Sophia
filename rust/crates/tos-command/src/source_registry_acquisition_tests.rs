use super::*;
use serde_json::{Value, json};
use sha1::Digest as _;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

struct Fixture {
    _temp: TempDir,
    root: PathBuf,
    payload_root: PathBuf,
    manifest_path: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repository");
        let payload_root = temp.path().join("payload-root");
        let contracts = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../ToS/contracts")
            .canonicalize()
            .unwrap();
        fs::create_dir_all(root.join("ToS/contracts")).unwrap();
        for entry in fs::read_dir(contracts).unwrap() {
            let path = entry.unwrap().path();
            if path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .ends_with(".schema.json")
            {
                fs::copy(
                    &path,
                    root.join("ToS/contracts").join(path.file_name().unwrap()),
                )
                .unwrap();
            }
        }
        fs::create_dir(&payload_root).unwrap();
        fs::set_permissions(&payload_root, fs::Permissions::from_mode(0o700)).unwrap();

        let package = json!({
            "target_slug":"fixture-sutta",
            "records":{},
            "rights":{
                "schema_version":"tos_rights_record_v1",
                "rights_id":"tos.rights.registry-fixture",
                "scope_refs":["fixture:registry"],
                "assessment_status":"not_assessed",
                "jurisdictions_reviewed":[],
                "source_refs":["fixture:registry"],
                "permissions":[],
                "restrictions":[],
                "visibility":"local_only",
                "redistribution_posture":"not_authorized",
                "derivative_posture":"local_research_only",
                "assessed_by":{"maker_type":"imported_source","agent_ref":"software:fixture"},
                "assessed_at":"2026-10-01T00:00:00+00:00",
                "rationale":"Synthetic fixture metadata only.",
                "review_status":"unreviewed",
                "record_version":1
            },
            "claims":[]
        });
        let package_ref = "ToS/source-witnesses/discovery/fixture/prepared-source-packages.jsonl";
        let package_path = root.join(package_ref);
        fs::create_dir_all(package_path.parent().unwrap()).unwrap();
        let mut package_bytes = serde_json::to_vec(&package).unwrap();
        package_bytes.push(b'\n');
        fs::write(&package_path, &package_bytes).unwrap();

        let snapshot_ref = "ToS/source-witnesses/discovery/fixture/source-registry-snapshot.json";
        let snapshot = b"{\"fixture\":\"bound normalized snapshot\"}\n";
        fs::write(root.join(snapshot_ref), snapshot).unwrap();
        let pin = "d6d54741b7f2ddfeca82f02c3f95eb3990b4e351";
        let manifest = json!({
            "schema_version":"tos_registry_first_planting_preparation_v1",
            "status":"prepared-not-acquired",
            "prepared_packages_ref":package_ref,
            "prepared_packages_sha256":sha256(&package_bytes),
            "source_registry_snapshot_ref":snapshot_ref,
            "source_registry_snapshot_sha256":sha256(snapshot),
            "provider_pins":{"bilara":pin},
            "metadata_observations":[],
            "targets":[{
                "slug":"fixture-sutta",
                "family":"pali-canon",
                "repository":"suttacentral/bilara-data",
                "pin":pin,
                "files":[]
            }],
            "totals":{"works":1,"payload_files":0,"payload_bytes":0}
        });
        let manifest_ref = "ToS/source-witnesses/discovery/fixture/manifest.json";
        let manifest_path = root.join(manifest_ref);
        fs::write(&manifest_path, json_bytes(&manifest).unwrap()).unwrap();

        Self {
            _temp: temp,
            root,
            payload_root,
            manifest_path,
        }
    }

    fn request(&self, operation: &str) -> Value {
        json!({
            "operation":operation,
            "root":self.root,
            "manifest_path":self.manifest_path,
            "payload_source_root":self.payload_root
        })
    }
}

fn tree_snapshot(root: &Path) -> Vec<(PathBuf, u32, Option<Vec<u8>>)> {
    fn visit(root: &Path, directory: &Path, output: &mut Vec<(PathBuf, u32, Option<Vec<u8>>)>) {
        let mut entries = fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        entries.sort();
        for path in entries {
            let metadata = fs::symlink_metadata(&path).unwrap();
            let body = if metadata.is_dir() {
                None
            } else {
                Some(fs::read(&path).unwrap())
            };
            output.push((
                path.strip_prefix(root).unwrap().to_path_buf(),
                metadata.permissions().mode() & 0o777,
                body,
            ));
            if metadata.is_dir() {
                visit(root, &path, output);
            }
        }
    }

    let mut output = Vec::new();
    visit(root, root, &mut output);
    output
}

#[test]
fn registry_preparation_verification_is_bound_and_payload_free() {
    let fixture = Fixture::new();
    let before = tree_snapshot(&fixture.root);
    let result = invoke(&fixture.request("registry.verify_preparation")).unwrap();

    assert_eq!(result["status"], "preparation-verified");
    assert_eq!(result["registry_bound"], true);
    assert_eq!(result["payloads_downloaded"], false);
    assert_eq!(result["totals"]["payload_files"], 0);
    assert_eq!(result["totals"]["payload_bytes"], 0);
    assert_eq!(tree_snapshot(&fixture.root), before);
    assert!(tree_snapshot(&fixture.payload_root).is_empty());
}

#[test]
fn registry_preparation_rejects_a_payload_url_outside_its_pinned_provider() {
    let fixture = Fixture::new();
    let mut manifest = read_json(&fixture.manifest_path).unwrap();
    manifest["targets"][0]["paths"] = json!({
        "item_root":"ToS/source-witnesses/works/pali-canon/fixture-sutta/items/root"
    });
    manifest["targets"][0]["files"] = json!([{
        "upstream_path":"root/pli/ms/wrong.json",
        "basename":"wrong.json",
        "byte_size":1,
        "git_blob_sha1":"0000000000000000000000000000000000000000",
        "media_type":"application/json",
        "url":"https://example.invalid/unbound-source"
    }]);
    manifest["totals"]["payload_files"] = json!(1);
    manifest["totals"]["payload_bytes"] = json!(1);
    fs::write(&fixture.manifest_path, json_bytes(&manifest).unwrap()).unwrap();
    let before = tree_snapshot(&fixture.root);

    let error = invoke(&fixture.request("registry.verify_preparation")).unwrap_err();
    assert!(error.contains("unbound source URL"));
    assert_eq!(tree_snapshot(&fixture.root), before);
    assert!(tree_snapshot(&fixture.payload_root).is_empty());
}

#[test]
fn registry_acquisition_rejects_invalid_selection_before_any_write() {
    let fixture = Fixture::new();
    let repository_before = tree_snapshot(&fixture.root);
    let payloads_before = tree_snapshot(&fixture.payload_root);

    let mut malformed = fixture.request("registry.acquire");
    malformed["target_slugs"] = json!("fixture-sutta");
    assert!(
        invoke(&malformed)
            .unwrap_err()
            .contains("target_slugs must be an array of strings")
    );

    let mut unknown = fixture.request("registry.acquire");
    unknown["target_slugs"] = json!(["unknown-target"]);
    assert!(
        invoke(&unknown)
            .unwrap_err()
            .contains("unknown requested acquisition target")
    );

    let mut malformed_root = fixture.request("registry.acquire");
    malformed_root["payload_source_root"] = json!({"path":fixture.payload_root});
    assert!(
        invoke(&malformed_root)
            .unwrap_err()
            .contains("payload_source_root must be a string")
    );

    assert_eq!(tree_snapshot(&fixture.root), repository_before);
    assert_eq!(tree_snapshot(&fixture.payload_root), payloads_before);
}

#[test]
fn registry_transfer_keeps_exact_pinned_bytes_and_a_completed_receipt() {
    let fixture = Fixture::new();
    let target = json!({
        "slug":"fixture-sutta",
        "paths":{"item_root":"ToS/source-witnesses/works/pali-canon/fixture-sutta/items/root"}
    });
    let item_root = target["paths"]["item_root"].as_str().unwrap();
    let basename = "fixture_root-pli-ms.json";
    let body = b"{\"fixture:1\":\"Exact fixture source text\"}\n".to_vec();
    let mut blob = sha1::Sha1::new();
    blob.update(format!("blob {}\0", body.len()).as_bytes());
    blob.update(&body);
    let git_blob_sha1 = format!("{:x}", blob.finalize());
    let url = "https://raw.githubusercontent.com/suttacentral/bilara-data/d6d54741b7f2ddfeca82f02c3f95eb3990b4e351/root/pli/ms/fixture_root-pli-ms.json";
    let entry = json!({
        "upstream_path":format!("root/pli/ms/{basename}"),
        "basename":basename,
        "byte_size":body.len(),
        "git_blob_sha1":git_blob_sha1,
        "media_type":"application/json",
        "url":url
    });
    let relative = format!("{item_root}/payload/{basename}");
    fs::write(fixture.root.join(".gitignore"), format!("/{relative}\n")).unwrap();
    assert!(
        Command::new("git")
            .args(["init", "-q"])
            .current_dir(&fixture.root)
            .status()
            .unwrap()
            .success()
    );

    let expected_body = body.clone();
    let expected_blob = entry["git_blob_sha1"].as_str().unwrap().to_owned();
    TEST_FETCH_CALLBACK.with(|callback| {
        *callback.borrow_mut() = Some(Box::new(move |request| {
            assert_eq!(request["url"], url);
            assert_eq!(request["target_slug"], "fixture-sutta");
            assert_eq!(request["expected_byte_size"], expected_body.len());
            assert_eq!(
                request["expected_git_blob_sha1"].as_str(),
                Some(expected_blob.as_str())
            );
            Ok(expected_body.clone())
        }));
    });

    let log = fixture
        .root
        .join("ToS/source-witnesses/discovery/fixture/acquisition-transfers.jsonl");
    let (retained, receipt) = transfer(
        &fixture.root,
        &target,
        &entry,
        &log,
        Some(&fixture.payload_root),
        true,
    )
    .unwrap();
    TEST_FETCH_CALLBACK.with(|callback| *callback.borrow_mut() = None);

    assert_eq!(retained, body);
    assert_eq!(receipt["status"], "completed");
    assert_eq!(receipt["http_status"], 200);
    assert_eq!(receipt["final_url"], url);
    assert_eq!(receipt["sha256"], sha256(&retained));
    assert_eq!(receipt["expected_git_blob_sha1"], entry["git_blob_sha1"]);
    let payload = fixture
        .payload_root
        .join(item_root.strip_prefix("ToS/source-witnesses/").unwrap())
        .join("payload")
        .join(basename);
    assert_eq!(fs::read(&payload).unwrap(), retained);
    assert_eq!(
        fs::metadata(&payload).unwrap().permissions().mode() & 0o777,
        0o444
    );
    let rows = fs::read_to_string(log)
        .unwrap()
        .lines()
        .map(|line| strict_json(line.as_bytes()).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["status"], "started");
    assert_eq!(rows[1], receipt);
    assert!(!fixture.root.join(&relative).exists());
    assert!(
        !fixture
            .root
            .join(format!("{item_root}/item.manifest.json"))
            .exists()
    );
}

fn inspected(target: &Value, basename: &str, body: &str) -> Result<Value> {
    inspect_payloads(
        target,
        &[(json!({"basename":basename}), body.as_bytes().to_vec())],
    )
}

#[test]
fn reviewed_tei_profiles_bind_language_role_identity_and_exact_opening() {
    let prefix = r#"<TEI xmlns="http://www.tei-c.org/ns/1.0"><teiHeader><fileDesc/></teiHeader>"#;
    for (language, role, kind, xml_language, xml_role, urn, words) in [
        (
            "en",
            "translation",
            "perseus-tei-translation",
            "eng",
            "translation",
            "urn:cts:greekLit:test.eng1",
            "translated words ",
        ),
        (
            "la",
            "source_language",
            "perseus-tei-latin-work",
            "lat",
            "edition",
            "urn:cts:latinLit:fixture.lat1",
            "ratio et natura ",
        ),
        (
            "grc",
            "source_language",
            "perseus-tei-work",
            "grc",
            "edition",
            "urn:cts:greekLit:test.grc1",
            "ἀ",
        ),
    ] {
        let body = format!(
            r#"{prefix}<text><body xml:base="{urn}"><div type="{xml_role}" xml:lang="{xml_language}"><div type="textpart" subtype="section" n="1">{}</div></div></body></text></TEI>"#,
            words.repeat(1100)
        );
        let mut target = json!({"slug":"profile-fixture","language":language,"expression_role":role,
            "coverage":{"kind":kind,"citation_scope":"hierarchical_divisions","cts_urn":urn,
            "identity_anchor":"body_xml_base","header_prefix_sha256":sha256(prefix.as_bytes())}});
        let report = inspected(&target, "source.xml", &body).unwrap();
        assert_eq!(report["source_bytes_changed"], false);
        for (before, after) in [
            (
                format!("xml:lang=\"{xml_language}\""),
                "xml:lang=\"wrong\"".into(),
            ),
            (urn.to_owned(), "urn:cts:other:wrong1".into()),
            (format!("type=\"{xml_role}\""), "type=\"wrong\"".into()),
        ] {
            assert!(inspected(&target, "source.xml", &body.replace(&before, &after)).is_err());
        }
        target["coverage"]["reviewed_body_prefix_bytes"] = json!(prefix.len() + 100);
        target["coverage"]["reviewed_body_prefix_sha256"] =
            json!(sha256(&body.as_bytes()[..prefix.len() + 100]));
        inspected(&target, "source.xml", &body).unwrap();
        target["coverage"]["reviewed_body_prefix_sha256"] = json!("0".repeat(64));
        assert!(inspected(&target, "source.xml", &body).is_err());
    }
}

#[test]
fn tei_repeated_markers_and_unnumbered_scope_preserve_source_addresses() {
    let repeated = r#"<div xmlns="http://www.tei-c.org/ns/1.0"><div type="textpart" subtype="book" n="1">First</div><div type="textpart" subtype="book" n="1">Third</div></div>"#;
    let root = parse_xml(repeated.as_bytes()).unwrap();
    assert!(tei_division_addresses(&root, None, false, false).is_err());
    let (addresses, _, repeats) = tei_division_addresses(&root, None, false, true).unwrap();
    assert_eq!(addresses.iter().collect::<BTreeSet<_>>().len(), 2);
    assert_eq!(
        repeats,
        vec![json!({"source_address":[["book","1"]],"occurrences":2})]
    );
    let nested = r#"<div xmlns="http://www.tei-c.org/ns/1.0"><div type="textpart" subtype="fragments"><div type="textpart" subtype="section" n="1">fragment</div></div><div type="textpart" subtype="speech"><div type="textpart" subtype="section" n="1">speech</div></div></div>"#;
    let root = parse_xml(nested.as_bytes()).unwrap();
    let (addresses, _, _) = tei_division_addresses(&root, None, false, false).unwrap();
    let addresses = addresses
        .iter()
        .map(|p| serde_json::from_str::<Value>(p).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        json!(addresses),
        json!([
            [["fragments", null]],
            [["fragments", null], ["section", "1"]],
            [["speech", null]],
            [["speech", null], ["section", "1"]]
        ])
    );
    for (a, b) in [
        ("subtype=\"speech\"", "subtype=\"fragments\""),
        (" n=\"1\"", ""),
        ("subtype=\"speech\"", "subtype=\"speech\" n=\"\""),
        ("subtype=\"speech\"", "subtype=\"speech\" n=\" \""),
    ] {
        assert!(
            tei_division_addresses(
                &parse_xml(nested.replace(a, b).as_bytes()).unwrap(),
                None,
                false,
                false
            )
            .is_err()
        );
    }
    let prefix = r#"<TEI xmlns="http://www.tei-c.org/ns/1.0"><teiHeader><fileDesc/></teiHeader>"#;
    let body = format!(
        r#"{prefix}<text><body xml:base="urn:cts:latinLit:fixture.eng1"><div type="translation" xml:lang="eng"><div type="textpart" subtype="book" n="1"><p><milestone unit="section" n="1"/>{}</p></div><div type="textpart" subtype="book" n="2"><p><milestone unit="section" n="1"/>text</p></div></div></body></text></TEI>"#,
        "English translation ".repeat(100)
    );
    let target = json!({"slug":"milestones","language":"en","expression_role":"translation","coverage":{"kind":"perseus-tei-translation","cts_urn":"urn:cts:latinLit:fixture.eng1","identity_anchor":"body_xml_base","citation_scope":"hierarchical_divisions_and_section_milestones","header_prefix_sha256":sha256(prefix.as_bytes())}});
    let original = inspected(&target, "source.xml", &body).unwrap();
    assert_eq!(original["files"][0]["division_count"], 4);
    let repeated = body.replacen("<p>", r#"<p><milestone unit="section" n="1"/>"#, 1);
    let observed = inspected(&target, "source.xml", &repeated).unwrap();
    assert_eq!(observed["files"][0]["division_count"], 5);
    assert_eq!(
        observed["files"][0]["repeated_section_markers"],
        json!([{"source_address":[["book","1"],["division_occurrence","1"],["section","1"]],"occurrences":2}])
    );
    assert_ne!(
        observed["files"][0]["division_addresses_sha256"],
        original["files"][0]["division_addresses_sha256"]
    );
    assert!(
        inspected(
            &target,
            "source.xml",
            &body.replace("unit=\"section\" n=\"1\"", "unit=\"section\"")
        )
        .is_err()
    );
}

#[test]
fn bilara_and_osis_profiles_preserve_reviewed_units_and_language_boundaries() {
    for (kind, suffix, language, role) in [
        ("bilara-root", "root-pli-ms", "pli", "source_language"),
        (
            "bilara-translation",
            "translation-en-sujato",
            "en",
            "translation",
        ),
    ] {
        let body = r#"{"dn1:0.1":"Heading", "dn1:1":"Text"}"#;
        let basename = format!("dn1_{suffix}.json");
        let target = json!({"slug":"dn1","language":language,"expression_role":role,"coverage":{"kind":kind,"uid":"dn1","file_count":1,"reviewed_body_prefix_bytes":20,"reviewed_body_prefix_sha256":sha256(&body.as_bytes()[..20])}});
        let report = inspected(&target, &basename, body).unwrap();
        assert_eq!(report["textual_acceptance"], false);
        assert!(inspected(&target, &basename, &body.replace("Heading", "Changed")).is_err());
        for length in [Value::Null, json!(0), json!(true), json!(body.len() + 1)] {
            let mut changed = target.clone();
            changed["coverage"]["reviewed_body_prefix_bytes"] = length;
            assert!(inspected(&changed, &basename, body).is_err());
        }
        let mut changed = target.clone();
        changed["coverage"]
            .as_object_mut()
            .unwrap()
            .remove("reviewed_body_prefix_sha256");
        assert!(inspected(&changed, &basename, body).is_err());
    }
    let target = json!({"slug":"dn1","language":"en","expression_role":"translation","coverage":{"kind":"bilara-translation","uid":"dn1","file_count":1}});
    for body in [
        r#"{"dn10:1":"Other"}"#,
        r#"{"dn1":"Missing"}"#,
        r#"{"dn1:":"Empty"}"#,
        r#"{"dn1:1":"A","dn1:1":"B"}"#,
        r#"{"dn1:1":1}"#,
        r#"{"dn1:1":"  "}"#,
        "{}",
        "[]",
    ] {
        assert!(inspected(&target, "dn1_translation-en-sujato.json", body).is_err());
    }
    for basename in [
        "dn1_root-pli-ms.json",
        "dn1_translation-en-bodhi.json",
        "dn1_translation-de-sujato.json",
    ] {
        assert!(inspected(&target, basename, r#"{"dn1:1":"Text"}"#).is_err());
    }
    let target =
        json!({"slug":"osis","coverage":{"kind":"osis-book","book":"Prov","chapter_count":1}});
    let body = r#"<osis xmlns="http://www.bibletechnologies.net/2003/OSIS/namespace"><chapter osisID="Prov.1"><verse osisID="Prov.1.1"><w>אַ</w></verse></chapter></osis>"#;
    let report = inspected(&target, "Prov.xml", body).unwrap();
    assert_eq!(report["source_bytes_changed"], false);
    assert_eq!(report["files"][0]["chapter_ids"], json!(["Prov.1"]));
    assert_eq!(report["files"][0]["word_count"], 1);
    for altered in [
        body.replace("Prov.1.1", "Job.1.1"),
        body.replace("Prov.1\"", "Prov.2\""),
    ] {
        assert!(inspected(&target, "Prov.xml", &altered).is_err());
    }
}

#[test]
fn egyptian_and_german_components_remain_separate_and_text_free() {
    let target = json!({"slug":"egyptian-example","coverage":{"kind":"oraec-composition"},"ids":{"expression":"tos.expression.example.egy","translation_expression":"tos.expression.example.de"}});
    let mut source = json!({"sentences":[{"translation":"German translation","words":[{"written_form":"Egyptian form"}]}]});
    let report = inspected(&target, "source.json", &source.to_string()).unwrap();
    let c = &report["component_witnesses"];
    assert_eq!(c[0]["language"], "egy");
    assert_eq!(c[1]["language"], "de");
    assert_eq!(c[0]["file_id"], c[1]["file_id"]);
    assert_eq!(
        c[0]["selectors"][0]["json_pointer_pattern"],
        "/sentences/*/words/*/written_form"
    );
    assert!(!report.to_string().contains("German translation"));
    source["sentences"][0]["words"][0]["cotext_translation"] = json!("German lexical gloss");
    let report = inspected(&target, "source.json", &source.to_string()).unwrap();
    assert_eq!(
        report["component_witnesses"][1]["selectors"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(!report.to_string().contains("German lexical gloss"));
    source["sentences"][0]
        .as_object_mut()
        .unwrap()
        .remove("translation");
    source["sentences"][0]["words"][0]
        .as_object_mut()
        .unwrap()
        .remove("cotext_translation");
    assert!(inspected(&target, "source.json", &source.to_string()).is_err());
}

#[test]
fn exact_registry_bytes_paths_and_metadata_time_refuse_ambiguous_input() {
    let body = b"exact\r\n";
    let entry = json!({"basename":"source.txt","byte_size":body.len(),"git_blob_sha1":format!("{:x}",Sha1::digest(b"blob 7\0exact\r\n"))});
    assert_eq!(check_file(body, &entry).unwrap(), sha256(body));
    for altered in [b"exact\n".as_slice(), b"other\r\n"] {
        assert!(check_file(altered, &entry).is_err());
    }
    for raw in [
        r#"{"x":1,"x":2}"#,
        r#"{"x":NaN}"#,
        r#"{"x":Infinity}"#,
        r#"{"x":-Infinity}"#,
    ] {
        assert!(strict_json(raw.as_bytes()).is_err());
    }
    assert_eq!(
        strict_json("{\"x\":\"é\"}".as_bytes()).unwrap(),
        json!({"x":"é"})
    );
    let root = tempfile::tempdir().unwrap();
    for reference in [
        "../outside",
        "/outside",
        "a/../outside",
        "a\\outside",
        "a//b",
        "a/./b",
        "a\0b",
    ] {
        assert!(path_ref(root.path(), reference).is_err());
    }
    std::os::unix::fs::symlink(root.path().parent().unwrap(), root.path().join("link")).unwrap();
    assert!(path_ref(root.path(), "link/outside").is_err());
    let mut observation =
        json!({"started_at":"2026-09-09T07:00:00+00:00","ended_at":"2026-09-09T07:00:02.5+00:00"});
    assert_eq!(metadata_elapsed_seconds(&observation).unwrap(), 2.5);
    observation["elapsed_seconds"] = json!(1.2);
    assert_eq!(metadata_elapsed_seconds(&observation).unwrap(), 1.2);
    for value in [json!(-1), json!(true), json!("unknown"), Value::Null] {
        observation["elapsed_seconds"] = value;
        assert!(metadata_elapsed_seconds(&observation).is_err());
    }
    observation
        .as_object_mut()
        .unwrap()
        .remove("elapsed_seconds");
    observation["ended_at"] = json!("2026-09-09T06:59:59+00:00");
    assert!(metadata_elapsed_seconds(&observation).is_err());
}

#[test]
fn claim_and_discovery_identity_conflicts_refuse_before_item_reuse_or_writes() {
    for defect in [
        "claim",
        "claim-other-owner",
        "discovery",
        "discovery-item",
        "discovery-other-path",
    ] {
        for installed in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path();
            let claim_ref =
                format!("{SOURCE}/relations/work-expression/work-expression-claims.jsonl");
            let claim = json!({"claim_id":"tos.claim.shared-title","subject_ref":"tos.work.plutarch.de-fato"});
            let target = json!({"slug":"de-fato","operation_date":"2026-09-09","ids":{"work":"tos.work.plutarch.de-fato","item":"tos.item.plutarch.de-fato"},"paths":{"item_root":"item"}});
            let package = json!({"claims":[{"path":claim_ref,"record":claim}]});
            if defect.starts_with("claim") {
                let owner = if defect == "claim" {
                    claim_ref.clone()
                } else {
                    format!("{SOURCE}/relations/other/source-claims.jsonl")
                };
                let mut old = claim;
                old["subject_ref"] = json!("tos.work.cicero.de-fato");
                fs::create_dir_all(root.join(&owner).parent().unwrap()).unwrap();
                append_jsonl(&root.join(owner), &old).unwrap();
            } else {
                let name = if defect == "discovery-other-path" {
                    "other.json"
                } else {
                    "registry-de-fato.2026-09-09.v1.json"
                };
                let mut ids = json!(["tos.work.plutarch.de-fato", "tos.item.plutarch.de-fato"]);
                if defect == "discovery" {
                    ids[0] = json!("tos.work.other");
                }
                if defect == "discovery-item" {
                    ids[1] = json!("tos.item.other");
                }
                let path = root.join(SOURCE).join("discovery/runs").join(name);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(path,json_bytes(&json!({"discovery_id":"tos.discovery.registry-de-fato.2026-09-09.v1","target":{"known_tos_refs":ids}})).unwrap()).unwrap();
            }
            if installed {
                fs::create_dir(root.join("item")).unwrap();
                fs::write(root.join("item/item.json"), b"{}").unwrap();
            }
            let before = tree_snapshot(root);
            let error = install_target(
                root,
                &root.join("manifest.json"),
                &json!({}),
                &target,
                &package,
                None,
                false,
            )
            .unwrap_err();
            assert!(
                error.contains("existing bibliographic claim")
                    || error.contains("discovery run identity collision"),
                "{error}"
            );
            assert_eq!(tree_snapshot(root), before);
        }
    }
}
