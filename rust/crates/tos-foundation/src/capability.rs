use crate::json::{CanonicalProfile, FORMAT_VERSION, JsonEmissionProfile, JsonMode};
use crate::logical_ref::LogicalRecordRefV1;
use crate::unicode::UnicodeProfile;

// Retained only for the existing web-codec capability wire. No in-memory
// descriptor registry or descriptor-construction API is supplied by Foundation.
const LEGACY_DESCRIPTOR_WIRE_TAG: &str = "tos_foundation_descriptors_v1";

/// Versioned disclosure of what this small foundation package actually supports.
/// A caller must not infer source admission or complete schema validation from it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoundationCapabilities {
    pub json_format: &'static str,
    /// Compatibility wire tag; it does not advertise a descriptor registry.
    pub descriptor_format: &'static str,
    pub json_profiles: [&'static str; 2],
    pub canonical_profile: &'static str,
    pub canonical_profiles: [&'static str; 3],
    pub emission_profiles: [&'static str; 3],
    pub logical_record_ref_profile: &'static str,
    pub unicode_profiles: [&'static str; 1],
    pub unicode_data_version: &'static str,
    pub canonical_float_supported: bool,
    pub strict_duplicate_rejection: bool,
    pub request_last_wins: bool,
    pub escaped_lone_surrogate_preserved: bool,
    pub canonical_lone_surrogate_supported: bool,
}

pub const fn capabilities() -> FoundationCapabilities {
    FoundationCapabilities {
        json_format: FORMAT_VERSION,
        descriptor_format: LEGACY_DESCRIPTOR_WIRE_TAG,
        json_profiles: [
            JsonMode::PublishedStrict.as_str(),
            JsonMode::RequestLastWins.as_str(),
        ],
        canonical_profile: CanonicalProfile::CorpusSnapshotV1.as_str(),
        canonical_profiles: [
            CanonicalProfile::CorpusSnapshotV1.as_str(),
            CanonicalProfile::SourceRecordDigestV1.as_str(),
            CanonicalProfile::SourceCommandInputV1.as_str(),
        ],
        emission_profiles: [
            JsonEmissionProfile::SourceFormSetPublishedV1.as_str(),
            JsonEmissionProfile::SourceWitnessCatalogPublishedV3.as_str(),
            JsonEmissionProfile::SourceFoundationLabReportV1.as_str(),
        ],
        logical_record_ref_profile: LogicalRecordRefV1::PROFILE,
        unicode_profiles: [UnicodeProfile::PythonNativeUnicodeV1.as_str()],
        unicode_data_version: UnicodeProfile::PythonNativeUnicodeV1.ucd_version(),
        canonical_float_supported: CanonicalProfile::CorpusSnapshotV1.supports_float(),
        strict_duplicate_rejection: true,
        request_last_wins: true,
        escaped_lone_surrogate_preserved: true,
        canonical_lone_surrogate_supported: false,
    }
}
