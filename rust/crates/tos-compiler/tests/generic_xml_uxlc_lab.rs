//! Native maintained regression consumer of the completed GenericXML A/B/C lab.
//! Historical experiment programs/receipts are data, never executed here.
#[path = "generic_xml_uxlc_lab/inputs.rs"]
mod inputs;
#[path = "generic_xml_uxlc_lab/kernel.rs"]
mod kernel;
use kernel::*;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

const MANIFEST_RAW: &str = include_str!(
    "../../../../ToS/research-packets/foundation-laboratory-2026-07/generic-xml-resource-inventory-uxlc-abc-v1/input-manifest.json"
);
const FREEZE_RAW: &str = include_str!(
    "../../../../ToS/research-packets/foundation-laboratory-2026-07/generic-xml-resource-inventory-uxlc-abc-v1/freeze-receipt-v16.json"
);
fn manifest() -> Value {
    serde_json::from_str(MANIFEST_RAW).unwrap()
}
fn fixture(id: &str) -> Vec<u8> {
    manifest()["fixtures"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == id)
        .unwrap()["xml"]
        .as_str()
        .unwrap()
        .as_bytes()
        .to_vec()
}
fn payload(kind: &str, id: &str) -> Value {
    serde_json::from_str(
        inputs::PUBLIC
            .iter()
            .find(|(k, i, _)| *k == kind && *i == id)
            .unwrap()
            .2,
    )
    .unwrap()
}
fn xml(id: &str) -> Xml {
    parse(&fixture(id), 8 * 1024 * 1024, 100_000, 256).unwrap()
}
fn rejects(result: Result<Value>, message: &str) {
    assert!(result.unwrap_err().contains(message), "expected {message}");
}

#[test]
fn whole_public_candidate_source_returns_and_selection_refusals() {
    let manifest = manifest();
    // Every retained public successful observation is consumed against its
    // original fixture; duplicate run observations do not create new authority.
    for (kind, id, raw) in inputs::PUBLIC {
        let payload: Value = serde_json::from_str(raw).unwrap();
        let schema = payload["schema_version"].as_str().unwrap();
        let lab = manifest["lab_id"].as_str().unwrap();
        let source = fixture(id);
        candidate(
            &payload,
            &source,
            Selection {
                candidate: Some(kind),
                input: Some(id),
                lab: Some(lab),
                schema: Some(schema),
                provider: None,
            },
        )
        .unwrap_or_else(|error| panic!("{kind}:{id}: {error}"));
    }
    let raw = fixture("P1-no-namespace");
    let value = payload("A", "P1-no-namespace");
    for (selection, error) in [
        (
            Selection {
                candidate: Some("B"),
                ..Default::default()
            },
            "requested candidate",
        ),
        (
            Selection {
                input: Some("wrong"),
                ..Default::default()
            },
            "input ID mismatch",
        ),
        (
            Selection {
                lab: Some("wrong"),
                ..Default::default()
            },
            "lab ID mismatch",
        ),
        (
            Selection {
                schema: Some("wrong"),
                ..Default::default()
            },
            "schema version mismatch",
        ),
    ] {
        rejects(candidate(&value, &raw, selection), error);
    }
    let mut value = value;
    value["resources"][0]["resource_kind"] = "xml_element".into();
    rejects(
        candidate(&value, &raw, Selection::default()),
        "A resource shape mismatch",
    );
    let mut value = payload("A", "P1-no-namespace");
    let extra = value["resources"][0].clone();
    value["resources"].as_array_mut().unwrap().push(extra);
    rejects(
        candidate(&value, &raw, Selection::default()),
        "A resource shape mismatch",
    );
    let mut value = payload("A", "P1-no-namespace");
    value["file_binding"]["sha256"] = "wrong".into();
    rejects(
        candidate(&value, &raw, Selection::default()),
        "file binding mismatch",
    );
}

#[test]
fn b_exact_topology_metadata_scope_and_complete_summary() {
    let id = "P4-duplicate-siblings";
    let source = fixture(id);
    let root = xml(id);
    let original = payload("B", id);
    assert_eq!(b(&original, &root).unwrap()["summary"], root.summary());
    let mut value = original.clone();
    value["resources"][1]["resource_id"] = value["resources"][0]["resource_id"].clone();
    rejects(b(&value, &root), "resource IDs must be unique");
    let mut value = original.clone();
    value["resources"][0]["resource_kind"] = "provider_word".into();
    rejects(b(&value, &root), "resource kind mismatch");
    for field in [
        "resource_count",
        "attribute_count",
        "max_depth",
        "namespace_uris",
        "ordered_topology_sha256",
        "unordered_element_shape_sha256",
    ] {
        let mut value = original.clone();
        value["summary"][field] = Value::Null;
        rejects(b(&value, &root), "B summary mismatch");
    }
    let mut value = original.clone();
    value["summary"]["unregistered"] = 1.into();
    rejects(b(&value, &root), "B summary shape mismatch");
    let mut value = original.clone();
    value["scope"]["excluded"] = json!(["nothing"]);
    rejects(b(&value, &root), "B scope value mismatch");
    let mut value = original.clone();
    value["resources"][1]["locator"]["path"][1]["same_name_sibling_position"] = 999.into();
    assert!(b(&value, &root).is_err());
    let mut value = original;
    value["resources"][1]["locator"]["preorder"] = 1.into();
    rejects(
        candidate(&value, &source, Selection::default()),
        "preorder mismatch",
    );
    let id = "P2-prefix-a";
    let value = payload("B", id);
    assert_eq!(value["summary"], xml(id).summary());
    // Prefix aliases and attribute values are excluded from structural identity.
    assert_eq!(xml("P2-prefix-a").summary(), xml("P2-prefix-b").summary());
}

#[test]
fn provider_complete_unique_membership_coordinates_and_composed_owner() {
    let id = "PC1-uxlc-shape";
    let root = xml(id);
    let source = fixture(id);
    let original = payload("C", id);
    let registered = original["provider_context"].clone();
    let result = projection(&original, &root, None, Some(&registered)).unwrap();
    assert_eq!(result["expected_resource_count"], 7);
    assert_eq!(result["unique_resource_count"], 7);
    let mut value = original.clone();
    value["resources"].as_array_mut().unwrap().pop();
    rejects(
        projection(&value, &root, None, None),
        "incomplete or duplicated",
    );
    let mut value = original.clone();
    let duplicate = value["resources"][0].clone();
    value["resources"].as_array_mut().unwrap().push(duplicate);
    rejects(
        projection(&value, &root, None, None),
        "incomplete or duplicated",
    );
    let mut value = original.clone();
    value["resources"][1]["resource_id"] = value["resources"][0]["resource_id"].clone();
    rejects(
        projection(&value, &root, None, None),
        "resource IDs must be unique",
    );
    let mut wrong = registered.clone();
    wrong["edition"] = "wrong".into();
    rejects(
        projection(&original, &root, None, Some(&wrong)),
        "registered selection",
    );
    for field in [
        "resource_count",
        "verse_count",
        "word_count",
        "word_counts_by_verse",
    ] {
        let mut value = original.clone();
        value["summary"][field] = Value::Null;
        rejects(
            projection(&value, &root, None, None),
            "projection summary mismatch",
        );
    }
    let mut value = original.clone();
    value["resources"][1]["provider_coordinate"]["chapter"] = "wrong".into();
    assert!(projection(&value, &root, None, None).is_err());
    for field in [
        "source_text_included",
        "generic_xml_owner_claimed",
        "intrinsic_or_cross_corpus_word_ids_claimed",
        "accepted_structure_claimed",
    ] {
        let mut value = original.clone();
        value[field] = true.into();
        rejects(
            candidate(&value, &source, Selection::default()),
            "C claim posture mismatch",
        );
    }
    let composed = payload("BC", id);
    let result = projection(
        &composed["projection"],
        &root,
        Some(&composed["owner"]),
        None,
    )
    .unwrap();
    assert_eq!(result["generic_refs_verified"], 7);
    let mut value = composed.clone();
    value["projection"]["generic_owner_candidate"] = "A".into();
    rejects(
        candidate(&value, &source, Selection::default()),
        "generic projection owner must be B",
    );
    for field in [
        "source_text_included",
        "element_content_fingerprints_included",
        "intrinsic_ids_claimed",
        "cross_file_identity_claimed",
        "tei_classification_claimed",
    ] {
        let mut value = composed.clone();
        value["owner"][field] = true.into();
        rejects(
            candidate(&value, &source, Selection::default()),
            "BC owner claim posture mismatch",
        );
    }
    let mut value = composed.clone();
    value["owner"]["candidate"] = "A".into();
    rejects(
        candidate(&value, &source, Selection::default()),
        "owner candidate mismatch",
    );
    let mut value = composed.clone();
    value["owner"]["lab_id"] = "wrong".into();
    rejects(
        candidate(
            &value,
            &source,
            Selection {
                lab: composed["lab_id"].as_str(),
                ..Default::default()
            },
        ),
        "owner lab ID mismatch",
    );
    let mut value = composed;
    value["owner"]["schema_version"] = "wrong".into();
    rejects(
        candidate(&value, &source, Selection::default()),
        "owner schema version mismatch",
    );
}

#[test]
fn selection_receipt_requires_complete_unique_checks_and_matching_observed_hash() {
    let observations = json!({"processes":[{"candidate":"A","selection_kind":"fixture","selection_id":"P1","output_sha256":"a","exit_code":0},{"candidate":"A","selection_kind":"fixture","selection_id":"P1","output_sha256":"a","exit_code":0},{"candidate":"B","selection_kind":"fixture","selection_id":"P1","output_sha256":"b","exit_code":0},{"candidate":"C","selection_kind":"source","selection_id":"selected","output_sha256":"c","exit_code":0}]});
    let mut checks = observations["processes"].as_array().unwrap().clone();
    checks.remove(1);
    let complete = json!({"checks":checks});
    assert_eq!(consumer_bindings(&observations, &complete)["ok"], true);
    let mut missing = complete.clone();
    missing["checks"].as_array_mut().unwrap().pop();
    let result = consumer_bindings(&observations, &missing);
    assert_eq!(result["ok"], false);
    assert_eq!(
        result["missing_selection_keys"],
        json!(["C:source:selected"])
    );
    let mut duplicate = complete.clone();
    let row = duplicate["checks"][0].clone();
    duplicate["checks"].as_array_mut().unwrap().push(row);
    assert_eq!(
        consumer_bindings(&observations, &duplicate)["duplicate_consumer_selection_keys"],
        json!(["A:fixture:P1"])
    );
    let mut wrong = complete.clone();
    wrong["checks"][0]["output_sha256"] = "wrong".into();
    assert_eq!(consumer_bindings(&observations, &wrong)["ok"], false);
    let mut unexpected = complete;
    unexpected["checks"][0]["selection_id"] = "unexpected".into();
    assert_eq!(
        consumer_bindings(&observations, &unexpected)["unexpected_selection_keys"],
        json!(["A:fixture:unexpected"])
    );
}

#[test]
fn authority_claims_are_negative_recursive_and_provider_specific() {
    assert!(!authority(&json!("")));
    assert!(!authority(&json!("this is canonical authority")));
    assert!(authority(&json!(
        "generic return only; no accepted source-text or publication authority"
    )));
    assert!(!authority(&json!(
        "no semantic authority; publication approved"
    )));
    assert!(!authority(&json!("no authority; canonical\u{0301}")));
    assert!(!authority(&json!("no authority; public\u{001c}contract")));
    let payloads = json!({"ok":{"authority_boundary":"no source-text, semantic, graph, canon or publication authority","owner":{"authority_boundary":"no source-text or publication authority"},"projection":{"authority_boundary":"no source-text authority"}},"bad":{"authority_boundary":"no source-text authority","owner":{"authority_boundary":""}}});
    let result = authority_boundaries(&payloads);
    assert_eq!(result["checks"]["ok"], true);
    assert_eq!(result["checks"]["bad:owner"], false);
    assert_eq!(result["ok"], false);
    let hits = fingerprint_keys(
        &json!([{"resource_id":"x","value_sha256":"deadbeef"},{"deep":{"source-content-digest":"x"}}]),
        "resources",
    );
    assert!(hits.iter().any(|p| p.ends_with("value_sha256")));
    assert!(hits.iter().any(|p| p.ends_with("source-content-digest")));
    assert_eq!(
        content_equality(&json!({"B:fixture:P1":{"nested":[{"content_equality_claimed":true}]}}))["invalid_labels"],
        json!(["B:fixture:P1"])
    );
    let result = word_claims(
        &json!({"selected:C":{"candidate":"C","intrinsic_or_cross_corpus_word_ids_claimed":false,"accepted_structure_claimed":false},"replay:C":{"candidate":"C","intrinsic_or_cross_corpus_word_ids_claimed":true,"accepted_structure_claimed":false},"BC:projection":{"accepted_structure_claimed":true}}),
    );
    assert_eq!(
        result["invalid_labels"],
        json!(["BC:projection", "replay:C"])
    );
    let expected = manifest()["parser_posture"].clone();
    let mut payloads = json!({"A":{"candidate":"A","parser_posture":expected},"B":{"candidate":"B","parser_posture":expected},"C":{"candidate":"C","parser_posture":expected},"BC":{"candidate":"BC"},"BC:owner":{"candidate":"B","parser_posture":expected},"BC:projection":{"schema_version":"projection"}});
    assert_eq!(parser_postures(&payloads, &expected)["ok"], true);
    payloads["A"]["parser_posture"]["resolve_entities"] = true.into();
    assert_eq!(
        parser_postures(&payloads, &expected)["invalid_labels"],
        json!(["A"])
    );
}

#[test]
fn source_disclosure_covers_all_sources_xml_content_and_malformed_fallback() {
    let selected = parse(b"<root><value>selected-only</value></root>", 4096, 100, 20).unwrap();
    let replay = parse(b"<root><value>replay-only</value></root>", 4096, 100, 20).unwrap();
    let union = source_values(&selected)
        .into_iter()
        .chain(source_values(&replay))
        .collect::<BTreeSet<_>>();
    assert!(union.contains("selected-only") && union.contains("replay-only"));
    assert_eq!(
        source_values(
            &parse(
                b"<root><value>bbb</value><value>aaa</value></root>",
                4096,
                100,
                20
            )
            .unwrap()
        ),
        vec!["aaa", "bbb"]
    );
    let root=parse(b"<root secret='attribute-secret'>before<!--comment-secret-->after<?pi pi-secret?>tail</root>",4096,100,20).unwrap();
    let values = source_values(&root);
    for content in [
        "attribute-secret",
        "before",
        "comment-secret",
        "after",
        "pi-secret",
        "tail",
    ] {
        assert!(values.contains(&content.to_owned()), "{content}");
    }
    let values = source_values(
        &parse(
            b"<root>alpha&amp;beta<![CDATA[-suffix]]></root>",
            4096,
            100,
            20,
        )
        .unwrap(),
    );
    assert_eq!(values, vec!["alpha&beta-suffix"]);
    assert_eq!(
        value_hits(
            &["alpha".into()],
            &["prefix-alpha-suffix".into()],
            true,
            false
        ),
        vec!["alpha"]
    );
    assert_eq!(
        value_hits(
            &["alpha".into()],
            &["prefix-alpha-suffix".into()],
            false,
            true
        ),
        vec!["alpha"]
    );
    assert!(
        value_hits(
            &["alpha".into()],
            &["prefixalphasuffix".into()],
            false,
            true
        )
        .is_empty()
    );
    let malformed = json!({"fixtures":[{"id":"malformed","xml":"<root secret=\"private-secret\"><item>private-secret</root>"}],"security_fixtures":[]});
    let (strings, _) = fixture_strings(&malformed);
    assert!(strings.contains(&"private-secret".into()));
    assert_eq!(
        value_hits(&["private-secret".into()], &strings, true, false),
        vec!["private-secret"]
    );
    let strings = json_strings(
        &json!({"expanded_name":{"local_name":"structural","namespace_uri":"structural"},"body":"secret"}),
    );
    assert!(strings.contains(&"secret".into()));
    assert!(!strings.contains(&"structural".into()));
    for bad in [
        b"<!DOCTYPE root><root/>".as_slice(),
        b"<root>&external;</root>",
        b"<root/><other/>",
        b"<root><x></root>",
        b"text<root/>",
        b"<root p:a='1'/>",
    ] {
        assert!(parse(bad, 4096, 100, 20).is_err());
    }
}

#[test]
fn complete_tracked_scan_reports_indexes_never_source_values_or_control_collisions() {
    let values = vec!["alpha".into(), "replay-only-sensitive-value".into()];
    let empty_manifest = json!({"fixtures":[],"security_fixtures":[]});
    let files = [
        TrackedFile {
            relative: "nested/tracked-output.md",
            bytes: Some(b"replay-only-sensitive-value\n"),
            excluded: false,
            manifest: false,
        },
        TrackedFile {
            relative: "nested/tracked-output.json",
            bytes: Some(b"{\"value\":\"prefix-alpha-suffix\"}"),
            excluded: false,
            manifest: false,
        },
    ];
    let result = disclosure_scan(&values, &empty_manifest, &files);
    assert_eq!(result["tracked_file_count"], 2);
    assert_eq!(result["scanned_file_count"], 2);
    assert_eq!(
        result["leaks"],
        json!([
            "nested/tracked-output.json:source-value-1",
            "nested/tracked-output.md:source-value-2"
        ])
    );
    assert!(!result.to_string().contains("sensitive-value"));
    let manifest = manifest();
    let values = vec!["Unicode/XML Leningrad Codex".into()];
    let result = disclosure_scan(
        &values,
        &manifest,
        &[TrackedFile {
            relative: "input-manifest.json",
            bytes: Some(MANIFEST_RAW.as_bytes()),
            excluded: false,
            manifest: true,
        }],
    );
    assert_eq!(result["ok"], true);
    assert_eq!(result["manifest_control_collision_count"], 1);
    assert!(result.get("manifest_control_collisions").is_none());
    assert!(!result.to_string().contains(&values[0]));
    let result = disclosure_scan(
        &values,
        &manifest,
        &[
            TrackedFile {
                relative: "missing.md",
                bytes: None,
                excluded: false,
                manifest: false,
            },
            TrackedFile {
                relative: "binary.md",
                bytes: Some(b"\xff"),
                excluded: false,
                manifest: false,
            },
            TrackedFile {
                relative: "excluded.md",
                bytes: Some(b"anything"),
                excluded: true,
                manifest: false,
            },
        ],
    );
    assert_eq!(result["ok"], false);
    assert_eq!(result["excluded_file_count"], 1);
    assert_eq!(result["scanned_file_count"], 1);
}

#[test]
fn source_receipts_rebind_actual_current_digest_and_size() {
    let original = b"<root/>";
    let changed = b"<root><changed/></root>";
    for id in ["selected", "replay"] {
        let manifest = json!({"exact_sources":{id:{"path":"explicit-synthetic.xml"}}});
        let receipt =
            json!({"sources":{id:{"source_sha256":sha(original),"source_bytes":original.len()}}});
        assert_eq!(
            source_binding(Some(original), &manifest, &receipt, id)["ok"],
            true
        );
        let result = source_binding(Some(changed), &manifest, &receipt, id);
        assert_eq!(result["ok"], false);
        assert!(
            result["failures"]
                .as_array()
                .unwrap()
                .contains(&json!("source-receipt-digest-mismatch"))
        );
        assert_eq!(source_binding(None, &manifest, &receipt, id)["ok"], false);
    }
}

#[test]
fn private_output_mode_ignore_scope_and_admission_allowlist() {
    let observations = json!({"processes":[{"selection_kind":"source","output_scope":"absolute","output_ref":"/synthetic/private/outputs/selected/run-1/a.json"}]});
    for (mode, ignored, expected) in [
        (0o644, true, false),
        (0o600, false, false),
        (0o600, true, true),
    ] {
        let file = PrivateFile {
            path: "/synthetic/private/outputs/selected/run-1/a.json",
            regular: true,
            mode,
            ignored,
        };
        assert_eq!(
            private_posture(
                "/synthetic/private",
                true,
                true,
                0o700,
                true,
                &observations,
                &[file]
            )["ok"],
            expected
        );
    }
    let outside = json!({"processes":[{"selection_kind":"source","output_scope":"absolute","output_ref":"/synthetic/private-other/a.json"}]});
    assert_eq!(
        private_posture("/synthetic/private", true, true, 0o700, true, &outside, &[])["ok"],
        false
    );
    let manifest = json!({"public_contract_control":{"admission_boundary":{"baseline_ref":"base","roots":["ToS/canon"],"baseline_tree_ids":{"ToS/canon":"tree"},"allowed_changed_paths":[]}}});
    let changed = changed_paths(&[
        "ToS/canon/committed.json\nToS/canon/staged.json\n",
        "ToS/canon/untracked.json\n",
        "ToS/canon/ignored.json\n",
    ]);
    assert_eq!(
        changed,
        vec![
            "ToS/canon/committed.json",
            "ToS/canon/ignored.json",
            "ToS/canon/staged.json",
            "ToS/canon/untracked.json"
        ]
    );
    let result = admission_boundary(&manifest, &json!({"ToS/canon":"tree"}), &changed);
    assert_eq!(result["ok"], false);
    assert_eq!(result["unexpected_changed_paths"], json!(changed));
    assert_eq!(
        admission_boundary(&manifest, &json!({"ToS/canon":"wrong"}), &[])["ok"],
        false
    );
}

#[test]
fn completed_method_findings_require_exact_frozen_bytes_and_new_methods_refuse() {
    let prior_result = include_bytes!(
        "../../../../ToS/research-packets/foundation-laboratory-2026-07/generic-xml-resource-inventory-uxlc-abc-v1/comparison-result.json"
    );
    assert_eq!(
        sha(prior_result),
        "a8818e65c9f233d4085c9b7681ce3ce2938c705ad07fbb140f4af70688ffe56f"
    );
    let prior_result: Value = serde_json::from_slice(prior_result).unwrap();
    let gate = prior_result["gates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|gate| gate["gate_id"] == "G21-independent-source-return")
        .unwrap();
    assert_eq!(gate["passed"], true);
    assert_eq!(
        gate["evidence"]["source_independence"]["active_consumer_sha256"],
        "7d603e8f11376800eb698e3266c51486ac54a4455823d7fd2a3c37e59ca4045f"
    );
    assert_eq!(gate["evidence"]["source_independence"]["ok"], true);
    // This authentic historical finding is weaker than a new native verdict.
    for field in [
        "public_contract_promoted",
        "publication_authorized",
        "source_text_accepted",
        "semantic_or_graph_state_created",
        "uxlc_item_admitted",
    ] {
        assert_eq!(prior_result[field], false);
    }
    let prefix = "ToS/research-packets/foundation-laboratory-2026-07/generic-xml-resource-inventory-uxlc-abc-v1/";
    let originals=[("build_candidate.py",include_bytes!("../../../../ToS/research-packets/foundation-laboratory-2026-07/generic-xml-resource-inventory-uxlc-abc-v1/build_candidate.py").as_slice()),("run_experiment.py",include_bytes!("../../../../ToS/research-packets/foundation-laboratory-2026-07/generic-xml-resource-inventory-uxlc-abc-v1/run_experiment.py").as_slice()),("consume_owner.py",include_bytes!("../../../../ToS/research-packets/foundation-laboratory-2026-07/generic-xml-resource-inventory-uxlc-abc-v1/consume_owner.py").as_slice()),("evaluate_lab.py",include_bytes!("../../../../ToS/research-packets/foundation-laboratory-2026-07/generic-xml-resource-inventory-uxlc-abc-v1/evaluate_lab.py").as_slice())];
    let mut actual = originals
        .iter()
        .map(|(name, raw)| (format!("{prefix}{name}"), raw.to_vec()))
        .collect::<BTreeMap<_, _>>();
    let required = actual.keys().cloned().collect::<Vec<_>>();
    let freeze: Value = serde_json::from_str(FREEZE_RAW).unwrap();
    assert_eq!(frozen_methods(&freeze, &actual, &required)["ok"], true);
    let old:Value=serde_json::from_str(include_str!("../../../../ToS/research-packets/foundation-laboratory-2026-07/generic-xml-resource-inventory-uxlc-abc-v1/freeze-receipt-v6.json")).unwrap();
    assert_eq!(frozen_methods(&old, &actual, &required)["ok"], false);
    actual.insert(
        format!("{prefix}consume_owner.py"),
        b"import build_candidate\n".to_vec(),
    );
    let result = frozen_methods(&freeze, &actual, &required);
    assert_eq!(result["ok"], false);
    assert_eq!(
        result["active_method_digest_mismatches"],
        json!([format!("{prefix}consume_owner.py")])
    );
    assert!(
        result.get("forbidden_imports").is_none(),
        "a digest refusal must not fabricate an AST finding"
    );
}
