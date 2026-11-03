use super::*;
#[test]
fn retained_parallel_readiness_keeps_witnesses_and_authority_separate() {
    let readiness: Value = serde_json::from_str(include_str!("../../../../../ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/technical-markup/parallel-technical-readiness.v1.json")).unwrap();
    assert_eq!(
        readiness["verification_status"],
        "agent_verified_complete_technical_parallel"
    );
    let german = &readiness["witnesses"]["german_dta_first_editions"];
    let russian = &readiness["witnesses"]["russian_antonovsky_1911"];
    assert_eq!(german["reading_unit_count"], 81);
    assert_eq!(russian["reading_unit_count"], 81);
    assert_eq!(
        german["reading_units_by_part"],
        russian["reading_units_by_part"]
    );
    assert!(
        readiness["checks"]
            .as_object()
            .unwrap()
            .values()
            .all(|v| v == true)
    );
    assert_eq!(
        readiness["authority_boundary"]["technical_parallelism"],
        true
    );
    for field in [
        "translation_alignment_created",
        "ordinal_equality_creates_correspondence",
        "linguistic_or_semantic_authority",
    ] {
        assert_eq!(readiness["authority_boundary"][field], false);
    }
}
fn recipe() -> Value {
    let curation: Value = serde_json::from_str(include_str!("../../../../../ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/de-schmeitzner-1883-part-1/editions/chemnitz-schmeitzner-1883-part-1/items/dta-sbb-corrected-tei-p5/edition-curation.v1.json")).unwrap();
    curation["recipes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|recipe| recipe["stage"] == "tei-technical-markup")
        .unwrap()
        .clone()
}
fn fixture() -> Value {
    serde_json::from_str(include_str!("../fixtures/tei-synthetic-parity.json")).unwrap()
}
fn observed(ctx: &ResearchExecution, f: &Value) -> ObservedPart {
    observed_with_recipe(ctx, f, &recipe())
}
fn observed_with_recipe(ctx: &ResearchExecution, f: &Value, selected: &Value) -> ObservedPart {
    let resources = array(&f["resources"])
        .unwrap()
        .iter()
        .map(|r| (s(&r["locator"]["tei_path"]).unwrap().to_owned(), r.clone()))
        .collect();
    let major = array(&f["major"])
        .unwrap()
        .iter()
        .map(|r| {
            (
                (
                    s(&r["part_label"]).unwrap().to_owned(),
                    s(&r["resource_id"]).unwrap().to_owned(),
                ),
                r["value"].clone(),
            )
        })
        .collect();
    observe(
        ctx,
        f["config"].clone(),
        s(&f["payload_ref"]).unwrap().into(),
        s(&f["xml"]).unwrap().as_bytes(),
        &resources,
        &major,
        &TextProfile::selected(selected, &f["plan"]).unwrap(),
    )
    .unwrap()
}
fn schema_root() -> tempfile::TempDir {
    let t = tempfile::tempdir().unwrap();
    let p = t.path().join(SCHEMA);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(
        p,
        include_bytes!("../../../../../ToS/contracts/source-text-unit-packet-v1.schema.json"),
    )
    .unwrap();
    t
}
#[test]
fn exact_xml_identity_citation_and_packet_match_retained_recipe() {
    let t = schema_root();
    let ctx = ResearchExecution::new(t.path(), 60).unwrap();
    let f = fixture();
    let p = observed(&ctx, &f);
    assert_eq!(p.content, f["expected_text"]);
    assert_eq!(p.units.len(), f["unit_count"].as_u64().unwrap() as usize);
    assert!(p.units.iter().all(|u| u.resource_id != "aux"));
    let parts = vec![p];
    let issued = issue(&f["plan"], &parts, &recipe()).unwrap();
    assert_eq!(
        sha(&encode(&ctx, &issued, true).unwrap()),
        f["issuance_sha256"]
    );
    let ids = identities(&issued).unwrap();
    let chosen = recipe();
    let profile = CitationProfile::selected(&ctx, &chosen).unwrap();
    let cs = citations(
        &ctx,
        &f["plan"],
        &parts,
        &ids,
        &profile,
        frozen_schema(&chosen, "citation-spine").unwrap(),
    )
    .unwrap();
    let csraw = jsonl(&ctx, &cs).unwrap();
    assert_eq!(sha(&csraw), f["citations_sha256"]);
    let method = records::method(
        s(&recipe()["plan_ref"]).unwrap(),
        s(&recipe()["legacy_builder_ref"]).unwrap(),
        s(&recipe()["legacy_event_id"]).unwrap(),
        s(&f["plan"]["created_at"]).unwrap(),
        s(&f["plan"]["language"]).unwrap(),
        &recipe()["record_profile"],
    );
    let packet = packet(
        &ctx,
        &f["plan"],
        &parts[0],
        &ids,
        s(&f["plan"]["outputs"]["citation_spine_ref"]).unwrap(),
        &sha(&csraw),
        &method,
        false,
        &recipe(),
    )
    .unwrap();
    assert_eq!(
        sha(&encode(&ctx, &packet, true).unwrap()),
        f["packet_sha256"]
    );
    let paragraph = parts[0]
        .units
        .iter()
        .find(|u| u.unit_kind == "paragraph")
        .unwrap();
    let exact =
        &parts[0].content[parts[0].positions[paragraph.start]..parts[0].positions[paragraph.end]];
    assert_eq!(exact, "A😀\nB old tail.");
}
#[test]
fn record_profile_requires_selected_scheme() {
    let mut selected = recipe();
    assert!(validate_record_profile(&selected).is_ok());
    selected["record_profile"]["scheme_name"] = Value::Null;
    assert!(validate_record_profile(&selected).is_err());
}
#[test]
fn part_selection_uses_plan_and_refuses_gaps_and_duplicates() {
    let mut plan = json!({"source_items":[{"part_order":1,"part_label":"A"},{"part_order":2,"part_label":"B"}]});
    assert_eq!(part_orders(&plan).unwrap(), BTreeSet::from([1, 2]));
    plan["source_items"][1]["part_order"] = json!(3);
    assert!(part_orders(&plan).is_err());
    plan["source_items"][1]["part_order"] = json!(2);
    plan["source_items"][1]["part_label"] = json!("A");
    assert!(part_orders(&plan).is_err());
}
#[test]
fn text_profile_refuses_choice_rules_inconsistent_with_plan() {
    let mut selected = recipe();
    let fixture = fixture();
    assert!(TextProfile::selected(&selected, &fixture["plan"]).is_ok());
    selected["tei_text_profile"]["choice_preference"] = json!(["orig", "sic", "abbr"]);
    assert!(TextProfile::selected(&selected, &fixture["plan"]).is_err());
    let mut alternate = fixture;
    selected["tei_text_profile"]["choice_preference"] = json!(["corr", "sic", "orig", "abbr"]);
    alternate["plan"]["extraction_policy"]["choice_policy"] =
        json!("prefer-corr-then-sic-then-orig-then-abbr-then-first");
    let root = schema_root();
    let ctx = ResearchExecution::new(root.path(), 60).unwrap();
    let observed = observed_with_recipe(&ctx, &alternate, &selected);
    assert!(observed.content.contains("new tail"));
    assert!(!observed.content.contains("old tail"));
}
#[test]
fn issuance_profile_rejects_duplicate_fields_and_invalid_digest_length() {
    let mut selected = recipe();
    assert!(validate_identity_recipe(&selected).is_ok());
    selected["identity_issuance"]["fixed_kinds"][1]["field"] =
        selected["identity_issuance"]["fixed_kinds"][0]["field"].clone();
    assert!(validate_identity_recipe(&selected).is_err());
    selected = recipe();
    selected["identity_issuance"]["digest_hex_characters"] = json!(65);
    assert!(validate_identity_recipe(&selected).is_err());
}
#[test]
fn rejects_wrong_namespace_dtd_empty_units_and_duplicate_identities() {
    let t = schema_root();
    let ctx = ResearchExecution::new(t.path(), 60).unwrap();
    let f = fixture();
    let mut config = f["config"].clone();
    config["excluded_resource_ids"] = json!([]);
    for raw in [
        "<TEI xmlns=\"wrong\"><text><body><p>x</p></body></text></TEI>",
        "<!DOCTYPE TEI><TEI xmlns=\"http://www.tei-c.org/ns/1.0\"><text><body><p>x</p></body></text></TEI>",
        "<TEI xmlns=\"http://www.tei-c.org/ns/1.0\"><text><body><p/></body></text></TEI>",
    ] {
        assert!(
            observe(
                &ctx,
                config.clone(),
                "payload.xml".into(),
                raw.as_bytes(),
                &BTreeMap::new(),
                &BTreeMap::new(),
                &TextProfile::selected(&recipe(), &f["plan"]).unwrap()
            )
            .is_err()
        );
    }
    let p = observed(&ctx, &f);
    let mut v = issue(&f["plan"], &[p], &recipe()).unwrap();
    v["unit_identities"][1]["unit_id"] = v["unit_identities"][0]["unit_id"].clone();
    assert!(identities(&v).is_err());
}
#[test]
fn packet_refuses_reciprocal_drift_and_semantic_promotion() {
    let t = schema_root();
    let ctx = ResearchExecution::new(t.path(), 60).unwrap();
    let f = fixture();
    let parts = vec![observed(&ctx, &f)];
    let ids = identities(&issue(&f["plan"], &parts, &recipe()).unwrap()).unwrap();
    let chosen = recipe();
    let profile = CitationProfile::selected(&ctx, &chosen).unwrap();
    let cs = citations(
        &ctx,
        &f["plan"],
        &parts,
        &ids,
        &profile,
        frozen_schema(&chosen, "citation-spine").unwrap(),
    )
    .unwrap();
    let method = records::method(
        s(&recipe()["plan_ref"]).unwrap(),
        BUILDER,
        "tos.event.native.test",
        s(&f["plan"]["created_at"]).unwrap(),
        s(&f["plan"]["language"]).unwrap(),
        &recipe()["record_profile"],
    );
    let p = packet(
        &ctx,
        &f["plan"],
        &parts[0],
        &ids,
        "ToS/source-witnesses/test.jsonl",
        &sha(&jsonl(&ctx, &cs).unwrap()),
        &method,
        false,
        &recipe(),
    )
    .unwrap();
    let mut v = p.clone();
    v["units"][0]["ordered_child_unit_refs"] = json!([]);
    assert!(validate_packet(&ctx, &v, None).is_err());
    let mut v = p.clone();
    v["units"][1]["semantic_promotion"] = json!(true);
    assert!(validate_packet(&ctx, &v, None).is_err());
    let mut v = p;
    v["anchors"][0]["exact_sha256"] = json!("0".repeat(64));
    assert!(validate_packet(&ctx, &v, Some((&parts[0].content, &parts[0].positions))).is_err());
}
