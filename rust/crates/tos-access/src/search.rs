//! Parse the selected compressed search request.
use crate::{AccessError, AccessErrorCode, PreparedPacket};
use std::sync::Arc;
use tos_foundation::JsonValue;
use tos_query::AbortProbe;

pub struct SearchRequest(tos_query::compressed_search::CompressedSearchRequest);
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
            || args
                .object_get("mode")
                .is_some_and(|value| value.as_str() != Some("compressed"))
            || args
                .object_get("offset")
                .is_some_and(|value| value.as_u64() != Some(0))
        {
            return Err(invalid());
        }
        let clean = JsonValue::Object(
            fields
                .iter()
                .filter(|(key, value)| {
                    !matches!(key.as_str(), Some("mode" | "offset"))
                        && !(matches!(
                            key.as_str(),
                            Some("sources" | "kind_ids" | "predicate_ids" | "cursor")
                        ) && matches!(value, JsonValue::Null))
                })
                .cloned()
                .collect(),
        );
        tos_query::compressed_search::CompressedSearchRequest::from_json(&clean)
            .map(Self)
            .map_err(|_| invalid())
    }
    pub fn execute<'hold, E: crate::common::ScopedAccessExecutor<'hold> + ?Sized>(
        self,
        executor: &E,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'hold>, AccessError> {
        if executor.knowledge_search_compressed_available() {
            executor.knowledge_search_compressed(self.0, probe)
        } else {
            Err(AccessError::new(
                AccessErrorCode::Unavailable,
                "selected search mode unavailable",
            ))
        }
    }
}
