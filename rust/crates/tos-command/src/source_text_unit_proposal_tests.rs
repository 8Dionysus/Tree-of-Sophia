use super::*;
use std::time::Duration;
fn id(kind: &str, n: u64) -> String {
    format!("tos.{kind}.sid-{n:032x}")
}
#[derive(Clone)]
struct Input {
    layer: Value,
    binding: Value,
    config: Value,
    request: Value,
    text: String,
    packet: Option<Value>,
}
impl Input {
    fn new() -> Self {
        let text = "P\r\ncafe\u{301} ZTAIL".to_owned();
        let digest = Digest256::of_bytes(text.as_bytes()).to_hex();
        let layer = json!({"schema_version":"tos_source_text_layer_v1","layer_id":id("text-layer",900),"layer_version":1,
   "source_binding":{"work_ref":"work","expression_ref":"expression","edition_ref":"edition","item_ref":"item","source_file_ref":"file","source_file_sha256":"a".repeat(64)},
   "representation":{"text_scope":{"start":3,"end":10,"position_unit":"unicode_code_point","interval":"half_open"},"content_sha256":digest,"content_file_id":format!("tos.file.sha256.{digest}"),"media_type":"text/plain","content_ref":"local/content.txt","language":"und","character_normalization":"none","content_visibility":"local_only","publication_authorized":false,"rights_record_refs":[{"ref":"rights.json"}]}});
        let binding = json!({"schema_version":"tos_native_text_layer_binding_v1","text_layer":{"layer_id":layer["layer_id"],"layer_version":1,"record_ref":"local/source-text-layer.v1.json","record_sha256":"b".repeat(64)}});
        let config = json!({"allowed_text_scope":{"start":3,"end":10},"packet_id":id("source-text-unit-packet",1),"scheme_id":id("text-unit-scheme",2),"segmentation_id":id("text-segmentation",3),"scope_anchor_ref":id("anchor",4),"provenance_event_id":"event.proposal","principal_id":"agent.proposal",
   "unit_slots":[{"unit_id":id("text-unit",5),"anchor_ref":id("anchor",6),"unit_kind":"surface_token"},{"unit_id":id("text-unit",7),"anchor_ref":id("anchor",8),"unit_kind":"surface_token"}],"gap_anchor_refs":[id("anchor",9),id("anchor",10),id("anchor",11)],
   "scheme":{"scheme_name":"Exact synthetic spans","analysis_role":"source_structure","boundary_basis":"manual","policies":{"normalization":"no-text-mutation-separate-successor-layer","unreported_gaps_allowed":false,"overlap":"forbid"}},
   "method":{"maker_kind":"software","agent_ref":"agent.proposal","provenance_event_ref":"event.proposal","output_posture":"method_result_not_source_or_linguistic_truth","locale":null}});
        let span = |n, start, end| json!({"unit_id":id("text-unit",n),"start":start,"end":end,"certainty":{"value":0.37,"meaning":CONFIDENCE_MEANING},"status_reason":"Synthetic selection, no semantic admission"});
        let request = json!({"spans":[span(5,3,8),span(7,9,10)],"excluded_gaps":[{"anchor_ref":id("anchor",9),"start":8,"end":9}]});
        Self {
            layer,
            binding,
            config,
            request,
            text,
            packet: None,
        }
    }
    fn build(&self) -> SourceCommandResult<Value> {
        let f = |v: &Value| cmd::parse(&serde_json::to_vec(v).unwrap()).unwrap();
        let packet = self.packet.as_ref().map(f);
        let (out, raw) = build_text_unit_packet(
            packet.as_ref(),
            &f(&self.layer),
            &f(&self.binding),
            &self.text,
            &f(&self.config),
            &f(&self.request),
            Instant::now() + Duration::from_secs(5),
            &AtomicBool::new(false),
        )?;
        assert_eq!(raw.last(), Some(&b'\n'));
        assert_eq!(raw.len(), cmd::canonical(&out)?.len() + 1);
        Ok(serde_json::from_slice(&raw).unwrap())
    }
    fn with_predecessor(mut self) -> Self {
        let mut old = self.clone();
        old.config["packet_id"] = json!(id("source-text-unit-packet", 101));
        old.config["scheme_id"] = json!(id("text-unit-scheme", 102));
        old.config["segmentation_id"] = json!(id("text-segmentation", 103));
        old.config["scope_anchor_ref"] = json!(id("anchor", 104));
        old.config["unit_slots"] = json!([{"unit_id":id("text-unit",105),"anchor_ref":id("anchor",106),"unit_kind":"surface_token"}]);
        old.config["gap_anchor_refs"] = json!([]);
        old.request["spans"] = json!([{"unit_id":id("text-unit",105),"start":3,"end":10,"certainty":{"value":0.5,"meaning":CONFIDENCE_MEANING},"status_reason":"predecessor"}]);
        old.request["excluded_gaps"] = json!([]);
        let packet = old.build().unwrap();
        self.binding["schema_version"] = json!("tos_native_text_unit_binding_v1");
        self.binding["packet_ref"] = json!("local/source-text-unit.v1.json");
        self.binding["packet_sha256"] = json!("c".repeat(64));
        self.binding["packet_id"] = packet["packet_id"].clone();
        self.binding["packet_version"] = json!(1);
        self.binding["unit_id"] = json!(id("text-unit", 105));
        self.packet = Some(packet);
        self
    }
}
#[test]
fn exact_nonzero_unicode_spans_gaps_and_proposal_authority() {
    for input in [Input::new(), Input::new().with_predecessor()] {
        let packet = input.build().unwrap();
        assert_eq!(packet, input.build().unwrap());
        assert_eq!(packet["anchors"][1]["selector"]["start"], 3);
        assert_eq!(
            packet["anchors"][1]["exact_sha256"],
            Digest256::of_bytes("cafe\u{301}".as_bytes()).to_hex()
        );
        assert_eq!(
            packet["anchors"][2]["exact_sha256"],
            Digest256::of_bytes(b" ").to_hex()
        );
        assert_ne!(
            packet["anchors"][0]["exact_sha256"],
            Digest256::of_bytes(input.text.as_bytes()).to_hex()
        );
        assert_eq!(packet["segmentations"][0]["status"], "proposed");
        assert_eq!(packet["segmentations"][0]["review_refs"], json!([]));
        assert_eq!(packet["segmentations"][0]["source_text_authority"], false);
        assert_eq!(
            packet["rights_and_visibility"]["publication_authorized"],
            false
        );
    }
    let mut input = Input::new();
    input.request["spans"][0]["end"] = json!(9);
    input.request["excluded_gaps"] = json!([]);
    let packet = input.build().unwrap();
    assert_eq!(
        packet["segmentations"][0]["coverage"]["coverage_posture"],
        "exhaustive_nonoverlapping"
    );
}
#[test]
fn partition_and_boundaries_refuse_hidden_gaps_repair_and_unselected_spans() {
    let original = Input::new();
    for (pointer, value) in [
        ("/spans/0/end", json!(9)),
        ("/spans/0/start", json!(3.0)),
        ("/spans/0/start", json!(true)),
        ("/spans/0/start", json!(2)),
        ("/spans/0/end", json!(3)),
        ("/spans/1/end", json!(11)),
        ("/spans/1/start", json!(7)),
        ("/spans/0/certainty/value", json!(-0.1)),
        ("/spans/0/certainty/value", json!(1.1)),
        ("/spans/0/certainty/meaning", json!("truth-probability")),
        ("/excluded_gaps", json!([])),
        ("/spans/0/unit_id", json!(id("text-unit", 999))),
    ] {
        let mut input = original.clone();
        *input.request.pointer_mut(pointer).unwrap() = value;
        assert!(input.build().is_err(), "{pointer}");
    }
    for text in [
        original.text.replace("\r\n", "\n"),
        original.text.replace("e\u{301}", "é"),
        "cafe\u{301} Z".into(),
    ] {
        let mut input = original.clone();
        input.text = text;
        assert!(input.build().is_err());
    }
    for (pointer, value) in [
        ("/allowed_text_scope/start", json!(2)),
        ("/allowed_text_scope/end", json!(11)),
        ("/unit_slots/1/unit_id", json!(id("text-unit", 5))),
        ("/unit_slots/1/anchor_ref", json!(id("anchor", 6))),
        ("/unit_slots/0/unit_kind", json!("empty_analytic_node")),
        ("/scheme/policies/normalization", json!("NFC")),
        ("/method/output_posture", json!("source_truth")),
    ] {
        let mut input = original.clone();
        *input.config.pointer_mut(pointer).unwrap() = value;
        assert!(input.build().is_err(), "{pointer}");
    }
}
#[test]
fn stronger_packet_restriction_and_source_language_are_not_relabeled() {
    for visibility in ["restricted", "unknown"] {
        let mut input = Input::new().with_predecessor();
        let rights = &mut input.packet.as_mut().unwrap()["rights_and_visibility"];
        rights["packet_visibility"] = json!(visibility);
        rights["effective_visibility"] = json!(visibility);
        assert!(input.build().is_err());
        let mut input = Input::new();
        input.layer["representation"]["content_visibility"] = json!(visibility);
        assert_eq!(
            input.build().unwrap()["rights_and_visibility"]["effective_visibility"],
            visibility
        );
    }
    let mut input = Input::new();
    input.config["method"]["locale"] = json!("fr");
    assert!(input.build().is_err());
}

#[test]
fn normalization_uses_frozen_scope_and_maximum_partition_is_bounded() {
    let mut input = Input::new();
    input.text = "e\u{301}\r\nAB CD".into();
    let digest = Digest256::of_bytes(input.text.as_bytes()).to_hex();
    input.layer["representation"]["content_sha256"] = json!(digest);
    input.layer["representation"]["content_file_id"] = json!(format!("tos.file.sha256.{digest}"));
    input.layer["representation"]["text_scope"]["start"] = json!(4);
    input.layer["representation"]["text_scope"]["end"] = json!(9);
    input.layer["representation"]["character_normalization"] = json!("NFC");
    input.config["allowed_text_scope"] = json!({"start":4,"end":9});
    input.request["spans"][0]["start"] = json!(4);
    input.request["spans"][0]["end"] = json!(6);
    input.request["spans"][1]["start"] = json!(7);
    input.request["spans"][1]["end"] = json!(9);
    input.request["excluded_gaps"][0]["start"] = json!(6);
    input.request["excluded_gaps"][0]["end"] = json!(7);
    assert_eq!(
        input.build().unwrap()["source_layer"]["unicode_form"],
        "NFC"
    );
    input.layer["representation"]["text_scope"]["start"] = json!(0);
    assert!(
        input.build().is_err(),
        "decomposed prefix now belongs to the NFC scope"
    );

    let mut input = Input::new();
    input.text = "a".repeat(256);
    let digest = Digest256::of_bytes(input.text.as_bytes()).to_hex();
    input.layer["representation"]["content_sha256"] = json!(digest);
    input.layer["representation"]["content_file_id"] = json!(format!("tos.file.sha256.{digest}"));
    input.layer["representation"]["text_scope"]["start"] = json!(0);
    input.layer["representation"]["text_scope"]["end"] = json!(256);
    input.config["allowed_text_scope"] = json!({"start":0,"end":256});
    input.config["unit_slots"] = json!((0..256).map(|n|json!({"unit_id":id("text-unit",1000+n),"anchor_ref":id("anchor",2000+n),"unit_kind":"surface_token"})).collect::<Vec<_>>());
    input.request["spans"] = json!((0..256).map(|n|json!({"unit_id":id("text-unit",1000+n),"start":n,"end":n+1,"certainty":{"value":0.5,"meaning":CONFIDENCE_MEANING},"status_reason":"bounded fixture"})).collect::<Vec<_>>());
    input.config["gap_anchor_refs"] = json!([]);
    input.request["excluded_gaps"] = json!([]);
    let packet = input.build().unwrap();
    assert_eq!(packet["units"].as_array().unwrap().len(), 256);
    assert_eq!(packet["anchors"].as_array().unwrap().len(), 257);
    input.config["unit_slots"].as_array_mut().unwrap().push(json!({"unit_id":id("text-unit",1256),"anchor_ref":id("anchor",2256),"unit_kind":"surface_token"}));
    assert!(input.build().is_err());
}
