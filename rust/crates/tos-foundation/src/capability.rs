use crate::descriptor::DESCRIPTOR_FORMAT_VERSION;
use crate::json::{CanonicalProfile, FORMAT_VERSION};

/// Versioned disclosure of what this small foundation package actually supports.
/// A caller must not infer source admission or complete schema validation from it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoundationCapabilities {
    pub json_format: &'static str,
    pub descriptor_format: &'static str,
    pub canonical_profile: &'static str,
    pub canonical_float_supported: bool,
    pub strict_duplicate_rejection: bool,
    pub request_last_wins: bool,
    pub escaped_lone_surrogate_preserved: bool,
    pub canonical_lone_surrogate_supported: bool,
}

pub const fn capabilities() -> FoundationCapabilities {
    FoundationCapabilities {
        json_format: FORMAT_VERSION,
        descriptor_format: DESCRIPTOR_FORMAT_VERSION,
        canonical_profile: CanonicalProfile::CorpusSnapshotV1.as_str(),
        canonical_float_supported: CanonicalProfile::CorpusSnapshotV1.supports_float(),
        strict_duplicate_rejection: true,
        request_last_wins: true,
        escaped_lone_surrogate_preserved: true,
        canonical_lone_surrogate_supported: false,
    }
}
