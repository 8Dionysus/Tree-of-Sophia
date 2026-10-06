use std::collections::{BTreeMap, HashMap, HashSet};

use crate::digest::Digest256;
use crate::error::{FoundationError, FoundationErrorCode as Code, Result};

pub const FORMAT_VERSION: &str = "tos_foundation_json_v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JsonMode {
    /// Published source and generated carrier input: decoded duplicate names fail.
    PublishedStrict,
    /// Legacy request/cursor input: last value wins at the first key position.
    RequestLastWins,
    /// Explicit legacy Python input compatibility. Retains nonfinite float
    /// lexemes; this does not make them publishable or canonically encodable.
    LegacyPythonObserved,
}

impl JsonMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PublishedStrict => "tos_published_json_v1",
            Self::RequestLastWins => "tos_request_last_wins_json_v1",
            Self::LegacyPythonObserved => "tos_legacy_python_observed_json_v1",
        }
    }

    pub fn from_profile(profile: &str) -> Result<Self> {
        match profile {
            "tos_published_json_v1" => Ok(Self::PublishedStrict),
            "tos_request_last_wins_json_v1" => Ok(Self::RequestLastWins),
            "tos_legacy_python_observed_json_v1" => Ok(Self::LegacyPythonObserved),
            _ => Err(FoundationError::new(
                Code::UnsupportedFormat,
                "unknown JSON parse profile",
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JsonLimits {
    pub max_bytes: usize,
    pub max_depth: usize,
    pub max_visits: usize,
    pub max_integer_digits: usize,
}

impl Default for JsonLimits {
    fn default() -> Self {
        Self {
            max_bytes: 1_048_576,
            max_depth: 64,
            max_visits: 300_000,
            max_integer_digits: 4_300,
        }
    }
}

impl JsonLimits {
    pub fn new(
        max_bytes: usize,
        max_depth: usize,
        max_visits: usize,
        max_integer_digits: usize,
    ) -> Result<Self> {
        let limits = Self {
            max_bytes,
            max_depth,
            max_visits,
            max_integer_digits,
        };
        limits.validate()?;
        Ok(limits)
    }

    fn validate(self) -> Result<()> {
        if self.max_bytes == 0
            || self.max_depth == 0
            || self.max_visits == 0
            || self.max_integer_digits == 0
        {
            return Err(FoundationError::new(
                Code::BudgetExceeded,
                "JSON limits must be positive",
            ));
        }
        // Parsing and emission recurse once per container. A caller-controlled
        // usize depth must not turn a bounded codec into a stack overflow.
        if self.max_depth > 128 {
            return Err(FoundationError::new(
                Code::BudgetExceeded,
                "JSON depth limit exceeds safe maximum",
            ));
        }
        Ok(())
    }
}

/// WTF-16 retains legacy escaped lone surrogates without pretending they are UTF-8.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct JsonString {
    units: Vec<u16>,
    utf8: Option<String>,
}

impl JsonString {
    pub fn from_utf8(value: &str) -> Self {
        Self {
            units: value.encode_utf16().collect(),
            utf8: Some(value.to_owned()),
        }
    }

    fn from_units(units: Vec<u16>) -> Self {
        let utf8 = String::from_utf16(&units).ok();
        Self { units, utf8 }
    }

    pub fn as_str(&self) -> Option<&str> {
        self.utf8.as_deref()
    }
    pub fn units(&self) -> &[u16] {
        &self.units
    }
    /// Retained heap storage of the two existing buffers, excluding the
    /// inline JsonString slot charged by the owning container.
    pub fn retained_storage_bytes(&self) -> Result<usize> {
        self.units
            .capacity()
            .checked_mul(std::mem::size_of::<u16>())
            .and_then(|bytes| bytes.checked_add(self.utf8.as_ref().map_or(0, String::capacity)))
            .ok_or_else(|| {
                FoundationError::new(
                    Code::BudgetExceeded,
                    "JSON string retained storage overflow",
                )
            })
    }
    pub fn has_lone_surrogate(&self) -> bool {
        self.utf8.is_none()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JsonNumberKind {
    Int,
    Float,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsonNumber {
    pub kind: JsonNumberKind,
    pub lexeme: String,
}

impl JsonNumber {
    /// Python float observation for an explicitly decoded legacy value.
    /// NaN remains non-reflexive; structural `JsonValue` equality is not Python
    /// numeric equality. Exact identity still belongs to original input bytes.
    pub fn as_python_float(&self) -> Option<f64> {
        if self.kind != JsonNumberKind::Float {
            return None;
        }
        match self.lexeme.as_str() {
            "NaN" => Some(f64::NAN),
            "Infinity" => Some(f64::INFINITY),
            "-Infinity" => Some(f64::NEG_INFINITY),
            _ => self.lexeme.parse().ok(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(JsonNumber),
    String(JsonString),
    Array(Vec<JsonValue>),
    Object(Vec<(JsonString, JsonValue)>),
}

impl JsonValue {
    /// Logical retained storage of this value's owned buffers. The inline root
    /// slot belongs to its enclosing owner; array/object slots are charged once
    /// at actual Vec capacity, including spare capacity. Borrowed aliases do not
    /// create another charge. This is not allocator/RSS accounting.
    pub fn retained_storage_bytes(&self) -> Result<usize> {
        fn add(left: usize, right: usize) -> Result<usize> {
            left.checked_add(right).ok_or_else(|| {
                FoundationError::new(Code::BudgetExceeded, "JSON retained storage overflow")
            })
        }
        fn slots<T>(capacity: usize) -> Result<usize> {
            capacity
                .checked_mul(std::mem::size_of::<T>())
                .ok_or_else(|| {
                    FoundationError::new(
                        Code::BudgetExceeded,
                        "JSON retained container storage overflow",
                    )
                })
        }
        match self {
            Self::Null | Self::Bool(_) => Ok(0),
            Self::Number(number) => Ok(number.lexeme.capacity()),
            Self::String(value) => value.retained_storage_bytes(),
            Self::Array(values) => {
                let mut bytes = slots::<JsonValue>(values.capacity())?;
                for value in values {
                    bytes = add(bytes, value.retained_storage_bytes()?)?;
                }
                Ok(bytes)
            }
            Self::Object(entries) => {
                let mut bytes = slots::<(JsonString, JsonValue)>(entries.capacity())?;
                for (key, value) in entries {
                    bytes = add(bytes, key.retained_storage_bytes()?)?;
                    bytes = add(bytes, value.retained_storage_bytes()?)?;
                }
                Ok(bytes)
            }
        }
    }

    pub fn as_object(&self) -> Option<&[(JsonString, JsonValue)]> {
        match self {
            Self::Object(entries) => Some(entries),
            _ => None,
        }
    }
    pub fn object_get(&self, name: &str) -> Option<&Self> {
        let entries = self.as_object()?;
        let units: Vec<u16> = name.encode_utf16().collect();
        entries
            .iter()
            .find(|(key, _)| key.units == units)
            .map(|(_, value)| value)
    }
    pub fn as_array(&self) -> Option<&[Self]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => value.as_str(),
            _ => None,
        }
    }
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Self::Number(JsonNumber {
                kind: JsonNumberKind::Int,
                lexeme,
            }) if !lexeme.starts_with('-') => lexeme.parse().ok(),
            _ => None,
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// Feed a top-level array field to the maintained canonical digest visitor
    /// without materializing its items in the `JsonValue` tree. `self` must
    /// contain the named field as an empty array placeholder. The feed callback
    /// supplies one already-bounded item at a time through the provided writer;
    /// that writer applies the same escaping, ordering, structural visit, and
    /// output-byte rules as `canonical_feed_digest_v1`.
    pub fn canonical_feed_digest_v1_with_streamed_array<F>(
        &self,
        profile: CanonicalProfile,
        limits: JsonLimits,
        array_field: &str,
        feed: F,
        hasher: &mut crate::Digest256Hasher,
        written: &mut usize,
        visits: &mut usize,
        depth: usize,
    ) -> Result<()>
    where
        F: FnMut(&mut dyn FnMut(&JsonValue) -> Result<()>) -> Result<()>,
    {
        limits.validate()?;
        let style = if profile == CanonicalProfile::CorpusSnapshotV1 {
            WriteStyle::PythonCompactLf
        } else {
            WriteStyle::PythonCompact
        };
        let mut output = JsonOutput {
            sink: JsonSink::Digest {
                hasher,
                bytes: *written,
            },
            poll: JsonCheck::new(None),
        };
        write_root_with_streamed_array(
            self,
            array_field,
            feed,
            &mut output,
            depth,
            visits,
            limits,
            style,
        )?;
        if style.newline() {
            emit(&mut output, b"\n", limits)?;
        }
        output.poll.now()?;
        *written = output.len();
        Ok(())
    }

    /// Make a new top-level object without exactly one named member.
    pub fn without_top_field(&self, name: &str) -> Result<Self> {
        let entries = self
            .as_object()
            .ok_or_else(|| FoundationError::new(Code::InvalidJson, "expected top-level object"))?;
        let units: Vec<u16> = name.encode_utf16().collect();
        let mut removed = 0;
        let retained = entries
            .iter()
            .filter_map(|(key, value)| {
                if key.units == units {
                    removed += 1;
                    None
                } else {
                    Some((key.clone(), value.clone()))
                }
            })
            .collect();
        if removed != 1 {
            return Err(FoundationError::new(
                Code::InvalidJson,
                "top-level member not found exactly once",
            ));
        }
        Ok(Self::Object(retained))
    }
}

#[derive(Clone, Debug)]
pub struct JsonDocument {
    root: JsonValue,
    mode: JsonMode,
    visits: usize,
}

impl PartialEq for JsonDocument {
    fn eq(&self, other: &Self) -> bool {
        self.root == other.root && self.mode == other.mode
    }
}

impl Eq for JsonDocument {}

impl JsonDocument {
    pub fn root(&self) -> &JsonValue {
        &self.root
    }
    pub fn into_root(self) -> JsonValue {
        self.root
    }
    pub fn mode(&self) -> JsonMode {
        self.mode
    }
    /// Number of successful value visits performed by the existing parser.
    /// Reading this count does not traverse or reparse the retained tree.
    pub fn visits(&self) -> usize {
        self.visits
    }
}

pub fn parse_json(raw: &[u8], mode: JsonMode, limits: JsonLimits) -> Result<JsonDocument> {
    parse_json_inner(raw, mode, limits, None, None)
}

/// The legacy Item decoder supplies its remaining logical parser workspace.
/// This uses the same grammar; allocator overhead and RSS remain separate.
pub fn parse_json_with_state_budget(
    raw: &[u8],
    mode: JsonMode,
    limits: JsonLimits,
    available: usize,
) -> Result<JsonDocument> {
    parse_json_inner(raw, mode, limits, Some((0, available)), None)
}

/// Same parser with both the original logical state remainder and caller cutoff.
pub fn parse_json_with_state_budget_and_check(
    raw: &[u8],
    mode: JsonMode,
    limits: JsonLimits,
    available: usize,
    check: &mut dyn FnMut() -> Result<()>,
) -> Result<JsonDocument> {
    parse_json_inner(raw, mode, limits, Some((0, available)), Some(check))
}

/// Cooperatively check caller cancellation/deadline through the existing parser.
/// Callback errors are returned unchanged. This does not change the parse profile.
pub fn parse_json_with_check(
    raw: &[u8],
    mode: JsonMode,
    limits: JsonLimits,
    check: &mut dyn FnMut() -> Result<()>,
) -> Result<JsonDocument> {
    parse_json_inner(raw, mode, limits, None, Some(check))
}

fn parse_json_inner(
    raw: &[u8],
    mode: JsonMode,
    limits: JsonLimits,
    state: Option<(usize, usize)>,
    check: Option<&mut dyn FnMut() -> Result<()>>,
) -> Result<JsonDocument> {
    parse_json_inner_with_admission(raw, mode, limits, state, check, None)
}

fn parse_json_inner_with_admission<'callback>(
    raw: &[u8],
    mode: JsonMode,
    limits: JsonLimits,
    state: Option<(usize, usize)>,
    check: Option<&'callback mut dyn FnMut() -> Result<()>>,
    admit: Option<&'callback mut dyn FnMut(usize, usize) -> Result<()>>,
) -> Result<JsonDocument> {
    limits.validate()?;
    if raw.len() > limits.max_bytes {
        return Err(FoundationError::new(
            Code::BudgetExceeded,
            "JSON byte budget exceeded",
        ));
    }
    let mut poll = JsonCheck::with_admission(check, admit);
    poll.now()?;
    // Preserve pre-grammar UTF-8 rejection and the exact first-invalid offset.
    // Extending a truncated scalar by at most three bytes avoids unsafe str conversion.
    let mut at = 0;
    while at < raw.len() {
        poll.now()?;
        let mut end = at.saturating_add(CHECK_BYTES).min(raw.len());
        poll.admit_work(end - at, 0)?;
        let valid = match std::str::from_utf8(&raw[at..end]) {
            Err(error) if error.error_len().is_none() && end < raw.len() => {
                let scalar = at + error.valid_up_to();
                let width = match raw[scalar] {
                    0xc2..=0xdf => 2,
                    0xe0..=0xef => 3,
                    _ => 4,
                };
                end = scalar.saturating_add(width).min(raw.len());
                poll.admit_work(end - at, 0)?;
                std::str::from_utf8(&raw[at..end])
            }
            result => result,
        };
        valid.map_err(|error| {
            FoundationError::new(Code::InvalidUtf8, "JSON input is not UTF-8")
                .at(at + error.valid_up_to())
        })?;
        at = end;
    }
    let mut parser = Parser {
        raw,
        at: 0,
        visits: 0,
        mode,
        limits,
        state,
        poll,
        next_check: 0,
    };
    // Recursive keys and values temporarily coexist with container indexes.
    // Their fixed stack slots are priced once, independently of node count.
    parser.charge(
        (limits.max_depth + 1)
            .checked_mul(
                std::mem::size_of::<JsonValue>()
                    + std::mem::size_of::<JsonString>()
                    + std::mem::size_of::<HashMap<Vec<u16>, usize>>()
                    + 2 * std::mem::size_of::<Vec<JsonValue>>(),
            )
            .ok_or_else(|| {
                parser.error(Code::BudgetExceeded, "JSON parser state budget exceeded")
            })?,
    )?;
    let root = parser.value(0)?;
    parser.spaces()?;
    if parser.at != raw.len() {
        return Err(parser.error(Code::InvalidJson, "trailing JSON input"));
    }
    parser.poll.now()?;
    let visits = parser.visits;
    Ok(JsonDocument { root, mode, visits })
}

pub fn parse_json_profile(raw: &[u8], profile: &str, limits: JsonLimits) -> Result<JsonDocument> {
    parse_json(raw, JsonMode::from_profile(profile)?, limits)
}

struct Parser<'a, 'c> {
    raw: &'a [u8],
    at: usize,
    visits: usize,
    state: Option<(usize, usize)>,
    mode: JsonMode,
    limits: JsonLimits,
    poll: JsonCheck<'c>,
    next_check: usize,
}

impl Parser<'_, '_> {
    fn check(&mut self) -> Result<()> {
        if self.at >= self.next_check {
            self.poll.now()?;
            self.next_check = self.at.saturating_add(CHECK_BYTES);
        }
        Ok(())
    }
    fn charge(&mut self, amount: usize) -> Result<()> {
        if let Some((used, available)) = &mut self.state {
            *used = used
                .checked_add(amount)
                .filter(|n| *n <= *available)
                .ok_or_else(|| {
                    FoundationError::new(Code::BudgetExceeded, "JSON parser state budget exceeded")
                })?;
        }
        Ok(())
    }
    fn reserve<T>(&mut self, values: &mut Vec<T>, additional: usize) -> Result<()> {
        if self.state.is_none() {
            return Ok(());
        }
        let needed = values
            .len()
            .checked_add(additional)
            .ok_or_else(|| self.error(Code::BudgetExceeded, "JSON parser state budget exceeded"))?;
        if needed > values.capacity() {
            let capacity = needed.max(values.capacity().saturating_mul(2)).max(4);
            self.charge(
                capacity
                    .checked_sub(values.capacity())
                    .and_then(|n| n.checked_mul(std::mem::size_of::<T>()))
                    .ok_or_else(|| {
                        self.error(Code::BudgetExceeded, "JSON parser state budget exceeded")
                    })?,
            )?;
            values
                .try_reserve_exact(capacity - values.len())
                .map_err(|_| {
                    self.error(Code::BudgetExceeded, "JSON parser state budget exceeded")
                })?;
        }
        Ok(())
    }
    fn units_push(&mut self, units: &mut Vec<u16>, unit: u16) -> Result<()> {
        self.reserve(units, 1)?;
        units.push(unit);
        Ok(())
    }
    fn finish_string(&mut self, units: Vec<u16>) -> Result<JsonString> {
        if self.state.is_none() && self.poll.check.is_none() {
            return Ok(JsonString::from_units(units));
        }
        let mut length = Some(0usize);
        for (index, c) in char::decode_utf16(units.iter().copied()).enumerate() {
            if index % 4096 == 0 {
                self.poll.now()?;
            }
            length = length.and_then(|n| n.checked_add(c.ok()?.len_utf8()));
        }
        let utf8 = if let Some(length) = length {
            self.charge(length)?;
            let mut text = String::new();
            text.try_reserve_exact(length).map_err(|_| {
                self.error(Code::BudgetExceeded, "JSON parser state budget exceeded")
            })?;
            for (index, c) in char::decode_utf16(units.iter().copied()).enumerate() {
                if index % 4096 == 0 {
                    self.poll.now()?;
                }
                text.push(c.map_err(|_| {
                    self.error(Code::InvalidUnicodeScalar, "decoded string changed")
                })?);
            }
            Some(text)
        } else {
            None
        };
        Ok(JsonString { units, utf8 })
    }
    fn error(&self, code: Code, detail: &'static str) -> FoundationError {
        FoundationError::new(code, detail).at(self.at)
    }
    fn spaces(&mut self) -> Result<()> {
        while matches!(self.raw.get(self.at), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.at += 1;
            self.check()?;
        }
        Ok(())
    }
    fn value(&mut self, depth: usize) -> Result<JsonValue> {
        self.check()?;
        if depth > self.limits.max_depth || self.visits >= self.limits.max_visits {
            return Err(self.error(Code::BudgetExceeded, "JSON structural budget exceeded"));
        }
        self.poll.admit_work(0, 1)?;
        self.visits += 1;
        self.spaces()?;
        match self.raw.get(self.at) {
            Some(b'"') => Ok(JsonValue::String(self.string()?)),
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b't') => {
                self.literal(b"true")?;
                Ok(JsonValue::Bool(true))
            }
            Some(b'f') => {
                self.literal(b"false")?;
                Ok(JsonValue::Bool(false))
            }
            Some(b'n') => {
                self.literal(b"null")?;
                Ok(JsonValue::Null)
            }
            Some(b'N' | b'I') if self.mode == JsonMode::LegacyPythonObserved => {
                self.python_constant()
            }
            Some(b'-')
                if self.mode == JsonMode::LegacyPythonObserved
                    && self.raw.get(self.at + 1) == Some(&b'I') =>
            {
                self.python_constant()
            }
            Some(b'-' | b'0'..=b'9') => Ok(JsonValue::Number(self.number()?)),
            _ => Err(self.error(Code::InvalidJson, "expected JSON value")),
        }
    }
    fn python_constant(&mut self) -> Result<JsonValue> {
        let expected: &[u8] = match self.raw[self.at] {
            b'N' => b"NaN",
            b'I' => b"Infinity",
            b'-' => b"-Infinity",
            _ => unreachable!(),
        };
        // Charge retained lexeme bytes before allocating, as for finite numbers.
        self.literal(expected)?;
        self.charge(expected.len())?;
        Ok(JsonValue::Number(JsonNumber {
            kind: JsonNumberKind::Float,
            lexeme: std::str::from_utf8(expected).unwrap().to_owned(),
        }))
    }

    fn literal(&mut self, expected: &[u8]) -> Result<()> {
        if self.raw.get(self.at..self.at + expected.len()) != Some(expected) {
            return Err(self.error(Code::InvalidJson, "invalid JSON literal"));
        }
        self.at += expected.len();
        Ok(())
    }
    fn take(&mut self, expected: u8) -> Result<()> {
        if self.raw.get(self.at) != Some(&expected) {
            return Err(self.error(Code::InvalidJson, "unexpected JSON token"));
        }
        self.at += 1;
        Ok(())
    }
    fn string(&mut self) -> Result<JsonString> {
        self.take(b'"')?;
        let mut units = Vec::new();
        loop {
            self.check()?;
            let byte = *self
                .raw
                .get(self.at)
                .ok_or_else(|| self.error(Code::InvalidJson, "unterminated JSON string"))?;
            match byte {
                b'"' => {
                    self.at += 1;
                    return self.finish_string(units);
                }
                b'\\' => {
                    self.at += 1;
                    let escaped = *self
                        .raw
                        .get(self.at)
                        .ok_or_else(|| self.error(Code::InvalidJson, "incomplete JSON escape"))?;
                    self.at += 1;
                    match escaped {
                        b'"' | b'\\' | b'/' => self.units_push(&mut units, escaped as u16)?,
                        b'b' => self.units_push(&mut units, 8)?,
                        b'f' => self.units_push(&mut units, 12)?,
                        b'n' => self.units_push(&mut units, 10)?,
                        b'r' => self.units_push(&mut units, 13)?,
                        b't' => self.units_push(&mut units, 9)?,
                        b'u' => {
                            let hex = self.raw.get(self.at..self.at + 4).ok_or_else(|| {
                                self.error(Code::InvalidJson, "short Unicode escape")
                            })?;
                            let mut unit = 0u16;
                            for &digit in hex {
                                unit = (unit << 4)
                                    | match digit {
                                        b'0'..=b'9' => (digit - b'0') as u16,
                                        b'a'..=b'f' => (digit - b'a' + 10) as u16,
                                        b'A'..=b'F' => (digit - b'A' + 10) as u16,
                                        _ => {
                                            return Err(self.error(
                                                Code::InvalidJson,
                                                "invalid Unicode escape",
                                            ));
                                        }
                                    };
                            }
                            self.at += 4;
                            self.units_push(&mut units, unit)?;
                        }
                        _ => return Err(self.error(Code::InvalidJson, "invalid JSON escape")),
                    }
                }
                0..=31 => {
                    return Err(self.error(Code::InvalidJson, "unescaped control in JSON string"));
                }
                _ => {
                    let width = match byte {
                        0..=0x7f => 1,
                        0xc2..=0xdf => 2,
                        0xe0..=0xef => 3,
                        _ => 4,
                    };
                    // The bounded preflight already validated this exact scalar.
                    let ch = std::str::from_utf8(&self.raw[self.at..self.at + width])
                        .map_err(|_| self.error(Code::InvalidUtf8, "JSON input is not UTF-8"))?
                        .chars()
                        .next()
                        .ok_or_else(|| self.error(Code::InvalidJson, "invalid JSON string"))?;
                    for &unit in ch.encode_utf16(&mut [0u16; 2]).iter() {
                        self.units_push(&mut units, unit)?;
                    }
                    self.at += ch.len_utf8();
                }
            }
        }
    }
    fn object(&mut self, depth: usize) -> Result<JsonValue> {
        self.take(b'{')?;
        self.spaces()?;
        let mut entries: Vec<(JsonString, JsonValue)> = Vec::new();
        let mut positions: HashMap<Vec<u16>, usize> = HashMap::new();
        let mut checked_positions: BTreeMap<Digest256, usize> = BTreeMap::new();
        let mut checked_next: Vec<usize> = Vec::new();
        let mut index_capacity_charge = 0usize;
        if self.raw.get(self.at) == Some(&b'}') {
            self.at += 1;
            return Ok(JsonValue::Object(entries));
        }
        loop {
            self.spaces()?;
            if self.raw.get(self.at) != Some(&b'"') {
                return Err(self.error(Code::InvalidJson, "object key must be string"));
            }
            let key = self.string()?;
            self.spaces()?;
            self.take(b':')?;
            let value = self.value(depth + 1)?;
            let checked_digest = if self.poll.check.is_some() {
                Some(checked_units_digest(&key.units, &mut self.poll)?)
            } else {
                None
            };
            let position = if let Some(digest) = checked_digest {
                let mut position = None;
                let mut candidate = checked_positions
                    .get(&digest)
                    .copied()
                    .unwrap_or(usize::MAX);
                while candidate != usize::MAX {
                    self.poll.work(256)?;
                    if checked_units_equal(&key.units, &entries[candidate].0.units, &mut self.poll)?
                    {
                        position = Some(candidate);
                        break;
                    }
                    candidate = checked_next[candidate];
                }
                position
            } else {
                positions.get(&key.units).copied()
            };
            if let Some(position) = position {
                if self.mode == JsonMode::PublishedStrict {
                    return Err(self.error(Code::DuplicateMember, "duplicate decoded JSON member"));
                }
                entries[position].1 = value;
            } else {
                if self.state.is_some() {
                    if positions.len() == positions.capacity() {
                        // Pinned std HashMap grows in power-of-two buckets;
                        // admit a conservative bucket-slot ceiling before it.
                        let slots = positions
                            .len()
                            .checked_add(1)
                            .and_then(|n| n.max(4).checked_next_power_of_two())
                            .and_then(|n| n.checked_mul(2))
                            .ok_or_else(|| {
                                self.error(
                                    Code::BudgetExceeded,
                                    "JSON parser state budget exceeded",
                                )
                            })?;
                        self.charge(
                            slots
                                .saturating_sub(index_capacity_charge)
                                .checked_mul(std::mem::size_of::<(Vec<u16>, usize)>() + 1)
                                // The pinned table also retains a trailing SIMD control group.
                                .and_then(|n| {
                                    n.checked_add(if index_capacity_charge == 0 { 16 } else { 0 })
                                })
                                .ok_or_else(|| {
                                    self.error(
                                        Code::BudgetExceeded,
                                        "JSON parser state budget exceeded",
                                    )
                                })?,
                        )?;
                        index_capacity_charge = slots;
                        positions.try_reserve(1).map_err(|_| {
                            self.error(Code::BudgetExceeded, "JSON parser state budget exceeded")
                        })?;
                    }
                    self.charge(
                        key.units
                            .len()
                            .checked_mul(std::mem::size_of::<u16>())
                            .ok_or_else(|| {
                                self.error(
                                    Code::BudgetExceeded,
                                    "JSON parser state budget exceeded",
                                )
                            })?,
                    )?;
                }
                self.reserve(&mut entries, 1)?;
                if let Some(digest) = checked_digest {
                    let previous = checked_positions
                        .insert(digest, entries.len())
                        .unwrap_or(usize::MAX);
                    checked_next.push(previous);
                } else {
                    positions.insert(key.units.clone(), entries.len());
                }
                entries.push((key, value));
            }
            self.spaces()?;
            match self.raw.get(self.at) {
                Some(b',') => {
                    self.at += 1;
                }
                Some(b'}') => {
                    self.at += 1;
                    return Ok(JsonValue::Object(entries));
                }
                _ => return Err(self.error(Code::InvalidJson, "expected object comma or close")),
            }
        }
    }
    fn array(&mut self, depth: usize) -> Result<JsonValue> {
        self.take(b'[')?;
        self.spaces()?;
        let mut items = Vec::new();
        if self.raw.get(self.at) == Some(&b']') {
            self.at += 1;
            return Ok(JsonValue::Array(items));
        }
        loop {
            let value = self.value(depth + 1)?;
            self.reserve(&mut items, 1)?;
            items.push(value);
            self.spaces()?;
            match self.raw.get(self.at) {
                Some(b',') => {
                    self.at += 1;
                }
                Some(b']') => {
                    self.at += 1;
                    return Ok(JsonValue::Array(items));
                }
                _ => return Err(self.error(Code::InvalidJson, "expected array comma or close")),
            }
        }
    }
    fn number(&mut self) -> Result<JsonNumber> {
        let start = self.at;
        if self.raw[self.at] == b'-' {
            self.at += 1;
        }
        let integer_start = self.at;
        match self.raw.get(self.at) {
            Some(b'0') => {
                self.at += 1;
                if matches!(self.raw.get(self.at), Some(b'0'..=b'9')) {
                    return Err(self.error(Code::InvalidNumber, "leading zero"));
                }
            }
            Some(b'1'..=b'9') => {
                while matches!(self.raw.get(self.at), Some(b'0'..=b'9')) {
                    self.check()?;
                    self.at += 1;
                }
            }
            _ => return Err(self.error(Code::InvalidNumber, "missing integer digits")),
        }
        let integer_digits = self.at - integer_start;
        let mut kind = JsonNumberKind::Int;
        if self.raw.get(self.at) == Some(&b'.') {
            kind = JsonNumberKind::Float;
            self.at += 1;
            let fraction_start = self.at;
            while matches!(self.raw.get(self.at), Some(b'0'..=b'9')) {
                self.check()?;
                self.at += 1;
            }
            if self.at == fraction_start {
                return Err(self.error(Code::InvalidNumber, "missing fraction digits"));
            }
        }
        if matches!(self.raw.get(self.at), Some(b'e' | b'E')) {
            kind = JsonNumberKind::Float;
            self.at += 1;
            if matches!(self.raw.get(self.at), Some(b'+' | b'-')) {
                self.at += 1;
            }
            let exponent_start = self.at;
            while matches!(self.raw.get(self.at), Some(b'0'..=b'9')) {
                self.check()?;
                self.at += 1;
            }
            if self.at == exponent_start {
                return Err(self.error(Code::InvalidNumber, "missing exponent digits"));
            }
        }
        self.charge(self.at - start)?;
        let mut lexeme = String::new();
        for chunk in self.raw[start..self.at].chunks(CHECK_BYTES) {
            self.poll.now()?;
            // Number grammar admits only ASCII here.
            lexeme.push_str(
                std::str::from_utf8(chunk)
                    .map_err(|_| self.error(Code::InvalidUtf8, "JSON input is not UTF-8"))?,
            );
        }
        if kind == JsonNumberKind::Int && integer_digits > self.limits.max_integer_digits {
            return Err(self.error(Code::BudgetExceeded, "integer digit budget exceeded"));
        }
        if kind == JsonNumberKind::Float
            && self.mode != JsonMode::LegacyPythonObserved
            && !lexeme.parse::<f64>().is_ok_and(f64::is_finite)
        {
            return Err(self.error(Code::NonfiniteFloat, "nonfinite or unrepresentable float"));
        }
        Ok(JsonNumber { kind, lexeme })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalProfile {
    /// `scripts/corpus_store.py` v1: sorted compact JSON plus one LF.
    CorpusSnapshotV1,
    /// `knowledge_assessment.py` source-record digest: sorted compact JSON, no LF.
    SourceRecordDigestV1,
    /// Legacy `source_commands.py` request identity: the same bytes as the
    /// source-record digest, but a distinct owner contract and lifetime.
    SourceCommandInputV1,
}

impl CanonicalProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CorpusSnapshotV1 => "tos_corpus_snapshot_canonical_v1",
            Self::SourceRecordDigestV1 => "tos_source_record_digest_v1",
            Self::SourceCommandInputV1 => "tos_source_command_input_v1",
        }
    }
    pub const fn supports_float(self) -> bool {
        true
    }
    pub fn from_profile(profile: &str) -> Result<Self> {
        match profile {
            "tos_corpus_snapshot_canonical_v1" => Ok(Self::CorpusSnapshotV1),
            "tos_source_record_digest_v1" => Ok(Self::SourceRecordDigestV1),
            "tos_source_command_input_v1" => Ok(Self::SourceCommandInputV1),
            _ => Err(FoundationError::new(
                Code::UnsupportedFormat,
                "unknown canonical profile",
            )),
        }
    }
}

pub fn emit_preserved_json(document: &JsonDocument, limits: JsonLimits) -> Result<Vec<u8>> {
    emit_value_preserved_json(document.root(), limits)
}

/// Bounded compact emission for an explicitly constructed ordered value.
/// This retains numeric lexemes; it does not select a transport response ABI.
pub fn emit_value_preserved_json(value: &JsonValue, limits: JsonLimits) -> Result<Vec<u8>> {
    write_document(value, limits, WriteStyle::PreservedCompact)
}

/// Published packet framing: Python compact JSON with insertion-ordered object
/// members, Python numeric spelling and no final line feed. This selects only
/// bytes; it does not grant publication or change canonical digest profiles.
pub fn emit_python_compact_json(value: &JsonValue, limits: JsonLimits) -> Result<Vec<u8>> {
    write_document(value, limits, WriteStyle::PythonPublishedCompact)
}

/// Insertion-ordered Python compact bytes under the original owner's state,
/// cooperative cutoff and aggregate visit/work allowance. `admit` runs before
/// each output-sink byte chunk and node visit in both passes. Numeric
/// validation additionally admits its input-byte windows before inspection.
/// Successfully admitted prefixes stay charged even on later failure.
pub fn emit_python_compact_json_with_state_budget_and_visits_and_check(
    value: &JsonValue,
    limits: JsonLimits,
    available: usize,
    check: &mut dyn FnMut() -> Result<()>,
    admit: &mut dyn FnMut(usize, usize) -> Result<()>,
) -> Result<(Vec<u8>, usize)> {
    write_with_state_budget_and_visits(
        value,
        WriteStyle::PythonPublishedCompact,
        limits,
        available,
        Some(check),
        Some(admit),
        true,
    )
}

/// Produce owner-profile bytes using Python's sorted compact JSON spelling.
/// Only `CorpusSnapshotV1` includes a final line feed.
pub fn canonical_bytes_v1(
    value: &JsonValue,
    profile: CanonicalProfile,
    limits: JsonLimits,
) -> Result<Vec<u8>> {
    match profile {
        CanonicalProfile::CorpusSnapshotV1 => {
            write_document(value, limits, WriteStyle::PythonCompactLf)
        }
        CanonicalProfile::SourceRecordDigestV1 | CanonicalProfile::SourceCommandInputV1 => {
            write_document(value, limits, WriteStyle::PythonCompact)
        }
    }
}

/// The same canonical byte writer with cooperative checks, unchanged profiles.
pub fn canonical_bytes_v1_with_check(
    value: &JsonValue,
    profile: CanonicalProfile,
    limits: JsonLimits,
    check: &mut dyn FnMut() -> Result<()>,
) -> Result<Vec<u8>> {
    let style = if profile == CanonicalProfile::CorpusSnapshotV1 {
        WriteStyle::PythonCompactLf
    } else {
        WriteStyle::PythonCompact
    };
    let mut bytes = Vec::new();
    let mut output = JsonOutput::bytes(&mut bytes);
    output.poll = JsonCheck::new(Some(check));
    write_document_into(value, limits, style, &mut output)?;
    Ok(bytes)
}

/// Same canonical visitor with an original remaining-state output reservation.
/// Counting retains no output. No growing response buffer is permitted after
/// admission; the resulting allocation's actual capacity is checked as well.
pub fn canonical_bytes_v1_with_state_budget(
    value: &JsonValue,
    profile: CanonicalProfile,
    limits: JsonLimits,
    available: usize,
) -> Result<Vec<u8>> {
    canonical_bytes_v1_with_state_budget_and_visits(value, profile, limits, available).map(|v| v.0)
}

/// Count and emit share the original visit grant as well as state workspace.
pub fn canonical_bytes_v1_with_state_budget_and_visits(
    value: &JsonValue,
    profile: CanonicalProfile,
    limits: JsonLimits,
    available: usize,
) -> Result<(Vec<u8>, usize)> {
    canonical_state_and_visits(value, profile, limits, available, None)
}

/// Same canonical state/visit owner with the caller's ORIGINAL cooperative
/// cutoff/cancellation probe. The caller pre-admits shared work before entry.
pub fn canonical_bytes_v1_with_state_budget_and_visits_and_check(
    value: &JsonValue,
    profile: CanonicalProfile,
    limits: JsonLimits,
    available: usize,
    check: &mut dyn FnMut() -> Result<()>,
) -> Result<(Vec<u8>, usize)> {
    canonical_state_and_visits(value, profile, limits, available, Some(check))
}

fn canonical_state_and_visits(
    value: &JsonValue,
    profile: CanonicalProfile,
    limits: JsonLimits,
    available: usize,
    check: Option<&mut dyn FnMut() -> Result<()>>,
) -> Result<(Vec<u8>, usize)> {
    let style = match profile {
        CanonicalProfile::CorpusSnapshotV1 => WriteStyle::PythonCompactLf,
        CanonicalProfile::SourceRecordDigestV1 | CanonicalProfile::SourceCommandInputV1 => {
            WriteStyle::PythonCompact
        }
    };
    write_with_state_budget_and_visits(value, style, limits, available, check, None, false)
}

/// Resource text: Python ensure_ascii=False, indent=2, sort_keys=True, no LF.
/// Reuses the owner visitor, actual state reservation and original count+emit visits.
/// The owner reserves original declared two-pass work before traversal or sorting.
pub fn emit_python_pretty_sorted_json_with_state_budget(
    value: &JsonValue,
    limits: JsonLimits,
    available: usize,
    check: &mut dyn FnMut() -> Result<()>,
    admit: &mut dyn FnMut(usize, usize) -> Result<()>,
) -> Result<(Vec<u8>, usize)> {
    write_with_state_budget_and_visits(
        value,
        WriteStyle::PythonPretty2Sorted,
        limits,
        available,
        Some(check),
        Some(admit),
        false,
    )
}

fn write_with_state_budget_and_visits(
    value: &JsonValue,
    style: WriteStyle,
    mut limits: JsonLimits,
    available: usize,
    mut check: Option<&mut dyn FnMut() -> Result<()>>,
    mut admit: Option<&mut dyn FnMut(usize, usize) -> Result<()>>,
    incremental_admission: bool,
) -> Result<(Vec<u8>, usize)> {
    let scratch_slots = limits
        .max_depth
        .checked_add(1)
        .and_then(|depth| {
            depth.checked_mul(std::mem::size_of::<Vec<(usize, &(JsonString, JsonValue))>>())
        })
        .and_then(|n| n.checked_add(std::mem::size_of::<String>()))
        .and_then(|n| n.checked_add(std::mem::size_of::<String>()))
        .and_then(|n| n.checked_add(std::mem::size_of::<FormatCount>()))
        .and_then(|n| {
            n.checked_add(if check.is_some() {
                2 * std::mem::size_of::<JsonOutput>()
            } else {
                0
            })
        })
        .ok_or_else(state_error)?;
    if scratch_slots > available {
        return Err(state_error());
    }
    // Preserve the original pretty-render reserve contract. Compact rendering
    // admits each bounded byte/visit prefix before work in both passes.
    if !incremental_admission {
        if let Some(admit) = admit.as_mut() {
            admit(limits.max_bytes, limits.max_visits)?;
        }
    }
    let (count, used) = {
        let mut count_sink = JsonOutput {
            poll: JsonCheck::with_admission(
                check
                    .as_mut()
                    .map(|callback| &mut **callback as &mut dyn FnMut() -> Result<()>),
                if incremental_admission {
                    admit.as_mut().map(|callback| {
                        &mut **callback as &mut dyn FnMut(usize, usize) -> Result<()>
                    })
                } else {
                    None
                },
            ),
            sink: JsonSink::StateCount {
                count: 0,
                available,
                scratch: scratch_slots,
            },
        };
        let (count_visits, count_numeric) =
            write_document_into_with_visits(value, limits, style, &mut count_sink, true)?;
        let used = count_visits
            .checked_add(count_numeric)
            .ok_or_else(state_error)?;
        let count = count_sink.len();
        if count
            > available
                .checked_sub(scratch_slots)
                .ok_or_else(state_error)?
        {
            return Err(state_error());
        }
        (count, used)
    };
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(count).map_err(|_| state_error())?;
    if bytes.capacity()
        > available
            .checked_sub(scratch_slots)
            .ok_or_else(state_error)?
    {
        return Err(state_error());
    }
    limits.max_bytes = limits.max_bytes.min(count);
    limits.max_visits = limits
        .max_visits
        .checked_sub(used)
        .ok_or_else(state_error)?;
    let mut output = JsonOutput {
        poll: JsonCheck::with_admission(
            check
                .as_mut()
                .map(|callback| &mut **callback as &mut dyn FnMut() -> Result<()>),
            if incremental_admission {
                admit
                    .as_mut()
                    .map(|callback| &mut **callback as &mut dyn FnMut(usize, usize) -> Result<()>)
            } else {
                None
            },
        ),
        sink: JsonSink::StateBytes {
            bytes: &mut bytes,
            available,
            scratch: scratch_slots,
        },
    };
    let (emit_visits, emit_numeric) =
        write_document_into_with_visits(value, limits, style, &mut output, true)?;
    if bytes.len() != count {
        return Err(FoundationError::new(
            Code::InvalidJson,
            "canonical count differs",
        ));
    }
    let visits = used
        .checked_add(emit_visits)
        .and_then(|n| n.checked_add(emit_numeric))
        .ok_or_else(state_error)?;
    Ok((bytes, visits))
}

/// Emit the exact bytes from `canonical_bytes_v1` and report the value visits
/// and numeric-lexeme parser visits from that same emission pass.
pub fn canonical_bytes_v1_with_visits(
    value: &JsonValue,
    profile: CanonicalProfile,
    limits: JsonLimits,
) -> Result<(Vec<u8>, usize, usize)> {
    let style = match profile {
        CanonicalProfile::CorpusSnapshotV1 => WriteStyle::PythonCompactLf,
        CanonicalProfile::SourceRecordDigestV1 | CanonicalProfile::SourceCommandInputV1 => {
            WriteStyle::PythonCompact
        }
    };
    let mut bytes = Vec::new();
    let (writer_visits, numeric_parse_visits) = write_document_into_with_visits(
        value,
        limits,
        style,
        &mut JsonOutput::bytes(&mut bytes),
        true,
    )?;
    Ok((bytes, writer_visits, numeric_parse_visits))
}

/// Count canonical bytes through the same closed output visitor without retaining discarded bytes.
pub fn canonical_count_v1(
    value: &JsonValue,
    profile: CanonicalProfile,
    limits: JsonLimits,
) -> Result<usize> {
    let style = match profile {
        CanonicalProfile::CorpusSnapshotV1 => WriteStyle::PythonCompactLf,
        CanonicalProfile::SourceRecordDigestV1 | CanonicalProfile::SourceCommandInputV1 => {
            WriteStyle::PythonCompact
        }
    };
    let mut output = JsonOutput::count();
    write_document_into(value, limits, style, &mut output)?;
    Ok(output.len())
}

/// Feed one bounded canonical fragment to the actual inventory digest sink.
/// Same ordering/number/escape/structural limits as canonical_bytes_v1.
pub fn canonical_feed_digest_v1(
    value: &JsonValue,
    profile: CanonicalProfile,
    limits: JsonLimits,
    hasher: &mut crate::Digest256Hasher,
    written: &mut usize,
    visits: &mut usize,
    depth: usize,
) -> Result<()> {
    canonical_feed_digest_inner(value, profile, limits, hasher, written, visits, depth, None)
}

/// The same canonical digest visitor with a cooperative caller check.
/// On error the external digest sink may contain a partial fragment; discard it.
pub fn canonical_feed_digest_v1_with_check(
    value: &JsonValue,
    profile: CanonicalProfile,
    limits: JsonLimits,
    hasher: &mut crate::Digest256Hasher,
    written: &mut usize,
    visits: &mut usize,
    depth: usize,
    check: &mut dyn FnMut() -> Result<()>,
) -> Result<()> {
    canonical_feed_digest_inner(
        value,
        profile,
        limits,
        hasher,
        written,
        visits,
        depth,
        Some(check),
    )
}

fn canonical_feed_digest_inner(
    value: &JsonValue,
    profile: CanonicalProfile,
    limits: JsonLimits,
    hasher: &mut crate::Digest256Hasher,
    written: &mut usize,
    visits: &mut usize,
    depth: usize,
    check: Option<&mut dyn FnMut() -> Result<()>>,
) -> Result<()> {
    limits.validate()?;
    let mut output = JsonOutput {
        sink: JsonSink::Digest {
            hasher,
            bytes: *written,
        },
        poll: JsonCheck::new(check),
    };
    let style = if profile == CanonicalProfile::CorpusSnapshotV1 {
        WriteStyle::PythonCompactLf
    } else {
        WriteStyle::PythonCompact
    };
    let mut numeric_parse_visits = 0;
    write_value(
        value,
        &mut output,
        depth,
        visits,
        &mut numeric_parse_visits,
        limits,
        style,
        false,
    )?;
    if style.newline() {
        emit(&mut output, b"\n", limits)?;
    }
    output.poll.now()?;
    *written = output.len();
    Ok(())
}

/// Exact published bytes of the legacy public Work/HumanForm set as a whole.
/// The receipt is an embedded field; this is not a standalone receipt codec.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JsonEmissionProfile {
    SourceFormSetPublishedV1,
    SourceWitnessCatalogPublishedV3,
    SourceFoundationLabReportV1,
}

impl JsonEmissionProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SourceFormSetPublishedV1 => "tos_source_form_set_published_v1",
            Self::SourceWitnessCatalogPublishedV3 => "tos_source_witness_catalog_published_v3",
            Self::SourceFoundationLabReportV1 => "tos_source_foundation_lab_report_v1",
        }
    }

    pub fn from_profile(profile: &str) -> Result<Self> {
        match profile {
            "tos_source_form_set_published_v1" => Ok(Self::SourceFormSetPublishedV1),
            "tos_source_witness_catalog_published_v3" => Ok(Self::SourceWitnessCatalogPublishedV3),
            "tos_source_foundation_lab_report_v1" => Ok(Self::SourceFoundationLabReportV1),
            _ => Err(FoundationError::new(
                Code::UnsupportedFormat,
                "unknown JSON emission profile",
            )),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodedJson {
    pub bytes: Vec<u8>,
    pub sha256: Digest256,
}

/// Python `json.dumps(value, ensure_ascii=False, allow_nan=False, indent=2)
/// + '\n'` for a caller-built, insertion-ordered entire form-set value.
pub fn emit_json_profile(
    value: &JsonValue,
    profile: JsonEmissionProfile,
    limits: JsonLimits,
) -> Result<EncodedJson> {
    let bytes = match profile {
        JsonEmissionProfile::SourceFormSetPublishedV1
        | JsonEmissionProfile::SourceWitnessCatalogPublishedV3 => {
            if value.as_object().is_none() {
                return Err(FoundationError::new(
                    Code::InvalidJson,
                    if profile == JsonEmissionProfile::SourceFormSetPublishedV1 {
                        "form set must be a JSON object"
                    } else {
                        "catalog manifest must be a JSON object"
                    },
                ));
            }
            write_document(value, limits, WriteStyle::PythonPretty2Lf)?
        }
        JsonEmissionProfile::SourceFoundationLabReportV1 => {
            if value.as_object().is_none() {
                return Err(FoundationError::new(
                    Code::InvalidJson,
                    "foundation lab report must be a JSON object",
                ));
            }
            write_document(value, limits, WriteStyle::PythonPretty2SortedLf)?
        }
    };
    Ok(EncodedJson {
        sha256: Digest256::of_bytes(&bytes),
        bytes,
    })
}

pub fn canonical_digest_v1(
    value: &JsonValue,
    profile: CanonicalProfile,
    limits: JsonLimits,
) -> Result<Digest256> {
    Ok(Digest256::of_bytes(&canonical_bytes_v1(
        value, profile, limits,
    )?))
}

/// Decode exact raw source/command input with duplicate rejection before
/// canonicalization. This entry point cannot silently collapse a request's
/// duplicate members under `RequestLastWins`.
pub fn canonical_raw_bytes_v1(
    raw: &[u8],
    profile: CanonicalProfile,
    limits: JsonLimits,
) -> Result<Vec<u8>> {
    let document = parse_json(raw, JsonMode::PublishedStrict, limits)?;
    canonical_bytes_v1(document.root(), profile, limits)
}

pub fn canonical_raw_bytes_profile(
    raw: &[u8],
    profile: &str,
    limits: JsonLimits,
) -> Result<Vec<u8>> {
    canonical_raw_bytes_v1(raw, CanonicalProfile::from_profile(profile)?, limits)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WriteStyle {
    PreservedCompact,
    PythonPublishedCompact,
    PythonCompact,
    PythonCompactLf,
    PythonPretty2Lf,
    PythonPretty2Sorted,
    PythonPretty2SortedLf,
}

impl WriteStyle {
    fn sort_keys(self) -> bool {
        matches!(
            self,
            Self::PythonCompact
                | Self::PythonCompactLf
                | Self::PythonPretty2Sorted
                | Self::PythonPretty2SortedLf
        )
    }
    fn python_numbers(self) -> bool {
        self != Self::PreservedCompact
    }
    fn pretty(self) -> bool {
        matches!(
            self,
            Self::PythonPretty2Lf | Self::PythonPretty2Sorted | Self::PythonPretty2SortedLf
        )
    }
    fn newline(self) -> bool {
        matches!(
            self,
            Self::PythonCompactLf | Self::PythonPretty2Lf | Self::PythonPretty2SortedLf
        )
    }
}

const CHECK_BYTES: usize = 65_536;
const CHECK_SHORT_KEY_UNITS: usize = 128;
const CHECK_SMALL_OBJECT_KEYS: usize = 32;
struct JsonCheck<'c> {
    check: Option<&'c mut dyn FnMut() -> Result<()>>,
    admit: Option<&'c mut dyn FnMut(usize, usize) -> Result<()>>,
    work: usize,
}
impl<'c> JsonCheck<'c> {
    fn new(check: Option<&'c mut dyn FnMut() -> Result<()>>) -> Self {
        Self::with_admission(check, None)
    }
    fn with_admission(
        check: Option<&'c mut dyn FnMut() -> Result<()>>,
        admit: Option<&'c mut dyn FnMut(usize, usize) -> Result<()>>,
    ) -> Self {
        Self {
            check,
            admit,
            work: 0,
        }
    }
    fn admit_work(&mut self, bytes: usize, visits: usize) -> Result<()> {
        if let Some(admit) = self.admit.as_mut() {
            admit(bytes, visits)?;
        }
        Ok(())
    }
    fn now(&mut self) -> Result<()> {
        if let Some(check) = self.check.as_mut() {
            check()?;
        }
        self.work = 0;
        Ok(())
    }
    fn work(&mut self, amount: usize) -> Result<()> {
        self.work = self.work.saturating_add(amount);
        if self.work >= CHECK_BYTES {
            self.now()?;
        }
        Ok(())
    }
}

// Closed sinks share the existing visitor, styles, escaping and budget law.
enum JsonSink<'a> {
    Bytes(&'a mut Vec<u8>),
    Count(usize),
    StateBytes {
        bytes: &'a mut Vec<u8>,
        available: usize,
        scratch: usize,
    },
    StateCount {
        count: usize,
        available: usize,
        scratch: usize,
    },
    Digest {
        hasher: &'a mut crate::Digest256Hasher,
        bytes: usize,
    },
}
struct JsonOutput<'a, 'c> {
    sink: JsonSink<'a>,
    poll: JsonCheck<'c>,
}
impl<'a, 'c> JsonOutput<'a, 'c> {
    fn bytes(bytes: &'a mut Vec<u8>) -> Self {
        Self {
            sink: JsonSink::Bytes(bytes),
            poll: JsonCheck::new(None),
        }
    }
    fn count() -> Self {
        Self {
            sink: JsonSink::Count(0),
            poll: JsonCheck::new(None),
        }
    }
    fn state_remaining(&self) -> Option<usize> {
        match &self.sink {
            JsonSink::StateBytes {
                bytes,
                available,
                scratch,
            } => Some(
                available
                    .saturating_sub(bytes.capacity())
                    .saturating_sub(*scratch),
            ),
            JsonSink::StateCount {
                available, scratch, ..
            } => Some(available.saturating_sub(*scratch)),
            _ => None,
        }
    }
    fn reserve_scratch(&mut self, amount: usize) -> Result<()> {
        if let Some(remaining) = self.state_remaining() {
            if amount > remaining {
                return Err(state_error());
            }
            match &mut self.sink {
                JsonSink::StateBytes { scratch, .. } | JsonSink::StateCount { scratch, .. } => {
                    *scratch = scratch.checked_add(amount).ok_or_else(state_error)?
                }
                _ => {}
            }
        }
        Ok(())
    }
    fn release_scratch(&mut self, amount: usize) {
        match &mut self.sink {
            JsonSink::StateBytes { scratch, .. } | JsonSink::StateCount { scratch, .. } => {
                *scratch -= amount
            }
            _ => {}
        }
    }
    fn len(&self) -> usize {
        match &self.sink {
            JsonSink::Bytes(bytes) => bytes.len(),
            JsonSink::Count(count) | JsonSink::StateCount { count, .. } => *count,
            JsonSink::StateBytes { bytes, .. } => bytes.len(),
            JsonSink::Digest { bytes, .. } => *bytes,
        }
    }
}
fn write_document(value: &JsonValue, limits: JsonLimits, style: WriteStyle) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    write_document_into(value, limits, style, &mut JsonOutput::bytes(&mut bytes))?;
    Ok(bytes)
}
fn write_document_into(
    value: &JsonValue,
    limits: JsonLimits,
    style: WriteStyle,
    output: &mut JsonOutput<'_, '_>,
) -> Result<()> {
    write_document_into_with_visits(value, limits, style, output, false).map(|_| ())
}
fn write_document_into_with_visits(
    value: &JsonValue,
    limits: JsonLimits,
    style: WriteStyle,
    output: &mut JsonOutput<'_, '_>,
    combined_visit_limit: bool,
) -> Result<(usize, usize)> {
    limits.validate()?;
    output.poll.now()?;
    let mut visits = 0;
    let mut numeric_parse_visits = 0;
    write_value(
        value,
        output,
        0,
        &mut visits,
        &mut numeric_parse_visits,
        limits,
        style,
        combined_visit_limit,
    )?;
    if style.newline() {
        emit(output, b"\n", limits)?;
    }
    output.poll.now()?;
    Ok((visits, numeric_parse_visits))
}
fn emit(output: &mut JsonOutput<'_, '_>, bytes: &[u8], limits: JsonLimits) -> Result<()> {
    let next = output
        .len()
        .checked_add(bytes.len())
        .filter(|next| *next <= limits.max_bytes)
        .ok_or_else(|| {
            FoundationError::new(Code::BudgetExceeded, "JSON output byte budget exceeded")
        })?;
    for chunk in bytes.chunks(CHECK_BYTES) {
        output.poll.work(chunk.len())?;
        output.poll.admit_work(chunk.len(), 0)?;
        match &mut output.sink {
            JsonSink::Bytes(output) => output.extend_from_slice(chunk),
            JsonSink::Count(count) | JsonSink::StateCount { count, .. } => *count = next,
            JsonSink::StateBytes { bytes: output, .. } => {
                if next > output.capacity() {
                    return Err(state_error());
                }
                output.extend_from_slice(chunk);
            }
            JsonSink::Digest {
                hasher,
                bytes: count,
            } => {
                hasher.update(chunk);
                *count = next;
            }
        }
    }
    Ok(())
}

/// Render a finite IEEE-754 value with CPython's `repr(float)` layout used by
/// `json.dumps`. Rust's shortest round-trip decimal supplies the significant
/// digits; Python's fixed/scientific threshold and exponent spelling are
/// applied without converting an integer through binary64.
fn state_error() -> FoundationError {
    FoundationError::new(Code::BudgetExceeded, "canonical writer state/visit budget")
}
struct FormatCount(usize);
impl std::fmt::Write for FormatCount {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        self.0 = self.0.checked_add(text.len()).ok_or(std::fmt::Error)?;
        Ok(())
    }
}
fn python_float_into(
    value: f64,
    shortest: &str,
    out: &mut impl std::fmt::Write,
) -> std::fmt::Result {
    if value == 0.0 {
        return out.write_str(if value.is_sign_negative() {
            "-0.0"
        } else {
            "0.0"
        });
    }
    let negative = value.is_sign_negative();
    let (mantissa, suffix) = match shortest.find('e') {
        Some(at) => (
            &shortest[..at],
            shortest[at + 1..]
                .parse::<i32>()
                .expect("finite f64 exponent"),
        ),
        None => (shortest, 0),
    };
    let decimal_position = mantissa.find('.').unwrap_or(mantissa.len()) as i32;
    let digits = || mantissa.bytes().filter(|c| *c != b'.');
    let leading = digits().take_while(|c| *c == b'0').count();
    let total = digits().count();
    let trailing = digits().rev().take_while(|c| *c == b'0').count();
    let length = total - leading - trailing;
    let exponent = suffix + decimal_position - 1 - leading as i32;
    let write_digits = |out: &mut dyn std::fmt::Write, start: usize, end: usize| {
        for digit in digits().skip(leading + start).take(end - start) {
            out.write_char(digit as char)?;
        }
        std::fmt::Result::Ok(())
    };
    if negative {
        out.write_char('-')?;
    }
    if (-4..16).contains(&exponent) {
        let point = exponent + 1;
        if point <= 0 {
            out.write_str("0.")?;
            for _ in 0..-point {
                out.write_char('0')?;
            }
            write_digits(out, 0, length)?;
        } else if point as usize >= length {
            write_digits(out, 0, length)?;
            for _ in length..point as usize {
                out.write_char('0')?;
            }
            out.write_str(".0")?;
        } else {
            write_digits(out, 0, point as usize)?;
            out.write_char('.')?;
            write_digits(out, point as usize, length)?;
        }
    } else {
        write_digits(out, 0, 1)?;
        if length > 1 {
            out.write_char('.')?;
            write_digits(out, 1, length)?;
        }
        write!(
            out,
            "e{}{:02}",
            if exponent < 0 { '-' } else { '+' },
            exponent.unsigned_abs()
        )?;
    }
    Ok(())
}
fn python_float_text(value: f64) -> String {
    let shortest = value.abs().to_string();
    let mut result = String::new();
    python_float_into(value, &shortest, &mut result).expect("String formatting");
    result
}
fn emit_state_float(value: f64, output: &mut JsonOutput<'_, '_>, limits: JsonLimits) -> Result<()> {
    use std::fmt::Write;
    let mut count = FormatCount(0);
    write!(&mut count, "{}", value.abs()).map_err(|_| state_error())?;
    if count.0 > output.state_remaining().ok_or_else(state_error)? {
        return Err(state_error());
    }
    let mut shortest = String::new();
    shortest
        .try_reserve_exact(count.0)
        .map_err(|_| state_error())?;
    output.reserve_scratch(shortest.capacity())?;
    write!(&mut shortest, "{}", value.abs()).map_err(|_| state_error())?;
    count.0 = 0;
    python_float_into(value, &shortest, &mut count).map_err(|_| state_error())?;
    if count.0 > output.state_remaining().ok_or_else(state_error)? {
        return Err(state_error());
    }
    let mut result = String::new();
    result
        .try_reserve_exact(count.0)
        .map_err(|_| state_error())?;
    output.reserve_scratch(result.capacity())?;
    python_float_into(value, &shortest, &mut result).map_err(|_| state_error())?;
    emit(output, result.as_bytes(), limits)?;
    let result_capacity = result.capacity();
    let shortest_capacity = shortest.capacity();
    drop(result);
    drop(shortest);
    output.release_scratch(result_capacity);
    output.release_scratch(shortest_capacity);
    Ok(())
}

fn write_value(
    value: &JsonValue,
    output: &mut JsonOutput<'_, '_>,
    depth: usize,
    visits: &mut usize,
    numeric_parse_visits: &mut usize,
    limits: JsonLimits,
    style: WriteStyle,
    combined_visit_limit: bool,
) -> Result<()> {
    output.poll.work(256)?;
    let completed_visits = if combined_visit_limit {
        visits.checked_add(*numeric_parse_visits).ok_or_else(|| {
            FoundationError::new(Code::BudgetExceeded, "JSON visit counter overflow")
        })?
    } else {
        *visits
    };
    if depth > limits.max_depth || completed_visits >= limits.max_visits {
        return Err(FoundationError::new(
            Code::BudgetExceeded,
            "JSON output structural budget exceeded",
        ));
    }
    output.poll.admit_work(0, 1)?;
    *visits += 1;
    match value {
        JsonValue::Null => emit(output, b"null", limits)?,
        JsonValue::Bool(true) => emit(output, b"true", limits)?,
        JsonValue::Bool(false) => emit(output, b"false", limits)?,
        JsonValue::Number(number) => {
            // JsonValue is public for owner-schema traversal. Do not trust a caller-built
            // Number to carry a valid JSON lexeme or the declared numeric kind.
            let mut validation_limits = limits;
            if combined_visit_limit {
                let completed_visits =
                    visits.checked_add(*numeric_parse_visits).ok_or_else(|| {
                        FoundationError::new(Code::BudgetExceeded, "JSON visit counter overflow")
                    })?;
                let remaining = limits.max_visits.saturating_sub(completed_visits);
                if remaining == 0 {
                    return Err(FoundationError::new(
                        Code::BudgetExceeded,
                        "JSON writer budget exceeded",
                    ));
                }
                validation_limits.max_visits = remaining;
            }
            let state_remaining = output.state_remaining().map(|available| (0, available));
            let checked = parse_json_inner_with_admission(
                number.lexeme.as_bytes(),
                JsonMode::PublishedStrict,
                validation_limits,
                state_remaining,
                output
                    .poll
                    .check
                    .as_mut()
                    .map(|check| &mut **check as &mut dyn FnMut() -> Result<()>),
                output
                    .poll
                    .admit
                    .as_mut()
                    .map(|admit| &mut **admit as &mut dyn FnMut(usize, usize) -> Result<()>),
            )?;
            *numeric_parse_visits = numeric_parse_visits
                .checked_add(checked.visits())
                .ok_or_else(|| {
                    FoundationError::new(Code::BudgetExceeded, "JSON visit counter overflow")
                })?;
            let same_number = if output.poll.check.is_some() {
                match checked.root() {
                    JsonValue::Number(checked_number) if checked_number.kind == number.kind => {
                        checked_bytes_equal(
                            checked_number.lexeme.as_bytes(),
                            number.lexeme.as_bytes(),
                            &mut output.poll,
                        )?
                    }
                    _ => false,
                }
            } else {
                checked.root() == value
            };
            if !same_number {
                return Err(FoundationError::new(
                    Code::InvalidNumber,
                    "number lexeme and kind disagree",
                ));
            }
            drop(checked);
            if style.python_numbers() && number.kind == JsonNumberKind::Float {
                let value = number.lexeme.parse::<f64>().map_err(|_| {
                    FoundationError::new(Code::InvalidNumber, "float lexeme is invalid")
                })?;
                if output.state_remaining().is_some() {
                    emit_state_float(value, output, limits)?;
                } else {
                    emit(output, python_float_text(value).as_bytes(), limits)?;
                }
            } else if style.python_numbers() && number.lexeme == "-0" {
                emit(output, b"0", limits)?;
            } else {
                emit(output, number.lexeme.as_bytes(), limits)?;
            }
        }
        JsonValue::String(value) => write_string(value, output, style.python_numbers(), limits)?,
        JsonValue::Array(items) => {
            emit(output, b"[", limits)?;
            for (index, item) in items.iter().enumerate() {
                if index != 0 {
                    emit(
                        output,
                        if style.pretty() {
                            &b",\n"[..]
                        } else {
                            &b","[..]
                        },
                        limits,
                    )?;
                } else if style.pretty() {
                    emit(output, b"\n", limits)?;
                }
                if style.pretty() {
                    emit_indent(output, depth + 1, limits)?;
                }
                write_value(
                    item,
                    output,
                    depth + 1,
                    visits,
                    numeric_parse_visits,
                    limits,
                    style,
                    combined_visit_limit,
                )?;
            }
            if style.pretty() && !items.is_empty() {
                emit(output, b"\n", limits)?;
                emit_indent(output, depth, limits)?;
            }
            emit(output, b"]", limits)?;
        }
        JsonValue::Object(entries) => {
            let completed_visits = if combined_visit_limit {
                visits.checked_add(*numeric_parse_visits).ok_or_else(|| {
                    FoundationError::new(Code::BudgetExceeded, "JSON visit counter overflow")
                })?
            } else {
                *visits
            };
            if entries.len() > limits.max_visits.saturating_sub(completed_visits) {
                return Err(FoundationError::new(
                    Code::BudgetExceeded,
                    "JSON output structural budget exceeded",
                ));
            }
            let stateful = output.state_remaining().is_some();
            let mut state_ordered = None;
            if stateful {
                let bytes = entries
                    .len()
                    .checked_mul(std::mem::size_of::<(usize, &(JsonString, JsonValue))>())
                    .ok_or_else(state_error)?;
                if bytes > output.state_remaining().ok_or_else(state_error)? {
                    return Err(state_error());
                }
                let mut ordered = Vec::new();
                ordered
                    .try_reserve_exact(entries.len())
                    .map_err(|_| state_error())?;
                let actual = ordered
                    .capacity()
                    .checked_mul(std::mem::size_of::<(usize, &(JsonString, JsonValue))>())
                    .ok_or_else(state_error)?;
                output.reserve_scratch(actual)?;
                ordered.extend(entries.iter().enumerate());
                let checked_scalar_keys = output.poll.check.is_some();
                if checked_scalar_keys {
                    // PublishedStrict can retain escaped lone surrogates. Refuse this
                    // writer's unsupported scalar keys cooperatively before any sort.
                    for (key, _) in entries {
                        output.poll.work(256)?;
                        if key.as_str().is_none() {
                            return Err(FoundationError::new(
                                Code::InvalidUnicodeScalar,
                                "JSON output key is not a Unicode scalar string",
                            ));
                        }
                    }
                    output.poll.now()?;
                    // Same existing no-allocation checked scalar sort; equal UTF-16
                    // keys remain adjacent after this validated scalar ordering.
                    checked_key_sort(&mut ordered, &mut output.poll)?;
                } else {
                    ordered.sort_unstable_by(|(_, (left, _)), (_, (right, _))| {
                        left.units.cmp(&right.units)
                    });
                }
                for pair in ordered.windows(2) {
                    let duplicate = if checked_scalar_keys {
                        checked_units_equal(
                            &pair[0].1.0.units,
                            &pair[1].1.0.units,
                            &mut output.poll,
                        )?
                    } else {
                        pair[0].1.0.units == pair[1].1.0.units
                    };
                    if duplicate {
                        return Err(FoundationError::new(
                            Code::DuplicateMember,
                            "duplicate decoded JSON member",
                        ));
                    }
                }
                state_ordered = Some((ordered, actual));
            } else {
                // Decide once for the whole object: the fallback collision chain
                // stores original entry ordinals and must see every entry.
                let checked_short_object = entries.len() <= CHECK_SMALL_OBJECT_KEYS
                    && entries
                        .iter()
                        .all(|(key, _)| key.units.len() <= CHECK_SHORT_KEY_UNITS);
                let mut seen = HashSet::new();
                let mut checked_short_seen: HashSet<&[u16]> = HashSet::new();
                let mut checked_seen: BTreeMap<Digest256, usize> = BTreeMap::new();
                let mut checked_next: Vec<usize> = Vec::new();
                for (ordinal, (key, _)) in entries.iter().enumerate() {
                    output.poll.now()?;
                    let duplicate = if output.poll.check.is_some() && checked_short_object {
                        // Exact equality still resolves hash collisions. Even a fully
                        // collided table has <=32 keys of <=256 bytes per poll.
                        let duplicate = !checked_short_seen.insert(key.units.as_slice());
                        output.poll.now()?;
                        duplicate
                    } else if output.poll.check.is_some() {
                        let digest = checked_units_digest(&key.units, &mut output.poll)?;
                        let mut candidate =
                            checked_seen.get(&digest).copied().unwrap_or(usize::MAX);
                        let mut duplicate = false;
                        while candidate != usize::MAX {
                            output.poll.work(256)?;
                            if checked_units_equal(
                                &key.units,
                                &entries[candidate].0.units,
                                &mut output.poll,
                            )? {
                                duplicate = true;
                                break;
                            }
                            candidate = checked_next[candidate];
                        }
                        if !duplicate {
                            let previous =
                                checked_seen.insert(digest, ordinal).unwrap_or(usize::MAX);
                            checked_next.push(previous);
                        }
                        duplicate
                    } else {
                        !seen.insert(&key.units)
                    };
                    if duplicate {
                        return Err(FoundationError::new(
                            Code::DuplicateMember,
                            "duplicate decoded JSON member",
                        ));
                    }
                }
            }
            emit(output, b"{", limits)?;
            if style.sort_keys() {
                let (mut ordered, reserved) = match state_ordered.take() {
                    Some((keys, charge)) => (keys, charge),
                    None => {
                        let mut ordered = Vec::with_capacity(entries.len());
                        for (ordinal, entry) in entries.iter().enumerate() {
                            output.poll.work(256)?;
                            ordered.push((ordinal, entry));
                        }
                        (ordered, 0)
                    }
                };
                if stateful && output.poll.check.is_none() {
                    // Original ordinal preserves stable ordering for distinct
                    // WTF-16 keys whose as_str() is None, without sort scratch.
                    ordered.sort_unstable_by(
                        |(left_index, (left, _)), (right_index, (right, _))| {
                            left.as_str()
                                .cmp(&right.as_str())
                                .then(left_index.cmp(right_index))
                        },
                    );
                } else {
                    if output.poll.check.is_some() {
                        checked_key_sort(&mut ordered, &mut output.poll)?;
                    } else {
                        ordered.sort_by(|(_, (left, _)), (_, (right, _))| {
                            left.as_str().cmp(&right.as_str())
                        });
                    }
                }
                for (index, (_, (key, item))) in ordered.iter().copied().enumerate() {
                    if index != 0 {
                        emit(
                            output,
                            if style.pretty() {
                                &b",\n"[..]
                            } else {
                                &b","[..]
                            },
                            limits,
                        )?;
                    } else if style.pretty() {
                        emit(output, b"\n", limits)?;
                    }
                    if style.pretty() {
                        emit_indent(output, depth + 1, limits)?;
                    }
                    write_string(key, output, true, limits)?;
                    emit(
                        output,
                        if style.pretty() {
                            &b": "[..]
                        } else {
                            &b":"[..]
                        },
                        limits,
                    )?;
                    write_value(
                        item,
                        output,
                        depth + 1,
                        visits,
                        numeric_parse_visits,
                        limits,
                        style,
                        combined_visit_limit,
                    )?;
                }
                drop(ordered);
                if stateful {
                    output.release_scratch(reserved);
                }
            } else {
                if let Some((keys, reserved)) = state_ordered.take() {
                    drop(keys);
                    output.release_scratch(reserved);
                }
                for (index, (key, item)) in entries.iter().enumerate() {
                    if index != 0 {
                        emit(
                            output,
                            if style.pretty() {
                                &b",\n"[..]
                            } else {
                                &b","[..]
                            },
                            limits,
                        )?;
                    } else if style.pretty() {
                        emit(output, b"\n", limits)?;
                    }
                    if style.pretty() {
                        emit_indent(output, depth + 1, limits)?;
                    }
                    write_string(key, output, style.python_numbers(), limits)?;
                    emit(
                        output,
                        if style.pretty() {
                            &b": "[..]
                        } else {
                            &b":"[..]
                        },
                        limits,
                    )?;
                    write_value(
                        item,
                        output,
                        depth + 1,
                        visits,
                        numeric_parse_visits,
                        limits,
                        style,
                        combined_visit_limit,
                    )?;
                }
            }
            if style.pretty() && !entries.is_empty() {
                emit(output, b"\n", limits)?;
                emit_indent(output, depth, limits)?;
            }
            emit(output, b"}", limits)?;
        }
    }
    Ok(())
}

fn write_root_with_streamed_array<F>(
    value: &JsonValue,
    array_field: &str,
    mut feed: F,
    output: &mut JsonOutput<'_, '_>,
    depth: usize,
    visits: &mut usize,
    limits: JsonLimits,
    style: WriteStyle,
) -> Result<()>
where
    F: FnMut(&mut dyn FnMut(&JsonValue) -> Result<()>) -> Result<()>,
{
    let mut numeric_parse_visits = 0;
    if depth > limits.max_depth || *visits >= limits.max_visits {
        return Err(FoundationError::new(
            Code::BudgetExceeded,
            "JSON output structural budget exceeded",
        ));
    }
    *visits += 1;
    let JsonValue::Object(entries) = value else {
        return Err(FoundationError::new(
            Code::InvalidJson,
            "streamed canonical root must be an object",
        ));
    };
    if entries.len() > limits.max_visits.saturating_sub(*visits) {
        return Err(FoundationError::new(
            Code::BudgetExceeded,
            "JSON output structural budget exceeded",
        ));
    }
    let mut seen = HashSet::new();
    if entries.iter().any(|(key, _)| !seen.insert(&key.units)) {
        return Err(FoundationError::new(
            Code::DuplicateMember,
            "duplicate decoded JSON member",
        ));
    }
    let mut field_count = 0usize;
    for (key, item) in entries {
        if key.as_str() == Some(array_field) {
            field_count += 1;
            if !matches!(item, JsonValue::Array(items) if items.is_empty()) {
                return Err(FoundationError::new(
                    Code::InvalidJson,
                    "streamed canonical field requires an empty array placeholder",
                ));
            }
        }
    }
    if field_count != 1 {
        return Err(FoundationError::new(
            Code::InvalidJson,
            "streamed canonical array field is missing or duplicated",
        ));
    }

    emit(output, b"{", limits)?;
    let mut ordered: Vec<_> = entries.iter().collect();
    if style.sort_keys() {
        ordered.sort_by(|(left, _), (right, _)| left.as_str().cmp(&right.as_str()));
    }
    for (index, (key, item)) in ordered.into_iter().enumerate() {
        if index != 0 {
            emit(
                output,
                if style.pretty() {
                    &b",\n"[..]
                } else {
                    &b","[..]
                },
                limits,
            )?;
        } else if style.pretty() {
            emit(output, b"\n", limits)?;
        }
        if style.pretty() {
            emit_indent(output, depth + 1, limits)?;
        }
        write_string(key, output, true, limits)?;
        emit(
            output,
            if style.pretty() {
                &b": "[..]
            } else {
                &b":"[..]
            },
            limits,
        )?;
        if key.as_str() == Some(array_field) {
            write_streamed_array(&mut feed, output, depth + 1, visits, limits, style)?;
        } else {
            write_value(
                item,
                output,
                depth + 1,
                visits,
                &mut numeric_parse_visits,
                limits,
                style,
                false,
            )?;
        }
    }
    if style.pretty() && !entries.is_empty() {
        emit(output, b"\n", limits)?;
        emit_indent(output, depth, limits)?;
    }
    emit(output, b"}", limits)
}

fn write_streamed_array<F>(
    feed: &mut F,
    output: &mut JsonOutput<'_, '_>,
    depth: usize,
    visits: &mut usize,
    limits: JsonLimits,
    style: WriteStyle,
) -> Result<()>
where
    F: FnMut(&mut dyn FnMut(&JsonValue) -> Result<()>) -> Result<()>,
{
    let mut numeric_parse_visits = 0;
    if depth > limits.max_depth || *visits >= limits.max_visits {
        return Err(FoundationError::new(
            Code::BudgetExceeded,
            "JSON output structural budget exceeded",
        ));
    }
    *visits += 1;
    emit(output, b"[", limits)?;
    let mut item_count = 0usize;
    let mut write_item = |item: &JsonValue| -> Result<()> {
        if item_count != 0 {
            emit(
                output,
                if style.pretty() {
                    &b",\n"[..]
                } else {
                    &b","[..]
                },
                limits,
            )?;
        } else if style.pretty() {
            emit(output, b"\n", limits)?;
        }
        if style.pretty() {
            emit_indent(output, depth + 1, limits)?;
        }
        write_value(
            item,
            output,
            depth + 1,
            visits,
            &mut numeric_parse_visits,
            limits,
            style,
            false,
        )?;
        item_count = item_count
            .checked_add(1)
            .ok_or_else(|| FoundationError::new(Code::BudgetExceeded, "JSON array is too large"))?;
        Ok(())
    };
    feed(&mut write_item)?;
    drop(write_item);
    if style.pretty() && item_count != 0 {
        emit(output, b"\n", limits)?;
        emit_indent(output, depth, limits)?;
    }
    emit(output, b"]", limits)
}

fn emit_indent(output: &mut JsonOutput<'_, '_>, depth: usize, limits: JsonLimits) -> Result<()> {
    const SPACES: [u8; 256] = [b' '; 256];
    let count = depth.checked_mul(2).ok_or_else(|| {
        FoundationError::new(Code::BudgetExceeded, "JSON indentation depth exceeded")
    })?;
    let spaces = SPACES.get(..count).ok_or_else(|| {
        FoundationError::new(Code::BudgetExceeded, "JSON indentation depth exceeded")
    })?;
    emit(output, spaces, limits)
}

fn write_string(
    value: &JsonString,
    output: &mut JsonOutput<'_, '_>,
    strict_utf8: bool,
    limits: JsonLimits,
) -> Result<()> {
    if strict_utf8 && value.has_lone_surrogate() {
        return Err(FoundationError::new(
            Code::InvalidUnicodeScalar,
            "canonical UTF-8 cannot encode a lone surrogate",
        ));
    }
    emit(output, b"\"", limits)?;
    let mut units = value.units.iter().copied().peekable();
    while let Some(unit) = units.next() {
        output.poll.work(4)?;
        let ch = if (0xd800..=0xdbff).contains(&unit)
            && units
                .peek()
                .is_some_and(|next| (0xdc00..=0xdfff).contains(next))
        {
            let low = units.next().expect("checked peek");
            char::from_u32(0x10000 + (((unit - 0xd800) as u32) << 10) + (low - 0xdc00) as u32)
        } else {
            char::from_u32(unit as u32)
        };
        if let Some(ch) = ch {
            match ch {
                '"' => emit(output, b"\\\"", limits)?,
                '\\' => emit(output, b"\\\\", limits)?,
                '\u{0008}' => emit(output, b"\\b", limits)?,
                '\u{000c}' => emit(output, b"\\f", limits)?,
                '\n' => emit(output, b"\\n", limits)?,
                '\r' => emit(output, b"\\r", limits)?,
                '\t' => emit(output, b"\\t", limits)?,
                _ if (ch as u32) < 32 => write_unicode_escape(unit, output, limits)?,
                _ => {
                    let mut buffer = [0u8; 4];
                    emit(output, ch.encode_utf8(&mut buffer).as_bytes(), limits)?;
                }
            }
        } else {
            write_unicode_escape(unit, output, limits)?;
        }
    }
    emit(output, b"\"", limits)?;
    Ok(())
}

fn write_unicode_escape(
    unit: u16,
    output: &mut JsonOutput<'_, '_>,
    limits: JsonLimits,
) -> Result<()> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut escaped = *b"\\u0000";
    for (index, shift) in [12, 8, 4, 0].into_iter().enumerate() {
        escaped[index + 2] = HEX[((unit >> shift) & 15) as usize];
    }
    emit(output, &escaped, limits)
}

fn checked_key_sort(
    entries: &mut [(usize, &(JsonString, JsonValue))],
    poll: &mut JsonCheck<'_>,
) -> Result<()> {
    fn compare(
        a: &JsonString,
        b: &JsonString,
        poll: &mut JsonCheck<'_>,
    ) -> Result<std::cmp::Ordering> {
        match (a.as_str(), b.as_str()) {
            (Some(a), Some(b)) => {
                for (a, b) in a
                    .as_bytes()
                    .chunks(CHECK_BYTES)
                    .zip(b.as_bytes().chunks(CHECK_BYTES))
                {
                    poll.now()?;
                    let ordering = a.cmp(b);
                    if ordering != std::cmp::Ordering::Equal {
                        return Ok(ordering);
                    }
                }
                Ok(a.len().cmp(&b.len()))
            }
            (a, b) => {
                poll.now()?;
                Ok(a.cmp(&b))
            }
        }
    }
    checked_heap_sort(entries, poll, |a, b, poll| {
        Ok(compare(&a.1.0, &b.1.0, poll)?.then_with(|| a.0.cmp(&b.0)))
    })
}

fn checked_heap_sort<T, F>(
    entries: &mut [T],
    poll: &mut JsonCheck<'_>,
    mut compare: F,
) -> Result<()>
where
    F: FnMut(&T, &T, &mut JsonCheck<'_>) -> Result<std::cmp::Ordering>,
{
    fn sift<T, F>(
        entries: &mut [T],
        mut root: usize,
        end: usize,
        poll: &mut JsonCheck<'_>,
        compare: &mut F,
    ) -> Result<()>
    where
        F: FnMut(&T, &T, &mut JsonCheck<'_>) -> Result<std::cmp::Ordering>,
    {
        loop {
            poll.work(256)?;
            let Some(mut child) = root
                .checked_mul(2)
                .and_then(|n| n.checked_add(1))
                .filter(|n| *n < end)
            else {
                break;
            };
            if child + 1 < end && compare(&entries[child], &entries[child + 1], poll)?.is_lt() {
                child += 1;
            }
            if !compare(&entries[root], &entries[child], poll)?.is_lt() {
                break;
            }
            entries.swap(root, child);
            root = child;
        }
        Ok(())
    }
    for root in (0..entries.len() / 2).rev() {
        sift(entries, root, entries.len(), poll, &mut compare)?;
    }
    for end in (1..entries.len()).rev() {
        poll.work(256)?;
        entries.swap(0, end);
        sift(entries, 0, end, poll, &mut compare)?;
    }
    poll.now()
}

/// Order object members for order-independent structural comparison only.
/// This is not canonical emission: exact numeric lexemes, string units, values
/// and array order remain unchanged. Uses the shared checked heapsort in place,
/// without scratch allocation. The caller owns both retained parse trees and
/// supplies its original remaining work allowance and cooperative cutoff.
/// Work counts value visits, key comparisons and compared UTF-16 chunks.
pub fn order_json_object_members_with_check(
    value: &mut JsonValue,
    limits: JsonLimits,
    check: &mut dyn FnMut() -> Result<()>,
) -> Result<usize> {
    fn charge(work: &mut usize, limit: usize) -> Result<()> {
        *work = work.checked_add(1).filter(|n| *n <= limit).ok_or_else(|| {
            FoundationError::new(Code::BudgetExceeded, "JSON member ordering work budget")
        })?;
        Ok(())
    }
    fn visit(
        value: &mut JsonValue,
        depth: usize,
        limits: JsonLimits,
        work: &mut usize,
        poll: &mut JsonCheck<'_>,
    ) -> Result<()> {
        poll.now()?;
        charge(work, limits.max_visits)?;
        if depth > limits.max_depth {
            return Err(FoundationError::new(
                Code::BudgetExceeded,
                "JSON member ordering depth budget",
            ));
        }
        match value {
            JsonValue::Object(entries) => {
                checked_heap_sort(entries, poll, |a, b, poll| {
                    charge(work, limits.max_visits)?;
                    for (a, b) in
                        a.0.units()
                            .chunks(CHECK_BYTES / 2)
                            .zip(b.0.units().chunks(CHECK_BYTES / 2))
                    {
                        poll.now()?;
                        charge(work, limits.max_visits)?;
                        let order = a.cmp(b);
                        if order != std::cmp::Ordering::Equal {
                            return Ok(order);
                        }
                    }
                    Ok(a.0.units().len().cmp(&b.0.units().len()))
                })?;
                for (_, value) in entries {
                    visit(value, depth + 1, limits, work, poll)?;
                }
            }
            JsonValue::Array(entries) => {
                for value in entries {
                    visit(value, depth + 1, limits, work, poll)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    limits.validate()?;
    let mut work = 0;
    visit(
        value,
        1,
        limits,
        &mut work,
        &mut JsonCheck::new(Some(check)),
    )?;
    Ok(work)
}

// Auxiliary fixed-size digest keys accelerate only exact decoded-member lookup.
// Collisions always use the original UTF-16 units; this is not corpus identity.
fn checked_units_digest(units: &[u16], poll: &mut JsonCheck<'_>) -> Result<Digest256> {
    let mut digest = crate::Digest256Hasher::new();
    let mut bytes = [0u8; 8192];
    for chunk in units.chunks(bytes.len() / 2) {
        poll.now()?;
        for (unit, bytes) in chunk.iter().zip(bytes.chunks_exact_mut(2)) {
            bytes.copy_from_slice(&unit.to_le_bytes());
        }
        digest.update(&bytes[..chunk.len() * 2]);
    }
    poll.now()?;
    Ok(digest.finalize())
}

fn checked_units_equal(a: &[u16], b: &[u16], poll: &mut JsonCheck<'_>) -> Result<bool> {
    if a.len() != b.len() {
        return Ok(false);
    }
    for (a, b) in a.chunks(CHECK_BYTES / 2).zip(b.chunks(CHECK_BYTES / 2)) {
        poll.now()?;
        if a != b {
            return Ok(false);
        }
    }
    Ok(true)
}

fn checked_bytes_equal(a: &[u8], b: &[u8], poll: &mut JsonCheck<'_>) -> Result<bool> {
    if a.len() != b.len() {
        return Ok(false);
    }
    for (a, b) in a.chunks(CHECK_BYTES).zip(b.chunks(CHECK_BYTES)) {
        poll.now()?;
        if a != b {
            return Ok(false);
        }
    }
    Ok(true)
}
