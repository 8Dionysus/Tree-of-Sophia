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

mod segment_index_read;
mod segment_index_walk;
mod segment_locator;
mod segment_member;
mod segment_object;
pub use segment_index_read::{
    SegmentIndexReadLimitsV1, SegmentIndexReaderV1, SegmentLocatedValueV1,
};
pub use segment_index_walk::SegmentIndexPageVisitV1;
pub use segment_locator::{
    DecodedSegmentIndexPageV1, SEGMENT_INDEX_KEY_MAX_BYTES_V1, SEGMENT_INDEX_PAGE_HEADER_BYTES_V1,
    SEGMENT_INDEX_PAGE_MAX_BYTES_V1, SegmentExtentV1, SegmentIndexEntryV1,
    SegmentIndexInternalEntryV1, SegmentIndexKeySpaceV1, SegmentIndexLeafEntryV1,
    SegmentIndexPageKindV1, SegmentIndexPageLimitsV1, SegmentIndexRootV1, SegmentIndexSummaryV1,
    SegmentLocatorRootV1, decode_segment_index_page,
};
pub use segment_member::{
    SEGMENT_MEMBER_VALUE_BYTES_V1, SegmentMemberMetadataV1, SegmentMemberReaderV1,
    decode_segment_member_value,
};

mod segment_index_read_v2;
mod segment_index_walk_v2;
mod segment_locator_v2;
mod segment_member_v2;
pub use segment_index_read_v2::{
    SegmentIndexClosureV2, SegmentIndexMutationPathReceiptV2, SegmentIndexMutationPathVisitV2,
    SegmentIndexMutationSelectionV2, SegmentIndexPageVisitV2, SegmentIndexReadLimitsV2,
    SegmentIndexReaderV2, SegmentLocatedValueV2,
};
pub use segment_index_walk_v2::{SegmentIndexIntervalClosureV2, SegmentIndexRetainedClosureV2};
pub use segment_locator_v2::{
    DecodedSegmentIndexPageV2, SEGMENT_INDEX_KEY_MAX_BYTES_V2, SEGMENT_INDEX_PAGE_HEADER_BYTES_V2,
    SEGMENT_INDEX_PAGE_MAX_BYTES_V2, SegmentIndexEntryV2, SegmentIndexInternalEntryV2,
    SegmentIndexLeafEntryV2, SegmentIndexLogicalSummaryV2, SegmentIndexPageLimitsV2,
    SegmentIndexRootV2, decode_segment_index_page_v2,
};
pub use segment_member_v2::SegmentMemberReaderV2;

mod segment_subtree_memo_v2;
pub use segment_subtree_memo_v2::{SegmentSubtreeMemoLimitsV2, SegmentSubtreeMemoV2};
