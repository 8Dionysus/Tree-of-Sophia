//! Exact existing snapshot/declaration framing for the Claim publication caller.
//! This is private representation mechanics; bytes carry no source admission.
use serde_json::Value;
use tos_compiler::{Error, Result};
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonMode, canonical_raw_bytes_v1, parse_json,
};

pub(super) fn parse(raw: &[u8], maximum: usize) -> Result<Value> {
    let limits = JsonLimits::new(maximum, 128, 1_000_000, 4096)
        .map_err(|_| Error::Budget("Claim JSON parse limits"))?;
    parse_json(raw, JsonMode::PublishedStrict, limits).map_err(|e| Error::Source(e.to_string()))?;
    serde_json::from_slice(raw).map_err(|_| Error::Invalid("Claim strict JSON"))
}
pub(super) fn canonical(value: &Value, maximum: usize) -> Result<Vec<u8>> {
    // Count first through serde's streaming visitor. The selected Foundation
    // profile supplies Python number spelling, sorted keys and the existing LF.
    struct Counter(usize, usize);
    impl std::io::Write for Counter {
        fn write(&mut self, raw: &[u8]) -> std::io::Result<usize> {
            self.0 = self
                .0
                .checked_add(raw.len())
                .filter(|n| *n <= self.1)
                .ok_or_else(|| std::io::Error::other("Claim JSON byte bound"))?;
            Ok(raw.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer(Counter(0, maximum), value)
        .map_err(|_| Error::Budget("Claim publication JSON input"))?;
    let raw = serde_json::to_vec(value).map_err(|_| Error::Invalid("Claim publication JSON"))?;
    canonical_raw_bytes_v1(
        &raw,
        CanonicalProfile::CorpusSnapshotV1,
        JsonLimits::new(maximum, 128, 1_000_000, 4096)
            .map_err(|_| Error::Budget("Claim publication JSON limits"))?,
    )
    .map_err(|e| Error::Source(e.to_string()))
}
pub(super) fn digest(raw: &[u8]) -> String {
    Digest256::of_bytes(raw).to_hex()
}
pub(super) fn row_digest(value: &Value, maximum: usize) -> Result<String> {
    Ok(digest(&canonical(value, maximum)?))
}
pub(super) fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 4096)
        .ok_or(Error::Invalid("Claim publication bounded identifier"))
}
pub(super) fn number(value: &Value, key: &str) -> Result<u64> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .filter(|n| *n <= 9_007_199_254_740_991)
        .ok_or(Error::Invalid("Claim publication exact count"))
}
pub(super) fn sha(value: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(Error::Invalid("Claim publication lowercase SHA256"));
    }
    Digest256::from_hex(value)
        .map(|_| ())
        .map_err(|_| Error::Invalid("Claim publication SHA256"))
}
