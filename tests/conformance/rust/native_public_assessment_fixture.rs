//! Independent, Rust-owned public assessment fixtures for native CLI tests.
//!
//! The fixture contains synthetic assessment actors and source-copy wording;
//! it does not make a linguistic, scholarly, rights, or canon judgment.
use serde_json::{Value, json};
use std::fs;
use std::io;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_foundation::Digest256;

const POLICY_REF: &str = "ToS/doctrine/semantic-interchange/assessment-policy.v1.json";
const WORK_REF: &str =
    "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/work.json";
const FORMS_REF: &str = "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/work.human-forms.json";
const SUBJECT_ID: &str = "tos.work.friedrich-nietzsche.jenseits-von-gut-und-boese";
const NOTICE: &str = "Synthetic fixture only; not a substantive assessment.";
const NOW: &str = "2026-09-05T12:00:00Z";
const START: &str = "2026-09-01T00:00:00Z";
const END: &str = "2099-01-01T00:00:00Z";

/// Files and native-ready request templates used by a public assessment test.
#[derive(Clone, Debug)]
pub struct NativePublicAssessmentFixture {
    pub version: u8,
    pub owner_config_path: PathBuf,
    pub public_root: PathBuf,
    pub owner_context_path: PathBuf,
    pub journal_directory: PathBuf,
    pub owner_config: Value,
    pub owner_context: Value,
    pub subject_ids: Vec<String>,
    pub subject_refs: Vec<Value>,
    pub describe_requests: Vec<Value>,
    pub append_assessments: Vec<Value>,
    pub withdrawal_assessments: Vec<Value>,
    pub source_root: Option<PathBuf>,
    pub source_record: Option<Value>,
    pub source_ref: Option<Value>,
    pub source_bytes: Option<Vec<u8>>,
    pub form_set_path: Option<PathBuf>,
    pub form_set_bytes: Option<Vec<u8>>,
    pub forms: Vec<Value>,
    pub nodes: Vec<Value>,
    pub pending_selection: Option<usize>,
    pub ready_selections: Vec<usize>,
    /// Exact TextUnit record ref after changing only its source-access scope
    /// to metadata-only; v1/v2 have no native TextUnit subject.
    pub metadata_subject: Option<Value>,
    pub private_inventory: Vec<Value>,
    /// Exact regular-file inventory of the public source root at construction.
    pub preserved: Vec<Value>,
}

/// Reusable native source and scoped review inputs for the positive two-flame
/// Sign promotion process scenario. The fixture grants no production authority.
#[derive(Clone, Debug)]
pub struct NativePublicSignFixture {
    pub public_root: PathBuf,
    pub assessment_owner_config_path: PathBuf,
    pub assessment_owner_context_path: PathBuf,
    pub sign_owner_config_path: PathBuf,
    pub candidate_id: String,
    pub occurrence_ids: [String; 2],
    pub source_text: Vec<u8>,
    /// One append-ready `tos_knowledge_assessment_v1` row.
    pub assessment: Value,
    /// Sign source record before the CLI-provided promotion basis is inserted.
    pub sign_record: Value,
    /// Prepare/create form selectors, not authored form records.
    pub forms: Vec<Value>,
    /// Relative path of the source Claim stream, used for exact source revision replay.
    pub source_path: String,
}

/// Construct one public v1 inline, v2 source-backed, or v3 native TextUnit subject.
pub fn native_public_assessment_fixture(
    repository: &Path,
    root: &Path,
    version: u8,
) -> io::Result<NativePublicAssessmentFixture> {
    match version {
        1 => build_inline_v1(repository, root),
        2 => build_public_v2(repository, root, 1),
        3 => build_public_v3(repository, root),
        _ => Err(invalid_input("public assessment fixture supports v1/v2/v3")),
    }
}

/// Construct a source-closed, public-synthetic two-flame motif fixture for the
/// process-level Sign promotion route. The retained source file is hash-pinned;
/// its exact two native text anchors, Occurrence descriptions, Claim, and
/// assessment references are written into an isolated public fixture root.
pub fn native_public_sign_fixture(
    repository: &Path,
    root: &Path,
) -> io::Result<NativePublicSignFixture> {
    const SOURCE_REF: &str = "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/public-synthetic-source.txt";
    const SOURCE_SHA256: &str = "7057e4420a5bf7779f384ec722015e4c2cdb2eaad8fea809d3e536b9d6c7acc3";
    const CANDIDATE: &str = "tos.claim.fixture.flame-continuity-under-change";
    const OCCURRENCES: [&str; 2] = [
        "tos.occurrence.semantic-v2.synthetic-flame-1",
        "tos.occurrence.semantic-v2.synthetic-flame-2",
    ];
    const PACKET_REF: &str = "ToS/source-witnesses/works/synthetic-native-binding/technical-markup/synthetic-binding/source-text-unit.synthetic.v1.json";
    const LAYER_REF: &str = "ToS/source-witnesses/works/synthetic-native-binding/technical-markup/synthetic-binding/source-text-layer.synthetic.v1.json";
    const CONTENT_REF: &str = "ToS/source-witnesses/works/synthetic-native-binding/technical-markup/synthetic-binding/public-synthetic-content.txt";
    const ITEM_REF_PATH: &str = "ToS/source-witnesses/works/synthetic-native-binding/expressions/und-synthetic/editions/synthetic-edition/items/synthetic-item/item.json";
    const ITEM_MANIFEST_REF: &str = "ToS/source-witnesses/works/synthetic-native-binding/expressions/und-synthetic/editions/synthetic-edition/items/synthetic-item/item.manifest.json";
    const ITEM_PAYLOAD_REF: &str = "ToS/source-witnesses/works/synthetic-native-binding/expressions/und-synthetic/editions/synthetic-edition/items/synthetic-item/payload/public-synthetic-source.txt";
    const RIGHTS_REF: &str = "ToS/source-witnesses/works/synthetic-native-binding/expressions/und-synthetic/editions/synthetic-edition/items/synthetic-item/rights.json";
    const SOURCE_ANCHOR_REF: &str = "ToS/source-witnesses/works/synthetic-native-binding/technical-markup/synthetic-binding/source-anchor-v2.synthetic.v1.json";
    const PUBLIC_AUTHORITY_REF: &str = "ToS/source-witnesses/works/synthetic-native-binding/technical-markup/synthetic-binding/synthetic-flame-publication-authority.json";
    const ITEM_ID: &str = "tos.item.synthetic.flame-source";
    const ORIGIN_ANCHOR_ID: &str = "tos.anchor.synthetic.flame-source-origin";
    const LAYER_ID: &str = "tos.text-layer.synthetic.flame-source";
    const PACKET_ID: &str = "tos.source-text-unit-packet.sid-33333333333333333333333333333333";
    const SEGMENTATION_ID: &str = "tos.text-segmentation.sid-44444444444444444444444444444444";
    const UNIT_IDS: [&str; 2] = [
        "tos.text-unit.sid-11111111111111111111111111111111",
        "tos.text-unit.sid-22222222222222222222222222222222",
    ];
    const CLAIM_PATH: &str = "ToS/source-witnesses/relations/flame-continuity/source-claims.jsonl";

    let base = native_public_assessment_fixture(repository, &root.join("base"), 3)?;
    let public_root = base.public_root;
    let source_text = fs::read(repository.join(SOURCE_REF))?;
    let source_text_utf8 = std::str::from_utf8(&source_text).map_err(invalid_data)?;
    if source_text.len() != 109
        || source_text_utf8.chars().count() != 109
        || !source_text.is_ascii()
        || source_text.last() != Some(&b'\n')
        || Digest256::of_bytes(&source_text).to_hex() != SOURCE_SHA256
    {
        return Err(invalid_data(
            "two-flame fixture source pin or shape differs",
        ));
    }
    let source_sha = Digest256::of_bytes(&source_text).to_hex();
    let source_file_id = format!("tos.file.sha256.{source_sha}");
    let content_file_id = source_file_id.clone();
    write_public_bytes(&public_root.join(SOURCE_REF), &source_text)?;
    write_public_bytes(&public_root.join(ITEM_PAYLOAD_REF), &source_text)?;
    write_public_bytes(&public_root.join(CONTENT_REF), &source_text)?;

    // The public native layer and its Item now bind the same exact retained
    // bytes. The input remains a public-synthetic fixture declaration only.
    let mut item: Value = serde_json::from_slice(&fs::read(public_root.join(ITEM_REF_PATH))?)
        .map_err(invalid_data)?;
    item["record_id"] = json!(ITEM_ID);
    item["preferred_label"] = json!("Public-synthetic two-flame source fixture");
    item["source_refs"] = json!([SOURCE_REF, ITEM_MANIFEST_REF]);
    let item_bytes = serde_json::to_vec(&item).map_err(invalid_data)?;
    write_public_bytes(&public_root.join(ITEM_REF_PATH), &item_bytes)?;
    let mut item_manifest: Value =
        serde_json::from_slice(&fs::read(public_root.join(ITEM_MANIFEST_REF))?)
            .map_err(invalid_data)?;
    item_manifest["item_id"] = json!(ITEM_ID);
    item_manifest["visibility"] = json!("public_payload");
    item_manifest["payload_files"] = json!([{
        "file_id":source_file_id,"relative_path":"payload/public-synthetic-source.txt",
        "original_basename":"public-synthetic-source.txt","media_type":"text/plain",
        "byte_size":source_text.len(),"sha256":source_sha,"fixity_verified_at":NOW
    }]);
    let manifest_bytes = serde_json::to_vec(&item_manifest).map_err(invalid_data)?;
    write_public_bytes(&public_root.join(ITEM_MANIFEST_REF), &manifest_bytes)?;
    let rights = json!({"schema_version":"tos_rights_record_v1","rights_id":"tos.rights.fixture.flame-source",
        "scope_refs":[ITEM_ID,format!("tos.file.sha256.{source_sha}"),LAYER_ID],
        "assessment_status":"permission_granted","jurisdictions_reviewed":[],
        "source_refs":[SOURCE_REF],"permissions":["Public-synthetic test fixture use only."],
        "restrictions":["Fixture-local declaration; no external publication permission is asserted."],
        "visibility":"public_payload","redistribution_posture":"authorized_with_conditions",
        "derivative_posture":"allowed_with_conditions",
        "assessed_by":{"maker_type":"imported_source","agent_ref":"source:public-synthetic-fixture"},
        "assessed_at":NOW,"rationale":"Fixture-local public-synthetic source input; this record is not a general rights determination.",
        "review_status":"unreviewed","record_version":1,"supersedes_rights_ref":null});
    let rights_bytes = serde_json::to_vec(&rights).map_err(invalid_data)?;
    let rights_sha = Digest256::of_bytes(&rights_bytes).to_hex();
    write_public_bytes(&public_root.join(RIGHTS_REF), &rights_bytes)?;
    let publication_authority = json!({"schema_version":"tos_fixture_public_synthetic_publication_authority_v1",
        "authority_id":"fixture:public-synthetic-flame-source","source_ref":SOURCE_REF,
        "source_sha256":source_sha,"scope":"isolated conformance fixture only",
        "publication_authorized":true,"private_source_used":false,
        "notice":"This fixture-local flag confers no external publication permission."});
    let publication_authority_bytes =
        serde_json::to_vec(&publication_authority).map_err(invalid_data)?;
    let publication_authority_sha = Digest256::of_bytes(&publication_authority_bytes).to_hex();
    write_public_bytes(
        &public_root.join(PUBLIC_AUTHORITY_REF),
        &publication_authority_bytes,
    )?;
    let mut source_anchor: Value =
        serde_json::from_slice(&fs::read(public_root.join(SOURCE_ANCHOR_REF))?)
            .map_err(invalid_data)?;
    source_anchor["anchor_id"] = json!(ORIGIN_ANCHOR_ID);
    source_anchor["passage_id"] = json!("tos.passage.synthetic.flame-source");
    source_anchor["target"]["item_id"] = json!(ITEM_ID);
    source_anchor["target"]["file_id"] = json!(source_file_id);
    source_anchor["target"]["file_sha256"] = json!(source_sha);
    source_anchor["target"]["media_type"] = json!("text/plain");
    source_anchor["publication_boundary"]["source_content_visibility"] = json!("public");
    source_anchor["publication_boundary"]["public_payload_expected"] = json!(true);
    source_anchor["selector_payload"]["expression"]["selector"]["selector"]["start"] = json!(0);
    source_anchor["selector_payload"]["expression"]["selector"]["selector"]["end"] = json!(109);
    source_anchor["selector_payload"]["expression"]["selector"]["state"]["representation_ref"] =
        json!(ITEM_PAYLOAD_REF);
    source_anchor["selector_payload"]["expression"]["selector"]["state"]["representation_sha256"] =
        json!(source_sha);
    source_anchor["selector_payload"]["expression"]["selector"]["state"]["media_type"] =
        json!("text/plain");
    source_anchor["provenance_event_ref"] = json!("tos.event.fixture.flame-source-binding");
    let source_anchor_bytes = serde_json::to_vec(&source_anchor).map_err(invalid_data)?;
    let source_anchor_sha = Digest256::of_bytes(&source_anchor_bytes).to_hex();
    write_public_bytes(&public_root.join(SOURCE_ANCHOR_REF), &source_anchor_bytes)?;

    let mut layer: Value =
        serde_json::from_slice(&fs::read(public_root.join(LAYER_REF))?).map_err(invalid_data)?;
    layer["layer_id"] = json!(LAYER_ID);
    layer["provenance_event_ref"] = json!("tos.event.fixture.flame-source-binding");
    layer["derivation"]["maker"]["agent_ref"] = json!("software:two-flame-sign-fixture-builder");
    layer["derivation"]["maker"]["method"] = json!(
        "Byte-exact fixture binding of the retained public-synthetic source; no OCR or normalization."
    );
    layer["source_binding"]["item_ref"] = json!(ITEM_ID);
    layer["source_binding"]["source_file_ref"] = json!(source_file_id);
    layer["source_binding"]["source_file_sha256"] = json!(source_sha);
    layer["source_binding"]["anchors"] = json!([{"anchor_id":ORIGIN_ANCHOR_ID,
        "anchor_record_ref":SOURCE_ANCHOR_REF,"anchor_record_sha256":source_anchor_sha}]);
    layer["representation"]["content_visibility"] = json!("public");
    layer["representation"]["publication_authorized"] = json!(true);
    layer["representation"]["storage"] = json!("tracked");
    layer["representation"]["tracked_content"] = json!(true);
    layer["representation"]["publication_authority_refs"] = json!([{"ref":PUBLIC_AUTHORITY_REF,
        "sha256":publication_authority_sha}]);
    layer["representation"]["rights_record_refs"] = json!([{"ref":RIGHTS_REF,"sha256":rights_sha}]);
    let content_sha = source_sha.clone();
    layer["representation"]["content_ref"] = json!(CONTENT_REF);
    layer["representation"]["content_file_id"] = json!(content_file_id);
    layer["representation"]["content_sha256"] = json!(content_sha);
    layer["representation"]["language"] = json!("en");
    layer["representation"]["text_scope"] = json!({
        "start":0,"end":109,"interval":"half_open","position_unit":"unicode_code_point"
    });
    let layer_bytes = serde_json::to_vec(&layer).map_err(invalid_data)?;
    let layer_sha = Digest256::of_bytes(&layer_bytes).to_hex();
    write_public_bytes(&public_root.join(LAYER_REF), &layer_bytes)?;

    let mut packet: Value =
        serde_json::from_slice(&fs::read(public_root.join(PACKET_REF))?).map_err(invalid_data)?;
    // In this packet field the digest names the text bytes, while the native
    // binding below separately names the layer record digest.
    packet["source_layer"]["text_layer_sha256"] = json!(source_sha);
    packet["source_layer"]["language"] = json!("en");
    packet["source_layer"]["visibility"] = json!("public");
    packet["source_layer"]["publication_authorized"] = json!(true);
    packet["packet_id"] = json!(PACKET_ID);
    packet["source_scope"]["item_ref"] = json!(ITEM_ID);
    packet["source_scope"]["file_ref"] = json!(source_file_id);
    packet["source_scope"]["file_sha256"] = json!(source_sha);
    packet["rights_and_visibility"]["effective_visibility"] = json!("public");
    packet["rights_and_visibility"]["packet_visibility"] = json!("public");
    packet["rights_and_visibility"]["private_source_used"] = json!(false);
    packet["rights_and_visibility"]["publication_authorized"] = json!(true);
    packet["rights_and_visibility"]["rights_record_refs"] = json!([RIGHTS_REF]);
    packet["rights_and_visibility"]["source_visibility"] = json!("public");
    let make_anchor = |id: &str, role: &str, ordinal: usize, start: usize, end: usize| {
        json!({"anchor_ref":id,"anchor_role":role,"ordinal":ordinal,
            "exact_sha256":Digest256::of_bytes(&source_text[start..end]).to_hex(),
            "selector":{"type":"text_position","start":start,"end":end,
                "interval":"half_open","position_unit":"unicode_code_point"},
            "source_return":{"locator_ref":CONTENT_REF,"required":true},
            "text_layer_ref":LAYER_REF,"text_layer_sha256":source_sha})
    };
    let scope_anchor = "tos.anchor.synthetic.flame-scope";
    let flame_anchors = [
        "tos.anchor.semantic-v2.synthetic-flame-1",
        "tos.anchor.semantic-v2.synthetic-flame-2",
    ];
    let gap_anchors = [
        "tos.anchor.synthetic.flame-gap-1",
        "tos.anchor.synthetic.flame-gap-2",
        "tos.anchor.synthetic.flame-gap-3",
    ];
    packet["anchors"] = json!([
        make_anchor(scope_anchor, "scope", 1, 0, 109),
        make_anchor(flame_anchors[0], "content", 2, 13, 18),
        make_anchor(flame_anchors[1], "content", 3, 62, 67),
        make_anchor(gap_anchors[0], "gap", 4, 0, 13),
        make_anchor(gap_anchors[1], "gap", 5, 18, 62),
        make_anchor(gap_anchors[2], "gap", 6, 67, 109)
    ]);
    packet["segmentations"][0]["segmentation_id"] = json!(SEGMENTATION_ID);
    packet["segmentations"][0]["coverage"]["scope_anchor_ref"] = json!(scope_anchor);
    packet["segmentations"][0]["coverage"]["excluded_anchor_refs"] = json!(gap_anchors);
    let mut first_unit = packet["units"][0].clone();
    first_unit["unit_id"] = json!(UNIT_IDS[0]);
    first_unit["ordered_anchor_refs"] = json!([flame_anchors[0]]);
    first_unit["status_reason"] =
        json!("Exact first flame span in the public-synthetic fixture; no semantic authority.");
    let mut second_unit = packet["units"][0].clone();
    second_unit["unit_id"] = json!(UNIT_IDS[1]);
    second_unit["ordered_anchor_refs"] = json!([flame_anchors[1]]);
    second_unit["status_reason"] =
        json!("Exact second flame span in the public-synthetic fixture; no semantic authority.");
    packet["units"] = json!([first_unit, second_unit]);
    packet["segmentations"][0]["ordered_unit_refs"] = json!(UNIT_IDS);
    let packet_bytes = serde_json::to_vec(&packet).map_err(invalid_data)?;
    let packet_sha = Digest256::of_bytes(&packet_bytes).to_hex();
    write_public_bytes(&public_root.join(PACKET_REF), &packet_bytes)?;

    let native_binding = |index: usize| -> io::Result<Value> {
        let unit = &packet["units"][index];
        Ok(json!({"schema_version":"tos_native_text_unit_binding_v1",
            "packet_ref":PACKET_REF,"packet_sha256":packet_sha,"packet_id":packet["packet_id"],
            "packet_version":packet["packet_version"],"unit_id":unit["unit_id"],"unit_version":unit["unit_version"],
            "ordered_anchor_refs":unit["ordered_anchor_refs"],
            "segmentation_id":packet["segmentations"][0]["segmentation_id"],
            "segmentation_version":packet["segmentations"][0]["segmentation_version"],
            "text_layer":{"record_ref":LAYER_REF,"record_sha256":layer_sha,
                "layer_id":layer["layer_id"],"layer_version":layer["layer_version"]},
            "source_record_refs":{"work":"ToS/source-witnesses/works/synthetic-native-binding/work.json",
                "expression":"ToS/source-witnesses/works/synthetic-native-binding/expressions/und-synthetic/expression.json",
                "edition":"ToS/source-witnesses/works/synthetic-native-binding/expressions/und-synthetic/editions/synthetic-edition/edition.json",
                "item":"ToS/source-witnesses/works/synthetic-native-binding/expressions/und-synthetic/editions/synthetic-edition/items/synthetic-item/item.json"}}))
    };
    let origin = "synthetic-two-flame-sign-source-origin";
    let native_selection = json!([
        {"binding":native_binding(0)?,"origin_id":origin,"read_scope":"exact_public"},
        {"binding":native_binding(1)?,"origin_id":origin,"read_scope":"exact_public"}
    ]);
    let files = super::command_text_cases::authored_text_files(&public_root);
    let deadline = Instant::now() + Duration::from_secs(240);
    let cancelled = AtomicBool::new(false);
    let store = root.join("sign-fixture-cut");
    let revision = super::validation_cut_cases::write_cut_store(&files, &store);
    let cut = super::command_form_cases::open_cut(&store, revision, deadline, &cancelled);
    let mut worker = super::command_form_cases::schemas(&cut, deadline, &cancelled);
    let mut reader = FrozenPublicRead(files);
    let mut unit_refs = Vec::new();
    let mut unit_quality_refs = Vec::new();
    for index in 0..2 {
        let binding = native_binding(index)?;
        let exact = tos_command::resolve_public_native_assessment_for_conformance(
            &mut reader,
            &mut worker,
            &foundation_value(&binding)?,
            origin,
            tos_command::PublicNativeReadScopeForConformance::PublicContent,
            deadline,
            &cancelled,
        )
        .map_err(invalid_data)?;
        let records = exact
            .records
            .iter()
            .map(serde_value)
            .collect::<io::Result<Vec<_>>>()?;
        unit_refs.push(native_record_ref(
            &records,
            packet["units"][index]["unit_id"].as_str().unwrap(),
        )?);
        let layer_id = binding["text_layer"]["layer_id"]
            .as_str()
            .ok_or_else(|| invalid_data("native quality layer identity absent"))?;
        unit_quality_refs.push(native_record_ref(&records, layer_id)?);
    }
    use tos_validation::source_cut::CutSchemaExecutor;
    worker
        .finish(deadline, &cancelled)
        .map_err(|e| invalid_data(format!("{e:?}")))?;

    let occurrence_records = OCCURRENCES.iter().enumerate().map(|(index,id)| -> io::Result<Value> {
        let offset = if index == 0 { 13 } else { 62 };
        Ok(json!({"schema_version":"tos_occurrence_description_record_v1","record_type":"occurrence",
            "record_id":id,"record_version":1,"preferred_label":"flame",
            "field_languages":{"preferred_label":{"language":"en","script":"Latn"},
                "notes":{"language":"en","script":"Latn"}},
            "identity_status":"provisional","source_refs":[PACKET_REF],"external_identifiers":[],
            "same_as_posture":"no_equivalence_claim","visibility":"public_metadata_only",
            "notes":"Two-flame source fixture occurrence; no historical or general semantic claim.",
            "semantic_scope":{"scope_note":"One exact synthetic occurrence only.",
                "identity_criterion":"This identity is bound to the exact native unit, not to shared spelling.",
                "language":"en","script":"Latn"},
            "semantic_content":{"language":"en","script":"Latn",
                "occurrence_account":format!("Literal word flame at source offset {offset}."),
                "context_account":if index==0 {"Dawn line, paired only with the named dusk occurrence."} else {"Dusk line, paired only with the named dawn occurrence."}},
            "native_text_binding":native_binding(index)?,"supersedes_ref":null}))
    }).collect::<io::Result<Vec<Value>>>()?;
    let candidate = json!({"schema_version":"tos_source_occurrence_motif_claim_v1",
        "claim_id":CANDIDATE,"claim_type":"relation","assertion_layer":"semantic_interpretation",
        "subject_ref":OCCURRENCES[0],"predicate":"occurrence_motif_proposal",
        "object":{"kind":"motif-proposal","members":OCCURRENCES,
            "proposed_signification":"Across this pair, the repeated flame carries a remembered promise into an open question. The candidate is a motif of continuity under change, bounded to these two synthetic occurrences.",
            "grouping_basis":"The exact repeated word flame, its dawn/dusk pairing, and the promise/question contrast.",
            "source_scope":"Only the two named public-synthetic occurrences in this exact source packet.",
            "contrast":"They may be two separate literal flame images with no stable symbolism; the second line questions an answer without establishing a general signification.",
            "limitations":"No historical, Nietzsche, tradition, translation, truth, canon, or publication generalization.",
            "source_wording":{"wording_kind":"research_paraphrase",
                "text":"A bounded proposal of flame continuity under change.","language":"en","script":"Latn"}},
        "evidence_refs":[PACKET_REF],"maker":{"agent_ref":"software:two-flame-sign-fixture-builder","maker_type":"software"},
        "provenance_event_ref":"tos.event.fixture.flame-sign-claim","epistemic_status":"inferred",
        "review_status":"unreviewed","claim_version":1,"visibility":"public_metadata_only",
        "qualifiers":{"statement":"A bounded proposal that the repeated flame carries a remembered promise into an open question.",
            "statement_language":"en","statement_script":"Latn"}});
    let source_path = CLAIM_PATH.to_owned();
    let occurrence_paths = [
        "ToS/source-witnesses/occurrences/synthetic-flame-1/occurrence.json",
        "ToS/source-witnesses/occurrences/synthetic-flame-2/occurrence.json",
    ];
    for (path, row) in occurrence_paths.iter().zip(&occurrence_records) {
        write_public_bytes(
            &public_root.join(path),
            &serde_json::to_vec(row).map_err(invalid_data)?,
        )?;
    }
    let candidate_raw = serde_json::to_vec(&candidate).map_err(invalid_data)?;
    write_public_bytes(
        &public_root.join(&source_path),
        &[candidate_raw, vec![b'\n']].concat(),
    )?;

    let candidate_ref = record_ref(CANDIDATE, 1, &candidate)?;
    let occurrence_refs = occurrence_records
        .iter()
        .zip(OCCURRENCES)
        .map(|(row, id)| record_ref(id, 1, row))
        .collect::<io::Result<Vec<_>>>()?;
    let policy_raw =
        fs::read(repository.join("ToS/doctrine/semantic-interchange/assessment-policy.v3.json"))?;
    let policy_payload: Value = serde_json::from_slice(&policy_raw).map_err(invalid_data)?;
    let policy_ref = record_ref("tos.policy.knowledge-assessment", 3, &policy_payload)?;
    let policy = envelope(
        "tos.policy.knowledge-assessment",
        3,
        policy_payload,
        Value::Null,
    );
    let executor_payload =
        json!({"procedure_ref":"fixture:source-check","model_ref":"fixture:not-a-real-model"});
    let executor_ref = record_ref("tos.method.fixture-review", 1, &executor_payload)?;
    let executor = envelope(
        "tos.method.fixture-review",
        1,
        executor_payload,
        Value::Null,
    );
    let calibration_payload = json!({"synthetic":true,"fixture_only":true});
    let calibration_ref = record_ref("tos.review.fixture-calibration", 1, &calibration_payload)?;
    let calibration = envelope(
        "tos.review.fixture-calibration",
        1,
        calibration_payload,
        Value::Null,
    );
    let competence_id = "tos.competence.agent-codex-tos-foundation";
    let competence_payload = json!({"schema_version":"tos_knowledge_assessment_competence_v1",
        "competence_id":competence_id,"competence_version":1,"actor_id":"agent:codex-tos-foundation",
        "assertion_layers":["semantic_interpretation"],"languages":["en"],"profile_ids":["sign-promotion"],
        "execution_profiles":[executor_ref],"state":"verified","valid_from":START,"valid_until":END,
        "evidence_refs":[calibration_ref],"issuer_ref":"fixture:simulated-competence-grant"});
    let competence_ref = record_ref(competence_id, 1, &competence_payload)?;
    let competence = envelope(competence_id, 1, competence_payload, Value::Null);
    let authority_id = "tos.authority.agent-codex-tos-foundation";
    let authority_payload = json!({"schema_version":"tos_knowledge_assessment_authority_v1",
        "authority_id":authority_id,"authority_version":1,"actor_id":"agent:codex-tos-foundation","actor_kind":"agent",
        "policy":policy_ref,"profile_ids":["sign-promotion"],"assertion_layers":["semantic_interpretation"],
        "languages":["en"],"uses":["sign-promotion"],"subject_prefixes":["tos.claim.fixture.flame-continuity-under-change"],
        "decisions":["admit","admit-with-limits","reject","dispute","defer","withdraw"],
        "competence_refs":[competence_ref],"independence_group":"agent:codex-tos-foundation","can_supersede_others":false,
        "state":"active","valid_from":START,"valid_until":END,"issuer_ref":"fixture:simulated-operator-grant"});
    let authority_ref = record_ref(authority_id, 1, &authority_payload)?;
    let authority = envelope(authority_id, 1, authority_payload, Value::Null);
    let assessment = json!({"schema_version":"tos_knowledge_assessment_v1","assessment_id":"tos.review.fixture.flame-continuity-under-change",
        "subject":candidate_ref,"policy":policy_ref,"profile_id":"sign-promotion","authority":authority_ref,
        "competence":competence_ref,"reviewer":{"actor_id":"agent:codex-tos-foundation","kind":"agent"},
        "decision":"admit-with-limits",
        "rationale":"The repeated flame supports a bounded continuity-under-change interpretation across dawn and dusk. The phrase 'carries a remembered promise into an open question' is interpretive, not a factual event stated by the text.",
        "language":"en","evidence":[
            {"record":occurrence_refs[0],"stance":"supports","locator":"Exact dawn occurrence record bound to the selected native text unit."},
            {"record":unit_refs[0],"stance":"supports","locator":"Exact native TextUnit assessment record for the dawn occurrence."},
            {"record":unit_quality_refs[0],"stance":"supports","locator":"Exact TextLayer quality record for the dawn native unit."},
            {"record":occurrence_refs[1],"stance":"supports","locator":"Exact dusk occurrence record bound to the selected native text unit."},
            {"record":unit_refs[1],"stance":"supports","locator":"Exact native TextUnit assessment record for the dusk occurrence."},
            {"record":unit_quality_refs[1],"stance":"supports","locator":"Exact TextLayer quality record for the dusk native unit."}],
        "counterevidence_search":{"status":"searched","note":"Fixture-local source-visible review retained the counterreading of two separate literal flame images with no stable symbolism."},
        "limits":["Only these two public-synthetic occurrences.","The carries-into wording is interpretive, not a factual text event.",
            "No historical, Nietzsche, tradition, translation, truth, canon, or publication claim."],
        "method":{"procedure_ref":"fixture:source-check","invocation_ref":"fixture:no-real-invocation","model_ref":"fixture:not-a-real-model","execution_profile":executor_ref},
        "issued_at":NOW,"supersedes":[]});
    let mut subjects = serde_json::Map::new();
    subjects.insert(
        CANDIDATE.to_owned(),
        json!({"record":candidate_ref,"assertion_layer":"semantic_interpretation",
        "risk":"moderate","languages":["en"],"maker_id":"software:two-flame-sign-fixture-builder",
        "requested_use":"sign-promotion","access_allowed":true}),
    );
    let unit_maker = packet["segmentations"][0]["maker"]["agent_ref"].clone();
    for (index, binding) in [native_binding(0)?, native_binding(1)?].iter().enumerate() {
        let unit_id = binding["unit_id"]
            .as_str()
            .ok_or_else(|| invalid_data("native unit identity absent"))?;
        let unit_ref = unit_refs
            .get(index)
            .ok_or_else(|| invalid_data("native unit reference absent"))?
            .clone();
        subjects.insert(
            unit_id.to_owned(),
            json!({"record":unit_ref,"assertion_layer":"textual_observation",
            "risk":"low","languages":["en"],"maker_id":unit_maker.clone(),
            "requested_use":"research","access_allowed":true}),
        );
    }
    let source_records = std::iter::once(
        json!({"path":source_path.clone(),"record_id":CANDIDATE,"origin_id":origin}),
    )
    .chain(
        OCCURRENCES
            .into_iter()
            .zip(occurrence_paths)
            .map(|(id, path)| json!({"path":path,"record_id":id,"origin_id":origin})),
    )
    .collect::<Vec<_>>();
    let assessment_owner_config = json!({"schema_version":"tos_local_assessment_owner_v3","uid":fs::metadata(root)?.uid(),
        "principal_id":"agent:codex-tos-foundation","execution_profile":executor_ref,"policy":policy,
        "authorities":[authority],"competencies":[competence],"records":[calibration,executor],
        "journal_directory":root.join("sign-assessment-journal"),"source_root":public_root,
        "source_records":source_records,"native_text_units":native_selection,"subjects":subjects});
    let assessment_journal = root.join("sign-assessment-journal");
    secure_dir(&assessment_journal)?;
    let assessment_owner_config_path = root.join("sign-assessment-owner.json");
    write_private_json(&assessment_owner_config_path, &assessment_owner_config)?;
    let private_root = root.join("unused-private-boundary");
    secure_dir(&private_root)?;
    let context = owner_context(&public_root, &private_root);
    let assessment_owner_context_path = root.join("sign-assessment-context.json");
    write_private_json(&assessment_owner_context_path, &context)?;
    let sign_record = json!({"schema_version":"tos_sign_description_record_v1","record_type":"sign",
        "record_id":"tos.sign.fixture.flame-continuity-under-change","record_version":1,
        "preferred_label":"Flame continuity under change","identity_status":"provisional",
        "source_refs":[source_path],"external_identifiers":[],"same_as_posture":"no_equivalence_claim",
        "notes":"Bounded candidate for the two named synthetic flame occurrences: a remembered promise carried into an open question. This remains an interpretation of this pair only; it makes no historical, Nietzsche, tradition, translation, truth, canon, or publication claim.",
        "field_languages":{"preferred_label":{"language":"en","script":"Latn"},"notes":{"language":"en","script":"Latn"}},
        "visibility":"public_metadata_only","promotion_basis":null});
    let forms = vec![
        json!({"form_id":"tos.form.synthetic-sign.flame-continuity.name","field_id":"metadata.preferred-name"}),
        json!({"form_id":"tos.form.synthetic-sign.flame-continuity.note","field_id":"metadata.source-note"}),
    ];
    // Native creation checks the target's parent before prepare/promote; the
    // captured public-profile seed has no Sign destination directory.
    secure_dir(&public_root.join("ToS/source-witnesses/signs"))?;
    let sign_owner_config = json!({"schema_version":"tos_local_sign_promote_owner_v1","uid":fs::metadata(root)?.uid(),
        "principal_id":"software:synthetic-sign-writer","maker_type":"software",
        "source_root":public_root,"source_path":"ToS/source-witnesses/signs/flame-continuity/sign.json",
        "record_id":"tos.sign.fixture.flame-continuity-under-change","provenance_event_id":"tos.event.fixture.flame-sign-issuance",
        "authority_ref":"fixture:separate-synthetic-sign-issuance-grant","expires_at":END,
        "allowed_operations":["sign.promote"],"allowed_form_ids":[forms[0]["form_id"],forms[1]["form_id"]],
        "promotion_assessment_owner_config":assessment_owner_config_path,
        "promotion_candidate_id":CANDIDATE,"profile_type_id":"tos.entity.sign"});
    let sign_owner_config_path = root.join("sign-owner.json");
    write_private_json(&sign_owner_config_path, &sign_owner_config)?;
    let occurrence_ids = [OCCURRENCES[0].to_owned(), OCCURRENCES[1].to_owned()];
    Ok(NativePublicSignFixture {
        public_root,
        assessment_owner_config_path,
        assessment_owner_context_path,
        sign_owner_config_path,
        candidate_id: CANDIDATE.to_owned(),
        occurrence_ids,
        source_text,
        assessment,
        sign_record,
        forms,
        source_path,
    })
}

/// Construct the read-batch fixture: one pending freeform form and six ready-
/// eligible source-copy forms over the same exact work record.
pub fn native_public_v2_assessed_form_batch_fixture(
    repository: &Path,
    root: &Path,
) -> io::Result<NativePublicAssessmentFixture> {
    build_public_v2(repository, root, 7)
}

fn build_inline_v1(repository: &Path, root: &Path) -> io::Result<NativePublicAssessmentFixture> {
    initialize_root(root)?;
    let public_root = root.join("inline-contracts");
    secure_dir(&public_root)?;
    copy_contracts(repository, &public_root)?;
    let private_root = root.join("unused-private-boundary");
    secure_dir(&private_root)?;
    let journal_directory = root.join("journal");
    secure_dir(&journal_directory)?;

    let (policy, policy_ref) = policy_record(repository)?;
    let executor_payload =
        json!({"procedure_ref":"fixture:source-check","model_ref":"fixture:not-a-real-model"});
    let executor_id = "tos.method.fixture-review";
    let executor_ref = record_ref(executor_id, 1, &executor_payload)?;
    let executor = envelope(executor_id, 1, executor_payload.clone(), Value::Null);
    let calibration_payload = json!({"synthetic":true});
    let calibration_id = "tos.review.fixture-calibration";
    let calibration_ref = record_ref(calibration_id, 1, &calibration_payload)?;
    let calibration = envelope(calibration_id, 1, calibration_payload, Value::Null);

    let subject_id = "tos.claim.fixture";
    let subject_payload = json!({"claim":"synthetic assertion"});
    let subject_ref = record_ref(subject_id, 1, &subject_payload)?;
    let subject = envelope(subject_id, 1, subject_payload, Value::Null);
    let evidence_id = "tos.file.fixture-a";
    let evidence_payload = json!({"text":"synthetic source A"});
    let evidence_ref = record_ref(evidence_id, 1, &evidence_payload)?;
    let evidence = envelope(evidence_id, 1, evidence_payload, json!("source-a"));
    let source_b = envelope(
        "tos.file.fixture-b",
        1,
        json!({"text":"synthetic source B"}),
        json!("source-b"),
    );

    let (competence, competence_ref) = competence(
        "assessor-a",
        &executor_ref,
        &calibration_ref,
        "bibliographic_assertion",
    )?;
    let (authority, authority_ref) = authority(
        "assessor-a",
        &policy_ref,
        &competence_ref,
        "tos.claim.",
        "bibliographic_assertion",
    )?;
    let assessment = assessment(
        "tos.review.assessor-a",
        &subject_ref,
        &policy_ref,
        &authority_ref,
        &competence_ref,
        &evidence_ref,
        &executor_ref,
        "source-observation",
        "ru",
    );
    let mut subjects = serde_json::Map::new();
    subjects.insert(
        subject_id.to_owned(),
        json!({
            "record":subject_ref,
            "assertion_layer":"bibliographic_assertion",
            "risk":"low","languages":["de"],"maker_id":"extractor",
            "requested_use":"research","access_allowed":true
        }),
    );
    let owner_config = json!({
        "schema_version":"tos_local_assessment_owner_v1",
        "uid":fs::metadata(root)?.uid(),
        "principal_id":"assessor-a",
        "execution_profile":executor_ref,
        "policy":policy,
        "authorities":[authority],
        "competencies":[competence],
        "records":[subject,evidence,source_b,calibration,executor],
        "journal_directory":journal_directory,
        "subjects":subjects
    });
    let owner_config_path = root.join("owner.json");
    write_private_json(&owner_config_path, &owner_config)?;
    let owner_context = owner_context(&public_root, &private_root);
    let owner_context_path = root.join("native-context.json");
    write_private_json(&owner_context_path, &owner_context)?;
    let describe_requests = vec![describe_request(subject_id)];
    let append_assessments = vec![assessment.clone()];
    let withdrawal_assessments = vec![withdrawal_from(&assessment)?];
    let preserved = inventory(&public_root)?;
    Ok(NativePublicAssessmentFixture {
        version: 1,
        owner_config_path,
        public_root,
        owner_context_path,
        journal_directory,
        owner_config,
        owner_context,
        subject_ids: vec![subject_id.to_owned()],
        subject_refs: vec![subject_ref],
        describe_requests,
        append_assessments,
        withdrawal_assessments,
        source_root: None,
        source_record: None,
        source_ref: None,
        source_bytes: None,
        form_set_path: None,
        form_set_bytes: None,
        forms: Vec::new(),
        nodes: Vec::new(),
        pending_selection: None,
        ready_selections: Vec::new(),
        metadata_subject: None,
        private_inventory: Vec::new(),
        preserved,
    })
}

fn build_public_v2(
    repository: &Path,
    root: &Path,
    form_count: usize,
) -> io::Result<NativePublicAssessmentFixture> {
    if form_count == 0 || form_count > 7 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "public v2 fixture form count must be 1..=7",
        ));
    }
    initialize_root(root)?;
    let public_root = root.join("source-copy");
    secure_dir(&public_root)?;
    copy_contracts(repository, &public_root)?;
    let private_root = root.join("unused-private-boundary");
    secure_dir(&private_root)?;
    let journal_directory = root.join("journal");
    secure_dir(&journal_directory)?;

    let source_bytes = fs::read(repository.join(WORK_REF))?;
    let source_record: Value = serde_json::from_slice(&source_bytes).map_err(invalid_data)?;
    let source_version = source_record["record_version"]
        .as_u64()
        .ok_or_else(|| invalid_data("work record version is absent"))?;
    let source_ref = record_ref(SUBJECT_ID, source_version, &source_record)?;
    let work_path = public_root.join(WORK_REF);
    write_public_bytes(&work_path, &source_bytes)?;

    let mut forms = Vec::with_capacity(form_count);
    let mut metadata_context = Vec::new();
    for key in [
        "identity_status",
        "same_as_posture",
        "semantic_scope",
        "semantic_content",
        "form_identity",
        "native_text_binding",
    ] {
        if source_record.get(key).is_some() {
            metadata_context.push(format!("/{key}"));
        }
    }
    if source_record.pointer("/field_languages/notes").is_some() {
        metadata_context.push("/field_languages/notes".to_owned());
    }
    let notes_language = source_record
        .pointer("/field_languages/notes/language")
        .cloned()
        .unwrap_or(Value::Null);
    let notes_script = source_record
        .pointer("/field_languages/notes/script")
        .cloned()
        .unwrap_or(Value::Null);
    for index in 0..form_count {
        let id = if form_count == 1 {
            "tos.form.fixture-assessed".to_owned()
        } else {
            format!("tos.form.batch-fixture-{index}")
        };
        let form = if index == 0 {
            json!({"schema_version":"tos_human_form_v1","form_id":id,"form_version":1,
                "subject":source_ref,"role":"hover","language":"ru","script":"Cyrl",
                "creator_id":"fixture-writer","revises":null,
                "bindings":{"context":{"record":source_ref,"pointer":""}},
                "content":{"kind":"freeform","text":"Синтетическая формулировка для проверки механики."}})
        } else {
            let mut bindings = json!({
                "wording":{"record":source_ref,"pointer":"/notes"}
            });
            for (context_index, pointer) in metadata_context.iter().enumerate() {
                bindings[format!("context-{context_index}")] =
                    json!({"record":source_ref,"pointer":pointer});
            }
            json!({"schema_version":"tos_human_form_v1","form_id":id,"form_version":1,
                "subject":source_ref,"role":"hover","language":notes_language,"script":notes_script,
                "creator_id":"fixture-writer","revises":null,"bindings":bindings,
                "content":{"kind":"source-copy","slot":"wording"}})
        };
        forms.push(form);
    }
    let form_set = json!({"schema_version":"tos_human_form_set_v1","subject":source_ref,
        "forms":forms,"prior_forms":[]});
    let form_set_bytes = serde_json::to_vec(&form_set).map_err(invalid_data)?;
    let form_set_path = public_root.join(FORMS_REF);
    write_public_bytes(&form_set_path, &form_set_bytes)?;

    let (policy, policy_ref) = policy_record(repository)?;
    let executor_payload =
        json!({"procedure_ref":"fixture:source-check","model_ref":"fixture:not-a-real-model"});
    let executor_ref = record_ref("tos.method.fixture-review", 1, &executor_payload)?;
    let executor = envelope(
        "tos.method.fixture-review",
        1,
        executor_payload,
        Value::Null,
    );
    let calibration_payload = json!({"synthetic":true});
    let calibration_ref = record_ref("tos.review.fixture-calibration", 1, &calibration_payload)?;
    let calibration = envelope(
        "tos.review.fixture-calibration",
        1,
        calibration_payload,
        Value::Null,
    );
    let source_b = envelope(
        "tos.file.fixture-b",
        1,
        json!({"text":"synthetic source B"}),
        json!("source-b"),
    );
    let (competence, competence_ref) = competence(
        "assessor-a",
        &executor_ref,
        &calibration_ref,
        "human_projection",
    )?;
    let (authority, authority_ref) = authority(
        "assessor-a",
        &policy_ref,
        &competence_ref,
        "tos.form.",
        "human_projection",
    )?;
    let source_row =
        json!({"path":WORK_REF,"record_id":SUBJECT_ID,"origin_id":"fixture-source-origin"});
    let source_form_rows: Vec<Value> = forms
        .iter()
        .map(|form| {
            json!({
                "path":FORMS_REF,"record_id":form["form_id"],"origin_id":null
            })
        })
        .collect();
    let source_records: Vec<Value> = std::iter::once(source_row)
        .chain(source_form_rows)
        .collect();
    let mut subjects = serde_json::Map::new();
    let mut subject_ids = Vec::with_capacity(form_count);
    let mut subject_refs = Vec::with_capacity(form_count);
    let mut describe_requests = Vec::with_capacity(form_count);
    let mut append_assessments = Vec::with_capacity(form_count);
    let mut withdrawal_assessments = Vec::with_capacity(form_count);
    let mut nodes_forms = Vec::with_capacity(form_count);
    for form in &forms {
        let id = form["form_id"]
            .as_str()
            .ok_or_else(|| invalid_input("missing form id"))?
            .to_owned();
        let form_ref = record_ref(&id, 1, form)?;
        subjects.insert(
            id.clone(),
            json!({"record":form_ref,"assertion_layer":"human_projection",
            "risk":"low","languages":["ru","de"],"maker_id":"fixture-writer",
            "requested_use":"research","access_allowed":true}),
        );
        let assessment = assessment(
            &format!("tos.review.{id}"),
            &form_ref,
            &policy_ref,
            &authority_ref,
            &competence_ref,
            &source_ref,
            &executor_ref,
            "interpretation",
            "ru",
        );
        subject_ids.push(id.clone());
        subject_refs.push(form_ref.clone());
        describe_requests.push(describe_request(&id));
        append_assessments.push(assessment.clone());
        withdrawal_assessments.push(withdrawal_from(&assessment)?);
        nodes_forms.push(json!({"schema_version":"tos_human_form_materialization_v1",
            "form":form_ref,"subject":source_ref,"state":"needs-assessment","display_text":null,
            "context":[],"issues":["assessment.required"],"admission":null,
            "performs_semantic_assessment":false}));
    }
    let owner_config = json!({
        "schema_version":"tos_local_assessment_owner_v2",
        "uid":fs::metadata(root)?.uid(),
        "principal_id":"assessor-a",
        "execution_profile":executor_ref,
        "policy":policy,
        "authorities":[authority],
        "competencies":[competence],
        "records":[source_b,calibration,executor],
        "journal_directory":journal_directory,
        "source_root":public_root,
        "source_records":source_records,
        "subjects":subjects
    });
    let owner_config_path = root.join("owner.json");
    write_private_json(&owner_config_path, &owner_config)?;
    let owner_context = owner_context(&public_root, &private_root);
    let owner_context_path = root.join("native-context.json");
    write_private_json(&owner_context_path, &owner_context)?;
    let nodes = vec![json!({"node_id":"fixture:source","source_ref":WORK_REF,
        "source_sha256":source_ref["digest"].as_str().unwrap_or_default().trim_start_matches("sha256:"),
        "properties":{"source_record":source_record,"human_forms_source_ref":FORMS_REF,
            "human_forms":nodes_forms}})];
    let preserved = inventory(&public_root)?;
    Ok(NativePublicAssessmentFixture {
        version: 2,
        owner_config_path,
        public_root: public_root.clone(),
        owner_context_path,
        journal_directory,
        owner_config,
        owner_context,
        subject_ids,
        subject_refs,
        describe_requests,
        append_assessments,
        withdrawal_assessments,
        source_root: Some(public_root),
        source_record: Some(source_record),
        source_ref: Some(source_ref),
        source_bytes: Some(source_bytes),
        form_set_path: Some(form_set_path),
        form_set_bytes: Some(form_set_bytes),
        forms,
        nodes,
        pending_selection: Some(0),
        ready_selections: (1..form_count).collect(),
        metadata_subject: None,
        private_inventory: Vec::new(),
        preserved,
    })
}

/// Immutable test transport. Actual CLI calls independently reopen the same
/// selected source root through the production protected source reader.
struct FrozenPublicRead(std::collections::BTreeMap<String, Vec<u8>>);
impl tos_command::PublicNativeReadForConformance for FrozenPublicRead {
    fn read(
        &mut self,
        reference: &str,
        _kind: tos_command::PublicNativeReadKindForConformance,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> tos_command::source_command::SourceCommandResult<Vec<u8>> {
        use tos_command::source_command::SourceCommandError as E;
        self.verify_current(deadline, cancelled)?;
        tos_foundation::RelativePath::parse(reference)
            .map_err(|_| E::Invalid("fixture relative source path"))?;
        let bytes = self
            .0
            .get(reference)
            .ok_or(E::Invalid("fixture source absent"))?;
        if bytes.len() > max_bytes {
            return Err(E::Invalid("fixture read byte cap"));
        }
        Ok(bytes.clone())
    }
    fn verify_current(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> tos_command::source_command::SourceCommandResult<()> {
        if Instant::now() >= deadline || cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(tos_command::source_command::SourceCommandError::Invalid(
                "fixture execution ended",
            ));
        }
        Ok(())
    }
    fn owner_local(
        &self,
        _reference: &str,
    ) -> tos_command::source_command::SourceCommandResult<bool> {
        Ok(false)
    }
}

fn build_public_v3(repository: &Path, root: &Path) -> io::Result<NativePublicAssessmentFixture> {
    initialize_root(root)?;
    let deadline = Instant::now() + Duration::from_secs(240);
    let cancelled = AtomicBool::new(false);
    // Use the complete original public binding from the retained Profile seed.
    // The public Text construction seed replaces its Item manifest for a new
    // Markdown payload, so its older TextUnit intentionally no longer matches.
    // Only this seed's public subtree is selected by FrozenPublicRead below.
    let workspace = root.join("public-text-fixture");
    let _captured = super::native_python_fixture(
        "private-profile-base",
        &[("source-root", &workspace)],
        &["rust/crates/tos-command/src/source_sign_native.rs"],
    );
    let public_root = workspace.join("public");
    copy_contracts(repository, &public_root)?;
    let base = "ToS/source-witnesses/works/synthetic-native-binding";
    let home = format!("{base}/technical-markup/synthetic-binding");
    let packet_ref = format!("{home}/source-text-unit.synthetic.v1.json");
    let layer_ref_path = format!("{home}/source-text-layer.synthetic.v1.json");
    let packet_bytes = fs::read(public_root.join(&packet_ref))?;
    let packet: Value = serde_json::from_slice(&packet_bytes).map_err(invalid_data)?;
    let layer_bytes = fs::read(public_root.join(&layer_ref_path))?;
    let layer: Value = serde_json::from_slice(&layer_bytes).map_err(invalid_data)?;
    let unit = &packet["units"][0];
    let segmentation = &packet["segmentations"][0];
    let expression = format!("{base}/expressions/und-synthetic");
    let edition = format!("{expression}/editions/synthetic-edition");
    let binding = json!({
        "schema_version":"tos_native_text_unit_binding_v1",
        "packet_ref":packet_ref,"packet_sha256":Digest256::of_bytes(&packet_bytes).to_hex(),
        "packet_id":packet["packet_id"],"packet_version":packet["packet_version"],
        "unit_id":unit["unit_id"],"unit_version":unit["unit_version"],
        "ordered_anchor_refs":unit["ordered_anchor_refs"],
        "segmentation_id":segmentation["segmentation_id"],
        "segmentation_version":segmentation["segmentation_version"],
        "text_layer":{"record_ref":layer_ref_path,
            "record_sha256":Digest256::of_bytes(&layer_bytes).to_hex(),
            "layer_id":layer["layer_id"],"layer_version":layer["layer_version"]},
        "source_record_refs":{"work":format!("{base}/work.json"),
            "expression":format!("{expression}/expression.json"),
            "edition":format!("{edition}/edition.json"),
            "item":format!("{edition}/items/synthetic-item/item.json")}
    });
    let subject_id = binding["unit_id"]
        .as_str()
        .ok_or_else(|| invalid_data("native TextUnit identity absent"))?
        .to_owned();
    let origin = "synthetic-native-text-assessment-origin";
    let native_selection_value = json!([{"binding":binding,"origin_id":origin,
        "read_scope":"exact_owner_local"}]);
    let files = super::command_text_cases::authored_text_files(&public_root);
    let store = root.join("public-fixture-cut");
    let revision = super::validation_cut_cases::write_cut_store(&files, &store);
    let cut = super::command_form_cases::open_cut(&store, revision, deadline, &cancelled);
    let mut worker = super::command_form_cases::schemas(&cut, deadline, &cancelled);
    let mut reader = FrozenPublicRead(files);
    let binding_value = foundation_value(&binding)?;
    let exact = tos_command::resolve_public_native_assessment_for_conformance(
        &mut reader,
        &mut worker,
        &binding_value,
        origin,
        tos_command::PublicNativeReadScopeForConformance::ExactOwnerLocal,
        deadline,
        &cancelled,
    )
    .map_err(invalid_data)?;
    let metadata = tos_command::resolve_public_native_assessment_for_conformance(
        &mut reader,
        &mut worker,
        &binding_value,
        origin,
        tos_command::PublicNativeReadScopeForConformance::MetadataOnly,
        deadline,
        &cancelled,
    )
    .map_err(invalid_data)?;
    use tos_validation::source_cut::CutSchemaExecutor;
    worker
        .finish(deadline, &cancelled)
        .map_err(|e| invalid_data(format!("{e:?}")))?;
    let native_records = exact
        .records
        .iter()
        .map(serde_value)
        .collect::<io::Result<Vec<_>>>()?;
    let metadata_records = metadata
        .records
        .iter()
        .map(serde_value)
        .collect::<io::Result<Vec<_>>>()?;
    let subject_ref = native_record_ref(&native_records, &subject_id)?;
    let metadata_subject = native_record_ref(&metadata_records, &subject_id)?;
    if subject_ref == metadata_subject {
        return Err(invalid_data(
            "content and metadata-only subjects must differ",
        ));
    }
    let layer_id = binding["text_layer"]["layer_id"]
        .as_str()
        .ok_or_else(|| invalid_data("native layer identity absent"))?;
    let layer_ref = native_record_ref(&native_records, layer_id)?;
    let access_language = metadata
        .summary
        .object_get("language")
        .and_then(tos_foundation::JsonValue::as_str)
        .unwrap_or("und")
        .to_owned();

    let (policy, policy_ref) = policy_record(repository)?;
    let executor_payload =
        json!({"procedure_ref":"fixture:source-check","model_ref":"fixture:not-a-real-model"});
    let executor_ref = record_ref("tos.method.fixture-review", 1, &executor_payload)?;
    let executor = envelope(
        "tos.method.fixture-review",
        1,
        executor_payload,
        Value::Null,
    );
    let calibration_payload = json!({"synthetic":true});
    let calibration_ref = record_ref("tos.review.fixture-calibration", 1, &calibration_payload)?;
    let calibration = envelope(
        "tos.review.fixture-calibration",
        1,
        calibration_payload,
        Value::Null,
    );
    let competence_id = "tos.competence.assessor-a";
    let competence_payload = json!({
        "schema_version":"tos_knowledge_assessment_competence_v1",
        "competence_id":competence_id,"competence_version":1,"actor_id":"assessor-a",
        "assertion_layers":["textual_observation","linguistic_analysis"],
        "languages":[access_language],"profile_ids":["source-observation","interpretation","identity","high-consequence"],
        "execution_profiles":[executor_ref],"state":"verified","valid_from":START,"valid_until":END,
        "evidence_refs":[calibration_ref],"issuer_ref":"fixture:trusted-issuer-not-a-real-competence-claim"
    });
    let competence_ref = record_ref(competence_id, 1, &competence_payload)?;
    let competence = envelope(competence_id, 1, competence_payload, Value::Null);
    let authority_id = "tos.authority.assessor-a";
    let authority_payload = json!({
        "schema_version":"tos_knowledge_assessment_authority_v1",
        "authority_id":authority_id,"authority_version":1,"actor_id":"assessor-a","actor_kind":"agent",
        "policy":policy_ref,"profile_ids":["source-observation","interpretation","identity","high-consequence"],
        "assertion_layers":["textual_observation","linguistic_analysis"],"languages":[access_language],
        "uses":["research"],"subject_prefixes":["tos.text-unit."],
        "decisions":["admit","admit-with-limits","reject","dispute","defer","withdraw"],
        "competence_refs":[competence_ref],"independence_group":"assessor-a","can_supersede_others":false,
        "state":"active","valid_from":START,"valid_until":END,"issuer_ref":"fixture:trusted-operator-grant"
    });
    let authority_ref = record_ref(authority_id, 1, &authority_payload)?;
    let authority = envelope(authority_id, 1, authority_payload, Value::Null);
    let review = assessment(
        &format!("tos.review.{subject_id}"),
        &subject_ref,
        &policy_ref,
        &authority_ref,
        &competence_ref,
        &layer_ref,
        &executor_ref,
        "source-observation",
        &access_language,
    );
    let subject = json!({"record":subject_ref,"assertion_layer":"textual_observation",
        "risk":"low","languages":[access_language],"maker_id":segmentation["maker"]["agent_ref"],
        "requested_use":"research","access_allowed":true});
    let mut subjects = serde_json::Map::new();
    subjects.insert(subject_id.clone(), subject);
    let owner_config = json!({
        "schema_version":"tos_local_assessment_owner_v3","uid":fs::metadata(root)?.uid(),
        "principal_id":"assessor-a","execution_profile":executor_ref,"policy":policy,
        "authorities":[authority],"competencies":[competence],"records":[calibration,executor],
        "journal_directory":root.join("assessment-journal"),"source_root":public_root,
        "source_records":[],"native_text_units":native_selection_value,"subjects":subjects
    });
    let journal_directory = root.join("assessment-journal");
    secure_dir(&journal_directory)?;
    let owner_config_path = root.join("owner-v3.json");
    write_private_json(&owner_config_path, &owner_config)?;
    let private_root = root.join("unused-private-boundary");
    secure_dir(&private_root)?;
    let owner_context = owner_context(&public_root, &private_root);
    let owner_context_path = root.join("native-context.json");
    write_private_json(&owner_context_path, &owner_context)?;
    let preserved = inventory(&public_root)?;
    let private_inventory = inventory(&private_root)?;
    Ok(NativePublicAssessmentFixture {
        version: 3,
        owner_config_path,
        public_root: public_root.clone(),
        owner_context_path,
        journal_directory,
        owner_config,
        owner_context,
        subject_ids: vec![subject_id.clone()],
        subject_refs: vec![subject_ref.clone()],
        describe_requests: vec![describe_request(&subject_id)],
        append_assessments: vec![review.clone()],
        withdrawal_assessments: vec![withdrawal_from(&review)?],
        source_root: Some(public_root),
        source_record: None,
        source_ref: None,
        source_bytes: Some(packet_bytes),
        form_set_path: None,
        form_set_bytes: None,
        forms: Vec::new(),
        nodes: Vec::new(),
        pending_selection: None,
        ready_selections: Vec::new(),
        metadata_subject: Some(metadata_subject),
        private_inventory,
        preserved,
    })
}

fn policy_record(repository: &Path) -> io::Result<(Value, Value)> {
    let raw = fs::read(repository.join(POLICY_REF))?;
    let payload: Value = serde_json::from_slice(&raw).map_err(invalid_data)?;
    let id = "tos.policy.knowledge-assessment";
    let reference = record_ref(id, 1, &payload)?;
    Ok((envelope(id, 1, payload, Value::Null), reference))
}

fn competence(
    actor: &str,
    execution_profile: &Value,
    evidence: &Value,
    layer: &str,
) -> io::Result<(Value, Value)> {
    let id = format!("tos.competence.{actor}");
    let payload = json!({"schema_version":"tos_knowledge_assessment_competence_v1",
        "competence_id":id,"competence_version":1,"actor_id":actor,
        "assertion_layers":[layer],"languages":["ru","de"],
        "profile_ids":["source-observation","interpretation","identity","high-consequence"],
        "execution_profiles":[execution_profile],"state":"verified","valid_from":START,
        "valid_until":END,"evidence_refs":[evidence],
        "issuer_ref":"fixture:trusted-issuer-not-a-real-competence-claim"});
    let reference = record_ref(&id, 1, &payload)?;
    Ok((envelope(&id, 1, payload, Value::Null), reference))
}

fn authority(
    actor: &str,
    policy: &Value,
    competence: &Value,
    prefix: &str,
    layer: &str,
) -> io::Result<(Value, Value)> {
    let id = format!("tos.authority.{actor}");
    let payload = json!({"schema_version":"tos_knowledge_assessment_authority_v1",
        "authority_id":id,"authority_version":1,"actor_id":actor,"actor_kind":"agent",
        "policy":policy,"profile_ids":["source-observation","interpretation","identity","high-consequence"],
        "assertion_layers":[layer],"languages":["ru","de"],"uses":["research"],
        "subject_prefixes":[prefix],"decisions":["admit","admit-with-limits","reject","dispute","defer","withdraw"],
        "competence_refs":[competence],"independence_group":actor,"can_supersede_others":false,
        "state":"active","valid_from":START,"valid_until":END,"issuer_ref":"fixture:trusted-operator-grant"});
    let reference = record_ref(&id, 1, &payload)?;
    Ok((envelope(&id, 1, payload, Value::Null), reference))
}

fn assessment(
    id: &str,
    subject: &Value,
    policy: &Value,
    authority: &Value,
    competence: &Value,
    evidence: &Value,
    executor: &Value,
    profile: &str,
    language: &str,
) -> Value {
    json!({"schema_version":"tos_knowledge_assessment_v1","assessment_id":id,
        "subject":subject,"policy":policy,"profile_id":profile,"authority":authority,
        "competence":competence,"reviewer":{"actor_id":"assessor-a","kind":"agent"},
        "decision":"admit","rationale":"Синтетическая проверка механики; не реальное содержательное review.",
        "language":language,"evidence":[{"record":evidence,"stance":"supports","locator":"Synthetic fixture input; no substantive judgment."}],
        "counterevidence_search":{"status":"searched","note":"Синтетическая проверка поля, не реальный поиск."},
        "limits":[],"method":{"procedure_ref":"fixture:source-check","invocation_ref":"fixture:no-real-invocation",
            "model_ref":"fixture:not-a-real-model","execution_profile":executor},
        "issued_at":NOW,"supersedes":[]})
}

fn withdrawal_from(admission: &Value) -> io::Result<Value> {
    let mut withdrawal = admission.clone();
    let prior_id = admission["assessment_id"]
        .as_str()
        .ok_or_else(|| invalid_input("admission assessment id is absent"))?;
    let prior_ref = record_ref(prior_id, 1, admission)?;
    withdrawal["assessment_id"] = json!(format!("{prior_id}.withdrawal"));
    withdrawal["decision"] = json!("withdraw");
    withdrawal["supersedes"] = json!([prior_ref]);
    Ok(withdrawal)
}

fn describe_request(subject_id: &str) -> Value {
    json!({"schema_version":"tos_local_assessment_command_v1","operation":"describe","subject_id":subject_id})
}

fn record_ref(id: &str, version: u64, payload: &Value) -> io::Result<Value> {
    // Cargo feature unification may enable serde_json's insertion-order map.
    // Record references always use the declared source-command canonical profile.
    let raw = serde_json::to_vec(payload).map_err(invalid_data)?;
    let parsed = tos_foundation::parse_json(
        &raw,
        tos_foundation::JsonMode::PublishedStrict,
        tos_foundation::JsonLimits::default(),
    )
    .map_err(invalid_data)?;
    let canonical = tos_foundation::canonical_bytes_v1(
        parsed.root(),
        tos_foundation::CanonicalProfile::SourceCommandInputV1,
        tos_foundation::JsonLimits::default(),
    )
    .map_err(invalid_data)?;
    Ok(json!({"id":id,"version":version,"digest":Digest256::of_bytes(&canonical).to_prefixed()}))
}

fn envelope(id: &str, version: u64, payload: Value, origin_id: Value) -> Value {
    json!({"id":id,"version":version,"payload":payload,"origin_id":origin_id})
}

fn owner_context(public_root: &Path, private_root: &Path) -> Value {
    json!({"schema_version":"tos_owner_local_source_context_v1",
        "store_id":"sid-77777777777777777777777777777777",
        "public_root":public_root,"private_root":private_root,
        "private_prefix":"ToS/source-witnesses/owner-local/sid-77777777777777777777777777777777/"})
}

fn initialize_root(root: &Path) -> io::Result<()> {
    fs::create_dir_all(root)?;
    fs::set_permissions(root, fs::Permissions::from_mode(0o700))
}

fn secure_dir(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

fn write_private_json(path: &Path, value: &Value) -> io::Result<()> {
    let raw = serde_json::to_vec(value).map_err(invalid_data)?;
    fs::write(path, raw)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

fn write_public_bytes(path: &Path, raw: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, raw)
}

fn copy_contracts(repository: &Path, public_root: &Path) -> io::Result<()> {
    let source = repository.join("ToS/contracts");
    let target = public_root.join("ToS/contracts");
    copy_tree(&source, &target)
}

fn copy_tree(source: &Path, target: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    if metadata.file_type().is_symlink() {
        return Err(invalid_input("contract source contains a symlink"));
    }
    if metadata.is_dir() {
        fs::create_dir_all(target)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            copy_tree(&entry.path(), &target.join(entry.file_name()))?;
        }
    } else if metadata.is_file() {
        if metadata.len() > 8_388_608 {
            return Err(invalid_input("contract file exceeds fixture bound"));
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source, target)?;
    } else {
        return Err(invalid_input("contract tree contains a non-file entry"));
    }
    Ok(())
}

fn inventory(root: &Path) -> io::Result<Vec<Value>> {
    fn collect(dir: &Path, rows: &mut Vec<Value>, bytes: &mut u64) -> io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                return Err(invalid_input("fixture source tree contains a symlink"));
            }
            if metadata.is_dir() {
                collect(&path, rows, bytes)?;
            } else if metadata.is_file() {
                *bytes = bytes
                    .checked_add(metadata.len())
                    .ok_or_else(|| invalid_input("fixture byte overflow"))?;
                if rows.len() >= 2048 || *bytes > 33_554_432 {
                    return Err(invalid_input(
                        "fixture source tree exceeds count/byte bounds",
                    ));
                }
                let raw = fs::read(&path)?;
                rows.push(json!({"path":path,"bytes":raw.len(),"sha256":Digest256::of_bytes(&raw).to_hex()}));
            } else {
                return Err(invalid_input(
                    "fixture source tree contains a non-file entry",
                ));
            }
        }
        Ok(())
    }
    let mut rows = Vec::new();
    let mut bytes = 0u64;
    collect(root, &mut rows, &mut bytes)?;
    rows.sort_by(|left, right| left["path"].as_str().cmp(&right["path"].as_str()));
    Ok(rows)
}

fn invalid_input(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn invalid_data(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

fn foundation_value(value: &Value) -> io::Result<tos_foundation::JsonValue> {
    Ok(tos_foundation::parse_json(
        &serde_json::to_vec(value).map_err(invalid_data)?,
        tos_foundation::JsonMode::PublishedStrict,
        tos_foundation::JsonLimits::default(),
    )
    .map_err(invalid_data)?
    .into_root())
}

fn serde_value(value: &tos_foundation::JsonValue) -> io::Result<Value> {
    let raw =
        tos_foundation::emit_python_compact_json(value, tos_foundation::JsonLimits::default())
            .map_err(invalid_data)?;
    serde_json::from_slice(&raw).map_err(invalid_data)
}

fn native_record_ref(records: &[Value], record_id: &str) -> io::Result<Value> {
    let record = records
        .iter()
        .find(|record| record["id"].as_str() == Some(record_id))
        .ok_or_else(|| invalid_data("resolved native TextUnit envelope is absent"))?;
    let version = record["version"]
        .as_u64()
        .ok_or_else(|| invalid_data("resolved native TextUnit version is absent"))?;
    let payload = record
        .get("payload")
        .ok_or_else(|| invalid_data("resolved native TextUnit payload is absent"))?;
    record_ref(record_id, version, payload)
}
