use tos_foundation::temporal::{calendar_adapters::*, TemporalBudget};
use tos_foundation::{parse_json, JsonLimits, JsonMode, JsonValue};
fn parse(s: &str) -> JsonValue {
    parse_json(
        s.as_bytes(),
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap()
    .root()
    .clone()
}
fn f<'a>(v: &'a JsonValue, k: &str) -> &'a JsonValue {
    v.object_get(k).unwrap_or(&JsonValue::Null)
}
fn s<'a>(v: &'a JsonValue, k: &str) -> Option<&'a str> {
    f(v, k).as_str()
}
fn run(
    op: fn(
        &JsonValue,
        &JsonValue,
        TemporalBudget,
    ) -> Result<JsonValue, tos_foundation::temporal::TemporalError>,
    input: &str,
) -> JsonValue {
    op(&parse(input), &parse("{}"), TemporalBudget::default()).unwrap()
}
#[test]
fn calendar_months_require_anchor_and_policy_and_keep_calendar_duration() {
    let r = run(
        add_duration,
        r#"{"duration":{"kind":"Calendar","unit":"month","value":1}}"#,
    );
    assert_eq!(s(&r, "reason"), Some("MissingAnchor"));
    let r = run(
        add_duration,
        r#"{"anchor":{"position":"2024-01-31","resolution":"day","trs_ref":"opaque-v7"},"duration":{"kind":"Calendar","unit":"month","value":1}}"#,
    );
    assert_eq!(s(&r, "reason"), Some("CalendarPolicyRequired"));
    for (raw, amount, expected) in [
        ("2024-01-31", 1, "2024-02-29"),
        ("2023-01-31", 1, "2023-02-28"),
        ("2024-03-31", -1, "2024-02-29"),
        ("2024-12-31", 2, "2025-02-28"),
    ] {
        let i = format!(
            r#"{{"anchor":{{"position":"{raw}","resolution":"day","trs_ref":"opaque-v7"}},"duration":{{"kind":"Calendar","unit":"month","value":{amount}}},"arithmetic_policy":{{"month_end":"ClampToLastValidDay","calendar":"GregorianProleptic","policy_ref":"policy-revision-9"}}}}"#
        );
        let r = run(add_duration, &i);
        assert_eq!(s(&r, "status"), Some("Resolved"));
        assert_eq!(s(f(&r, "value"), "position"), Some(expected));
        assert_eq!(s(f(&r, "value"), "trs_ref"), Some("opaque-v7"));
        assert_eq!(f(&r, "elapsed_duration_inferred"), &JsonValue::Bool(false));
    }
}
#[test]
fn crosswalk_versions_are_immutable_and_labels_are_opaque() {
    for (version, mapping) in [
        ("arbitrary-a", 42),
        ("arbitrary-b", 43),
        ("arbitrary-a", 42),
    ] {
        let raw = format!(
            r#"{{"position":{{"type":"OrdinalPosition","value":10,"trs_ref":"renamed-source-v1"}},"target_trs":"renamed-target-v2","crosswalk_version":"{version}","fixture_mapping":{{"10":{mapping}}}}}"#
        );
        let input = parse(&raw);
        let before = input.clone();
        let r = convert(&input, &parse("{}"), TemporalBudget::default()).unwrap();
        assert_eq!(input, before);
        assert_eq!(s(&r, "status"), Some("Unique"));
        assert_eq!(f(f(&r, "position"), "value"), &parse(&mapping.to_string()));
        assert_eq!(s(&r, "crosswalk_version"), Some(version));
        assert_eq!(f(&r, "preserved_input"), &input);
    }
    for (entry, status) in [
        ("[42,43]", "Ambiguous"),
        ("null", "Unsupported"),
        (
            r#"{"status":"Partial","unresolved":"era","precision_loss":["fractional-coordinate-lost"]}"#,
            "Partial",
        ),
        (
            r#"{"status":"PrecisionLoss","precision_loss":["day"]}"#,
            "PrecisionLoss",
        ),
    ] {
        let raw = format!(
            r#"{{"position":{{"value":10,"trs_ref":"s-v1"}},"target_trs":"t-v1","crosswalk_version":"map-8","fixture_mapping":{{"10":{entry}}}}}"#
        );
        let r = run(convert, &raw);
        assert_eq!(s(&r, "status"), Some(status));
        if status == "Partial" {
            assert_eq!(
                f(&r, "precision_loss"),
                &parse(r#"["fractional-coordinate-lost"]"#)
            );
        }
        if status == "PrecisionLoss" {
            assert_eq!(f(&r, "precision_loss"), &parse(r#"["day"]"#));
        }
    }
    let r = run(
        convert,
        r#"{"position":{"value":11,"trs_ref":"s-v1"},"target_trs":"t-v1","crosswalk_version":"map-v1","fixture_mapping":{"10":42}}"#,
    );
    assert_eq!(s(&r, "status"), Some("OutOfCoverage"));
}
fn zone(calendar: &str, offsets: &str, local: &str) -> String {
    format!(
        r#"{{{calendar}"local_date_time":"{local}","zone_ref":"renamed-zone","transition_fixture":{{"fixture_id":"renamed-rules-v4","validity_window":["2021-10-31T00:00:00","2021-10-31T03:00:00"],"offsets_for_local_wall_time":{offsets}}}}}"#
    )
}
#[test]
fn original_fold_missing_calendar_is_gap_and_clarified_fold_is_actual_computation() {
    let raw = zone("", r#"["+02:00","+01:00"]"#, "2021-10-31T01:30:00");
    let r = run(resolve_position, &raw);
    assert_eq!(s(&r, "reason"), Some("ExplicitCalendarRulesRequired"));
    let raw = zone(
        r#""calendar":"GregorianProleptic","#,
        r#"["+02:00","+01:00"]"#,
        "2021-10-31T01:30:00",
    );
    let r = run(resolve_position, &raw);
    assert_eq!(s(&r, "status"), Some("Ambiguous"));
    let cs = f(&r, "candidates").as_array().unwrap();
    assert_eq!(s(&cs[0], "instant_utc"), Some("2021-10-30T23:30:00Z"));
    assert_eq!(s(&cs[1], "instant_utc"), Some("2021-10-31T00:30:00Z"));
    assert_eq!(
        s(&r, "rules_source"),
        Some("source-profile-fixture-not-IANA")
    );
    let r = run(
        resolve_position,
        &zone(
            r#""calendar":"GregorianProleptic","#,
            "[]",
            "2021-10-31T01:30:00",
        ),
    );
    assert_eq!(s(&r, "reason"), Some("NonexistentLocalTime"));
    let r = run(
        resolve_position,
        &zone(
            r#""calendar":"GregorianProleptic","#,
            r#"["+02:00"]"#,
            "2021-10-31T03:00:00",
        ),
    );
    assert_eq!(s(&r, "status"), Some("OutOfCoverage"));
}
#[test]
fn named_seasons_need_versioned_applicability_not_a_generic_spring() {
    let i = r#"{"position":{"year":1344,"season_code":21,"trs_ref":"opaque-v9"}}"#;
    let r = run(derive_named_extent, i);
    assert_eq!(s(&r, "reason"), Some("SeasonBoundaryRuleNotSupplied"));
    let r = run(
        derive_named_extent,
        r#"{"position":{"year":1344,"season_code":21,"trs_ref":"opaque-v9"},"season_boundary_rule":{"version":"source-table-r1","trs_ref":"opaque-v9","calendar":"GregorianProleptic","boundaries":[{"year":1344,"season_code":21,"start":"1344-03-15","end_exclusive":"1344-06-12"}]}}"#,
    );
    assert_eq!(s(&r, "status"), Some("Resolved"));
    assert_eq!(s(f(&r, "value"), "start"), Some("1344-03-15"));
    assert_eq!(f(&r, "event_extent_inferred"), &JsonValue::Bool(false));
}

fn projection_input(id: &str, calendar: &str) -> String {
    format!(
        r#"{{"source_extent":{{"type":"CalendarDayBucket","calendar":"{calendar}","start":"1945-04-02","end_exclusive":"1945-04-03","trs_ref":"{id}"}},"postgres_type":"daterange","bound_form":"[)","sidecar_claim_ref":"{id}-claim@r8","sidecar":{{"raw_claim":{{"raw":"about April 2?","qualifier":"Uncertain","resolution":"day","rights":"restricted-source"}},"trs_ref":"{id}","precision_assessment":{{"target":"date","assessment":"low"}},"provenance":{{"source":"{id}-witness@r2","locator":"folio/λ","digest":"opaque-fixity"}},"additional_source_metadata":{{"language":"la","unknown":null}}}}}}"#
    )
}
#[test]
fn postgres_projection_roundtrips_arbitrary_sidecar_ids_qualifiers_and_provenance() {
    for id in ["arbitrary-calendar@r7", "renamed-calendar@r91"] {
        let raw = projection_input(id, "GregorianProleptic");
        let i = parse(&raw);
        let before = i.clone();
        let r = project_postgres_range(&i, &parse("{}"), TemporalBudget::default()).unwrap();
        assert_eq!(s(&r, "status"), Some("Projected"));
        assert_eq!(s(&r, "postgres_value"), Some("[1945-04-02,1945-04-03)"));
        assert_eq!(f(&r, "sidecar"), f(&i, "sidecar"));
        assert_eq!(f(&r, "preserved_input"), &i);
        assert_eq!(i, before);
        assert_eq!(
            s(f(f(&r, "sidecar"), "raw_claim"), "qualifier"),
            Some("Uncertain")
        );
        assert_eq!(f(&r, "event_extent_inferred"), &JsonValue::Bool(false));
        assert_eq!(
            s(f(&r, "loss_report"), "range_only_warning"),
            Some("RangeDoesNotEncodeSourceSemantics")
        );
    }
    // Claim exact refs use numeric versions and a digest, independently of TRS
    // labels. Numeric exact refs cannot fall back to permissive legacy forms;
    // legacy string-version refs keep their existing uninterpreted metadata.
    for (claim_ref, projected) in [
        (
            format!(
                r#"{{"id":"arbitrary-owner-claim","version":7,"digest":"sha256:{}"}}"#,
                "a".repeat(64)
            ),
            true,
        ),
        (
            r#"{"id":"legacy-claim","version":"revision-seven"}"#.into(),
            true,
        ),
        (
            format!(
                r#"{{"id":"claim","version":0,"digest":"sha256:{}"}}"#,
                "a".repeat(64)
            ),
            false,
        ),
        (
            format!(
                r#"{{"id":"claim","version":7.0,"digest":"sha256:{}"}}"#,
                "a".repeat(64)
            ),
            false,
        ),
        (
            format!(
                r#"{{"id":"claim","version":"7","digest":"sha256:{}"}}"#,
                "a".repeat(64)
            ),
            true,
        ),
        (
            r#"{"id":"claim","version":7,"digest":"not-a-digest"}"#.into(),
            false,
        ),
        (r#"{"id":"claim","version":7}"#.into(), false),
        (
            format!(
                r#"{{"id":"claim","version":7,"digest":"sha256:{}","admitted":true}}"#,
                "a".repeat(64)
            ),
            false,
        ),
    ] {
        let mut input = parse(&projection_input("opaque-trs@v7", "GregorianProleptic"));
        let expected_ref = parse(&claim_ref);
        set(&mut input, "sidecar_claim_ref", expected_ref.clone());
        let out = project_postgres_range(&input, &parse("{}"), TemporalBudget::default()).unwrap();
        assert_eq!(
            s(&out, "status"),
            Some(if projected { "Projected" } else { "Unresolved" })
        );
        assert_eq!(f(&out, "preserved_input"), &input);
        if projected {
            assert_eq!(f(&out, "sidecar_claim_ref"), &expected_ref);
            assert_eq!(f(&out, "sidecar"), f(&input, "sidecar"));
        }
    }
}
#[test]
fn original_postgres_vector_cannot_invent_calendar_or_resolve_missing_sidecar() {
    let raw = r#"{"source_extent":{"type":"CalendarDayBucket","start":"1945-04-02","end_exclusive":"1945-04-03","trs_ref":"TRS:GregorianProleptic:fixture-v1"},"postgres_type":"daterange","bound_form":"[)","sidecar_claim_ref":"claim-day-1"}"#;
    let r = run(project_postgres_range, raw);
    assert_eq!(s(&r, "status"), Some("Unresolved"));
    assert_eq!(s(&r, "reason"), Some("ProjectionContextRequired"));
    assert!(f(&r, "postgres_value").is_null());
    assert_eq!(f(&r, "preserved_input"), &parse(raw));
    assert_eq!(f(&r, "missing_requirements").as_array().unwrap().len(), 5);
}
#[test]
fn projection_rejects_unsupported_calendar_unit_and_bound_semantics() {
    let raw = projection_input("id@v3", "Julian");
    assert_eq!(
        s(&run(project_postgres_range, &raw), "reason"),
        Some("UnsupportedProjectionCalendar")
    );
    let raw = projection_input("id@v3", "GregorianProleptic");
    for (from, to, expected) in [
        (
            "\"CalendarDayBucket\"",
            "\"ActualInterval\"",
            "ExplicitCalendarDayBucketRequired",
        ),
        (
            "\"bound_form\":\"[)\"",
            "\"bound_form\":\"[]\"",
            "UnsupportedPostgresBoundForm",
        ),
        (
            "\"type\":\"CalendarDayBucket\"",
            "\"type\":\"CalendarDayBucket\",\"unit\":\"second\"",
            "UnsupportedProjectionUnit",
        ),
        (
            "\"type\":\"CalendarDayBucket\"",
            "\"type\":\"CalendarDayBucket\",\"unit\":1",
            "UnsupportedProjectionUnit",
        ),
        ("1945-04-03", "1945-04-04", "NotSingleDayBucket"),
    ] {
        let r = run(project_postgres_range, &raw.replace(from, to));
        assert_eq!(s(&r, "reason"), Some(expected));
    }
}
#[test]
fn projection_sidecar_trs_mismatch_never_launders_a_claim() {
    let raw = projection_input("id@v3", "GregorianProleptic").replace(
        "\"trs_ref\":\"id@v3\",\"precision_assessment\"",
        "\"trs_ref\":\"other@v1\",\"precision_assessment\"",
    );
    let r = run(project_postgres_range, &raw);
    assert_eq!(s(&r, "reason"), Some("SidecarTrsMismatch"));
    assert!(f(&r, "postgres_value").is_null());
}

fn set(v: &mut JsonValue, key: &str, value: JsonValue) {
    let JsonValue::Object(o) = v else {
        panic!("test object required")
    };
    if let Some((_, old)) = o.iter_mut().find(|(k, _)| k.as_str() == Some(key)) {
        *old = value;
    } else {
        o.push((tos_foundation::JsonString::from_utf8(key), value));
    }
}
fn txt(s: &str) -> JsonValue {
    JsonValue::String(tos_foundation::JsonString::from_utf8(s))
}
fn crosswalk_record(
    id: &str,
    version: u64,
    rows: &str,
    max: i64,
    prior: Option<&JsonValue>,
) -> JsonValue {
    let mut payload = parse(
        r#"{"source_trs":{"id":"source","version":"r7"},"target_trs":{"id":"target","version":"r9"},"rule_profile":"finite-mapping-v1","rules_version":"rule","applicability":{"min":0,"max":10,"unit":"step","origin":"declared-zero"},"rows":{},"predecessor":null,"provenance":{"evidence_ref":"synthetic-source@r1","reason":"explicit mapping correction"}}"#,
    );
    set(&mut payload, "rows", parse(rows));
    set(
        &mut payload,
        "rules_version",
        txt(&format!("rules-{version}")),
    );
    let mut a = f(&payload, "applicability").clone();
    set(&mut a, "max", parse(&max.to_string()));
    set(&mut payload, "applicability", a);
    if let Some(p) = prior {
        set(&mut payload, "predecessor", f(p, "record_ref").clone());
    }
    let bytes = tos_foundation::canonical_bytes_v1(
        &payload,
        tos_foundation::CanonicalProfile::SourceRecordDigestV1,
        JsonLimits::default(),
    )
    .unwrap();
    let digest = tos_foundation::Digest256::of_bytes(&bytes).to_prefixed();
    let mut env = parse(
        r#"{"record_ref":{"id":"","version":1,"digest":""},"source_revision":"","record_bytes":""}"#,
    );
    let mut reference = f(&env, "record_ref").clone();
    set(&mut reference, "id", txt(id));
    set(&mut reference, "version", parse(&version.to_string()));
    set(&mut reference, "digest", txt(&digest));
    set(&mut env, "record_ref", reference);
    set(
        &mut env,
        "source_revision",
        txt(&format!("sha256:{}", "a".repeat(64))),
    );
    set(
        &mut env,
        "record_bytes",
        txt(std::str::from_utf8(&bytes).unwrap()),
    );
    env
}
fn record_request(env: &JsonValue) -> JsonValue {
    let mut i = parse(
        r#"{"position":{"value":4,"trs_ref":{"id":"source","version":"r7"},"unit":"step","origin":"declared-zero"},"target_trs":{"id":"target","version":"r9"}}"#,
    );
    set(&mut i, "record", env.clone());
    set(&mut i, "requested_ref", f(env, "record_ref").clone());
    set(
        &mut i,
        "expected_source_revision",
        f(env, "source_revision").clone(),
    );
    i
}

fn stored_crosswalk_wrapper(id: &str, version: u64, prior: &JsonValue) -> JsonValue {
    let mut root = parse(
        r#"{"schema_version":"tos_temporal_crosswalk_record_v1","record_type":"temporal-crosswalk","record_id":"","record_version":1,"preferred_label":"Synthetic mapping","identity_status":"provisional","source_refs":["synthetic:fixture"],"external_identifiers":[],"same_as_posture":"not_assessed","visibility":"public_metadata_only","semantic_content":{"profile":"tos_stored_finite_crosswalk_v1","source_trs":{"id":"opaque-source","version":"s7"},"target_trs":{"id":"opaque-target","version":"t9"},"rule_profile":"finite-mapping-v1","rules_version":"r1","applicability":{"min":10,"max":12,"unit":"step","origin":"declared-zero"},"rows":{"10":100,"12":120},"predecessor_source_record_ref":null,"provenance":{"fixture":"synthetic","uninterpreted":[false,null,{"tag":"raw"}]}}}"#,
    );
    set(&mut root, "record_id", txt(id));
    set(&mut root, "record_version", parse(&version.to_string()));
    let mut payload = f(&root, "semantic_content").clone();
    set(&mut payload, "predecessor_source_record_ref", prior.clone());
    set(&mut root, "semantic_content", payload);
    root
}

fn stored_crosswalk_request(root: &JsonValue) -> JsonValue {
    use tos_foundation::{canonical_bytes_v1, CanonicalProfile, Digest256};
    let bytes = canonical_bytes_v1(
        root,
        CanonicalProfile::SourceRecordDigestV1,
        JsonLimits::default(),
    )
    .unwrap();
    // Exact retained bytes deliberately differ from canonical source identity.
    let raw = format!(" \n{}\n", std::str::from_utf8(&bytes).unwrap());
    let mut owner = parse(r#"{"id":"","version":1,"digest":""}"#);
    set(&mut owner, "id", f(root, "record_id").clone());
    set(&mut owner, "version", f(root, "record_version").clone());
    set(
        &mut owner,
        "digest",
        txt(&Digest256::of_bytes(&bytes).to_prefixed()),
    );
    let mut member = parse(
        r#"{"path":"ToS/source-witnesses/temporal-crosswalks/synthetic/temporal-crosswalk.json","sha256":"","size_bytes":0}"#,
    );
    set(
        &mut member,
        "sha256",
        txt(&Digest256::of_bytes(raw.as_bytes()).to_prefixed()),
    );
    set(&mut member, "size_bytes", parse(&raw.len().to_string()));
    let mut env = parse(
        r#"{"source_record_ref":{},"raw_member_ref":{},"source_revision":"","record_bytes":""}"#,
    );
    let fence = txt(&format!("sha256:{}", "a".repeat(64)));
    set(&mut env, "source_record_ref", owner.clone());
    set(&mut env, "raw_member_ref", member.clone());
    set(&mut env, "source_revision", fence.clone());
    set(&mut env, "record_bytes", txt(&raw));
    let mut req = parse(
        r#"{"schema_version":"tos_stored_crosswalk_decode_request_v1","record":{},"requested_source_record_ref":{},"requested_raw_member_ref":{},"expected_source_revision":""}"#,
    );
    set(&mut req, "record", env);
    set(&mut req, "requested_source_record_ref", owner);
    set(&mut req, "requested_raw_member_ref", member);
    set(&mut req, "expected_source_revision", fence);
    req
}
#[test]
fn crosswalk_candidates_and_exact_reads_keep_versioned_bytes_and_coverage() {
    use tos_foundation::temporal::crosswalk_records::{
        convert_crosswalk_record, prepare_crosswalk_version,
    };
    for id in ["crosswalk-A", "arbitrary-renamed-stream"] {
        let v1 = crosswalk_record(id, 1, r#"{"0":7,"4":42,"10":90}"#, 10, None);
        let retained = v1.clone();
        let v2 = crosswalk_record(id, 2, r#"{"0":7,"4":43,"10":90,"12":50}"#, 20, Some(&v1));
        for env in [&v1, &v2] {
            let mut req = record_request(env);
            if env == &v2 {
                set(&mut req, "predecessor_record", v1.clone());
            }
            let out =
                prepare_crosswalk_version(&req, &parse("{}"), TemporalBudget::default()).unwrap();
            assert_eq!(s(&out, "status"), Some("PreparedCrosswalkVersionCandidate"));
            assert_eq!(f(&out, "persisted"), &JsonValue::Bool(false));
            assert_eq!(f(&out, "candidate_record"), env);
            assert_eq!(s(&out, "writer_binding"), Some("PendingOwnerBinding"));
        }
        for (env, n, status, value) in [
            (&v1, 4, "Unique", Some(42)),
            (&v2, 4, "Unique", Some(43)),
            (&v1, 12, "OutOfCoverage", None),
            (&v2, 12, "Unique", Some(50)),
            (&v1, 0, "Unique", Some(7)),
            (&v1, 10, "Unique", Some(90)),
            (&v1, 4, "Unique", Some(42)),
        ] {
            let mut req = record_request(env);
            let mut p = f(&req, "position").clone();
            set(&mut p, "value", parse(&n.to_string()));
            set(&mut req, "position", p);
            set(
                &mut req,
                "other_records",
                JsonValue::Array(vec![v2.clone()]),
            );
            let out =
                convert_crosswalk_record(&req, &parse("{}"), TemporalBudget::default()).unwrap();
            assert_eq!(s(&out, "status"), Some(status));
            if let Some(value) = value {
                assert_eq!(f(f(&out, "position"), "value"), &parse(&value.to_string()));
                assert_eq!(f(&out, "crosswalk_record_ref"), f(env, "record_ref"));
            } else {
                assert!(f(&out, "position").is_null());
            }
            assert_eq!(f(&out, "preserved_input"), &req);
        }
        assert_eq!(v1, retained);
        assert_eq!(s(&v1, "record_bytes"), s(&retained, "record_bytes"));
    }
    // Stored-wrapper risk row: raw/canonical identity and retained provenance
    // remain distinct for both versions and arbitrary permitted stream IDs.
    use tos_foundation::temporal::stored_crosswalk::decode_stored_crosswalk_record;
    for id in [
        "tos.temporal-crosswalk.fixture-a",
        "tos.temporal-crosswalk.renamed-z9",
    ] {
        let first = stored_crosswalk_wrapper(id, 1, &JsonValue::Null);
        let v1 = stored_crosswalk_request(&first);
        let second = stored_crosswalk_wrapper(id, 2, f(&v1, "requested_source_record_ref"));
        for root in [&first, &second, &first] {
            let req = stored_crosswalk_request(root);
            let original = req.clone();
            let context =
                parse(r#"{"uninterpreted":{"authority":false,"tags":[null,"synthetic"]}}"#);
            let out =
                decode_stored_crosswalk_record(&req, &context, TemporalBudget::default()).unwrap();
            assert_eq!(s(&out, "status"), Some("DecodedCrosswalkSourceWrapper"));
            assert_eq!(s(&out, "authentication"), Some("SuppliedBytesOnly"));
            assert_eq!(
                s(&out, "conversion_policy_admission"),
                Some("NotEstablished")
            );
            assert_eq!(f(&out, "writer_performed"), &JsonValue::Bool(false));
            assert!(out.object_get("stored").is_none());
            // The transport contains canonical JSON, whose object order can
            // differ from the original fixture. Preserve strict parsed-wire
            // equality and separately verify the fixture's canonical content.
            let raw = s(f(&req, "record"), "record_bytes").unwrap();
            let transported = parse(raw);
            assert_eq!(f(&out, "source_record"), &transported);
            assert_eq!(
                f(&out, "semantic_content"),
                f(&transported, "semantic_content")
            );
            let canonical = |value: &JsonValue| {
                tos_foundation::canonical_bytes_v1(
                    value,
                    tos_foundation::CanonicalProfile::SourceRecordDigestV1,
                    JsonLimits::default(),
                )
                .unwrap()
            };
            assert_eq!(canonical(f(&out, "source_record")), canonical(root));
            let raw_digest = tos_foundation::Digest256::of_bytes(raw.as_bytes()).to_prefixed();
            assert_eq!(
                s(f(&out, "raw_member_ref"), "sha256"),
                Some(raw_digest.as_str())
            );
            assert_eq!(
                f(f(&out, "raw_member_ref"), "size_bytes").as_u64(),
                Some(raw.len() as u64)
            );
            assert_eq!(f(&out, "preserved_input"), &original);
            assert_eq!(f(&out, "preserved_context"), &context);
            assert_eq!(req, original);
            assert_eq!(
                f(&out, "source_record_ref"),
                f(&req, "requested_source_record_ref")
            );
            assert_eq!(
                f(&out, "raw_member_ref"),
                f(&req, "requested_raw_member_ref")
            );
            assert_ne!(
                f(f(&out, "source_record_ref"), "digest"),
                f(f(&out, "raw_member_ref"), "sha256")
            );
        }
    }
}
#[test]
fn crosswalk_record_guards_reject_tampered_bindings_without_writes() {
    use tos_foundation::temporal::crosswalk_records::{
        convert_crosswalk_record, prepare_crosswalk_version,
    };
    // Stored-wrapper risk row: independently resealed malformed source content
    // cannot hide behind valid raw/canonical hashes or gain policy authority.
    use tos_foundation::temporal::stored_crosswalk::decode_stored_crosswalk_record;
    let wrapper = stored_crosswalk_wrapper("tos.temporal-crosswalk.guards", 1, &JsonValue::Null);
    let valid = stored_crosswalk_request(&wrapper);
    for defect in [
        "raw-as-canonical",
        "canonical-as-raw",
        "size",
        "path",
        "fence",
        "requested-version",
        "unknown-profile",
        "row-outside",
        "float-row",
        "first-predecessor",
        "metadata",
        "bytes-budget",
        "members-budget",
        "work-budget",
    ] {
        let mut req = valid.clone();
        let mut budget = TemporalBudget::default();
        match defect {
            "unknown-profile" | "row-outside" | "float-row" | "first-predecessor" | "metadata" => {
                let mut root = wrapper.clone();
                let mut payload = f(&root, "semantic_content").clone();
                match defect {
                    "unknown-profile" => set(&mut payload, "profile", txt("unknown-rule-profile")),
                    "row-outside" => set(&mut payload, "rows", parse(r#"{"13":130}"#)),
                    "float-row" => set(&mut payload, "rows", parse(r#"{"10":100.0}"#)),
                    "first-predecessor" => set(
                        &mut payload,
                        "predecessor_source_record_ref",
                        f(&valid, "requested_source_record_ref").clone(),
                    ),
                    "metadata" => set(&mut root, "record_type", txt("claim")),
                    _ => unreachable!(),
                }
                set(&mut root, "semantic_content", payload);
                req = stored_crosswalk_request(&root);
            }
            "fence" => set(
                &mut req,
                "expected_source_revision",
                txt(&format!("sha256:{}", "b".repeat(64))),
            ),
            "requested-version" => {
                let mut r = f(&req, "requested_source_record_ref").clone();
                set(&mut r, "version", parse("2"));
                set(&mut req, "requested_source_record_ref", r);
            }
            "bytes-budget" => budget.max_expression_bytes = 1,
            "members-budget" => budget.max_members = 1,
            "work-budget" => budget.max_pairings = 1,
            _ => {
                let mut env = f(&req, "record").clone();
                if defect == "raw-as-canonical" {
                    let mut r = f(&env, "source_record_ref").clone();
                    set(
                        &mut r,
                        "digest",
                        f(f(&env, "raw_member_ref"), "sha256").clone(),
                    );
                    set(&mut env, "source_record_ref", r.clone());
                    set(&mut req, "requested_source_record_ref", r);
                } else {
                    let mut r = f(&env, "raw_member_ref").clone();
                    match defect {
                        "canonical-as-raw" => set(
                            &mut r,
                            "sha256",
                            f(f(&env, "source_record_ref"), "digest").clone(),
                        ),
                        "size" => set(&mut r, "size_bytes", parse("1")),
                        "path" => set(&mut r, "path", txt("../outside")),
                        _ => unreachable!(),
                    }
                    set(&mut env, "raw_member_ref", r.clone());
                    set(&mut req, "requested_raw_member_ref", r);
                }
                set(&mut req, "record", env);
            }
        }
        let out = decode_stored_crosswalk_record(&req, &parse("{}"), budget);
        if defect == "unknown-profile" {
            let out = out.unwrap();
            assert_eq!(s(&out, "status"), Some("Unsupported"));
            assert_eq!(f(&out, "preserved_input"), &req);
            assert_eq!(
                s(&out, "conversion_policy_admission"),
                Some("NotEstablished")
            );
        } else {
            assert!(out.is_err(), "{defect}");
        }
    }
    let v1 = crosswalk_record("opaque-id", 1, r#"{"4":42}"#, 10, None);
    let v2 = crosswalk_record("opaque-id", 2, r#"{"4":43}"#, 20, Some(&v1));
    for defect in [
        "bytes",
        "requested_ref",
        "fence",
        "predecessor_bytes",
        "predecessor_ref",
    ] {
        let mut req = record_request(&v2);
        set(&mut req, "predecessor_record", v1.clone());
        match defect {
            "bytes" => {
                let mut r = f(&req, "record").clone();
                set(&mut r, "record_bytes", txt("{}"));
                set(&mut req, "record", r);
            }
            "requested_ref" => {
                let mut r = f(&req, "requested_ref").clone();
                set(&mut r, "version", parse("3"));
                set(&mut req, "requested_ref", r);
            }
            "fence" => set(
                &mut req,
                "expected_source_revision",
                txt(&format!("sha256:{}", "b".repeat(64))),
            ),
            "predecessor_bytes" => {
                let mut r = f(&req, "predecessor_record").clone();
                set(&mut r, "record_bytes", txt("{}"));
                set(&mut req, "predecessor_record", r);
            }
            "predecessor_ref" => {
                let mut r = f(&req, "predecessor_record").clone();
                let mut reference = f(&r, "record_ref").clone();
                set(&mut reference, "id", txt("wrong-stream"));
                set(&mut r, "record_ref", reference);
                set(&mut req, "predecessor_record", r);
            }
            _ => unreachable!(),
        }
        assert!(
            prepare_crosswalk_version(&req, &parse("{}"), TemporalBudget::default()).is_err(),
            "{defect}"
        );
    }
    let req = record_request(&v1);
    let mut budget = TemporalBudget::default();
    budget.max_expression_bytes = 1;
    assert!(matches!(
        convert_crosswalk_record(&req, &parse("{}"), budget),
        Err(tos_foundation::temporal::TemporalError::ResourceExhausted(
            _
        ))
    ));
    let mut wrong_axis = req.clone();
    let mut p = f(&wrong_axis, "position").clone();
    set(&mut p, "unit", txt("different-unit"));
    set(&mut wrong_axis, "position", p);
    assert_eq!(
        s(
            &convert_crosswalk_record(&wrong_axis, &parse("{}"), TemporalBudget::default())
                .unwrap(),
            "status"
        ),
        Some("Incomparable")
    );
    for (key, metadata) in [
        ("uncertainty", r#"{"width":null}"#),
        ("uncertainty", r#"{"width":5}"#),
        ("uncertainty", "null"),
        ("uncertainty", "false"),
        ("resolution", r#""year""#),
        ("resolution", "null"),
        ("resolution", "5"),
        ("qualifier", "null"),
    ] {
        let mut request = req.clone();
        let mut position = f(&request, "position").clone();
        set(&mut position, key, parse(metadata));
        set(&mut request, "position", position);
        let out =
            convert_crosswalk_record(&request, &parse("{}"), TemporalBudget::default()).unwrap();
        assert_eq!(s(&out, "status"), Some("Unresolved"));
        assert_eq!(s(&out, "reason"), Some("NonExactPosition"));
        assert!(f(&out, "position").is_null());
        assert_eq!(f(&out, "preserved_input"), &request);
    }
}
