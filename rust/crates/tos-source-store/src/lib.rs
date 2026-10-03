//! Read-only current-format ToS corpus snapshots.
//!
//! This crate verifies mechanical exactness. Source meaning, rights, admission,
//! publication and current-use authority remain with their owners.

mod archive;
mod cut;
mod error;
mod git_capture;
mod limits;
mod manifest;
mod object;
mod pinned_sqlite;
mod secure_open;
mod software;
mod streamed_cut;

pub use cut::{
    CorpusCutReader, CutReadLimits, RetiredSourceMemberV1, SourceMemberStreamV1, SourceMemberV1,
    SourceMembershipV1, SourcePresenceV1, has_authored_source_descendants_v1,
    is_authored_source_path_v1,
};
pub use error::{Result, StoreError, StoreErrorCode};
pub use limits::ReadLimits;
pub use manifest::{
    CorpusDescriptor, CorpusReader, MemberMetadata, RetirementMetadata, Selector, Snapshot,
};
pub use software::{
    SOFTWARE_COMPANION_PROFILE_V1, SoftwareCaptureReader, SoftwareCaptureSelectionV1,
    SoftwareComponentSelectionV1,
};
pub use streamed_cut::{
    StreamedCorpusCutReaderV1, StreamedCutReadLimitsV1, StreamedRetiredSourceMemberV1,
    StreamedRevisionV1, StreamedSourceMemberStreamV1, StreamedSourceMemberV1,
};

pub use archive::{
    CaptureReadUsage, CaptureRestoreLimits, CaptureVerification, restore_capture, verify_capture,
    verify_capture_with_usage,
};

pub use git_capture::{CaptureGitRequest, CaptureGitResult, GitCaptureLimits, capture_git};

mod source_cut_restore;
pub use source_cut_restore::{SourceCutRestoreResult, restore_source_cut};

pub use git_capture::{GitMemberDescriptor, read_git_member, resolve_git_member};

mod publication;
pub use publication::{MetadataPublicationEpoch, validate_metadata_publication};

mod pinned_sqlite_aux;
pub use pinned_sqlite::PinnedSqliteConnection;
pub use pinned_sqlite_aux::{
    PinnedSqliteAuxLimits, PinnedSqliteAuxRequest, PinnedSqliteAuxScope, PinnedSqliteIoBudget,
    PinnedSqliteIoFailure, PinnedSqliteIoSnapshot, PinnedSqliteSpaceBudget,
    PinnedSqliteSpaceReservation, PinnedSqliteSpaceSnapshot,
};
