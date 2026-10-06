//! Controlled Whole/Site bridge over the maintained indexed-v2 kernel.
//!
//! The compiler owns the model and original budget; this module supplies only
//! typed Search traits and unit-only, synchronous delivery. It never accepts a
//! Verified fallback, path, SQLite connection, state constructor, or lease
//! escape.

use tos_compiler::{
    ControlledGramStat, ControlledKnowledgeModel, ControlledPostingPage, ControlledSearchCandidate,
    ControlledSearchKind, ControlledSidecarModel, Error as CompilerError,
    KnowledgeSelectedExpectation, KnowledgeSourceBasis, QueryVocabulary,
};
use tos_foundation::{JsonNumberKind, JsonValue, OwnedState, python_decimal_value_unicode16_v1};

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

const INDEXED_LIMIT_DEFAULT: usize = 40;
const INDEXED_LIMIT_MIN: usize = 1;
const INDEXED_LIMIT_MAX: usize = 100;

/// Match Reference's bounded indexed-search limit conversion without
/// delegating request parsing to Python. Invalid/missing values use the public
/// default; accepted values are truncated as Python int() does, then clamped.
pub fn indexed_limit_from_foundation(value: Option<&JsonValue>) -> usize {
    let parsed = match value {
        None | Some(JsonValue::Null) => return INDEXED_LIMIT_DEFAULT,
        Some(JsonValue::Bool(value)) => Some(if *value { 1 } else { 0 }),
        Some(JsonValue::Number(number)) => match number.kind {
            JsonNumberKind::Int => decimal_limit(&number.lexeme),
            JsonNumberKind::Float => number
                .as_python_float()
                .filter(|value| value.is_finite())
                .map(|value| {
                    if value <= INDEXED_LIMIT_MIN as f64 {
                        INDEXED_LIMIT_MIN
                    } else if value >= INDEXED_LIMIT_MAX as f64 {
                        INDEXED_LIMIT_MAX
                    } else {
                        value.trunc() as usize
                    }
                }),
        },
        Some(JsonValue::String(value)) => value.as_str().and_then(decimal_limit),
        Some(JsonValue::Array(_) | JsonValue::Object(_)) => None,
    };
    parsed
        .unwrap_or(INDEXED_LIMIT_DEFAULT)
        .clamp(INDEXED_LIMIT_MIN, INDEXED_LIMIT_MAX)
}

fn decimal_limit(raw: &str) -> Option<usize> {
    fn python_space(ch: char) -> bool {
        matches!(
            ch,
            '\u{0009}'..='\u{000d}'
                | '\u{001c}'..='\u{0020}'
                | '\u{0085}'
                | '\u{00a0}'
                | '\u{1680}'
                | '\u{2000}'..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
        )
    }

    let text = raw.trim_matches(python_space);
    let (negative, digits) = match text.strip_prefix('-') {
        Some(digits) => (true, digits),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let mut magnitude = 0usize;
    let mut saw_digit = false;
    let mut previous_digit = false;
    let mut chars = digits.chars().peekable();
    while let Some(ch) = chars.next() {
        if let Some(digit) = python_decimal_value_unicode16_v1(ch) {
            magnitude = magnitude
                .saturating_mul(10)
                .saturating_add(usize::from(digit))
                .min(INDEXED_LIMIT_MAX + 1);
            saw_digit = true;
            previous_digit = true;
        } else if ch == '_'
            && previous_digit
            && chars
                .peek()
                .copied()
                .and_then(python_decimal_value_unicode16_v1)
                .is_some()
        {
            previous_digit = false;
        } else {
            return None;
        }
    }
    if !saw_digit || !previous_digit {
        return None;
    }
    Some(if negative {
        INDEXED_LIMIT_MIN
    } else {
        magnitude.clamp(INDEXED_LIMIT_MIN, INDEXED_LIMIT_MAX)
    })
}

pub(crate) fn compiler_query_error(reason: CompilerError) -> SearchV2Error {
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

/// Query-owned narrow reader seam for the two compiler-issued controlled
/// carriers. The compiler remains independent of QRY and never receives a
/// query trait or ranking policy from this crate.
trait ControlledSearchSource {
    fn source_check_pin(&self) -> tos_compiler::Result<()>;
    fn source_charge_query_work(&self, bytes: usize) -> tos_compiler::Result<()>;
    fn source_check_open_vm_admission(&self, maximum: u64) -> tos_compiler::Result<()>;
    fn selected_expectation(&self) -> &KnowledgeSelectedExpectation;
    fn source_basis(&self) -> &KnowledgeSourceBasis;
    fn search_index_profile(&self) -> &str;
    fn source_gram_stat(
        &mut self,
        kind: ControlledSearchKind,
        gram: &str,
        vm: u64,
        rows: u64,
        decoded: u64,
    ) -> tos_compiler::Result<ControlledGramStat>;
    fn source_seek_postings(
        &mut self,
        kind: ControlledSearchKind,
        gram: &str,
        after: Option<u64>,
        rows: usize,
        vm: u64,
        decoded: u64,
    ) -> tos_compiler::Result<ControlledPostingPage>;
    fn source_exact_candidate(
        &mut self,
        kind: ControlledSearchKind,
        position: u64,
        budget: CandidateReadBudget,
    ) -> tos_compiler::Result<ControlledSearchCandidate>;
    fn with_query_workspace(
        &mut self,
        bytes: usize,
        operation: impl FnOnce(&mut Self) -> tos_compiler::Result<()>,
    ) -> tos_compiler::Result<()>;
    fn with_binding_workspace(
        &mut self,
        vocabulary: &QueryVocabulary,
        descriptor: &[u8],
        operation: impl for<'value> FnOnce(
            &mut Self,
            &'value JsonValue,
        ) -> tos_compiler::Result<()>,
    ) -> tos_compiler::Result<()>;
}

macro_rules! impl_controlled_search_source {
    ($model:ident<$($lifetime:lifetime),+>) => {
        impl<$($lifetime),+> ControlledSearchSource for $model<$($lifetime),+> {
            fn source_check_pin(&self) -> tos_compiler::Result<()> {
                self.check_pin()
            }
            fn source_charge_query_work(&self, bytes: usize) -> tos_compiler::Result<()> {
                self.charge_query_work(bytes)
            }
            fn source_check_open_vm_admission(&self, maximum: u64) -> tos_compiler::Result<()> {
                self.check_query_open_vm_admission(maximum)
            }
            fn selected_expectation(&self) -> &KnowledgeSelectedExpectation {
                self.selection()
            }
            fn source_basis(&self) -> &KnowledgeSourceBasis {
                self.source_basis()
            }
            fn search_index_profile(&self) -> &str {
                self.search_index_profile()
            }
            fn source_gram_stat(
                &mut self,
                kind: ControlledSearchKind,
                gram: &str,
                vm: u64,
                rows: u64,
                decoded: u64,
            ) -> tos_compiler::Result<ControlledGramStat> {
                self.gram_stat(kind, gram, vm, rows, decoded)
            }
            fn source_seek_postings(
                &mut self,
                kind: ControlledSearchKind,
                gram: &str,
                after: Option<u64>,
                rows: usize,
                vm: u64,
                decoded: u64,
            ) -> tos_compiler::Result<ControlledPostingPage> {
                self.seek_postings(kind, gram, after, rows, vm, decoded)
            }
            fn source_exact_candidate(
                &mut self,
                kind: ControlledSearchKind,
                position: u64,
                budget: CandidateReadBudget,
            ) -> tos_compiler::Result<ControlledSearchCandidate> {
                self.exact_candidate(
                    kind,
                    position,
                    budget.max_vm_steps,
                    budget.max_decoded_bytes,
                    budget.max_payload_bytes,
                    budget.max_field_bytes,
                    budget.max_document_chars,
                )
            }
            fn with_query_workspace(
                &mut self,
                bytes: usize,
                operation: impl FnOnce(&mut Self) -> tos_compiler::Result<()>,
            ) -> tos_compiler::Result<()> {
                self.with_owned_query_workspace(bytes, operation)
            }
            fn with_binding_workspace(
                &mut self,
                vocabulary: &QueryVocabulary,
                descriptor: &[u8],
                operation: impl for<'value> FnOnce(
                    &mut Self,
                    &'value JsonValue,
                ) -> tos_compiler::Result<()>,
            ) -> tos_compiler::Result<()> {
                self.with_owned_binding_workspace(vocabulary, descriptor, operation)
            }
        }
    };
}

impl_controlled_search_source!(ControlledKnowledgeModel<'model, 'state, 'budget>);
impl_controlled_search_source!(ControlledSidecarModel<'borrow, 'model, 'state, 'budget>);

/// Thin QRY-owned adapter lets the maintained kernel use either authentic
/// compiler reader without duplicating posting admission, candidate ranking,
/// page assembly, cursor, response, or disclosure semantics.
struct ControlledSearchAdapter<'source, M> {
    source: &'source mut M,
}

impl<M: ControlledSearchSource> SearchGramModel for ControlledSearchAdapter<'_, M> {
    fn gram_stat(
        &mut self,
        kind: SearchKind,
        gram: &str,
        max_vm_steps: u64,
        max_rows: u64,
        max_decoded_bytes: u64,
    ) -> Result<GramStat, SearchV2Error> {
        let stat = self
            .source
            .source_gram_stat(
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

impl<M: ControlledSearchSource> SearchPostingModel for ControlledSearchAdapter<'_, M> {
    fn seek_postings(
        &mut self,
        kind: SearchKind,
        gram: &str,
        after: Option<u64>,
        max_rows: usize,
        max_vm_steps: u64,
        max_decoded_bytes: u64,
    ) -> Result<PostingPage, SearchV2Error> {
        let page = self
            .source
            .source_seek_postings(
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

impl<M: ControlledSearchSource> SearchCandidateModel for ControlledSearchAdapter<'_, M> {
    fn exact_candidate(
        &mut self,
        kind: SearchKind,
        position: u64,
        budget: CandidateReadBudget,
    ) -> Result<(SelectedSearchCandidate, CandidateReadCharge), SearchV2Error> {
        let candidate = self
            .source
            .source_exact_candidate(controlled_kind(kind), position, budget)
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

impl<M: ControlledSearchSource> IndexedSearchModel for ControlledSearchAdapter<'_, M> {
    fn check_bound(&self, bound: &BoundCmpKnowledge<'_>) -> Result<(), SearchV2Error> {
        self.source
            .source_check_pin()
            .map_err(|_| error(SearchV2ErrorCode::StaleSelection, "selected controlled knowledge pin changed"))?;
        bound.check_controlled_source_parts(
            self.source.selected_expectation(),
            self.source.source_basis(),
            self.source.search_index_profile(),
        )
    }

    fn check_open_vm_budget(&self, maximum: u64) -> Result<(), SearchV2Error> {
        self.source
            .source_check_open_vm_admission(maximum)
            .map_err(compiler_query_error)
    }
}

fn with_controlled_binding<M: ControlledSearchSource>(
    model: &mut M,
    vocabulary: &QueryVocabulary,
    authored_descriptor: &[u8],
    consume: impl FnOnce(&mut M, &BoundCmpKnowledge<'_>) -> tos_compiler::Result<()>,
) -> tos_compiler::Result<()> {
    let mut callback_result = None;
    let workspace_result = model.with_binding_workspace(
        vocabulary,
        authored_descriptor,
        |model, descriptor| {
            callback_result = Some((|| {
                model.source_check_pin()?;
                let bound = bind_controlled_knowledge_from_parts(
                    model.selected_expectation(),
                    model.source_basis(),
                    model.search_index_profile(),
                    vocabulary,
                    descriptor,
                )
                .map_err(|_| CompilerError::Invalid("controlled semantic binding refused"))?;
                let result = consume(model, &bound);
                let pin_result = model.source_check_pin();
                result?;
                pin_result?;
                Ok(())
            })());
            Ok(())
        },
    );
    let pin_result = model.source_check_pin();
    workspace_result?;
    pin_result?;
    callback_result.unwrap_or(Err(CompilerError::Invalid(
        "controlled binding callback did not run",
    )))
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
    with_controlled_binding(model, vocabulary, authored_descriptor, consume)
}

/// Sidecar-backed counterpart to the source-only binding seam. It retains the
/// same compiler-owned descriptor/state hold and cache/source fences.
pub fn with_controlled_sidecar_knowledge_binding(
    model: &mut ControlledSidecarModel<'_, '_, '_, '_>,
    vocabulary: &QueryVocabulary,
    authored_descriptor: &[u8],
    consume: impl FnOnce(
        &mut ControlledSidecarModel<'_, '_, '_, '_>,
        &BoundCmpKnowledge<'_>,
    ) -> tos_compiler::Result<()>,
) -> tos_compiler::Result<()> {
    with_controlled_binding(model, vocabulary, authored_descriptor, consume)
}

/// Header-only selected disclosure with the maintained Inspect scope and lease.
/// Body and lease stay inside synchronous delivery under the compiler state hold.
pub fn execute_controlled_knowledge_header_response<'hold, A>(
    model: &mut ControlledKnowledgeModel<'_, '_, '_>,
    bound: &BoundCmpKnowledge<'_>, authority: &mut A,
    budget: crate::InspectBudget,
    deliver: impl FnOnce(&[u8]) -> Result<(), SearchV2Error>,
) -> Result<(), SearchV2Error>
where A: crate::InspectCurrentAuthority<'hold> + ?Sized {
    if budget.max_rows == 0 || budget.max_response_bytes == 0 {
        return Err(error(SearchV2ErrorCode::BudgetExceeded, "controlled header budget"));
    }
    model.check_query_open_vm_admission(budget.max_open_vm_steps).map_err(compiler_query_error)?;
    bound.check_controlled_model(model)?;
    let forecast = authority.disclosure_metadata_state_upper_bound()?;
    let mut outcome = Ok(());
    let frame = std::mem::size_of_val(&deliver)
        .checked_add(std::mem::size_of::<crate::search_v2::CurrentPolicyBinding>())
        .and_then(|n| n.checked_add(std::mem::size_of::<crate::IndexedDisclosureScope>()))
        .and_then(|n| n.checked_add(std::mem::size_of::<Option<std::sync::Arc<dyn crate::AbortProbe>>>()
            + std::mem::size_of::<Result<(), SearchV2Error>>() * 3))
        .ok_or_else(|| error(SearchV2ErrorCode::BudgetExceeded, "controlled header frame state"))?;
    model.with_owned_query_workspace(forecast.checked_add(frame)
        .ok_or(CompilerError::Budget("controlled header policy workspace")).map_err(compiler_query_error)?, |model| {
        let policy = authority.policy_binding();
        let scope = authority.disclosure_scope();
        let actual = policy.retained_state_bytes().map_err(|_| CompilerError::Budget("controlled header policy state"))?
            .checked_add(crate::knowledge_inspect::scope_owned_state(&scope)
                .map_err(|_| CompilerError::Budget("controlled header scope state"))?)
            .ok_or(CompilerError::Budget("controlled header metadata state"))?;
        if actual > forecast { return Err(CompilerError::Budget("controlled header metadata forecast")); }
        outcome = (|| {
            scope.validate_for(bound, &policy, "tos_knowledge_header", "read_only_public_knowledge_header_v1")?;
            authority.check_selected()?;
            if let Some(proof) = bound.source_basis().managed_source() { authority.authorize_managed_source_current(proof)?; }
            if let Some(proof) = bound.source_basis().managed_source_v2() { authority.authorize_managed_source_v2_current(proof)?; }
            let mut delivered = Ok(());
            model.with_controlled_header(budget.max_payload_bytes.min(budget.max_response_bytes),
                budget.max_decoded_bytes, budget.max_read_vm_steps, budget.json, |body| {
                    delivered = (|| {
                        if let Some(reason) = authority.abort_probe().and_then(|probe| probe.reason()) {
                            return Err(error(match reason { crate::AbortReason::Cancelled => SearchV2ErrorCode::Cancelled,
                                crate::AbortReason::DeadlineExceeded => SearchV2ErrorCode::DeadlineExceeded }, "controlled header aborted"));
                        }
                        authority.check_selected()?;
                        let mut lease = authority.acquire_disclosure(&scope, &[])?;
                        lease.recheck()?;
                        deliver(body)?;
                        lease.recheck()
                    })();
                    Ok(())
                }).map_err(compiler_query_error)?;
            delivered
        })();
        Ok(())
    }).map_err(compiler_query_error)?;
    outcome
}

/// Exact catalog delivery under the original controlled model and an actual
/// Catalog lease. The same authority also supplies its existing owned metadata
/// forecast; no policy/scope clone is allocated before that admission.
pub fn execute_controlled_catalog_response<'hold, A>(
    model: &mut ControlledKnowledgeModel<'_, '_, '_>,
    bound: &BoundCmpKnowledge<'_>, authority: &mut A,
    budget: crate::CatalogBudget,
    deliver: impl FnOnce(&[u8]) -> Result<(), crate::CatalogError>,
) -> Result<(), crate::CatalogError>
where A: crate::CatalogCurrentAuthority<'hold> + crate::InspectCurrentAuthority<'hold> {
    execute_controlled_catalog_payload_response(model, bound, authority, budget,
        |raw, _| deliver(raw))
}

pub(crate) fn execute_controlled_catalog_payload_response<'hold, A>(
    model: &mut ControlledKnowledgeModel<'_, '_, '_>,
    bound: &BoundCmpKnowledge<'_>, authority: &mut A,
    budget: crate::CatalogBudget,
    deliver: impl FnOnce(&[u8], &JsonValue) -> Result<(), crate::CatalogError>,
) -> Result<(), crate::CatalogError>
where A: crate::CatalogCurrentAuthority<'hold> + crate::InspectCurrentAuthority<'hold> {
    use crate::{CatalogCurrentAuthority as C, InspectCurrentAuthority as I};
    use crate::knowledge_catalog::{CatalogError, CatalogErrorCode};
    let refused = || CatalogError { code: CatalogErrorCode::BudgetExceeded,
        message: "controlled catalog owner refused" };
    let compiler_error = |e| CatalogError { code: match e {
        CompilerError::Budget(_) | CompilerError::SqliteVmBudget { .. } => CatalogErrorCode::BudgetExceeded,
        CompilerError::Invalid(_) => CatalogErrorCode::CorruptSelectedCarrier,
        _ => CatalogErrorCode::PolicyBindingUnavailable,
    }, message: "controlled catalog owner refused" };
    model.check_query_open_vm_admission(budget.max_open_vm_steps).map_err(compiler_error)?;
    bound.check_controlled_model(model).map_err(|_| CatalogError {
        code: CatalogErrorCode::StaleSelection, message: "controlled catalog binding differs" })?;
    let forecast = I::disclosure_metadata_state_upper_bound(authority)
        .map_err(|_| refused())?
        .checked_add(std::mem::size_of::<crate::CatalogDisclosureScope>())
        .and_then(|n| n.checked_add(std::mem::size_of_val(&deliver)))
        .ok_or_else(refused)?;
    let mut outcome = Ok(());
    model.with_owned_query_workspace(forecast, |model| {
        let policy = C::policy_binding(authority);
        let scope = C::disclosure_scope(authority);
        outcome = (|| {
            scope.validate(bound, &policy)?;
            C::check_selected(authority)?;
            if let Some(proof) = bound.source_basis().managed_source() { C::authorize_managed_source_current(authority, proof)?; }
            if let Some(proof) = bound.source_basis().managed_source_v2() { C::authorize_managed_source_v2_current(authority, proof)?; }
            let mut delivered = Ok(());
            model.with_controlled_catalog(budget.max_packet_bytes, budget.max_decoded_bytes,
                budget.max_read_vm_steps, budget.json, |body, catalog| {
                    delivered = (|| {
                        bound.validate_catalog_identity(catalog, budget.json).map_err(|_| CatalogError {
                            code: CatalogErrorCode::CorruptSelectedCarrier, message: "controlled catalog identity differs" })?;
                        C::authorize_current(authority, bound.selection().catalog_packet_sha256)?;
                        C::check_selected(authority)?;
                        if let Some(reason) = C::abort_probe(authority).and_then(|probe| probe.reason()) {
                            return Err(CatalogError { code: match reason {
                                crate::AbortReason::Cancelled => CatalogErrorCode::Cancelled,
                                crate::AbortReason::DeadlineExceeded => CatalogErrorCode::DeadlineExceeded,
                            }, message: "controlled catalog aborted" });
                        }
                        let mut lease = C::acquire_disclosure(authority, &scope, bound.selection().catalog_packet_sha256)?;
                        lease.recheck()?;
                        deliver(body, catalog)?;
                        lease.recheck()
                    })();
                    Ok(())
                }).map_err(compiler_error)?;
            delivered
        })();
        Ok(())
    }).map_err(compiler_error)?;
    outcome
}

fn request_strings(
    value: Option<&JsonValue>,
    total_bytes: &mut usize,
    allow_empty: bool,
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
        if (!allow_empty && text.is_empty())
            || text.chars().count() > MAX_CONTROLLED_FILTER_CODE_POINTS
        {
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
    let sources = request_strings(value.object_get("sources"), &mut field_bytes, false)?;
    let kind_ids = request_strings(value.object_get("kind_ids"), &mut field_bytes, true)?;
    let predicate_ids = request_strings(value.object_get("predicate_ids"), &mut field_bytes, true)?;
    if field_bytes > MAX_CONTROLLED_FIELDS_BYTES {
        return Err(error(SearchV2ErrorCode::BudgetExceeded, "indexed request fields exceed cap"));
    }
    let limit = indexed_limit_from_foundation(value.object_get("limit"));
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
        Option<Result<(), SearchV2Error>>,
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
    execute_controlled_indexed_search_response(
        model,
        bound,
        authority,
        request_value,
        supplied_cursor,
        budget,
        cursor_factory,
        deliver,
    )
}

/// Execute the same maintained indexed-v2 kernel over the borrowed cache
/// reader. Ranking, candidate verification, response and disclosure semantics
/// remain shared with the source-only controlled model path.
pub fn execute_scoped_controlled_sidecar_indexed_search_response<'hold, C, A>(
    model: &mut ControlledSidecarModel<'_, '_, '_, '_>,
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
    execute_controlled_indexed_search_response(
        model,
        bound,
        authority,
        request_value,
        supplied_cursor,
        budget,
        cursor_factory,
        deliver,
    )
}

fn execute_controlled_indexed_search_response<'hold, M, C, A>(
    model: &mut M,
    bound: &BoundCmpKnowledge<'_>,
    authority: &'hold mut A,
    request_value: &JsonValue,
    supplied_cursor: Option<&str>,
    budget: crate::IndexedPageBudget,
    cursor_factory: impl FnOnce(SearchContinuationState, &str) -> Result<C, SearchV2Error>,
    deliver: impl FnOnce(&[u8]) -> Result<(), SearchV2Error>,
) -> Result<(), SearchV2Error>
where
    M: ControlledSearchSource,
    C: IndexedWireCursorCodec,
    A: ScopedIndexedKnowledgeAuthority<'hold> + ?Sized,
{
    let forecast = query_workspace_upper_bound(request_value, bound, budget)?;
    let request_heap = request_value.owned_heap_bytes().map_err(|_| {
        error(SearchV2ErrorCode::BudgetExceeded, "indexed request work geometry unavailable")
    })?;
    model.source_check_pin().map_err(compiler_query_error)?;
    let mut query_result = None;
    model
        .with_query_workspace(forecast, |model| {
            query_result = Some((|| {
                model.source_charge_query_work(request_heap).map_err(compiler_query_error)?;
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
                let mut adapter = ControlledSearchAdapter { source: model };
                let mut packet = execute_scoped_indexed_search_page_normalized(
                    &mut adapter,
                    bound,
                    authority,
                    &mut codec,
                    request,
                    normalized,
                    cursor_in,
                    budget,
                )?;
                drop(adapter);
                packet.recheck()?;
                let work = usize::try_from(packet.work_bytes()).map_err(|_| {
                    error(
                        SearchV2ErrorCode::BudgetExceeded,
                        "controlled QRY work exceeds address space",
                    )
                })?;
                model.source_charge_query_work(work).map_err(compiler_query_error)?;
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
