//! Bounded native catalog compatibility packet from one cold-admitted model.
//! Whole-catalog delivery is an explicit admitted job; it never scans core rows.

use std::{
    ops::Deref,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use rusqlite::{ErrorCode, OptionalExtension, params};
use tos_compiler::VerifiedKnowledgeModel;
use tos_foundation::{Digest256, JsonLimits, JsonMode, parse_json};

use crate::{knowledge_binding::BoundCmpKnowledge, search_v2::CurrentPolicyBinding};

pub const CATALOG_OPERATION_ID: &str = "tos.knowledge.catalog";
pub const CATALOG_CARRIER_LAYER: &str = "tos_knowledge_public_graph_projection_v1";
pub const CATALOG_INTENDED_USE: &str = "read_only_public_knowledge_catalog_v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogErrorCode {
    Cancelled,
    DeadlineExceeded,
    BudgetExceeded,
    StaleSelection,
    CorruptSelectedCarrier,
    PolicyBindingUnavailable,
    Unauthorized,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogError {
    pub code: CatalogErrorCode,
    pub message: &'static str,
}

fn error(code: CatalogErrorCode, message: &'static str) -> CatalogError {
    CatalogError { code, message }
}

#[derive(Clone, Copy, Debug)]
pub struct CatalogBudget {
    pub max_open_vm_steps: u64,
    pub max_read_vm_steps: u64,
    pub max_packet_bytes: usize,
    pub max_decoded_bytes: usize,
    pub json: JsonLimits,
}

/// Exact current public-projection scope; CMP custody is never a rights grant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogDisclosureScope {
    pub operation_id: String,
    pub carrier_layer: String,
    pub intended_use: String,
    pub selected_model_receipt_id: String,
    pub source_cut: String,
    pub through_commit_seq: u64,
    pub source_membership_root: Digest256,
    pub descriptor_sha256: Digest256,
    pub selected_index_sha256: Digest256,
    pub catalog_packet_sha256: Digest256,
    pub policy_issuer_ref: String,
    pub policy_receipt_id: String,
    pub policy_scope: String,
    pub policy_epoch: String,
    pub withdrawal_generation: String,
}

impl CatalogDisclosureScope {
    fn validate(
        &self,
        bound: &BoundCmpKnowledge<'_>,
        policy: &CurrentPolicyBinding,
    ) -> Result<(), CatalogError> {
        let selected = bound.selection();
        if self.operation_id != CATALOG_OPERATION_ID
            || self.carrier_layer != CATALOG_CARRIER_LAYER
            || self.intended_use != CATALOG_INTENDED_USE
            || self.selected_model_receipt_id != bound.owner_receipt_id()
            || self.source_cut != selected.source_cut
            || self.through_commit_seq != selected.through_commit_seq
            || self.source_membership_root != selected.source_membership_root
            || self.descriptor_sha256 != selected.vocabulary.descriptor_sha256
            || self.selected_index_sha256 != selected.index_root_sha256
            || self.catalog_packet_sha256 != selected.catalog_packet_sha256
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
                CatalogErrorCode::PolicyBindingUnavailable,
                "catalog disclosure scope unavailable",
            ));
        }
        Ok(())
    }
}

/// Must serialize publication withdrawal until the final transport flush.
pub trait CatalogDisclosureLease: Send {
    fn recheck(&mut self) -> Result<(), CatalogError>;
}

/// The ToS publication owner supplies current authorization for the exact
/// selected public catalog carrier. A synthetic implementation is test-only.
pub trait CatalogCurrentAuthority {
    fn abort_probe(&self) -> Option<Arc<dyn crate::AbortProbe>> { None }
    fn policy_binding(&self) -> CurrentPolicyBinding;
    fn disclosure_scope(&self) -> CatalogDisclosureScope;
    fn check_selected(&mut self) -> Result<(), CatalogError>;
    fn authorize_current(&mut self, packet_sha256: Digest256) -> Result<(), CatalogError>;
    fn acquire_disclosure(
        &mut self,
        scope: &CatalogDisclosureScope,
        packet_sha256: Digest256,
    ) -> Result<Box<dyn CatalogDisclosureLease>, CatalogError>;
}

pub struct DisclosableCatalog {
    body: Vec<u8>,
    lease: Box<dyn CatalogDisclosureLease>,
}

impl Deref for DisclosableCatalog {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        &self.body
    }
}

impl DisclosableCatalog {
    /// Move the authenticated bytes and held lease into a transport packet.
    /// The adapter must retain and recheck the lease through the final flush.
    pub fn into_parts(self) -> (Vec<u8>, Box<dyn CatalogDisclosureLease>) {
        (self.body, self.lease)
    }
    pub fn recheck(&mut self) -> Result<(), CatalogError> {
        self.lease.recheck()
    }
}

fn sql_error(reason: rusqlite::Error) -> CatalogError {
    if matches!(reason, rusqlite::Error::SqliteFailure(failure, _) if failure.code == ErrorCode::OperationInterrupted)
    {
        error(
            CatalogErrorCode::BudgetExceeded,
            "catalog SQLite VM budget exceeded",
        )
    } else {
        error(
            CatalogErrorCode::CorruptSelectedCarrier,
            "selected catalog lookup failed",
        )
    }
}

/// Return the exact compatibility packet only if its complete selected bytes
/// fit the caller's budget. No row is returned when any size/type cap fails.
pub fn execute_selected_catalog<A: CatalogCurrentAuthority + ?Sized>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    budget: CatalogBudget,
) -> Result<DisclosableCatalog, CatalogError> {
    if budget.max_open_vm_steps == 0
        || model.open_vm_steps() > budget.max_open_vm_steps
        || budget.max_read_vm_steps == 0
        || budget.max_packet_bytes == 0
        || budget.max_decoded_bytes < budget.max_packet_bytes.saturating_add(32)
        || budget.json.max_bytes < budget.max_packet_bytes
        || budget.max_packet_bytes > i64::MAX as usize
    {
        return Err(error(
            CatalogErrorCode::BudgetExceeded,
            "catalog admission unavailable",
        ));
    }
    bound.check_model(model).map_err(|_| {
        error(
            CatalogErrorCode::StaleSelection,
            "selected catalog binding changed",
        )
    })?;
    let policy = authority.policy_binding();
    let abort = authority.abort_probe();
    let check_abort = || match abort.as_ref().and_then(|probe| probe.reason()) {
        Some(crate::AbortReason::Cancelled) => Err(error(CatalogErrorCode::Cancelled, "catalog query cancelled")),
        Some(crate::AbortReason::DeadlineExceeded) => Err(error(CatalogErrorCode::DeadlineExceeded, "catalog query deadline exceeded")),
        None => Ok(()),
    };
    check_abort()?;
    let scope = authority.disclosure_scope();
    scope.validate(bound, &policy)?;
    authority.check_selected()?;
    if let Some(proof) = bound.source_basis().managed_source() { authority.authorize_managed_source_current(proof)?; }

    let connection = model.connection();
    let count = Arc::new(AtomicU64::new(0));
    let observed = Arc::clone(&count);
    let cap = budget.max_read_vm_steps;
    let vm_abort = abort.clone();
    connection.progress_handler(
        1,
        Some(move || vm_abort.as_ref().is_some_and(|probe| probe.reason().is_some()) || observed.fetch_add(1, Ordering::Relaxed) >= cap),
    );
    let selected = (|| {
        let mut statement = connection.prepare_cached(
            "SELECT packet_len,
                    CASE WHEN typeof(packet_sha256)='blob' AND length(packet_sha256)=32 THEN packet_sha256 ELSE NULL END,
                    CASE WHEN typeof(packet)='blob' AND packet_len BETWEEN 0 AND ?2
                              AND length(packet)=packet_len THEN packet ELSE NULL END
             FROM catalog_index_meta WHERE descriptor_sha256=?1"
        ).map_err(sql_error)?;
        statement
            .query_row(
                params![
                    bound.selection().vocabulary.descriptor_sha256.to_hex(),
                    budget.max_packet_bytes as i64
                ],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, Option<Vec<u8>>>(1)?,
                        row.get::<_, Option<Vec<u8>>>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(sql_error)
    })();
    connection.progress_handler(0, None::<fn() -> bool>);
    check_abort()?;
    if count.load(Ordering::Relaxed) > budget.max_read_vm_steps {
        return Err(error(
            CatalogErrorCode::BudgetExceeded,
            "catalog lookup exceeded VM budget",
        ));
    }
    let (length, raw_sha, body) = selected?.ok_or_else(|| {
        error(
            CatalogErrorCode::CorruptSelectedCarrier,
            "selected catalog row missing",
        )
    })?;
    if length < 0 || length as usize > budget.max_packet_bytes {
        return Err(error(
            CatalogErrorCode::BudgetExceeded,
            "selected catalog packet exceeds cap",
        ));
    }
    let raw_sha = raw_sha.ok_or_else(|| {
        error(
            CatalogErrorCode::CorruptSelectedCarrier,
            "selected catalog digest width differs",
        )
    })?;
    let body = body.ok_or_else(|| {
        error(
            CatalogErrorCode::CorruptSelectedCarrier,
            "selected catalog packet type or length differs",
        )
    })?;
    if body.len() != length as usize
        || body.len().saturating_add(raw_sha.len()) > budget.max_decoded_bytes
        || Digest256::of_bytes(&body) != bound.selection().catalog_packet_sha256
        || raw_sha.as_slice() != Digest256::of_bytes(&body).as_bytes()
    {
        return Err(error(
            CatalogErrorCode::CorruptSelectedCarrier,
            "selected catalog packet digest differs",
        ));
    }
    let mut limits = budget.json;
    limits.max_bytes = limits.max_bytes.min(body.len());
    let parsed = parse_json(&body, JsonMode::PublishedStrict, limits).map_err(|_| {
        error(
            CatalogErrorCode::CorruptSelectedCarrier,
            "selected catalog JSON invalid",
        )
    })?;
    let root = parsed.root();
    bound.validate_catalog_identity(root, budget.json).map_err(|reason| error(
        if reason.code == crate::search_v2::SearchV2ErrorCode::BudgetExceeded { CatalogErrorCode::BudgetExceeded }
        else { CatalogErrorCode::CorruptSelectedCarrier },
        "selected catalog identity differs",
    ))?;
    authority.authorize_current(bound.selection().catalog_packet_sha256)?;
    authority.check_selected()?;
    bound.check_model(model).map_err(|_| {
        error(
            CatalogErrorCode::StaleSelection,
            "selected catalog binding changed",
        )
    })?;
    if let Some(proof) = bound.source_basis().managed_source() { authority.authorize_managed_source_current(proof)?; }
    let mut lease =
        authority.acquire_disclosure(&scope, bound.selection().catalog_packet_sha256)?;
    lease.recheck()?;
    check_abort()?;
    Ok(DisclosableCatalog { body, lease })
}
