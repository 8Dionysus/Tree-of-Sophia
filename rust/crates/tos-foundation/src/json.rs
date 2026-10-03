use std::collections::{HashMap, HashSet};

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
    parse_json_inner(raw, mode, limits, None)
}

/// The legacy Item decoder supplies its remaining logical parser workspace.
/// This uses the same grammar; allocator overhead and RSS remain separate.
pub fn parse_json_with_state_budget(
    raw: &[u8],
    mode: JsonMode,
    limits: JsonLimits,
    available: usize,
) -> Result<JsonDocument> {
    parse_json_inner(raw, mode, limits, Some((0, available)))
}

fn parse_json_inner(
    raw: &[u8],
    mode: JsonMode,
    limits: JsonLimits,
    state: Option<(usize, usize)>,
) -> Result<JsonDocument> {
    limits.validate()?;
    if raw.len() > limits.max_bytes {
        return Err(FoundationError::new(
            Code::BudgetExceeded,
            "JSON byte budget exceeded",
        ));
    }
    let source = std::str::from_utf8(raw).map_err(|error| {
        FoundationError::new(Code::InvalidUtf8, "JSON input is not UTF-8").at(error.valid_up_to())
    })?;
    let mut parser = Parser {
        source,
        raw,
        at: 0,
        visits: 0,
        mode,
        limits,
        state,
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
    parser.spaces();
    if parser.at != raw.len() {
        return Err(parser.error(Code::InvalidJson, "trailing JSON input"));
    }
    let visits = parser.visits;
    Ok(JsonDocument { root, mode, visits })
}

pub fn parse_json_profile(raw: &[u8], profile: &str, limits: JsonLimits) -> Result<JsonDocument> {
    parse_json(raw, JsonMode::from_profile(profile)?, limits)
}

struct Parser<'a> {
    source: &'a str,
    raw: &'a [u8],
    at: usize,
    visits: usize,
    state: Option<(usize, usize)>,
    mode: JsonMode,
    limits: JsonLimits,
}

impl Parser<'_> {
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
        if self.state.is_none() {
            return Ok(JsonString::from_units(units));
        }
        let length = char::decode_utf16(units.iter().copied())
            .try_fold(0usize, |n, c| n.checked_add(c.ok()?.len_utf8()));
        let utf8 = if let Some(length) = length {
            self.charge(length)?;
            let mut text = String::new();
            text.try_reserve_exact(length).map_err(|_| {
                self.error(Code::BudgetExceeded, "JSON parser state budget exceeded")
            })?;
            for c in char::decode_utf16(units.iter().copied()) {
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
    fn spaces(&mut self) {
        while matches!(self.raw.get(self.at), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.at += 1;
        }
    }
    fn value(&mut self, depth: usize) -> Result<JsonValue> {
        if depth > self.limits.max_depth || self.visits >= self.limits.max_visits {
            return Err(self.error(Code::BudgetExceeded, "JSON structural budget exceeded"));
        }
        self.visits += 1;
        self.spaces();
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
                    let ch = self.source[self.at..]
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
        self.spaces();
        let mut entries = Vec::new();
        let mut positions: HashMap<Vec<u16>, usize> = HashMap::new();
        let mut index_capacity_charge = 0usize;
        if self.raw.get(self.at) == Some(&b'}') {
            self.at += 1;
            return Ok(JsonValue::Object(entries));
        }
        loop {
            self.spaces();
            if self.raw.get(self.at) != Some(&b'"') {
                return Err(self.error(Code::InvalidJson, "object key must be string"));
            }
            let key = self.string()?;
            self.spaces();
            self.take(b':')?;
            let value = self.value(depth + 1)?;
            if let Some(&position) = positions.get(&key.units) {
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
                positions.insert(key.units.clone(), entries.len());
                entries.push((key, value));
            }
            self.spaces();
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
        self.spaces();
        let mut items = Vec::new();
        if self.raw.get(self.at) == Some(&b']') {
            self.at += 1;
            return Ok(JsonValue::Array(items));
        }
        loop {
            let value = self.value(depth + 1)?;
            self.reserve(&mut items, 1)?;
            items.push(value);
            self.spaces();
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
                self.at += 1;
            }
            if self.at == exponent_start {
                return Err(self.error(Code::InvalidNumber, "missing exponent digits"));
            }
        }
        self.charge(self.at - start)?;
        let lexeme = self.source[start..self.at].to_owned();
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
        &mut JsonOutput::Bytes(&mut bytes),
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
    let mut output = JsonOutput::Count(0);
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
    limits.validate()?;
    let mut output = JsonOutput::Digest {
        hasher,
        bytes: *written,
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
    PythonPretty2SortedLf,
}

impl WriteStyle {
    fn sort_keys(self) -> bool {
        matches!(
            self,
            Self::PythonCompact | Self::PythonCompactLf | Self::PythonPretty2SortedLf
        )
    }
    fn python_numbers(self) -> bool {
        self != Self::PreservedCompact
    }
    fn pretty(self) -> bool {
        matches!(self, Self::PythonPretty2Lf | Self::PythonPretty2SortedLf)
    }
    fn newline(self) -> bool {
        matches!(
            self,
            Self::PythonCompactLf | Self::PythonPretty2Lf | Self::PythonPretty2SortedLf
        )
    }
}

// Closed sinks share the existing visitor, styles, escaping and budget law.
enum JsonOutput<'a> {
    Bytes(&'a mut Vec<u8>),
    Count(usize),
    Digest {
        hasher: &'a mut crate::Digest256Hasher,
        bytes: usize,
    },
}
impl JsonOutput<'_> {
    fn len(&self) -> usize {
        match self {
            Self::Bytes(bytes) => bytes.len(),
            Self::Count(count) => *count,
            Self::Digest { bytes, .. } => *bytes,
        }
    }
}
fn write_document(value: &JsonValue, limits: JsonLimits, style: WriteStyle) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    write_document_into(value, limits, style, &mut JsonOutput::Bytes(&mut bytes))?;
    Ok(bytes)
}
fn write_document_into(
    value: &JsonValue,
    limits: JsonLimits,
    style: WriteStyle,
    output: &mut JsonOutput<'_>,
) -> Result<()> {
    write_document_into_with_visits(value, limits, style, output, false).map(|_| ())
}
fn write_document_into_with_visits(
    value: &JsonValue,
    limits: JsonLimits,
    style: WriteStyle,
    output: &mut JsonOutput<'_>,
    combined_visit_limit: bool,
) -> Result<(usize, usize)> {
    limits.validate()?;
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
    Ok((visits, numeric_parse_visits))
}
fn emit(output: &mut JsonOutput<'_>, bytes: &[u8], limits: JsonLimits) -> Result<()> {
    let next = output
        .len()
        .checked_add(bytes.len())
        .filter(|next| *next <= limits.max_bytes)
        .ok_or_else(|| {
            FoundationError::new(Code::BudgetExceeded, "JSON output byte budget exceeded")
        })?;
    match output {
        JsonOutput::Bytes(output) => output.extend_from_slice(bytes),
        JsonOutput::Count(count) => *count = next,
        JsonOutput::Digest {
            hasher,
            bytes: count,
        } => {
            hasher.update(bytes);
            *count = next;
        }
    }
    Ok(())
}

/// Render a finite IEEE-754 value with CPython's `repr(float)` layout used by
/// `json.dumps`. Rust's shortest round-trip decimal supplies the significant
/// digits; Python's fixed/scientific threshold and exponent spelling are
/// applied without converting an integer through binary64.
fn python_float_text(value: f64) -> String {
    if value == 0.0 {
        return if value.is_sign_negative() {
            "-0.0"
        } else {
            "0.0"
        }
        .to_owned();
    }
    let negative = value.is_sign_negative();
    let shortest = value.abs().to_string();
    let (mantissa, exponent_suffix) = match shortest.find(|ch| ch == 'e' || ch == 'E') {
        Some(position) => (
            &shortest[..position],
            shortest[position + 1..]
                .parse::<i32>()
                .expect("finite f64 exponent"),
        ),
        None => (shortest.as_str(), 0),
    };
    let decimal_position = mantissa.find('.').unwrap_or(mantissa.len()) as i32;
    let mut digits: String = mantissa.chars().filter(|ch| *ch != '.').collect();
    let leading = digits.bytes().take_while(|byte| *byte == b'0').count();
    let exponent = exponent_suffix + decimal_position - 1 - leading as i32;
    digits.drain(..leading);
    while digits.len() > 1 && digits.ends_with('0') {
        digits.pop();
    }
    let mut result = String::with_capacity(shortest.len() + 8);
    if negative {
        result.push('-');
    }
    if (-4..16).contains(&exponent) {
        let point = exponent + 1;
        if point <= 0 {
            result.push_str("0.");
            for _ in 0..-point {
                result.push('0');
            }
            result.push_str(&digits);
        } else if point as usize >= digits.len() {
            result.push_str(&digits);
            for _ in 0..(point as usize - digits.len()) {
                result.push('0');
            }
            result.push_str(".0");
        } else {
            result.push_str(&digits[..point as usize]);
            result.push('.');
            result.push_str(&digits[point as usize..]);
        }
    } else {
        result.push(digits.as_bytes()[0] as char);
        if digits.len() > 1 {
            result.push('.');
            result.push_str(&digits[1..]);
        }
        result.push('e');
        result.push(if exponent < 0 { '-' } else { '+' });
        let magnitude = exponent.unsigned_abs();
        if magnitude < 10 {
            result.push('0');
        }
        result.push_str(&magnitude.to_string());
    }
    result
}

fn write_value(
    value: &JsonValue,
    output: &mut JsonOutput<'_>,
    depth: usize,
    visits: &mut usize,
    numeric_parse_visits: &mut usize,
    limits: JsonLimits,
    style: WriteStyle,
    combined_visit_limit: bool,
) -> Result<()> {
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
            let checked = parse_json(
                number.lexeme.as_bytes(),
                JsonMode::PublishedStrict,
                validation_limits,
            )?;
            *numeric_parse_visits = numeric_parse_visits
                .checked_add(checked.visits())
                .ok_or_else(|| {
                    FoundationError::new(Code::BudgetExceeded, "JSON visit counter overflow")
                })?;
            if checked.root() != value {
                return Err(FoundationError::new(
                    Code::InvalidNumber,
                    "number lexeme and kind disagree",
                ));
            }
            if style.python_numbers() && number.kind == JsonNumberKind::Float {
                let value = number.lexeme.parse::<f64>().map_err(|_| {
                    FoundationError::new(Code::InvalidNumber, "float lexeme is invalid")
                })?;
                emit(output, python_float_text(value).as_bytes(), limits)?;
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
            let mut seen = HashSet::new();
            if entries.iter().any(|(key, _)| !seen.insert(&key.units)) {
                return Err(FoundationError::new(
                    Code::DuplicateMember,
                    "duplicate decoded JSON member",
                ));
            }
            emit(output, b"{", limits)?;
            if style.sort_keys() {
                let mut ordered: Vec<_> = entries.iter().collect();
                ordered.sort_by(|(left, _), (right, _)| left.as_str().cmp(&right.as_str()));
                for (index, (key, item)) in ordered.into_iter().enumerate() {
                    if index != 0 {
                        emit(output, b",", limits)?;
                    }
                    write_string(key, output, true, limits)?;
                    emit(output, b":", limits)?;
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
            } else {
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

fn emit_indent(output: &mut JsonOutput<'_>, depth: usize, limits: JsonLimits) -> Result<()> {
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
    output: &mut JsonOutput<'_>,
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

fn write_unicode_escape(unit: u16, output: &mut JsonOutput<'_>, limits: JsonLimits) -> Result<()> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut escaped = *b"\\u0000";
    for (index, shift) in [12, 8, 4, 0].into_iter().enumerate() {
        escaped[index + 2] = HEX[((unit >> shift) & 15) as usize];
    }
    emit(output, &escaped, limits)
}
