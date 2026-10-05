//! Complete bounded inspect packets on a cold-admitted CMP selection.
//! Alias sets are complete or refused; no prefix stands in for all matches.
use crate::{
    knowledge_binding::BoundCmpKnowledge,
    knowledge_packet::IndexedDisclosureScope,
    search_v2::{CurrentPolicyBinding, SearchKind, SearchV2Error, SearchV2ErrorCode},
    source_read_projection::text,
};
use rusqlite::{ErrorCode, params};
use std::{
    collections::BTreeSet,
    ops::Deref,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use tos_compiler::VerifiedKnowledgeModel;
use tos_foundation::{
    CanonicalProfile, Digest256, FoundationError, FoundationErrorCode, JsonDocument, JsonLimits,
    JsonMode, JsonValue, OwnedState, canonical_bytes_v1,
    canonical_bytes_v1_with_state_budget_and_visits, canonical_bytes_v1_with_visits, parse_json,
    parse_json_with_state_budget,
};

pub const NODE_INSPECT_OPERATION: &str = "tos.knowledge.node.inspect";
pub const RELATION_INSPECT_OPERATION: &str = "tos.knowledge.relation.inspect";
pub const INSPECT_INTENDED_USE: &str = "read_only_public_knowledge_inspect_v1";
fn error(code: SearchV2ErrorCode, message: &'static str) -> SearchV2Error {
    SearchV2Error { code, message }
}
fn corrupt(message: &'static str) -> SearchV2Error {
    error(SearchV2ErrorCode::CorruptSelectedCarrier, message)
}
fn budget_error() -> SearchV2Error {
    error(SearchV2ErrorCode::BudgetExceeded, "inspect budget exceeded")
}
fn sql_error(reason: rusqlite::Error) -> SearchV2Error {
    if matches!(reason,rusqlite::Error::SqliteFailure(failure,_) if failure.code==ErrorCode::OperationInterrupted)
    {
        budget_error()
    } else {
        corrupt("selected inspect query failed")
    }
}
pub use crate::inspect_plan::InspectBudget;

/// Optional cumulative JSON work meter for a bounded composite read. Ordinary
/// readers pass no meter and retain their existing per-document limits.
pub struct InspectVisitMeter {
    remaining: usize,
    failed: bool,
}
impl InspectVisitMeter {
    pub fn new(max_visits: usize) -> Self {
        Self {
            remaining: max_visits,
            failed: false,
        }
    }
    pub fn remaining(&self) -> usize {
        self.remaining
    }
    pub fn failed(&self) -> bool {
        self.failed
    }
    fn budget_error() -> FoundationError {
        FoundationError::new(
            FoundationErrorCode::BudgetExceeded,
            "cumulative selected JSON visit budget exceeded",
        )
    }
    fn charge(&mut self, visits: usize) -> Result<(), FoundationError> {
        let Some(remaining) = self.remaining.checked_sub(visits) else {
            self.failed = true;
            return Err(Self::budget_error());
        };
        self.remaining = remaining;
        Ok(())
    }
    pub fn parse_json(
        &mut self,
        raw: &[u8],
        mode: JsonMode,
        mut limits: JsonLimits,
    ) -> Result<JsonDocument, FoundationError> {
        if self.failed || self.remaining == 0 {
            self.failed = true;
            return Err(Self::budget_error());
        }
        limits.max_visits = limits.max_visits.min(self.remaining);
        match parse_json(raw, mode, limits) {
            Ok(document) => {
                self.charge(document.visits())?;
                Ok(document)
            }
            Err(reason) => {
                // Failed parses do not expose their partial visit counter, so
                // this composite refuses rather than allowing unmetered retry.
                self.failed = true;
                Err(reason)
            }
        }
    }
    pub fn canonical_bytes(
        &mut self,
        value: &JsonValue,
        profile: CanonicalProfile,
        mut limits: JsonLimits,
    ) -> Result<Vec<u8>, FoundationError> {
        if self.failed || self.remaining == 0 {
            self.failed = true;
            return Err(Self::budget_error());
        }
        limits.max_visits = limits.max_visits.min(self.remaining);
        match canonical_bytes_v1_with_visits(value, profile, limits) {
            Ok((bytes, writer_visits, numeric_parse_visits)) => {
                let visits = writer_visits
                    .checked_add(numeric_parse_visits)
                    .ok_or_else(|| {
                        self.failed = true;
                        Self::budget_error()
                    })?;
                self.charge(visits)?;
                Ok(bytes)
            }
            Err(reason) => {
                self.failed = true;
                Err(reason)
            }
        }
    }
}
/// A retained row's authenticated full projection. Authority includes every
/// consulted carrier, including endpoint/context carriers and every alias.
pub struct InspectedCarrier {
    pub kind: SearchKind,
    pub id: String,
    pub position: u64,
    pub payload_sha256: Digest256,
    pub payload: JsonValue,
}
#[derive(Clone, Debug)]
pub struct ObservedInspectCarrier {
    pub kind: SearchKind,
    pub id: String,
    pub source_graph: String,
    pub position: u64,
    pub payload_sha256: Digest256,
}
pub trait InspectDisclosureLease: Send {
    fn recheck(&mut self) -> Result<(), SearchV2Error>;
}
/// The hold lifetime comes from the owner, not the temporary adapter borrow.
/// Owned providers may support 'static; a scoped managed provider supports
/// only its actual held-read lifetime, including final transport delivery.
pub trait InspectCurrentAuthority<'hold> {
    /// Forecast owned policy/scope clone metadata from the already-held source,
    /// before the state-aware carrier path calls either owned-return method.
    fn disclosure_metadata_state_upper_bound(&self) -> Result<usize, SearchV2Error> {
        Err(error(
            SearchV2ErrorCode::Unavailable,
            "selected disclosure state forecast unavailable",
        ))
    }

    /// Descriptive managed basis must match the command owner's privately
    /// retained parent under its already-held current read guards. This check
    /// grants no original/carrier access and must remain covered by the same
    /// disclosure lease; its recheck must not reacquire owner locks.
    fn authorize_managed_source_current(
        &mut self,
        _: &tos_compiler::ManagedSourceProofV1,
    ) -> Result<(), SearchV2Error> {
        Err(error(
            SearchV2ErrorCode::Unavailable,
            "managed selected source authorization unavailable",
        ))
    }
    fn authorize_managed_source_v2_current(
        &mut self,
        _: &tos_compiler::ManagedSourceProofV2,
    ) -> Result<(), SearchV2Error> {
        Err(error(
            SearchV2ErrorCode::Unavailable,
            "managed selected source authorization unavailable",
        ))
    }
    /// Exact captured/public corpus originals, under the same current release
    /// projection hold. This does not grant source text or authored admission.
    fn authorize_corpus_original_current(
        &mut self,
        _: &tos_compiler::CorpusOriginalReceipt,
        _: tos_compiler::CorpusOriginalCollection,
        _: u64,
        _: &[u8],
        _: Digest256,
    ) -> Result<(), SearchV2Error> {
        Err(error(
            SearchV2ErrorCode::Unavailable,
            "selected corpus original authorization unavailable",
        ))
    }
    /// Cold-verified GraphViews index identity, covered by the same corpus
    /// projection hold. No original body or source-text grant is implied.
    fn authorize_corpus_view_identity_current(
        &mut self,
        _: &tos_compiler::CorpusOriginalReceipt,
        _: u64,
        _: Option<&str>,
        _: Digest256,
    ) -> Result<(), SearchV2Error> {
        Err(error(
            SearchV2ErrorCode::Unavailable,
            "selected corpus view identity authorization unavailable",
        ))
    }
    /// The current projection hold covers the exact selected philosophy
    /// header and every consulted original row. Custody alone is not a grant.
    fn authorize_philosophy_original_current(
        &mut self,
        _: &tos_compiler::PhilosophyOriginalReceipt,
        _: tos_compiler::PhilosophyOriginalCollection,
        _: u64,
        _: &[u8],
        _: Digest256,
    ) -> Result<(), SearchV2Error> {
        Err(error(
            SearchV2ErrorCode::Unavailable,
            "selected philosophy original authorization unavailable",
        ))
    }
    /// Original navigation custody is distinct from normalized carrier access.
    /// The same disclosure lease must cover this selected component, including
    /// its original membership index, header and every consulted rights row.
    fn authorize_navigation_original_current(
        &mut self,
        _: &tos_compiler::NavigationOriginalReceipt,
        _: i64,
        _: &[u8],
        _: Digest256,
    ) -> Result<(), SearchV2Error> {
        Err(error(
            SearchV2ErrorCode::Unavailable,
            "selected navigation original authorization unavailable",
        ))
    }
    /// Exact owner-carried registry bytes, never ambient data-root discovery.
    /// A contracts disclosure lease must cover both grants through final flush.
    fn authorize_registry_current(
        &mut self,
        _: &str,
        _: &[u8],
        _: Digest256,
    ) -> Result<(), SearchV2Error> {
        Err(error(
            SearchV2ErrorCode::Unavailable,
            "selected registry authorization unavailable",
        ))
    }
    /// Stored-lens discovery consults the exact selected public catalog.
    /// The acquired disclosure lease must also cover this catalog grant.
    fn authorize_catalog_current(&mut self, _: Digest256) -> Result<(), SearchV2Error> {
        Err(error(
            SearchV2ErrorCode::Unavailable,
            "selected catalog authorization unavailable",
        ))
    }
    /// Transport cancellation is independent of source authorization.
    fn abort_probe(&self) -> Option<Arc<dyn crate::AbortProbe>> {
        None
    }
    fn policy_binding(&self) -> CurrentPolicyBinding;
    fn disclosure_scope(&self) -> IndexedDisclosureScope;
    fn check_selected(&mut self) -> Result<(), SearchV2Error>;
    fn authorize_current(&mut self, carrier: &InspectedCarrier) -> Result<(), SearchV2Error>;
    /// Borrowed selected-row authorization avoids retaining a second parsed
    /// payload tree. The temporary clone stays under the caller's same-state
    /// workspace hold.
    fn authorize_current_borrowed(
        &mut self,
        kind: SearchKind,
        id: &str,
        position: u64,
        payload_sha256: Digest256,
        payload: &JsonValue,
    ) -> Result<(), SearchV2Error> {
        let carrier = InspectedCarrier {
            kind,
            id: id.to_owned(),
            position,
            payload_sha256,
            payload: payload.clone(),
        };
        self.authorize_current(&carrier)
    }
    fn acquire_disclosure(
        &mut self,
        scope: &IndexedDisclosureScope,
        consulted: &[ObservedInspectCarrier],
    ) -> Result<Box<dyn InspectDisclosureLease + 'hold>, SearchV2Error>;
}
pub struct DisclosableInspect<'hold> {
    body: Vec<u8>,
    lease: Box<dyn InspectDisclosureLease + 'hold>,
}
impl Deref for DisclosableInspect<'_> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.body
    }
}
impl<'hold> DisclosableInspect<'hold> {
    pub(crate) fn from_controlled(
        body: Vec<u8>,
        lease: Box<dyn InspectDisclosureLease + 'hold>,
    ) -> Self {
        Self { body, lease }
    }

    /// Move authenticated bytes and the disclosure hold into a transport packet.
    pub fn into_parts(self) -> (Vec<u8>, Box<dyn InspectDisclosureLease + 'hold>) {
        (self.body, self.lease)
    }
    pub fn recheck(&mut self) -> Result<(), SearchV2Error> {
        self.lease.recheck()
    }
}

pub(crate) fn scope_owned_state(scope: &IndexedDisclosureScope) -> Result<usize, SearchV2Error> {
    let IndexedDisclosureScope {
        operation_id,
        carrier_layer,
        intended_use,
        selected_model_receipt_id,
        source_cut,
        through_commit_seq: _,
        source_membership_root: _,
        descriptor_sha256: _,
        selected_index_sha256: _,
        policy_issuer_ref,
        policy_receipt_id,
        policy_scope,
        policy_epoch,
        withdrawal_generation,
    } = scope;
    [
        operation_id,
        carrier_layer,
        intended_use,
        selected_model_receipt_id,
        source_cut,
        policy_issuer_ref,
        policy_receipt_id,
        policy_scope,
        policy_epoch,
        withdrawal_generation,
    ]
    .into_iter()
    .try_fold(
        std::mem::size_of::<IndexedDisclosureScope>(),
        |bytes, text| bytes.checked_add(text.capacity()).ok_or_else(budget_error),
    )
}

/// One caller-owned remaining workspace; reservations are never independent grants.
struct InspectWorkspace {
    limit: usize,
    retained: usize,
}
impl InspectWorkspace {
    fn available(&self) -> Result<usize, SearchV2Error> {
        self.limit
            .checked_sub(self.retained)
            .ok_or_else(budget_error)
    }
    fn retain(&mut self, bytes: usize) -> Result<(), SearchV2Error> {
        self.retained = self
            .retained
            .checked_add(bytes)
            .filter(|n| *n <= self.limit)
            .ok_or_else(budget_error)?;
        Ok(())
    }
}

pub(crate) struct Reader<'a, 'b, A: ?Sized> {
    model: &'a mut VerifiedKnowledgeModel<'b>,
    authority: &'a mut A,
    budget: InspectBudget,
    decoded: u64,
    rows: u64,
    state: Option<InspectWorkspace>,
    work_remaining: Option<u64>,
    consulted: Vec<ObservedInspectCarrier>,
    scope: &'a IndexedDisclosureScope,
    visit_meter: Option<&'a mut InspectVisitMeter>,
}
impl<'hold, A: InspectCurrentAuthority<'hold> + ?Sized> Reader<'_, '_, A> {
    fn parse_document(
        visit_meter: Option<&mut InspectVisitMeter>,
        raw: &[u8],
        mode: JsonMode,
        limits: JsonLimits,
    ) -> Result<JsonDocument, FoundationError> {
        match visit_meter {
            Some(meter) => meter.parse_json(raw, mode, limits),
            None => parse_json(raw, mode, limits),
        }
    }
    fn canonical_bytes(
        &mut self,
        value: &JsonValue,
        profile: CanonicalProfile,
        limits: JsonLimits,
    ) -> Result<Vec<u8>, FoundationError> {
        match self.visit_meter.as_deref_mut() {
            Some(meter) => meter.canonical_bytes(value, profile, limits),
            None => canonical_bytes_v1(value, profile, limits),
        }
    }
    fn charge_state_work(&mut self, visits: usize) -> Result<(), SearchV2Error> {
        let remaining = self.work_remaining.as_mut().ok_or_else(budget_error)?;
        *remaining = remaining
            .checked_sub(u64::try_from(visits).map_err(|_| budget_error())?)
            .ok_or_else(budget_error)?;
        Ok(())
    }
    fn state_json_limits(&self) -> Result<JsonLimits, SearchV2Error> {
        let mut limits = self.budget.json;
        limits.max_visits = limits.max_visits.min(
            usize::try_from(self.work_remaining.ok_or_else(budget_error)?)
                .map_err(|_| budget_error())?,
        );
        if limits.max_visits == 0 {
            return Err(budget_error());
        }
        Ok(limits)
    }
    pub(crate) fn reserve_state(&mut self, bytes: usize) -> Result<(), SearchV2Error> {
        self.state.as_mut().ok_or_else(budget_error)?.retain(bytes)
    }
    pub(crate) fn parse_state_packet(&mut self, raw: &[u8]) -> Result<JsonValue, SearchV2Error> {
        let available = self.state.as_ref().ok_or_else(budget_error)?.available()?;
        let document = parse_json_with_state_budget(
            raw,
            JsonMode::PublishedStrict,
            self.state_json_limits()?,
            available,
        )
        .map_err(|_| budget_error())?;
        self.charge_state_work(document.visits())?;
        let value = document.into_root();
        self.reserve_state(value.retained_storage_bytes().map_err(|_| budget_error())?)?;
        Ok(value)
    }
    fn original_error(reason: tos_compiler::Error) -> SearchV2Error {
        match reason {
            tos_compiler::Error::Budget(_) | tos_compiler::Error::SqliteVmBudget { .. } => {
                budget_error()
            }
            _ => corrupt("selected navigation original read failed"),
        }
    }
    pub(crate) fn external_projection_bytes(&mut self, bytes: usize) -> Result<(), SearchV2Error> {
        if bytes == 0 || bytes > self.budget.max_payload_bytes {
            return Err(budget_error());
        }
        self.charge_original(1, bytes as u64)
    }
    fn charge_original(&mut self, rows: usize, bytes: u64) -> Result<(), SearchV2Error> {
        self.rows = self
            .rows
            .checked_add(rows as u64)
            .ok_or_else(budget_error)?;
        self.decoded = self.decoded.checked_add(bytes).ok_or_else(budget_error)?;
        if self.rows > self.budget.max_rows || self.decoded > self.budget.max_decoded_bytes {
            return Err(budget_error());
        }
        self.check_interrupt()
    }
    pub(crate) fn original_receipt(
        &mut self,
    ) -> Result<tos_compiler::NavigationOriginalReceipt, SearchV2Error> {
        self.check_interrupt()?;
        if !self.model.navigation_original_available() {
            return Err(error(
                SearchV2ErrorCode::Unavailable,
                "selected navigation originals unavailable",
            ));
        }
        self.model.check_pin().map_err(|_| {
            error(
                SearchV2ErrorCode::StaleSelection,
                "selected navigation pin changed",
            )
        })?;
        let receipt = self
            .model
            .navigation_original_receipt()
            .map_err(Self::original_error)?
            .clone();
        let rows = receipt
            .nodes
            .checked_add(receipt.edges)
            .and_then(|n| n.checked_add(receipt.rights))
            .and_then(|n| n.checked_add(1))
            .ok_or_else(budget_error)?;
        let bytes = receipt
            .total_bytes
            .checked_add(receipt.member_index_bytes)
            .ok_or_else(budget_error)?;
        if rows > self.budget.max_rows.saturating_sub(self.rows)
            || bytes > self.budget.max_decoded_bytes.saturating_sub(self.decoded)
        {
            return Err(budget_error());
        }
        Ok(receipt)
    }
    pub(crate) fn philosophy_receipt(
        &mut self,
    ) -> Result<tos_compiler::PhilosophyOriginalReceipt, SearchV2Error> {
        self.check_interrupt()?;
        if !self.model.philosophy_original_available() {
            return Err(error(
                SearchV2ErrorCode::Unavailable,
                "selected philosophy originals unavailable",
            ));
        }
        self.model.check_pin().map_err(|_| {
            error(
                SearchV2ErrorCode::StaleSelection,
                "selected philosophy pin changed",
            )
        })?;
        let receipt = self
            .model
            .philosophy_original_receipt()
            .map_err(|reason| match reason {
                tos_compiler::Error::Budget(_) | tos_compiler::Error::SqliteVmBudget { .. } => {
                    budget_error()
                }
                _ => corrupt("selected philosophy original receipt invalid"),
            })?
            .clone();
        let rows = receipt
            .nodes
            .checked_add(receipt.edges)
            .and_then(|n| n.checked_add(1))
            .ok_or_else(budget_error)?;
        if rows > self.budget.max_rows.saturating_sub(self.rows)
            || receipt.total_bytes > self.budget.max_decoded_bytes.saturating_sub(self.decoded)
        {
            return Err(budget_error());
        }
        Ok(receipt)
    }
    pub(crate) fn corpus_receipt(
        &mut self,
    ) -> Result<tos_compiler::CorpusOriginalReceipt, SearchV2Error> {
        self.check_interrupt()?;
        if !self.model.corpus_original_available() {
            return Err(error(
                SearchV2ErrorCode::Unavailable,
                "selected corpus originals unavailable",
            ));
        }
        if let Some(state) = self.state.as_mut() {
            let receipt = self
                .model
                .corpus_original_receipt()
                .map_err(Self::original_error)?;
            let forecast = receipt.retained_state_bytes().map_err(|_| budget_error())?;
            state.retain(forecast)?;
            let copy = receipt.clone();
            if copy.retained_state_bytes().map_err(|_| budget_error())? > forecast {
                return Err(budget_error());
            }
            return Ok(copy);
        }
        self.model
            .corpus_original_receipt()
            .map(Clone::clone)
            .map_err(|reason| match reason {
                tos_compiler::Error::Budget(_) | tos_compiler::Error::SqliteVmBudget { .. } => {
                    budget_error()
                }
                _ => corrupt("selected corpus original receipt invalid"),
            })
    }
    pub(crate) fn corpus_view_identity(
        &mut self,
        receipt: &tos_compiler::CorpusOriginalReceipt,
        after: Option<u64>,
    ) -> Result<Option<tos_compiler::CorpusOriginalViewIdentity>, SearchV2Error> {
        self.check_interrupt()?;
        if self.rows >= self.budget.max_rows {
            return Err(budget_error());
        }
        let bytes = self
            .budget
            .max_decoded_bytes
            .saturating_sub(self.decoded)
            .min(self.budget.max_payload_bytes as u64);
        let page = self
            .model
            .corpus_original_view_identities_under_caller_budget(
                after,
                1,
                self.budget.max_field_bytes,
                bytes,
            )
            .map_err(|reason| match reason {
                tos_compiler::Error::Budget(_) | tos_compiler::Error::SqliteVmBudget { .. } => {
                    budget_error()
                }
                _ => corrupt("selected corpus view identity read failed"),
            })?;
        self.charge_original(page.rows.len(), page.decoded_bytes)?;
        let Some(row) = page.rows.into_iter().next() else {
            return Ok(None);
        };
        if after.is_some_and(|previous| row.ordinal <= previous) {
            return Err(corrupt("selected corpus view identity order differs"));
        }
        let sha = Digest256::from_hex(&row.raw_sha256)
            .map_err(|_| corrupt("selected corpus view identity digest invalid"))?;
        self.authority.authorize_corpus_view_identity_current(
            receipt,
            row.ordinal,
            row.view_id.as_deref(),
            sha,
        )?;
        self.check_interrupt()?;
        Ok(Some(row))
    }
    pub(crate) fn corpus_row(
        &mut self,
        receipt: &tos_compiler::CorpusOriginalReceipt,
        collection: tos_compiler::CorpusOriginalCollection,
        selector: &tos_compiler::CorpusOriginalSelector,
        after: Option<u64>,
    ) -> Result<Option<(u64, JsonValue)>, SearchV2Error> {
        self.check_interrupt()?;
        if self.state.is_some() {
            if self.rows >= self.budget.max_rows {
                return Err(budget_error());
            }
            if !matches!(selector, tos_compiler::CorpusOriginalSelector::All) {
                return Err(budget_error());
            }
            let available = self.state.as_ref().unwrap().available()?;
            let overhead = std::mem::size_of::<tos_compiler::CorpusOriginalRow>()
                .checked_add(64)
                .ok_or_else(budget_error)?;
            let cap = available
                .checked_sub(overhead)
                .ok_or_else(budget_error)?
                .min(self.budget.max_payload_bytes)
                .min(
                    usize::try_from(self.budget.max_decoded_bytes.saturating_sub(self.decoded))
                        .map_err(|_| budget_error())?,
                );
            if cap == 0 {
                return Err(budget_error());
            }
            let row = self
                .model
                .corpus_original_all_row_with_state_budget(
                    collection, after, cap, cap as u64, available,
                )
                .map_err(Self::original_error)?;
            let Some(row) = row else {
                return Ok(None);
            };
            self.charge_state_work(1)?;
            self.charge_original(1, row.raw.len() as u64)?;
            let sha = Digest256::of_bytes(&row.raw);
            self.authority.authorize_corpus_original_current(
                receipt,
                collection,
                row.ordinal,
                &row.raw,
                sha,
            )?;
            self.check_interrupt()?;
            let raw_state = std::mem::size_of::<tos_compiler::CorpusOriginalRow>()
                .checked_add(row.raw.capacity())
                .and_then(|n| n.checked_add(row.raw_sha256.capacity()))
                .ok_or_else(budget_error)?;
            let parser_available = self
                .state
                .as_ref()
                .unwrap()
                .available()?
                .checked_sub(raw_state)
                .ok_or_else(budget_error)?;
            let document = parse_json_with_state_budget(
                &row.raw,
                JsonMode::PublishedStrict,
                self.state_json_limits()?,
                parser_available,
            )
            .map_err(|reason| {
                if reason.code == FoundationErrorCode::BudgetExceeded {
                    budget_error()
                } else {
                    corrupt("selected corpus original JSON invalid")
                }
            })?;
            self.charge_state_work(document.visits())?;
            let value = document.into_root();
            self.state
                .as_mut()
                .unwrap()
                .retain(value.retained_storage_bytes().map_err(|_| budget_error())?)?;
            return Ok(Some((row.ordinal, value)));
        }
        let bytes = self
            .budget
            .max_decoded_bytes
            .saturating_sub(self.decoded)
            .min(self.budget.max_payload_bytes as u64);
        let row_cap = usize::try_from(bytes).map_err(|_| budget_error())?;
        let page = self
            .model
            .corpus_original_page_under_caller_budget(
                collection, selector, after, 1, row_cap, bytes,
            )
            .map_err(|reason| match reason {
                tos_compiler::Error::Budget(_) | tos_compiler::Error::SqliteVmBudget { .. } => {
                    budget_error()
                }
                _ => corrupt("selected corpus original read failed"),
            })?;
        self.charge_original(page.rows.len(), page.decoded_bytes)?;
        let Some(row) = page.rows.into_iter().next() else {
            return Ok(None);
        };
        let sha = Digest256::of_bytes(&row.raw);
        if sha.to_hex() != row.raw_sha256 {
            return Err(corrupt("selected corpus original digest differs"));
        }
        self.authority.authorize_corpus_original_current(
            receipt,
            collection,
            row.ordinal,
            &row.raw,
            sha,
        )?;
        self.check_interrupt()?;
        let value = Self::parse_document(
            self.visit_meter.as_deref_mut(),
            &row.raw,
            JsonMode::PublishedStrict,
            self.budget.json,
        )
        .map_err(|_| corrupt("selected corpus original JSON invalid"))?
        .into_root();
        Ok(Some((row.ordinal, value)))
    }
    pub(crate) fn philosophy_row(
        &mut self,
        receipt: &tos_compiler::PhilosophyOriginalReceipt,
        collection: tos_compiler::PhilosophyOriginalCollection,
        after: Option<u64>,
    ) -> Result<Option<(u64, JsonValue)>, SearchV2Error> {
        self.check_interrupt()?;
        if self.rows >= self.budget.max_rows {
            return Err(budget_error());
        }
        let bytes = self
            .budget
            .max_decoded_bytes
            .saturating_sub(self.decoded)
            .min(self.budget.max_payload_bytes as u64);
        let row_cap = usize::try_from(bytes).map_err(|_| budget_error())?;
        let page = self
            .model
            .philosophy_original_page_under_caller_budget(collection, after, 1, row_cap, bytes)
            .map_err(|reason| match reason {
                tos_compiler::Error::Budget(_) | tos_compiler::Error::SqliteVmBudget { .. } => {
                    budget_error()
                }
                _ => corrupt("selected philosophy original read failed"),
            })?;
        self.charge_original(page.rows.len(), page.decoded_bytes)?;
        let Some(row) = page.rows.into_iter().next() else {
            return Ok(None);
        };
        let sha = Digest256::of_bytes(&row.raw);
        if sha.to_hex() != row.raw_sha256 {
            return Err(corrupt("selected philosophy original digest differs"));
        }
        self.authority.authorize_philosophy_original_current(
            receipt,
            collection,
            row.ordinal,
            &row.raw,
            sha,
        )?;
        self.check_interrupt()?;
        let value = Self::parse_document(
            self.visit_meter.as_deref_mut(),
            &row.raw,
            JsonMode::PublishedStrict,
            self.budget.json,
        )
        .map_err(|_| corrupt("selected philosophy original JSON invalid"))?
        .into_root();
        Ok(Some((row.ordinal, value)))
    }
    pub(crate) fn original_row(
        &mut self,
        receipt: &tos_compiler::NavigationOriginalReceipt,
        after: Option<i64>,
    ) -> Result<Option<(i64, JsonValue)>, SearchV2Error> {
        self.check_interrupt()?;
        if self.rows >= self.budget.max_rows {
            return Err(budget_error());
        }
        let bytes = self
            .budget
            .max_decoded_bytes
            .saturating_sub(self.decoded)
            .min(self.budget.max_payload_bytes as u64);
        let page = self
            .model
            .navigation_original_page_under_caller_budget(after, 1, bytes as usize, bytes)
            .map_err(Self::original_error)?;
        self.charge_original(page.rows.len(), page.decoded_bytes)?;
        let Some((ordinal, raw)) = page.rows.into_iter().next() else {
            return Ok(None);
        };
        self.authority.authorize_navigation_original_current(
            receipt,
            ordinal,
            &raw,
            Digest256::of_bytes(&raw),
        )?;
        self.check_interrupt()?;
        let value = Self::parse_document(
            self.visit_meter.as_deref_mut(),
            &raw,
            JsonMode::PublishedStrict,
            self.budget.json,
        )
        .map_err(|_| corrupt("selected navigation original JSON invalid"))?
        .into_root();
        Ok(Some((ordinal, value)))
    }
    pub(crate) fn original_member(
        &mut self,
        collection: &str,
        after: Option<&str>,
    ) -> Result<Option<tos_compiler::NavigationOriginalMember>, SearchV2Error> {
        self.check_interrupt()?;
        if self.rows >= self.budget.max_rows {
            return Err(budget_error());
        }
        let bytes = self
            .budget
            .max_decoded_bytes
            .saturating_sub(self.decoded)
            .min(self.budget.max_payload_bytes as u64);
        let page = self
            .model
            .navigation_original_members_under_caller_budget(collection, after, 1, bytes)
            .map_err(Self::original_error)?;
        self.charge_original(page.rows.len(), page.decoded_bytes)?;
        Ok(page.rows.into_iter().next())
    }
    pub(crate) fn registry_current(
        &mut self,
        id: &str,
        raw: &[u8],
        sha: Digest256,
    ) -> Result<(), SearchV2Error> {
        self.check_interrupt()?;
        self.rows = self.rows.checked_add(1).ok_or_else(budget_error)?;
        self.decoded = self
            .decoded
            .checked_add(raw.len() as u64)
            .ok_or_else(budget_error)?;
        if self.rows > self.budget.max_rows || self.decoded > self.budget.max_decoded_bytes {
            return Err(budget_error());
        }
        self.authority.authorize_registry_current(id, raw, sha)?;
        self.check_interrupt()
    }
    /// Use the same transport probe between bounded non-SQL work units.
    pub(crate) fn check_interrupt(&mut self) -> Result<(), SearchV2Error> {
        match self
            .authority
            .abort_probe()
            .and_then(|probe| probe.reason())
        {
            Some(crate::AbortReason::Cancelled) => {
                return Err(error(
                    SearchV2ErrorCode::Cancelled,
                    "selected knowledge query cancelled",
                ));
            }
            Some(crate::AbortReason::DeadlineExceeded) => {
                return Err(error(
                    SearchV2ErrorCode::DeadlineExceeded,
                    "selected knowledge query deadline exceeded",
                ));
            }
            None => {}
        }
        self.authority.check_selected()
    }
    pub(crate) fn abort_probe(&self) -> Option<Arc<dyn crate::AbortProbe>> {
        self.authority.abort_probe()
    }
    pub(crate) fn disclosure_scope(&self) -> &IndexedDisclosureScope {
        self.scope
    }
    /// Exact same-cut certified normalized scope counts. Does not walk graph
    /// rows to rediscover a producer count already bound by cold admission.
    pub(crate) fn scope_count(
        &mut self,
        kind: SearchKind,
        sources: &[String],
    ) -> Result<u64, SearchV2Error> {
        self.authority.check_selected()?;
        let list = JsonValue::Array(sources.iter().map(|source| text(source)).collect());
        let encoded = self
            .canonical_bytes(
                &list,
                CanonicalProfile::SourceRecordDigestV1,
                self.budget.json,
            )
            .map_err(|_| budget_error())?;
        let encoded =
            std::str::from_utf8(&encoded).map_err(|_| corrupt("scope sources invalid"))?;
        let field = if kind == SearchKind::Nodes {
            "expected_node_count"
        } else {
            "expected_relation_count"
        };
        let sql = format!(
            "SELECT {field} FROM source_scope WHERE source_graph IN (SELECT value FROM json_each(?1))"
        );
        let mut statement = self
            .model
            .connection()
            .prepare_cached(&sql)
            .map_err(sql_error)?;
        let mut rows = statement.query([encoded]).map_err(sql_error)?;
        let mut total = 0u64;
        let mut count = 0;
        while let Some(row) = rows.next().map_err(sql_error)? {
            let n = row.get::<_, i64>(0).map_err(sql_error)?;
            if n < 0 {
                return Err(corrupt("selected source scope count invalid"));
            }
            self.rows = self.rows.checked_add(1).ok_or_else(budget_error)?;
            self.decoded = self.decoded.checked_add(8).ok_or_else(budget_error)?;
            if self.rows > self.budget.max_rows || self.decoded > self.budget.max_decoded_bytes {
                return Err(budget_error());
            }
            total = total.checked_add(n as u64).ok_or_else(budget_error)?;
            count += 1;
        }
        if count != sources.iter().collect::<BTreeSet<_>>().len() {
            return Err(error(
                SearchV2ErrorCode::IndexIncomplete,
                "selected source scope metadata incomplete",
            ));
        }
        Ok(total)
    }
    /// Digest-bound selected graph metadata, including exact property grammar.
    pub(crate) fn header(&mut self) -> Result<JsonValue, SearchV2Error> {
        self.authority.check_selected()?;
        let mut statement=self.model.connection().prepare_cached("SELECT packet_len,CASE WHEN typeof(packet_sha256)='blob' AND length(packet_sha256)=32 THEN packet_sha256 END,CASE WHEN typeof(packet)='blob' AND packet_len BETWEEN 0 AND ?1 AND length(packet)=packet_len THEN packet END FROM graph_header WHERE singleton=1").map_err(sql_error)?;
        let mut rows = statement
            .query([self.budget.max_payload_bytes as i64])
            .map_err(sql_error)?;
        let row = rows
            .next()
            .map_err(sql_error)?
            .ok_or_else(|| corrupt("selected header absent"))?;
        let length = row.get::<_, i64>(0).map_err(sql_error)?;
        if length < 0 || length as usize > self.budget.max_payload_bytes {
            return Err(budget_error());
        }
        self.rows = self.rows.checked_add(1).ok_or_else(budget_error)?;
        self.decoded = self
            .decoded
            .checked_add(length as u64 + 40)
            .ok_or_else(budget_error)?;
        if self.rows > self.budget.max_rows || self.decoded > self.budget.max_decoded_bytes {
            return Err(budget_error());
        }
        // Aggregate admission precedes copying the header BLOB out of SQLite.
        let sha = row
            .get::<_, Option<Vec<u8>>>(1)
            .map_err(sql_error)?
            .ok_or_else(|| corrupt("selected header digest width invalid"))?;
        let payload = row
            .get::<_, Option<Vec<u8>>>(2)
            .map_err(sql_error)?
            .ok_or_else(|| corrupt("selected header length/type invalid"))?;
        if Digest256::of_bytes(&payload).as_bytes() != sha.as_slice() {
            return Err(corrupt("selected header digest differs"));
        }
        Self::parse_document(
            self.visit_meter.as_deref_mut(),
            &payload,
            JsonMode::PublishedStrict,
            self.budget.json,
        )
        .map(|doc| doc.into_root())
        .map_err(|_| corrupt("selected header JSON invalid"))
    }
    /// A stored spec is source-owned catalog data, never a request-supplied
    /// replacement. Admission occurs before copying its complete bounded BLOB.
    pub(crate) fn catalog_packet(
        &mut self,
        bound: &BoundCmpKnowledge<'_>,
    ) -> Result<JsonValue, SearchV2Error> {
        self.authority.check_selected()?;
        self.authority
            .authorize_catalog_current(bound.selection().catalog_packet_sha256)?;
        let mut statement = self.model.connection().prepare_cached("SELECT packet_len,CASE WHEN typeof(packet_sha256)='blob' AND length(packet_sha256)=32 THEN packet_sha256 END,CASE WHEN typeof(packet)='blob' AND packet_len BETWEEN 0 AND ?2 AND length(packet)=packet_len THEN packet END FROM catalog_index_meta WHERE descriptor_sha256=?1").map_err(sql_error)?;
        let mut rows = statement
            .query(params![
                bound.selection().vocabulary.descriptor_sha256.to_hex(),
                self.budget.max_payload_bytes as i64
            ])
            .map_err(sql_error)?;
        let row = rows
            .next()
            .map_err(sql_error)?
            .ok_or_else(|| corrupt("selected catalog absent"))?;
        let length = row.get::<_, i64>(0).map_err(sql_error)?;
        if length < 0 || length as usize > self.budget.max_payload_bytes {
            return Err(budget_error());
        }
        self.rows = self.rows.checked_add(1).ok_or_else(budget_error)?;
        self.decoded = self
            .decoded
            .checked_add(length as u64 + 40)
            .ok_or_else(budget_error)?;
        if self.rows > self.budget.max_rows || self.decoded > self.budget.max_decoded_bytes {
            return Err(budget_error());
        }
        let sha = row
            .get::<_, Option<Vec<u8>>>(1)
            .map_err(sql_error)?
            .ok_or_else(|| corrupt("selected catalog digest width invalid"))?;
        let payload = row
            .get::<_, Option<Vec<u8>>>(2)
            .map_err(sql_error)?
            .ok_or_else(|| corrupt("selected catalog length/type invalid"))?;
        if sha.as_slice() != bound.selection().catalog_packet_sha256.as_bytes()
            || Digest256::of_bytes(&payload) != bound.selection().catalog_packet_sha256
        {
            return Err(corrupt("selected catalog digest differs"));
        }
        let packet = Self::parse_document(
            self.visit_meter.as_deref_mut(),
            &payload,
            JsonMode::PublishedStrict,
            self.budget.json,
        )
        .map_err(|_| corrupt("selected catalog JSON invalid"))?
        .into_root();
        match self.visit_meter.as_deref_mut() {
            Some(meter) => {
                bound.validate_catalog_identity_metered(&packet, self.budget.json, meter)?
            }
            None => bound.validate_catalog_identity(&packet, self.budget.json)?,
        }
        Ok(packet)
    }
    /// Complete retained carrier keysets in source/encounter order. Candidate
    /// staging tables are deliberately absent from cold-admitted runtime files;
    /// final lens selection applies its declared sorts after this bounded scan.
    /// Payload reads still go through authenticated items().
    pub(crate) fn candidate_ids(
        &mut self,
        kind: SearchKind,
        sources: &[String],
        after: Option<(&str, i64)>,
        limit: usize,
    ) -> Result<Vec<(String, i64, String)>, SearchV2Error> {
        if limit == 0 || limit > self.budget.max_rows as usize || limit > i64::MAX as usize {
            return Err(budget_error());
        }
        self.authority.check_selected()?;
        let list = JsonValue::Array(sources.iter().map(|source| text(source)).collect());
        let encoded = self
            .canonical_bytes(
                &list,
                CanonicalProfile::SourceRecordDigestV1,
                self.budget.json,
            )
            .map_err(|_| budget_error())?;
        let encoded =
            std::str::from_utf8(&encoded).map_err(|_| corrupt("candidate sources invalid"))?;
        let (table, index) = if kind == SearchKind::Nodes {
            ("knowledge_nodes", "knowledge_nodes_source_order")
        } else {
            ("knowledge_relations", "knowledge_relations_source_order")
        };
        let (source, position) = after.unwrap_or(("", -1));
        let sql = format!(
            "SELECT CASE WHEN length(CAST(source_graph AS BLOB))<=?5 THEN source_graph END,source_order,CASE WHEN length(CAST(id AS BLOB))<=?5 THEN id END FROM {table} INDEXED BY {index} WHERE source_graph IN (SELECT value FROM json_each(?1)) AND (source_graph,source_order)>(?2,?3) ORDER BY source_graph,source_order LIMIT ?4"
        );
        let mut statement = self
            .model
            .connection()
            .prepare_cached(&sql)
            .map_err(sql_error)?;
        let mut rows = statement
            .query(params![
                encoded,
                source,
                position,
                limit as i64,
                self.budget.max_field_bytes as i64
            ])
            .map_err(sql_error)?;
        let mut result = vec![];
        while let Some(row) = rows.next().map_err(sql_error)? {
            let source = row
                .get::<_, Option<String>>(0)
                .map_err(sql_error)?
                .ok_or_else(budget_error)?;
            let position = row.get::<_, i64>(1).map_err(sql_error)?;
            if position < 0 {
                return Err(corrupt("candidate source order invalid"));
            }
            let id = row
                .get::<_, Option<String>>(2)
                .map_err(sql_error)?
                .ok_or_else(budget_error)?;
            // Disjoint field borrows permit charging before storing the row.
            self.rows = self.rows.checked_add(1).ok_or_else(budget_error)?;
            self.decoded = self
                .decoded
                .checked_add((source.len() + id.len() + 8) as u64)
                .ok_or_else(budget_error)?;
            if self.rows > self.budget.max_rows || self.decoded > self.budget.max_decoded_bytes {
                return Err(budget_error());
            }
            result.push((source, position, id));
        }
        Ok(result)
    }
    /// Two bounded directed keysets merged by exact raw relation ID. The end
    /// is meaningful because CMP cold admission certified both complete rows.
    pub(crate) fn incident_ids(
        &mut self,
        node: &str,
        after: &str,
        limit: usize,
    ) -> Result<Vec<String>, SearchV2Error> {
        if limit == 0 || limit > self.budget.max_rows as usize || limit > i64::MAX as usize {
            return Err(budget_error());
        }
        self.authority.check_selected()?;
        let sql = "SELECT CASE WHEN length(CAST(id AS BLOB))<=?4 THEN id END FROM (SELECT id FROM (SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_from_id WHERE from_id=?1 AND id>?2 ORDER BY id LIMIT ?3) UNION SELECT id FROM (SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_to_id WHERE to_id=?1 AND id>?2 ORDER BY id LIMIT ?3)) ORDER BY id LIMIT ?3";
        let mut statement = self
            .model
            .connection()
            .prepare_cached(sql)
            .map_err(sql_error)?;
        let mut rows = statement
            .query(params![
                node,
                after,
                limit as i64,
                self.budget.max_field_bytes as i64
            ])
            .map_err(sql_error)?;
        let mut result = vec![];
        while let Some(row) = rows.next().map_err(sql_error)? {
            let id = row
                .get::<_, Option<String>>(0)
                .map_err(sql_error)?
                .ok_or_else(budget_error)?;
            self.rows = self.rows.checked_add(1).ok_or_else(budget_error)?;
            self.decoded = self
                .decoded
                .checked_add(id.len() as u64)
                .ok_or_else(budget_error)?;
            if self.rows > self.budget.max_rows || self.decoded > self.budget.max_decoded_bytes {
                return Err(budget_error());
            }
            result.push(id);
        }
        Ok(result)
    }
    pub(crate) fn identity_ids(
        &mut self,
        entity: &str,
        sources: &[String],
        after: &str,
        limit: usize,
    ) -> Result<Vec<String>, SearchV2Error> {
        if limit == 0 || limit > self.budget.max_rows as usize || limit > i64::MAX as usize {
            return Err(budget_error());
        }
        self.authority.check_selected()?;
        let list = JsonValue::Array(sources.iter().map(|source| text(source)).collect());
        let encoded = self
            .canonical_bytes(
                &list,
                CanonicalProfile::SourceRecordDigestV1,
                self.budget.json,
            )
            .map_err(|_| budget_error())?;
        let encoded =
            std::str::from_utf8(&encoded).map_err(|_| corrupt("identity sources invalid"))?;
        let sql = "SELECT CASE WHEN length(CAST(id AS BLOB))<=?5 THEN id END FROM knowledge_nodes INDEXED BY knowledge_nodes_entity_id WHERE entity_id=?1 AND id>?2 AND source_graph IN (SELECT value FROM json_each(?3)) ORDER BY id LIMIT ?4";
        let mut statement = self
            .model
            .connection()
            .prepare_cached(sql)
            .map_err(sql_error)?;
        let mut rows = statement
            .query(params![
                entity,
                after,
                encoded,
                limit as i64,
                self.budget.max_field_bytes as i64
            ])
            .map_err(sql_error)?;
        let mut result = vec![];
        while let Some(row) = rows.next().map_err(sql_error)? {
            let id = row
                .get::<_, Option<String>>(0)
                .map_err(sql_error)?
                .ok_or_else(budget_error)?;
            self.rows = self.rows.checked_add(1).ok_or_else(budget_error)?;
            self.decoded = self
                .decoded
                .checked_add(id.len() as u64)
                .ok_or_else(budget_error)?;
            if self.rows > self.budget.max_rows || self.decoded > self.budget.max_decoded_bytes {
                return Err(budget_error());
            }
            result.push(id);
        }
        Ok(result)
    }
    /// SQL CASE enforces field/payload transfer caps before row allocation.
    pub(crate) fn items(
        &mut self,
        kind: SearchKind,
        field: &str,
        id: &str,
        limit: usize,
        ordered_id: bool,
    ) -> Result<Vec<JsonValue>, SearchV2Error> {
        Ok(self
            .items_with_sizes(kind, field, id, limit, ordered_id)?
            .into_iter()
            .map(|(value, _)| value)
            .collect())
    }
    pub(crate) fn items_with_sizes(
        &mut self,
        kind: SearchKind,
        field: &str,
        id: &str,
        limit: usize,
        ordered_id: bool,
    ) -> Result<Vec<(JsonValue, usize)>, SearchV2Error> {
        if limit == 0 {
            return Ok(vec![]);
        }
        self.authority.check_selected()?;
        self.model.check_pin().map_err(|_| {
            error(
                SearchV2ErrorCode::StaleSelection,
                "inspect selection changed",
            )
        })?;
        let table = if kind == SearchKind::Nodes {
            "knowledge_nodes"
        } else {
            "knowledge_relations"
        };
        let index = match (kind, field) {
            (_, "id") => "",
            (SearchKind::Nodes, "entity_id") => " INDEXED BY knowledge_nodes_entity",
            (SearchKind::Nodes, "native_id") => " INDEXED BY knowledge_nodes_native",
            (SearchKind::Relations, "native_id") => " INDEXED BY knowledge_relations_native",
            _ => return Err(corrupt("invalid internal inspect selector")),
        };
        let order = if ordered_id { "id" } else { "source_order" };
        let sql=format!("SELECT CASE WHEN typeof(id)='text' AND length(CAST(id AS BLOB))<=?2 THEN id END,source_order,
            CASE WHEN typeof(payload_len)='integer' AND payload_len BETWEEN 0 AND ?3 THEN payload_len END,
            CASE WHEN typeof(payload_sha256)='blob' AND length(payload_sha256)=32 THEN payload_sha256 END,
            CASE WHEN typeof(payload)='blob' AND payload_len BETWEEN 0 AND ?3 AND length(payload)=payload_len THEN payload END
            FROM {table}{index} WHERE {field}=?1 ORDER BY {order} LIMIT ?4");
        let mut statement = self
            .model
            .connection()
            .prepare_cached(&sql)
            .map_err(sql_error)?;
        let mut rows = statement
            .query(params![
                id,
                self.budget.max_field_bytes as i64,
                self.budget.max_payload_bytes as i64,
                limit as i64
            ])
            .map_err(sql_error)?;
        let mut values = vec![];
        while let Some(row) = rows.next().map_err(sql_error)? {
            self.rows = self.rows.checked_add(1).ok_or_else(budget_error)?;
            if self.rows > self.budget.max_rows {
                return Err(budget_error());
            }
            let Some(row_id) = row.get::<_, Option<String>>(0).map_err(sql_error)? else {
                return Err(budget_error());
            };
            let position = row.get::<_, i64>(1).map_err(sql_error)?;
            if position < 0 {
                return Err(corrupt("inspect source order invalid"));
            }
            let Some(length) = row.get::<_, Option<i64>>(2).map_err(sql_error)? else {
                return Err(budget_error());
            };
            let Some(sha) = row.get::<_, Option<Vec<u8>>>(3).map_err(sql_error)? else {
                return Err(corrupt("inspect carrier digest width invalid"));
            };
            // Prevent aggregate transfer allocation, not merely post-read cap.
            self.decoded = self
                .decoded
                .checked_add(row_id.len() as u64 + 48 + length as u64)
                .ok_or_else(budget_error)?;
            if self.decoded > self.budget.max_decoded_bytes {
                return Err(budget_error());
            }
            let Some(payload) = row.get::<_, Option<Vec<u8>>>(4).map_err(sql_error)? else {
                return Err(corrupt("inspect carrier length/type differs"));
            };
            let payload_sha = Digest256::of_bytes(&payload);
            if payload.len() != length as usize || payload_sha.as_bytes() != sha.as_slice() {
                return Err(corrupt("inspect carrier digest differs"));
            }
            let mut limits = self.budget.json;
            limits.max_bytes = limits.max_bytes.min(payload.len());
            let parsed = Self::parse_document(
                self.visit_meter.as_deref_mut(),
                &payload,
                JsonMode::PublishedStrict,
                limits,
            )
            .map_err(|_| corrupt("inspect carrier JSON invalid"))?;
            let value = parsed.root().clone();
            if value.object_get("id").and_then(JsonValue::as_str) != Some(row_id.as_str()) {
                return Err(corrupt("inspect carrier ID mirror differs"));
            }
            let carrier = InspectedCarrier {
                kind,
                id: row_id,
                position: position as u64,
                payload_sha256: payload_sha,
                payload: value.clone(),
            };
            self.authority.authorize_current(&carrier)?;
            self.consulted.push(ObservedInspectCarrier {
                kind: carrier.kind,
                id: carrier.id,
                position: carrier.position,
                payload_sha256: carrier.payload_sha256,
                source_graph: carrier
                    .payload
                    .object_get("source_graph")
                    .and_then(JsonValue::as_str)
                    .ok_or_else(|| corrupt("inspect carrier source invalid"))?
                    .to_owned(),
            });
            values.push((value, payload.len()));
        }
        Ok(values)
    }
    fn incident(
        &mut self,
        matches: &[String],
        limit: usize,
    ) -> Result<(u64, Vec<JsonValue>), SearchV2Error> {
        let ids = JsonValue::Array(matches.iter().map(|value| text(value)).collect());
        let encoded = self
            .canonical_bytes(
                &ids,
                CanonicalProfile::SourceRecordDigestV1,
                self.budget.json,
            )
            .map_err(|_| budget_error())?;
        let encoded = std::str::from_utf8(&encoded).map_err(|_| corrupt("inspect IDs invalid"))?;
        let union = "SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_from WHERE from_id IN (SELECT value FROM json_each(?1)) UNION SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_to WHERE to_id IN (SELECT value FROM json_each(?1))";
        let total: i64 = self
            .model
            .connection()
            .query_row(
                &format!("SELECT count(*) FROM ({union})"),
                [encoded],
                |row| row.get(0),
            )
            .map_err(sql_error)?;
        if total < 0 {
            return Err(corrupt("inspect incident count invalid"));
        }
        let selected = {
            let mut statement=self.model.connection().prepare_cached(&format!("SELECT CASE WHEN length(CAST(id AS BLOB))<=?3 THEN id END FROM ({union}) ORDER BY id LIMIT ?2")).map_err(sql_error)?;
            statement
                .query_map(
                    params![encoded, limit as i64, self.budget.max_field_bytes as i64],
                    |row| row.get::<_, Option<String>>(0),
                )
                .map_err(sql_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(sql_error)?
        };
        let mut related = vec![];
        for id in selected {
            let id = id.ok_or_else(budget_error)?;
            let rows = self.items(SearchKind::Relations, "id", &id, 1, true)?;
            if rows.len() != 1 {
                return Err(corrupt("inspect incident carrier missing"));
            }
            related.extend(rows);
        }
        Ok((total as u64, related))
    }
}

/// Exact legacy packet semantics, with selected-row closure and current held
/// disclosure. Caller budgets may refuse an expensive complete packet.
pub fn execute_selected_inspect<'hold, A: InspectCurrentAuthority<'hold> + ?Sized>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    kind: SearchKind,
    identifier: &str,
    relation_limit: usize,
    budget: InspectBudget,
) -> Result<DisclosableInspect<'hold>, SearchV2Error> {
    let request = crate::InspectRequest::new(kind, identifier, relation_limit, budget)?;
    let operation = if kind == SearchKind::Nodes {
        NODE_INSPECT_OPERATION
    } else {
        RELATION_INSPECT_OPERATION
    };
    execute_selected_carrier_packet(
        model,
        bound,
        authority,
        operation,
        INSPECT_INTENDED_USE,
        budget,
        |read| {
            let mut plan = if let Some(revision) = bound.source_revision() {
                let authority_boundary = parse_json(
                    bound.authority_boundary().as_bytes(),
                    JsonMode::PublishedStrict,
                    budget.json,
                )
                .map_err(|_| corrupt("inspect authority boundary invalid"))?
                .into_root();
                crate::InspectPlan::new(request, revision.to_owned(), authority_boundary, budget)?
            } else {
                // Only this adapter supplies the digest-bound selected header.
                let header = read.header()?;
                crate::InspectPlan::from_managed_header(
                    request,
                    header,
                    bound,
                    budget,
                    &mut read.decoded,
                )?
            };
            let probe = read.authority.abort_probe();
            while let Some(need) = plan.need().cloned() {
                read.check_interrupt()?;
                let before = read.decoded;
                match need {
                    crate::InspectNeed::Lookup {
                        kind,
                        selector,
                        identifier,
                        limit,
                    } => {
                        let rows = read.items(
                            kind,
                            selector,
                            &identifier,
                            limit.checked_add(1).ok_or_else(budget_error)?,
                            false,
                        )?;
                        plan.resume_lookup(rows, read.decoded - before, probe.as_deref())?;
                    }
                    crate::InspectNeed::NodeIncident {
                        ids,
                        relation_limit,
                    } => {
                        let (total, rows) = read.incident(&ids, relation_limit)?;
                        plan.resume_incident(total, rows, read.decoded - before, probe.as_deref())?;
                    }
                    crate::InspectNeed::RelationEndpoints { ids } => {
                        let mut rows = Vec::new();
                        for id in ids {
                            rows.extend(read.items(SearchKind::Nodes, "id", &id, 1, true)?);
                        }
                        plan.resume_endpoints(rows, read.decoded - before, probe.as_deref())?;
                    }
                }
            }
            plan.into_packet()
        },
    )
}

// The exact-carrier families share one bounded read and disclosure lifetime.
pub(crate) fn execute_selected_carrier_packet<
    'hold,
    A: InspectCurrentAuthority<'hold> + ?Sized,
    F,
>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    operation: &str,
    intended_use: &str,
    budget: InspectBudget,
    compute: F,
) -> Result<DisclosableInspect<'hold>, SearchV2Error>
where
    F: FnOnce(&mut Reader<'_, '_, A>) -> Result<JsonValue, SearchV2Error>,
{
    execute_selected_carrier_packet_observed(
        model,
        bound,
        authority,
        operation,
        intended_use,
        budget,
        compute,
        |_| Ok(()),
    )
}

pub(crate) fn execute_selected_carrier_packet_with_state<
    'hold,
    A: InspectCurrentAuthority<'hold> + ?Sized,
    F,
>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    operation: &str,
    intended_use: &str,
    budget: InspectBudget,
    available: usize,
    max_work_steps: u64,
    compute: F,
) -> Result<DisclosableInspect<'hold>, SearchV2Error>
where
    F: FnOnce(&mut Reader<'_, '_, A>) -> Result<JsonValue, SearchV2Error>,
{
    execute_selected_carrier_packet_observed_with_meter(
        model,
        bound,
        authority,
        operation,
        intended_use,
        budget,
        None,
        Some((available, max_work_steps)),
        compute,
        |_| Ok(()),
    )
}

// E3 alone needs the actual emitted response for staged checkpoint accounting.
// The observer cannot return/substitute bytes. Failure drops the staged hold
// before disclosure; successful commit remains outside this common read path.
pub(crate) fn execute_selected_carrier_packet_observed<
    'hold,
    A: InspectCurrentAuthority<'hold> + ?Sized,
    F,
    O,
>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    operation: &str,
    intended_use: &str,
    budget: InspectBudget,
    compute: F,
    observe: O,
) -> Result<DisclosableInspect<'hold>, SearchV2Error>
where
    F: FnOnce(&mut Reader<'_, '_, A>) -> Result<JsonValue, SearchV2Error>,
    O: FnOnce(&[u8]) -> Result<(), SearchV2Error>,
{
    execute_selected_carrier_packet_observed_with_meter(
        model,
        bound,
        authority,
        operation,
        intended_use,
        budget,
        None,
        None,
        compute,
        observe,
    )
}

pub(crate) fn execute_selected_carrier_packet_metered<
    'hold,
    A: InspectCurrentAuthority<'hold> + ?Sized,
    F,
>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    operation: &str,
    intended_use: &str,
    budget: InspectBudget,
    meter: &mut InspectVisitMeter,
    compute: F,
) -> Result<DisclosableInspect<'hold>, SearchV2Error>
where
    F: FnOnce(&mut Reader<'_, '_, A>) -> Result<JsonValue, SearchV2Error>,
{
    execute_selected_carrier_packet_with_optional_meter(
        model,
        bound,
        authority,
        operation,
        intended_use,
        budget,
        Some(meter),
        compute,
    )
}

pub(crate) fn execute_selected_carrier_packet_with_optional_meter<
    'hold,
    A: InspectCurrentAuthority<'hold> + ?Sized,
    F,
>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    operation: &str,
    intended_use: &str,
    budget: InspectBudget,
    visit_meter: Option<&mut InspectVisitMeter>,
    compute: F,
) -> Result<DisclosableInspect<'hold>, SearchV2Error>
where
    F: FnOnce(&mut Reader<'_, '_, A>) -> Result<JsonValue, SearchV2Error>,
{
    execute_selected_carrier_packet_observed_with_meter(
        model,
        bound,
        authority,
        operation,
        intended_use,
        budget,
        visit_meter,
        None,
        compute,
        |_| Ok(()),
    )
}

fn execute_selected_carrier_packet_observed_with_meter<
    'hold,
    A: InspectCurrentAuthority<'hold> + ?Sized,
    F,
    O,
>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    operation: &str,
    intended_use: &str,
    budget: InspectBudget,
    mut visit_meter: Option<&mut InspectVisitMeter>,
    available_state: Option<(usize, u64)>,
    compute: F,
    observe: O,
) -> Result<DisclosableInspect<'hold>, SearchV2Error>
where
    F: FnOnce(&mut Reader<'_, '_, A>) -> Result<JsonValue, SearchV2Error>,
    O: FnOnce(&[u8]) -> Result<(), SearchV2Error>,
{
    if budget.max_open_vm_steps == 0
        || model.open_vm_steps() > budget.max_open_vm_steps
        || budget.max_read_vm_steps == 0
        || budget.max_matches == 0
        || budget.max_matches >= i64::MAX as usize
        || budget.max_rows == 0
        || budget.max_payload_bytes == 0
        || budget.max_payload_bytes > i64::MAX as usize
        || budget.max_field_bytes == 0
        || budget.max_field_bytes > i64::MAX as usize
        || budget.max_response_bytes == 0
        || budget.max_decoded_bytes == 0
    {
        return Err(budget_error());
    }
    bound.check_model(model)?;
    let mut metadata_forecast = None;
    let mut state = match available_state {
        None => None,
        Some((limit, _)) => {
            let mut state = InspectWorkspace { limit, retained: 0 };
            state.retain(
                std::mem::size_of::<Reader<'_, '_, A>>()
                    .checked_add(std::mem::size_of::<DisclosableInspect<'hold>>())
                    .and_then(|n| n.checked_add(std::mem::size_of::<AtomicU64>()))
                    .and_then(|n| n.checked_add(std::mem::size_of::<JsonValue>()))
                    .ok_or_else(budget_error)?,
            )?;
            let forecast = authority.disclosure_metadata_state_upper_bound()?;
            state.retain(forecast)?;
            metadata_forecast = Some(forecast);
            Some(state)
        }
    };
    let policy = authority.policy_binding();
    let abort = authority.abort_probe();
    let check_abort = || match abort.as_ref().and_then(|probe| probe.reason()) {
        Some(crate::AbortReason::Cancelled) => Err(SearchV2Error {
            code: SearchV2ErrorCode::Cancelled,
            message: "selected knowledge query cancelled",
        }),
        Some(crate::AbortReason::DeadlineExceeded) => Err(SearchV2Error {
            code: SearchV2ErrorCode::DeadlineExceeded,
            message: "selected knowledge query deadline exceeded",
        }),
        None => Ok(()),
    };
    check_abort()?;
    let scope = authority.disclosure_scope();
    if let Some(forecast) = metadata_forecast {
        let actual = policy
            .retained_state_bytes()
            .map_err(|_| budget_error())?
            .checked_add(scope_owned_state(&scope)?)
            .ok_or_else(budget_error)?;
        if actual > forecast {
            return Err(budget_error());
        }
    }
    scope.validate_for(bound, &policy, operation, intended_use)?;
    authority.check_selected()?;
    if let Some(proof) = bound.source_basis().managed_source() {
        authority.authorize_managed_source_current(proof)?;
    }
    if let Some(proof) = bound.source_basis().managed_source_v2() {
        authority.authorize_managed_source_v2_current(proof)?;
    }
    let steps = Arc::new(AtomicU64::new(0));
    let observed = Arc::clone(&steps);
    let cap = budget.max_read_vm_steps;
    let vm_abort = abort.clone();
    model.connection().progress_handler(
        1,
        Some(move || {
            vm_abort
                .as_ref()
                .is_some_and(|probe| probe.reason().is_some())
                || observed.fetch_add(1, Ordering::Relaxed) >= cap
        }),
    );
    let result = (|| {
        let mut read = Reader {
            model,
            authority,
            budget,
            decoded: 0,
            rows: 0,
            state: state.take(),
            work_remaining: available_state.map(|(_, work)| work),
            consulted: vec![],
            scope: &scope,
            visit_meter: visit_meter.take(),
        };
        let value = compute(&mut read)?;
        check_abort()?;
        let mut limits = budget.json;
        limits.max_bytes = limits.max_bytes.min(budget.max_response_bytes);
        let body = if read.state.is_some() {
            let mut state_limits = read.state_json_limits()?;
            state_limits.max_bytes = limits.max_bytes;
            let available = read.state.as_ref().unwrap().available()?;
            let (body, visits) = canonical_bytes_v1_with_state_budget_and_visits(
                &value,
                CanonicalProfile::SourceRecordDigestV1,
                state_limits,
                available,
            )
            .map_err(|reason| {
                if reason.code == FoundationErrorCode::BudgetExceeded {
                    budget_error()
                } else {
                    corrupt("selected knowledge response cannot be emitted")
                }
            })?;
            read.charge_state_work(visits)?;
            body
        } else {
            read.canonical_bytes(&value, CanonicalProfile::SourceRecordDigestV1, limits)
                .map_err(|reason| {
                    if reason.code == FoundationErrorCode::BudgetExceeded {
                        budget_error()
                    } else {
                        corrupt("selected knowledge response cannot be emitted")
                    }
                })?
        };
        observe(&body)?;
        check_abort()?;
        read.authority.check_selected()?;
        bound.check_model(read.model)?;
        if let Some(proof) = bound.source_basis().managed_source() {
            read.authority.authorize_managed_source_current(proof)?;
        }
        if let Some(proof) = bound.source_basis().managed_source_v2() {
            read.authority.authorize_managed_source_v2_current(proof)?;
        }
        let mut lease = read.authority.acquire_disclosure(&scope, &read.consulted)?;
        lease.recheck()?;
        check_abort()?;
        Ok(DisclosableInspect { body, lease })
    })();
    model.connection().progress_handler(0, None::<fn() -> bool>);
    check_abort()?;
    if steps.load(Ordering::Relaxed) > budget.max_read_vm_steps {
        return Err(budget_error());
    }
    result
}

/// Explicit completed selected graph metadata without node/relation materialization.
/// Its dedicated operation/intended-use scope must be granted by the held owner;
/// a catalog lease alone does not select this raw header disclosure.
pub fn execute_selected_knowledge_header<'hold, A: InspectCurrentAuthority<'hold> + ?Sized>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    budget: InspectBudget,
) -> Result<DisclosableInspect<'hold>, SearchV2Error> {
    execute_selected_carrier_packet(
        model,
        bound,
        authority,
        "tos_knowledge_header",
        "read_only_public_knowledge_header_v1",
        budget,
        |read| {
            let header = read.header()?;
            if header.as_object().is_none() {
                return Err(corrupt("selected knowledge header is not an object"));
            }
            Ok(header)
        },
    )
}
