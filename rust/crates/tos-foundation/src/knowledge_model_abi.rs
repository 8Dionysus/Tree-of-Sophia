//! Exact selected knowledge model ABI identities shared by the private
//! producer and transport-independent query semantics. These names describe
//! a physical posting format; they do not grant source or disclosure rights.

pub const KNOWLEDGE_MODEL_ABI_V2_POSTINGS_V1: &str = "tos_knowledge_read_model_v2_postings_v1";
pub const KNOWLEDGE_MODEL_ABI_V3_POSTINGS_V1: &str = "tos_knowledge_read_model_v3_postings_v1";
pub const KNOWLEDGE_MODEL_ABI_V4_POSTINGS_V1: &str = "tos_knowledge_read_model_v4_postings_v1";
pub const KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1: &str = "tos_knowledge_read_model_v5_postings_v1";
pub const KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1_CARRIER_ONCE_V1: &str =
    "tos_knowledge_read_model_v5_postings_v1_carrier_once_v1";
pub const KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1_CARRIER_ONCE_V2: &str =
    "tos_knowledge_read_model_v5_postings_v1_carrier_once_v2";
pub const KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1_CARRIER_ONCE_V3: &str =
    "tos_knowledge_read_model_v5_postings_v1_carrier_once_v3";
pub const KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V2_CARRIER_ONCE_V4: &str =
    "tos_knowledge_read_model_v5_postings_v2_carrier_once_v4";
pub const KNOWLEDGE_MODEL_ABI_V6_POSTINGS_V1: &str = "tos_knowledge_read_model_v6_postings_v1";

/// Native PostgreSQL bootstrap anchor. It retains authenticated graph,
/// catalog and Original carriers; it has no offline search tables or root.
/// Search is a separately versioned PostgreSQL projection, never an empty index.
pub const KNOWLEDGE_MODEL_ABI_MANAGED_NAVIGATION_V1: &str = "tos_knowledge_managed_navigation_v1";

/// Native PostgreSQL bootstrap anchor with exact Original carriers. Normalized
/// graph rows and offline search are absent capabilities; the current graph is
/// served by the PostgreSQL projection. V1 remains readable as its own format.
pub const KNOWLEDGE_MODEL_ABI_MANAGED_NAVIGATION_V2: &str = "tos_knowledge_managed_navigation_v2";

pub fn is_managed_navigation_model_abi(abi: &str) -> bool {
    matches!(
        abi,
        KNOWLEDGE_MODEL_ABI_MANAGED_NAVIGATION_V1 | KNOWLEDGE_MODEL_ABI_MANAGED_NAVIGATION_V2
    )
}

pub fn is_managed_knowledge_model_abi(abi: &str) -> bool {
    matches!(
        abi,
        KNOWLEDGE_MODEL_ABI_V6_POSTINGS_V1
            | KNOWLEDGE_MODEL_ABI_MANAGED_NAVIGATION_V1
            | KNOWLEDGE_MODEL_ABI_MANAGED_NAVIGATION_V2
    )
}

/// Only the current compressed-posting selected format. Distinct versions
/// still require their own component/source-basis checks at cold admission.
pub const KNOWLEDGE_POSTINGS_MODEL_ABIS: [&str; 9] = [
    KNOWLEDGE_MODEL_ABI_V2_POSTINGS_V1,
    KNOWLEDGE_MODEL_ABI_V3_POSTINGS_V1,
    KNOWLEDGE_MODEL_ABI_V4_POSTINGS_V1,
    KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1,
    KNOWLEDGE_MODEL_ABI_V6_POSTINGS_V1,
    KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1_CARRIER_ONCE_V1,
    KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1_CARRIER_ONCE_V2,
    KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1_CARRIER_ONCE_V3,
    KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V2_CARRIER_ONCE_V4,
];
