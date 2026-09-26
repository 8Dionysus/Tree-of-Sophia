//! Typed request and semantic continuation state for explicit indexed search v2.
//!
//! This module does not execute a search, scan carriers, or encode a public
//! cursor. The adapter owns request framing, cursor authentication/TTL, and
//! token compatibility. A selected read-model backend owns bounded indexed
//! candidate and carrier verification; this module supplies its normalized
//! query and typed per-kind keyset state.

use std::collections::BTreeSet;

use tos_foundation::{Digest256, python_lower_unicode16_v1, python_strip_unicode16_v1};

pub const INDEXED_SEARCH_V2_OPERATION: &str = "tos_knowledge_search_indexed_v2";
pub const SEARCH_READ_MODEL_ABI_V1: &str = "tos_knowledge_read_model_v1";
pub const SEARCH_READ_MODEL_ABI_V2: &str = "tos_knowledge_read_model_v2";
pub const QUERY_PRIMITIVE_PROFILE: &str = "tos-query-primitives-v1";
pub const SEARCH_UNICODE_PROFILE: &str = "tos-python-native-unicode-v1";
pub const SEARCH_QUERY_MAX_CODE_POINTS: usize = 256;
pub const SEARCH_QUERY_MAX_UTF8_BYTES: usize = SEARCH_QUERY_MAX_CODE_POINTS * 4;
pub const SEARCH_QUERY_MIN_CODE_POINTS: usize = 3;
pub const SEARCH_V2_MAX_PAGE_SIZE: usize = 100;
pub const SEARCH_V2_MAX_FILTER_VALUES: usize = 100;
pub const SEARCH_V2_MAX_FILTER_VALUE_CODE_POINTS: usize = 256;
pub const SEARCH_V2_MAX_FILTER_VALUE_UTF8_BYTES: usize = 4 * SEARCH_V2_MAX_FILTER_VALUE_CODE_POINTS;

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

/// Explicit indexed-v2 input after the transport has decoded its fields.
/// Empty filters mean unrestricted membership; v2 has no offset field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexedSearchV2Request {
    pub query: String,
    pub sources: Vec<String>,
    pub kind_ids: Vec<String>,
    pub predicate_ids: Vec<String>,
    pub limit: usize,
}

/// Canonical semantic request consumed by a bounded indexed backend.
/// `sources == None` means all descriptor-registered sources. Empty kind and
/// predicate vectors likewise mean unrestricted membership for that field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedIndexedSearchV2Request {
    query: String,
    sources: Option<Vec<String>>,
    kind_ids: Vec<String>,
    predicate_ids: Vec<String>,
    limit: usize,
}

impl NormalizedIndexedSearchV2Request {
    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn sources(&self) -> Option<&[String]> {
        self.sources.as_deref()
    }

    pub fn kind_ids(&self) -> &[String] {
        &self.kind_ids
    }

    pub fn predicate_ids(&self) -> &[String] {
        &self.predicate_ids
    }

    pub fn limit(&self) -> usize {
        self.limit
    }
}

impl IndexedSearchV2Request {
    /// Normalize only the explicitly selected v2 operation. Empty or
    /// omitted filters normalize to the same unrestricted value. Query
    /// normalization is pinned to FND's Python/Unicode-16 primitives.
    pub fn normalize<V: SelectedQueryVocabulary + ?Sized>(
        self,
        selection: &SearchSelectionBinding,
        vocabulary: &V,
    ) -> Result<NormalizedIndexedSearchV2Request, SearchV2Error> {
        validate_selection(&selection, vocabulary)?;
        if !(1..=SEARCH_V2_MAX_PAGE_SIZE).contains(&self.limit) {
            return Err(SearchV2Error::new(
                SearchV2ErrorCode::InvalidRequest,
                "indexed search v2 limit must be between 1 and 100",
            ));
        }
        let filter_values = self
            .sources
            .len()
            .checked_add(self.kind_ids.len())
            .and_then(|count| count.checked_add(self.predicate_ids.len()))
            .ok_or_else(|| {
                SearchV2Error::new(
                    SearchV2ErrorCode::InvalidRequest,
                    "indexed search v2 filter count overflow",
                )
            })?;
        if filter_values > SEARCH_V2_MAX_FILTER_VALUES {
            return Err(SearchV2Error::new(
                SearchV2ErrorCode::InvalidRequest,
                "indexed search v2 filters exceed 100 values",
            ));
        }

        let query = normalize_query(&self.query)?;
        let sources = canonical_filter(
            self.sources,
            |id| {
                vocabulary
                    .registered_source_ids()
                    .binary_search_by(|registered| registered.as_str().cmp(id))
                    .is_ok()
            },
            "source",
        )?;
        // The authored vocabulary does not enumerate all possible owner
        // kind/predicate literals. Exact filters may match zero selected rows.
        let kind_ids = canonical_filter(self.kind_ids, |_| true, "kind")?.unwrap_or_default();
        let predicate_ids =
            canonical_filter(self.predicate_ids, |_| true, "predicate")?.unwrap_or_default();

        Ok(NormalizedIndexedSearchV2Request {
            query,
            sources,
            kind_ids,
            predicate_ids,
            limit: self.limit,
        })
    }
}

fn normalize_query(raw: &str) -> Result<String, SearchV2Error> {
    // Any valid UTF-8 string over this byte ceiling necessarily contains more
    // than 256 Unicode scalar values. Reject it before a linear character
    // count; the FND primitive repeats the exact code-point guard.
    if raw.len() > SEARCH_QUERY_MAX_UTF8_BYTES {
        return Err(SearchV2Error::new(
            SearchV2ErrorCode::QueryTooLong,
            "indexed search v2 query exceeds 256 code points",
        ));
    }
    let stripped = python_strip_unicode16_v1(raw, SEARCH_QUERY_MAX_CODE_POINTS).map_err(|_| {
        SearchV2Error::new(
            SearchV2ErrorCode::QueryTooLong,
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
        SearchV2Error::new(
            SearchV2ErrorCode::QueryTooLong,
            "indexed search v2 normalized query exceeds 256 code points",
        )
    })?;
    if normalized.chars().count() < SEARCH_QUERY_MIN_CODE_POINTS {
        return Err(SearchV2Error::new(
            SearchV2ErrorCode::QueryTooShort,
            "indexed search v2 requires at least 3 normalized code points",
        ));
    }
    Ok(normalized)
}

fn canonical_filter(
    values: Vec<String>,
    contains: impl Fn(&str) -> bool,
    field: &'static str,
) -> Result<Option<Vec<String>>, SearchV2Error> {
    let mut canonical = BTreeSet::new();
    for value in values {
        if value.is_empty()
            || value.len() > SEARCH_V2_MAX_FILTER_VALUE_UTF8_BYTES
            || value.chars().count() > SEARCH_V2_MAX_FILTER_VALUE_CODE_POINTS
            || !contains(&value)
        {
            return Err(SearchV2Error::new(
                SearchV2ErrorCode::InvalidRequest,
                match field {
                    "source" => "indexed search v2 source filter is not in the selected vocabulary",
                    "kind" => "indexed search v2 kind filter is empty or overlong",
                    _ => "indexed search v2 predicate filter is empty or overlong",
                },
            ));
        }
        canonical.insert(value);
    }
    if canonical.is_empty() {
        Ok(None)
    } else {
        Ok(Some(canonical.into_iter().collect()))
    }
}

fn validate_selection<V: SelectedQueryVocabulary + ?Sized>(
    selection: &SearchSelectionBinding,
    vocabulary: &V,
) -> Result<(), SearchV2Error> {
    if !selection.complete {
        return Err(SearchV2Error::new(
            SearchV2ErrorCode::SelectionIncomplete,
            "selected knowledge search model is incomplete",
        ));
    }
    if selection.semantic_primitive_profile != QUERY_PRIMITIVE_PROFILE
        || selection.search_unicode_profile != SEARCH_UNICODE_PROFILE
    {
        return Err(SearchV2Error::new(
            SearchV2ErrorCode::UnsupportedProfile,
            "selected knowledge search model uses an unsupported primitive profile",
        ));
    }
    if selection.model_abi != SEARCH_READ_MODEL_ABI_V1
        && selection.model_abi != SEARCH_READ_MODEL_ABI_V2
    {
        return Err(SearchV2Error::new(
            SearchV2ErrorCode::UnsupportedModel,
            "selected knowledge search model ABI is unsupported",
        ));
    }
    if selection.source_cut.is_empty()
        || selection.entity_registry_id.is_empty()
        || selection.entity_registry_version.is_empty()
        || selection.relation_registry_id.is_empty()
        || selection.relation_registry_version.is_empty()
        || selection.index_generation.is_empty()
        || selection.route_map_version.is_empty()
        || selection.reader_abi.is_empty()
        || selection.vocabulary.descriptor_version == 0
        || &selection.vocabulary != vocabulary.binding()
        || vocabulary
            .registered_source_ids()
            .iter()
            .any(|id| id.is_empty())
        || vocabulary
            .registered_source_ids()
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(SearchV2Error::new(
            SearchV2ErrorCode::StaleSelection,
            "selected search model and vocabulary binding do not match",
        ));
    }
    Ok(())
}

fn validate_policy_binding(policy: &CurrentPolicyBinding) -> Result<(), SearchV2Error> {
    if policy.scope.is_empty()
        || policy.issuer_ref.is_empty()
        || policy.authorization_receipt_id.is_empty()
        || policy.policy_epoch.is_empty()
        || policy.withdrawal_generation.is_empty()
    {
        return Err(SearchV2Error::new(
            SearchV2ErrorCode::PolicyBindingUnavailable,
            "current owner policy binding is unavailable",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchV2ErrorCode {
    InvalidRequest,
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
pub struct SearchV2Error {
    pub code: SearchV2ErrorCode,
    pub message: &'static str,
}

impl SearchV2Error {
    pub(crate) const fn new(code: SearchV2ErrorCode, message: &'static str) -> Self {
        Self { code, message }
    }
}

impl std::fmt::Display for SearchV2Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for SearchV2Error {}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum SearchRank {
    ExactIdentity,
    IdentityPrefix,
    VisibleDisplaySubstring,
    OtherSerializedCarrierSubstring,
}

/// The stable per-kind index order key. The executor must build `lower_id`
/// from the selected `search_documents.id_lower` and verify its relation to
/// the bounded source carrier before advancing this state. This constructor
/// does not verify index rows or carrier bytes. `source_position` is the exact
/// nonnegative CMP `source_order` position (restricted to SQLite's i63 ABI).
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct SearchOrderKey {
    rank: SearchRank,
    lower_id: String,
    source_position: u64,
}

impl SearchOrderKey {
    pub fn new(
        rank: SearchRank,
        lower_id: String,
        source_position: u64,
    ) -> Result<Self, SearchV2Error> {
        if lower_id.is_empty() || source_position > i64::MAX as u64 {
            return Err(SearchV2Error::new(
                SearchV2ErrorCode::InvalidRequest,
                "invalid indexed search order key",
            ));
        }
        Ok(Self {
            rank,
            lower_id,
            source_position,
        })
    }

    pub fn rank(&self) -> SearchRank {
        self.rank
    }

    pub fn lower_id(&self) -> &str {
        &self.lower_id
    }

    pub fn source_position(&self) -> u64 {
        self.source_position
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchKind {
    Nodes,
    Relations,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct KindContinuation {
    after: Option<SearchOrderKey>,
    exhausted: bool,
}

impl KindContinuation {
    fn new() -> Self {
        Self {
            after: None,
            exhausted: false,
        }
    }
}

/// Typed semantic continuation state; deliberately has no token encoding,
/// expiry, MAC, or authorization semantics. Adapter continuation contracts
/// remain version- and backend-specific.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchContinuationState {
    selection: SearchSelectionBinding,
    current_policy: CurrentPolicyBinding,
    request: NormalizedIndexedSearchV2Request,
    nodes: KindContinuation,
    relations: KindContinuation,
}

impl SearchContinuationState {
    pub fn new<V: SelectedQueryVocabulary + ?Sized>(
        selection: SearchSelectionBinding,
        request: NormalizedIndexedSearchV2Request,
        current_policy: CurrentPolicyBinding,
        vocabulary: &V,
    ) -> Result<Self, SearchV2Error> {
        validate_selection(&selection, vocabulary)?;
        validate_policy_binding(&current_policy)?;
        Ok(Self {
            selection,
            current_policy,
            request,
            nodes: KindContinuation::new(),
            relations: KindContinuation::new(),
        })
    }

    pub fn request(&self) -> &NormalizedIndexedSearchV2Request {
        &self.request
    }

    pub fn selection(&self) -> &SearchSelectionBinding {
        &self.selection
    }

    pub fn current_policy(&self) -> &CurrentPolicyBinding {
        &self.current_policy
    }

    pub fn after(&self, kind: SearchKind) -> Option<&SearchOrderKey> {
        self.progress(kind).after.as_ref()
    }

    pub fn is_exhausted(&self, kind: SearchKind) -> bool {
        self.progress(kind).exhausted
    }

    /// Commit a completely verified per-kind ranked page. For a nonterminal
    /// page, the key must be that of the last returned hit; using a later
    /// examined false positive would skip unseen ranked results. Advancing
    /// requires a strict increase so an empty page cannot loop forever.
    pub fn advance(
        &mut self,
        kind: SearchKind,
        last_returned: Option<SearchOrderKey>,
        exhausted: bool,
    ) -> Result<(), SearchV2Error> {
        let current = self.progress_mut(kind);
        if current.exhausted {
            return Err(SearchV2Error::new(
                SearchV2ErrorCode::StaleContinuation,
                "indexed search kind is already exhausted",
            ));
        }
        if exhausted {
            if last_returned.is_some() {
                return Err(SearchV2Error::new(
                    SearchV2ErrorCode::InvalidRequest,
                    "exhausted indexed search page cannot retain a continuation key",
                ));
            }
            current.after = None;
            current.exhausted = true;
            return Ok(());
        }
        let Some(next) = last_returned else {
            return Err(SearchV2Error::new(
                SearchV2ErrorCode::MissingProgress,
                "nonterminal indexed search page must advance its order key",
            ));
        };
        if current
            .after
            .as_ref()
            .is_some_and(|previous| next <= *previous)
        {
            return Err(SearchV2Error::new(
                SearchV2ErrorCode::NonMonotoneProgress,
                "indexed search continuation order key did not advance",
            ));
        }
        current.after = Some(next);
        Ok(())
    }

    /// Refuse continuation if the query, filters, selected descriptor, any
    /// bound root, or semantic profile changed since the previous page.
    pub fn validate_resume<V: SelectedQueryVocabulary + ?Sized>(
        &self,
        request: &NormalizedIndexedSearchV2Request,
        selection: &SearchSelectionBinding,
        current_policy: &CurrentPolicyBinding,
        vocabulary: &V,
    ) -> Result<(), SearchV2Error> {
        validate_selection(selection, vocabulary)?;
        validate_policy_binding(current_policy)?;
        if selection != &self.selection {
            return Err(SearchV2Error::new(
                SearchV2ErrorCode::StaleSelection,
                "indexed search selection changed during continuation",
            ));
        }
        if current_policy != &self.current_policy {
            return Err(SearchV2Error::new(
                SearchV2ErrorCode::StalePolicy,
                "current owner policy binding changed during continuation",
            ));
        }
        if request != &self.request {
            return Err(SearchV2Error::new(
                SearchV2ErrorCode::StaleContinuation,
                "indexed search request changed during continuation",
            ));
        }
        Ok(())
    }

    fn progress(&self, kind: SearchKind) -> &KindContinuation {
        match kind {
            SearchKind::Nodes => &self.nodes,
            SearchKind::Relations => &self.relations,
        }
    }

    fn progress_mut(&mut self, kind: SearchKind) -> &mut KindContinuation {
        match kind {
            SearchKind::Nodes => &mut self.nodes,
            SearchKind::Relations => &mut self.relations,
        }
    }
}
