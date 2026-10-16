use tos_foundation::{Digest256, JsonLimits};
use tos_query::{QueryErrorCode, SearchDocumentBudget, verify_indexed_search_document};

// CPython 3.14.7 / Unicode 16:
// json.dumps(json.loads(raw), ensure_ascii=False, sort_keys=True).lower()
// -> {"a": ["ος", "straße"], "n": 1e-05, "z": "a:b,c"}
// 49 code points, 52 UTF-8 bytes, SHA256 below. The compact raw order differs,
// and punctuation in the value must not acquire spaces.
const RAW: &[u8] = r#"{"z":"A:B,C","a":["ΟΣ","Straße"],"n":1e-5}"#.as_bytes();
const PYTHON_SHA: &str = "f0c2c37d2f6cdab19c99ed6fc5ced6b4f744a980d496bdeea991ae3929f43011";

fn limits() -> SearchDocumentBudget {
    SearchDocumentBudget {
        max_carrier_bytes: RAW.len(),
        max_document_bytes: 52,
        max_document_code_points: 49,
        json: JsonLimits::default(),
    }
}

#[test]
fn verifies_python_spaced_lower_document_and_exact_limits() {
    let expected = Digest256::from_hex(PYTHON_SHA).unwrap();
    let document = verify_indexed_search_document(RAW, 49, expected, limits()).unwrap();
    assert_eq!(
        document.lower,
        "{\"a\": [\"ος\", \"straße\"], \"n\": 1e-05, \"z\": \"a:b,c\"}"
    );
    assert_eq!(document.code_points, 49);

    let mut bytes_short = limits();
    bytes_short.max_document_bytes = 51;
    assert_eq!(
        verify_indexed_search_document(RAW, 49, expected, bytes_short)
            .unwrap_err()
            .code,
        QueryErrorCode::BudgetExceeded
    );
    let mut points_short = limits();
    points_short.max_document_code_points = 48;
    assert_eq!(
        verify_indexed_search_document(RAW, 49, expected, points_short)
            .unwrap_err()
            .code,
        QueryErrorCode::BudgetExceeded
    );
    assert_eq!(
        verify_indexed_search_document(RAW, 48, expected, limits())
            .unwrap_err()
            .code,
        QueryErrorCode::CorruptSelectedCarrier
    );
    assert_eq!(
        verify_indexed_search_document(RAW, 49, Digest256::of_bytes(b"wrong"), limits())
            .unwrap_err()
            .code,
        QueryErrorCode::CorruptSelectedCarrier
    );
}

#[test]
fn refuses_invalid_wtf16_selected_search_document() {
    let raw = br#"{"x":"\ud800"}"#;
    let mut budget = limits();
    budget.max_carrier_bytes = raw.len();
    assert_eq!(
        verify_indexed_search_document(raw, 0, Digest256::of_bytes(b""), budget)
            .unwrap_err()
            .code,
        QueryErrorCode::CorruptSelectedCarrier
    );
}

#[test]
fn preserves_escaped_punctuation_inside_json_strings() {
    let raw = br#"{"a":"left\\\"quote,colon:right","b":2}"#;
    let expected =
        Digest256::from_hex("47a9d884cbdf7396320c541829edacbbdce5b588f96f8595a2cf832a30277f1e")
            .unwrap();
    let budget = SearchDocumentBudget {
        max_carrier_bytes: raw.len(),
        max_document_bytes: 42,
        max_document_code_points: 42,
        json: JsonLimits::default(),
    };
    let document = verify_indexed_search_document(raw, 42, expected, budget).unwrap();
    assert_eq!(
        document.lower,
        r#"{"a": "left\\\"quote,colon:right", "b": 2}"#
    );
}
