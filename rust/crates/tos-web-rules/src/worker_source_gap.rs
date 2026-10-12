//! Cloudflare Worker adapter for the maintained public source-gap query.
//! Exact selected member bytes arrive through the private D1 binding; all
//! membership validation, normalization, ranking and response construction
//! stay in `tos-query::source_gap`.

use tos_foundation::{Digest256, JsonLimits};
use tos_query::{
    AbortProbe, AbortReason,
    source_gap::{
        PublicSourceGapRecord, SourceGapBudget, SourceGapRequest, compute_source_gap_packet,
    },
};
use wasm_bindgen::{JsValue, prelude::wasm_bindgen};

const SOURCE_MAX_BYTES: usize = 64 * 1024 * 1024;
const MAX_RECORDS: usize = 100;
const MAX_RECORD_BYTES: usize = 256_000;
const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
const MAX_WORK_STEPS: u64 = 100_000_000;
const RECORD_PREFIX: &str = "ToS/source-witnesses/access-requests/public-ledger/";
const RECORD_SUFFIX: &str = ".access-request.json";

struct NoAbort;
impl AbortProbe for NoAbort {
    fn reason(&self) -> Option<AbortReason> {
        None
    }
}

fn error(message: &str) -> JsValue {
    JsValue::from_str(message)
}

/// Stateful to avoid creating a second JSON-escaped copy of the complete
/// bounded source cut when the Worker streams exact D1 chunks into WASM.
#[wasm_bindgen]
pub struct WorkerSourceGapSearch {
    query: String,
    limit: usize,
    total_bytes: usize,
    records: Vec<(String, Vec<u8>)>,
    finished: bool,
}

#[wasm_bindgen]
impl WorkerSourceGapSearch {
    #[wasm_bindgen(constructor)]
    pub fn new(query: &str, limit: usize) -> Result<WorkerSourceGapSearch, JsValue> {
        if !(1..=100).contains(&limit) {
            return Err(error("source-gap limit must be 1..100"));
        }
        if query.chars().count() > 256 {
            return Err(error(
                "InvalidRequest: source-gap query exceeds 256 characters",
            ));
        }
        if query.len() > 64 * 1024 {
            return Err(error("source-gap query exceeds request budget"));
        }
        Ok(Self {
            query: query.to_owned(),
            limit,
            total_bytes: 0,
            records: Vec::with_capacity(MAX_RECORDS),
            finished: false,
        })
    }

    pub fn add_record(
        &mut self,
        source_ref: &str,
        expected_sha256: &str,
        raw: &str,
    ) -> Result<(), JsValue> {
        let filename = source_ref
            .strip_prefix(RECORD_PREFIX)
            .ok_or_else(|| error("source-gap source path is outside the public ledger"))?;
        if self.finished
            || self.records.len() >= MAX_RECORDS
            || filename.is_empty()
            || filename.contains('/')
            || !filename.ends_with(RECORD_SUFFIX)
            || self
                .records
                .last()
                .is_some_and(|(previous, _)| previous.as_str() >= source_ref)
        {
            return Err(error(
                "source-gap selected member order or profile is invalid",
            ));
        }
        if raw.len() > MAX_RECORD_BYTES {
            return Err(error("source-gap selected record exceeds byte budget"));
        }
        let digest = Digest256::of_bytes(raw.as_bytes()).to_hex();
        if digest != expected_sha256 {
            return Err(error("source-gap selected source digest differs"));
        }
        self.total_bytes = self
            .total_bytes
            .checked_add(raw.len())
            .filter(|bytes| *bytes <= SOURCE_MAX_BYTES)
            .ok_or_else(|| error("source-gap selected input exceeds byte budget"))?;
        self.records
            .push((source_ref.to_owned(), raw.as_bytes().to_vec()));
        Ok(())
    }

    /// Return the exact maintained QRY response body. The Worker only parses
    /// the result to retain its established HTTP/HEAD framing.
    pub fn finish(&mut self) -> Result<Vec<u8>, JsValue> {
        if self.finished {
            return Err(error("source-gap search session is already complete"));
        }
        self.finished = true;
        let records = self
            .records
            .iter()
            .map(|(source_ref, raw)| PublicSourceGapRecord { source_ref, raw })
            .collect::<Vec<_>>();
        compute_source_gap_packet(
            &records,
            &SourceGapRequest {
                query: self.query.clone(),
                limit: self.limit,
            },
            SourceGapBudget {
                json: JsonLimits::new(MAX_RESPONSE_BYTES, 64, 1_000_000, 64)
                    .expect("constant source-gap Worker limits are valid"),
                max_work_steps: MAX_WORK_STEPS,
                max_response_bytes: MAX_RESPONSE_BYTES,
            },
            &NoAbort,
        )
        .map_err(|error| {
            JsValue::from_str(&format!("source-gap {:?}: {}", error.code, error.message))
        })
    }
}
