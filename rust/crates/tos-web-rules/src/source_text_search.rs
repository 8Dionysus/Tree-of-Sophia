//! Thin WASM adapter for the shared source-text search kernel.
//!
//! Live source-text search stays behind its selected PG owner, which holds the
//! currentness and rights fence and returns a bounded page. This adapter is
//! for explicit portable candidate packets and parity fixtures; it cannot
//! establish their completeness, custody or disclosure authority.

use tos_foundation::JsonLimits;
use tos_query::{AbortProbe, AbortReason};

const MAX_REQUEST_BYTES: usize = 64 * 1024;
const MAX_SELECTION_BYTES: usize = 16 * 1024;
const MAX_CANDIDATE_SET_BYTES: usize = 8 * 1024 * 1024;
const MAX_PACKET_VISITS: usize = 400_000;
const BUDGET_JSON: &[u8] = br#"{"max_candidates":256,"max_text_code_points":1000000,"max_text_bytes":4194304,"max_excerpt_code_points":512,"max_response_bytes":262144,"max_work_steps":4000000}"#;

struct NoHostAbort;

impl AbortProbe for NoHostAbort {
    fn reason(&self) -> Option<AbortReason> {
        None
    }
}

/// Execute the maintained QRY semantics over one explicit, bounded candidate
/// packet. Production browser search sends only its request to the PG endpoint
/// and receives the owner's already-rendered page; it never sends private
/// candidate text through this WASM function.
pub(crate) fn execute_source_text_search_wasm_v1(
    request_json: &[u8],
    selection_json: &[u8],
    candidate_set_json: &[u8],
) -> Result<Vec<u8>, String> {
    if request_json.len() > MAX_REQUEST_BYTES
        || selection_json.len() > MAX_SELECTION_BYTES
        || candidate_set_json.len() > MAX_CANDIDATE_SET_BYTES
    {
        return Err("budget_exceeded: source-text WASM packet exceeds byte limit".to_owned());
    }
    let probe = NoHostAbort;
    tos_query::source_text_search::execute_source_text_search_json(
        request_json,
        selection_json,
        candidate_set_json,
        BUDGET_JSON,
        JsonLimits {
            max_bytes: MAX_CANDIDATE_SET_BYTES,
            max_depth: 32,
            max_visits: MAX_PACKET_VISITS,
            max_integer_digits: 4_300,
        },
        &probe,
    )
    .map_err(|error| format!("{}: {}", error.code.as_str(), error.message))
}
