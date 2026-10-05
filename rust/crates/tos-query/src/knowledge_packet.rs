//! Native indexed-v2 packet assembly from one pinned CMP knowledge model.
//! A packet is transportable only while its source-owner disclosure hold lives.

use std::ops::Deref;

use tos_compiler::VerifiedKnowledgeModel;
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonMode, JsonNumber, JsonNumberKind, JsonString,
    JsonValue, canonical_bytes_v1, parse_json,
};

use crate::knowledge_binding::BoundCmpKnowledge;
use crate::search_candidate::SelectedSearchCandidate;
use crate::search_execute::{
    ObservedSearchCandidate, PrivateSearchKindPage, SearchCurrentAuthority, SearchKindBudget,
    advance_private_kind, execute_private_kind_page, exhausted_private_kind,
};
use crate::search_v2::{
    CurrentPolicyBinding, INDEXED_SEARCH_V2_OPERATION, IndexedSearchV2Request,
    SearchContinuationState, SearchKind, SearchV2Error, SearchV2ErrorCode, SelectedQueryVocabulary,
};

pub const INDEXED_SEARCH_OPERATION_ID: &str = "tos.knowledge.search";
pub const INDEXED_SEARCH_CARRIER_LAYER: &str = "tos_knowledge_public_graph_projection_v1";
pub const INDEXED_SEARCH_INTENDED_USE: &str = "read_only_public_knowledge_search_v1";

fn error(code: SearchV2ErrorCode, message: &'static str) -> SearchV2Error {
    SearchV2Error { code, message }
}

/// Owner-issued public-projection scope. Selected model custody/receipt and
/// current publication/withdrawal policy are separate facts; neither a graph
/// digest nor an Item payload rights ref grants this metadata search use.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexedDisclosureScope {
    pub operation_id: String,
    pub carrier_layer: String,
    pub intended_use: String,
    pub selected_model_receipt_id: String,
    pub source_cut: String,
    pub through_commit_seq: u64,
    pub source_membership_root: Digest256,
    pub descriptor_sha256: Digest256,
    pub selected_index_sha256: Digest256,
    pub policy_issuer_ref: String,
    pub policy_receipt_id: String,
    pub policy_scope: String,
    pub policy_epoch: String,
    pub withdrawal_generation: String,
}

impl IndexedDisclosureScope {
    pub(crate) fn validate_for(
        &self,
        bound: &BoundCmpKnowledge<'_>,
        policy: &CurrentPolicyBinding,
        operation_id: &str,
        intended_use: &str,
    ) -> Result<(), SearchV2Error> {
        if self.operation_id != operation_id
            || self.carrier_layer != INDEXED_SEARCH_CARRIER_LAYER
            || self.intended_use != intended_use
            || self.selected_model_receipt_id != bound.owner_receipt_id()
            || self.source_cut != bound.selection().source_cut
            || self.through_commit_seq != bound.selection().through_commit_seq
            || self.source_membership_root != bound.selection().source_membership_root
            || self.descriptor_sha256 != bound.selection().vocabulary.descriptor_sha256
            || self.selected_index_sha256 != bound.selection().index_root_sha256
            || self.policy_scope != policy.scope
            || self.policy_issuer_ref != policy.issuer_ref
            || self.policy_receipt_id != policy.authorization_receipt_id
            || self.policy_epoch != policy.policy_epoch
            || self.withdrawal_generation != policy.withdrawal_generation
            || policy.scope.is_empty()
            || policy.issuer_ref.is_empty()
            || policy.authorization_receipt_id.is_empty()
            || policy.policy_epoch.is_empty()
        {
            return Err(error(
                SearchV2ErrorCode::PolicyBindingUnavailable,
                "indexed public-projection disclosure scope unavailable",
            ));
        }
        Ok(())
    }
}

/// This is a held owner fence, not a one-time rights observation. It must
/// serialize withdrawal through the final synchronous transport flush.
pub trait IndexedDisclosureLease: Send {
    fn recheck(&mut self) -> Result<(), SearchV2Error>;
}

/// The exact ToS publication/visibility owner supplies these callbacks.
/// No fixture callback or CMP selected receipt itself can activate a public
/// route. Every consulted selected carrier, including filtered and false-
/// positive rows, must be covered by the returned hold.
pub trait IndexedKnowledgeAuthority {
    fn policy_binding(&self) -> CurrentPolicyBinding;
    fn disclosure_scope(&self) -> IndexedDisclosureScope;
    fn check_selected(&mut self) -> Result<(), SearchV2Error>;
    fn authorize_current(
        &mut self,
        candidate: &SelectedSearchCandidate,
    ) -> Result<(), SearchV2Error>;
    fn acquire_disclosure(
        &mut self,
        scope: &IndexedDisclosureScope,
        consulted: &[ObservedSearchCandidate],
    ) -> Result<Box<dyn IndexedDisclosureLease>, SearchV2Error>;
}

/// Borrowed owner disclosure contract for a selected invocation whose hold
/// cannot outlive its authenticated source capture. It shares the static
/// route's kernel and all currentness, scope and budget checks.
pub trait ScopedIndexedKnowledgeAuthority<'hold> {
    fn policy_binding(&self) -> CurrentPolicyBinding;
    fn disclosure_scope(&self) -> IndexedDisclosureScope;
    fn check_selected(&mut self) -> Result<(), SearchV2Error>;
    fn authorize_current(
        &mut self,
        candidate: &SelectedSearchCandidate,
    ) -> Result<(), SearchV2Error>;
    fn acquire_disclosure(
        &mut self,
        scope: &IndexedDisclosureScope,
        consulted: &[ObservedSearchCandidate],
    ) -> Result<Box<dyn IndexedDisclosureLease + 'hold>, SearchV2Error>;
}

// An existing static owner already returns an owned lease, which may safely
// serve any shorter delivery hold without changing its authority or payload.
impl<'hold, A: IndexedKnowledgeAuthority + ?Sized> ScopedIndexedKnowledgeAuthority<'hold> for A {
    fn policy_binding(&self) -> CurrentPolicyBinding {
        IndexedKnowledgeAuthority::policy_binding(self)
    }
    fn disclosure_scope(&self) -> IndexedDisclosureScope {
        IndexedKnowledgeAuthority::disclosure_scope(self)
    }
    fn check_selected(&mut self) -> Result<(), SearchV2Error> {
        IndexedKnowledgeAuthority::check_selected(self)
    }
    fn authorize_current(
        &mut self,
        candidate: &SelectedSearchCandidate,
    ) -> Result<(), SearchV2Error> {
        IndexedKnowledgeAuthority::authorize_current(self, candidate)
    }
    fn acquire_disclosure(
        &mut self,
        scope: &IndexedDisclosureScope,
        consulted: &[ObservedSearchCandidate],
    ) -> Result<Box<dyn IndexedDisclosureLease + 'hold>, SearchV2Error> {
        IndexedKnowledgeAuthority::acquire_disclosure(self, scope, consulted)
    }
}

struct AuthorityAdapter<'a, 'hold, A: ?Sized>(&'a mut A, std::marker::PhantomData<&'hold ()>);
impl<'hold, A: ScopedIndexedKnowledgeAuthority<'hold> + ?Sized> SearchCurrentAuthority
    for AuthorityAdapter<'_, 'hold, A>
{
    fn check_selected(&mut self) -> Result<(), SearchV2Error> {
        self.0.check_selected()
    }
    fn authorize_current(
        &mut self,
        candidate: &SelectedSearchCandidate,
    ) -> Result<(), SearchV2Error> {
        self.0.authorize_current(candidate)
    }
}

/// Wire token semantics remain local to the native adapter. Python's unsigned
/// base64url JSON token with a 15-minute expiry and Worker's epoch-bound token
/// are not advertised as interoperable or as authorization.
pub trait IndexedWireCursorCodec {
    fn decode(&mut self, token: &str) -> Result<SearchContinuationState, SearchV2Error>;
    fn encode(&mut self, state: &SearchContinuationState) -> Result<String, SearchV2Error>;
}

#[derive(Clone, Copy, Debug)]
pub struct IndexedPageBudget {
    pub nodes: SearchKindBudget,
    pub relations: SearchKindBudget,
    /// Actual CMP cold/warm SQLite connection startup VM receipt. File SHA
    /// admission is a separate bounded one-time selected-model operation.
    pub max_open_vm_steps: u64,
    pub max_response_bytes: usize,
    pub max_cursor_bytes: usize,
    pub json: JsonLimits,
}

/// The body is inaccessible as a standalone Vec; the owner hold stays alive
/// while the transport writes/flushed it and is dropped on cancellation.
pub struct DisclosableIndexedSearch {
    body: Vec<u8>,
    lease: Box<dyn IndexedDisclosureLease>,
}

impl std::fmt::Debug for DisclosableIndexedSearch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DisclosableIndexedSearch")
            .field("body_bytes", &self.body.len())
            .finish_non_exhaustive()
    }
}

impl Deref for DisclosableIndexedSearch {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        &self.body
    }
}

impl DisclosableIndexedSearch {
    /// Move authenticated bytes and the disclosure hold into a transport packet.
    pub fn into_parts(self) -> (Vec<u8>, Box<dyn IndexedDisclosureLease>) {
        (self.body, self.lease)
    }
    pub fn recheck(&mut self) -> Result<(), SearchV2Error> {
        self.lease.recheck()
    }
}

/// Packet carrying an actual borrowed disclosure lease through synchronous
/// delivery. Bytes and hold move together; cancellation drops both.
pub struct DisclosableScopedIndexedSearch<'hold> {
    body: Vec<u8>,
    lease: Box<dyn IndexedDisclosureLease + 'hold>,
}

impl std::fmt::Debug for DisclosableScopedIndexedSearch<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DisclosableScopedIndexedSearch")
            .field("body_bytes", &self.body.len())
            .finish_non_exhaustive()
    }
}
impl Deref for DisclosableScopedIndexedSearch<'_> {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        &self.body
    }
}
impl<'hold> DisclosableScopedIndexedSearch<'hold> {
    pub fn into_parts(self) -> (Vec<u8>, Box<dyn IndexedDisclosureLease + 'hold>) {
        (self.body, self.lease)
    }
    pub fn recheck(&mut self) -> Result<(), SearchV2Error> {
        self.lease.recheck()
    }
}

fn key(name: &str) -> JsonString {
    JsonString::from_utf8(name)
}
fn string(value: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(value))
}
fn number(value: u64) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: value.to_string(),
    })
}
fn object(entries: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        entries
            .into_iter()
            .map(|(name, value)| (key(name), value))
            .collect(),
    )
}
fn nullable(value: Option<&str>) -> JsonValue {
    value.map_or(JsonValue::Null, string)
}

fn selected_rows(
    page: &PrivateSearchKindPage,
    limits: JsonLimits,
) -> Result<JsonValue, SearchV2Error> {
    let mut rows = Vec::with_capacity(page.hits.len());
    for hit in &page.hits {
        let mut row_limits = limits;
        row_limits.max_bytes = row_limits.max_bytes.min(hit.payload.len());
        let parsed =
            parse_json(&hit.payload, JsonMode::PublishedStrict, row_limits).map_err(|_| {
                error(
                    SearchV2ErrorCode::CorruptSelectedCarrier,
                    "selected indexed result carrier is invalid",
                )
            })?;
        if parsed.root().as_object().is_none() {
            return Err(error(
                SearchV2ErrorCode::CorruptSelectedCarrier,
                "selected indexed result is not an object",
            ));
        }
        rows.push(parsed.into_root());
    }
    Ok(JsonValue::Array(rows))
}

fn work(page: &PrivateSearchKindPage) -> Result<JsonValue, SearchV2Error> {
    let sql_pages = page
        .gram_charge
        .lookups
        .checked_add(page.posting_charge.lookups)
        .ok_or_else(|| {
            error(
                SearchV2ErrorCode::BudgetExceeded,
                "indexed SQL-page charge overflow",
            )
        })?;
    Ok(object(vec![
        ("candidate_rows", number(page.candidate_charge.rows)),
        ("verified_chars", number(page.verified_chars)),
        ("sql_pages", number(sql_pages)),
    ]))
}

fn packet(
    bound: &BoundCmpKnowledge<'_>,
    raw_query: &str,
    cursor_in: Option<&str>,
    next_cursor: Option<&str>,
    request: &crate::search_v2::NormalizedIndexedSearchV2Request,
    first_page: bool,
    nodes: &PrivateSearchKindPage,
    relations: &PrivateSearchKindPage,
    limits: JsonLimits,
) -> Result<Vec<u8>, SearchV2Error> {
    let mut authority_limits = limits;
    authority_limits.max_bytes = authority_limits
        .max_bytes
        .min(bound.authority_boundary().len());
    let authority = parse_json(
        bound.authority_boundary().as_bytes(),
        JsonMode::PublishedStrict,
        authority_limits,
    )
    .map_err(|_| {
        error(
            SearchV2ErrorCode::CorruptSelectedCarrier,
            "selected authority boundary is invalid",
        )
    })?
    .into_root();
    if authority.as_object().is_none() {
        return Err(error(
            SearchV2ErrorCode::CorruptSelectedCarrier,
            "selected authority boundary is not an object",
        ));
    }
    let sources = request.sources().unwrap_or(bound.registered_source_ids());
    let filters = object(vec![
        (
            "sources",
            JsonValue::Array(sources.iter().map(|s| string(s)).collect()),
        ),
        (
            "kind_ids",
            JsonValue::Array(request.kind_ids().iter().map(|s| string(s)).collect()),
        ),
        (
            "predicate_ids",
            JsonValue::Array(request.predicate_ids().iter().map(|s| string(s)).collect()),
        ),
    ]);
    let no_more = !nodes.has_more && !relations.has_more;
    let exact_count = |page: &PrivateSearchKindPage| {
        if first_page && !page.has_more {
            number(page.matching_total)
        } else {
            JsonValue::Null
        }
    };
    let body = object(vec![
        ("schema", string(INDEXED_SEARCH_V2_OPERATION)),
        ("source_revision", string(bound.require_source_revision()?)),
        ("query", string(raw_query)),
        ("filters", filters),
        (
            "page",
            object(vec![
                ("cursor", nullable(cursor_in)),
                ("next_cursor", nullable(next_cursor)),
                ("limit_per_kind", number(request.limit() as u64)),
                ("ordering_scope", string("global-rank")),
                ("has_more", JsonValue::Bool(!no_more)),
            ]),
        ),
        (
            "counts",
            object(vec![
                ("matching_nodes", exact_count(nodes)),
                ("matching_relations", exact_count(relations)),
                ("returned_nodes", number(nodes.hits.len() as u64)),
                ("returned_relations", number(relations.hits.len() as u64)),
                (
                    "scope",
                    string("exact-if-kind-exhausted-without-continuation"),
                ),
            ]),
        ),
        ("nodes", selected_rows(nodes, limits)?),
        ("relations", selected_rows(relations, limits)?),
        ("authority_boundary", authority),
        (
            "work",
            object(vec![
                ("nodes", work(nodes)?),
                ("relations", work(relations)?),
            ]),
        ),
    ]);
    canonical_bytes_v1(&body, CanonicalProfile::SourceRecordDigestV1, limits).map_err(|_| {
        error(
            SearchV2ErrorCode::BudgetExceeded,
            "indexed response serialization budget exceeded",
        )
    })
}

/// Execute both kinds against the *same* pinned selected model. Any error
/// leaves semantic continuation unchanged and publishes no body. A final
/// owner hold covers all consulted carriers through transport flush.
pub fn execute_indexed_search_page<A: IndexedKnowledgeAuthority + ?Sized>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    cursor_codec: &mut dyn IndexedWireCursorCodec,
    request: IndexedSearchV2Request,
    cursor_in: Option<&str>,
    budget: IndexedPageBudget,
) -> Result<DisclosableIndexedSearch, SearchV2Error> {
    let scoped = execute_scoped_indexed_search_page::<'static, A>(
        model,
        bound,
        authority,
        cursor_codec,
        request,
        cursor_in,
        budget,
    )?;
    let (body, lease) = scoped.into_parts();
    Ok(DisclosableIndexedSearch { body, lease })
}

/// Same indexed page kernel with a borrowed owner-issued delivery hold.
pub fn execute_scoped_indexed_search_page<
    'hold,
    A: ScopedIndexedKnowledgeAuthority<'hold> + ?Sized,
>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    cursor_codec: &mut dyn IndexedWireCursorCodec,
    request: IndexedSearchV2Request,
    cursor_in: Option<&str>,
    budget: IndexedPageBudget,
) -> Result<DisclosableScopedIndexedSearch<'hold>, SearchV2Error> {
    bound.require_source_revision()?;
    let raw_query = request.query.clone();
    if budget.max_open_vm_steps == 0
        || model.open_vm_steps() > budget.max_open_vm_steps
        || budget.max_response_bytes == 0
        || budget.max_cursor_bytes == 0
        || raw_query.len() > budget.max_response_bytes
    {
        return Err(error(
            SearchV2ErrorCode::BudgetExceeded,
            "indexed packet admission unavailable",
        ));
    }
    if cursor_in.is_some_and(|value| value.is_empty() || value.len() > budget.max_cursor_bytes) {
        return Err(error(
            SearchV2ErrorCode::InvalidRequest,
            "indexed cursor is empty or exceeds adapter cap",
        ));
    }
    bound.check_model(model)?;
    let policy = authority.policy_binding();
    let scope = authority.disclosure_scope();
    scope.validate_for(
        bound,
        &policy,
        INDEXED_SEARCH_OPERATION_ID,
        INDEXED_SEARCH_INTENDED_USE,
    )?;
    let normalized = request.normalize(bound.selection(), bound)?;
    let state = match cursor_in {
        Some(token) => cursor_codec.decode(token)?,
        None => SearchContinuationState::new(
            bound.selection().clone(),
            normalized.clone(),
            policy.clone(),
            bound,
        )?,
    };
    state.validate_resume(&normalized, bound.selection(), &policy, bound)?;
    let first_page = cursor_in.is_none();
    if first_page
        && (state.after(SearchKind::Nodes).is_some()
            || state.after(SearchKind::Relations).is_some()
            || state.is_exhausted(SearchKind::Nodes)
            || state.is_exhausted(SearchKind::Relations))
    {
        return Err(error(
            SearchV2ErrorCode::StaleContinuation,
            "indexed first page has continuation state",
        ));
    }
    if !first_page
        && state.after(SearchKind::Nodes).is_none()
        && state.after(SearchKind::Relations).is_none()
        && !state.is_exhausted(SearchKind::Nodes)
        && !state.is_exhausted(SearchKind::Relations)
    {
        return Err(error(
            SearchV2ErrorCode::StaleContinuation,
            "indexed cursor has no prior semantic progress",
        ));
    }
    let mut next_state = state.clone();
    let mut owner = AuthorityAdapter::<'_, 'hold, A>(authority, std::marker::PhantomData);
    let nodes = if next_state.is_exhausted(SearchKind::Nodes) {
        exhausted_private_kind()
    } else {
        execute_private_kind_page(
            model,
            bound,
            &mut owner,
            SearchKind::Nodes,
            next_state.request(),
            next_state.after(SearchKind::Nodes),
            budget.nodes,
        )?
    };
    if !next_state.is_exhausted(SearchKind::Nodes) {
        advance_private_kind(&mut next_state, SearchKind::Nodes, &nodes)?;
    }
    let relations = if next_state.is_exhausted(SearchKind::Relations) {
        exhausted_private_kind()
    } else {
        execute_private_kind_page(
            model,
            bound,
            &mut owner,
            SearchKind::Relations,
            next_state.request(),
            next_state.after(SearchKind::Relations),
            budget.relations,
        )?
    };
    if !next_state.is_exhausted(SearchKind::Relations) {
        advance_private_kind(&mut next_state, SearchKind::Relations, &relations)?;
    }
    owner.check_selected()?;
    bound.check_model(model)?;
    let has_more = !next_state.is_exhausted(SearchKind::Nodes)
        || !next_state.is_exhausted(SearchKind::Relations);
    let next_cursor = if has_more {
        let encoded = cursor_codec.encode(&next_state)?;
        if encoded.is_empty() || encoded.len() > budget.max_cursor_bytes {
            return Err(error(
                SearchV2ErrorCode::BudgetExceeded,
                "indexed cursor exceeds adapter budget",
            ));
        }
        Some(encoded)
    } else {
        None
    };
    let mut limits = budget.json;
    limits.max_bytes = limits.max_bytes.min(budget.max_response_bytes);
    let body = packet(
        bound,
        &raw_query,
        cursor_in,
        next_cursor.as_deref(),
        state.request(),
        first_page,
        &nodes,
        &relations,
        limits,
    )?;
    let mut consulted = nodes.observed;
    consulted.extend(relations.observed);
    let mut lease = owner.0.acquire_disclosure(&scope, &consulted)?;
    lease.recheck()?;
    bound.check_model(model)?;
    Ok(DisclosableScopedIndexedSearch { body, lease })
}
