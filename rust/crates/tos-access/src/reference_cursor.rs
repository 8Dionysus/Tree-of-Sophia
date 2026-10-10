//! Source-compatible unsigned cursors for the ordinary public indexed search.
//!
//! The outer Reference envelope is shared with the weak QueryStore owner, but
//! this CMP adapter alone maps its V3 children to QRY continuation progress.
//! Cursor bytes carry position, never source authority or a disclosure grant.
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value, json};
use tos_foundation::Digest256;
use tos_query::{
    IndexedWireCursorCodec,
    search_v2::{
        SearchContinuationProgress, SearchContinuationState, SearchKind, SearchOrderKey,
        SearchRank, SearchV2Error, SearchV2ErrorCode,
    },
};

pub(crate) const REFERENCE_INDEXED_CURSOR_MAX_BYTES: usize = 8 * 1024;
pub(crate) const REFERENCE_SEARCH_CHILD_MAX_BYTES: usize = 2 * 1024;
pub(crate) const REFERENCE_CURSOR_TTL_SECONDS: u64 = 15 * 60;
const OUTER_SCHEMA: &str = "tos_knowledge_search_indexed_cursor_v1";
const CHILD_SCHEMA: &str = "tos_knowledge_search_cursor_v1";
const BASE64URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn error(code: SearchV2ErrorCode, message: &'static str) -> SearchV2Error {
    SearchV2Error { code, message }
}
fn invalid(message: &'static str) -> SearchV2Error {
    error(SearchV2ErrorCode::InvalidRequest, message)
}
fn stale_source() -> SearchV2Error {
    error(
        SearchV2ErrorCode::StaleSelection,
        "indexed cursor source changed",
    )
}
fn stale_query() -> SearchV2Error {
    error(
        SearchV2ErrorCode::StaleContinuation,
        "indexed cursor query or filters changed",
    )
}

pub(crate) fn reference_cursor_now_seconds() -> Result<u64, SearchV2Error> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .map_err(|_| {
            error(
                SearchV2ErrorCode::Unavailable,
                "indexed cursor clock unavailable",
            )
        })
}

fn b64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'-' => Some(62),
        b'_' => Some(63),
        _ => None,
    }
}

fn base64url_decode(token: &str, cap: usize) -> Result<Vec<u8>, SearchV2Error> {
    if token.is_empty() || token.len() > cap || token.len() % 4 == 1 {
        return Err(invalid("indexed cursor encoding is invalid"));
    }
    let mut out = Vec::with_capacity(token.len().saturating_mul(3) / 4);
    let mut accumulator = 0u32;
    let mut bits = 0u32;
    for byte in token.bytes() {
        let value = b64_value(byte).ok_or_else(|| invalid("indexed cursor is not base64url"))?;
        accumulator = (accumulator << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((accumulator >> bits) & 0xff) as u8);
        }
    }
    if bits != 0 && (accumulator & ((1u32 << bits) - 1)) != 0 {
        return Err(invalid("indexed cursor base64url tail is not canonical"));
    }
    if out.len() > cap {
        return Err(invalid("indexed cursor decoded size exceeds cap"));
    }
    Ok(out)
}

fn base64url_encode(raw: &[u8], cap: usize) -> Result<String, SearchV2Error> {
    let output_len = raw
        .len()
        .checked_mul(4)
        .and_then(|len| len.checked_add(2))
        .map(|len| len / 3)
        .ok_or_else(|| invalid("indexed cursor size overflow"))?;
    if output_len > cap {
        return Err(error(
            SearchV2ErrorCode::BudgetExceeded,
            "indexed cursor byte budget exceeded",
        ));
    }
    let mut out = String::with_capacity(output_len);
    for chunk in raw.chunks(3) {
        let a = chunk[0];
        let b = *chunk.get(1).unwrap_or(&0);
        let c = *chunk.get(2).unwrap_or(&0);
        out.push(BASE64URL[(a >> 2) as usize] as char);
        out.push(BASE64URL[(((a & 0x03) << 4) | (b >> 4)) as usize] as char);
        if chunk.len() >= 2 {
            out.push(BASE64URL[(((b & 0x0f) << 2) | (c >> 6)) as usize] as char);
        }
        if chunk.len() == 3 {
            out.push(BASE64URL[(c & 0x3f) as usize] as char);
        }
    }
    Ok(out)
}

/// Decode or encode one Reference base64url/compact-JSON payload. The caller
/// supplies the protocol's separate outer or child byte cap.
pub(crate) fn decode_reference_cursor_payload(
    token: &str,
    cap: usize,
) -> Result<Value, SearchV2Error> {
    let raw = base64url_decode(token, cap)?;
    serde_json::from_slice(&raw).map_err(|_| invalid("indexed cursor JSON is invalid"))
}

pub(crate) fn encode_reference_cursor_payload(
    value: &Value,
    cap: usize,
) -> Result<String, SearchV2Error> {
    // The Reference wire format sorts every object, independently of Cargo
    // feature unification enabling serde_json's insertion-ordered maps.
    let mut sorted = value.clone();
    sorted.sort_all_objects();
    let raw = serde_json::to_vec(&sorted)
        .map_err(|_| invalid("indexed cursor JSON cannot be encoded"))?;
    if raw.len() > cap {
        return Err(error(
            SearchV2ErrorCode::BudgetExceeded,
            "indexed cursor byte budget exceeded",
        ));
    }
    base64url_encode(&raw, cap)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReferenceCursorBinding {
    source_revision: Option<String>,
    query: String,
    sources: Vec<String>,
    kind_ids: Vec<String>,
    predicate_ids: Vec<String>,
}

impl ReferenceCursorBinding {
    pub(crate) fn new(
        source_revision: impl Into<String>,
        query: impl Into<String>,
        sources: Vec<String>,
        kind_ids: Vec<String>,
        predicate_ids: Vec<String>,
    ) -> Result<Self, SearchV2Error> {
        let source_revision = source_revision.into();
        if source_revision.is_empty() {
            return Err(invalid("indexed cursor binding exceeds Reference bounds"));
        }
        Self::new_for_query_store(
            Some(source_revision),
            query,
            sources,
            kind_ids,
            predicate_ids,
        )
    }

    /// Weak QueryStore binds the authentic graph-header value, including null.
    /// WholeRoot keeps using `new`, which requires its canonical string cut.
    pub(crate) fn new_for_query_store(
        source_revision: Option<String>,
        query: impl Into<String>,
        mut sources: Vec<String>,
        mut kind_ids: Vec<String>,
        mut predicate_ids: Vec<String>,
    ) -> Result<Self, SearchV2Error> {
        let query = query.into();
        for values in [&mut sources, &mut kind_ids, &mut predicate_ids] {
            values.sort();
            values.dedup();
            if values.iter().any(|value| value.chars().count() > 256) {
                return Err(invalid(
                    "indexed cursor filter value exceeds Reference bound",
                ));
            }
        }
        let filter_count = sources
            .len()
            .checked_add(kind_ids.len())
            .and_then(|count| count.checked_add(predicate_ids.len()))
            .ok_or_else(|| invalid("indexed cursor filter count overflow"))?;
        if query.chars().count() > 256 || filter_count > 100 {
            return Err(invalid("indexed cursor binding exceeds Reference bounds"));
        }
        Ok(Self {
            source_revision,
            query,
            sources,
            kind_ids,
            predicate_ids,
        })
    }

    fn filters_value(&self) -> Value {
        json!({
            "sources": &self.sources,
            "kind_ids": &self.kind_ids,
            "predicate_ids": &self.predicate_ids,
        })
    }

    pub(crate) fn filters_digest(&self) -> Result<String, SearchV2Error> {
        let mut value = self.filters_value();
        value.sort_all_objects();
        let raw = serde_json::to_vec(&value)
            .map_err(|_| invalid("indexed cursor filters cannot be encoded"))?;
        Ok(Digest256::of_bytes(&raw).to_hex())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReferenceCursorEnvelope {
    pub(crate) nodes: Option<String>,
    pub(crate) relations: Option<String>,
    pub(crate) nodes_exhausted: bool,
    pub(crate) relations_exhausted: bool,
}

impl ReferenceCursorEnvelope {
    pub(crate) fn new(
        nodes: Option<String>,
        relations: Option<String>,
        nodes_exhausted: bool,
        relations_exhausted: bool,
    ) -> Result<Self, SearchV2Error> {
        let envelope = Self {
            nodes,
            relations,
            nodes_exhausted,
            relations_exhausted,
        };
        envelope.validate()?;
        Ok(envelope)
    }

    fn validate(&self) -> Result<(), SearchV2Error> {
        for (child, exhausted) in [
            (&self.nodes, self.nodes_exhausted),
            (&self.relations, self.relations_exhausted),
        ] {
            match (child, exhausted) {
                (None, true) => {}
                (Some(value), false)
                    if !value.is_empty() && value.len() <= REFERENCE_SEARCH_CHILD_MAX_BYTES => {}
                _ => return Err(invalid("indexed cursor child/exhaustion state is invalid")),
            }
        }
        Ok(())
    }
}

fn exact_keys(object: &Map<String, Value>, expected: &[&str]) -> bool {
    object.len() == expected.len() && expected.iter().all(|key| object.contains_key(*key))
}

fn string_array(value: &Value) -> Option<Vec<String>> {
    value
        .as_array()?
        .iter()
        .map(|item| item.as_str().map(str::to_owned))
        .collect()
}

fn parse_filters(value: &Value) -> Result<(Vec<String>, Vec<String>, Vec<String>), SearchV2Error> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("indexed cursor filters are invalid"))?;
    if !exact_keys(object, &["sources", "kind_ids", "predicate_ids"]) {
        return Err(invalid("indexed cursor filters are invalid"));
    }
    let mut sources = string_array(&object["sources"])
        .ok_or_else(|| invalid("indexed cursor sources are invalid"))?;
    let mut kind_ids = string_array(&object["kind_ids"])
        .ok_or_else(|| invalid("indexed cursor kinds are invalid"))?;
    let mut predicate_ids = string_array(&object["predicate_ids"])
        .ok_or_else(|| invalid("indexed cursor predicates are invalid"))?;
    for values in [&mut sources, &mut kind_ids, &mut predicate_ids] {
        let before = values.clone();
        values.sort();
        values.dedup();
        if *values != before || values.iter().any(|value| value.chars().count() > 256) {
            return Err(invalid("indexed cursor filters are not canonical"));
        }
    }
    Ok((sources, kind_ids, predicate_ids))
}

pub(crate) fn decode_reference_indexed_cursor(
    token: &str,
    binding: &ReferenceCursorBinding,
) -> Result<ReferenceCursorEnvelope, SearchV2Error> {
    let value = decode_reference_cursor_payload(token, REFERENCE_INDEXED_CURSOR_MAX_BYTES)?;
    let object = value
        .as_object()
        .ok_or_else(|| invalid("indexed cursor envelope is invalid"))?;
    const KEYS: &[&str] = &[
        "filters",
        "nodes",
        "nodes_exhausted",
        "query",
        "relations",
        "relations_exhausted",
        "schema",
        "source_revision",
    ];
    if !exact_keys(object, KEYS) || object["schema"].as_str() != Some(OUTER_SCHEMA) {
        return Err(invalid("indexed cursor envelope is invalid"));
    }
    let source_revision_matches = match (&object["source_revision"], &binding.source_revision) {
        (Value::Null, None) => true,
        (Value::String(actual), Some(expected)) => actual == expected,
        _ => false,
    };
    if !source_revision_matches {
        return Err(stale_source());
    }
    let (sources, kind_ids, predicate_ids) = parse_filters(&object["filters"])?;
    if object["query"].as_str() != Some(binding.query.as_str())
        || sources != binding.sources
        || kind_ids != binding.kind_ids
        || predicate_ids != binding.predicate_ids
    {
        return Err(stale_query());
    }
    let nodes_exhausted = object["nodes_exhausted"]
        .as_bool()
        .ok_or_else(|| invalid("indexed cursor node exhaustion flag is invalid"))?;
    let relations_exhausted = object["relations_exhausted"]
        .as_bool()
        .ok_or_else(|| invalid("indexed cursor relation exhaustion flag is invalid"))?;
    let child = |value: &Value| -> Result<Option<String>, SearchV2Error> {
        match value {
            Value::Null => Ok(None),
            Value::String(value)
                if !value.is_empty() && value.len() <= REFERENCE_SEARCH_CHILD_MAX_BYTES =>
            {
                Ok(Some(value.clone()))
            }
            _ => Err(invalid("indexed cursor child is invalid")),
        }
    };
    ReferenceCursorEnvelope::new(
        child(&object["nodes"])?,
        child(&object["relations"])?,
        nodes_exhausted,
        relations_exhausted,
    )
}

pub(crate) fn encode_reference_indexed_cursor(
    binding: &ReferenceCursorBinding,
    envelope: &ReferenceCursorEnvelope,
) -> Result<String, SearchV2Error> {
    envelope.validate()?;
    let value = json!({
        "schema": OUTER_SCHEMA,
        "source_revision": binding.source_revision,
        "query": binding.query,
        "filters": binding.filters_value(),
        "nodes": envelope.nodes,
        "relations": envelope.relations,
        "nodes_exhausted": envelope.nodes_exhausted,
        "relations_exhausted": envelope.relations_exhausted,
    });
    encode_reference_cursor_payload(&value, REFERENCE_INDEXED_CURSOR_MAX_BYTES)
}

fn rank_to_number(rank: SearchRank) -> u64 {
    match rank {
        SearchRank::ExactIdentity => 0,
        SearchRank::IdentityPrefix => 1,
        SearchRank::VisibleDisplaySubstring => 2,
        SearchRank::OtherSerializedCarrierSubstring => 3,
    }
}
fn rank_from_number(value: u64) -> Result<SearchRank, SearchV2Error> {
    match value {
        0 => Ok(SearchRank::ExactIdentity),
        1 => Ok(SearchRank::IdentityPrefix),
        2 => Ok(SearchRank::VisibleDisplaySubstring),
        3 => Ok(SearchRank::OtherSerializedCarrierSubstring),
        _ => Err(invalid("indexed cursor rank is invalid")),
    }
}
fn kind_name(kind: SearchKind) -> &'static str {
    match kind {
        SearchKind::Nodes => "nodes",
        SearchKind::Relations => "relations",
    }
}
fn kind_slot(kind: SearchKind) -> usize {
    match kind {
        SearchKind::Nodes => 0,
        SearchKind::Relations => 1,
    }
}

fn parse_read_model_child(
    token: &str,
    kind: SearchKind,
    binding: &ReferenceCursorBinding,
    filters_digest: &str,
) -> Result<(SearchOrderKey, String), SearchV2Error> {
    let value = decode_reference_cursor_payload(token, REFERENCE_SEARCH_CHILD_MAX_BYTES)?;
    let object = value
        .as_object()
        .ok_or_else(|| invalid("indexed search child cursor is invalid"))?;
    const KEYS: &[&str] = &[
        "schema",
        "source_revision",
        "kind",
        "query",
        "n",
        "gram",
        "position",
        "filters_digest",
        "issued_at",
        "expires_at",
        "ordering",
        "rank",
        "id",
    ];
    if !exact_keys(object, KEYS) || object["schema"].as_str() != Some(CHILD_SCHEMA) {
        return Err(invalid("indexed search child cursor is invalid"));
    }
    let source_revision = binding
        .source_revision
        .as_deref()
        .ok_or_else(stale_source)?;
    if object["source_revision"].as_str() != Some(source_revision) {
        return Err(stale_source());
    }
    if object["kind"].as_str() != Some(kind_name(kind))
        || object["query"].as_str() != Some(binding.query.as_str())
        || object["filters_digest"].as_str() != Some(filters_digest)
    {
        return Err(stale_query());
    }
    let integer = |key: &str| {
        object[key]
            .as_u64()
            .ok_or_else(|| invalid("indexed search child cursor integer is invalid"))
    };
    if integer("n")? != u64::from(tos_query::search_index::INDEXED_SEARCH_GRAM_CODEPOINTS_V1)
        || object["ordering"].as_str() != Some("rank-id-position")
    {
        return Err(invalid("indexed search child cursor ordering is invalid"));
    }
    let gram = object["gram"]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid("indexed search child cursor gram is invalid"))?
        .to_owned();
    let expires = integer("expires_at")?;
    let _issued = integer("issued_at")?;
    if expires < reference_cursor_now_seconds()? {
        return Err(error(
            SearchV2ErrorCode::CursorExpired,
            "indexed cursor expired; restart",
        ));
    }
    let rank = rank_from_number(integer("rank")?)?;
    let id = object["id"]
        .as_str()
        .filter(|value| !value.is_empty() && *value == value.to_lowercase())
        .ok_or_else(|| invalid("indexed search child cursor id is invalid"))?
        .to_owned();
    let position = integer("position")?;
    let key = SearchOrderKey::new(rank, id, position)?;
    Ok((key, gram))
}

fn encode_read_model_child(
    kind: SearchKind,
    key: &SearchOrderKey,
    gram: &str,
    binding: &ReferenceCursorBinding,
    filters_digest: &str,
    issued_at: u64,
) -> Result<String, SearchV2Error> {
    if gram.is_empty() {
        return Err(invalid("indexed search selected gram is absent"));
    }
    let value = json!({
        "schema": CHILD_SCHEMA,
        "source_revision": binding.source_revision.as_deref().ok_or_else(stale_source)?,
        "kind": kind_name(kind),
        "query": binding.query,
        "n": tos_query::search_index::INDEXED_SEARCH_GRAM_CODEPOINTS_V1,
        "gram": gram,
        "position": key.source_position(),
        "filters_digest": filters_digest,
        "issued_at": issued_at,
        "expires_at": issued_at.checked_add(REFERENCE_CURSOR_TTL_SECONDS).ok_or_else(|| invalid("indexed cursor expiry overflow"))?,
        "ordering": "rank-id-position",
        "rank": rank_to_number(key.rank()),
        "id": key.lower_id(),
    });
    encode_reference_cursor_payload(&value, REFERENCE_SEARCH_CHILD_MAX_BYTES)
}

/// CMP SourceRoot adapter. QRY still validates the fresh source, full
/// selection, query, current policy, authorization and lease on every page.
pub(crate) struct NativeReferenceIndexedCursorCodec {
    initial: SearchContinuationState,
    binding: ReferenceCursorBinding,
    filters_digest: String,
    decoded_grams: [Option<String>; 2],
    selected_grams: [Option<String>; 2],
}

impl NativeReferenceIndexedCursorCodec {
    pub(crate) fn new(
        initial: SearchContinuationState,
        source_revision: &str,
        registered_source_ids: &[String],
    ) -> Result<Self, SearchV2Error> {
        let request = initial.request();
        let sources = request.sources().unwrap_or(registered_source_ids).to_vec();
        let binding = ReferenceCursorBinding::new(
            source_revision,
            request.query(),
            sources,
            request.kind_ids().to_vec(),
            request.predicate_ids().to_vec(),
        )?;
        let filters_digest = binding.filters_digest()?;
        Ok(Self {
            initial,
            binding,
            filters_digest,
            decoded_grams: [None, None],
            selected_grams: [None, None],
        })
    }
}

impl IndexedWireCursorCodec for NativeReferenceIndexedCursorCodec {
    fn decode(&mut self, token: &str) -> Result<SearchContinuationState, SearchV2Error> {
        let envelope = decode_reference_indexed_cursor(token, &self.binding)?;
        let mut parse_child = |child: Option<String>,
                               exhausted: bool,
                               kind: SearchKind|
         -> Result<SearchContinuationProgress, SearchV2Error> {
            if exhausted {
                return Ok(SearchContinuationProgress::Exhausted);
            }
            let child = child.ok_or_else(|| invalid("indexed cursor child is absent"))?;
            let (key, gram) =
                parse_read_model_child(&child, kind, &self.binding, &self.filters_digest)?;
            self.decoded_grams[kind_slot(kind)] = Some(gram);
            Ok(SearchContinuationProgress::After(key))
        };
        let nodes = parse_child(envelope.nodes, envelope.nodes_exhausted, SearchKind::Nodes)?;
        let relations = parse_child(
            envelope.relations,
            envelope.relations_exhausted,
            SearchKind::Relations,
        )?;
        drop(parse_child);
        self.initial.from_untrusted_progress(nodes, relations)
    }

    fn observe_page_gram(
        &mut self,
        kind: SearchKind,
        selected_gram: Option<&str>,
    ) -> Result<(), SearchV2Error> {
        let slot = kind_slot(kind);
        if let Some(decoded) = self.decoded_grams[slot].as_deref() {
            if selected_gram != Some(decoded) {
                return Err(invalid("indexed cursor selected gram changed"));
            }
        }
        self.selected_grams[slot] = selected_gram.map(str::to_owned);
        Ok(())
    }

    fn encode(&mut self, state: &SearchContinuationState) -> Result<String, SearchV2Error> {
        if state.cursor_bindings_v1() != self.initial.cursor_bindings_v1() {
            return Err(stale_query());
        }
        let issued_at = reference_cursor_now_seconds()?;
        let child = |kind: SearchKind,
                     progress: SearchContinuationProgress|
         -> Result<Option<String>, SearchV2Error> {
            match progress {
                SearchContinuationProgress::Exhausted => Ok(None),
                SearchContinuationProgress::After(key) => {
                    let gram = self.selected_grams[kind_slot(kind)]
                        .as_deref()
                        .ok_or_else(|| invalid("indexed cursor selected gram is absent"))?;
                    encode_read_model_child(
                        kind,
                        &key,
                        gram,
                        &self.binding,
                        &self.filters_digest,
                        issued_at,
                    )
                    .map(Some)
                }
                SearchContinuationProgress::Fresh => {
                    Err(invalid("indexed cursor has no continuation progress"))
                }
            }
        };
        let nodes = child(SearchKind::Nodes, state.wire_progress(SearchKind::Nodes))?;
        let relations = child(
            SearchKind::Relations,
            state.wire_progress(SearchKind::Relations),
        )?;
        drop(child);
        let envelope = ReferenceCursorEnvelope::new(
            nodes,
            relations,
            state.is_exhausted(SearchKind::Nodes),
            state.is_exhausted(SearchKind::Relations),
        )?;
        encode_reference_indexed_cursor(&self.binding, &envelope)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_wire_keys_and_filter_hash_are_sorted_with_any_map_features() {
        let value = json!({"z": [{"z": "é", "a": 1}], "a": true});
        let token = encode_reference_cursor_payload(&value, 1024).unwrap();
        assert_eq!(
            base64url_decode(&token, 1024).unwrap(),
            "{\"a\":true,\"z\":[{\"a\":1,\"z\":\"é\"}]}".as_bytes(),
        );
        let binding =
            ReferenceCursorBinding::new("revision", "query", vec![], vec![], vec![]).unwrap();
        assert_eq!(
            binding.filters_digest().unwrap(),
            Digest256::of_bytes(br#"{"kind_ids":[],"predicate_ids":[],"sources":[]}"#,).to_hex()
        );
    }
}
