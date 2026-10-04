//! Bounded source representation and exact maintained projection digests.
use crate::{Error, Result};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    io::{self, BufReader, Read, Write},
};
use tos_foundation::{
    CanonicalProfile, Digest256Hasher, FoundationError, FoundationErrorCode, JsonLimits, JsonMode,
    canonical_bytes_v1_with_check, canonical_feed_digest_v1_with_check, parse_json_with_check,
};

const CHECKED_IO_BYTES: usize = 64 * 1024;

/// Source decoding at a selected consumer seam, never source admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhilosophySourceReadProfile {
    PublishedStrict,
    LegacyPythonJsonLoads,
}

pub(crate) fn json_limits(max: usize) -> Result<JsonLimits> {
    JsonLimits::new(max, 96, 2_000_000, 4096).map_err(|e| Error::Source(e.to_string()))
}
pub(crate) fn canonical(raw: &[u8], max: usize) -> Result<Vec<u8>> {
    canonical_with_check(raw, max, &mut || Ok(()))
}
pub(crate) fn canonical_with_check(
    raw: &[u8],
    max: usize,
    check: &mut impl FnMut() -> Result<()>,
) -> Result<Vec<u8>> {
    let limits = json_limits(max)?;
    let doc = checked_foundation_call(check, |checked| {
        parse_json_with_check(raw, JsonMode::PublishedStrict, limits, checked)
    })?;
    checked_foundation_call(check, |checked| {
        canonical_bytes_v1_with_check(
            doc.root(),
            CanonicalProfile::SourceRecordDigestV1,
            limits,
            checked,
        )
    })
}

fn checked_foundation_call<T>(
    check: &mut dyn FnMut() -> Result<()>,
    call: impl FnOnce(&mut dyn FnMut() -> tos_foundation::Result<()>) -> tos_foundation::Result<T>,
) -> Result<T> {
    let mut callback_error = None;
    let result = {
        let mut checked = || match check() {
            Ok(()) => Ok(()),
            Err(error) => {
                callback_error = Some(error);
                Err(FoundationError::new(
                    FoundationErrorCode::BudgetExceeded,
                    "philosophy operation guard refused checked JSON work",
                ))
            }
        };
        call(&mut checked)
    };
    // Recheck the original compiler guard before translating a Foundation
    // refusal, preserving the caller's cancellation/deadline error class.
    check()?;
    if let Some(error) = callback_error {
        return Err(error);
    }
    result.map_err(|error| Error::Source(error.to_string()))
}

struct InstanceWriter<'a> {
    raw: Vec<u8>,
    max: usize,
    check: &'a mut dyn FnMut() -> Result<()>,
    callback_error: &'a mut Option<Error>,
}
impl InstanceWriter<'_> {
    fn poll(&mut self) -> io::Result<()> {
        if let Err(error) = (self.check)() {
            *self.callback_error = Some(error);
            return Err(io::Error::other("philosophy JSON write guard refused"));
        }
        Ok(())
    }
}
impl Write for InstanceWriter<'_> {
    fn write(&mut self, value: &[u8]) -> io::Result<usize> {
        if self
            .raw
            .len()
            .checked_add(value.len())
            .is_none_or(|n| n > self.max)
        {
            return Err(io::Error::other("philosophy JSON bytes"));
        }
        for chunk in value.chunks(CHECKED_IO_BYTES) {
            self.poll()?;
            self.raw.extend_from_slice(chunk);
        }
        Ok(value.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.poll()
    }
}

struct CheckedSliceReader<'a, 'c> {
    raw: &'a [u8],
    position: usize,
    check: &'c mut dyn FnMut() -> Result<()>,
    callback_error: &'c mut Option<Error>,
}

impl CheckedSliceReader<'_, '_> {
    fn poll(&mut self) -> io::Result<()> {
        if let Err(error) = (self.check)() {
            *self.callback_error = Some(error);
            return Err(io::Error::other("philosophy JSON reader guard refused"));
        }
        Ok(())
    }
}

impl Read for CheckedSliceReader<'_, '_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.poll()?;
        if output.is_empty() {
            return Ok(0);
        }
        let count = output
            .len()
            .min(CHECKED_IO_BYTES)
            .min(self.raw.len().saturating_sub(self.position));
        if count == 0 {
            self.poll()?;
            return Ok(0);
        }
        output[..count].copy_from_slice(&self.raw[self.position..self.position + count]);
        self.position += count;
        self.poll()?;
        Ok(count)
    }
}

fn canonical_equal_with_check(
    left: &[u8],
    right: &[u8],
    check: &mut impl FnMut() -> Result<()>,
) -> Result<bool> {
    check()?;
    if left.len() != right.len() {
        return Ok(false);
    }
    for (left, right) in left
        .chunks(CHECKED_IO_BYTES)
        .zip(right.chunks(CHECKED_IO_BYTES))
    {
        check()?;
        if left != right {
            return Ok(false);
        }
    }
    check()?;
    Ok(true)
}

fn serde_value_with_check(raw: &[u8], check: &mut impl FnMut() -> Result<()>) -> Result<Value> {
    let mut callback_error = None;
    let decoded = {
        let reader = CheckedSliceReader {
            raw,
            position: 0,
            check: &mut *check,
            callback_error: &mut callback_error,
        };
        let capacity = raw.len().clamp(1, CHECKED_IO_BYTES);
        let mut buffered = BufReader::with_capacity(capacity, reader);
        serde_json::from_reader::<_, Value>(&mut buffered)
    };
    check()?;
    if let Some(error) = callback_error {
        return Err(error);
    }
    decoded.map_err(|_| {
        Error::Source("philosophy source representation unsupported by serde JSON".into())
    })
}

pub(crate) fn bytes(v: &Value, max: usize) -> Result<Vec<u8>> {
    bytes_with_check(v, max, &mut || Ok(()))
}
pub(crate) fn bytes_with_check(
    v: &Value,
    max: usize,
    check: &mut impl FnMut() -> Result<()>,
) -> Result<Vec<u8>> {
    let raw = instance_with_check(v, max, check)?;
    canonical_with_check(&raw, max, check)
}

fn instance_with_check(
    v: &Value,
    max: usize,
    check: &mut impl FnMut() -> Result<()>,
) -> Result<Vec<u8>> {
    // Preserve the serde/FND instance serialization law while refusing during
    // emission, before a late whole-field instance allocates beyond its cap.
    check()?;
    let mut callback_error = None;
    let mut writer = InstanceWriter {
        raw: Vec::new(),
        max,
        check: &mut *check,
        callback_error: &mut callback_error,
    };
    let result = serde_json::to_writer(&mut writer, v);
    let raw = std::mem::take(&mut writer.raw);
    drop(writer);
    if let Some(error) = callback_error.take() {
        return Err(error);
    }
    result.map_err(|_| Error::Budget("philosophy JSON bytes"))?;
    check()?;
    Ok(raw)
}
pub(crate) fn parse(raw: &[u8], max: usize) -> Result<Value> {
    parse_with_profile(raw, max, PhilosophySourceReadProfile::PublishedStrict)
}
pub(crate) fn parse_with_profile(
    raw: &[u8],
    max: usize,
    profile: PhilosophySourceReadProfile,
) -> Result<Value> {
    parse_with_profile_and_check(raw, max, profile, &mut || Ok(()))
}
pub(crate) fn parse_with_profile_and_check(
    raw: &[u8],
    max: usize,
    profile: PhilosophySourceReadProfile,
    check: &mut impl FnMut() -> Result<()>,
) -> Result<Value> {
    let limits = json_limits(max)?;
    let mode = match profile {
        PhilosophySourceReadProfile::PublishedStrict => JsonMode::PublishedStrict,
        PhilosophySourceReadProfile::LegacyPythonJsonLoads => JsonMode::RequestLastWins,
    };
    let document = checked_foundation_call(check, |checked| {
        parse_json_with_check(raw, mode, limits, checked)
    })?;
    let original = checked_foundation_call(check, |checked| {
        canonical_bytes_v1_with_check(
            document.root(),
            CanonicalProfile::SourceRecordDigestV1,
            limits,
            checked,
        )
    })?;
    let v = serde_value_with_check(&original, check)?;
    // No widened integer or native UTF-16 source record may silently become a
    // different portable source body at this explicit representation seam.
    let roundtrip = bytes_with_check(&v, max, check)?;
    let equal = canonical_equal_with_check(&roundtrip, &original, check)?;
    drop(roundtrip);
    if !equal {
        return Err(Error::Source(
            "philosophy source representation loses canonical material".into(),
        ));
    }
    check()?;
    Ok(v)
}
pub(crate) fn object(raw: &[u8], max: usize) -> Result<Value> {
    let v = parse(raw, max)?;
    if !v.is_object() {
        return Err(Error::Invalid("philosophy source object"));
    }
    Ok(v)
}
pub(crate) fn object_with_profile(
    raw: &[u8],
    max: usize,
    profile: PhilosophySourceReadProfile,
) -> Result<Value> {
    let v = parse_with_profile(raw, max, profile)?;
    if !v.is_object() {
        return Err(Error::Invalid("philosophy source object"));
    }
    Ok(v)
}
pub(crate) fn object_with_profile_and_check(
    raw: &[u8],
    max: usize,
    profile: PhilosophySourceReadProfile,
    check: &mut impl FnMut() -> Result<()>,
) -> Result<Value> {
    let v = parse_with_profile_and_check(raw, max, profile, check)?;
    if !v.is_object() {
        return Err(Error::Invalid("philosophy source object"));
    }
    Ok(v)
}
pub(crate) fn required<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or(Error::Invalid("philosophy required string"))
}
pub(crate) fn array<'a>(v: &'a Value, key: &str) -> Result<&'a [Value]> {
    v.get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or(Error::Invalid("philosophy required array"))
}
pub(crate) fn strings(v: &Value) -> Result<Vec<String>> {
    v.as_array()
        .ok_or(Error::Invalid("philosophy string array"))?
        .iter()
        .map(|v| {
            v.as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .ok_or(Error::Invalid("philosophy string array item"))
        })
        .collect()
}
pub(crate) fn string_set(v: &Value) -> BTreeSet<String> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}
pub(crate) fn truth(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64() != Some(0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}
pub(crate) fn string(v: &Value) -> String {
    match v {
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::String(s) => s.clone(),
        Value::Number(n) => String::from_utf8(
            canonical(n.to_string().as_bytes(), 4096).expect("finite portable JSON scalar"),
        )
        .expect("portable numeric UTF-8"),
        _ => v.to_string(),
    }
}
pub(crate) fn fallback(v: &Value, fallback: &str) -> String {
    if truth(v) { string(v) } else { fallback.into() }
}
pub(crate) fn digest(v: &Value, max: usize) -> Result<String> {
    digest_with_check(v, max, &mut || Ok(()))
}
pub(crate) fn digest_with_check(
    v: &Value,
    max: usize,
    check: &mut impl FnMut() -> Result<()>,
) -> Result<String> {
    // Preserve serde/FND normalization while streaming canonical bytes to the
    // digest sink instead of retaining and rescanning a second full byte Vec.
    let raw = instance_with_check(v, max, check)?;
    let limits = json_limits(max)?;
    let document = checked_foundation_call(check, |checked| {
        parse_json_with_check(&raw, JsonMode::PublishedStrict, limits, checked)
    })?;
    let mut hasher = Digest256Hasher::new();
    let mut written = 0;
    let mut visits = 0;
    checked_foundation_call(check, |checked| {
        canonical_feed_digest_v1_with_check(
            document.root(),
            CanonicalProfile::SourceRecordDigestV1,
            limits,
            &mut hasher,
            &mut written,
            &mut visits,
            0,
            checked,
        )
    })?;
    Ok(hasher.finalize().to_hex())
}

// SHA-1 is used only by the source-owned legacy endpoint/cluster naming grammar,
// not as transport fixity or admission. This streaming implementation retains
// that exact mechanical naming formula without a new identity authority.
pub(crate) fn sha1_hex(value: &str) -> String {
    let raw = value.as_bytes();
    let mut state = [
        0x67452301u32,
        0xefcdab89,
        0x98badcfe,
        0x10325476,
        0xc3d2e1f0,
    ];
    fn block(state: &mut [u32; 5], raw: &[u8]) {
        let mut w = [0u32; 80];
        for (i, b) in raw.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes(b.try_into().expect("word"));
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = *state;
        for (i, word) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5a827999),
                20..=39 => (b ^ c ^ d, 0x6ed9eba1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8f1bbcdc),
                _ => (b ^ c ^ d, 0xca62c1d6),
            };
            let t = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        for (s, n) in state.iter_mut().zip([a, b, c, d, e]) {
            *s = s.wrapping_add(n);
        }
    }
    let mut chunks = raw.chunks_exact(64);
    for chunk in &mut chunks {
        block(&mut state, chunk);
    }
    let tail = chunks.remainder();
    let mut padded = [0u8; 128];
    padded[..tail.len()].copy_from_slice(tail);
    padded[tail.len()] = 0x80;
    let length = if tail.len() < 56 { 64 } else { 128 };
    padded[length - 8..length].copy_from_slice(&((raw.len() as u64) * 8).to_be_bytes());
    for chunk in padded[..length].chunks_exact(64) {
        block(&mut state, chunk);
    }
    state.iter().map(|n| format!("{n:08x}")).collect()
}

pub(crate) fn check_run(
    deadline: std::time::Instant,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<()> {
    if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(Error::Invalid("philosophy source cancelled"));
    }
    if std::time::Instant::now() >= deadline {
        return Err(Error::Budget("philosophy source deadline"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_legacy_names_match_independent_hashlib_vectors() {
        for (value, expected) in [
            (String::new(), "da39a3ee5e6b4b0d3255bfef95601890afd80709"),
            ("abc".to_owned(), "a9993e364706816aba3e25717850c26c9cd0d89d"),
            ("a".repeat(55), "c1c8bbdc22796e28c0e15163d20899b65621d65a"),
            ("a".repeat(56), "c2db330f6083854c99d4b5bfb6e8f29f201be699"),
            ("a".repeat(64), "0098ba824b5c16427bd7a1122a5a442a25ec644d"),
            (
                "Источник\0cluster".to_owned(),
                "fb9fc85ee557cbabb1cd26f1b6aa103f3501ba48",
            ),
        ] {
            assert_eq!(sha1_hex(&value), expected);
        }
    }
    #[test]
    fn unsupported_source_representation_is_never_rewritten() {
        assert!(parse(br#"{"value":"\ud800"}"#, 4096).is_err());
        assert!(parse(br#"{"value":1,"value":2}"#, 4096).is_err());
        assert_eq!(
            parse(br#"{"nested":{"unknown":[true,1]}}"#, 4096).unwrap()["nested"]["unknown"][0],
            true
        );
    }

    #[test]
    fn legacy_consumer_decoding_keeps_strict_publication_separate() {
        let raw = br#"{"value":1,"value":2}"#;
        assert!(parse(raw, 4096).is_err());
        assert_eq!(
            parse_with_profile(
                raw,
                4096,
                PhilosophySourceReadProfile::LegacyPythonJsonLoads
            )
            .unwrap()["value"],
            2,
        );
        assert!(
            parse_with_profile(
                br#"{"value":"\ud800"}"#,
                4096,
                PhilosophySourceReadProfile::LegacyPythonJsonLoads,
            )
            .is_err()
        );
    }

    #[test]
    fn arbitrary_precision_integer_round_trips_without_rewrite() {
        let raw = br#"{"value":1844674407370955161601}"#;
        let parsed = parse(raw, 4096).unwrap();
        assert_eq!(
            parsed["value"].as_number().unwrap().to_string(),
            "1844674407370955161601"
        );
        assert_eq!(bytes(&parsed, 4096).unwrap(), canonical(raw, 4096).unwrap());
    }
}

/// Python str.strip/split/re whitespace includes these four C0 separators.
pub(crate) fn source_space(c: char) -> bool {
    c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}')
}
