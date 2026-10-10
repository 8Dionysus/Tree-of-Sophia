//! Local prepared search-v3 request and authenticated continuation framing.
//! Cursor authenticity binds a selected publication; it grants no admission.
use std::collections::BTreeSet;
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, JsonMode, JsonNumber, JsonNumberKind,
    JsonString, JsonValue, canonical_bytes_v1, emit_python_compact_json, parse_json,
    python_lower_unicode16_v1, python_strip_unicode16_v1,
};

pub const SCHEMA: &str = "tos_knowledge_search_compressed_v3";
pub const CURSOR_SCHEMA: &str = "tos_published_search_cursor_v1";
pub const MAX_ADDRESS: u64 = (1u64 << 53) - 1;
pub const MAX_CURSOR_BYTES: usize = 65_536;
pub(crate) const SOURCES: &[&str] = &[
    "candidate-intake",
    "canon",
    "philosophy",
    "repository",
    "semantic-interchange",
    "source-claims",
    "source-navigation",
];
pub(crate) const MAC_DOMAIN: &[u8] = b"tos-published-search-v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompressedSearchErrorCode {
    InvalidRequest,
    StaleBinding,
    CursorInvalid,
    CursorExpired,
    BudgetExceeded,
    Unavailable,
    Cancelled,
    DeadlineExceeded,
}
#[derive(Debug)]
pub struct CompressedSearchError {
    pub code: CompressedSearchErrorCode,
    pub message: String,
}
impl std::fmt::Display for CompressedSearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
impl std::error::Error for CompressedSearchError {}
pub(crate) type Result<T> = std::result::Result<T, CompressedSearchError>;
pub(crate) fn err(
    code: CompressedSearchErrorCode,
    message: impl Into<String>,
) -> CompressedSearchError {
    CompressedSearchError {
        code,
        message: message.into(),
    }
}
pub(crate) fn unavailable(message: impl Into<String>) -> CompressedSearchError {
    err(CompressedSearchErrorCode::Unavailable, message)
}
pub(crate) fn budget(message: impl Into<String>) -> CompressedSearchError {
    err(CompressedSearchErrorCode::BudgetExceeded, message)
}
pub(crate) fn invalid(message: impl Into<String>) -> CompressedSearchError {
    err(CompressedSearchErrorCode::InvalidRequest, message)
}
pub(crate) fn cursor_error(message: impl Into<String>) -> CompressedSearchError {
    err(CompressedSearchErrorCode::CursorInvalid, message)
}

#[derive(Clone, Debug)]
pub struct CompressedSearchRequest {
    pub query: String,
    pub sources: Vec<String>,
    pub kind_ids: Vec<String>,
    pub predicate_ids: Vec<String>,
    pub cursor: Option<String>,
    pub limit: usize,
}
impl Default for CompressedSearchRequest {
    fn default() -> Self {
        Self {
            query: String::new(),
            sources: Vec::new(),
            kind_ids: Vec::new(),
            predicate_ids: Vec::new(),
            cursor: None,
            limit: 40,
        }
    }
}
impl CompressedSearchRequest {
    /// Transport decoding stays strict before any database access. Empty source
    /// lists select the same complete maintained source set as omitted lists.
    pub fn from_json(value: &JsonValue) -> Result<Self> {
        if value.as_object().is_none() {
            return Err(invalid("compressed search request must be an object"));
        }
        if value.object_get("offset").is_some() {
            return Err(invalid("compressed search does not accept offsets"));
        }
        let mut request = Self::default();
        if let Some(query) = value.object_get("query") {
            request.query = query
                .as_str()
                .ok_or_else(|| invalid("knowledge search query must be a string"))?
                .to_owned();
        }
        for (name, output) in [
            ("sources", &mut request.sources),
            ("kind_ids", &mut request.kind_ids),
            ("predicate_ids", &mut request.predicate_ids),
        ] {
            if let Some(values) = value.object_get(name).filter(|v| !v.is_null()) {
                let values = values
                    .as_array()
                    .ok_or_else(|| invalid(format!("{name} must contain at most 100 strings")))?;
                if values.len() > 100 {
                    return Err(invalid(format!("{name} must contain at most 100 strings")));
                }
                for v in values {
                    output.push(
                        v.as_str()
                            .ok_or_else(|| invalid(format!("{name} must contain strings")))?
                            .to_owned(),
                    );
                }
            }
        }
        if let Some(limit) = value.object_get("limit") {
            request.limit = usize::try_from(
                limit
                    .as_u64()
                    .ok_or_else(|| invalid("limit must be an integer in 1..100"))?,
            )
            .map_err(|_| invalid("limit must be an integer in 1..100"))?;
        }
        if let Some(cursor) = value.object_get("cursor").filter(|v| !v.is_null()) {
            request.cursor = Some(
                cursor
                    .as_str()
                    .ok_or_else(|| cursor_error("invalid published search cursor"))?
                    .to_owned(),
            );
        }
        request.normalize()?;
        if let Some(cursor) = request.cursor.as_deref() {
            decode_outer(cursor)?;
        }
        Ok(request)
    }
    pub(crate) fn normalize(&self) -> Result<NormalizedRequest> {
        if !(1..=100).contains(&self.limit) {
            return Err(invalid("limit must be an integer in 1..100"));
        }
        if self.query.len() > 1024 || self.query.chars().count() > 256 {
            return Err(invalid("knowledge search query exceeds 256 characters"));
        }
        let stripped = python_strip_unicode16_v1(&self.query, 256)
            .map_err(|_| invalid("knowledge search query exceeds 256 characters"))?;
        let needle = python_lower_unicode16_v1(stripped, 256, 256, 1024)
            .map_err(|_| invalid("knowledge search query exceeds 256 characters"))?;
        let sources = filter(&self.sources, true)?;
        let kind_ids = filter(&self.kind_ids, false)?;
        let predicate_ids = filter(&self.predicate_ids, false)?;
        let filters = object(vec![
            ("sources", strings(&sources)),
            ("kind_ids", strings(&kind_ids)),
            ("predicate_ids", strings(&predicate_ids)),
        ]);
        compact(
            &JsonValue::Array(vec![string(&needle), filters.clone()]),
            65_536,
        )
        .map_err(|_| invalid("search query/filter frame exceeds 65536 bytes"))?;
        if let Some(cursor) = &self.cursor {
            if cursor.is_empty() || cursor.len() > MAX_CURSOR_BYTES {
                return Err(cursor_error("invalid published search cursor length"));
            }
        }
        Ok(NormalizedRequest {
            needle,
            display_query: stripped.to_owned(),
            sources,
            kind_ids,
            predicate_ids,
            filters,
        })
    }
}
pub(crate) struct NormalizedRequest {
    pub needle: String,
    pub display_query: String,
    pub sources: Vec<String>,
    pub kind_ids: Vec<String>,
    pub predicate_ids: Vec<String>,
    pub filters: JsonValue,
}
fn filter(values: &[String], known: bool) -> Result<Vec<String>> {
    if values.len() > 100 {
        return Err(invalid("filters must contain at most 100 strings"));
    }
    let set: BTreeSet<_> = values.iter().filter(|s| !s.is_empty()).cloned().collect();
    if known && set.iter().any(|s| !SOURCES.contains(&s.as_str())) {
        return Err(invalid("unsupported knowledge sources"));
    }
    Ok(if known && set.is_empty() {
        SOURCES.iter().map(|s| s.to_string()).collect()
    } else {
        set.into_iter().collect()
    })
}

#[derive(Clone, Copy, Debug)]
pub struct PublishedSearchLimits {
    pub candidate_budget: usize,
    pub verification_bytes: usize,
    pub metadata_bytes: usize,
    pub body_bytes: usize,
}
impl Default for PublishedSearchLimits {
    fn default() -> Self {
        Self {
            candidate_budget: 256,
            verification_bytes: 65_536,
            metadata_bytes: 4 * 1024 * 1024,
            body_bytes: 4 * 1024 * 1024,
        }
    }
}
impl PublishedSearchLimits {
    pub(crate) fn validate(self) -> Result<()> {
        if !(2..=4096).contains(&self.candidate_budget)
            || !(8192..=8 * 1024 * 1024).contains(&self.verification_bytes)
            || !(4 * 1024 * 1024..=64 * 1024 * 1024).contains(&self.metadata_bytes)
            || !(1..=64 * 1024 * 1024).contains(&self.body_bytes)
        {
            Err(invalid("invalid published search limits"))
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Pending {
    pub address: u64,
    pub rank: u8,
    pub id_hash: String,
}
#[derive(Clone, Debug)]
pub(crate) struct KindState {
    pub cursor: Option<JsonValue>,
    pub exhausted: bool,
    pub pending: Vec<Pending>,
}
impl KindState {
    fn new() -> Self {
        Self {
            cursor: None,
            exhausted: false,
            pending: Vec::new(),
        }
    }
    fn json(&self) -> JsonValue {
        object(vec![
            ("cursor", self.cursor.clone().unwrap_or(JsonValue::Null)),
            ("exhausted", JsonValue::Bool(self.exhausted)),
            (
                "pending",
                JsonValue::Array(
                    self.pending
                        .iter()
                        .map(|p| {
                            JsonValue::Array(vec![
                                number(p.address),
                                number(p.rank as u64),
                                string(&p.id_hash),
                            ])
                        })
                        .collect(),
                ),
            ),
        ])
    }
}
pub(crate) struct OuterState {
    pub binding: String,
    pub query: String,
    pub expires: u64,
    pub nodes: KindState,
    pub relations: KindState,
}
impl OuterState {
    pub fn new(binding: String, query: String, now: u64) -> Result<Self> {
        Ok(Self {
            binding,
            query,
            expires: now
                .checked_add(900)
                .filter(|v| *v <= MAX_ADDRESS)
                .ok_or_else(|| invalid("invalid search clock"))?,
            nodes: KindState::new(),
            relations: KindState::new(),
        })
    }
    pub fn json(&self) -> JsonValue {
        object(vec![
            ("schema", string(CURSOR_SCHEMA)),
            ("binding", string(&self.binding)),
            ("query", string(&self.query)),
            ("expires", number(self.expires)),
            ("nodes", self.nodes.json()),
            ("relations", self.relations.json()),
        ])
    }
}
/// Keep the exact parsed member order for MAC verification; canonical binding
/// hashing is separately insensitive to object order across process restarts.
pub(crate) fn decode_outer(token: &str) -> Result<(OuterState, JsonValue, String)> {
    if token.is_empty() || token.len() > MAX_CURSOR_BYTES {
        return Err(cursor_error("invalid published search cursor length"));
    }
    let bytes = base64_decode(token)?;
    let envelope = parse_json(
        &bytes,
        JsonMode::PublishedStrict,
        json_limits(MAX_CURSOR_BYTES),
    )
    .map_err(|_| cursor_error("invalid published search cursor"))?
    .into_root();
    require_keys(&envelope, &["state", "mac"])?;
    let raw = get(&envelope, "state")?;
    let mac = hex(get(&envelope, "mac")?)?.to_owned();
    require_keys(
        raw,
        &[
            "schema",
            "binding",
            "query",
            "expires",
            "nodes",
            "relations",
        ],
    )?;
    if get(raw, "schema")?.as_str() != Some(CURSOR_SCHEMA) {
        return Err(cursor_error("invalid published search cursor schema"));
    }
    let binding = hex(get(raw, "binding")?)?.to_owned();
    let query = hex(get(raw, "query")?)?.to_owned();
    let expires = integer(get(raw, "expires")?, 1, MAX_ADDRESS)?;
    Ok((
        OuterState {
            binding,
            query,
            expires,
            nodes: decode_kind(get(raw, "nodes")?)?,
            relations: decode_kind(get(raw, "relations")?)?,
        },
        raw.clone(),
        mac,
    ))
}
fn decode_kind(value: &JsonValue) -> Result<KindState> {
    require_keys(value, &["cursor", "exhausted", "pending"])?;
    let exhausted = get(value, "exhausted")?
        .as_bool()
        .ok_or_else(|| cursor_error("invalid cursor exhaustion"))?;
    let cursor = get(value, "cursor")?;
    if !cursor.is_null() && (cursor.as_object().is_none() || exhausted) {
        return Err(cursor_error("invalid inner cursor"));
    }
    let raw = get(value, "pending")?
        .as_array()
        .ok_or_else(|| cursor_error("invalid pending cursor"))?;
    if raw.len() > 100 {
        return Err(cursor_error("too many pending search matches"));
    }
    let mut addresses = BTreeSet::new();
    let mut pending = Vec::new();
    for row in raw {
        let row = row
            .as_array()
            .filter(|r| r.len() == 3)
            .ok_or_else(|| cursor_error("invalid pending match"))?;
        let address = integer(&row[0], 1, MAX_ADDRESS)?;
        if !addresses.insert(address) {
            return Err(cursor_error("duplicate pending match"));
        }
        pending.push(Pending {
            address,
            rank: integer(&row[1], 0, 3)? as u8,
            id_hash: hex(&row[2])?.to_owned(),
        });
    }
    Ok(KindState {
        cursor: (!cursor.is_null()).then(|| cursor.clone()),
        exhausted,
        pending,
    })
}
pub(crate) fn encode_outer(state: &OuterState, key: &[u8]) -> Result<String> {
    let state = state.json();
    let mac = hmac(key, MAC_DOMAIN, &compact(&state, MAX_CURSOR_BYTES)?);
    let bytes = compact(
        &object(vec![("state", state), ("mac", string(&mac))]),
        MAX_CURSOR_BYTES,
    )?;
    let encoded = base64_encode(&bytes);
    if encoded.len() > MAX_CURSOR_BYTES {
        return Err(budget("published search cursor exceeds its framing budget"));
    }
    Ok(encoded)
}
pub(crate) fn hmac(key: &[u8], domain: &[u8], data: &[u8]) -> String {
    let mut inner_pad = [0x36; 64];
    let mut outer_pad = [0x5c; 64];
    let hashed;
    let key = if key.len() > 64 {
        hashed = Digest256::of_bytes(key);
        hashed.as_bytes().as_slice()
    } else {
        key
    };
    for (i, b) in key.iter().enumerate() {
        inner_pad[i] ^= *b;
        outer_pad[i] ^= *b;
    }
    let mut hash = Digest256Hasher::new();
    hash.update(&inner_pad);
    hash.update(domain);
    hash.update(data);
    let inner = hash.finalize();
    let mut hash = Digest256Hasher::new();
    hash.update(&outer_pad);
    hash.update(inner.as_bytes());
    hash.finalize().to_hex()
}
pub(crate) fn constant_time_equal(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes().zip(b.bytes()).fold(0u8, |d, (a, b)| d | (a ^ b)) == 0
}
pub(crate) fn hash(value: &JsonValue) -> Result<String> {
    Ok(Digest256::of_bytes(
        &canonical_bytes_v1(
            value,
            CanonicalProfile::SourceRecordDigestV1,
            json_limits(2 * MAX_CURSOR_BYTES + 64),
        )
        .map_err(|_| invalid("invalid search binding framing"))?,
    )
    .to_hex())
}
pub(crate) fn id_hash(value: &JsonValue) -> Result<String> {
    Ok(Digest256::of_bytes(&sorted_default(value, 1_900_000)?).to_hex())
}
pub(crate) fn compact(value: &JsonValue, max: usize) -> Result<Vec<u8>> {
    emit_python_compact_json(value, json_limits(max))
        .map_err(|_| budget("JSON framing exceeds search budget"))
}
pub(crate) fn sorted_default(value: &JsonValue, max: usize) -> Result<Vec<u8>> {
    tos_compiler::local_prepared_search::default_json(value, max)
        .map_err(|_| budget("JSON framing exceeds search budget"))
}
pub(crate) fn json_limits(max: usize) -> JsonLimits {
    JsonLimits {
        max_bytes: max,
        max_depth: 64,
        max_visits: 300_000,
        max_integer_digits: 4300,
    }
}
pub(crate) fn object(fields: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        fields
            .into_iter()
            .map(|(k, v)| (JsonString::from_utf8(k), v))
            .collect(),
    )
}
pub(crate) fn string(value: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(value))
}
pub(crate) fn strings(values: &[String]) -> JsonValue {
    JsonValue::Array(values.iter().map(|v| string(v)).collect())
}
pub(crate) fn number(value: u64) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: value.to_string(),
    })
}
pub(crate) fn get<'a>(value: &'a JsonValue, key: &str) -> Result<&'a JsonValue> {
    value
        .object_get(key)
        .ok_or_else(|| cursor_error("incomplete cursor state"))
}
pub(crate) fn require_keys(value: &JsonValue, keys: &[&str]) -> Result<()> {
    let fields = value
        .as_object()
        .ok_or_else(|| cursor_error("cursor must be an object"))?;
    if fields.len() != keys.len()
        || fields
            .iter()
            .any(|(k, _)| !k.as_str().is_some_and(|k| keys.contains(&k)))
    {
        Err(cursor_error("invalid cursor member set"))
    } else {
        Ok(())
    }
}
pub(crate) fn integer(value: &JsonValue, low: u64, high: u64) -> Result<u64> {
    value
        .as_u64()
        .filter(|v| *v >= low && *v <= high)
        .ok_or_else(|| cursor_error("invalid cursor integer"))
}
pub(crate) fn hex(value: &JsonValue) -> Result<&str> {
    let s = value
        .as_str()
        .ok_or_else(|| cursor_error("invalid cursor digest"))?;
    Digest256::from_hex(s).map_err(|_| cursor_error("invalid cursor digest"))?;
    Ok(s)
}
pub(crate) fn base64_encode(raw: &[u8]) -> String {
    const ABC: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity((raw.len() * 4 + 2) / 3);
    for c in raw.chunks(3) {
        let n = ((c[0] as u32) << 16)
            | ((c.get(1).copied().unwrap_or(0) as u32) << 8)
            | c.get(2).copied().unwrap_or(0) as u32;
        out.push(ABC[(n >> 18) as usize] as char);
        out.push(ABC[((n >> 12) & 63) as usize] as char);
        if c.len() > 1 {
            out.push(ABC[((n >> 6) & 63) as usize] as char);
        }
        if c.len() > 2 {
            out.push(ABC[(n & 63) as usize] as char);
        }
    }
    out
}
pub(crate) fn base64_decode(raw: &str) -> Result<Vec<u8>> {
    if raw.len() % 4 == 1 {
        return Err(cursor_error("invalid cursor encoding"));
    }
    let mut out = Vec::with_capacity(raw.len() * 3 / 4);
    let mut bits = 0u32;
    let mut count = 0;
    for byte in raw.bytes() {
        let digit = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => return Err(cursor_error("invalid cursor alphabet")),
        };
        bits = (bits << 6) | digit as u32;
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
            bits &= (1 << count) - 1;
        }
    }
    // The maintained Python decoder accepts unused low bits. Cursor identity
    // and integrity are authenticated decoded JSON, not its base64 spelling.
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_matches_rfc4231_and_domain_is_part_of_authenticated_bytes() {
        let expected = "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7";
        assert_eq!(hmac(&[0x0b; 20], b"", b"Hi There"), expected);
        assert_eq!(hmac(&[0x0b; 20], b"Hi ", b"There"), expected);
        assert!(!constant_time_equal(
            expected,
            &hmac(&[0x0b; 20], MAC_DOMAIN, b"Hi There")
        ));
    }

    #[test]
    fn transport_keeps_short_query_and_python_unicode_lower_expansion() {
        let request = CompressedSearchRequest::from_json(&object(vec![
            ("query", string("  İ  ")),
            (
                "sources",
                JsonValue::Array(vec![string(""), string("philosophy"), string("philosophy")]),
            ),
        ]))
        .unwrap();
        let normalized = request.normalize().unwrap();
        assert_eq!(normalized.needle, "i\u{307}");
        assert_eq!(normalized.sources, ["philosophy"]);
        let too_long = CompressedSearchRequest {
            query: "İ".repeat(256),
            ..Default::default()
        };
        assert_eq!(
            too_long.normalize().err().unwrap().code,
            CompressedSearchErrorCode::InvalidRequest
        );
        assert!(
            CompressedSearchRequest {
                query: "a".into(),
                ..Default::default()
            }
            .normalize()
            .is_ok()
        );
    }

    #[test]
    fn continuation_uses_persisted_secret_and_binding_hash_ignores_member_order() {
        let a = object(vec![("epoch", number(1)), ("source", string("x"))]);
        let b = object(vec![("source", string("x")), ("epoch", number(1))]);
        assert_eq!(hash(&a).unwrap(), hash(&b).unwrap());
        let state =
            OuterState::new(hash(&a).unwrap(), hash(&string("query")).unwrap(), 100).unwrap();
        let token = encode_outer(&state, &[7; 32]).unwrap();
        let (restarted, raw, mac) = decode_outer(&token).unwrap();
        assert_eq!(restarted.expires, 1000);
        assert!(constant_time_equal(
            &mac,
            &hmac(
                &[7; 32],
                MAC_DOMAIN,
                &compact(&raw, MAX_CURSOR_BYTES).unwrap()
            )
        ));
        assert!(!constant_time_equal(
            &mac,
            &hmac(
                &[8; 32],
                MAC_DOMAIN,
                &compact(&raw, MAX_CURSOR_BYTES).unwrap()
            )
        ));
    }
}
