//! Bounded, versioned and deliberately unsigned indexed paging request.
//! Fresh QRY selection/current bindings and ReleaseLease authorize every page;
//! the token only carries where a caller asks to resume public search order.
use std::time::{SystemTime, UNIX_EPOCH};
use tos_foundation::{Digest256, Digest256Hasher};
use tos_query::{
    IndexedWireCursorCodec,
    search_v2::{
        SearchContinuationProgress, SearchContinuationState, SearchKind, SearchOrderKey,
        SearchRank, SearchV2Error, SearchV2ErrorCode,
    },
};

const MAGIC: &[u8; 8] = b"TOSIDX01";
const TTL_SECONDS: u64 = 900;
const MAX_KEY_BYTES: usize = 3 * 1024;
pub(crate) const MAX_CURSOR_BYTES: usize = 16 * 1024;
const HEX: &[u8; 16] = b"0123456789abcdef";

pub(crate) struct NativeIndexedCursorCodec {
    initial: SearchContinuationState,
    model_receipt_digest: Digest256,
}

fn error(code: SearchV2ErrorCode, message: &'static str) -> SearchV2Error {
    SearchV2Error { code, message }
}
fn invalid(message: &'static str) -> SearchV2Error {
    error(SearchV2ErrorCode::InvalidRequest, message)
}
fn budget() -> SearchV2Error {
    error(
        SearchV2ErrorCode::BudgetExceeded,
        "indexed cursor byte budget exceeded",
    )
}
fn now_seconds() -> Result<u64, SearchV2Error> {
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
fn rank_byte(rank: SearchRank) -> u8 {
    match rank {
        SearchRank::ExactIdentity => 0,
        SearchRank::IdentityPrefix => 1,
        SearchRank::VisibleDisplaySubstring => 2,
        SearchRank::OtherSerializedCarrierSubstring => 3,
    }
}
fn rank_from_byte(value: u8) -> Result<SearchRank, SearchV2Error> {
    match value {
        0 => Ok(SearchRank::ExactIdentity),
        1 => Ok(SearchRank::IdentityPrefix),
        2 => Ok(SearchRank::VisibleDisplaySubstring),
        3 => Ok(SearchRank::OtherSerializedCarrierSubstring),
        _ => Err(invalid("indexed cursor rank is invalid")),
    }
}
fn push_progress(
    raw: &mut Vec<u8>,
    progress: SearchContinuationProgress,
) -> Result<(), SearchV2Error> {
    match progress {
        SearchContinuationProgress::Fresh => raw.push(0),
        SearchContinuationProgress::Exhausted => raw.push(1),
        SearchContinuationProgress::After(key) => {
            let id = key.lower_id().as_bytes();
            if id.is_empty() || id.len() > MAX_KEY_BYTES {
                return Err(budget());
            }
            raw.push(2);
            raw.push(rank_byte(key.rank()));
            raw.extend_from_slice(&key.source_position().to_be_bytes());
            raw.extend_from_slice(&(id.len() as u16).to_be_bytes());
            raw.extend_from_slice(id);
        }
    }
    Ok(())
}
fn hex_encode(raw: &[u8]) -> Result<String, SearchV2Error> {
    if raw
        .len()
        .checked_mul(2)
        .is_none_or(|len| len > MAX_CURSOR_BYTES)
    {
        return Err(budget());
    }
    let mut out = String::with_capacity(raw.len() * 2);
    for byte in raw {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 15) as usize] as char);
    }
    Ok(out)
}
fn hex_decode(token: &str) -> Result<Vec<u8>, SearchV2Error> {
    if token.is_empty() || token.len() > MAX_CURSOR_BYTES || !token.len().is_multiple_of(2) {
        return Err(invalid("indexed cursor encoding is invalid"));
    }
    let nibble = |byte: u8| match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    };
    token
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = nibble(pair[0]).ok_or_else(|| invalid("indexed cursor hex is invalid"))?;
            let low = nibble(pair[1]).ok_or_else(|| invalid("indexed cursor hex is invalid"))?;
            Ok((high << 4) | low)
        })
        .collect()
}

struct Reader<'a> {
    raw: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8], SearchV2Error> {
        let end = self
            .at
            .checked_add(len)
            .ok_or_else(|| invalid("indexed cursor length overflow"))?;
        let value = self
            .raw
            .get(self.at..end)
            .ok_or_else(|| invalid("indexed cursor is truncated"))?;
        self.at = end;
        Ok(value)
    }
    fn byte(&mut self) -> Result<u8, SearchV2Error> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, SearchV2Error> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, SearchV2Error> {
        Ok(u64::from_be_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn digest(&mut self) -> Result<Digest256, SearchV2Error> {
        Ok(Digest256::from_bytes(self.take(32)?.try_into().unwrap()))
    }
    fn progress(&mut self) -> Result<SearchContinuationProgress, SearchV2Error> {
        match self.byte()? {
            0 => Ok(SearchContinuationProgress::Fresh),
            1 => Ok(SearchContinuationProgress::Exhausted),
            2 => {
                let rank = rank_from_byte(self.byte()?)?;
                let position = self.u64()?;
                let len = usize::from(self.u16()?);
                if len == 0 || len > MAX_KEY_BYTES {
                    return Err(invalid("indexed cursor key byte length is invalid"));
                }
                let id = std::str::from_utf8(self.take(len)?)
                    .map_err(|_| invalid("indexed cursor key is not UTF-8"))?;
                let key = SearchOrderKey::new(rank, id.to_owned(), position)?;
                Ok(SearchContinuationProgress::After(key))
            }
            _ => Err(invalid("indexed cursor progress tag is invalid")),
        }
    }
}

impl NativeIndexedCursorCodec {
    pub(crate) fn new(initial: SearchContinuationState, model_receipt_id: &str) -> Self {
        let mut receipt = Digest256Hasher::new();
        receipt.update(b"tos-native-indexed-model-receipt-v1\0");
        receipt.update(&(model_receipt_id.len() as u64).to_be_bytes());
        receipt.update(model_receipt_id.as_bytes());
        Self {
            initial,
            model_receipt_digest: receipt.finalize(),
        }
    }
    fn selected_binding(&self) -> Digest256 {
        let mut binding = Digest256Hasher::new();
        binding.update(b"tos-native-indexed-selection-v1\0");
        binding.update(self.initial.cursor_bindings_v1()[0].as_bytes());
        binding.update(self.model_receipt_digest.as_bytes());
        binding.finalize()
    }
}
impl IndexedWireCursorCodec for NativeIndexedCursorCodec {
    fn decode(&mut self, token: &str) -> Result<SearchContinuationState, SearchV2Error> {
        let raw = hex_decode(token)?;
        let mut read = Reader { raw: &raw, at: 0 };
        if read.take(MAGIC.len())? != MAGIC {
            return Err(invalid("indexed cursor version is invalid"));
        }
        let expires = read.u64()?;
        let now = now_seconds()?;
        if expires < now {
            return Err(error(
                SearchV2ErrorCode::CursorExpired,
                "indexed cursor expired; restart",
            ));
        }
        if expires > now.saturating_add(TTL_SECONDS) {
            return Err(invalid("indexed cursor expiry is outside native range"));
        }
        let [_, request, policy] = self.initial.cursor_bindings_v1();
        let selected = self.selected_binding();
        if read.digest()? != selected {
            return Err(error(
                SearchV2ErrorCode::StaleSelection,
                "indexed cursor selected model or source changed",
            ));
        }
        if read.digest()? != request {
            return Err(error(
                SearchV2ErrorCode::StaleContinuation,
                "indexed cursor query or filters changed",
            ));
        }
        if read.digest()? != policy {
            return Err(error(
                SearchV2ErrorCode::StalePolicy,
                "indexed cursor current release policy changed",
            ));
        }
        let nodes = read.progress()?;
        let relations = read.progress()?;
        if read.at != raw.len() {
            return Err(invalid("indexed cursor has trailing bytes"));
        }
        self.initial.from_untrusted_progress(nodes, relations)
    }

    fn encode(&mut self, state: &SearchContinuationState) -> Result<String, SearchV2Error> {
        if state.cursor_bindings_v1() != self.initial.cursor_bindings_v1() {
            return Err(error(
                SearchV2ErrorCode::StaleContinuation,
                "indexed cursor result changed binding",
            ));
        }
        let expires = now_seconds()?.checked_add(TTL_SECONDS).ok_or_else(budget)?;
        let mut raw = Vec::with_capacity(256);
        raw.extend_from_slice(MAGIC);
        raw.extend_from_slice(&expires.to_be_bytes());
        let [_, request, policy] = state.cursor_bindings_v1();
        for digest in [self.selected_binding(), request, policy] {
            raw.extend_from_slice(digest.as_bytes());
        }
        push_progress(&mut raw, state.wire_progress(SearchKind::Nodes))?;
        push_progress(&mut raw, state.wire_progress(SearchKind::Relations))?;
        hex_encode(&raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tos_foundation::KNOWLEDGE_POSTINGS_MODEL_ABIS;
    use tos_query::search_v2::{
        CurrentPolicyBinding, IndexedSearchV2Request, QUERY_PRIMITIVE_PROFILE,
        QueryVocabularyBinding, SEARCH_UNICODE_PROFILE, SearchSelectionBinding,
        SelectedQueryVocabulary,
    };

    struct Vocabulary(QueryVocabularyBinding);
    impl SelectedQueryVocabulary for Vocabulary {
        fn binding(&self) -> &QueryVocabularyBinding {
            &self.0
        }
        fn registered_source_ids(&self) -> &[String] {
            &[]
        }
    }
    fn base(policy_epoch: &str, query: &str) -> SearchContinuationState {
        base_with_cut(policy_epoch, query, "selected-cut")
    }
    fn base_with_cut(policy_epoch: &str, query: &str, source_cut: &str) -> SearchContinuationState {
        let digest = Digest256::of_bytes(b"native cursor fixture");
        let vocabulary = Vocabulary(QueryVocabularyBinding {
            descriptor_sha256: digest,
            descriptor_version: 1,
        });
        let selection = SearchSelectionBinding {
            model_abi: KNOWLEDGE_POSTINGS_MODEL_ABIS[0].into(),
            vocabulary: vocabulary.0.clone(),
            semantic_primitive_profile: QUERY_PRIMITIVE_PROFILE.into(),
            search_unicode_profile: SEARCH_UNICODE_PROFILE.into(),
            source_cut: source_cut.into(),
            through_commit_seq: 9,
            source_membership_root: digest,
            history_root_sha256: None,
            entity_registry_id: "entity".into(),
            entity_registry_version: "v1".into(),
            entity_registry_sha256: digest,
            relation_registry_id: "relation".into(),
            relation_registry_version: "v1".into(),
            relation_registry_sha256: digest,
            graph_root_sha256: digest,
            catalog_packet_sha256: digest,
            catalog_index_root_sha256: digest,
            source_scope_root_sha256: digest,
            search_index_root_sha256: digest,
            index_root_sha256: digest,
            index_generation: "generation".into(),
            route_map_version: "routes".into(),
            reader_abi: "reader".into(),
            complete: true,
        };
        let request = IndexedSearchV2Request {
            query: query.into(),
            sources: vec![],
            kind_ids: vec![],
            predicate_ids: vec![],
            limit: 1,
        }
        .normalize(&selection, &vocabulary)
        .unwrap();
        SearchContinuationState::new(
            selection,
            request,
            CurrentPolicyBinding {
                scope: "selected public projection".into(),
                issuer_ref: "local release holder".into(),
                authorization_receipt_id: "pair".into(),
                policy_epoch: policy_epoch.into(),
                withdrawal_generation: "pair".into(),
            },
            &vocabulary,
        )
        .unwrap()
    }

    #[test]
    fn unsigned_cursor_roundtrips_progress_across_instances() {
        let initial = base("epoch", "search");
        let mut state = initial.clone();
        for position in [1, 2, 7] {
            state
                .advance(
                    SearchKind::Nodes,
                    Some(
                        SearchOrderKey::new(
                            SearchRank::IdentityPrefix,
                            format!("id-{position:04}"),
                            position,
                        )
                        .unwrap(),
                    ),
                    false,
                )
                .unwrap();
            let token = NativeIndexedCursorCodec::new(initial.clone(), "receipt-a")
                .encode(&state)
                .unwrap();
            let decoded = NativeIndexedCursorCodec::new(initial.clone(), "receipt-a")
                .decode(&token)
                .unwrap();
            assert_eq!(decoded, state);
        }
    }

    #[test]
    fn cursor_refuses_changed_binding_bad_expiry_and_malformed_bytes() {
        let initial = base("epoch", "search");
        let mut state = initial.clone();
        state
            .advance(
                SearchKind::Nodes,
                Some(SearchOrderKey::new(SearchRank::ExactIdentity, "id".into(), 1).unwrap()),
                false,
            )
            .unwrap();
        let token = NativeIndexedCursorCodec::new(initial.clone(), "receipt-a")
            .encode(&state)
            .unwrap();
        assert_eq!(
            NativeIndexedCursorCodec::new(initial.clone(), "receipt-b")
                .decode(&token)
                .unwrap_err()
                .code,
            SearchV2ErrorCode::StaleSelection
        );
        assert_eq!(
            NativeIndexedCursorCodec::new(
                base_with_cut("epoch", "search", "other cut"),
                "receipt-a"
            )
            .decode(&token)
            .unwrap_err()
            .code,
            SearchV2ErrorCode::StaleSelection
        );
        assert_eq!(
            NativeIndexedCursorCodec::new(base("other epoch", "search"), "receipt-a")
                .decode(&token)
                .unwrap_err()
                .code,
            SearchV2ErrorCode::StalePolicy
        );
        assert_eq!(
            NativeIndexedCursorCodec::new(base("epoch", "other"), "receipt-a")
                .decode(&token)
                .unwrap_err()
                .code,
            SearchV2ErrorCode::StaleContinuation
        );
        let mut expired = hex_decode(&token).unwrap();
        expired[8..16].copy_from_slice(&0u64.to_be_bytes());
        assert_eq!(
            NativeIndexedCursorCodec::new(initial.clone(), "receipt-a")
                .decode(&hex_encode(&expired).unwrap())
                .unwrap_err()
                .code,
            SearchV2ErrorCode::CursorExpired
        );
        expired[8..16].copy_from_slice(&u64::MAX.to_be_bytes());
        assert_eq!(
            NativeIndexedCursorCodec::new(initial.clone(), "receipt-a")
                .decode(&hex_encode(&expired).unwrap())
                .unwrap_err()
                .code,
            SearchV2ErrorCode::InvalidRequest
        );
        for bad in ["", "Z", &token[..token.len() - 1]] {
            assert_eq!(
                NativeIndexedCursorCodec::new(initial.clone(), "receipt-a")
                    .decode(bad)
                    .unwrap_err()
                    .code,
                SearchV2ErrorCode::InvalidRequest
            );
        }
    }
}
