//! Stable schema-execution identities shared by storage and validation.
use crate::Digest256;

pub const IN_PROCESS_SCHEMA_ENGINE_ID: &str = "tos_schema_engine_in_process_v1";

/// The observed legacy profile is evidence for migration comparison only.
/// A normative profile is selected by the source owner, not by installed
/// optional format packages on the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatProfile {
    /// Historical identifier retained in stored bindings and proof.
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

/// Identity of an in-process execution prepared for an exact schema set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InProcessSchemaExecutionBinding {
    pub schema_profile: FormatProfile,
    pub schema_set_sha256: Digest256,
    pub engine: &'static str,
    /// Absent only when reading an older binding that did not record input mode.
    pub input_mode_sha256: Option<Digest256>,
}

/// Versioned execution identity for adapters migrating from worker execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaExecutionBinding {
    Worker(CutPreparedSchemaExecutionBinding),
    InProcessV1(InProcessSchemaExecutionBinding),
}

impl From<CutPreparedSchemaExecutionBinding> for SchemaExecutionBinding {
    fn from(binding: CutPreparedSchemaExecutionBinding) -> Self {
        Self::Worker(binding)
    }
}

impl From<InProcessSchemaExecutionBinding> for SchemaExecutionBinding {
    fn from(binding: InProcessSchemaExecutionBinding) -> Self {
        Self::InProcessV1(binding)
    }
}

impl SchemaExecutionBinding {
    pub fn schema_profile(self) -> FormatProfile {
        match self {
            Self::Worker(binding) => binding.schema_profile,
            Self::InProcessV1(binding) => binding.schema_profile,
        }
    }

    pub fn schema_set_sha256(self) -> Digest256 {
        match self {
            Self::Worker(binding) => binding.schema_set_sha256,
            Self::InProcessV1(binding) => binding.schema_set_sha256,
        }
    }
}
