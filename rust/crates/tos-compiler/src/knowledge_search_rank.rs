//! ABI-selected physical rank fields. Decoded JSON and ranking semantics stay exact.
use crate::{
    Error, Result, knowledge_byte_codec as codec, knowledge_stage::KnowledgePayloadLayout,
};
use rusqlite::types::ValueRef;

pub(crate) struct SqlField<'a>(pub(crate) ValueRef<'a>);
impl rusqlite::ToSql for SqlField<'_> {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(rusqlite::types::ToSqlOutput::Borrowed(self.0))
    }
}

pub fn search_rank_fields_packed(abi: &str) -> bool {
    abi == tos_foundation::KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V2_CARRIER_ONCE_V4
}

pub fn search_rank_field_size(abi: &str, value: ValueRef<'_>, max_bytes: usize) -> Result<usize> {
    if max_bytes == 0 || max_bytes > 8 * 1024 * 1024 {
        return Err(Error::Budget("search rank field limit"));
    }
    match (search_rank_fields_packed(abi), value) {
        (false, ValueRef::Text(raw)) if raw.len() <= max_bytes => Ok(raw.len()),
        (true, ValueRef::Blob(stored)) if stored.len() <= codec::stored_bound(max_bytes)? => {
            codec::frame_metadata(stored, None, max_bytes).map(|v| v.0)
        }
        _ => Err(Error::Invalid(
            "search rank physical field differs from model ABI",
        )),
    }
}

pub fn search_rank_decode_workspace(max_bytes: usize) -> Result<usize> {
    max_bytes
        .checked_add(1)
        .and_then(|n| n.checked_add(codec::decoder_workspace_upper().ok()?))
        .ok_or(Error::Budget("search rank decode workspace"))
}

pub fn search_rank_decode_work_limit(max_bytes: usize) -> Result<u64> {
    (max_bytes as u64)
        .checked_add(codec::HEADER as u64)
        .and_then(|n| n.checked_mul(16))
        .and_then(|n| n.checked_add(1024))
        .ok_or(Error::Budget("search rank decode work"))
}

/// The caller admits the encoded SQL field and complete decoder workspace
/// before entering. Allocation, output length and decode work are finite.
pub fn decode_search_rank_field(
    abi: &str,
    value: ValueRef<'_>,
    max_bytes: usize,
    available: usize,
    work: &mut u64,
    work_cap: u64,
) -> Result<String> {
    let length = search_rank_field_size(abi, value, max_bytes)?;
    let bytes = match value {
        ValueRef::Text(raw) if !search_rank_fields_packed(abi) => {
            if length > available {
                return Err(Error::Budget("search rank text workspace"));
            }
            crate::knowledge_original_rows::charge_decode_work(work, work_cap, raw.len())?;
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(raw.len())
                .map_err(|_| Error::Budget("search rank text allocation"))?;
            if bytes.capacity() != raw.len() {
                return Err(Error::Budget("search rank text capacity"));
            }
            bytes.extend_from_slice(raw);
            bytes
        }
        ValueRef::Blob(stored) if search_rank_fields_packed(abi) => {
            crate::knowledge_original_rows::decode_packet(
                stored,
                length,
                max_bytes,
                available,
                KnowledgePayloadLayout::CarrierOnceV2,
                work,
                work_cap,
            )?
        }
        _ => return Err(Error::Invalid("search rank field ABI")),
    };
    crate::knowledge_original_rows::charge_decode_work(work, work_cap, bytes.len())?;
    String::from_utf8(bytes).map_err(|_| Error::Invalid("search rank decoded UTF8"))
}

pub(crate) fn with_encoded_pair<T>(
    packed: bool,
    state: Option<&crate::d1_public_capture::CreationState<'_>>,
    identity: &str,
    visible: &str,
    max_bytes: usize,
    consume: impl FnOnce(ValueRef<'_>, ValueRef<'_>) -> Result<T>,
) -> Result<T> {
    if packed {
        let state = state.ok_or(Error::Budget("packed search ranks require owned creation"))?;
        codec::with_encoded(state, identity.as_bytes(), max_bytes, |identity| {
            codec::with_encoded(state, visible.as_bytes(), max_bytes, |visible| {
                consume(ValueRef::Blob(identity), ValueRef::Blob(visible))
            })
        })
    } else {
        consume(
            ValueRef::Text(identity.as_bytes()),
            ValueRef::Text(visible.as_bytes()),
        )
    }
}

pub(crate) fn decode_pair_owned(
    abi: &str,
    identity: ValueRef<'_>,
    visible: ValueRef<'_>,
    max_bytes: usize,
    state: &crate::d1_public_capture::CreationState<'_>,
) -> Result<(String, String)> {
    if !search_rank_fields_packed(abi) {
        search_rank_field_size(abi, identity, max_bytes)?;
        search_rank_field_size(abi, visible, max_bytes)?;
        let (ValueRef::Text(a), ValueRef::Text(b)) = (identity, visible) else {
            unreachable!()
        };
        return Ok((
            std::str::from_utf8(a)
                .map_err(|_| Error::Invalid("rank UTF8"))?
                .to_owned(),
            std::str::from_utf8(b)
                .map_err(|_| Error::Invalid("rank UTF8"))?
                .to_owned(),
        ));
    }
    let a = search_rank_field_size(abi, identity, max_bytes)?;
    let b = search_rank_field_size(abi, visible, max_bytes)?;
    let workspace = search_rank_decode_workspace(a.max(b))?;
    let bytes = a
        .checked_add(b)
        .and_then(|n| n.checked_add(workspace))
        .ok_or(Error::Budget("search rank pair workspace"))?;
    let _hold = state.hold(bytes)?;
    let cap = search_rank_decode_work_limit(
        a.checked_add(b)
            .ok_or(Error::Budget("search rank pair work"))?,
    )?;
    state.active()?;
    state
        .charge_work(usize::try_from(cap).map_err(|_| Error::Budget("search rank work charge"))?)?;
    let mut work = 0;
    let identity = decode_search_rank_field(abi, identity, max_bytes, workspace, &mut work, cap)?;
    let visible = decode_search_rank_field(abi, visible, max_bytes, workspace, &mut work, cap)?;
    state.active()?;
    Ok((identity, visible))
}
