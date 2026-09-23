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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsonDocument {
    root: JsonValue,
    mode: JsonMode,
}

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
}

pub fn parse_json(raw: &[u8], mode: JsonMode, limits: JsonLimits) -> Result<JsonDocument> {
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
    };
    let root = parser.value(0)?;
    parser.spaces();
    if parser.at != raw.len() {
        return Err(parser.error(Code::InvalidJson, "trailing JSON input"));
    }
    Ok(JsonDocument { root, mode })
}

struct Parser<'a> {
    source: &'a str,
    raw: &'a [u8],
    at: usize,
    visits: usize,
    mode: JsonMode,
    limits: JsonLimits,
}

impl Parser<'_> {
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
            Some(b'-' | b'0'..=b'9') => Ok(JsonValue::Number(self.number()?)),
            _ => Err(self.error(Code::InvalidJson, "expected JSON value")),
        }
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
                    return Ok(JsonString::from_units(units));
                }
                b'\\' => {
                    self.at += 1;
                    let escaped = *self
                        .raw
                        .get(self.at)
                        .ok_or_else(|| self.error(Code::InvalidJson, "incomplete JSON escape"))?;
                    self.at += 1;
                    match escaped {
                        b'"' | b'\\' | b'/' => units.push(escaped as u16),
                        b'b' => units.push(8),
                        b'f' => units.push(12),
                        b'n' => units.push(10),
                        b'r' => units.push(13),
                        b't' => units.push(9),
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
                            units.push(unit);
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
                    units.extend(ch.encode_utf16(&mut [0u16; 2]).iter().copied());
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
            items.push(self.value(depth + 1)?);
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
        let lexeme = self.source[start..self.at].to_owned();
        if kind == JsonNumberKind::Int && integer_digits > self.limits.max_integer_digits {
            return Err(self.error(Code::BudgetExceeded, "integer digit budget exceeded"));
        }
        if kind == JsonNumberKind::Float && !lexeme.parse::<f64>().is_ok_and(f64::is_finite) {
            return Err(self.error(Code::NonfiniteFloat, "nonfinite or unrepresentable float"));
        }
        Ok(JsonNumber { kind, lexeme })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalProfile {
    /// `scripts/corpus_store.py` v1 canonical bytes. Currently floats fail closed.
    CorpusSnapshotV1,
}

impl CanonicalProfile {
    pub const fn as_str(self) -> &'static str {
        "tos_corpus_snapshot_canonical_v1"
    }
    pub const fn supports_float(self) -> bool {
        false
    }
}

pub fn emit_preserved_json(document: &JsonDocument, limits: JsonLimits) -> Result<Vec<u8>> {
    write_document(document.root(), limits, false, false)
}

/// Produce exact Python `json.dumps(..., sort_keys=True, ensure_ascii=False,
/// separators=(',', ':'), allow_nan=False) + '\n'` for the declared profile.
pub fn canonical_bytes_v1(
    value: &JsonValue,
    profile: CanonicalProfile,
    limits: JsonLimits,
) -> Result<Vec<u8>> {
    match profile {
        CanonicalProfile::CorpusSnapshotV1 => write_document(value, limits, true, true),
    }
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

fn write_document(
    value: &JsonValue,
    limits: JsonLimits,
    sort_keys: bool,
    newline: bool,
) -> Result<Vec<u8>> {
    limits.validate()?;
    let mut output = Vec::new();
    let mut visits = 0;
    write_value(value, &mut output, 0, &mut visits, limits, sort_keys)?;
    if newline {
        emit(&mut output, b"\n", limits)?;
    }
    Ok(output)
}

fn emit(output: &mut Vec<u8>, bytes: &[u8], limits: JsonLimits) -> Result<()> {
    if bytes.len() > limits.max_bytes.saturating_sub(output.len()) {
        return Err(FoundationError::new(
            Code::BudgetExceeded,
            "JSON output byte budget exceeded",
        ));
    }
    output.extend_from_slice(bytes);
    Ok(())
}

fn write_value(
    value: &JsonValue,
    output: &mut Vec<u8>,
    depth: usize,
    visits: &mut usize,
    limits: JsonLimits,
    sort_keys: bool,
) -> Result<()> {
    if depth > limits.max_depth || *visits >= limits.max_visits {
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
            let checked = parse_json(number.lexeme.as_bytes(), JsonMode::PublishedStrict, limits)?;
            if checked.root() != value {
                return Err(FoundationError::new(
                    Code::InvalidNumber,
                    "number lexeme and kind disagree",
                ));
            }
            if sort_keys && number.kind == JsonNumberKind::Float {
                return Err(FoundationError::new(
                    Code::UnsupportedCanonicalNumber,
                    "float formatter has not passed Python parity",
                ));
            }
            if sort_keys && number.lexeme == "-0" {
                emit(output, b"0", limits)?;
            } else {
                emit(output, number.lexeme.as_bytes(), limits)?;
            }
        }
        JsonValue::String(value) => write_string(value, output, sort_keys, limits)?,
        JsonValue::Array(items) => {
            emit(output, b"[", limits)?;
            for (index, item) in items.iter().enumerate() {
                if index != 0 {
                    emit(output, b",", limits)?;
                }
                write_value(item, output, depth + 1, visits, limits, sort_keys)?;
            }
            emit(output, b"]", limits)?;
        }
        JsonValue::Object(entries) => {
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
            emit(output, b"{", limits)?;
            if sort_keys {
                let mut ordered: Vec<_> = entries.iter().collect();
                ordered.sort_by(|(left, _), (right, _)| left.as_str().cmp(&right.as_str()));
                for (index, (key, item)) in ordered.into_iter().enumerate() {
                    if index != 0 {
                        emit(output, b",", limits)?;
                    }
                    write_string(key, output, true, limits)?;
                    emit(output, b":", limits)?;
                    write_value(item, output, depth + 1, visits, limits, sort_keys)?;
                }
            } else {
                for (index, (key, item)) in entries.iter().enumerate() {
                    if index != 0 {
                        emit(output, b",", limits)?;
                    }
                    write_string(key, output, false, limits)?;
                    emit(output, b":", limits)?;
                    write_value(item, output, depth + 1, visits, limits, sort_keys)?;
                }
            }
            emit(output, b"}", limits)?;
        }
    }
    Ok(())
}

fn write_string(
    value: &JsonString,
    output: &mut Vec<u8>,
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

fn write_unicode_escape(unit: u16, output: &mut Vec<u8>, limits: JsonLimits) -> Result<()> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut escaped = *b"\\u0000";
    for (index, shift) in [12, 8, 4, 0].into_iter().enumerate() {
        escaped[index + 2] = HEX[((unit >> shift) & 15) as usize];
    }
    emit(output, &escaped, limits)
}
