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

fn build_public_v3(repository: &Path, root: &Path) -> io::Result<NativePublicAssessmentFixture> {
    initialize_root(root)?;
    let deadline = Instant::now() + Duration::from_secs(240);
    let cancelled = AtomicBool::new(false);
    let seed =
        super::command_text_cases::native_text_unit_seed(repository, root, deadline, &cancelled);
    copy_contracts(repository, &seed.public_root)?;

    let packet_path = seed.private_root.join(&seed.unit_source_ref);
    let packet_bytes = fs::read(&packet_path)?;
    let packet: Value = serde_json::from_slice(&packet_bytes).map_err(invalid_data)?;
    let unit = packet["units"]
        .as_array()
        .and_then(|rows| rows.first())
        .ok_or_else(|| invalid_data("native TextUnit packet has no unit"))?;
    let segmentation = packet["segmentations"]
        .as_array()
        .and_then(|rows| rows.first())
        .ok_or_else(|| invalid_data("native TextUnit packet has no segmentation"))?;
    let layer_binding = seed.unit_owner_config["source_binding"]["text_layer"].clone();
    let source_record_refs = seed.unit_owner_config["source_binding"]["source_record_refs"].clone();
    let binding = json!({
        "schema_version":"tos_native_text_unit_binding_v1",
        "packet_ref":seed.unit_source_ref,
        "packet_sha256":Digest256::of_bytes(&packet_bytes).to_hex(),
        "packet_id":packet["packet_id"],
        "packet_version":packet["packet_version"],
        "unit_id":unit["unit_id"],
        "unit_version":unit["unit_version"],
        "ordered_anchor_refs":unit["ordered_anchor_refs"],
        "segmentation_id":segmentation["segmentation_id"],
        "segmentation_version":segmentation["segmentation_version"],
        "text_layer":layer_binding,
        "source_record_refs":source_record_refs
    });
    let subject_id = binding["unit_id"]
        .as_str()
        .ok_or_else(|| invalid_data("native TextUnit binding has no identity"))?
        .to_owned();
    let access = json!({
        "read_scope":"exact_owner_local",
        "access_allowed":true,
        "authority_ref":"test:synthetic-native-text-source-access"
    });
    let native_selection_value = json!([{
        "binding":binding.clone(),
        "origin_id":"synthetic-native-text-assessment-origin",
        "source_access":access.clone()
    }]);
    let native_selection = foundation_value(&native_selection_value)?;
    let worker = seed.worker_image.identity();
    let exact = tos_command::resolve_native_text_units_for_conformance(
        &seed.owner_context,
        &native_selection,
        &seed.cut,
        &worker.absolute_path,
        worker.sha256,
        deadline,
        &cancelled,
    )
    .map_err(invalid_data)?;
    let metadata_selection = json!([{
        "binding":binding.clone(),
        "origin_id":"synthetic-native-text-assessment-origin",
        "source_access":{
            "read_scope":"metadata_only",
            "access_allowed":true,
            "authority_ref":"test:synthetic-native-text-source-access"
        }
    }]);
    let metadata_selection = foundation_value(&metadata_selection)?;
    let metadata = tos_command::resolve_native_text_units_for_conformance(
        &seed.owner_context,
        &metadata_selection,
        &seed.cut,
        &worker.absolute_path,
        worker.sha256,
        deadline,
        &cancelled,
    )
    .map_err(invalid_data)?;
    let native_records = exact
        .native_records
        .iter()
        .map(serde_value)
        .collect::<io::Result<Vec<_>>>()?;
    let native_summaries = exact
        .native_summaries
        .iter()
        .map(serde_value)
        .collect::<io::Result<Vec<_>>>()?;
    let metadata_records = metadata
        .native_records
        .iter()
        .map(serde_value)
        .collect::<io::Result<Vec<_>>>()?;
    let metadata_summary = metadata
        .native_summaries
        .first()
        .ok_or_else(|| invalid_data("metadata-only native summary is absent"))?;
    let subject_ref = native_record_ref(&native_records, &subject_id)?;
    if summary_record_ref(&native_summaries, &subject_id)? != subject_ref {
        return Err(invalid_data(
            "exact TextUnit native record ref differs from resolver summary",
        ));
    }
    let metadata_subject = native_record_ref(&metadata_records, &subject_id)?;
    if summary_record_ref(
        &metadata
            .native_summaries
            .iter()
            .map(serde_value)
            .collect::<io::Result<Vec<_>>>()?,
        &subject_id,
    )? != metadata_subject
    {
        return Err(invalid_data(
            "metadata-only TextUnit native record ref differs from resolver summary",
        ));
    }
    let layer_id = binding["text_layer"]["layer_id"]
        .as_str()
        .ok_or_else(|| invalid_data("native TextUnit binding has no layer identity"))?;
    let layer_ref = summary_record_ref(&native_summaries, layer_id)?;
    let access_language = metadata_summary
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
        "languages":["ru","und"],"profile_ids":["source-observation","interpretation","identity","high-consequence"],
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
        "assertion_layers":["textual_observation","linguistic_analysis"],"languages":["ru","und"],
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
        "interpretation",
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
        "journal_directory":root.join("assessment-journal"),"source_root":seed.public_root,
        "source_records":[],"native_text_units":native_selection_value,
        "owner_local_source_records":[],"subjects":subjects
    });
    let journal_directory = root.join("assessment-journal");
    secure_dir(&journal_directory)?;
    let owner_config_path = root.join("owner-v3.json");
    write_private_json(&owner_config_path, &owner_config)?;
    let owner_context: Value =
        serde_json::from_slice(&fs::read(&seed.owner_context)?).map_err(invalid_data)?;
    let public_root = seed.public_root.clone();
    let owner_context_path = seed.owner_context.clone();
    let preserved = inventory(&public_root)?;
    let private_inventory = inventory(&seed.private_root)?;
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
    // serde_json's default map is key-sorted; compact encoding matches Record's
    // canonical UTF-8 JSON before SHA-256.
    let canonical = serde_json::to_vec(payload).map_err(invalid_data)?;
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
                rows.push(json!({"path":path,"sha256":Digest256::of_bytes(&raw).to_hex()}));
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

fn summary_record_ref(summaries: &[Value], record_id: &str) -> io::Result<Value> {
    summaries
        .iter()
        .flat_map(|summary| summary["record_refs"].as_array().into_iter().flatten())
        .find(|reference| reference["id"].as_str() == Some(record_id))
        .cloned()
        .ok_or_else(|| invalid_data("resolved native record reference is absent"))
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
