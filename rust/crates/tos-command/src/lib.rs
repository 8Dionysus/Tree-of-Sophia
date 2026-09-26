//! Laboratory command coordinator for synthetic source records.
//!
//! This crate does not grant ToS source admission, rights, publication, or
//! immutable-byte durability. `SyntheticByteRef` is deliberately not an STO
//! locator. A real owner protocol must replace the lab-only markers.

mod postgres_adapter;

use std::fmt;
use tos_foundation::{Digest256, Digest256Hasher};

pub use postgres_adapter::{Cut, PgCoordinator, Timing};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntheticByteRef {
    pub digest: Digest256,
    pub length: u64,
    pub lab_marker: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PredicateToken {
    pub kind: PredicateKind,
    pub owner: String,
    pub scope: String,
    pub token: String,
    pub definition_version: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PredicateKind {
    Unique,
    Range,
    Prefix,
    ReverseRefs,
    Interval,
}

impl PredicateKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unique => "unique",
            Self::Range => "range",
            Self::Prefix => "prefix",
            Self::ReverseRefs => "reverse",
            Self::Interval => "interval",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PredicateRead {
    Exact {
        namespace: String,
        key: String,
        expected_version: Option<u64>,
        expected_digest: Option<Digest256>,
    },
    Absent {
        namespace: String,
        key: String,
    },
    Generation {
        predicate: PredicateToken,
        observed_generation: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordWrite {
    pub namespace: String,
    pub key: String,
    pub expected_version: Option<u64>,
    pub bytes: SyntheticByteRef,
    /// Owner-declared invalidations. The lab cannot establish their coverage.
    pub invalidates: Vec<PredicateToken>,
}

/// Lab-only exact binding of proposed synthetic writes and their declared
/// invalidations. This is not a source command canonicalization profile.
pub fn synthetic_delta_digest(writes: &[RecordWrite]) -> Digest256 {
    fn part(hasher: &mut Digest256Hasher, bytes: &[u8]) {
        hasher.update(&(bytes.len() as u64).to_be_bytes());
        hasher.update(bytes);
    }
    let mut hasher = Digest256Hasher::new();
    part(&mut hasher, &(writes.len() as u64).to_be_bytes());
    for write in writes {
        part(&mut hasher, write.namespace.as_bytes());
        part(&mut hasher, write.key.as_bytes());
        match write.expected_version {
            Some(version) => {
                part(&mut hasher, b"present");
                part(&mut hasher, &version.to_be_bytes());
            }
            None => part(&mut hasher, b"absent"),
        }
        part(&mut hasher, write.bytes.digest.as_bytes());
        part(&mut hasher, &write.bytes.length.to_be_bytes());
        part(&mut hasher, write.bytes.lab_marker.as_bytes());
        part(&mut hasher, &(write.invalidates.len() as u64).to_be_bytes());
        for token in &write.invalidates {
            part(&mut hasher, token.kind.as_str().as_bytes());
            part(&mut hasher, token.owner.as_bytes());
            part(&mut hasher, token.scope.as_bytes());
            part(&mut hasher, token.token.as_bytes());
            part(&mut hasher, token.definition_version.as_bytes());
        }
    }
    hasher.finalize()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitValidationAttestation {
    pub prepare_base_revision: Digest256,
    pub prepare_overlay_id: String,
    pub prepare_delta_digest: Digest256,
    pub trace_digest: Digest256,
    pub checked_predicates_digest: Digest256,
    pub checked_rule_versions_digest: Digest256,
    pub owner_fences_digest: Digest256,
    pub schema_profile_id: String,
    pub schema_backend_digest: Digest256,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthorityFence {
    /// Exact source-local authority row version, checked under coordinator lock.
    Local { expected_version: u64 },
    /// Explicitly unsupported until the owner supplies a commit fence.
    ExternalUnsupported { owner: String, scope: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Candidate {
    pub domain: String,
    pub command_id: String,
    pub raw_request_digest: Digest256,
    pub input_profile_id: String,
    pub delta_digest: Digest256,
    pub reads: Vec<PredicateRead>,
    pub writes: Vec<RecordWrite>,
    pub authority: AuthorityFence,
    pub job_id: String,
    pub fence_epoch: u64,
    pub expected_rule_version: u64,
    /// Digest of the named rule/schema/registry/backend contract selected by owner.
    pub expected_contract_digest: Digest256,
    /// FullOnly binds the complete coordinator base. None is lab affected mode.
    pub full_base_seq: Option<u64>,
    pub attestation: CommitValidationAttestation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitReceipt {
    pub domain: String,
    pub command_id: String,
    pub commit_seq: u64,
    pub raw_request_digest: Digest256,
    pub delta_digest: Digest256,
    pub attestation_digest: Digest256,
    pub replayed: bool,
}

#[derive(Debug)]
pub enum Error {
    Database(postgres::Error),
    Conflict(&'static str),
    Refused(&'static str),
    UnsupportedExternalAuthority,
    InvalidInput(&'static str),
    Corrupt(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(f, "database: {error}"),
            Self::Conflict(reason) => write!(f, "conflict: {reason}"),
            Self::Refused(reason) => write!(f, "refused: {reason}"),
            Self::UnsupportedExternalAuthority => write!(f, "external authority fence unsupported"),
            Self::InvalidInput(reason) => write!(f, "invalid input: {reason}"),
            Self::Corrupt(reason) => write!(f, "corrupt coordinator: {reason}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<postgres::Error> for Error {
    fn from(value: postgres::Error) -> Self {
        Self::Database(value)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
