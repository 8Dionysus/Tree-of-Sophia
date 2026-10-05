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
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonMode, JsonNumber, JsonNumberKind, JsonString,
    JsonValue, canonical_bytes_v1, parse_json,
};

use crate::knowledge_inspect::InspectVisitMeter;
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
/// An owned provider may support 'static; a scoped provider's returned lease
/// and packet retain exactly its held-read lifetime through final delivery.
pub trait CatalogCurrentAuthority<'hold> {
    /// Match the descriptive proof to the command owner's opaque parent and
    /// already-held current guards. Catalog authorization remains separate;
    /// the same disclosure lease covers both checks through final delivery.
    fn authorize_managed_source_current(
        &mut self,
        _: &tos_compiler::ManagedSourceProofV1,
    ) -> Result<(), CatalogError> {
        Err(error(
            CatalogErrorCode::PolicyBindingUnavailable,
            "managed selected source authorization unavailable",
        ))
    }
    fn authorize_managed_source_v2_current(
        &mut self,
        _: &tos_compiler::ManagedSourceProofV2,
    ) -> Result<(), CatalogError> {
        Err(error(
            CatalogErrorCode::PolicyBindingUnavailable,
            "managed selected source authorization unavailable",
        ))
    }
    fn abort_probe(&self) -> Option<Arc<dyn crate::AbortProbe>> {
        None
    }
    fn policy_binding(&self) -> CurrentPolicyBinding;
    fn disclosure_scope(&self) -> CatalogDisclosureScope;
    fn check_selected(&mut self) -> Result<(), CatalogError>;
    fn authorize_current(&mut self, packet_sha256: Digest256) -> Result<(), CatalogError>;
    fn acquire_disclosure(
        &mut self,
        scope: &CatalogDisclosureScope,
        packet_sha256: Digest256,
    ) -> Result<Box<dyn CatalogDisclosureLease + 'hold>, CatalogError>;
}

pub struct DisclosableCatalog<'hold> {
    body: Vec<u8>,
    lease: Box<dyn CatalogDisclosureLease + 'hold>,
}

impl Deref for DisclosableCatalog<'_> {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        &self.body
    }
}

impl<'hold> DisclosableCatalog<'hold> {
    /// Move the authenticated bytes and held lease into a transport packet.
    /// The adapter must retain and recheck the lease through the final flush.
    pub fn into_parts(self) -> (Vec<u8>, Box<dyn CatalogDisclosureLease + 'hold>) {
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
pub fn execute_selected_catalog<'hold, A: CatalogCurrentAuthority<'hold> + ?Sized>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    budget: CatalogBudget,
) -> Result<DisclosableCatalog<'hold>, CatalogError> {
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
        Some(crate::AbortReason::Cancelled) => Err(error(
            CatalogErrorCode::Cancelled,
            "catalog query cancelled",
        )),
        Some(crate::AbortReason::DeadlineExceeded) => Err(error(
            CatalogErrorCode::DeadlineExceeded,
            "catalog query deadline exceeded",
        )),
        None => Ok(()),
    };
    check_abort()?;
    let scope = authority.disclosure_scope();
    scope.validate(bound, &policy)?;
    authority.check_selected()?;
    if let Some(proof) = bound.source_basis().managed_source() {
        authority.authorize_managed_source_current(proof)?;
    }
    if let Some(proof) = bound.source_basis().managed_source_v2() {
        authority.authorize_managed_source_v2_current(proof)?;
    }

    let connection = model.connection();
    let count = Arc::new(AtomicU64::new(0));
    let observed = Arc::clone(&count);
    let cap = budget.max_read_vm_steps;
    let vm_abort = abort.clone();
    connection.progress_handler(
        1,
        Some(move || {
            vm_abort
                .as_ref()
                .is_some_and(|probe| probe.reason().is_some())
                || observed.fetch_add(1, Ordering::Relaxed) >= cap
        }),
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
    bound
        .validate_catalog_identity(root, budget.json)
        .map_err(|reason| {
            error(
                if reason.code == crate::search_v2::SearchV2ErrorCode::BudgetExceeded {
                    CatalogErrorCode::BudgetExceeded
                } else {
                    CatalogErrorCode::CorruptSelectedCarrier
                },
                "selected catalog identity differs",
            )
        })?;
    authority.authorize_current(bound.selection().catalog_packet_sha256)?;
    authority.check_selected()?;
    bound.check_model(model).map_err(|_| {
        error(
            CatalogErrorCode::StaleSelection,
            "selected catalog binding changed",
        )
    })?;
    if let Some(proof) = bound.source_basis().managed_source() {
        authority.authorize_managed_source_current(proof)?;
    }
    if let Some(proof) = bound.source_basis().managed_source_v2() {
        authority.authorize_managed_source_v2_current(proof)?;
    }
    let mut lease =
        authority.acquire_disclosure(&scope, bound.selection().catalog_packet_sha256)?;
    lease.recheck()?;
    check_abort()?;
    Ok(DisclosableCatalog { body, lease })
}

fn health_text(value: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(value))
}
fn health_number(value: u64) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: value.to_string(),
    })
}
fn health_object(fields: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        fields
            .into_iter()
            .map(|(key, value)| (JsonString::from_utf8(key), value))
            .collect(),
    )
}
fn health_dynamic_object(fields: Vec<(String, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        fields
            .into_iter()
            .map(|(key, value)| (JsonString::from_utf8(&key), value))
            .collect(),
    )
}
fn health_exact_object(
    value: &JsonValue,
    keys: &[&str],
    optional_keys: &[&str],
    field: &'static str,
) -> Result<(), CatalogError> {
    let object = value.as_object().ok_or_else(|| {
        error(
            CatalogErrorCode::CorruptSelectedCarrier,
            "selected graph health count object is invalid",
        )
    })?;
    if keys.iter().any(|key| value.object_get(key).is_none())
        || object.iter().any(|(key, _)| {
            key.as_str()
                .is_none_or(|key| !keys.contains(&key) && !optional_keys.contains(&key))
        })
    {
        return Err(error(CatalogErrorCode::CorruptSelectedCarrier, field));
    }
    Ok(())
}
fn validate_health_graph_counts(counts: &JsonValue) -> Result<(), CatalogError> {
    health_exact_object(
        counts,
        &[
            "nodes",
            "relations",
            "sources",
            "display_coverage",
            "semantic_mapping",
        ],
        &["semantic_validation"],
        "selected graph count schema differs",
    )
}

// Move the bounded, parsed owner report into the response. This does not
// rerun semantic assessment or promote its meaning to source/canon authority.
fn take_health_semantic_validation(
    header: &mut JsonValue,
) -> Result<Option<JsonValue>, CatalogError> {
    let JsonValue::Object(fields) = header else {
        return Ok(None);
    };
    let Some((_, JsonValue::Object(counts))) = fields
        .iter_mut()
        .find(|(key, _)| key.as_str() == Some("counts"))
    else {
        return Ok(None);
    };
    let Some(index) = counts
        .iter()
        .position(|(key, _)| key.as_str() == Some("semantic_validation"))
    else {
        return Ok(None);
    };
    let report = &counts[index].1;
    if report.as_object().is_none()
        || report
            .object_get("valid")
            .and_then(JsonValue::as_bool)
            .is_none()
        || report
            .object_get("violations")
            .and_then(JsonValue::as_array)
            .is_none_or(|items| items.iter().any(|item| item.as_str().is_none()))
    {
        return Err(error(
            CatalogErrorCode::CorruptSelectedCarrier,
            "selected graph semantic validation report differs",
        ));
    }
    Ok(Some(counts.swap_remove(index).1))
}

fn health_fixed_counts(
    value: &JsonValue,
    keys: &[&'static str],
    message: &'static str,
) -> Result<JsonValue, CatalogError> {
    health_exact_object(value, keys, &[], message)?;
    let fields = keys
        .iter()
        .map(|key| {
            let count = value
                .object_get(key)
                .and_then(JsonValue::as_u64)
                .ok_or_else(|| error(CatalogErrorCode::CorruptSelectedCarrier, message))?;
            Ok((*key, health_number(count)))
        })
        .collect::<Result<Vec<_>, CatalogError>>()?;
    Ok(health_object(fields))
}
fn health_required_count(
    value: &JsonValue,
    key: &str,
    message: &'static str,
) -> Result<u64, CatalogError> {
    value
        .object_get(key)
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| error(CatalogErrorCode::CorruptSelectedCarrier, message))
}
fn health_count_map(
    value: &JsonValue,
    maximum_entries: usize,
    maximum_key_bytes: usize,
    allowed_keys: Option<&[String]>,
) -> Result<JsonValue, CatalogError> {
    let fields = value.as_object().ok_or_else(|| {
        error(
            CatalogErrorCode::CorruptSelectedCarrier,
            "selected graph health count map is invalid",
        )
    })?;
    if fields.len() > maximum_entries {
        return Err(error(
            CatalogErrorCode::BudgetExceeded,
            "selected graph health count map exceeds visit budget",
        ));
    }
    let mut copied = Vec::with_capacity(fields.len());
    for (key, value) in fields {
        let name = key.as_str().ok_or_else(|| {
            error(
                CatalogErrorCode::CorruptSelectedCarrier,
                "selected graph health count key is invalid",
            )
        })?;
        if name.len() > maximum_key_bytes
            || allowed_keys.is_some_and(|allowed| {
                allowed
                    .binary_search_by(|item| item.as_str().cmp(name))
                    .is_err()
            })
        {
            return Err(error(
                CatalogErrorCode::CorruptSelectedCarrier,
                "selected graph health count key is outside its selected vocabulary",
            ));
        }
        let count = value.as_u64().ok_or_else(|| {
            error(
                CatalogErrorCode::CorruptSelectedCarrier,
                "selected graph health count map value is invalid",
            )
        })?;
        copied.push((name.to_owned(), health_number(count)));
    }
    Ok(health_dynamic_object(copied))
}
fn health_packet_row_by_key(
    model: &VerifiedKnowledgeModel<'_>,
    sql: &str,
    key: &str,
    packet_cap: usize,
) -> Result<Option<(Option<i64>, Option<Vec<u8>>, Option<Vec<u8>>)>, CatalogError> {
    let mut statement = model.connection().prepare_cached(sql).map_err(sql_error)?;
    statement
        .query_row(params![key, packet_cap as i64], |row| {
            Ok((
                row.get::<_, Option<i64>>(0)?,
                row.get::<_, Option<Vec<u8>>>(1)?,
                row.get::<_, Option<Vec<u8>>>(2)?,
            ))
        })
        .optional()
        .map_err(sql_error)
}
fn health_graph_header_row(
    model: &VerifiedKnowledgeModel<'_>,
    packet_cap: usize,
) -> Result<Option<(Option<i64>, Option<Vec<u8>>, Option<Vec<u8>>)>, CatalogError> {
    let mut statement = model
        .connection()
        .prepare_cached(
            "SELECT CASE WHEN typeof(packet_len)='integer' THEN packet_len ELSE NULL END,
                    CASE WHEN typeof(packet_sha256)='blob' AND length(packet_sha256)=32 THEN packet_sha256 ELSE NULL END,
                    CASE WHEN typeof(packet)='blob' AND packet_len BETWEEN 0 AND ?1
                              AND length(packet)=packet_len THEN packet ELSE NULL END
             FROM graph_header WHERE singleton=1",
        )
        .map_err(sql_error)?;
    statement
        .query_row(params![packet_cap as i64], |row| {
            Ok((
                row.get::<_, Option<i64>>(0)?,
                row.get::<_, Option<Vec<u8>>>(1)?,
                row.get::<_, Option<Vec<u8>>>(2)?,
            ))
        })
        .optional()
        .map_err(sql_error)
}

/// Typed health metadata from the exact selected graph header and catalog.
/// The raw graph header stays private; only its schema and complete validated
/// aggregate counts are returned under existing catalog authority and lease.
pub fn execute_selected_knowledge_health_metadata<
    'hold,
    A: CatalogCurrentAuthority<'hold> + ?Sized,
>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    budget: CatalogBudget,
    meter: &mut InspectVisitMeter,
) -> Result<DisclosableCatalog<'hold>, CatalogError> {
    if budget.max_open_vm_steps == 0
        || model.open_vm_steps() > budget.max_open_vm_steps
        || budget.max_read_vm_steps == 0
        || budget.max_packet_bytes == 0
        || budget.max_decoded_bytes < 80
        || budget.json.max_bytes < budget.max_packet_bytes
        || budget.max_packet_bytes > i64::MAX as usize
        || budget.json.max_visits < 6
    {
        return Err(error(
            CatalogErrorCode::BudgetExceeded,
            "catalog health metadata admission unavailable",
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
        Some(crate::AbortReason::Cancelled) => Err(error(
            CatalogErrorCode::Cancelled,
            "catalog health query cancelled",
        )),
        Some(crate::AbortReason::DeadlineExceeded) => Err(error(
            CatalogErrorCode::DeadlineExceeded,
            "catalog health query deadline exceeded",
        )),
        None => Ok(()),
    };
    check_abort()?;
    let scope = authority.disclosure_scope();
    scope.validate(bound, &policy)?;
    authority.check_selected()?;
    if let Some(proof) = bound.source_basis().managed_source() {
        authority.authorize_managed_source_current(proof)?;
    }
    if let Some(proof) = bound.source_basis().managed_source_v2() {
        authority.authorize_managed_source_v2_current(proof)?;
    }

    let connection = model.connection();
    let steps = Arc::new(AtomicU64::new(0));
    let observed = Arc::clone(&steps);
    let cap = budget.max_read_vm_steps;
    let vm_abort = abort.clone();
    connection.progress_handler(
        1,
        Some(move || {
            vm_abort
                .as_ref()
                .is_some_and(|probe| probe.reason().is_some())
                || observed.fetch_add(1, Ordering::Relaxed) >= cap
        }),
    );
    let selected = (|| {
        let mut decoded = 0usize;
        let read_cap = |decoded: usize| {
            budget
                .max_decoded_bytes
                .saturating_sub(decoded)
                .saturating_sub(40)
                .min(budget.max_packet_bytes)
        };
        let catalog_cap = read_cap(decoded);
        if catalog_cap == 0 {
            return Err(error(
                CatalogErrorCode::BudgetExceeded,
                "selected catalog health metadata exceeds decoded byte cap",
            ));
        }
        let catalog_row = health_packet_row_by_key(
            model,
            "SELECT CASE WHEN typeof(packet_len)='integer' THEN packet_len ELSE NULL END,
                    CASE WHEN typeof(packet_sha256)='blob' AND length(packet_sha256)=32 THEN packet_sha256 ELSE NULL END,
                    CASE WHEN typeof(packet)='blob' AND packet_len BETWEEN 0 AND ?2
                              AND length(packet)=packet_len THEN packet ELSE NULL END
             FROM catalog_index_meta WHERE descriptor_sha256=?1",
            &bound.selection().vocabulary.descriptor_sha256.to_hex(),
            catalog_cap,
        )?
        .ok_or_else(|| {
            error(
                CatalogErrorCode::CorruptSelectedCarrier,
                "selected catalog row missing",
            )
        })?;
        let (catalog_length, catalog_sha, catalog_body) = catalog_row;
        let catalog_length = catalog_length
            .filter(|length| *length >= 0)
            .ok_or_else(|| {
                error(
                    CatalogErrorCode::CorruptSelectedCarrier,
                    "selected catalog length invalid",
                )
            })? as usize;
        let catalog_sha = catalog_sha.ok_or_else(|| {
            error(
                CatalogErrorCode::CorruptSelectedCarrier,
                "selected catalog digest width differs",
            )
        })?;
        let catalog_body = catalog_body.ok_or_else(|| {
            error(
                CatalogErrorCode::BudgetExceeded,
                "selected catalog packet type, length or cap differs",
            )
        })?;
        if catalog_length > catalog_cap
            || catalog_body.len() != catalog_length
            || Digest256::of_bytes(&catalog_body) != bound.selection().catalog_packet_sha256
            || catalog_sha.as_slice() != Digest256::of_bytes(&catalog_body).as_bytes()
        {
            return Err(error(
                CatalogErrorCode::CorruptSelectedCarrier,
                "selected catalog packet digest differs",
            ));
        }
        decoded = decoded
            .checked_add(catalog_body.len())
            .and_then(|total| total.checked_add(catalog_sha.len()))
            .ok_or_else(|| {
                error(
                    CatalogErrorCode::BudgetExceeded,
                    "selected catalog decoded byte count overflow",
                )
            })?;
        if decoded > budget.max_decoded_bytes {
            return Err(error(
                CatalogErrorCode::BudgetExceeded,
                "selected catalog packet exceeds decoded byte cap",
            ));
        }

        let header_cap = read_cap(decoded);
        if header_cap == 0 {
            return Err(error(
                CatalogErrorCode::BudgetExceeded,
                "selected graph header exceeds decoded byte cap",
            ));
        }
        let header_row = health_graph_header_row(model, header_cap)?.ok_or_else(|| {
            error(
                CatalogErrorCode::CorruptSelectedCarrier,
                "selected graph header missing",
            )
        })?;
        let (header_length, header_sha, header_body) = header_row;
        let header_length = header_length.filter(|length| *length >= 0).ok_or_else(|| {
            error(
                CatalogErrorCode::CorruptSelectedCarrier,
                "selected graph header length invalid",
            )
        })? as usize;
        let header_sha = header_sha.ok_or_else(|| {
            error(
                CatalogErrorCode::CorruptSelectedCarrier,
                "selected graph header digest width differs",
            )
        })?;
        let header_body = header_body.ok_or_else(|| {
            error(
                CatalogErrorCode::BudgetExceeded,
                "selected graph header type, length or cap differs",
            )
        })?;
        if header_length > header_cap
            || header_body.len() != header_length
            || header_sha.as_slice() != Digest256::of_bytes(&header_body).as_bytes()
        {
            return Err(error(
                CatalogErrorCode::CorruptSelectedCarrier,
                "selected graph header digest differs",
            ));
        }
        decoded = decoded
            .checked_add(header_body.len())
            .and_then(|total| total.checked_add(header_sha.len()))
            .ok_or_else(|| {
                error(
                    CatalogErrorCode::BudgetExceeded,
                    "selected health metadata decoded byte count overflow",
                )
            })?;
        if decoded > budget.max_decoded_bytes {
            return Err(error(
                CatalogErrorCode::BudgetExceeded,
                "selected health metadata exceeds decoded byte cap",
            ));
        }

        let mut catalog_limits = budget.json;
        catalog_limits.max_bytes = catalog_limits.max_bytes.min(catalog_body.len());
        let catalog = meter
            .parse_json(&catalog_body, JsonMode::PublishedStrict, catalog_limits)
            .map_err(|_| {
                error(
                    CatalogErrorCode::CorruptSelectedCarrier,
                    "selected catalog JSON invalid",
                )
            })?
            .into_root();
        bound
            .validate_catalog_identity_metered(&catalog, budget.json, meter)
            .map_err(|reason| {
                error(
                    if reason.code == crate::search_v2::SearchV2ErrorCode::BudgetExceeded {
                        CatalogErrorCode::BudgetExceeded
                    } else {
                        CatalogErrorCode::CorruptSelectedCarrier
                    },
                    "selected catalog identity differs",
                )
            })?;
        let graph_limits = {
            let mut limits = budget.json;
            limits.max_bytes = limits.max_bytes.min(header_body.len());
            limits
        };
        let mut header = meter
            .parse_json(&header_body, JsonMode::PublishedStrict, graph_limits)
            .map_err(|_| {
                error(
                    CatalogErrorCode::CorruptSelectedCarrier,
                    "selected graph header JSON invalid",
                )
            })?
            .into_root();
        if header.as_object().is_none() || catalog.as_object().is_none() {
            return Err(error(
                CatalogErrorCode::CorruptSelectedCarrier,
                "selected health metadata object invalid",
            ));
        }
        let graph_schema = header
            .object_get("schema")
            .and_then(JsonValue::as_str)
            .filter(|value| !value.is_empty() && value.len() <= budget.max_packet_bytes)
            .ok_or_else(|| {
                error(
                    CatalogErrorCode::CorruptSelectedCarrier,
                    "selected graph schema is absent or invalid",
                )
            })?;
        let catalog_schema = catalog
            .object_get("schema")
            .and_then(JsonValue::as_str)
            .filter(|value| !value.is_empty() && value.len() <= budget.max_packet_bytes)
            .ok_or_else(|| {
                error(
                    CatalogErrorCode::CorruptSelectedCarrier,
                    "selected catalog schema is absent or invalid",
                )
            })?;
        let counts = header.object_get("counts").ok_or_else(|| {
            error(
                CatalogErrorCode::CorruptSelectedCarrier,
                "selected graph counts are absent",
            )
        })?;
        validate_health_graph_counts(counts)?;
        let display_coverage = counts.object_get("display_coverage").ok_or_else(|| {
            error(
                CatalogErrorCode::CorruptSelectedCarrier,
                "selected graph display coverage is absent",
            )
        })?;
        health_exact_object(
            display_coverage,
            &[
                "node_titles",
                "node_summaries",
                "node_summary_states",
                "nodes_without_source_summary",
                "relation_labels",
                "relation_statements",
                "relation_explanations",
                "relation_explanation_states",
                "relations_without_source_explanation",
            ],
            &[],
            "selected graph display coverage schema differs",
        )?;
        let semantic_mapping = counts.object_get("semantic_mapping").ok_or_else(|| {
            error(
                CatalogErrorCode::CorruptSelectedCarrier,
                "selected graph semantic mapping is absent",
            )
        })?;
        let semantic_mapping = health_fixed_counts(
            semantic_mapping,
            &[
                "mapped_nodes",
                "unmapped_nodes",
                "mapped_relations",
                "unmapped_relations",
                "cross_layer_relations",
            ],
            "selected graph semantic mapping schema or count differs",
        )?;
        let nodes = health_required_count(
            counts,
            "nodes",
            "selected graph node count is absent or invalid",
        )?;
        let relations = health_required_count(
            counts,
            "relations",
            "selected graph relation count is absent or invalid",
        )?;
        let source_ids = bound.vocabulary().registered_source_ids.as_slice();
        let sources = health_count_map(
            counts.object_get("sources").ok_or_else(|| {
                error(
                    CatalogErrorCode::CorruptSelectedCarrier,
                    "selected graph source counts are absent",
                )
            })?,
            source_ids.len(),
            budget.max_packet_bytes,
            Some(source_ids),
        )?;
        let node_summary_states = health_count_map(
            display_coverage
                .object_get("node_summary_states")
                .ok_or_else(|| {
                    error(
                        CatalogErrorCode::CorruptSelectedCarrier,
                        "selected node summary states are absent",
                    )
                })?,
            budget.json.max_visits,
            budget.max_packet_bytes,
            None,
        )?;
        let relation_explanation_states = health_count_map(
            display_coverage
                .object_get("relation_explanation_states")
                .ok_or_else(|| {
                    error(
                        CatalogErrorCode::CorruptSelectedCarrier,
                        "selected relation explanation states are absent",
                    )
                })?,
            budget.json.max_visits,
            budget.max_packet_bytes,
            None,
        )?;
        let display_coverage = health_object(vec![
            (
                "node_titles",
                health_number(health_required_count(
                    display_coverage,
                    "node_titles",
                    "selected graph display coverage count is absent or invalid",
                )?),
            ),
            (
                "node_summaries",
                health_number(health_required_count(
                    display_coverage,
                    "node_summaries",
                    "selected graph display coverage count is absent or invalid",
                )?),
            ),
            ("node_summary_states", node_summary_states),
            (
                "nodes_without_source_summary",
                health_number(health_required_count(
                    display_coverage,
                    "nodes_without_source_summary",
                    "selected graph display coverage count is absent or invalid",
                )?),
            ),
            (
                "relation_labels",
                health_number(health_required_count(
                    display_coverage,
                    "relation_labels",
                    "selected graph display coverage count is absent or invalid",
                )?),
            ),
            (
                "relation_statements",
                health_number(health_required_count(
                    display_coverage,
                    "relation_statements",
                    "selected graph display coverage count is absent or invalid",
                )?),
            ),
            (
                "relation_explanations",
                health_number(health_required_count(
                    display_coverage,
                    "relation_explanations",
                    "selected graph display coverage count is absent or invalid",
                )?),
            ),
            ("relation_explanation_states", relation_explanation_states),
            (
                "relations_without_source_explanation",
                health_number(health_required_count(
                    display_coverage,
                    "relations_without_source_explanation",
                    "selected graph display coverage count is absent or invalid",
                )?),
            ),
        ]);
        // Native and Python graph owners attach semantic_validation when
        // semantic registries are selected. Preserve that owner report rather
        // than treating this declared optional count companion as corruption.
        let graph_schema = health_text(graph_schema);
        let catalog_schema = health_text(catalog_schema);
        let semantic_validation = take_health_semantic_validation(&mut header)?;
        let mut count_fields = vec![
            ("nodes", health_number(nodes)),
            ("relations", health_number(relations)),
            ("sources", sources),
            ("display_coverage", display_coverage),
            ("semantic_mapping", semantic_mapping),
        ];
        if let Some(report) = semantic_validation {
            count_fields.push(("semantic_validation", report));
        }
        let counts = health_object(count_fields);
        let output = health_object(vec![
            (
                "schema_version",
                health_text("tos_selected_knowledge_health_metadata_v1"),
            ),
            ("graph_schema", graph_schema),
            ("catalog_schema", catalog_schema),
            ("counts", counts),
        ]);
        let mut output_limits = budget.json;
        output_limits.max_bytes = output_limits.max_bytes.min(budget.max_packet_bytes);
        let body = meter
            .canonical_bytes(
                &output,
                CanonicalProfile::SourceRecordDigestV1,
                output_limits,
            )
            .map_err(|_| {
                error(
                    CatalogErrorCode::BudgetExceeded,
                    "selected health metadata packet exceeds budget",
                )
            })?;
        if body.len() > budget.max_packet_bytes {
            return Err(error(
                CatalogErrorCode::BudgetExceeded,
                "selected health metadata packet exceeds response cap",
            ));
        }
        Ok(body)
    })();
    connection.progress_handler(0, None::<fn() -> bool>);
    check_abort()?;
    if steps.load(Ordering::Relaxed) > budget.max_read_vm_steps {
        return Err(error(
            CatalogErrorCode::BudgetExceeded,
            "catalog health metadata lookup exceeded VM budget",
        ));
    }
    let body = selected?;
    authority.authorize_current(bound.selection().catalog_packet_sha256)?;
    authority.check_selected()?;
    bound.check_model(model).map_err(|_| {
        error(
            CatalogErrorCode::StaleSelection,
            "selected catalog binding changed",
        )
    })?;
    if let Some(proof) = bound.source_basis().managed_source() {
        authority.authorize_managed_source_current(proof)?;
    }
    if let Some(proof) = bound.source_basis().managed_source_v2() {
        authority.authorize_managed_source_v2_current(proof)?;
    }
    let mut lease =
        authority.acquire_disclosure(&scope, bound.selection().catalog_packet_sha256)?;
    lease.recheck()?;
    check_abort()?;
    Ok(DisclosableCatalog { body, lease })
}

#[cfg(test)]
mod health_count_tests {
    use super::*;

    fn header(report: Option<serde_json::Value>) -> JsonValue {
        let mut counts = serde_json::json!({"nodes":3,"relations":2,"sources":{},
            "display_coverage":{},"semantic_mapping":{}});
        if let Some(report) = report {
            counts["semantic_validation"] = report;
        }
        let raw = serde_json::to_vec(&serde_json::json!({"counts":counts})).unwrap();
        let header = parse_json(
            &raw,
            JsonMode::PublishedStrict,
            JsonLimits::new(raw.len(), 16, 1000, 20).unwrap(),
        )
        .unwrap()
        .into_root();
        validate_health_graph_counts(header.object_get("counts").unwrap()).unwrap();
        header
    }

    #[test]
    fn semantic_owner_report_is_optional_and_preserved_without_success_filtering() {
        let mut legacy = header(None);
        assert!(
            take_health_semantic_validation(&mut legacy)
                .unwrap()
                .is_none()
        );
        for report in [
            serde_json::json!({"valid":true,"violations":[],"checked_nodes":3,"gaps":[]}),
            serde_json::json!({"valid":false,"violations":["unresolved endpoint"],"checked_nodes":3}),
        ] {
            let mut native = header(Some(report));
            let expected = native
                .object_get("counts")
                .unwrap()
                .object_get("semantic_validation")
                .unwrap()
                .clone();
            assert_eq!(
                take_health_semantic_validation(&mut native).unwrap(),
                Some(expected)
            );
            assert!(
                native
                    .object_get("counts")
                    .unwrap()
                    .object_get("semantic_validation")
                    .is_none()
            );
        }
    }

    #[test]
    fn malformed_semantic_report_remains_a_corrupt_carrier() {
        for report in [
            serde_json::json!(null),
            serde_json::json!({"valid":"true","violations":[]}),
            serde_json::json!({"valid":true,"violations":null}),
            serde_json::json!({"valid":false,"violations":[7]}),
        ] {
            let error = take_health_semantic_validation(&mut header(Some(report))).unwrap_err();
            assert_eq!(error.code, CatalogErrorCode::CorruptSelectedCarrier);
        }
        let value = health_object(vec![
            ("required", health_number(1)),
            ("unknown", health_number(2)),
        ]);
        assert_eq!(
            health_exact_object(&value, &["required"], &["optional"], "unknown count")
                .unwrap_err()
                .code,
            CatalogErrorCode::CorruptSelectedCarrier
        );
        let missing = health_object(vec![("optional", health_number(1))]);
        assert!(
            health_exact_object(&missing, &["required"], &["optional"], "missing count").is_err()
        );
    }
}
