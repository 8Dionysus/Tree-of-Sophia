//! Persisted mechanical completion binding. This data is not an admission token.
use super::work::invalid;
use crate::SourceMembershipV1;
use std::io;
use tos_foundation::{CutPreparedSchemaExecutionBinding, Digest256};
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
    prepared_schema: CutPreparedSchemaExecutionBinding,
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
        prepared_schema: CutPreparedSchemaExecutionBinding,
        identity_count: u64,
        dependency_source_count: u64,
        dependency_count: u64,
    ) -> io::Result<Self> {
        if dependency_source_count > dependency_count {
            return Err(invalid("native completion root counts are inconsistent"));
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

    pub fn prepared_schema(self) -> CutPreparedSchemaExecutionBinding {
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

    /// Copy the exact payload fields needed by the V2 rootset codec. Keeping
    /// the tuple conversion here lets the sibling segment codec serialize the
    /// proof without reaching through its private fields.
    pub fn authenticated_root_fields(
        self,
    ) -> (
        Digest256,
        Option<SourceMembershipV1>,
        u64,
        CutPreparedSchemaExecutionBinding,
        u64,
        u64,
        u64,
    ) {
        (
            self.validator_sha256,
            self.membership_v1,
            self.source_bytes,
            self.prepared_schema,
            self.identity_count,
            self.dependency_source_count,
            self.dependency_count,
        )
    }
}
