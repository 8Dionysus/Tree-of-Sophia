//! Independent test-only transport for R/T3; no expected answers cross this seam.
use std::{env, fs};
use tos_foundation::temporal::{TemporalBudget, execute_temporal_operation};
use tos_foundation::{JsonLimits, JsonMode, emit_value_preserved_json, parse_json};

#[test]
#[ignore = "requires an explicit R/T3 request/output and admitted resource slot"]
fn rt3_execute_request() {
    let input_path = env::var_os("TOS_RT3_REQUEST").expect("explicit request path");
    let output_path = env::var_os("TOS_RT3_ACTUAL").expect("explicit actual path");
    let bytes = fs::read(input_path).expect("read independent request");
    let doc = parse_json(&bytes, JsonMode::PublishedStrict, JsonLimits::default())
        .expect("valid request JSON");
    let request = doc.root();
    assert!(request.object_get("expected").is_none());
    assert!(request.object_get("id").is_none());
    let actual = if env::var_os("TOS_RT3_INTEGER_WIRE").as_deref()
        == Some(std::ffi::OsStr::new("1"))
    {
        // Explicit opt-in transport; the real core wrapper owns all wire policy.
        tos_foundation::temporal::execute_temporal_integer_wire(request, TemporalBudget::default())
    } else {
        let operation = request
            .object_get("operation")
            .and_then(|v| v.as_str())
            .expect("operation");
        let input = request.object_get("input").expect("input");
        let context = request.object_get("context").expect("explicit context");
        execute_temporal_operation(operation, input, context, TemporalBudget::default())
    }
    .expect("operation returned system error; preserve test failure instead of fake domain result");
    let encoded = emit_value_preserved_json(&actual, JsonLimits::default())
        .expect("serializable actual response");
    fs::write(output_path, encoded).expect("write actual response");
}
