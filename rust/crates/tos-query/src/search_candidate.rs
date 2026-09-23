//! Verify one bounded selected search candidate against its exact carrier.
//! A gram posting is only a necessary substring condition, never a hit proof.

use tos_foundation::{
    CanonicalProfile, Digest256, FoundationErrorCode, JsonLimits, JsonMode, JsonString, JsonValue,
    canonical_bytes_v1, parse_json, python_lower_unicode16_v1,
};

use crate::search_v2::{
    NormalizedIndexedSearchV2Request, SearchKind, SearchOrderKey, SearchRank, SearchV2Error,
    SearchV2ErrorCode, SelectedQueryVocabulary,
};
use crate::{SearchDocumentBudget, verify_indexed_search_document};

#[derive(Clone, Debug)]
pub struct SelectedSearchCandidate {
    pub kind: SearchKind,
    pub position: u64,
    pub id: String,
    pub source_graph: String,
    pub kind_id: String,
    pub predicate_id: String,
    pub id_lower: String,
    pub native_id_lower: String,
    pub identity_values: String,
    pub visible_values: String,
    pub document_chars: u64,
    pub document_digest: Digest256,
    pub payload_sha256: Digest256,
    pub payload: Vec<u8>,
}

#[derive(Clone, Copy, Debug)]
pub struct CandidateReadBudget {
    pub max_vm_steps: u64,
    pub max_decoded_bytes: u64,
    pub max_payload_bytes: usize,
    pub max_field_bytes: usize,
    pub max_document_chars: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateReadCharge {
    pub vm_steps: u64,
    pub rows: u64,
    pub decoded_bytes: u64,
}

/// One exact row and carrier from the pinned complete selected model. The
/// implementation must bound every selected field/BLOB before transfer and
/// interrupt SQL at the supplied VM cap. It never grants current disclosure.
pub trait SearchCandidateModel {
    fn exact_candidate(
        &mut self,
        kind: SearchKind,
        position: u64,
        budget: CandidateReadBudget,
    ) -> Result<(SelectedSearchCandidate, CandidateReadCharge), SearchV2Error>;
}

#[derive(Clone, Copy, Debug)]
pub struct CandidateVerifyBudget {
    pub document: SearchDocumentBudget,
    pub max_rank_field_bytes: usize,
    pub max_rank_values: usize,
}

#[derive(Clone, Debug)]
pub struct VerifiedSearchCandidate {
    pub order: SearchOrderKey,
    pub source_graph: String,
    pub id: String,
    pub payload: Vec<u8>,
    pub payload_sha256: Digest256,
}

fn error(code: SearchV2ErrorCode, message: &'static str) -> SearchV2Error {
    SearchV2Error { code, message }
}

fn foundation_error(reason: tos_foundation::FoundationError) -> SearchV2Error {
    if reason.code == FoundationErrorCode::BudgetExceeded {
        error(
            SearchV2ErrorCode::BudgetExceeded,
            "search candidate verification budget exceeded",
        )
    } else {
        error(
            SearchV2ErrorCode::CorruptSelectedCarrier,
            "search candidate JSON invalid",
        )
    }
}

fn lower(value: &str, cap: usize) -> Result<String, SearchV2Error> {
    if value.len() > cap {
        return Err(error(
            SearchV2ErrorCode::BudgetExceeded,
            "rank value exceeds byte cap",
        ));
    }
    python_lower_unicode16_v1(value, cap, cap, cap).map_err(foundation_error)
}

fn rank_values(
    display: Option<&JsonValue>,
    fields: &[&str],
    budget: CandidateVerifyBudget,
) -> Result<Vec<String>, SearchV2Error> {
    let mut values = Vec::new();
    let mut decoded_bytes = 0usize;
    for field in fields {
        let Some(value) = display.and_then(|display| display.object_get(field)) else {
            continue;
        };
        let mut add = |text: &str| -> Result<(), SearchV2Error> {
            if values.len() >= budget.max_rank_values {
                return Err(error(
                    SearchV2ErrorCode::BudgetExceeded,
                    "rank value count exceeded",
                ));
            }
            let lowered = lower(text, budget.max_rank_field_bytes)?;
            decoded_bytes = decoded_bytes.checked_add(lowered.len()).ok_or_else(|| {
                error(
                    SearchV2ErrorCode::BudgetExceeded,
                    "rank value byte count overflow",
                )
            })?;
            if decoded_bytes > budget.max_rank_field_bytes {
                return Err(error(
                    SearchV2ErrorCode::BudgetExceeded,
                    "rank values exceed byte cap",
                ));
            }
            values.push(lowered);
            Ok(())
        };
        if let Some(text) = value.as_str() {
            add(text)?;
        } else if let Some(entries) = value.as_object() {
            for (_, item) in entries {
                if let Some(text) = item.as_str() {
                    add(text)?;
                }
            }
        }
    }
    Ok(values)
}

fn selected_array(values: &[String], cap: usize) -> Result<String, SearchV2Error> {
    let array = JsonValue::Array(
        values
            .iter()
            .map(|value| JsonValue::String(JsonString::from_utf8(value)))
            .collect(),
    );
    let mut limits = JsonLimits::default();
    limits.max_bytes = cap;
    let bytes = canonical_bytes_v1(&array, CanonicalProfile::SourceRecordDigestV1, limits)
        .map_err(foundation_error)?;
    String::from_utf8(bytes).map_err(|_| {
        error(
            SearchV2ErrorCode::CorruptSelectedCarrier,
            "rank array is not UTF-8",
        )
    })
}

/// Recompute every selected rank/document field before a candidate can affect
/// a result or continuation. Current rights are an independent later gate.
/// `None` is a verified false positive or a selected filter exclusion.
pub fn verify_search_candidate<V: SelectedQueryVocabulary + ?Sized>(
    candidate: SelectedSearchCandidate,
    request: &NormalizedIndexedSearchV2Request,
    vocabulary: &V,
    budget: CandidateVerifyBudget,
) -> Result<Option<VerifiedSearchCandidate>, SearchV2Error> {
    if candidate.position > i64::MAX as u64
        || candidate.id.is_empty()
        || candidate.source_graph.is_empty()
        || candidate.payload.len() > budget.document.max_carrier_bytes
        || candidate.id_lower.len() > budget.max_rank_field_bytes
        || candidate.native_id_lower.len() > budget.max_rank_field_bytes
        || candidate.identity_values.len() > budget.max_rank_field_bytes
        || candidate.visible_values.len() > budget.max_rank_field_bytes
    {
        return Err(error(
            SearchV2ErrorCode::BudgetExceeded,
            "selected search row exceeds admitted fields",
        ));
    }
    if Digest256::of_bytes(&candidate.payload) != candidate.payload_sha256 {
        return Err(error(
            SearchV2ErrorCode::CorruptSelectedCarrier,
            "selected payload digest differs",
        ));
    }
    let mut parse_limits = budget.document.json;
    parse_limits.max_bytes = parse_limits
        .max_bytes
        .min(budget.document.max_carrier_bytes);
    let parsed = parse_json(&candidate.payload, JsonMode::PublishedStrict, parse_limits)
        .map_err(foundation_error)?;
    let item = parsed.root();
    let field = |name| item.object_get(name).and_then(JsonValue::as_str);
    let term = match candidate.kind {
        SearchKind::Nodes => (&candidate.kind_id, "kind_id"),
        SearchKind::Relations => (&candidate.predicate_id, "predicate_id"),
    };
    if field("id") != Some(candidate.id.as_str())
        || field("source_graph") != Some(candidate.source_graph.as_str())
        || field(term.1) != Some(term.0.as_str())
        || !vocabulary.contains_source_id(&candidate.source_graph)
    {
        return Err(error(
            SearchV2ErrorCode::IndexIncomplete,
            "search row identity or vocabulary differs",
        ));
    }
    let native = field("native_id").unwrap_or("");
    let id_lower = lower(&candidate.id, budget.max_rank_field_bytes)?;
    let native_lower = lower(native, budget.max_rank_field_bytes)?;
    if id_lower != candidate.id_lower || native_lower != candidate.native_id_lower {
        return Err(error(
            SearchV2ErrorCode::CorruptSelectedCarrier,
            "selected lower identity differs",
        ));
    }
    let display = item.object_get("display");
    let (primary, visible): (&[&str], &[&str]) = match candidate.kind {
        SearchKind::Nodes => (&["title"], &["title", "kind_label", "summary"]),
        SearchKind::Relations => (
            &["label"],
            &["label", "inverse_label", "statement", "explanation"],
        ),
    };
    let identity_values = rank_values(display, primary, budget)?;
    let visible_values = rank_values(display, visible, budget)?;
    if selected_array(&identity_values, budget.max_rank_field_bytes)? != candidate.identity_values
        || selected_array(&visible_values, budget.max_rank_field_bytes)? != candidate.visible_values
    {
        return Err(error(
            SearchV2ErrorCode::CorruptSelectedCarrier,
            "selected rank fields differ",
        ));
    }
    let document = verify_indexed_search_document(
        &candidate.payload,
        candidate.document_chars,
        candidate.document_digest,
        budget.document,
    )
    .map_err(|reason| {
        error(
            match reason.code {
                crate::QueryErrorCode::BudgetExceeded => SearchV2ErrorCode::BudgetExceeded,
                _ => SearchV2ErrorCode::CorruptSelectedCarrier,
            },
            reason.message,
        )
    })?;
    let needle = request.query();
    if !document.lower.contains(needle) {
        return Ok(None);
    }
    if request
        .sources()
        .is_some_and(|ids| ids.binary_search(&candidate.source_graph).is_err())
        || match candidate.kind {
            SearchKind::Nodes => {
                !request.kind_ids().is_empty()
                    && request
                        .kind_ids()
                        .binary_search(&candidate.kind_id)
                        .is_err()
            }
            SearchKind::Relations => {
                !request.predicate_ids().is_empty()
                    && request
                        .predicate_ids()
                        .binary_search(&candidate.predicate_id)
                        .is_err()
            }
        }
    {
        return Ok(None);
    }
    let rank = if id_lower == needle
        || native_lower == needle
        || identity_values.iter().any(|value| value == needle)
    {
        SearchRank::ExactIdentity
    } else if id_lower.starts_with(needle)
        || native_lower.starts_with(needle)
        || identity_values
            .iter()
            .any(|value| value.starts_with(needle))
    {
        SearchRank::IdentityPrefix
    } else if visible_values.iter().any(|value| value.contains(needle)) {
        SearchRank::VisibleDisplaySubstring
    } else {
        SearchRank::OtherSerializedCarrierSubstring
    };
    let order = SearchOrderKey::new(rank, id_lower, candidate.position)?;
    Ok(Some(VerifiedSearchCandidate {
        order,
        source_graph: candidate.source_graph,
        id: candidate.id,
        payload: candidate.payload,
        payload_sha256: candidate.payload_sha256,
    }))
}
