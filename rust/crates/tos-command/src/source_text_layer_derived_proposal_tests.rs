use super::*;
use std::time::Duration;

#[derive(Clone)]
struct Input {
    config: Value,
    predecessor: Value,
    binding: Value,
    text: String,
}
impl Input {
    fn new(text: &str, form: &str) -> Self {
        let binding = json!({
            "work_ref":"tos.work.synthetic",
            "expression_ref":"tos.expression.synthetic",
            "edition_ref":"tos.edition.synthetic",
            "item_ref":"tos.item.synthetic",
            "source_file_ref":"tos.file.synthetic-source",
            "source_file_sha256":"c".repeat(64),
            "anchor_contract":"tos_source_anchor_v2",
            "anchors":[{"anchor_id":"tos.anchor.synthetic",
                "anchor_record_ref":"local/anchor.json",
                "anchor_record_sha256":"d".repeat(64)}]
        });
        let predecessor = json!({"layer_id":"tos.text-layer.predecessor","layer_version":1,"layer_role":"machine_transcription","source_binding":binding,
            "representation":{"content_sha256":Digest256::of_bytes(text.as_bytes()).to_hex(),"text_scope":span(0,text.chars().count()),"character_normalization":"none","line_break_posture":"source_preserved"},
            "uncertainty":{"status":"none","annotations":[]},
            "admission":{"review_status":"accepted","accepted_uses":["citation"],"promotion_authorized":true}});
        let config = json!({"allowed_operations":["text-layer.normalize"],"source_path":"local/successor/source-text-layer.v1.json",
            "policy":policy("text-layer.normalize",form,"manual_transcription").unwrap(),"input":{"binding":{"text_layer":{"layer_id":predecessor["layer_id"],"layer_version":1,"record_ref":"local/predecessor/source-text-layer.v1.json","record_sha256":"a".repeat(64)}}},
            "identities":{"layer_id":"tos.text-layer.successor","provenance_event_id":"tos.event.successor"},
            "maker":{"maker_type":"software","agent_ref":"agent.synthetic","method":"owner-proposed",
                "version":"1","configuration_digest":"e".repeat(64)},"language":"en","material":{},
            "limits":{"max_output_bytes":MAX_TEXT},"derivation_access":{"rights_record_refs":[{"ref":"local/rights.json","sha256":"b".repeat(64)}]}});
        Self {
            config,
            predecessor,
            binding,
            text: text.to_owned(),
        }
    }
    fn editorial(text: &str, edits: Vec<Value>) -> Self {
        let mut input = Self::new(text, "NFC");
        input.config["allowed_operations"] = json!(["text-layer.normalize-editorial"]);
        input.config["policy"] = policy(
            "text-layer.normalize-editorial",
            "editorial",
            "manual_transcription",
        )
        .unwrap();
        input.config["material"] = json!({"edits":edits});
        input
    }
    fn run(&self) -> SourceCommandResult<DerivedLayerOutput> {
        let convert = |v: &Value| cmd::parse(&serde_json::to_vec(v).unwrap()).unwrap();
        build_derived_layer(
            &convert(&self.config),
            &convert(&self.binding),
            Some(&convert(&self.predecessor)),
            Some(&self.text),
            None,
            Instant::now() + Duration::from_secs(5),
            &AtomicBool::new(false),
        )
    }
}
fn edit(text: &str, start: usize, end: usize, output: &str, reason: &str) -> Value {
    let input_exact = text
        .chars()
        .skip(start)
        .take(end - start)
        .collect::<String>();
    json!({"start":start,"end":end,"input_exact":input_exact,
        "input_sha256":Digest256::of_bytes(input_exact.as_bytes()).to_hex(),
        "output_exact":output,"reason":reason,"confidence":0.9})
}
#[test]
fn additive_unicode_forms_keep_exact_edits_rights_uncertainty_and_unreviewed_successor() {
    for (form, before, after) in [
        ("NFC", "e\u{301}", "é"),
        ("NFD", "é", "e\u{301}"),
        ("NFKC", "ﬁ ①", "fi 1"),
        ("NFKD", "é ①", "e\u{301} 1"),
        ("NFC", "already\r\nnormalized", "already\r\nnormalized"),
    ] {
        let input = Input::new(before, form);
        let saved = input.predecessor.clone();
        let output = input.run().unwrap();
        assert_eq!(output.files["content.txt"], after.as_bytes());
        let layer = serde(&output.layer).unwrap();
        let edit = &layer["derivation"]["change_payload"]["operations"][0];
        assert_eq!(edit["input_exact"], before);
        assert_eq!(edit["output_exact"], after);
        assert_eq!(edit["status"], "proposed");
        assert_eq!(
            edit["input_sha256"],
            Digest256::of_bytes(before.as_bytes()).to_hex()
        );
        assert_eq!(
            edit["output_sha256"],
            Digest256::of_bytes(after.as_bytes()).to_hex()
        );
        assert_eq!(layer["layer_version"], 2);
        assert_eq!(layer["supersedes_layer_ref"], saved["layer_id"]);
        assert_eq!(
            layer["representation"]["rights_record_refs"],
            input.config["derivation_access"]["rights_record_refs"]
        );
        assert_eq!(layer["uncertainty"], saved["uncertainty"]);
        assert_eq!(layer["admission"]["accepted_uses"], json!([]));
        assert_eq!(layer["admission"]["review_status"], "unreviewed");
        assert_eq!(layer["admission"]["promotion_authorized"], false);
        assert_eq!(layer["admission"]["human_review_performed"], false);
        assert_eq!(layer["representation"]["publication_authorized"], false);
        assert_eq!(
            output.files["source-text-layer.v1.json"].last(),
            Some(&b'\n')
        );
        assert_eq!(input.predecessor, saved);
    }
}

#[test]
fn editorial_normalization_uses_only_digest_bound_ordered_edits_and_preserves_private_successor_lineage()
 {
    let text = "the Old  text";
    let input = Input::editorial(
        text,
        vec![
            edit(
                text,
                4,
                7,
                "new",
                "Modern spelling selected for editorial reading",
            ),
            edit(
                text,
                9,
                13,
                "book",
                "Editorial wording selected for this reading",
            ),
        ],
    );
    let predecessor = input.predecessor.clone();
    let output = input.run().unwrap();
    assert_eq!(output.files["content.txt"], b"the new  book");

    let schema_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../ToS/contracts/source-text-layer.schema.json");
    let schema_raw = std::fs::read(schema_path).unwrap();
    let mut schema_engine = tos_validation::in_process::InProcessSchemaEngine::new(
        [tos_validation::SchemaResource {
            uri: LAYER_SCHEMA.to_owned(),
            raw: schema_raw,
        }],
        tos_validation::FormatProfile::AssertedSourceCandidateV1,
    )
    .unwrap();
    schema_engine.prepare(LAYER_SCHEMA).unwrap();
    let schema_report = schema_engine
        .validate(
            tos_validation::in_process::Unit {
                root_uri: LAYER_SCHEMA,
                raw: &output.files["source-text-layer.v1.json"],
                input_mode: tos_validation::in_process::InputMode::PublishedStrict,
            },
            tos_validation::executor::schema_diagnostics::Caps::CURRENT,
            Instant::now() + Duration::from_secs(5),
            &AtomicBool::new(false),
        )
        .unwrap();
    assert_eq!(
        schema_report.status,
        tos_validation::executor::schema_diagnostics::Status::Valid,
        "{schema_report:?}"
    );

    let layer = serde(&output.layer).unwrap();
    assert_eq!(layer["layer_role"], "normalized_text");
    assert_eq!(
        layer["representation"]["character_normalization"],
        "editorial"
    );
    assert_eq!(layer["derivation"]["method"], "editorial_normalization");
    assert_eq!(
        layer["derivation"]["preservation_goal"],
        "editorial_reading"
    );
    assert_eq!(layer["layer_version"], 2);
    assert_eq!(layer["supersedes_layer_ref"], predecessor["layer_id"]);
    assert_eq!(
        layer["derivation"]["input_layers"][0]["layer_id"],
        predecessor["layer_id"]
    );
    assert_eq!(
        layer["representation"]["rights_record_refs"],
        input.config["derivation_access"]["rights_record_refs"]
    );
    assert_eq!(layer["uncertainty"], predecessor["uncertainty"]);
    assert_eq!(layer["representation"]["content_visibility"], "local_only");
    assert_eq!(layer["representation"]["storage"], "ignored_local");
    assert_eq!(layer["representation"]["tracked_content"], false);
    assert_eq!(layer["representation"]["publication_authorized"], false);
    assert_eq!(layer["admission"]["review_status"], "unreviewed");
    assert_eq!(layer["admission"]["human_review_performed"], false);
    assert_eq!(layer["admission"]["accepted_uses"], json!([]));
    assert_eq!(layer["admission"]["promotion_authorized"], false);
    assert_eq!(input.predecessor, predecessor);

    let policy: Value = serde_json::from_slice(&output.files["derivation-policy.json"]).unwrap();
    assert_eq!(policy["operation"], "text-layer.normalize-editorial");
    assert_eq!(policy["method"], "editorial_normalization");
    assert_eq!(policy["edits_max_count"], MAX_EDITORIAL_EDITS);
    assert_eq!(
        policy["editorial_normalization_policy"]["schema_version"],
        "tos_native_editorial_normalization_policy_v1"
    );
    assert_eq!(
        policy["editorial_normalization_policy"]["policy_version"],
        1
    );
    assert_eq!(
        policy["editorial_normalization_policy"]["implicit_changes_allowed"],
        false
    );
    assert_eq!(
        policy["editorial_normalization_policy"]["preserve_unmentioned_input"],
        true
    );
    assert_eq!(
        layer["editorial_policy"]["policy_sha256"],
        Digest256::of_bytes(&output.files["derivation-policy.json"]).to_hex()
    );

    let operations = layer["derivation"]["change_payload"]["operations"]
        .as_array()
        .unwrap();
    assert_eq!(operations.len(), 2);
    for ((operation, original), replacement) in
        operations.iter().zip(["Old", "text"]).zip(["new", "book"])
    {
        assert_eq!(operation["operation"], "replace");
        assert_eq!(operation["input_exact"], original);
        assert_eq!(
            operation["input_sha256"],
            Digest256::of_bytes(original.as_bytes()).to_hex()
        );
        assert_eq!(operation["output_exact"], replacement);
        assert_eq!(operation["status"], "proposed");
    }
}

#[test]
fn editorial_normalization_rejects_bad_digest_order_normalized_predecessor_and_excess_edits() {
    let text = "old and older";
    let mut invalid_digest = Input::editorial(text, vec![edit(text, 0, 3, "new", "Explicit edit")]);
    invalid_digest.config["material"]["edits"][0]["input_sha256"] = json!("0".repeat(64));
    assert!(invalid_digest.run().is_err());

    let unordered = Input::editorial(
        text,
        vec![
            edit(text, 8, 13, "newer", "Later span first"),
            edit(text, 0, 3, "new", "Earlier span second"),
        ],
    );
    assert!(unordered.run().is_err());

    let mut normalized = Input::editorial(text, vec![edit(text, 0, 3, "new", "Explicit edit")]);
    normalized.predecessor["layer_role"] = json!("normalized_text");
    normalized.predecessor["representation"]["character_normalization"] = json!("editorial");
    assert!(matches!(
        normalized.run(),
        Err(SourceCommandError::Conflict(
            "native TextLayer predecessor differs"
        ))
    ));

    let text = "a ".repeat(MAX_EDITORIAL_EDITS);
    let edits = (0..MAX_EDITORIAL_EDITS)
        .map(|index| {
            edit(
                &text,
                index * 2,
                index * 2 + 1,
                "b",
                "Bounded explicit edit",
            )
        })
        .collect();
    assert!(Input::editorial(&text, edits).run().is_ok());

    let text = "a ".repeat(MAX_EDITORIAL_EDITS + 1);
    let edits = (0..MAX_EDITORIAL_EDITS + 1)
        .map(|index| {
            edit(
                &text,
                index * 2,
                index * 2 + 1,
                "b",
                "Over budget explicit edit",
            )
        })
        .collect();
    assert!(Input::editorial(&text, edits).run().is_err());
}
#[test]
fn derivation_refuses_rebinding_normalization_erasure_and_expansion_before_publication() {
    let base = Input::new("e\u{301}", "NFC");
    for field in ["scope", "version", "identity", "policy", "digest"] {
        let mut input = base.clone();
        match field {
            "scope" => input.predecessor["representation"]["text_scope"]["start"] = json!(1),
            "version" => input.config["input"]["binding"]["text_layer"]["layer_version"] = json!(2),
            "identity" => {
                input.config["identities"]["layer_id"] = input.predecessor["layer_id"].clone()
            }
            "policy" => input.config["policy"]["unicode_database_version"] = json!("unsupported"),
            "digest" => {
                input.predecessor["representation"]["content_sha256"] = json!("0".repeat(64))
            }
            _ => unreachable!(),
        }
        assert!(input.run().is_err(), "{field}");
    }
    let mut input = base.clone();
    input.predecessor["layer_role"] = json!("normalized_text");
    input.predecessor["representation"]["character_normalization"] = json!("NFC");
    input.config["allowed_operations"] = json!(["text-layer.correct"]);
    input.config["policy"] = policy("text-layer.correct", "none", "manual_transcription").unwrap();
    input.config["material"]["edits"] = json!([]);
    assert!(matches!(
        input.run(),
        Err(SourceCommandError::Conflict(
            "native TextLayer predecessor differs"
        ))
    ));
    assert!(Input::new(&"x".repeat(MAX_TEXT + 1), "NFC").run().is_err());
    // U+FDFA is only three UTF-8 bytes but its compatibility decomposition
    // is much larger; the output cap must cover expansion as it is produced.
    assert!(Input::new(&"\u{fdfa}".repeat(6000), "NFKD").run().is_err());
    let mut input = base.clone();
    input.config["allowed_operations"] = json!(["text-layer.correct"]);
    input.config["policy"] = policy("text-layer.correct", "none", "manual_transcription").unwrap();
    input.config["material"]["edits"] = json!(vec![json!({}); MAX_EDITS + 1]);
    assert!(input.run().is_err());
    let input = Input::new("é", "NFD");
    let convert = |v: &Value| cmd::parse(&serde_json::to_vec(v).unwrap()).unwrap();
    assert!(
        build_derived_layer(
            &convert(&input.config),
            &convert(&input.binding),
            Some(&convert(&input.predecessor)),
            Some(&input.text),
            None,
            Instant::now() + Duration::from_secs(5),
            &AtomicBool::new(true)
        )
        .is_err()
    );
}
