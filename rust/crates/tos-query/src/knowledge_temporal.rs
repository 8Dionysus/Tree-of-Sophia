//! Selected exact-ID temporal adapter; shares inspect's current held carrier
//! disclosure and caller-selected read/transfer admission.
use crate::{
    knowledge_binding::BoundCmpKnowledge,
    knowledge_inspect::{
        DisclosableInspect, InspectBudget, InspectCurrentAuthority, execute_selected_carrier_packet,
    },
    search_v2::{SearchKind, SearchV2Error, SearchV2ErrorCode},
    temporal_comparison::{TEMPORAL_INTENDED_USE, TEMPORAL_OPERATION, compare_temporal_operands},
};
use tos_compiler::VerifiedKnowledgeModel;
use tos_foundation::JsonValue;
pub fn execute_selected_temporal<'hold, A: InspectCurrentAuthority<'hold> + ?Sized>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    request: &JsonValue,
    budget: InspectBudget,
) -> Result<DisclosableInspect<'hold>, SearchV2Error> {
    bound.require_source_revision()?;
    let claim_source = bound
        .source_for_adapter("reified-bibliographic-claims-v1")
        .ok_or(SearchV2Error {
            code: SearchV2ErrorCode::Unavailable,
            message: "selected source Claim adapter unavailable",
        })?;
    execute_selected_carrier_packet(
        model,
        bound,
        authority,
        TEMPORAL_OPERATION,
        TEMPORAL_INTENDED_USE,
        budget,
        |read| {
            compare_temporal_operands(
                bound.require_source_revision()?,
                request,
                claim_source,
                |id| read.items(SearchKind::Nodes, "id", id, 1, false),
                budget.json,
            )
        },
    )
}

/// Source-preserving multi-Claim view over exact selected-model members.
/// The requested profile is display metadata, not an admitted source policy.
pub fn execute_selected_claim_projection<'hold, A: InspectCurrentAuthority<'hold> + ?Sized>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    request: &JsonValue,
    budget: InspectBudget,
) -> Result<DisclosableInspect<'hold>, SearchV2Error> {
    use crate::temporal_comparison::{
        CLAIM_PROJECTION_INTENDED_USE, CLAIM_PROJECTION_OPERATION, project_temporal_claims,
    };
    let revision = bound.require_source_revision()?;
    let claim_source = bound
        .source_for_adapter("reified-bibliographic-claims-v1")
        .ok_or(SearchV2Error {
            code: SearchV2ErrorCode::Unavailable,
            message: "selected source Claim adapter unavailable",
        })?;
    let mut limits = budget.json;
    limits.max_bytes = limits.max_bytes.min(budget.max_payload_bytes);
    // Bound request transfer before retaining per-member request clones.
    tos_foundation::canonical_bytes_v1(
        request,
        tos_foundation::CanonicalProfile::SourceRecordDigestV1,
        limits,
    )
    .map_err(|reason| SearchV2Error {
        code: if reason.code == tos_foundation::FoundationErrorCode::BudgetExceeded {
            SearchV2ErrorCode::BudgetExceeded
        } else {
            SearchV2ErrorCode::InvalidRequest
        },
        message: "Claim projection request exceeds JSON limits",
    })?;
    execute_selected_carrier_packet(
        model,
        bound,
        authority,
        CLAIM_PROJECTION_OPERATION,
        CLAIM_PROJECTION_INTENDED_USE,
        budget,
        |read| {
            project_temporal_claims(
                revision,
                request,
                claim_source,
                |id| read.items(SearchKind::Nodes, "id", id, 1, false),
                budget.json,
                budget
                    .max_matches
                    .min(usize::try_from(budget.max_rows).unwrap_or(usize::MAX)),
            )
        },
    )
}

/// Discover real Claim members from the selected source keyset, under one current hold.
/// Cursors are untrusted suffix positions, not proof of a visited prefix or chronology.
pub fn execute_selected_claim_list<'hold, A: InspectCurrentAuthority<'hold> + ?Sized>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    request: &JsonValue,
    budget: InspectBudget,
) -> Result<DisclosableInspect<'hold>, SearchV2Error> {
    use crate::source_read_projection::{object, text};
    use crate::temporal_comparison::{
        CLAIM_LIST_INTENDED_USE, CLAIM_LIST_OPERATION, ClaimListBinding, claim_list_cursor,
        evaluate_selected_claim_list_temporal, parse_claim_list_request, selected_claim_list_row,
    };
    use tos_foundation::{
        CanonicalProfile, Digest256, JsonNumber, JsonNumberKind, canonical_bytes_v1,
    };
    let revision = bound.require_source_revision()?;
    let source_graph = bound
        .source_for_adapter("reified-bibliographic-claims-v1")
        .ok_or(SearchV2Error {
            code: SearchV2ErrorCode::Unavailable,
            message: "selected source Claim adapter unavailable",
        })?;
    let mut limits = budget.json;
    limits.max_bytes = limits.max_bytes.min(budget.max_payload_bytes);
    canonical_bytes_v1(request, CanonicalProfile::SourceRecordDigestV1, limits).map_err(
        |reason| SearchV2Error {
            code: if reason.code == tos_foundation::FoundationErrorCode::BudgetExceeded {
                SearchV2ErrorCode::BudgetExceeded
            } else {
                SearchV2ErrorCode::InvalidRequest
            },
            message: "Claim list request exceeds JSON limits",
        },
    )?;
    let policy = authority.policy_binding();
    let policy_value = object(vec![
        ("scope", text(&policy.scope)),
        ("issuer_ref", text(&policy.issuer_ref)),
        ("receipt_id", text(&policy.authorization_receipt_id)),
        ("epoch", text(&policy.policy_epoch)),
        ("withdrawal_generation", text(&policy.withdrawal_generation)),
    ]);
    let policy_digest = Digest256::of_bytes(
        &canonical_bytes_v1(
            &policy_value,
            CanonicalProfile::SourceRecordDigestV1,
            budget.json,
        )
        .map_err(|_| SearchV2Error {
            code: SearchV2ErrorCode::BudgetExceeded,
            message: "Claim list policy binding budget",
        })?,
    )
    .to_hex();
    let index_digest = bound.selection().index_root_sha256.to_hex();
    let binding = ClaimListBinding {
        revision,
        source_graph,
        model_receipt: bound.owner_receipt_id(),
        index_digest: &index_digest,
        policy_digest: &policy_digest,
    };
    let default_temporal = tos_foundation::temporal::TemporalBudget::default();
    let temporal_budget = tos_foundation::temporal::TemporalBudget {
        max_expression_bytes: default_temporal
            .max_expression_bytes
            .min(budget.max_payload_bytes)
            .min(budget.max_response_bytes)
            .min(budget.json.max_bytes),
        max_members: default_temporal.max_members.min(budget.json.max_visits),
        max_pairings: default_temporal.max_pairings,
    };
    let source_revision =
        tos_foundation::SourceRevision(Digest256::from_hex(revision).map_err(|_| {
            SearchV2Error {
                code: SearchV2ErrorCode::StaleSelection,
                message: "Claim list source revision invalid",
            }
        })?);
    let parsed = parse_claim_list_request(
        request,
        &binding,
        budget.max_matches,
        budget.max_rows,
        budget.json,
        temporal_budget,
    )?;
    // Independent logical allowance, conservatively partitioned across every
    // candidate (including non-Claims). This is not SQLite VM-step accounting.
    let mut temporal_budget = temporal_budget;
    if parsed.filter.object_get("temporal").is_some() {
        temporal_budget.max_pairings = temporal_budget
            .max_pairings
            .min(budget.json.max_visits / parsed.scan_limit);
        if temporal_budget.max_pairings == 0
            || parsed
                .scan_limit
                .checked_mul(temporal_budget.max_pairings)
                .is_none_or(|work| work > budget.json.max_visits)
        {
            return Err(SearchV2Error {
                code: SearchV2ErrorCode::BudgetExceeded,
                message: "Claim list whole-page temporal allowance insufficient",
            });
        }
    }
    execute_selected_carrier_packet(
        model,
        bound,
        authority,
        CLAIM_LIST_OPERATION,
        CLAIM_LIST_INTENDED_USE,
        budget,
        |read| {
            let candidates = read.candidate_ids(
                SearchKind::Nodes,
                &[source_graph.to_owned()],
                parsed.after.as_ref().map(|(s, p)| (s.as_str(), *p)),
                parsed.scan_limit,
            )?;
            let fetched = candidates.len();
            let mut examined = 0usize;
            let mut last = None;
            let mut rows = Vec::new();
            for (graph, position, id) in candidates {
                read.check_interrupt()?;
                if graph != source_graph {
                    return Err(SearchV2Error {
                        code: SearchV2ErrorCode::CorruptSelectedCarrier,
                        message: "Claim list candidate source differs",
                    });
                }
                let mut payloads = read.items(SearchKind::Nodes, "id", &id, 1, false)?;
                if payloads.len() != 1 {
                    return Err(SearchV2Error {
                        code: SearchV2ErrorCode::CorruptSelectedCarrier,
                        message: "Claim list candidate payload absent",
                    });
                }
                let carrier = payloads.pop().ok_or(SearchV2Error {
                    code: SearchV2ErrorCode::CorruptSelectedCarrier,
                    message: "Claim list candidate payload absent",
                })?;
                let matched = selected_claim_list_row(
                    &carrier,
                    &id,
                    source_graph,
                    &parsed.filter,
                    budget.json,
                )?;
                examined += 1;
                last = Some(position);
                if let Some(mut row) = matched {
                    if let Some(temporal) = parsed.filter.object_get("temporal") {
                        let result = evaluate_selected_claim_list_temporal(
                            &row,
                            temporal,
                            source_revision,
                            temporal_budget,
                        )?;
                        match result.object_get("outcome").and_then(JsonValue::as_str) {
                            Some("NoMatch") => continue,
                            Some("Refused") => {
                                return Ok(object(vec![
                                    (
                                        "schema_version",
                                        text("tos_claim_list_non_total_refusal_v1"),
                                    ),
                                    ("outcome", text("NonTotalRefusal")),
                                    ("source_revision", text(revision)),
                                    ("selected_model_receipt_id", text(bound.owner_receipt_id())),
                                    ("filter", parsed.filter.clone()),
                                    ("refusal", result),
                                    ("assertion_scope", text("SupplementalCarrierOnly")),
                                    ("legacy_object_assessment", text("NotExamined")),
                                    ("binds_persisted_chronology", JsonValue::Bool(false)),
                                ]));
                            }
                            Some("Match") => {
                                let JsonValue::Object(ref mut fields) = row else {
                                    return Err(SearchV2Error {
                                        code: SearchV2ErrorCode::CorruptSelectedCarrier,
                                        message: "Claim list row invalid",
                                    });
                                };
                                fields.push((
                                    tos_foundation::JsonString::from_utf8("temporal_filter_result"),
                                    result,
                                ));
                            }
                            _ => {
                                return Err(SearchV2Error {
                                    code: SearchV2ErrorCode::CorruptSelectedCarrier,
                                    message: "Claim list temporal result invalid",
                                });
                            }
                        }
                    }
                    rows.push(row);
                }
                if rows.len() == parsed.page_size {
                    break;
                }
            }
            let exhausted = examined == fetched && fetched < parsed.scan_limit;
            let next = if exhausted {
                JsonValue::Null
            } else {
                last.map(|p| claim_list_cursor(&binding, &parsed.query_digest, p))
                    .ok_or(SearchV2Error {
                        code: SearchV2ErrorCode::CorruptSelectedCarrier,
                        message: "Claim list continuation made no progress",
                    })?
            };
            Ok(object(vec![
                ("schema_version", text("tos_claim_list_result_v1")),
                ("source_revision", text(revision)),
                ("selected_model_receipt_id", text(bound.owner_receipt_id())),
                ("filter", parsed.filter.clone()),
                ("rows", JsonValue::Array(rows)),
                (
                    "examined_candidates",
                    JsonValue::Number(JsonNumber {
                        kind: JsonNumberKind::Int,
                        lexeme: examined.to_string(),
                    }),
                ),
                ("exhausted", JsonValue::Bool(exhausted)),
                ("next", next),
                (
                    "projection_scope",
                    text("exact-selected-model-source-keyset-page"),
                ),
                ("cursor_scope", text("untrusted-suffix-position")),
                ("binds_persisted_chronology", JsonValue::Bool(false)),
            ]))
        },
    )
}
