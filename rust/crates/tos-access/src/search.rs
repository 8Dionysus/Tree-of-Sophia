//! Maintained search mode parsing; each engine remains an owner QRY function.
use crate::{AccessError, AccessErrorCode, IndexedSearchParams, PreparedPacket};
use std::sync::Arc;
use tos_foundation::JsonValue;
use tos_query::AbortProbe;
use tos_query::knowledge_legacy_search::{
    LegacySearchIntegerInput, LegacySearchRequest, normalize_legacy_search_integer,
};

fn legacy_number(
    value: Option<&JsonValue>,
    default: usize,
    minimum: usize,
    maximum: usize,
) -> Result<usize, AccessError> {
    let input = match value {
        None => LegacySearchIntegerInput::Missing,
        Some(JsonValue::Null) => LegacySearchIntegerInput::Null,
        Some(JsonValue::Bool(_)) => LegacySearchIntegerInput::Boolean,
        Some(JsonValue::Number(number)) if number.kind == tos_foundation::JsonNumberKind::Int => {
            LegacySearchIntegerInput::Integer(&number.lexeme)
        }
        Some(JsonValue::Number(number)) => {
            LegacySearchIntegerInput::Float(number.as_python_float().unwrap_or(f64::NAN))
        }
        Some(JsonValue::String(value)) => {
            LegacySearchIntegerInput::String(value.as_str().unwrap_or(""))
        }
        Some(_) => LegacySearchIntegerInput::Other,
    };
    normalize_legacy_search_integer(input, default, minimum, maximum)
        .map_err(|_| AccessError::new(AccessErrorCode::InvalidRequest, "invalid search arguments"))
}

pub enum SearchRequest {
    Legacy(LegacySearchRequest),
    Indexed(IndexedSearchParams),
    Compressed(tos_query::compressed_search::CompressedSearchRequest),
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
        if mode == "compressed" {
            let offset = number("offset", 0)?;
            if offset != 0 {
                return Err(AccessError::new(
                    AccessErrorCode::InvalidRequest,
                    "compressed search uses cursor, not offset",
                ));
            }
            let args = JsonValue::Object(
                clean
                    .as_object()
                    .ok_or_else(invalid)?
                    .iter()
                    .filter(|(key, _)| !matches!(key.as_str(), Some("mode" | "offset")))
                    .cloned()
                    .collect(),
            );
            return tos_query::compressed_search::CompressedSearchRequest::from_json(&args)
                .map(Self::Compressed)
                .map_err(|_| invalid());
        }
        let offset = legacy_number(clean.object_get("offset"), 0, 0, 100_000)?;
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
            limit: legacy_number(clean.object_get("limit"), 40, 1, 100)?,
        }))
    }
    pub fn execute<'hold, E: crate::common::ScopedAccessExecutor<'hold> + ?Sized>(
        self,
        executor: &E,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'hold>, AccessError> {
        match self {
            Self::Legacy(request) if executor.knowledge_search_legacy_available() => {
                executor.knowledge_search_legacy(request, probe)
            }
            Self::Indexed(request) if executor.knowledge_search_indexed_available() => {
                executor.knowledge_search_indexed(request, probe)
            }
            Self::Compressed(request) if executor.knowledge_search_compressed_available() => {
                executor.knowledge_search_compressed(request, probe)
            }
            _ => Err(AccessError::new(
                AccessErrorCode::Unavailable,
                "selected search mode unavailable",
            )),
        }
    }
}
