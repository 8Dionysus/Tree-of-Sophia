use std::cell::Cell;
use std::collections::HashMap;
use std::mem::size_of;
use tos_foundation::{
    CanonicalProfile, CanonicalReusePrepared, CanonicalReuseWorkspace, CanonicalSlice, Digest256,
    FoundationError, FoundationErrorCode, JsonLimits, JsonMode, JsonNumber, JsonNumberKind,
    JsonString, JsonValue, Result, canonical_bytes_v1_with_state_budget_and_visits_and_admission,
    canonical_source_command_into_reuse, parse_json, prepare_source_command_reuse,
};

// Same public-type enclosure as rich_digest_reservation(), less its bridge16384.
// This is source-sized test setup, not a measured layout or a larger wire limit.
const WRITER: usize = 65 * size_of::<Vec<(usize, &(JsonString, JsonValue))>>()
    + 2 * size_of::<String>()
    + size_of::<usize>()
    + 2 * (size_of::<(
        Option<&mut dyn FnMut() -> Result<()>>,
        Option<&mut dyn FnMut(usize, usize) -> Result<()>>,
        usize,
    )>() + size_of::<[usize; 12]>());
const SORT: usize = 19 * size_of::<(usize, &(JsonString, JsonValue))>();
const NUMERIC: usize = 65
    * (size_of::<JsonValue>()
        + size_of::<JsonString>()
        + size_of::<HashMap<Vec<u16>, usize>>()
        + 2 * size_of::<Vec<JsonValue>>());
const AVAILABLE: usize = WRITER + SORT + 8192 + NUMERIC + 8;

fn limits() -> JsonLimits {
    JsonLimits {
        max_bytes: 8192,
        ..JsonLimits::default()
    }
}

fn decoded(raw: &str) -> JsonValue {
    parse_json(raw.as_bytes(), JsonMode::PublishedStrict, limits())
        .unwrap()
        .into_root()
}

fn number(kind: JsonNumberKind, lexeme: &str) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind,
        lexeme: lexeme.to_owned(),
    })
}

fn prepared(value: &JsonValue) -> CanonicalReusePrepared<'_> {
    match prepare_source_command_reuse(value, &mut || Ok(()), &mut |_, _| Ok(())) {
        Ok(Some(prepared)) => prepared,
        Ok(None) => panic!("small supported fixture must select the reusable writer"),
        Err(error) => panic!("unexpected preparation error: {error:?}"),
    }
}

fn legacy(value: &JsonValue, limits: JsonLimits) -> Result<(Vec<u8>, usize)> {
    canonical_bytes_v1_with_state_budget_and_visits_and_admission(
        value,
        CanonicalProfile::SourceCommandInputV1,
        limits,
        AVAILABLE,
        &mut || Ok(()),
        &mut |_, _| Ok(()),
    )
}

fn selected<'w>(
    value: &JsonValue,
    owner: &'w mut CanonicalReuseWorkspace,
    limits: JsonLimits,
) -> Result<CanonicalSlice<'w>> {
    canonical_source_command_into_reuse(
        prepared(value),
        owner,
        limits,
        AVAILABLE,
        &mut || Ok(()),
        &mut |_, _| Ok(()),
    )
}

fn error_of<T>(result: Result<T>) -> FoundationError {
    match result {
        Err(error) => error,
        Ok(_) => panic!("operation unexpectedly succeeded"),
    }
}

fn assert_poisoned(owner: &mut CanonicalReuseWorkspace) {
    let root = JsonValue::Bool(true);
    let token = prepared(&root);
    let checks = Cell::new(0usize);
    let admits = Cell::new(0usize);
    let error = error_of(canonical_source_command_into_reuse(
        token,
        owner,
        limits(),
        AVAILABLE,
        &mut || {
            checks.set(checks.get() + 1);
            Ok(())
        },
        &mut |_, _| {
            admits.set(admits.get() + 1);
            Ok(())
        },
    ));
    assert_eq!(error.code, FoundationErrorCode::BudgetExceeded);
    assert_eq!((checks.get(), admits.get()), (0, 0));
}

#[test]
fn changing_nested_roots_preserve_bytes_and_wire_identity() {
    let cases = [
        (
            "{\"z\":[1,-0,{\"b\":true,\"a\":\"é\\n\"}],\"a\":null}",
            "{\"a\":null,\"z\":[1,0,{\"a\":\"é\\n\",\"b\":true}]}",
        ),
        ("[{\"same\":1},{\"same\":2}]", "[{\"same\":1},{\"same\":2}]"),
        ("{}", "{}"),
        (
            "{\"text\":\"a longer independent root without stale suffix\",\"array\":[null,false,[],{}]}",
            "{\"array\":[null,false,[],{}],\"text\":\"a longer independent root without stale suffix\"}",
        ),
    ];
    let mut owner = CanonicalReuseWorkspace::new().unwrap();
    let mut first_storage = None;
    for (raw, expected) in cases {
        // The root, prepared token and returned slice all end before the next call.
        let root = decoded(raw);
        let old = legacy(&root, limits()).unwrap();
        let storage = {
            let output = selected(&root, &mut owner, limits()).unwrap();
            assert_eq!(output.raw, expected.as_bytes());
            assert_eq!(output.raw, old.0.as_slice());
            assert_eq!(Digest256::of_bytes(output.raw), Digest256::of_bytes(&old.0));
            assert!(output.visits > 0 && output.visits <= limits().max_visits);
            assert_eq!(output.capacity, 8192);
            (output.raw.as_ptr(), output.capacity)
        };
        match first_storage {
            Some(first) => assert_eq!(storage, first),
            None => first_storage = Some(storage),
        }
    }
}

#[test]
fn storage_hints_return_none_without_validating() {
    let large_object = JsonValue::Object(
        (0..20)
            .rev()
            .map(|i| (JsonString::from_utf8(&format!("k{i:02}")), JsonValue::Null))
            .collect(),
    );
    let object_bytes = concat!(
        "{\"k00\":null,\"k01\":null,\"k02\":null,\"k03\":null,\"k04\":null,",
        "\"k05\":null,\"k06\":null,\"k07\":null,\"k08\":null,\"k09\":null,",
        "\"k10\":null,\"k11\":null,\"k12\":null,\"k13\":null,\"k14\":null,",
        "\"k15\":null,\"k16\":null,\"k17\":null,\"k18\":null,\"k19\":null}"
    );
    let cases = [
        (number(JsonNumberKind::Float, "1.25"), Some("1.25")),
        (number(JsonNumberKind::Int, "12345"), Some("12345")),
        (large_object, Some(object_bytes)),
        (number(JsonNumberKind::Int, "01"), None),
        (number(JsonNumberKind::Int, "1.0"), None),
        (number(JsonNumberKind::Float, "1"), None),
    ];
    for (root, expected) in cases {
        match prepare_source_command_reuse(&root, &mut || Ok(()), &mut |_, _| Ok(())) {
            Ok(None) => {}
            Ok(Some(_)) => panic!("unsupported hint must return None"),
            Err(error) => panic!("hint must not semantically validate: {error:?}"),
        }
        let old = legacy(&root, limits());
        match expected {
            Some(bytes) => assert_eq!(old.unwrap().0, bytes.as_bytes()),
            None => assert_eq!(error_of(old).code, FoundationErrorCode::InvalidNumber),
        }
    }
}

#[test]
fn decoded_duplicate_and_lone_surrogate_keys_refuse_and_poison() {
    let cases = [
        (
            JsonValue::Object(vec![
                (JsonString::from_utf8("a"), JsonValue::Null),
                (JsonString::from_utf16(&[0x0061]), JsonValue::Bool(true)),
            ]),
            FoundationErrorCode::DuplicateMember,
        ),
        (
            JsonValue::Object(vec![(JsonString::from_utf16(&[0xD800]), JsonValue::Null)]),
            FoundationErrorCode::InvalidUnicodeScalar,
        ),
    ];
    for (root, code) in cases {
        assert_eq!(error_of(legacy(&root, limits())).code, code);
        let mut owner = CanonicalReuseWorkspace::new().unwrap();
        assert_eq!(error_of(selected(&root, &mut owner, limits())).code, code);
        assert_poisoned(&mut owner);
    }
}

#[test]
fn negative_zero_and_full_numeric_validation_preserved() {
    let mut owner = CanonicalReuseWorkspace::new().unwrap();
    for (lexeme, spelling) in [
        ("-0", "0"),
        ("0", "0"),
        ("-1", "-1"),
        ("12", "12"),
        ("9999", "9999"),
    ] {
        let cases = [
            (number(JsonNumberKind::Int, lexeme), spelling.to_owned()),
            (
                JsonValue::Object(vec![(
                    JsonString::from_utf8("n"),
                    JsonValue::Array(vec![number(JsonNumberKind::Int, lexeme)]),
                )]),
                format!("{{\"n\":[{spelling}]}}"),
            ),
        ];
        for (root, expected) in cases {
            let old = legacy(&root, limits()).unwrap();
            let output = selected(&root, &mut owner, limits()).unwrap();
            assert_eq!(output.raw, expected.as_bytes());
            assert_eq!(output.raw, old.0.as_slice());
            assert_eq!(Digest256::of_bytes(output.raw), Digest256::of_bytes(&old.0));
        }
    }
}

fn sentinel() -> FoundationError {
    FoundationError {
        code: FoundationErrorCode::BudgetExceeded,
        byte_offset: Some(37),
        detail: "canonical reuse callback sentinel".to_owned(),
    }
}

#[derive(Clone, Copy)]
enum Callback {
    Check,
    Admit,
}

#[test]
fn callback_errors_survive_prepare_count_and_emit() {
    let root = decoded("{\"z\":[1,2],\"a\":{\"x\":true}}");
    for kind in [Callback::Check, Callback::Admit] {
        let error = match kind {
            Callback::Check => error_of(prepare_source_command_reuse(
                &root,
                &mut || Err(sentinel()),
                &mut |_, _| Ok(()),
            )),
            Callback::Admit => error_of(prepare_source_command_reuse(
                &root,
                &mut || Ok(()),
                &mut |_, _| Err(sentinel()),
            )),
        };
        assert_eq!(error, sentinel());
    }
    for kind in [Callback::Check, Callback::Admit] {
        // Exactly one successful control for this callback kind and exact root.
        // The last check is output.poll.now() AFTER emission; the last admit is
        // the root closing delimiter's emit admission, also in the emit pass.
        let checks = Cell::new(0usize);
        let admits = Cell::new(0usize);
        let mut control_owner = CanonicalReuseWorkspace::new().unwrap();
        let output = canonical_source_command_into_reuse(
            prepared(&root),
            &mut control_owner,
            limits(),
            AVAILABLE,
            &mut || {
                checks.set(checks.get() + 1);
                Ok(())
            },
            &mut |_, _| {
                admits.set(admits.get() + 1);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(output.raw, b"{\"a\":{\"x\":true},\"z\":[1,2]}");
        assert!(checks.get() > 1 && admits.get() > 1);
        let last = match kind {
            Callback::Check => checks.get(),
            Callback::Admit => admits.get(),
        };
        for failure_at in [1, last] {
            let checks = Cell::new(0usize);
            let admits = Cell::new(0usize);
            let mut owner = CanonicalReuseWorkspace::new().unwrap();
            let error = error_of(canonical_source_command_into_reuse(
                prepared(&root),
                &mut owner,
                limits(),
                AVAILABLE,
                &mut || {
                    checks.set(checks.get() + 1);
                    if matches!(kind, Callback::Check) && checks.get() == failure_at {
                        Err(sentinel())
                    } else {
                        Ok(())
                    }
                },
                &mut |_, _| {
                    admits.set(admits.get() + 1);
                    if matches!(kind, Callback::Admit) && admits.get() == failure_at {
                        Err(sentinel())
                    } else {
                        Ok(())
                    }
                },
            ));
            assert_eq!(error, sentinel());
            let observed = match kind {
                Callback::Check => checks.get(),
                Callback::Admit => admits.get(),
            };
            assert_eq!(observed, failure_at);
            assert_poisoned(&mut owner);
        }
    }
}

#[test]
fn wire_depth_visit_limits_refuse_without_recovery() {
    let cases = [
        (
            JsonValue::Bool(true),
            JsonLimits {
                max_bytes: 1,
                ..limits()
            },
        ),
        (
            decoded("[[[]]]"),
            JsonLimits {
                max_depth: 1,
                ..limits()
            },
        ),
        (
            decoded("[true]"),
            JsonLimits {
                max_visits: 1,
                ..limits()
            },
        ),
    ];
    for (root, lowered) in cases {
        assert_eq!(
            error_of(legacy(&root, lowered)).code,
            FoundationErrorCode::BudgetExceeded
        );
        let mut owner = CanonicalReuseWorkspace::new().unwrap();
        assert_eq!(
            error_of(selected(&root, &mut owner, lowered)).code,
            FoundationErrorCode::BudgetExceeded
        );
        assert_poisoned(&mut owner);
    }
}

#[test]
fn shrinking_numeric_spellings_keep_original_input_limit() {
    for (kind, lexeme, spelling) in [
        (JsonNumberKind::Int, "-0", "0"),
        (JsonNumberKind::Float, "1.0000", "1.0"),
        (JsonNumberKind::Float, "100e-2", "1.0"),
    ] {
        for (root, expected) in [
            (number(kind, lexeme), spelling.to_owned()),
            (
                JsonValue::Array(vec![number(kind, lexeme)]),
                format!("[{spelling}]"),
            ),
        ] {
            if kind == JsonNumberKind::Float {
                assert!(matches!(
                    prepare_source_command_reuse(&root, &mut || Ok(()), &mut |_, _| Ok(()),),
                    Ok(None)
                ));
            }
            for profile in [
                CanonicalProfile::SourceCommandInputV1,
                CanonicalProfile::SourceRecordDigestV1,
                CanonicalProfile::CorpusSnapshotV1,
            ] {
                let expected = if profile == CanonicalProfile::CorpusSnapshotV1 {
                    format!("{expected}\n")
                } else {
                    expected.clone()
                };
                let (raw, _) = canonical_bytes_v1_with_state_budget_and_visits_and_admission(
                    &root,
                    profile,
                    limits(),
                    AVAILABLE,
                    &mut || Ok(()),
                    &mut |_, _| Ok(()),
                )
                .unwrap();
                assert_eq!(raw, expected.as_bytes());
                assert_eq!(
                    raw,
                    tos_foundation::canonical_bytes_v1(&root, profile, limits()).unwrap()
                );
            }
        }
    }
}

#[test]
fn original_numeric_input_bound_and_strict_refusals_remain() {
    for (kind, lexeme, bound) in [
        (JsonNumberKind::Int, "-0", 1),
        (JsonNumberKind::Float, "1.0000", 4),
    ] {
        let root = number(kind, lexeme);
        let limited = JsonLimits {
            max_bytes: bound,
            ..limits()
        };
        let error = error_of(legacy(&root, limited));
        assert_eq!(error.code, FoundationErrorCode::BudgetExceeded);
        assert_eq!(error.detail, "JSON byte budget exceeded");
        if kind == JsonNumberKind::Int {
            let mut owner = CanonicalReuseWorkspace::new().unwrap();
            let error = error_of(selected(&root, &mut owner, limited));
            assert_eq!(error.code, FoundationErrorCode::BudgetExceeded);
            assert_eq!(error.detail, "JSON byte budget exceeded");
            assert_poisoned(&mut owner);
        }
    }
    for (kind, lexeme, code, detail) in [
        (
            JsonNumberKind::Int,
            "01",
            FoundationErrorCode::InvalidNumber,
            "leading zero",
        ),
        (
            JsonNumberKind::Int,
            "1.0",
            FoundationErrorCode::InvalidNumber,
            "number lexeme and kind disagree",
        ),
        (
            JsonNumberKind::Float,
            "1",
            FoundationErrorCode::InvalidNumber,
            "number lexeme and kind disagree",
        ),
        (
            JsonNumberKind::Float,
            "1e999",
            FoundationErrorCode::NonfiniteFloat,
            "nonfinite or unrepresentable float",
        ),
    ] {
        let error = error_of(legacy(&number(kind, lexeme), limits()));
        assert_eq!(error.code, code);
        assert_eq!(error.detail, detail);
    }
}
