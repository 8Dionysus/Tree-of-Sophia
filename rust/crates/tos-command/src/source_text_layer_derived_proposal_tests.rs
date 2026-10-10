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
        let binding = json!({"anchors":[{"anchor_id":"tos.anchor.synthetic"}],"work_ref":"tos.work.synthetic"});
        let predecessor = json!({"layer_id":"tos.text-layer.predecessor","layer_version":1,"layer_role":"machine_transcription","source_binding":binding,
            "representation":{"content_sha256":Digest256::of_bytes(text.as_bytes()).to_hex(),"text_scope":span(0,text.chars().count()),"character_normalization":"none","line_break_posture":"source_preserved"},
            "uncertainty":{"status":"recorded","annotations":[{"description":"Source uncertainty must remain"}]},
            "admission":{"review_status":"accepted","accepted_uses":["citation"],"promotion_authorized":true}});
        let config = json!({"allowed_operations":["text-layer.normalize"],"source_path":"local/successor/source-text-layer.v1.json",
            "policy":policy("text-layer.normalize",form,"manual_transcription").unwrap(),"input":{"binding":{"text_layer":{"layer_id":predecessor["layer_id"],"layer_version":1,"record_ref":"local/predecessor/source-text-layer.v1.json","record_sha256":"a".repeat(64)}}},
            "identities":{"layer_id":"tos.text-layer.successor","provenance_event_id":"event.successor"},
            "maker":{"maker_kind":"software","agent_ref":"agent.synthetic"},"language":"en","material":{},
            "limits":{"max_output_bytes":MAX_TEXT},"derivation_access":{"rights_record_refs":[{"ref":"local/rights.json","sha256":"b".repeat(64)}]}});
        Self {
            config,
            predecessor,
            binding,
            text: text.to_owned(),
        }
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
