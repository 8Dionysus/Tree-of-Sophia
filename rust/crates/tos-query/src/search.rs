//! Shared query types and Worker search normalization.

use tos_foundation::{Digest256, python_lower_unicode16_v1, python_strip_unicode16_v1};

pub const QUERY_PRIMITIVE_PROFILE: &str = "tos-query-primitives-v1";
pub const SEARCH_UNICODE_PROFILE: &str = "tos-python-native-unicode-v1";
pub const SEARCH_QUERY_MAX_CODE_POINTS: usize = 256;
pub const SEARCH_QUERY_MAX_UTF8_BYTES: usize = SEARCH_QUERY_MAX_CODE_POINTS * 4;
pub const SEARCH_QUERY_MIN_CODE_POINTS: usize = 3;
/// Exact authored descriptor identity. The selected source registration list
/// and CMP's sealed source scopes are checked separately; no second
/// vocabulary membership root is invented.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryVocabularyBinding {
    pub descriptor_sha256: Digest256,
    pub descriptor_version: u64,
}

/// Membership view for one selected descriptor. Implementations must be
/// backed by that exact descriptor and must not infer IDs from graph rows.
pub trait SelectedQueryVocabulary {
    fn binding(&self) -> &QueryVocabularyBinding;
    /// Exact authored source registration in strict ID order, including
    /// registered sources with zero selected rows. Derived source_scope rows
    /// cannot create registration.
    fn registered_source_ids(&self) -> &[String];
}

/// The immutable selection fields that determine this semantic search view.
/// Roots are copied from the CMP-selected envelope plus QRY's trusted-local
/// index-file digest. The producer/admission route must establish completeness;
/// `complete == true` alone is not evidence that registered inputs were not
/// omitted. This value is internal state, not a public token ABI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchSelectionBinding {
    pub model_abi: String,
    pub vocabulary: QueryVocabularyBinding,
    /// Exact authored QueryVocabulary semantic-primitive family.
    pub semantic_primitive_profile: String,
    /// CMP search-index lower/3-gram producer profile, independent of the
    /// broader authored query-primitive family.
    pub search_unicode_profile: String,
    pub source_cut: String,
    pub through_commit_seq: u64,
    /// CMD/source publication membership, distinct from vocabulary membership.
    pub source_membership_root: Digest256,
    /// Absent only when the selected producer has no history-root capability.
    pub history_root_sha256: Option<Digest256>,
    pub entity_registry_id: String,
    pub entity_registry_version: String,
    pub entity_registry_sha256: Digest256,
    pub relation_registry_id: String,
    pub relation_registry_version: String,
    pub relation_registry_sha256: Digest256,
    pub graph_root_sha256: Digest256,
    /// Exact selected catalog carrier, distinct from its indexed facet/routes.
    pub catalog_packet_sha256: Digest256,
    pub catalog_index_root_sha256: Digest256,
    pub source_scope_root_sha256: Digest256,
    pub search_index_root_sha256: Digest256,
    pub index_root_sha256: Digest256,
    pub index_generation: String,
    pub route_map_version: String,
    pub reader_abi: String,
    pub complete: bool,
}

/// Opaque current-policy identity issued by the source owner. This is not a
/// rights grant; callers must obtain/recheck it through that owner on every
/// page and bind it to continuation state. Empty values fail closed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentPolicyBinding {
    pub scope: String,
    pub issuer_ref: String,
    pub authorization_receipt_id: String,
    pub policy_epoch: String,
    pub withdrawal_generation: String,
}

fn normalize_query(raw: &str) -> Result<String, SearchError> {
    // Any valid UTF-8 string over this byte ceiling necessarily contains more
    // than 256 Unicode scalar values. Reject it before a linear character
    // count; the FND primitive repeats the exact code-point guard.
    if raw.len() > SEARCH_QUERY_MAX_UTF8_BYTES {
        return Err(SearchError::new(
            SearchErrorCode::QueryTooLong,
            "indexed search v2 query exceeds 256 code points",
        ));
    }
    let stripped = python_strip_unicode16_v1(raw, SEARCH_QUERY_MAX_CODE_POINTS).map_err(|_| {
        SearchError::new(
            SearchErrorCode::QueryTooLong,
            "indexed search v2 query exceeds 256 code points",
        )
    })?;
    let normalized = python_lower_unicode16_v1(
        stripped,
        SEARCH_QUERY_MAX_CODE_POINTS,
        SEARCH_QUERY_MAX_CODE_POINTS,
        SEARCH_QUERY_MAX_UTF8_BYTES,
    )
    .map_err(|_| {
        SearchError::new(
            SearchErrorCode::QueryTooLong,
            "indexed search v2 normalized query exceeds 256 code points",
        )
    })?;
    if normalized.chars().count() < SEARCH_QUERY_MIN_CODE_POINTS {
        return Err(SearchError::new(
            SearchErrorCode::QueryTooShort,
            "indexed search v2 requires at least 3 normalized code points",
        ));
    }
    Ok(normalized)
}

/// Query-normalization rule used by the bounded Worker/D1 access adapter. Legacy mode deliberately keeps its
/// historical empty/short-query behavior while using the same Unicode-16
/// strip/lower primitives and 256-code-point post-strip ceiling.
pub fn normalize_worker_search_query(
    raw: &str,
    indexed: bool,
) -> Result<(String, String), SearchError> {
    let query = if indexed {
        normalize_query(raw)?
    } else {
        // The legacy adapter historically allowed surrounding whitespace to
        // exceed the retained query ceiling; only the stripped value is
        // bounded. The incoming HTTP envelope itself is bounded by the owner.
        let stripped = python_strip_unicode16_v1(raw, raw.chars().count()).map_err(|_| {
            SearchError::new(
                SearchErrorCode::QueryTooLong,
                "knowledge search query exceeds 256 characters",
            )
        })?;
        if stripped.chars().count() > SEARCH_QUERY_MAX_CODE_POINTS {
            return Err(SearchError::new(
                SearchErrorCode::QueryTooLong,
                "knowledge search query exceeds 256 characters",
            ));
        }
        stripped.to_owned()
    };
    let needle = python_lower_unicode16_v1(
        &query,
        SEARCH_QUERY_MAX_CODE_POINTS,
        if indexed {
            SEARCH_QUERY_MAX_CODE_POINTS
        } else {
            2 * SEARCH_QUERY_MAX_CODE_POINTS
        },
        if indexed {
            SEARCH_QUERY_MAX_UTF8_BYTES
        } else {
            2 * SEARCH_QUERY_MAX_UTF8_BYTES
        },
    )
    .map_err(|_| {
        SearchError::new(
            SearchErrorCode::QueryTooLong,
            if indexed {
                "indexed search v2 normalized query exceeds 256 code points"
            } else {
                "knowledge search query exceeds 256 characters"
            },
        )
    })?;
    if indexed && needle.chars().count() < SEARCH_QUERY_MIN_CODE_POINTS {
        return Err(SearchError::new(
            SearchErrorCode::QueryTooShort,
            "indexed search v2 requires at least 3 normalized code points",
        ));
    }
    Ok((query, needle))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchErrorCode {
    Cancelled,
    DeadlineExceeded,
    InvalidRequest,
    UnknownIdentifier,
    QueryTooLong,
    QueryTooShort,
    SelectionIncomplete,
    UnsupportedProfile,
    UnsupportedModel,
    StaleSelection,
    StaleContinuation,
    /// Only an adapter whose versioned wire token has an expiry may emit it.
    CursorExpired,
    PolicyBindingUnavailable,
    StalePolicy,
    BudgetExceeded,
    IndexIncomplete,
    CorruptSelectedCarrier,
    Unavailable,
    NonMonotoneProgress,
    MissingProgress,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SearchError {
    pub code: SearchErrorCode,
    pub message: &'static str,
}

impl SearchError {
    pub(crate) const fn new(code: SearchErrorCode, message: &'static str) -> Self {
        Self { code, message }
    }
}

impl std::fmt::Display for SearchError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for SearchError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum SearchRank {
    ExactIdentity,
    IdentityPrefix,
    VisibleDisplaySubstring,
    OtherSerializedCarrierSubstring,
}

impl SearchRank {
    /// Stable numeric rank classes shared by native search, D1 ordering, and
    /// continuation validation.
    pub const CLASS_COUNT: u8 = 4;

    pub const fn class(self) -> u8 {
        match self {
            Self::ExactIdentity => 0,
            Self::IdentityPrefix => 1,
            Self::VisibleDisplaySubstring => 2,
            Self::OtherSerializedCarrierSubstring => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchKind {
    Nodes,
    Relations,
}
