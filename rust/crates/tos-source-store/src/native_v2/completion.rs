//! Persisted mechanical completion binding. This data is not an admission token.
use super::work::invalid;
use crate::SourceMembershipV1;
use std::io;
use tos_foundation::{Digest256, IN_PROCESS_SCHEMA_ENGINE_ID, SchemaExecutionBinding};
/// Durable mechanical binding for a current root produced by the native
/// admission route. This records the full V1 membership digest only when the
/// exact ordered membership was actually validated. Incremental V2 successors
/// may omit it; their current membership remains bound by the separate V2
/// authenticated tree commitment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeAdmissionCompletionProofV1 {
    validator_sha256: Digest256,
    membership_v1: Option<SourceMembershipV1>,
    source_bytes: u64,
    prepared_schema: SchemaExecutionBinding,
    identity_count: u64,
    dependency_source_count: u64,
    dependency_count: u64,
}

impl NativeAdmissionCompletionProofV1 {
    /// Reconstitute only a completion binding decoded from an authenticated
    /// current-root tuple. Admission issuance remains with the validated command state.
    pub fn from_authenticated_root_fields(
        validator_sha256: Digest256,
        membership_v1: Option<SourceMembershipV1>,
        source_bytes: u64,
        prepared_schema: impl Into<SchemaExecutionBinding>,
        identity_count: u64,
        dependency_source_count: u64,
        dependency_count: u64,
    ) -> io::Result<Self> {
        let prepared_schema = prepared_schema.into();
        if dependency_source_count > dependency_count {
            return Err(invalid("native completion root counts are inconsistent"));
        }
        if matches!(prepared_schema, SchemaExecutionBinding::InProcessV1(binding)
            if binding.engine != IN_PROCESS_SCHEMA_ENGINE_ID)
        {
            return Err(invalid("native completion schema engine differs"));
        }
        Ok(Self {
            validator_sha256,
            membership_v1,
            source_bytes,
            prepared_schema,
            identity_count,
            dependency_source_count,
            dependency_count,
        })
    }

    pub fn validator_sha256(self) -> Digest256 {
        self.validator_sha256
    }

    pub fn membership_v1(self) -> Option<SourceMembershipV1> {
        self.membership_v1
    }

    pub fn source_bytes(self) -> u64 {
        self.source_bytes
    }

    pub fn prepared_schema(self) -> SchemaExecutionBinding {
        self.prepared_schema
    }

    pub fn identity_count(self) -> u64 {
        self.identity_count
    }

    pub fn dependency_source_count(self) -> u64 {
        self.dependency_source_count
    }

    pub fn dependency_count(self) -> u64 {
        self.dependency_count
    }
}
