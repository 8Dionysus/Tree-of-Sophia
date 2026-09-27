//! Versioned software contracts plus exact owner-selected registry inputs.
//! The pure builder grants no source access or public disclosure authority.
use crate::{
    search_v2::{SearchV2Error, SearchV2ErrorCode},
    source_read_projection::{object, text},
};
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonMode, JsonValue, canonical_bytes_v1, parse_json,
};
/// Packaged software inputs are independent of data roots and process cwd.
pub const KNOWLEDGE_PROGRAM_CONTRACTS: &[(&str, &str, &[u8])] = &[
    (
        "api",
        "access/contracts/knowledge-api.v1.json",
        include_bytes!("../../../../access/contracts/knowledge-api.v1.json"),
    ),
    (
        "knowledge_graph",
        "access/contracts/knowledge-graph.v1.schema.json",
        include_bytes!("../../../../access/contracts/knowledge-graph.v1.schema.json"),
    ),
    (
        "knowledge_search_indexed",
        "access/contracts/knowledge-search-indexed.v2.schema.json",
        include_bytes!("../../../../access/contracts/knowledge-search-indexed.v2.schema.json"),
    ),
    (
        "readable_context",
        "access/contracts/readable-context.v1.schema.json",
        include_bytes!("../../../../access/contracts/readable-context.v1.schema.json"),
    ),
    (
        "lens_spec",
        "access/contracts/lens-spec.v1.schema.json",
        include_bytes!("../../../../access/contracts/lens-spec.v1.schema.json"),
    ),
    (
        "lens_result",
        "access/contracts/lens-result.v1.schema.json",
        include_bytes!("../../../../access/contracts/lens-result.v1.schema.json"),
    ),
    (
        "temporal_comparison_request",
        "access/contracts/temporal-comparison-request.v1.schema.json",
        include_bytes!("../../../../access/contracts/temporal-comparison-request.v1.schema.json"),
    ),
    (
        "temporal_comparison_result",
        "access/contracts/temporal-comparison-result.v1.schema.json",
        include_bytes!("../../../../access/contracts/temporal-comparison-result.v1.schema.json"),
    ),
    (
        "source_read",
        "access/contracts/source-read.v1.schema.json",
        include_bytes!("../../../../access/contracts/source-read.v1.schema.json"),
    ),
    (
        "entity_type_registry_schema",
        "ToS/contracts/semantic-entity-type-registry.schema.json",
        include_bytes!("../../../../ToS/contracts/semantic-entity-type-registry.schema.json"),
    ),
    (
        "relation_type_registry_schema",
        "ToS/contracts/semantic-relation-type-registry.schema.json",
        include_bytes!("../../../../ToS/contracts/semantic-relation-type-registry.schema.json"),
    ),
];
pub const KNOWLEDGE_REGISTRY_CONTRACTS: &[(&str, &str)] = &[
    (
        "entity_type_registry",
        "ToS/doctrine/semantic-interchange/entity-types.v1.json",
    ),
    (
        "relation_type_registry",
        "ToS/doctrine/semantic-interchange/relation-types.v1.json",
    ),
];
#[derive(Clone, Copy, Debug)]
pub struct KnowledgeContractBudget {
    pub max_input_bytes: usize,
    pub max_registry_bytes: usize,
    pub max_response_bytes: usize,
    pub json: JsonLimits,
}
fn err(code: SearchV2ErrorCode, message: &'static str) -> SearchV2Error {
    SearchV2Error { code, message }
}
/// Expected digests must originate in the caller's selected owner receipt.
/// Digest equality establishes identity only; the adapter retains source-owner
/// authorization and its disclosure hold through the final transport flush.
pub fn build_knowledge_contract_bundle(
    registry_bytes: [&[u8]; 2],
    expected: [Digest256; 2],
    budget: KnowledgeContractBudget,
) -> Result<JsonValue, SearchV2Error> {
    if budget.max_input_bytes == 0
        || budget.max_registry_bytes == 0
        || budget.max_response_bytes == 0
    {
        return Err(err(
            SearchV2ErrorCode::BudgetExceeded,
            "contract admission unavailable",
        ));
    }
    let mut total = 0usize;
    for bytes in KNOWLEDGE_PROGRAM_CONTRACTS
        .iter()
        .map(|(_, _, bytes)| *bytes)
        .chain(registry_bytes)
    {
        total = total.checked_add(bytes.len()).ok_or_else(|| {
            err(
                SearchV2ErrorCode::BudgetExceeded,
                "contract byte accounting overflow",
            )
        })?;
        if total > budget.max_input_bytes {
            return Err(err(
                SearchV2ErrorCode::BudgetExceeded,
                "contract inputs exceed cap",
            ));
        }
    }
    let mut contracts = vec![];
    let mut refs = vec![];
    let parse = |bytes: &[u8]| {
        let mut limits = budget.json;
        limits.max_bytes = limits.max_bytes.min(budget.max_input_bytes);
        parse_json(bytes, JsonMode::PublishedStrict, limits)
            .map(|p| p.into_root())
            .map_err(|reason| {
                err(
                    if reason.code == tos_foundation::FoundationErrorCode::BudgetExceeded {
                        SearchV2ErrorCode::BudgetExceeded
                    } else {
                        SearchV2ErrorCode::CorruptSelectedCarrier
                    },
                    "contract JSON admission failed",
                )
            })
    };
    for (id, path, bytes) in KNOWLEDGE_PROGRAM_CONTRACTS {
        contracts.push((*id, parse(bytes)?));
        refs.push(text(path));
    }
    for (i, (id, path)) in KNOWLEDGE_REGISTRY_CONTRACTS.iter().enumerate() {
        let bytes = registry_bytes[i];
        if bytes.len() > budget.max_registry_bytes {
            return Err(err(
                SearchV2ErrorCode::BudgetExceeded,
                "contract registry exceeds cap",
            ));
        }
        if Digest256::of_bytes(bytes) != expected[i] {
            return Err(err(
                SearchV2ErrorCode::StaleSelection,
                "selected contract registry bytes differ",
            ));
        }
        contracts.push((*id, parse(bytes)?));
        refs.push(text(path));
    }
    let packet = object(vec![
        ("schema", text("tos_knowledge_contract_bundle_v1")),
        ("contracts", object(contracts)),
        ("source_refs", JsonValue::Array(refs)),
        (
            "authority_boundary",
            object(vec![
                ("is_source", JsonValue::Bool(false)),
                ("writes_to_tree", JsonValue::Bool(false)),
                ("source_owner", text("Tree-of-Sophia/access/contracts")),
                (
                    "note",
                    text(
                        "This packet transports versioned access contracts; it does not author ToS meaning.",
                    ),
                ),
            ]),
        ),
    ]);
    let mut limits = budget.json;
    limits.max_bytes = limits.max_bytes.min(budget.max_response_bytes);
    canonical_bytes_v1(&packet, CanonicalProfile::SourceRecordDigestV1, limits).map_err(|_| {
        err(
            SearchV2ErrorCode::BudgetExceeded,
            "contract response exceeds cap",
        )
    })?;
    Ok(packet)
}
/// Bind the pure bundle to a cold-verified native selection. This is deliberately
/// not a Disclosable packet: callers still need exact registry source authority.
#[cfg(not(target_arch = "wasm32"))]
pub fn build_selected_knowledge_contract_bundle(
    model: &tos_compiler::VerifiedKnowledgeModel<'_>,
    bound: &crate::BoundCmpKnowledge<'_>,
    registry_bytes: [&[u8]; 2],
    budget: KnowledgeContractBudget,
) -> Result<JsonValue, SearchV2Error> {
    bound.check_model(model)?;
    let selected = model.selection();
    let entity = Digest256::from_hex(&selected.entity_registry_sha256).map_err(|_| {
        err(
            SearchV2ErrorCode::CorruptSelectedCarrier,
            "selected entity registry digest invalid",
        )
    })?;
    let relation = Digest256::from_hex(&selected.relation_registry_sha256).map_err(|_| {
        err(
            SearchV2ErrorCode::CorruptSelectedCarrier,
            "selected relation registry digest invalid",
        )
    })?;
    build_knowledge_contract_bundle(registry_bytes, [entity, relation], budget)
}

pub const KNOWLEDGE_CONTRACTS_OPERATION: &str = "tos.knowledge.contracts";
pub const KNOWLEDGE_CONTRACTS_INTENDED_USE: &str = "read_only_public_knowledge_contracts_v1";

/// Exact borrowed selected registry carriers plus the existing owner hold.
/// A missing production registry holder remains unavailable. Digest equality
/// alone cannot satisfy `authorize_registry_current` or acquire a public grant.
#[cfg(not(target_arch = "wasm32"))]
pub fn execute_selected_knowledge_contracts<'hold, A: crate::InspectCurrentAuthority<'hold> + ?Sized>(
    model: &mut tos_compiler::VerifiedKnowledgeModel<'_>,
    bound: &crate::BoundCmpKnowledge<'_>,
    authority: &mut A,
    registry_bytes: [&[u8]; 2],
    budget: KnowledgeContractBudget,
    inspect: crate::InspectBudget,
) -> Result<crate::DisclosableInspect<'hold>, SearchV2Error> {
    bound.check_model(model)?;
    let selection = bound.selection();
    let expected = [
        selection.entity_registry_sha256,
        selection.relation_registry_sha256,
    ];
    let ids = [
        &selection.entity_registry_id,
        &selection.relation_registry_id,
    ];
    let versions = [
        &selection.entity_registry_version,
        &selection.relation_registry_version,
    ];
    crate::knowledge_inspect::execute_selected_carrier_packet(
        model,
        bound,
        authority,
        KNOWLEDGE_CONTRACTS_OPERATION,
        KNOWLEDGE_CONTRACTS_INTENDED_USE,
        inspect,
        |reader| {
            reader.check_interrupt()?;
            let packet = build_knowledge_contract_bundle(registry_bytes, expected, budget)?;
            let contracts = packet.object_get("contracts").ok_or_else(|| {
                err(
                    SearchV2ErrorCode::CorruptSelectedCarrier,
                    "contract map absent",
                )
            })?;
            for (i, (key, _)) in KNOWLEDGE_REGISTRY_CONTRACTS.iter().enumerate() {
                let registry = contracts.object_get(key).ok_or_else(|| {
                    err(
                        SearchV2ErrorCode::CorruptSelectedCarrier,
                        "selected contract registry absent",
                    )
                })?;
                let version = registry
                    .object_get("registry_version")
                    .and_then(|v| match v {
                        JsonValue::Number(n) if n.kind == tos_foundation::JsonNumberKind::Int => {
                            Some(n.lexeme.as_str())
                        }
                        JsonValue::String(_) => v.as_str(),
                        _ => None,
                    });
                if registry
                    .object_get("registry_id")
                    .and_then(JsonValue::as_str)
                    != Some(ids[i].as_str())
                    || version != Some(versions[i].as_str())
                {
                    return Err(err(
                        SearchV2ErrorCode::StaleSelection,
                        "selected contract registry identity differs",
                    ));
                }
                reader.registry_current(ids[i], registry_bytes[i], expected[i])?;
            }
            Ok(packet)
        },
    )
}
