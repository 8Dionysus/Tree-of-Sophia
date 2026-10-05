//! Exact selected knowledge model ABI identities shared by the private
//! producer and transport-independent query semantics. These names describe
//! a physical posting format; they do not grant source or disclosure rights.

pub const KNOWLEDGE_MODEL_ABI_V2_POSTINGS_V1: &str = "tos_knowledge_read_model_v2_postings_v1";
pub const KNOWLEDGE_MODEL_ABI_V3_POSTINGS_V1: &str = "tos_knowledge_read_model_v3_postings_v1";
pub const KNOWLEDGE_MODEL_ABI_V4_POSTINGS_V1: &str = "tos_knowledge_read_model_v4_postings_v1";
pub const KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1: &str = "tos_knowledge_read_model_v5_postings_v1";
pub const KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1_CARRIER_ONCE_V1: &str =
    "tos_knowledge_read_model_v5_postings_v1_carrier_once_v1";
pub const KNOWLEDGE_MODEL_ABI_V6_POSTINGS_V1: &str = "tos_knowledge_read_model_v6_postings_v1";

/// Only the current compressed-posting selected format. Distinct versions
/// still require their own component/source-basis checks at cold admission.
pub const KNOWLEDGE_POSTINGS_MODEL_ABIS: [&str; 6] = [
    KNOWLEDGE_MODEL_ABI_V2_POSTINGS_V1,
    KNOWLEDGE_MODEL_ABI_V3_POSTINGS_V1,
    KNOWLEDGE_MODEL_ABI_V4_POSTINGS_V1,
    KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1,
    KNOWLEDGE_MODEL_ABI_V6_POSTINGS_V1,
    KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1_CARRIER_ONCE_V1,
];
