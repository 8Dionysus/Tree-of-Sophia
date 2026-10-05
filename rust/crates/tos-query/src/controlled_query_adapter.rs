//! Controlled Whole/Site bridge over the maintained indexed-v2 kernel.
//!
//! The compiler owns the model and original budget; this module supplies only
//! typed Search traits and unit-only, synchronous delivery. It never accepts a
//! Verified fallback, path, SQLite connection, state constructor, or lease
//! escape.

use tos_compiler::{
    ControlledKnowledgeModel, ControlledSearchKind, Error as CompilerError, QueryVocabulary,
};
use tos_foundation::{JsonValue, OwnedState};

use crate::{
    knowledge_binding::{BoundCmpKnowledge, bind_controlled_knowledge_from_parts},
    knowledge_packet::{
        IndexedSearchModel, IndexedWireCursorCodec, ScopedIndexedKnowledgeAuthority,
        execute_scoped_indexed_search_page_normalized,
    },
    search_candidate::{
        CandidateReadBudget, CandidateReadCharge, SearchCandidateModel, SelectedSearchCandidate,
    },
    search_execute::SearchKindBudget,
    search_index::{
        GramSeekCharge, GramStat, PostingPage, SearchGramModel, SearchPostingModel,
    },
    search_v2::{
        IndexedSearchV2Request, SearchContinuationState, SearchKind, SearchV2Error,
        SearchV2ErrorCode,
    },
};

const MAX_CONTROLLED_REQUEST_HEAP: usize = 256 * 1024;
const MAX_CONTROLLED_FIELDS_BYTES: usize = 6 * 1024;
const MAX_CONTROLLED_FILTERS: usize = 256;
const MAX_CONTROLLED_FILTER_CODE_POINTS: usize = 256;

fn error(code: SearchV2ErrorCode, message: &'static str) -> SearchV2Error {
    SearchV2Error { code, message }
}

fn compiler_query_error(reason: CompilerError) -> SearchV2Error {
    match reason {
        CompilerError::Budget(_) | CompilerError::SqliteVmBudget { .. } => error(
            SearchV2ErrorCode::BudgetExceeded,
            "controlled query owner budget exceeded",
        ),
        CompilerError::Invalid(_) => error(
            SearchV2ErrorCode::CorruptSelectedCarrier,
            "controlled selected knowledge is invalid",
        ),
        CompilerError::Sql(rusqlite::Error::SqliteFailure(failure, _))
            if failure.code == rusqlite::ErrorCode::OperationInterrupted =>
        {
            error(
                SearchV2ErrorCode::BudgetExceeded,
                "controlled selected query VM budget exceeded",
            )
        }
        _ => error(
            SearchV2ErrorCode::Unavailable,
            "controlled selected query owner refused",
        ),
    }
}

fn controlled_kind(kind: SearchKind) -> ControlledSearchKind {
    match kind {
        SearchKind::Nodes => ControlledSearchKind::Nodes,
        SearchKind::Relations => ControlledSearchKind::Relations,
    }
}

impl SearchGramModel for ControlledKnowledgeModel<'_, '_, '_> {
    fn gram_stat(
        &mut self,
        kind: SearchKind,
        gram: &str,
        max_vm_steps: u64,
        max_rows: u64,
        max_decoded_bytes: u64,
    ) -> Result<GramStat, SearchV2Error> {
        let stat = ControlledKnowledgeModel::gram_stat(
            self,
            controlled_kind(kind),
            gram,
            max_vm_steps,
            max_rows,
            max_decoded_bytes,
        )
        .map_err(compiler_query_error)?;
        Ok(GramStat {
            postings: stat.postings,
            charged: GramSeekCharge {
                lookups: 1,
                vm_steps: stat.vm_steps,
                rows: stat.rows,
                decoded_bytes: stat.decoded_bytes,
            },
        })
    }
}

impl SearchPostingModel for ControlledKnowledgeModel<'_, '_, '_> {
    fn seek_postings(
        &mut self,
        kind: SearchKind,
        gram: &str,
        after: Option<u64>,
        max_rows: usize,
        max_vm_steps: u64,
        max_decoded_bytes: u64,
    ) -> Result<PostingPage, SearchV2Error> {
        let page = ControlledKnowledgeModel::seek_postings(
            self,
            controlled_kind(kind),
            gram,
            after,
            max_rows,
            max_vm_steps,
            max_decoded_bytes,
        )
        .map_err(compiler_query_error)?;
        Ok(PostingPage {
            positions: page.positions,
            exhausted: page.exhausted,
            charged: GramSeekCharge {
                lookups: 1,
                vm_steps: page.vm_steps,
                rows: page.rows,
                decoded_bytes: page.decoded_bytes,
            },
        })
    }
}

impl SearchCandidateModel for ControlledKnowledgeModel<'_, '_, '_> {
    fn exact_candidate(
        &mut self,
        kind: SearchKind,
        position: u64,
        budget: CandidateReadBudget,
    ) -> Result<(SelectedSearchCandidate, CandidateReadCharge), SearchV2Error> {
        let candidate = ControlledKnowledgeModel::exact_candidate(
            self,
            controlled_kind(kind),
            position,
            budget.max_vm_steps,
            budget.max_decoded_bytes,
            budget.max_payload_bytes,
            budget.max_field_bytes,
            budget.max_document_chars,
        )
        .map_err(compiler_query_error)?;
        let charge = CandidateReadCharge {
            vm_steps: candidate.vm_steps,
            rows: candidate.rows,
            decoded_bytes: candidate.decoded_bytes,
        };
        Ok((
            SelectedSearchCandidate {
                kind,
                position: candidate.position,
                id: candidate.id,
                source_graph: candidate.source_graph,
                kind_id: candidate.kind_id,
                predicate_id: candidate.predicate_id,
                id_lower: candidate.id_lower,
                native_id_lower: candidate.native_id_lower,
                identity_values: candidate.identity_values,
                visible_values: candidate.visible_values,
                document_chars: candidate.document_chars,
                document_digest: candidate.document_digest,
                payload_sha256: candidate.payload_sha256,
                payload: candidate.payload,
            },
            charge,
        ))
    }
}

impl IndexedSearchModel for ControlledKnowledgeModel<'_, '_, '_> {
    fn check_bound(&self, bound: &BoundCmpKnowledge<'_>) -> Result<(), SearchV2Error> {
        bound.check_controlled_model(self)
    }

    fn check_open_vm_budget(&self, maximum: u64) -> Result<(), SearchV2Error> {
        ControlledKnowledgeModel::check_query_open_vm_admission(self, maximum)
            .map_err(compiler_query_error)
    }
}

/// Bind once within a compiler-derived reservation. Both the descriptor parse
/// tree and Bound owner stay held while the unit callback constructs same-owner
/// authority and completes Search delivery; neither can escape this function.
pub fn with_controlled_knowledge_binding(
    model: &mut ControlledKnowledgeModel<'_, '_, '_>,
    vocabulary: &QueryVocabulary,
    authored_descriptor: &[u8],
    consume: impl FnOnce(
        &mut ControlledKnowledgeModel<'_, '_, '_>,
        &BoundCmpKnowledge<'_>,
    ) -> tos_compiler::Result<()>,
) -> tos_compiler::Result<()> {
    model.with_owned_binding_workspace(vocabulary, authored_descriptor, |model, descriptor| {
        let bound = bind_controlled_knowledge_from_parts(model, vocabulary, descriptor)
            .map_err(|_| CompilerError::Invalid("controlled semantic binding refused"))?;
        consume(model, &bound)
    })
}

fn request_strings(
    value: Option<&JsonValue>,
    total_bytes: &mut usize,
) -> Result<Vec<String>, SearchV2Error> {
    let Some(value) = value else { return Ok(Vec::new()) };
    if value.is_null() {
        return Ok(Vec::new());
    }
    let Some(values) = value.as_array() else {
        return Err(error(SearchV2ErrorCode::InvalidRequest, "indexed filter must be an array"));
    };
    if values.len() > MAX_CONTROLLED_FILTERS {
        return Err(error(SearchV2ErrorCode::BudgetExceeded, "indexed filter count exceeds cap"));
    }
    let mut owned = Vec::with_capacity(values.len());
    for value in values {
        let Some(text) = value.as_str() else {
            return Err(error(SearchV2ErrorCode::InvalidRequest, "indexed filter entry must be a string"));
        };
        if text.is_empty() || text.chars().count() > MAX_CONTROLLED_FILTER_CODE_POINTS {
            return Err(error(SearchV2ErrorCode::InvalidRequest, "indexed filter value is empty or overlong"));
        }
        *total_bytes = total_bytes.checked_add(text.len()).ok_or_else(|| {
            error(SearchV2ErrorCode::BudgetExceeded, "indexed request field size overflow")
        })?;
        if *total_bytes > MAX_CONTROLLED_FIELDS_BYTES {
            return Err(error(SearchV2ErrorCode::BudgetExceeded, "indexed request fields exceed cap"));
        }
        owned.push(text.to_owned());
    }
    Ok(owned)
}

fn parse_request<'a>(
    value: &'a JsonValue,
    supplied_cursor: Option<&str>,
    max_cursor_bytes: usize,
) -> Result<(IndexedSearchV2Request, Option<&'a str>), SearchV2Error> {
    let Some(fields) = value.as_object() else {
        return Err(error(SearchV2ErrorCode::InvalidRequest, "indexed arguments must be an object"));
    };
    let mut seen = 0u8;
    for (key, _) in fields {
        let (bit, known) = match key.as_str() {
            Some("query") => (1, true),
            Some("sources") => (2, true),
            Some("kind_ids") => (4, true),
            Some("predicate_ids") => (8, true),
            Some("limit") => (16, true),
            Some("cursor") => (32, true),
            _ => (0, false),
        };
        if !known {
            return Err(error(SearchV2ErrorCode::InvalidRequest, "unknown indexed search field"));
        }
        if seen & bit != 0 {
            return Err(error(SearchV2ErrorCode::InvalidRequest, "duplicate indexed search field"));
        }
        seen |= bit;
    }
    let query = match value.object_get("query") {
        None => String::new(),
        Some(raw) => raw.as_str().ok_or_else(|| {
            error(SearchV2ErrorCode::InvalidRequest, "indexed query must be a string")
        })?.to_owned(),
    };
    let mut field_bytes = query.len();
    let sources = request_strings(value.object_get("sources"), &mut field_bytes)?;
    let kind_ids = request_strings(value.object_get("kind_ids"), &mut field_bytes)?;
    let predicate_ids = request_strings(value.object_get("predicate_ids"), &mut field_bytes)?;
    if field_bytes > MAX_CONTROLLED_FIELDS_BYTES {
        return Err(error(SearchV2ErrorCode::BudgetExceeded, "indexed request fields exceed cap"));
    }
    let limit = match value.object_get("limit") {
        None => 40,
        Some(raw) => usize::try_from(raw.as_u64().ok_or_else(|| {
            error(SearchV2ErrorCode::InvalidRequest, "indexed limit must be an unsigned integer")
        })?).map_err(|_| error(SearchV2ErrorCode::InvalidRequest, "indexed limit exceeds range"))?,
    };
    let cursor = match value.object_get("cursor") {
        None | Some(JsonValue::Null) => None,
        Some(raw) => {
            let cursor = raw.as_str().ok_or_else(|| {
                error(SearchV2ErrorCode::InvalidRequest, "indexed cursor must be a string or null")
            })?;
            if cursor.is_empty() || cursor.len() > max_cursor_bytes {
                return Err(error(SearchV2ErrorCode::InvalidRequest, "indexed cursor is empty or overlong"));
            }
            Some(cursor)
        }
    };
    if cursor != supplied_cursor {
        return Err(error(SearchV2ErrorCode::InvalidRequest, "indexed cursor argument differs from original request"));
    }
    Ok((IndexedSearchV2Request { query, sources, kind_ids, predicate_ids, limit }, cursor))
}

fn add(total: &mut usize, value: usize) -> Result<(), SearchV2Error> {
    *total = total.checked_add(value).ok_or_else(|| {
        error(SearchV2ErrorCode::BudgetExceeded, "controlled QRY workspace overflow")
    })?;
    Ok(())
}

fn mul(value: usize, count: usize) -> Result<usize, SearchV2Error> {
    value.checked_mul(count).ok_or_else(|| {
        error(SearchV2ErrorCode::BudgetExceeded, "controlled QRY workspace overflow")
    })
}

fn kind_workspace(budget: SearchKindBudget) -> Result<usize, SearchV2Error> {
    let mut bytes = 0usize;
    // `max_observed_bytes` is logical UTF-8. Reserve a factor of two for
    // retained String capacity plus the exact vector slots for every admitted
    // observed row, including filtered and false-positive candidates.
    add(&mut bytes, mul(usize::try_from(budget.max_observed_bytes).map_err(|_| {
        error(SearchV2ErrorCode::BudgetExceeded, "observed-byte cap exceeds address space")
    })?, 2)?)?;
    add(&mut bytes, mul(
        budget.max_observed_candidates,
        std::mem::size_of::<crate::search_execute::ObservedSearchCandidate>(),
    )?)?;
    // Page-backed postings and their decoded/returned positions.
    add(&mut bytes, usize::try_from(budget.postings.max_decoded_bytes).map_err(|_| {
        error(SearchV2ErrorCode::BudgetExceeded, "posting cap exceeds address space")
    })?)?;
    add(&mut bytes, mul(budget.postings.page_rows, std::mem::size_of::<u64>())?)?;
    // Candidate fields/payload can overlap the verifier's parsed document and
    // the retained top-(limit+1) result set.
    add(&mut bytes, mul(usize::try_from(budget.candidate.max_decoded_bytes).map_err(|_| {
        error(SearchV2ErrorCode::BudgetExceeded, "candidate cap exceeds address space")
    })?, 2)?)?;
    add(&mut bytes, mul(budget.candidate.max_payload_bytes, 2)?)?;
    add(&mut bytes, mul(budget.candidate.max_field_bytes, 8)?)?;
    add(&mut bytes, mul(budget.verify.document.max_document_bytes, 2)?)?;
    add(&mut bytes, mul(budget.verify.document.json.max_bytes, 2)?)?;
    add(&mut bytes, mul(budget.verify.max_rank_values, budget.verify.max_rank_field_bytes)?)?;
    add(&mut bytes, budget.max_selected_result_bytes)?;
    // Heap slots and order keys for at most the protocol's fixed limit+1 hits.
    add(&mut bytes, mul(101, std::mem::size_of::<crate::search_candidate::VerifiedSearchCandidate>())?)?;
    Ok(bytes)
}

fn query_workspace_upper_bound(
    request: &JsonValue,
    bound: &BoundCmpKnowledge<'_>,
    budget: crate::IndexedPageBudget,
) -> Result<usize, SearchV2Error> {
    use tos_foundation::OwnedState;
    let request_bytes = request.owned_heap_bytes().map_err(|_| {
        error(SearchV2ErrorCode::BudgetExceeded, "indexed request geometry unavailable")
    })?;
    if request_bytes > MAX_CONTROLLED_REQUEST_HEAP
        || budget.max_response_bytes == 0
        || budget.max_cursor_bytes == 0
    {
        return Err(error(SearchV2ErrorCode::BudgetExceeded, "controlled QRY admission unavailable"));
    }
    let selection = bound.selection().owned_heap_bytes().map_err(|_| {
        error(SearchV2ErrorCode::BudgetExceeded, "indexed selection geometry unavailable")
    })?;
    let mut bytes = std::mem::size_of::<(
        &JsonValue,
        &BoundCmpKnowledge<'_>,
        crate::IndexedPageBudget,
        IndexedSearchV2Request,
        crate::search_v2::NormalizedIndexedSearchV2Request,
        SearchContinuationState,
        Result<()>,
    )>();
    // Typed request, normalized filters, continuation and codec binding retain
    // separate String/vector owners from the already-held Foundation input.
    add(&mut bytes, mul(request_bytes, 4)?)?;
    add(&mut bytes, mul(selection, 2)?)?;
    add(&mut bytes, mul(budget.max_cursor_bytes, 4)?)?;
    add(&mut bytes, kind_workspace(budget.nodes)?)?;
    add(&mut bytes, kind_workspace(budget.relations)?)?;
    // The response JSON tree, canonical serialization buffer, selected payload
    // trees and final borrowed-delivery body overlap until transport returns.
    add(&mut bytes, mul(budget.max_response_bytes, 3)?)?;
    add(&mut bytes, mul(budget.json.max_bytes.min(budget.max_response_bytes), 2)?)?;
    if bytes == 0 {
        return Err(error(SearchV2ErrorCode::BudgetExceeded, "controlled QRY workspace is empty"));
    }
    Ok(bytes)
}

/// Full controlled indexed-QRY response. The original admitted argument value
/// is parsed only after its internally derived workspace is reserved. Cursor
/// factory construction, request normalization, page/candidate allocations,
/// output encoding and synchronous lease-fenced delivery all remain inside the
/// same unit callback and compiler-owned state hold.
pub fn execute_scoped_controlled_indexed_search_response<'hold, C, A>(
    model: &mut ControlledKnowledgeModel<'_, '_, '_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &'hold mut A,
    request_value: &JsonValue,
    supplied_cursor: Option<&str>,
    budget: crate::IndexedPageBudget,
    cursor_factory: impl FnOnce(SearchContinuationState, &str) -> Result<C, SearchV2Error>,
    deliver: impl FnOnce(&[u8]) -> Result<(), SearchV2Error>,
) -> Result<(), SearchV2Error>
where
    C: IndexedWireCursorCodec,
    A: ScopedIndexedKnowledgeAuthority<'hold> + ?Sized,
{
    let forecast = query_workspace_upper_bound(request_value, bound, budget)?;
    let request_heap = request_value.owned_heap_bytes().map_err(|_| {
        error(SearchV2ErrorCode::BudgetExceeded, "indexed request work geometry unavailable")
    })?;
    model.check_pin().map_err(compiler_query_error)?;
    let mut query_result = None;
    model
        .with_owned_query_workspace(forecast, |model| {
            query_result = Some((|| {
                model.charge_query_work(request_heap).map_err(compiler_query_error)?;
                let (request, cursor_in) = parse_request(
                    request_value,
                    supplied_cursor,
                    budget.max_cursor_bytes,
                )?;
                let normalized = request.clone().normalize(bound.selection(), bound)?;
                let policy = authority.policy_binding();
                let initial = SearchContinuationState::new(
                    bound.selection().clone(),
                    normalized.clone(),
                    policy,
                    bound,
                )?;
                let mut codec = cursor_factory(initial, bound.owner_receipt_id())?;
                let mut packet = execute_scoped_indexed_search_page_normalized(
                    model,
                    bound,
                    authority,
                    &mut codec,
                    request,
                    normalized,
                    cursor_in,
                    budget,
                )?;
                packet.recheck()?;
                let work = usize::try_from(packet.work_bytes()).map_err(|_| {
                    error(
                        SearchV2ErrorCode::BudgetExceeded,
                        "controlled QRY work exceeds address space",
                    )
                })?;
                model.charge_query_work(work).map_err(compiler_query_error)?;
                let (body, mut lease) = packet.into_parts();
                lease.recheck()?;
                deliver(&body)?;
                lease.recheck()?;
                drop(lease);
                drop(body);
                Ok(())
            })());
            // Keep the compiler owner's public reservation callback unit-only;
            // the scoped error is consumed only after the reservation closes.
            Ok(())
        })
        .map_err(compiler_query_error)?;
    query_result.unwrap_or_else(|| {
        Err(error(
            SearchV2ErrorCode::Unavailable,
            "controlled query callback did not run",
        ))
    })
}
