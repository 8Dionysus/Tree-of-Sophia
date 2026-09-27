//! Maintained search mode parsing; each engine remains an owner QRY function.
use crate::{AccessError, AccessErrorCode, AccessExecutor, IndexedSearchParams, PreparedPacket};
use std::sync::Arc;
use tos_foundation::JsonValue;
use tos_query::AbortProbe;
use tos_query::knowledge_legacy_search::LegacySearchRequest;

pub enum SearchRequest {
    Legacy(LegacySearchRequest),
    Indexed(IndexedSearchParams),
    Compressed,
}
impl SearchRequest {
    pub fn from_arguments(args: &JsonValue) -> Result<Self, AccessError> {
        let invalid =
            || AccessError::new(AccessErrorCode::InvalidRequest, "invalid search arguments");
        let fields = args.as_object().ok_or_else(invalid)?;
        let allowed = [
            "query",
            "sources",
            "kind_ids",
            "predicate_ids",
            "offset",
            "limit",
            "mode",
            "cursor",
        ];
        if fields
            .iter()
            .any(|(key, _)| !key.as_str().is_some_and(|key| allowed.contains(&key)))
        {
            return Err(invalid());
        }
        let mode = match args.object_get("mode") {
            None => "legacy",
            Some(value) => value.as_str().ok_or_else(invalid)?,
        };
        let clean = JsonValue::Object(
            fields
                .iter()
                .filter(|(key, value)| {
                    !(matches!(
                        key.as_str(),
                        Some("sources" | "kind_ids" | "predicate_ids" | "cursor")
                    ) && matches!(value, JsonValue::Null))
                })
                .cloned()
                .collect(),
        );
        if mode == "indexed" {
            return IndexedSearchParams::from_json(&clean).map(Self::Indexed);
        }
        if !matches!(mode, "legacy" | "compressed") {
            return Err(invalid());
        }
        let number = |key, default| match clean.object_get(key) {
            None => Ok(default),
            Some(value) => value
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .ok_or_else(invalid),
        };
        let offset = number("offset", 0)?;
        if mode == "compressed" {
            if offset != 0 {
                return Err(AccessError::new(
                    AccessErrorCode::InvalidRequest,
                    "compressed search uses cursor, not offset",
                ));
            }
            return Ok(Self::Compressed);
        }
        let strings = |key| -> Result<Option<Vec<String>>, AccessError> {
            clean
                .object_get(key)
                .map(|value| {
                    value
                        .as_array()
                        .ok_or_else(invalid)?
                        .iter()
                        .map(|value| value.as_str().map(str::to_owned).ok_or_else(invalid))
                        .collect()
                })
                .transpose()
        };
        Ok(Self::Legacy(LegacySearchRequest {
            query: clean
                .object_get("query")
                .map(|value| value.as_str().ok_or_else(invalid))
                .transpose()?
                .unwrap_or("")
                .into(),
            sources: strings("sources")?,
            kind_ids: strings("kind_ids")?.unwrap_or_default(),
            predicate_ids: strings("predicate_ids")?.unwrap_or_default(),
            offset,
            limit: number("limit", 40)?,
        }))
    }
    pub fn execute(
        self,
        executor: &dyn AccessExecutor,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket, AccessError> {
        match self {
            Self::Legacy(request) if executor.knowledge_search_legacy_available() => {
                executor.knowledge_search_legacy(request, probe)
            }
            Self::Indexed(request) if executor.knowledge_search_indexed_available() => {
                executor.knowledge_search_indexed(request, probe)
            }
            _ => Err(AccessError::new(
                AccessErrorCode::Unavailable,
                "selected search mode unavailable",
            )),
        }
    }
}
