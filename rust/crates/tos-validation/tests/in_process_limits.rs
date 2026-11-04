#![cfg(all(feature = "native", target_os = "linux", target_pointer_width = "64"))]
//! The in-process engine keeps the ceilings a schema worker had: a deadline and a
//! cancellation stop a running validation, and an operation envelope bounds its
//! checks, input, decoded state, diagnostics and CPU.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tos_foundation::Digest256;
use tos_validation::executor::BatchStreamBudget;
use tos_validation::executor::schema_diagnostics::{Caps, Status};
use tos_validation::in_process::{
    InProcessSchemaEngine, Incomplete, InputMode, Unit, UnitCeilings,
};
use tos_validation::item_rules::ItemRefusal;
use tos_validation::source_cut::{
    CutSchemaExecutor, InProcessCutIdentity, InProcessCutSchemaExecutor, InProcessOperationLimits,
};
use tos_validation::{FormatProfile, SchemaResource};

const ROOT: &str = "https://example.test/long";
const HEAVY: &str = "https://example.test/heavy";
const BRANCHES: usize = 300;
const SCHEMA: &[u8] = br##"{"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"https://example.test/long","type":"array","items":{"type":"object","properties":{"word":{"type":"string","pattern":"^(?:[a-z]|[a-z][a-z])+$"},"n":{"type":"integer","minimum":0}},"required":["word","n"]}}"##;

/// A schema whose validation, not the decoding of its instance, takes the time:
/// every item is tried against every branch of a `oneOf`.
fn heavy_schema() -> Vec<u8> {
    let branches = (0..BRANCHES)
        .map(|n| {
            format!(r#"{{"type":"object","properties":{{"n":{{"const":{n}}}}},"required":["n"]}}"#)
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(r#"{{"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"{HEAVY}","type":"array","items":{{"oneOf":[{branches}]}}}}"#)
        .into_bytes()
}

fn resources() -> Vec<SchemaResource> {
    vec![
        SchemaResource {
            uri: ROOT.into(),
            raw: SCHEMA.to_vec(),
        },
        SchemaResource {
            uri: HEAVY.into(),
            raw: heavy_schema(),
        },
    ]
}

/// An instance for the long schema.
#[allow(dead_code)]
fn long_instance(items: usize) -> Vec<u8> {
    let mut raw = b"[".to_vec();
    for i in 0..items {
        if i > 0 {
            raw.push(b',');
        }
        raw.extend_from_slice(
            format!(r#"{{"word":"abcdefghijklmnopqrstuvwxyz","n":{i}}}"#).as_bytes(),
        );
    }
    raw.push(b']');
    raw
}

fn heavy_instance(items: usize) -> Vec<u8> {
    let body = (0..items)
        .map(|i| format!(r#"{{"n":{}}}"#, i % BRANCHES))
        .collect::<Vec<_>>()
        .join(",");
    format!("[{body}]").into_bytes()
}

fn unit(raw: &[u8]) -> Unit<'_> {
    Unit {
        root_uri: HEAVY,
        raw,
        input_mode: InputMode::PublishedStrictSelected,
    }
}

fn engine() -> InProcessSchemaEngine {
    InProcessSchemaEngine::new(resources(), FormatProfile::AssertedSourceCandidateV1).unwrap()
}

/// An instance whose full validation takes at least a quarter of a second here,
/// and that time; the root is compiled before the measurement.
fn long_validation() -> (Vec<u8>, Duration) {
    let mut items = 1_000;
    loop {
        let raw = heavy_instance(items);
        let mut engine = engine();
        let never = AtomicBool::new(false);
        let far = Instant::now() + Duration::from_secs(600);
        engine
            .validate(unit(b"[]"), Caps::CURRENT, far, &never)
            .unwrap();
        let started = Instant::now();
        let report = engine
            .validate(unit(&raw), Caps::CURRENT, far, &never)
            .unwrap();
        let full = started.elapsed();
        assert_eq!(report.status, Status::Valid);
        if full >= Duration::from_millis(250) || items >= 512_000 {
            return (raw, full);
        }
        items *= 2;
    }
}

/// A fresh engine with the heavy root already compiled.
fn warm_engine() -> InProcessSchemaEngine {
    let mut engine = engine();
    engine
        .validate(
            unit(b"[]"),
            Caps::CURRENT,
            Instant::now() + Duration::from_secs(60),
            &AtomicBool::new(false),
        )
        .unwrap();
    engine
}

#[test]
fn a_deadline_stops_a_running_validation() {
    let (raw, full) = long_validation();
    let mut engine = warm_engine();
    let cancelled = AtomicBool::new(false);
    let started = Instant::now();
    let result = engine.validate(unit(&raw), Caps::CURRENT, started + full / 8, &cancelled);
    let spent = started.elapsed();
    assert_eq!(result.unwrap_err(), Incomplete::Deadline);
    assert!(
        spent < full / 2,
        "stopped after {spent:?} of a {full:?} validation"
    );
}

#[test]
fn a_cancellation_stops_a_running_validation() {
    let (raw, full) = long_validation();
    let mut engine = warm_engine();
    let cancelled = AtomicBool::new(false);
    let started = Instant::now();
    let result = std::thread::scope(|scope| {
        scope.spawn(|| {
            std::thread::sleep(full / 8);
            cancelled.store(true, Ordering::Relaxed);
        });
        engine.validate(
            unit(&raw),
            Caps::CURRENT,
            started + Duration::from_secs(600),
            &cancelled,
        )
    });
    let spent = started.elapsed();
    assert_eq!(result.unwrap_err(), Incomplete::Cancelled);
    assert!(
        spent < full / 2,
        "stopped after {spent:?} of a {full:?} validation"
    );
}

#[test]
fn a_cpu_ceiling_stops_a_running_validation() {
    let (raw, full) = long_validation();
    let mut engine = warm_engine();
    let cancelled = AtomicBool::new(false);
    let started = Instant::now();
    let result = engine.validate_within(
        unit(&raw),
        Caps::CURRENT,
        UnitCeilings {
            cpu: Some(Duration::from_micros(200)),
            max_state_bytes: None,
        },
        started + Duration::from_secs(600),
        &cancelled,
    );
    assert_eq!(result.unwrap_err(), Incomplete::BatchBudget);
    assert!(started.elapsed() < full / 2);
}

#[test]
fn a_decoded_state_ceiling_rejects_the_input() {
    let raw = heavy_instance(100);
    let mut engine = engine();
    let report = engine
        .validate_within(
            unit(&raw),
            Caps::CURRENT,
            UnitCeilings {
                cpu: None,
                max_state_bytes: Some(1024),
            },
            Instant::now() + Duration::from_secs(60),
            &AtomicBool::new(false),
        )
        .unwrap();
    assert_eq!(report.status, Status::InputRejected);
}

#[test]
fn one_root_compiles_once_whatever_the_input_mode() {
    let mut engine = engine();
    let cancelled = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(60);
    for input_mode in [
        InputMode::PublishedStrict,
        InputMode::PublishedStrictSelected,
        InputMode::SerdeNormalized,
    ] {
        let report = engine
            .validate(
                Unit {
                    root_uri: ROOT,
                    raw: br#"[{"word":"ab","n":1}]"#,
                    input_mode,
                },
                Caps::CURRENT,
                deadline,
                &cancelled,
            )
            .unwrap();
        assert_eq!(report.status, Status::Valid);
        assert_eq!(report.input_mode, input_mode);
    }
    assert_eq!(engine.compiled_validators(), 1);
}

fn fresh_executor() -> InProcessCutSchemaExecutor<&'static str> {
    InProcessCutSchemaExecutor::new(
        InProcessCutIdentity::Candidate("limits"),
        BTreeMap::from([
            (
                "contract".to_owned(),
                (ROOT.to_owned(), Digest256::of_bytes(SCHEMA)),
            ),
            (
                "heavy".to_owned(),
                (HEAVY.to_owned(), Digest256::of_bytes(&heavy_schema())),
            ),
        ]),
        resources(),
        FormatProfile::AssertedSourceCandidateV1,
        InputMode::PublishedStrictSelected,
    )
    .unwrap()
}

fn limits() -> InProcessOperationLimits {
    InProcessOperationLimits {
        max_checks: 1_000,
        max_total_input_bytes: 64 * 1024 * 1024,
        max_unit_input_bytes: 8 * 1024 * 1024,
        max_total_issues: 1_000,
        max_total_report_bytes: 1024 * 1024,
        wall: Duration::from_secs(600),
        cpu: Duration::from_secs(600),
        max_unit_state_bytes: 256 * 1024 * 1024,
    }
}

fn check(
    executor: &mut InProcessCutSchemaExecutor<&'static str>,
    raw: &[u8],
) -> Result<bool, ItemRefusal> {
    executor.check(
        "row",
        raw,
        "contract",
        Instant::now() + Duration::from_secs(600),
        &AtomicBool::new(false),
    )
}

fn check_heavy(
    executor: &mut InProcessCutSchemaExecutor<&'static str>,
    raw: &[u8],
) -> Result<bool, ItemRefusal> {
    executor.check(
        "row",
        raw,
        "heavy",
        Instant::now() + Duration::from_secs(600),
        &AtomicBool::new(false),
    )
}

const ONE: &[u8] = br#"[{"word":"ab","n":1}]"#;

#[test]
fn an_operation_runs_no_more_checks_than_its_envelope_allows() {
    let mut executor = fresh_executor();
    executor
        .set_operation_limits(InProcessOperationLimits {
            max_checks: 2,
            ..limits()
        })
        .unwrap();
    assert!(check(&mut executor, ONE).unwrap());
    assert!(check(&mut executor, ONE).unwrap());
    assert!(matches!(
        check(&mut executor, ONE),
        Err(ItemRefusal::Budget)
    ));
    assert_eq!(executor.operation_usage().checks, 2);
}

#[test]
fn an_operation_bounds_its_input_bytes() {
    let mut executor = fresh_executor();
    executor
        .set_operation_limits(InProcessOperationLimits {
            max_unit_input_bytes: ONE.len() as u64 - 1,
            ..limits()
        })
        .unwrap();
    assert!(matches!(
        check(&mut executor, ONE),
        Err(ItemRefusal::Budget)
    ));

    let mut executor = fresh_executor();
    executor
        .set_operation_limits(InProcessOperationLimits {
            max_total_input_bytes: ONE.len() as u64 * 2,
            ..limits()
        })
        .unwrap();
    assert!(check(&mut executor, ONE).unwrap());
    assert!(check(&mut executor, ONE).unwrap());
    assert!(matches!(
        check(&mut executor, ONE),
        Err(ItemRefusal::Budget)
    ));
}

#[test]
fn an_operation_bounds_decoded_state_diagnostics_and_cpu() {
    let mut executor = fresh_executor();
    executor
        .set_operation_limits(InProcessOperationLimits {
            max_unit_state_bytes: 64,
            ..limits()
        })
        .unwrap();
    assert!(matches!(
        check(&mut executor, ONE),
        Err(ItemRefusal::Budget)
    ));

    let mut executor = fresh_executor();
    executor
        .set_operation_limits(InProcessOperationLimits {
            max_total_report_bytes: 1,
            ..limits()
        })
        .unwrap();
    assert!(matches!(
        check(&mut executor, br#"[{"word":"A1","n":-1}]"#),
        Err(ItemRefusal::Budget)
    ));

    let (raw, _) = long_validation();
    let mut executor = fresh_executor();
    executor
        .set_operation_limits(InProcessOperationLimits {
            cpu: Duration::from_millis(1),
            ..limits()
        })
        .unwrap();
    assert!(matches!(
        check_heavy(&mut executor, &raw),
        Err(ItemRefusal::Budget)
    ));
}

#[test]
fn an_operation_envelope_is_set_once_before_its_first_check() {
    let mut executor = fresh_executor();
    assert!(check(&mut executor, ONE).unwrap());
    assert!(executor.set_operation_limits(limits()).is_err());
    let mut executor = fresh_executor();
    // A stream budget carries the same envelope through the executor trait.
    CutSchemaExecutor::set_operation_budget(&mut executor, BatchStreamBudget::laboratory())
        .unwrap();
    let carried = executor.operation_limits().unwrap();
    assert_eq!(
        carried.max_checks,
        BatchStreamBudget::laboratory().max_total_units
    );
    assert!(executor.set_operation_limits(limits()).is_err());
    assert!(
        InProcessOperationLimits {
            max_checks: 0,
            ..limits()
        }
        .validate()
        .is_err()
    );
}
