//! Private indexed-v2 per-kind execution. Selected data is not a public packet
//! until an owner-issued current-policy disclosure hold is acquired.

use crate::search_candidate::{
    CandidateReadBudget, CandidateReadCharge, CandidateVerifyBudget, SearchCandidateModel,
    SelectedSearchCandidate, VerifiedSearchCandidate, verify_search_candidate_charged,
};
use crate::search_index::{
    GramSeed, GramSeekBudget, GramSeekCharge, PostingSeekBudget, SearchGramModel,
    SearchPostingModel, choose_rarest_gram, visit_complete_postings,
};
use crate::search_v2::{
    NormalizedIndexedSearchV2Request, SearchContinuationState, SearchKind, SearchOrderKey,
    SearchV2Error, SearchV2ErrorCode, SelectedQueryVocabulary,
};
use tos_foundation::Digest256;

const OBSERVED_FIXED_BYTES_V1: u64 = 1 + 8 + 32;

#[derive(Clone, Copy, Debug)]
pub struct SearchKindBudget {
    pub grams: GramSeekBudget,
    pub postings: PostingSeekBudget,
    pub candidate: CandidateReadBudget,
    pub verify: CandidateVerifyBudget,
    pub max_candidate_vm_steps: u64,
    pub max_candidate_decoded_bytes: u64,
    pub max_verified_chars: u64,
    pub max_verified_bytes: u64,
    pub max_observed_candidates: usize,
    /// Logical UTF-8 ID/source bytes plus 1 kind, 8 position and 32 digest
    /// bytes per consulted carrier. This is not an allocator-heap meter.
    pub max_observed_bytes: u64,
    pub max_selected_result_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedSearchCandidate {
    pub kind: SearchKind,
    pub position: u64,
    pub id: String,
    pub source_graph: String,
    pub payload_sha256: Digest256,
}

pub(crate) trait SearchCurrentAuthority {
    fn check_selected(&mut self) -> Result<(), SearchV2Error>;
    fn authorize_current(
        &mut self,
        candidate: &SelectedSearchCandidate,
    ) -> Result<(), SearchV2Error>;
}

#[derive(Clone, Debug)]
pub(crate) struct PrivateSearchKindPage {
    pub(crate) hits: Vec<VerifiedSearchCandidate>,
    pub(crate) observed: Vec<ObservedSearchCandidate>,
    pub(crate) has_more: bool,
    pub(crate) matching_total: u64,
    pub(crate) gram_seed: Option<GramSeed>,
    pub(crate) gram_charge: GramSeekCharge,
    pub(crate) posting_charge: GramSeekCharge,
    pub(crate) candidate_charge: CandidateReadCharge,
    pub(crate) verified_chars: u64,
    pub(crate) verified_bytes: u64,
    pub(crate) observed_bytes: u64,
}

fn error(code: SearchV2ErrorCode, message: &'static str) -> SearchV2Error {
    SearchV2Error { code, message }
}

/// Scan at most the admitted selected posting count and retain only the best
/// `limit+1` verified hits in global `(rank, lower_id, source_position)` order.
/// This avoids a request-sized sort or graph load. Every consulted carrier,
/// including filtered rows and gram false positives, crosses current policy.
/// The returned page remains private and cannot be serialized for transport.
pub(crate) fn execute_private_kind_page<M, V, A>(
    model: &mut M,
    vocabulary: &V,
    authority: &mut A,
    kind: SearchKind,
    request: &NormalizedIndexedSearchV2Request,
    after: Option<&SearchOrderKey>,
    budget: SearchKindBudget,
) -> Result<PrivateSearchKindPage, SearchV2Error>
where
    M: SearchGramModel + SearchPostingModel + SearchCandidateModel,
    V: SelectedQueryVocabulary + ?Sized,
    A: SearchCurrentAuthority + ?Sized,
{
    authority.check_selected()?;
    if budget.max_observed_candidates == 0
        || (budget.max_observed_candidates as u64) < budget.grams.max_candidates
        || budget.max_selected_result_bytes == 0
        || budget.max_candidate_vm_steps == 0
        || budget.max_candidate_decoded_bytes == 0
        || budget.max_verified_chars == 0
        || budget.max_verified_bytes == 0
        || budget.max_observed_bytes == 0
    {
        return Err(error(
            SearchV2ErrorCode::BudgetExceeded,
            "indexed kind admission unavailable",
        ));
    }
    let seed = choose_rarest_gram(model, kind, request, budget.grams)?;
    if seed.gram.is_none() {
        authority.check_selected()?;
        return Ok(PrivateSearchKindPage {
            hits: Vec::new(),
            observed: Vec::new(),
            has_more: false,
            matching_total: 0,
            gram_seed: None,
            gram_charge: seed.charged,
            posting_charge: GramSeekCharge::default(),
            candidate_charge: CandidateReadCharge {
                vm_steps: 0,
                rows: 0,
                decoded_bytes: 0,
            },
            verified_chars: 0,
            verified_bytes: 0,
            observed_bytes: 0,
        });
    }
    let mut observed = Vec::new();
    let mut top = Vec::<VerifiedSearchCandidate>::new();
    let mut matching_total = 0u64;
    let mut candidate_charge = CandidateReadCharge {
        vm_steps: 0,
        rows: 0,
        decoded_bytes: 0,
    };
    let mut verified_chars = 0u64;
    let mut verified_bytes = 0u64;
    let mut observed_bytes = 0u64;
    let posting_charge =
        visit_complete_postings(model, kind, &seed, budget.postings, |model, position| {
            if observed.len() >= budget.max_observed_candidates {
                return Err(error(
                    SearchV2ErrorCode::BudgetExceeded,
                    "observed candidate cap exceeded",
                ));
            }
            let remaining_vm = budget
                .max_candidate_vm_steps
                .saturating_sub(candidate_charge.vm_steps);
            let remaining_bytes = budget
                .max_candidate_decoded_bytes
                .saturating_sub(candidate_charge.decoded_bytes);
            if remaining_vm == 0 || remaining_bytes == 0 {
                return Err(error(
                    SearchV2ErrorCode::BudgetExceeded,
                    "candidate work cap exhausted",
                ));
            }
            let mut read_budget = budget.candidate;
            read_budget.max_vm_steps = read_budget.max_vm_steps.min(remaining_vm);
            read_budget.max_decoded_bytes = read_budget.max_decoded_bytes.min(remaining_bytes);
            let (candidate, charged) = model.exact_candidate(kind, position, read_budget)?;
            if candidate.position != position
                || candidate.kind != kind
                || charged.rows != 1
                || charged.vm_steps > read_budget.max_vm_steps
                || charged.decoded_bytes > read_budget.max_decoded_bytes
            {
                return Err(error(
                    SearchV2ErrorCode::IndexIncomplete,
                    "selected candidate row or charge differs",
                ));
            }
            candidate_charge.vm_steps = candidate_charge
                .vm_steps
                .checked_add(charged.vm_steps)
                .ok_or_else(|| {
                    error(
                        SearchV2ErrorCode::BudgetExceeded,
                        "candidate VM charge overflow",
                    )
                })?;
            candidate_charge.rows =
                candidate_charge
                    .rows
                    .checked_add(charged.rows)
                    .ok_or_else(|| {
                        error(
                            SearchV2ErrorCode::BudgetExceeded,
                            "candidate row charge overflow",
                        )
                    })?;
            candidate_charge.decoded_bytes = candidate_charge
                .decoded_bytes
                .checked_add(charged.decoded_bytes)
                .ok_or_else(|| {
                    error(
                        SearchV2ErrorCode::BudgetExceeded,
                        "candidate byte charge overflow",
                    )
                })?;
            authority.authorize_current(&candidate)?;
            let observed_row_bytes = OBSERVED_FIXED_BYTES_V1
                .checked_add(candidate.id.len() as u64)
                .and_then(|sum| sum.checked_add(candidate.source_graph.len() as u64))
                .ok_or_else(|| {
                    error(
                        SearchV2ErrorCode::BudgetExceeded,
                        "observed candidate byte overflow",
                    )
                })?;
            observed_bytes = observed_bytes
                .checked_add(observed_row_bytes)
                .ok_or_else(|| {
                    error(
                        SearchV2ErrorCode::BudgetExceeded,
                        "observed candidate byte overflow",
                    )
                })?;
            if observed_bytes > budget.max_observed_bytes {
                return Err(error(
                    SearchV2ErrorCode::BudgetExceeded,
                    "observed candidate byte cap exceeded",
                ));
            }
            observed.push(ObservedSearchCandidate {
                kind,
                position,
                id: candidate.id.clone(),
                source_graph: candidate.source_graph.clone(),
                payload_sha256: candidate.payload_sha256,
            });
            let remaining_chars = budget.max_verified_chars.saturating_sub(verified_chars);
            let remaining_bytes = budget.max_verified_bytes.saturating_sub(verified_bytes);
            if candidate.document_chars > remaining_chars
                || remaining_chars == 0
                || remaining_bytes == 0
            {
                return Err(error(
                    SearchV2ErrorCode::BudgetExceeded,
                    "search verification work cap exhausted",
                ));
            }
            let mut verify_budget = budget.verify;
            verify_budget.document.max_document_code_points = verify_budget
                .document
                .max_document_code_points
                .min(remaining_chars.min(usize::MAX as u64) as usize);
            verify_budget.document.max_document_bytes = verify_budget
                .document
                .max_document_bytes
                .min(remaining_bytes.min(usize::MAX as u64) as usize);
            let verified =
                verify_search_candidate_charged(candidate, request, vocabulary, verify_budget)?;
            verified_chars = verified_chars
                .checked_add(verified.document_code_points)
                .ok_or_else(|| {
                    error(
                        SearchV2ErrorCode::BudgetExceeded,
                        "search verification char overflow",
                    )
                })?;
            verified_bytes = verified_bytes
                .checked_add(verified.document_bytes)
                .ok_or_else(|| {
                    error(
                        SearchV2ErrorCode::BudgetExceeded,
                        "search verification byte overflow",
                    )
                })?;
            if verified_chars > budget.max_verified_chars
                || verified_bytes > budget.max_verified_bytes
            {
                return Err(error(
                    SearchV2ErrorCode::BudgetExceeded,
                    "search verification work cap exceeded",
                ));
            }
            let Some(hit) = verified.hit else {
                return Ok(());
            };
            matching_total = matching_total.checked_add(1).ok_or_else(|| {
                error(
                    SearchV2ErrorCode::BudgetExceeded,
                    "search matching count overflow",
                )
            })?;
            if after.is_some_and(|key| hit.order <= *key) {
                return Ok(());
            }
            let insertion = top
                .binary_search_by(|existing| existing.order.cmp(&hit.order))
                .unwrap_or_else(|index| index);
            if insertion <= request.limit() {
                top.insert(insertion, hit);
                if top.len() > request.limit() + 1 {
                    top.pop();
                }
                let selected_bytes = top
                    .iter()
                    .try_fold(0usize, |sum, hit| sum.checked_add(hit.payload.len()))
                    .ok_or_else(|| {
                        error(
                            SearchV2ErrorCode::BudgetExceeded,
                            "selected result byte overflow",
                        )
                    })?;
                if selected_bytes > budget.max_selected_result_bytes {
                    return Err(error(
                        SearchV2ErrorCode::BudgetExceeded,
                        "selected result byte cap exceeded",
                    ));
                }
            }
            Ok(())
        })?;
    authority.check_selected()?;
    let has_more = top.len() > request.limit();
    if has_more {
        top.pop();
    }
    Ok(PrivateSearchKindPage {
        hits: top,
        observed,
        has_more,
        matching_total,
        gram_charge: seed.charged,
        gram_seed: Some(seed),
        posting_charge,
        candidate_charge,
        verified_chars,
        verified_bytes,
        observed_bytes,
    })
}

/// Advance only from the last returned ranked hit. The last examined gram
/// candidate may be a false positive or rank before/after an unseen result.
pub(crate) fn advance_private_kind(
    state: &mut SearchContinuationState,
    kind: SearchKind,
    page: &PrivateSearchKindPage,
) -> Result<(), SearchV2Error> {
    if page.has_more {
        let last = page.hits.last().ok_or_else(|| {
            error(
                SearchV2ErrorCode::MissingProgress,
                "nonterminal ranked page is empty",
            )
        })?;
        state.advance(kind, Some(last.order.clone()), false)
    } else {
        state.advance(kind, None, true)
    }
}

pub(crate) fn exhausted_private_kind() -> PrivateSearchKindPage {
    PrivateSearchKindPage {
        hits: Vec::new(),
        observed: Vec::new(),
        has_more: false,
        matching_total: 0,
        gram_seed: None,
        gram_charge: GramSeekCharge::default(),
        posting_charge: GramSeekCharge::default(),
        candidate_charge: CandidateReadCharge {
            vm_steps: 0,
            rows: 0,
            decoded_bytes: 0,
        },
        verified_chars: 0,
        verified_bytes: 0,
        observed_bytes: 0,
    }
}

#[cfg(test)]
mod tests {
    use tos_foundation::{
        CanonicalProfile, JsonLimits, JsonMode, JsonValue, canonical_bytes_v1, parse_json,
    };

    use super::*;
    use crate::search_document::SearchDocumentBudget;
    use crate::search_index::{GramStat, PostingPage};
    use crate::search_v2::{
        CurrentPolicyBinding, IndexedSearchV2Request, QUERY_PRIMITIVE_PROFILE,
        QueryVocabularyBinding, SEARCH_READ_MODEL_ABI_V1, SEARCH_UNICODE_PROFILE,
        SearchSelectionBinding,
    };

    fn field<'a>(value: &'a JsonValue, name: &str) -> &'a JsonValue {
        value.object_get(name).expect("oracle field")
    }

    struct Vocabulary {
        binding: QueryVocabularyBinding,
        sources: Vec<String>,
    }

    impl SelectedQueryVocabulary for Vocabulary {
        fn binding(&self) -> &QueryVocabularyBinding {
            &self.binding
        }
        fn registered_source_ids(&self) -> &[String] {
            &self.sources
        }
    }

    fn fixture() -> (
        Vec<SelectedSearchCandidate>,
        Vocabulary,
        NormalizedIndexedSearchV2Request,
        SearchSelectionBinding,
    ) {
        let oracle = parse_json(
            include_bytes!("../tests/fixtures/search_candidate_python_oracle.json"),
            JsonMode::PublishedStrict,
            JsonLimits::default(),
        )
        .unwrap()
        .into_root();
        let binding = QueryVocabularyBinding {
            descriptor_sha256: Digest256::of_bytes(b"fixture descriptor"),
            descriptor_version: 1,
        };
        let vocabulary = Vocabulary {
            binding: binding.clone(),
            sources: vec!["canon".into(), "philosophy".into()],
        };
        let digest = || Digest256::of_bytes(b"selected fixture root");
        let selection = SearchSelectionBinding {
            model_abi: SEARCH_READ_MODEL_ABI_V1.into(),
            vocabulary: binding,
            semantic_primitive_profile: QUERY_PRIMITIVE_PROFILE.into(),
            search_unicode_profile: SEARCH_UNICODE_PROFILE.into(),
            source_cut: "fixture-cut".into(),
            through_commit_seq: 1,
            source_membership_root: digest(),
            history_root_sha256: Some(digest()),
            entity_registry_id: "entity-registry".into(),
            entity_registry_version: "v1".into(),
            entity_registry_sha256: digest(),
            relation_registry_id: "relation-registry".into(),
            relation_registry_version: "v1".into(),
            relation_registry_sha256: digest(),
            graph_root_sha256: digest(),
            catalog_packet_sha256: digest(),
            catalog_index_root_sha256: digest(),
            source_scope_root_sha256: digest(),
            search_index_root_sha256: digest(),
            index_root_sha256: digest(),
            index_generation: "generation-1".into(),
            route_map_version: "route-map-1".into(),
            reader_abi: "reader-v1".into(),
            complete: true,
        };
        let request = IndexedSearchV2Request {
            query: field(&oracle, "query").as_str().unwrap().into(),
            sources: vec![],
            kind_ids: vec![],
            predicate_ids: vec![],
            limit: 2,
        }
        .normalize(&selection, &vocabulary)
        .unwrap();
        let rows = field(&oracle, "rows")
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                let item = field(row, "item");
                let payload = canonical_bytes_v1(
                    item,
                    CanonicalProfile::SourceRecordDigestV1,
                    JsonLimits::default(),
                )
                .unwrap();
                SelectedSearchCandidate {
                    kind: SearchKind::Nodes,
                    position: field(row, "position").as_u64().unwrap(),
                    id: field(item, "id").as_str().unwrap().into(),
                    source_graph: field(item, "source_graph").as_str().unwrap().into(),
                    kind_id: field(item, "kind_id").as_str().unwrap().into(),
                    predicate_id: String::new(),
                    id_lower: field(row, "id_lower").as_str().unwrap().into(),
                    native_id_lower: field(row, "native_id_lower").as_str().unwrap().into(),
                    identity_values: field(row, "identity_values").as_str().unwrap().into(),
                    visible_values: field(row, "visible_values").as_str().unwrap().into(),
                    document_chars: field(row, "document_chars").as_u64().unwrap(),
                    document_digest: Digest256::from_hex(
                        field(row, "document_digest").as_str().unwrap(),
                    )
                    .unwrap(),
                    payload_sha256: Digest256::of_bytes(&payload),
                    payload,
                }
            })
            .collect();
        (rows, vocabulary, request, selection)
    }

    struct Model {
        rows: Vec<SelectedSearchCandidate>,
        posting_calls: usize,
        candidate_calls: usize,
    }

    impl SearchGramModel for Model {
        fn gram_stat(
            &mut self,
            _: SearchKind,
            _: &str,
            _: u64,
            _: u64,
            _: u64,
        ) -> Result<GramStat, SearchV2Error> {
            Ok(GramStat {
                postings: Some(self.rows.len() as u64),
                charged: GramSeekCharge {
                    lookups: 1,
                    vm_steps: 1,
                    rows: 1,
                    decoded_bytes: 8,
                },
            })
        }
    }

    impl SearchPostingModel for Model {
        fn seek_postings(
            &mut self,
            _: SearchKind,
            _: &str,
            after: Option<u64>,
            max_rows: usize,
            _: u64,
            _: u64,
        ) -> Result<PostingPage, SearchV2Error> {
            self.posting_calls += 1;
            let positions: Vec<_> = self
                .rows
                .iter()
                .map(|row| row.position)
                .filter(|position| after.is_none_or(|previous| *position > previous))
                .take(max_rows)
                .collect();
            let count = positions.len() as u64;
            Ok(PostingPage {
                exhausted: positions.len() < max_rows,
                positions,
                charged: GramSeekCharge {
                    lookups: 1,
                    vm_steps: 1,
                    rows: count,
                    decoded_bytes: count * 8,
                },
            })
        }
    }

    impl SearchCandidateModel for Model {
        fn exact_candidate(
            &mut self,
            _: SearchKind,
            position: u64,
            budget: CandidateReadBudget,
        ) -> Result<(SelectedSearchCandidate, CandidateReadCharge), SearchV2Error> {
            self.candidate_calls += 1;
            let row = self.rows.get(position as usize).unwrap().clone();
            let bytes =
                row.payload.len() as u64 + row.id.len() as u64 + row.source_graph.len() as u64;
            assert!(bytes <= budget.max_decoded_bytes && budget.max_vm_steps > 0);
            Ok((
                row,
                CandidateReadCharge {
                    vm_steps: 1,
                    rows: 1,
                    decoded_bytes: bytes,
                },
            ))
        }
    }

    #[derive(Default)]
    struct Authority {
        denied: Option<u64>,
        consulted: Vec<u64>,
        checks: usize,
    }

    impl SearchCurrentAuthority for Authority {
        fn check_selected(&mut self) -> Result<(), SearchV2Error> {
            self.checks += 1;
            Ok(())
        }
        fn authorize_current(
            &mut self,
            candidate: &SelectedSearchCandidate,
        ) -> Result<(), SearchV2Error> {
            self.consulted.push(candidate.position);
            if self.denied == Some(candidate.position) {
                Err(error(
                    SearchV2ErrorCode::StalePolicy,
                    "owner withdrew selected carrier",
                ))
            } else {
                Ok(())
            }
        }
    }

    fn budget() -> SearchKindBudget {
        SearchKindBudget {
            grams: GramSeekBudget {
                max_lookups: 3,
                max_candidates: 7,
                max_vm_steps: 10,
                max_rows: 3,
                max_decoded_bytes: 24,
            },
            postings: PostingSeekBudget {
                max_probes: 4,
                max_rows: 8,
                max_decoded_bytes: 64,
                max_vm_steps: 10,
                page_rows: 2,
            },
            candidate: CandidateReadBudget {
                max_vm_steps: 10,
                max_decoded_bytes: 4096,
                max_payload_bytes: 4096,
                max_field_bytes: 4096,
                max_document_chars: 8192,
            },
            verify: CandidateVerifyBudget {
                document: SearchDocumentBudget {
                    max_carrier_bytes: 4096,
                    max_document_bytes: 8192,
                    max_document_code_points: 8192,
                    json: JsonLimits::default(),
                },
                max_rank_field_bytes: 4096,
                max_rank_values: 64,
            },
            max_candidate_vm_steps: 10,
            max_candidate_decoded_bytes: 8192,
            max_verified_chars: 8192,
            max_verified_bytes: 8192,
            max_observed_candidates: 7,
            max_observed_bytes: 8192,
            max_selected_result_bytes: 8192,
        }
    }

    #[test]
    fn private_page_matches_independent_rank_oracle_and_checks_false_positive_policy() {
        let (rows, vocabulary, request, selection) = fixture();
        let mut model = Model {
            rows,
            posting_calls: 0,
            candidate_calls: 0,
        };
        let mut authority = Authority::default();
        let page = execute_private_kind_page(
            &mut model,
            &vocabulary,
            &mut authority,
            SearchKind::Nodes,
            &request,
            None,
            budget(),
        )
        .unwrap();
        assert_eq!(
            page.hits
                .iter()
                .map(|hit| hit.id.as_str())
                .collect::<Vec<_>>(),
            ["alpha", "node.native"]
        );
        assert_eq!(page.matching_total, 6);
        assert!(page.has_more);
        assert_eq!(page.observed.len(), 7);
        assert_eq!(authority.consulted, [0, 1, 2, 3, 4, 5, 6]);
        assert_eq!(model.candidate_calls, 7);
        assert_eq!(model.posting_calls, 4);
        assert_eq!(page.candidate_charge.rows, 7);
        assert!(page.verified_chars > 0);
        assert!(page.observed_bytes > 0);
        let mut continuation = SearchContinuationState::new(
            selection,
            request.clone(),
            CurrentPolicyBinding {
                scope: "fixture-current-policy".into(),
                issuer_ref: "fixture-issuer".into(),
                authorization_receipt_id: "fixture-receipt".into(),
                policy_epoch: "fixture-policy-epoch".into(),
                withdrawal_generation: "fixture-withdrawal-1".into(),
            },
            &vocabulary,
        )
        .unwrap();
        advance_private_kind(&mut continuation, SearchKind::Nodes, &page).unwrap();
        assert_eq!(
            continuation.after(SearchKind::Nodes),
            page.hits.last().map(|hit| &hit.order)
        );
        assert_ne!(
            continuation
                .after(SearchKind::Nodes)
                .unwrap()
                .source_position(),
            6
        );

        let mut denied = Authority {
            denied: Some(6),
            ..Authority::default()
        };
        assert_eq!(
            execute_private_kind_page(
                &mut model,
                &vocabulary,
                &mut denied,
                SearchKind::Nodes,
                &request,
                None,
                budget()
            )
            .unwrap_err()
            .code,
            SearchV2ErrorCode::StalePolicy
        );
        assert_eq!(denied.consulted.last(), Some(&6));
    }

    #[test]
    fn one_under_verified_work_fails_without_private_result() {
        let (rows, vocabulary, request, _) = fixture();
        let mut model = Model {
            rows,
            posting_calls: 0,
            candidate_calls: 0,
        };
        let mut authority = Authority::default();
        let mut admitted = budget();
        admitted.max_verified_chars =
            model.rows.iter().map(|row| row.document_chars).sum::<u64>() - 1;
        assert_eq!(
            execute_private_kind_page(
                &mut model,
                &vocabulary,
                &mut authority,
                SearchKind::Nodes,
                &request,
                None,
                admitted
            )
            .unwrap_err()
            .code,
            SearchV2ErrorCode::BudgetExceeded
        );
        assert_eq!(model.candidate_calls, 7);
    }

    #[test]
    fn one_under_observed_identity_bytes_fails_without_private_result() {
        let (rows, vocabulary, request, _) = fixture();
        let expected: u64 = rows
            .iter()
            .map(|row| {
                OBSERVED_FIXED_BYTES_V1 + row.id.len() as u64 + row.source_graph.len() as u64
            })
            .sum();
        let mut model = Model {
            rows,
            posting_calls: 0,
            candidate_calls: 0,
        };
        let mut authority = Authority::default();
        let mut admitted = budget();
        admitted.max_observed_bytes = expected - 1;
        assert_eq!(
            execute_private_kind_page(
                &mut model,
                &vocabulary,
                &mut authority,
                SearchKind::Nodes,
                &request,
                None,
                admitted
            )
            .unwrap_err()
            .code,
            SearchV2ErrorCode::BudgetExceeded
        );
        assert_eq!(model.candidate_calls, 7);
    }
}
