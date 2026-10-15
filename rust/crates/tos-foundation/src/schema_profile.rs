//! Stable schema-execution identities shared by storage and validation.
use crate::Digest256;

/// The observed legacy profile is evidence for migration comparison only.
/// A normative profile is selected by the source owner, not by installed
/// optional format packages on the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatProfile {
    LegacyPythonObserved20260923,
    AssertedSourceCandidateV1,
}

impl FormatProfile {
    pub fn id(self) -> &'static str {
        match self {
            Self::LegacyPythonObserved20260923 => {
                "tos.schema.format.legacy-python-observed-20260923"
            }
            Self::AssertedSourceCandidateV1 => "tos.schema.format.asserted-source-candidate-v1",
        }
    }
}

/// Actual prepared-worker binding for schema execution over candidate input.
/// It deliberately omits the selected cut's `SourceRevision`; candidate
/// identity is carried by the source adapter's own opaque fence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CutPreparedSchemaExecutionBinding {
    pub schema_profile: FormatProfile,
    pub schema_set_sha256: Digest256,
    pub worker_sha256: Digest256,
    pub protocol: CutPreparedSchemaProtocol,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutPreparedSchemaProtocol {
    LegacyScalar,
    DiagnosticsV2 { caps_sha256: Digest256 },
}
