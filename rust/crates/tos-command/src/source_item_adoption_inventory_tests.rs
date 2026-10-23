//! Local exact-File inventory integration. Synthetic bytes, real owner/worker,
//! committed source consumer and replay; no format-derived rights authority.
use super::*;
use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

const PRIVATE_TEXT: &str = "SYNTHETIC_PRIVATE_SOURCE_TEXT";
const TEI: &[u8] = br#"<TEI xmlns="http://www.tei-c.org/ns/1.0"><text><body><div><p>SYNTHETIC_PRIVATE_SOURCE_TEXT</p></div></body></text></TEI>"#;

// A owns the prepared publication/consumer seam; the local owner fixture is shared.
#[path = "source_item_prepared_consumer_tests.rs"]
mod prepared_consumer;

/// A transferred test image must read an explicitly selected, exact source
/// checkout instead of the builder's baked-in CARGO_MANIFEST_DIR. This selects
/// fixture/software inputs only; the real owner, capture and currentness fences
/// below still execute unchanged.
pub(super) fn repository() -> PathBuf {
    let selected = std::env::var_os("TOS_ITEM_ADOPTION_TEST_REPOSITORY");
    let expected_tree = std::env::var_os("TOS_ITEM_ADOPTION_TEST_TREE");
    let (Some(selected), Some(expected_tree)) = (selected, expected_tree) else {
        assert!(
            std::env::var_os("TOS_ITEM_ADOPTION_TEST_REPOSITORY").is_none()
                && std::env::var_os("TOS_ITEM_ADOPTION_TEST_TREE").is_none(),
            "selected Item test repository and exact tree must be supplied together"
        );
        return support::repository();
    };
    let selected = PathBuf::from(selected);
    assert!(
        selected.is_absolute(),
        "Item test repository must be absolute"
    );
    let repository = selected
        .canonicalize()
        .expect("selected Item test repository");
    let expected_tree = expected_tree.into_string().expect("Item test tree UTF8");
    assert!(
        expected_tree.len() == 40 && expected_tree.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "Item test repository requires an exact Git tree"
    );
    let actual = std::process::Command::new("git")
        .arg("--no-optional-locks")
        .arg("-C")
        .arg(&repository)
        .args(["rev-parse", "HEAD^{tree}"])
        .output()
        .expect("read selected Item test tree");
    assert!(actual.status.success(), "selected Item test Git tree");
    assert_eq!(
        String::from_utf8(actual.stdout).unwrap().trim(),
        expected_tree,
        "selected Item test source tree differs"
    );
    let status = std::process::Command::new("git")
        .arg("--no-optional-locks")
        .arg("-C")
        .arg(&repository)
        .args(["status", "--porcelain=v1"])
        .output()
        .expect("read selected Item test source status");
    assert!(
        status.status.success() && status.stdout.is_empty(),
        "selected Item test source checkout must be clean"
    );
    repository
}

pub(super) fn select_payload(fixture: &mut Value, name: &str, media: &str, raw: &[u8]) {
    let input = PathBuf::from(fixture["input"].as_str().unwrap());
    fs::write(&input, raw).unwrap();
    fs::set_permissions(&input, fs::Permissions::from_mode(0o600)).unwrap();
    let owner = PathBuf::from(fixture["owner"].as_str().unwrap());
    let config = &mut fixture["config"];
    config["payload_basename"] = json!(name);
    config["original_basename"] = json!(name);
    config["media_type"] = json!(media);
    config["byte_size"] = json!(raw.len());
    config["sha256"] = json!(Digest256::of_bytes(raw).to_hex());
    fs::write(owner, serde_json::to_vec(config).unwrap()).unwrap();
}

struct Fixture {
    filesystem: CreationFilesystem,
    context: CommandContext,
    cut: CorpusCutReader,
    software: SoftwareCaptureReader,
    components: SoftwareComponentSelectionV1,
    config: Value,
    input: PathBuf,
    raw: Vec<u8>,
    deadline: Instant,
    cancelled: AtomicBool,
    _temporary: tempfile::TempDir,
}
impl Fixture {
    fn new(name: &str, media: &str, raw: &[u8]) -> Self {
        let repository = repository();
        let deadline = Instant::now() + Duration::from_secs(900);
        let cancelled = AtomicBool::new(false);
        let temporary = tempfile::tempdir().unwrap();
        let isolated =
            IsolatedCreationRoot::create(temporary.path(), deadline, &cancelled).unwrap();
        let root = isolated.path();
        let mut fixture =
            crate::source_creation_store::source_native_test_fixtures::item_orphan_recovery(
                &repository,
                root,
            );
        select_payload(&mut fixture, name, media, raw);
        let owner_path = Path::new(fixture["owner"].as_str().unwrap());
        let authored = support::authored(root);
        let mut files = authored.clone();
        for name in IMPLEMENTATIONS {
            let body = fs::read(repository.join(name)).unwrap();
            let target = root.join(name);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(&target, &body).unwrap();
            assert!(files.insert((*name).to_owned(), body).is_none());
        }
        let (software, components) =
            support::software(&repository, &files, temporary.path(), deadline, &cancelled);
        let (base_revision, cut) = support::cut(
            &authored,
            &temporary.path().join("original-cut"),
            deadline,
            &cancelled,
        );
        let context = CommandContext {
            base_revision,
            configuration_raw: fs::read(owner_path).unwrap(),
            request_raw: serde_json::to_vec(&fixture["proposal"]).unwrap(),
            recorded_at: "2026-09-09T12:00:00+00:00".to_owned(),
            effective_uid: u64::from(fs::metadata(root).unwrap().uid()),
            files: files
                .into_iter()
                .map(|(path, raw)| SourceFile {
                    path: RelativePath::parse(&path).unwrap(),
                    raw,
                })
                .collect(),
        };
        let filesystem =
            CreationFilesystem::select_isolated(&isolated, owner_path, deadline, &cancelled)
                .unwrap();
        Self {
            filesystem,
            context,
            cut,
            software,
            components,
            config: fixture["config"].clone(),
            input: PathBuf::from(fixture["input"].as_str().unwrap()),
            raw: raw.to_vec(),
            deadline,
            cancelled,
            _temporary: temporary,
        }
    }
    fn limits(&self) -> ItemLimits {
        ItemLimits {
            max_member_bytes: 2_097_152,
            max_total_bytes: 64_000_000,
            max_state_bytes: 16_777_216,
            max_issues: 256,
            deadline: self.deadline,
        }
    }
    fn prepare(&self) -> ItemAdoptionPreparation {
        let mut worker = support::worker(&self.cut, self.deadline, &self.cancelled);
        prepare_isolated_item_adoption_from_proposal(
            &self.filesystem,
            &self.context,
            &self.cut,
            &self.software,
            &self.components,
            &mut worker,
            self.limits(),
            &self.cancelled,
        )
        .unwrap()
    }
    fn execute(&self, request: &Value) -> SourceCommandResult<ItemAdoptionPublication> {
        let context = context_with_request(&self.context, serde_json::to_vec(request).unwrap());
        let mut worker = support::worker(&self.cut, self.deadline, &self.cancelled);
        execute_isolated_item_adoption_from_captures(
            &self.filesystem,
            &context,
            &self.cut,
            &self.software,
            &self.components,
            &mut worker,
            self.limits(),
            &self.cancelled,
        )
    }
    fn root(&self) -> &Path {
        &self.filesystem.root_path
    }
    fn item(&self) -> PathBuf {
        self.root()
            .join(self.config["item_source_path"].as_str().unwrap())
    }
    fn publication(&self) -> Vec<u8> {
        fs::read(
            self.root()
                .join("ToS/source-witnesses/.metadata-publication.json"),
        )
        .unwrap()
    }
}

fn prepared_request(prepared: &ItemAdoptionPreparation) -> Value {
    let mut request: Value =
        serde_json::from_slice(&cmd::canonical(prepared.request()).unwrap()).unwrap();
    request["command_id"] = json!("native-local-inventory-bridge");
    request
}

fn successful_profile(name: &str, media: &str, raw: &[u8], profile: &str) {
    let f = Fixture::new(name, media, raw);
    let before = f.publication();
    let prepared = f.prepare();
    let request = prepared_request(&prepared);
    let inventory = &request["inventory"];
    assert_eq!(
        inventory["profile"], profile,
        "supported profile must not silently take byte-only fallback: {name}"
    );
    assert!(request["inventory_limitation"].is_null());
    assert_eq!(inventory["file_id"], f.config["file_id"]);
    assert_eq!(inventory["file_sha256"], f.config["sha256"]);
    assert_eq!(inventory["media_type"], media);
    assert!(
        !serde_json::to_string(inventory)
            .unwrap()
            .contains(PRIVATE_TEXT)
    );
    assert_eq!(f.publication(), before, "preview must not publish");
    assert!(!f.item().exists());
    let committed = f.execute(&request).unwrap();
    assert!(committed.metadata_committed() && !committed.replayed());
    assert_eq!(
        committed.materializations(),
        Some(cmd::field(prepared.projected_result(), "prepared_materializations").unwrap())
    );
    assert_eq!(fs::read(&f.input).unwrap(), f.raw);
    let target = deposit::destination(&f.config).unwrap();
    assert_eq!(fs::read(target).unwrap(), f.raw);
    let home = f.item().parent().unwrap().to_owned();
    let rights: Value =
        serde_json::from_slice(&fs::read(home.join("rights.json")).unwrap()).unwrap();
    assert_eq!(
        rights, request["rights"],
        "inventory must not infer or rewrite rights"
    );
    assert_eq!(rights["visibility"], "local_only");
    assert_eq!(rights["redistribution_posture"], "not_authorized");
    let manifest: Value =
        serde_json::from_slice(&fs::read(home.join("item.manifest.json")).unwrap()).unwrap();
    assert_eq!(
        manifest["payload_files"][0],
        deposit::payload_entry(&f.config).unwrap()
    );
    let stored: Value =
        serde_json::from_slice(&fs::read(home.join("resource-inventory.json")).unwrap()).unwrap();
    assert_eq!(&stored["files"][0], inventory);
    assert_eq!(stored["source_text_included"], false);
    assert_eq!(stored["inventory_authority"], "mechanical_metadata_only");
    let provenance = fs::read_to_string(home.join("provenance.jsonl")).unwrap();
    assert!(provenance.contains(f.config["acquisition_event_id"].as_str().unwrap()));
    assert!(provenance.contains(f.config["inventory_event_id"].as_str().unwrap()));
    let claims: Value =
        serde_json::from_slice(&fs::read(home.join("source-claims.jsonl")).unwrap()).unwrap();
    let current = support::authored(f.root());
    let (_, current_cut) = support::cut(
        &current,
        &f._temporary.path().join("current-cut"),
        f.deadline,
        &f.cancelled,
    );
    let mut consumer_worker = support::worker(&current_cut, f.deadline, &f.cancelled);
    let observed = tos_validation::native_compound::verify_edition_item_from_cut(
        &current_cut,
        &mut consumer_worker,
        &format!(
            "{}/source-claims.jsonl",
            f.config["item_source_path"]
                .as_str()
                .unwrap()
                .rsplit_once('/')
                .unwrap()
                .0
        ),
        &claims,
        f.limits(),
        &f.cancelled,
    )
    .unwrap();
    consumer_worker.finish(f.deadline, &f.cancelled).unwrap();
    assert_eq!(
        observed.transport,
        tos_validation::native_compound::NativeTransportState::Committed
    );
    assert_eq!(observed.transaction_id, committed.transaction_id());
    assert_eq!(
        Some(observed.manifest_sha256.as_str()),
        committed.manifest_sha256()
    );
    let terminal = f.publication();
    let context = context_with_request(&f.context, serde_json::to_vec(&request).unwrap());
    let mut replay_worker = support::worker(&current_cut, f.deadline, &f.cancelled);
    let replay = replay_isolated_item_adoption_from_captures(
        &f.filesystem,
        &context,
        &f.cut,
        &current_cut,
        &f.software,
        &f.components,
        &mut replay_worker,
        f.limits(),
        &f.cancelled,
    )
    .unwrap();
    assert!(replay.replayed() && replay.metadata_committed());
    assert_eq!(replay.receipt(), committed.receipt());
    assert_eq!(replay.materializations(), committed.materializations());
    assert_eq!(f.publication(), terminal);
    assert_eq!(fs::read(&f.input).unwrap(), f.raw);
}

#[test]
fn local_inventory_bridge_tei_commit_consumer_and_replay() {
    successful_profile(
        "source.tei.xml",
        "application/tei+xml",
        TEI,
        "tei_structure_v1",
    );
}
#[test]
fn local_inventory_bridge_osis_commit_consumer_and_replay() {
    successful_profile("source.osis.xml", "application/osis+xml", br#"<osis xmlns="http://www.bibletechnologies.net/2003/OSIS/namespace"><osisText osisIDWork="synthetic"><div type="book" osisID="Gen"><chapter osisID="Gen.1"><verse osisID="Gen.1.1">SYNTHETIC_PRIVATE_SOURCE_TEXT</verse></chapter></div></osisText></osis>"#, "osis_structure_v1");
}
#[test]
fn local_inventory_bridge_json_commit_consumer_and_replay() {
    successful_profile(
        "registry.json",
        "application/json",
        br#"{"synthetic":"SYNTHETIC_PRIVATE_SOURCE_TEXT","items":[1,2]}"#,
        "json_members_v1",
    );
}
#[test]
fn local_inventory_bridge_scandata_commit_consumer_and_replay() {
    successful_profile("book_scandata.xml", "application/xml", br#"<book><bookData><leafCount>1</leafCount><dpi>300</dpi></bookData><pageData><page leafNum="0"><origWidth>100</origWidth><origHeight>200</origHeight></page></pageData></book>"#, "scandata_pages_v1");
}
#[test]
fn local_inventory_bridge_djvu_xml_commit_consumer_and_replay() {
    successful_profile("book.djvu.xml", "application/vnd.djvu+xml", br#"<DjVuXML><BODY><OBJECT width="100" height="200"><PARAM name="DPI" value="300"/><PARAGRAPH><LINE><WORD>SYNTHETIC_PRIVATE_SOURCE_TEXT</WORD></LINE></PARAGRAPH></OBJECT></BODY></DjVuXML>"#, "djvu_xml_pages_v1");
}

#[test]
fn local_inventory_bridge_forged_inventory_and_rights_preserve_state() {
    let f = Fixture::new("source.tei.xml", "application/tei+xml", TEI);
    let prepared = f.prepare();
    let request = prepared_request(&prepared);
    assert_eq!(request["inventory"]["profile"], "tei_structure_v1");
    let before = f.publication();
    let mut forged = request.clone();
    forged["rights"]["redistribution_posture"] = json!("authorized");
    assert!(f.execute(&forged).is_err());
    assert_eq!(f.publication(), before);
    assert!(!f.item().exists());
    let mut forged = request;
    forged["inventory"]["file_sha256"] = json!("0".repeat(64));
    assert!(f.execute(&forged).is_err());
    assert_eq!(f.publication(), before);
    assert!(!f.item().exists());
    assert!(!deposit::destination(&f.config).unwrap().exists());
    assert_eq!(fs::read(&f.input).unwrap(), f.raw);
}

#[test]
fn local_inventory_bridge_djvu_commit_consumer_and_replay() {
    let mut info = Vec::from(100u16.to_be_bytes());
    info.extend_from_slice(&200u16.to_be_bytes());
    info.extend_from_slice(&[0, 0]);
    info.extend_from_slice(&300u16.to_le_bytes());
    info.extend_from_slice(&[22, 0]);
    let mut raw = b"AT&TFORM".to_vec();
    raw.extend_from_slice(&(4u32 + 8 + info.len() as u32).to_be_bytes());
    raw.extend_from_slice(b"DJVUINFO");
    raw.extend_from_slice(&(info.len() as u32).to_be_bytes());
    raw.extend_from_slice(&info);
    successful_profile("book.djvu", "image/vnd.djvu", &raw, "djvu_pages_v1");
}

#[test]
fn local_inventory_bridge_abbyy_commit_consumer_and_replay() {
    use std::io::Write;
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(br#"<document xmlns="http://www.abbyy.com/FineReader_xml/FineReader6-schema-v1.xml"><page width="100" height="200" resolution="300"><block><par><line><formatting><charParams wordStart="true">SYNTHETIC_PRIVATE_SOURCE_TEXT</charParams></formatting></line></par></block></page></document>"#).unwrap();
    successful_profile(
        "scan.abbyy.xml.gz",
        "application/gzip",
        &encoder.finish().unwrap(),
        "abbyy_xml_pages_v1",
    );
}

fn stored_zip(name: &str, payload: &[u8]) -> Vec<u8> {
    let mut raw = Vec::new();
    let crc = crc32fast::hash(payload);
    let size = u32::try_from(payload.len()).unwrap();
    let name_len = u16::try_from(name.len()).unwrap();
    raw.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
    for n in [20u16, 0, 0, 0, 0] {
        raw.extend_from_slice(&n.to_le_bytes());
    }
    for n in [crc, size, size] {
        raw.extend_from_slice(&n.to_le_bytes());
    }
    raw.extend_from_slice(&name_len.to_le_bytes());
    raw.extend_from_slice(&0u16.to_le_bytes());
    raw.extend_from_slice(name.as_bytes());
    raw.extend_from_slice(payload);
    let offset = u32::try_from(raw.len()).unwrap();
    raw.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
    for n in [20u16, 20, 0, 0, 0, 0] {
        raw.extend_from_slice(&n.to_le_bytes());
    }
    for n in [crc, size, size] {
        raw.extend_from_slice(&n.to_le_bytes());
    }
    for n in [name_len, 0, 0, 0, 0] {
        raw.extend_from_slice(&n.to_le_bytes());
    }
    raw.extend_from_slice(&0u32.to_le_bytes());
    raw.extend_from_slice(&0u32.to_le_bytes());
    raw.extend_from_slice(name.as_bytes());
    let directory_size = u32::try_from(raw.len()).unwrap() - offset;
    raw.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    for n in [0u16, 0, 1, 1] {
        raw.extend_from_slice(&n.to_le_bytes());
    }
    raw.extend_from_slice(&directory_size.to_le_bytes());
    raw.extend_from_slice(&offset.to_le_bytes());
    raw.extend_from_slice(&0u16.to_le_bytes());
    raw
}
#[test]
fn local_inventory_bridge_jp2_zip_commit_consumer_and_replay() {
    successful_profile(
        "book_jp2.zip",
        "application/zip",
        &stored_zip("book_jp2/book_0000.jp2", b"synthetic-codestream"),
        "jp2_zip_pages_v1",
    );
}

fn one_page_pdf() -> Vec<u8> {
    use std::io::Write;
    let objects: [&[u8]; 4] = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Count 1 /Kids [3 0 R] >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 200] /Resources << >> /Contents 4 0 R >>",
        b"<< /Length 0 >>\nstream\n\nendstream",
    ];
    let mut raw = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(raw.len());
        writeln!(raw, "{} 0 obj", i + 1).unwrap();
        raw.extend_from_slice(body);
        raw.extend_from_slice(b"\nendobj\n");
    }
    let xref = raw.len();
    raw.extend_from_slice(b"xref\n0 5\n0000000000 65535 f \n");
    for offset in offsets {
        writeln!(raw, "{offset:010} 00000 n ").unwrap();
    }
    write!(
        raw,
        "trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n"
    )
    .unwrap();
    raw
}
#[test]
fn local_inventory_bridge_pdf_commit_consumer_and_replay() {
    // Existing bounded provider helpers must really execute, never a fabricated
    // PDF inventory or conditional skip when a selected helper is unavailable.
    successful_profile(
        "book.pdf",
        "application/pdf",
        &one_page_pdf(),
        "pdf_pages_v1",
    );
}
#[test]
fn local_inventory_bridge_utf8_commit_consumer_and_replay() {
    // Plain UTF-8 navigation retains observations/fingerprints, never text.
    successful_profile(
        "plain.txt",
        "text/plain",
        PRIVATE_TEXT.as_bytes(),
        "plain_utf8_file_v1",
    );
}

#[test]
fn local_inventory_bridge_tei_orphan_resume() {
    retained_orphan_case(
        "resume",
        Some(("source.tei.xml", "application/tei+xml", TEI)),
    );
}
#[test]
fn local_inventory_bridge_tei_orphan_rollback() {
    retained_orphan_case(
        "rollback",
        Some(("source.tei.xml", "application/tei+xml", TEI)),
    );
}

#[test]
fn local_inventory_bridge_unsupported_is_bytes_only_not_metadata_success() {
    let raw = b"Synthetic opaque retained bytes.";
    let f = Fixture::new("opaque.bin", "application/octet-stream", raw);
    let before = f.publication();
    let prepared = f.prepare();
    let request = prepared_request(&prepared);
    assert!(request["inventory"].is_null());
    assert!(request["inventory_limitation"].is_string());
    assert!(prepared.projected_outputs().is_empty());
    let committed = f.execute(&request).unwrap();
    assert!(!committed.metadata_committed());
    assert!(committed.receipt().is_none());
    assert!(committed.materializations().is_none());
    assert!(!f.item().exists());
    assert_eq!(f.publication(), before);
    assert_eq!(
        fs::read(deposit::destination(&f.config).unwrap()).unwrap(),
        raw
    );
    assert_eq!(fs::read(&f.input).unwrap(), raw);
}
