use flate2::{Compression, write::GzEncoder};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};
use tos_foundation::Digest256;

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn put(root: &Path, path: &Path, raw: &[u8]) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, raw).unwrap();
}
fn encode(value: &Value) -> Vec<u8> {
    serde_json::to_vec_pretty(value).unwrap()
}
fn digest(raw: &[u8]) -> String {
    Digest256::of_bytes(raw).to_hex()
}

#[test]
fn numbered_check_reconstructs_pages_and_refuses_changed_payload() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let rel = Path::new(
        "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/de-naumann-1886/editions/leipzig-c-g-naumann-1886/items/internet-archive-google-harvard-scan-pdf/edition-curation.v1.json",
    );
    let item = rel.parent().unwrap();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "tos-numbered-control-{}-{nonce}",
        std::process::id()
    )));
    let root = fixture.0.join("source");
    let input = fixture.0.join("input");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&input).unwrap();
    let mut curation: Value = serde_json::from_slice(&fs::read(repo.join(rel)).unwrap()).unwrap();
    let mut recipe = curation["recipes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["stage"] == "numbered-structure")
        .unwrap()
        .clone();
    recipe["unit_sequence"]["first_number"] = json!(1);
    recipe["unit_sequence"]["last_number"] = json!(3);
    recipe["unit_sequence"]["repeat_after"] = json!([]);
    recipe["unit_sequence"]["expected_count"] = json!(3);
    for key in [
        "page_overrides",
        "gap_review_keys",
        "ocr_disambiguation_keys",
        "repeated_number_review_keys",
        "required_ocr_keys",
        "ocr_evidence",
        "source_map_required_keys",
    ] {
        recipe[key] = json!([]);
    }
    for (key, n) in [
        ("source_page_count", 3),
        ("first_page", 1),
        ("last_page", 3),
    ] {
        recipe["candidate_profile"][key] = json!(n);
    }
    let manifest_path = item.join(curation["manifest"].as_str().unwrap());
    let mut manifest: Value =
        serde_json::from_slice(&fs::read(repo.join(&manifest_path)).unwrap()).unwrap();
    let mut xml = String::from(
        "<document xmlns=\"http://www.abbyy.com/FineReader_xml/FineReader6-schema-v1.xml\">",
    );
    for n in 1..=3 {
        xml.push_str(&format!(
            "<page><par><line t=\"500\"><charParams>{n}</charParams></line></par></page>"
        ));
    }
    xml.push_str("</document>");
    let mut gzip = GzEncoder::new(Vec::new(), Compression::default());
    gzip.write_all(xml.as_bytes()).unwrap();
    let payloads = [
        b"%PDF-synthetic".to_vec(),
        b"<synthetic/>".to_vec(),
        gzip.finish().unwrap(),
    ];
    let mut inventory = Vec::new();
    let mut first_payload = None;
    for ((entry, role), bytes) in manifest["payload_files"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .zip(recipe["payload_roles"].as_array().unwrap())
        .zip(payloads)
    {
        let hash = digest(&bytes);
        entry["file_id"] = json!(format!("tos.file.sha256.{hash}"));
        entry["sha256"] = json!(hash);
        entry["byte_size"] = json!(bytes.len());
        let path = item.join(entry["relative_path"].as_str().unwrap());
        put(&input, &path, &bytes);
        if first_payload.is_none() {
            first_payload = Some((path, bytes));
        }
        inventory.push(json!({"profile":role["inventory_profile"],"file_id":entry["file_id"],"file_sha256":entry["sha256"],"summary":{"page_count":3}}));
    }
    put(&root, &manifest_path, &encode(&manifest));
    put(
        &root,
        Path::new(manifest["resource_inventory_ref"].as_str().unwrap()),
        &encode(&json!({"files":inventory})),
    );
    put(
        &root,
        Path::new(manifest["rights_ref"].as_str().unwrap()),
        b"{}\n",
    );
    let edition = item
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("edition.json");
    put(&root, &edition, &fs::read(repo.join(&edition)).unwrap());
    let schema = Path::new("ToS/contracts/edition-curation.v1.schema.json");
    put(&root, schema, &fs::read(repo.join(schema)).unwrap());
    let mut map = json!({"map_id":recipe["map_id"],"work_ref":recipe["work_ref"],"expression_ref":recipe["expression_ref"],"edition_ref":recipe["edition_ref"],"item_ref":curation["owner_item_ref"],"provenance_event_ref":recipe["event_id"],"unit_starts":[]});
    for n in 1..=3 {
        map["unit_starts"].as_array_mut().unwrap().push(json!({"sequence":n,"unit_key":n.to_string(),"pdf_page":n,"anchor_ref":format!("{}{n}{}",recipe["identity"]["anchor_prefix"].as_str().unwrap(),recipe["identity"]["anchor_suffix"].as_str().unwrap()),"basis":recipe["default_basis"]}));
    }
    for output in recipe["frozen_outputs"].as_array_mut().unwrap() {
        let bytes = if output["role"] == "map" {
            map["schema_version"] = output["schema_version"].clone();
            encode(&map)
        } else {
            b"{}\n".to_vec()
        };
        output["expected_sha256"] = json!(digest(&bytes));
        put(
            &root,
            Path::new(output["repository_path"].as_str().unwrap()),
            &bytes,
        );
    }
    curation["recipes"] = json!([recipe]);
    put(&root, rel, &encode(&curation));
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_tos-research"))
            .args([
                "check",
                "--source-root",
                root.to_str().unwrap(),
                "--local-input-root",
                input.to_str().unwrap(),
                "--curation",
                rel.to_str().unwrap(),
                "--stage",
                "numbered-structure",
            ])
            .output()
            .unwrap()
    };
    let result = run();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let receipt: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(
        receipt["status"],
        "candidate-pages-and-frozen-fixity-verified"
    );
    assert_eq!(receipt["unit_count"], 3);
    let (path, mut bytes) = first_payload.unwrap();
    bytes[0] ^= 1;
    put(&input, &path, &bytes);
    let refusal = run();
    assert_eq!(refusal.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&refusal.stderr).contains("payload fixity or identity differs")
    );
}
