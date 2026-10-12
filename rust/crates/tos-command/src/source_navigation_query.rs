//! Bounded PostgreSQL read model for the current managed source-navigation
//! projection. The selected owner callback retains source, CMD and policy
//! fences through query preparation and the caller's final delivery.

use std::{
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

use postgres::Transaction;
use tos_compiler::VerifiedKnowledgeModel;
use tos_foundation::{Digest256, JsonLimits, JsonMode, parse_json};
use tos_query::{
    AbortProbe, AbortReason, AdjacencyPage, Binding, Charged, CurrentPolicy, DisclosureLease,
    DisclosureScope, ExactNode, QueryError, QueryErrorCode, RawRecord, ReadModel,
};

use crate::{
    DurableNavigationEventLimits, DurableNavigationHeldReadV2,
    DurableNavigationPublishedGenerationGuardV2, DurableProjectionProfile, DurableResult,
    source_managed_selection::{
        ManagedAgentSelectedParent, ManagedCurrentModelLease, ManagedSelectedProof,
        ManagedSelectionError, ManagedSelectionResult,
    },
    source_navigation_policy::{self, SourceNavigationPolicyReceiptV1},
};

const QUERY_TIMEOUT_CAP: Duration = Duration::from_secs(15);
const MAX_ID_BYTES: usize = 256;

fn query_error(code: QueryErrorCode, message: &'static str) -> QueryError {
    QueryError { code, message }
}

fn source_error<P: ManagedSelectedProof>(
    source: &ManagedCurrentModelLease<'_, P>,
    proof: &P,
) -> Result<(), QueryError> {
    source.require_managed_basis(proof).map_err(|_| {
        query_error(
            QueryErrorCode::StaleSelection,
            "managed source current hold changed",
        )
    })
}

fn parse_digest(value: &str) -> Result<Digest256, QueryError> {
    Digest256::from_hex(value.trim_end()).map_err(|_| {
        query_error(
            QueryErrorCode::CorruptSelectedCarrier,
            "navigation row digest is malformed",
        )
    })
}

fn checked_id(value: &str) -> Result<(), QueryError> {
    if value.is_empty() || value.len() > MAX_ID_BYTES || value.as_bytes().contains(&0) {
        return Err(query_error(
            QueryErrorCode::InvalidRequest,
            "source navigation identity is invalid",
        ));
    }
    Ok(())
}

fn checked_u64(value: usize, message: &'static str) -> Result<i64, QueryError> {
    i64::try_from(value).map_err(|_| query_error(QueryErrorCode::BudgetExceeded, message))
}

fn remaining_timeout(deadline: Instant, configured: Duration) -> Result<Duration, QueryError> {
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .ok_or_else(|| {
            query_error(
                QueryErrorCode::DeadlineExceeded,
                "source navigation deadline exceeded",
            )
        })?;
    let bounded = configured.min(QUERY_TIMEOUT_CAP).min(remaining);
    if bounded.is_zero() {
        return Err(query_error(
            QueryErrorCode::DeadlineExceeded,
            "source navigation deadline exceeded",
        ));
    }
    Ok(bounded)
}

fn verify_selected_binding<P: ManagedSelectedProof>(
    model: &VerifiedKnowledgeModel<'_>,
    selected: &ManagedAgentSelectedParent<P>,
    guard: &DurableNavigationPublishedGenerationGuardV2,
    domain: &str,
) -> Result<Binding, QueryError> {
    let owner = guard.owner_binding();
    let expected = model.selection();
    let model_digest = Digest256::from_hex(&expected.model_sha256).map_err(|_| {
        query_error(
            QueryErrorCode::StaleSelection,
            "selected model digest is invalid",
        )
    })?;
    let membership = Digest256::from_hex(&expected.membership_root).map_err(|_| {
        query_error(
            QueryErrorCode::StaleSelection,
            "selected source membership root is invalid",
        )
    })?;
    if owner.domain != domain
        || owner.owner_receipt_id != expected.owner_receipt_id
        || owner.managed_generation_digest != selected.generation().digest()
        || owner.managed_generation_commit_sequence != selected.generation().commit_seq()
        || owner.managed_agent_definition_digest != selected.generation().agent_definition_digest()
        || owner.source_membership_root != membership
        || owner.source_membership_count != selected.generation().member_count()
        || owner.source_projection_digest
            != Digest256::from_hex(&selected.source_proof().inventory_projection()).map_err(
                |_| {
                    query_error(
                        QueryErrorCode::StaleSelection,
                        "managed source projection digest is invalid",
                    )
                },
            )?
        || owner.selected_model_digest != model_digest
        || owner.selected_model_abi != expected.model_abi
        || owner.selected_selection_profile != model.selection_profile()
        || owner.selected_source_cut != expected.source_cut
        || owner.selected_membership_root != membership
        || owner.selected_index_generation != expected.index_generation
        || owner.selected_route_map_version != expected.route_map_version
        || owner.selected_reader_abi != expected.reader_abi
        || owner.selected_authority_boundary != expected.authority_boundary
        || guard.published_sequence() > guard.domain_head_sequence()
    {
        return Err(query_error(
            QueryErrorCode::StaleSelection,
            "published navigation index differs from selected managed model",
        ));
    }
    let binding = Binding {
        source_cut: owner.selected_source_cut.clone(),
        through_commit_seq: guard.published_sequence(),
        membership_root: owner.selected_membership_root,
        projection_root: owner.source_projection_digest,
        index_root: guard.published_root(),
        index_generation: owner.selected_index_generation.clone(),
        route_map_version: owner.selected_route_map_version.clone(),
        reader_abi: owner.selected_reader_abi.clone(),
        model_abi: owner.selected_model_abi.clone(),
        selection_profile: owner.selected_selection_profile.clone(),
    };
    Ok(binding)
}

/// Result of checking the exact last-published source-navigation generation.
/// A lagging consumer remains an explicit non-ready outcome and never invokes
/// the reader callback.
pub enum SourceNavigationPublishedReadV2<T> {
    Ready(T),
    NotReady,
}

/// Execute a read and its consumer callback under the exact managed source
/// hold and the same PostgreSQL repeatable-read transaction that established
/// published-generation readiness. `consume` must finish any transport write
/// before it returns.
#[allow(clippy::too_many_arguments)]
pub fn with_published_source_navigation_v2<'selected, 'request, P, T, E>(
    selected: &'selected ManagedAgentSelectedParent<P>,
    coordinator: &mut crate::DurablePgCoordinator,
    store: &tos_segment_store::SegmentStore,
    filesystem: &crate::source_creation_store::CreationFilesystem,
    contract: Digest256,
    rule_version: u64,
    rights_version: u64,
    job_id: &str,
    job_fence: u64,
    domain: &'request str,
    consumer_id: &'request str,
    projection_profile: &DurableProjectionProfile,
    limits: DurableNavigationEventLimits,
    policy_receipt: &SourceNavigationPolicyReceiptV1,
    policy_receipt_sha256: Digest256,
    owner_configuration_sha256: Digest256,
    deadline: Instant,
    cancelled: &'request AtomicBool,
    consume: impl for<'lease, 'tx_ref, 'tx> FnOnce(
        SourceNavigationPgReadModel<'lease, 'tx_ref, 'tx, 'selected, 'request, P>,
    ) -> Result<T, E>,
) -> ManagedSelectionResult<SourceNavigationPublishedReadV2<Result<T, E>>>
where
    P: ManagedSelectedProof,
{
    let mut limits = limits;
    limits.statement_timeout =
        remaining_timeout(deadline, limits.statement_timeout).map_err(|_| {
            ManagedSelectionError::Durable(crate::DurableError::Refused(
                "source navigation deadline exhausted",
            ))
        })?;
    let result = crate::durable_adapter::durable_projection::with_published_navigation_generation_v2_selected_owner(
        selected,
        coordinator,
        store,
        filesystem,
        contract,
        rule_version,
        rights_version,
        job_id,
        job_fence,
        domain,
        consumer_id,
        projection_profile,
        limits,
        deadline,
        cancelled,
        |model, source, guard, tx| {
            let binding =
                verify_selected_binding(model, selected, guard, domain).map_err(|_| {
                    ManagedSelectionError::Durable(crate::DurableError::Conflict(
                        "published navigation index differs from selected managed model",
                    ))
                })?;
            let selected_export = model
                .navigation_original_receipt()
                .map_err(|_| {
                    ManagedSelectionError::Durable(crate::DurableError::Refused(
                        "selected source export receipt is absent",
                    ))
                })?;
            let expected_export_id = source_navigation_policy::selected_export_receipt_id(
                &selected_export.component_root_sha256,
            )
            .map_err(|_| {
                ManagedSelectionError::Durable(crate::DurableError::Refused(
                    "selected source export receipt is invalid",
                ))
            })?;
            if policy_receipt.export_receipt_id != expected_export_id
                || policy_receipt.model_receipt_id != model.selection().owner_receipt_id
                || policy_receipt.model_abi != model.selection().model_abi
                || policy_receipt.selection_profile != model.selection_profile()
            {
                return Err(ManagedSelectionError::Durable(
                    crate::DurableError::Refused(
                        "source-owner policy receipt differs from selected model",
                    ),
                ));
            }
            let (disclosure_scope, mut policy) = source_navigation_policy::read_locked_v1(
                tx,
                domain,
                consumer_id,
                owner_configuration_sha256,
                policy_receipt,
                policy_receipt_sha256,
                &binding,
                &model.selection().owner_receipt_id,
                &model.selection().source_cut,
            )
            .map_err(|_| {
                ManagedSelectionError::Durable(crate::DurableError::Refused(
                    "current source-owner navigation policy is absent or mismatched",
                ))
            })?;
            disclosure_scope
                .validate_for_selected_model(&binding, &guard.owner_binding().owner_receipt_id)
                .map_err(|_| {
                    ManagedSelectionError::Durable(crate::DurableError::Refused(
                        "source-owner metadata disclosure scope is absent or mismatched",
                    ))
                })?;
            let mut read_model = SourceNavigationPgReadModel {
                tx,
                domain,
                consumer_id,
                guard: guard.clone(),
                binding,
                disclosure_scope,
                policy: Box::new(policy),
                source,
                proof: selected.source_proof(),
                deadline,
                cancelled,
                abort_probe: None,
                limits,
            };
            Ok(consume(read_model))
        },
    )?;
    Ok(match result {
        DurableNavigationHeldReadV2::Ready(value) => SourceNavigationPublishedReadV2::Ready(value),
        DurableNavigationHeldReadV2::NotReady(_) => SourceNavigationPublishedReadV2::NotReady,
    })
}

/// Indexed QRY adapter over the authenticated baseline/event overlay.
/// Construction is private to the owner callback above.
pub struct SourceNavigationPgReadModel<
    'lease,
    'tx_ref,
    'tx,
    'selected,
    'request,
    P: ManagedSelectedProof,
> {
    tx: &'tx_ref mut Transaction<'tx>,
    domain: &'request str,
    consumer_id: &'request str,
    guard: DurableNavigationPublishedGenerationGuardV2,
    binding: Binding,
    disclosure_scope: DisclosureScope,
    policy: Box<dyn CurrentPolicy>,
    source: &'lease ManagedCurrentModelLease<'lease, P>,
    proof: &'selected P,
    deadline: Instant,
    cancelled: &'request AtomicBool,
    abort_probe: Option<Arc<dyn AbortProbe>>,
    limits: DurableNavigationEventLimits,
}

struct ManagedPgDisclosureLease<'lease, 'selected, P: ManagedSelectedProof> {
    inner: Box<dyn DisclosureLease>,
    source: &'lease ManagedCurrentModelLease<'lease, P>,
    proof: &'selected P,
}

impl<P: ManagedSelectedProof> DisclosureLease for ManagedPgDisclosureLease<'_, '_, P> {
    fn recheck(&mut self) -> Result<(), QueryError> {
        self.inner.recheck()?;
        source_error(self.source, self.proof)?;
        self.inner.recheck()?;
        Ok(())
    }
}

impl<P: ManagedSelectedProof> SourceNavigationPgReadModel<'_, '_, '_, '_, '_, P> {
    pub fn set_abort_probe(&mut self, probe: Arc<dyn AbortProbe>) {
        self.abort_probe = Some(probe);
    }

    fn check_current(&self) -> Result<(), QueryError> {
        if self.cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(query_error(
                QueryErrorCode::Cancelled,
                "source navigation cancelled",
            ));
        }
        if Instant::now() >= self.deadline {
            return Err(query_error(
                QueryErrorCode::DeadlineExceeded,
                "source navigation deadline exceeded",
            ));
        }
        if let Some(reason) = self.abort_probe.as_ref().and_then(|probe| probe.reason()) {
            return Err(match reason {
                AbortReason::Cancelled => {
                    query_error(QueryErrorCode::Cancelled, "source navigation cancelled")
                }
                AbortReason::DeadlineExceeded => query_error(
                    QueryErrorCode::DeadlineExceeded,
                    "source navigation deadline exceeded",
                ),
            });
        }
        source_error(self.source, self.proof)
    }

    fn charge(&self, probes: u64, rows: u64, bytes: u64) -> Charged {
        Charged {
            probes,
            rows,
            bytes,
            cpu_steps: 0,
            cpu_meter: tos_query::CpuStepsMeter::Unavailable,
        }
    }

    fn node_lookup(
        &mut self,
        id: &str,
        max_bytes: usize,
        max_carrier_bytes: usize,
        max_work_probes: u64,
        max_work_rows: u64,
    ) -> Result<(Option<RawRecord>, Charged), QueryError> {
        checked_id(id)?;
        if max_work_probes < 1 {
            return Err(query_error(
                QueryErrorCode::BudgetExceeded,
                "source node probe budget exceeded",
            ));
        }
        self.check_current()?;
        let row_limit = i64::try_from(max_work_rows)
            .ok()
            .and_then(|rows| rows.checked_add(1))
            .ok_or_else(|| {
                query_error(
                    QueryErrorCode::BudgetExceeded,
                    "source node row budget overflow",
                )
            })?;
        let carrier_cap = checked_u64(max_carrier_bytes, "source node byte budget overflow")?;
        let candidates = self.tx.query(
            "WITH candidates AS (
                 SELECT row_id AS id,visible,row_sha256,octet_length(row_json)::bigint AS size,row_json
                 FROM cmd2_projection_navigation_bootstrap_row
                 WHERE domain=$1 AND consumer_id=$2 AND row_kind='node' AND active AND row_id=$3
                 UNION ALL
                 SELECT node_id AS id,visible,row_sha256,octet_length(row_json)::bigint AS size,row_json
                 FROM cmd2_projection_navigation_node
                 WHERE domain=$1 AND consumer_id=$2 AND node_id=$3
                 LIMIT $5
             )
             SELECT id,visible,row_sha256,size,
                    CASE WHEN size <= $4 THEN row_json END AS row_json
             FROM candidates ORDER BY id COLLATE \"C\"",
            &[&self.domain, &self.consumer_id, &id, &carrier_cap, &row_limit],
        ).map_err(|_| query_error(QueryErrorCode::CorruptSelectedCarrier, "source node index query failed"))?;
        if candidates.len() as u64 > max_work_rows {
            return Err(query_error(
                QueryErrorCode::BudgetExceeded,
                "source node row budget exceeded",
            ));
        }
        let mut charged_bytes = 0u64;
        let mut found = None;
        let mut effective: Option<(bool, Digest256, Vec<u8>)> = None;
        for row in &candidates {
            let visible: bool = row.get("visible");
            let id_value: String = row.get("id");
            let sha_value: String = row.get("row_sha256");
            let size = usize::try_from(row.get::<_, i64>("size")).map_err(|_| {
                query_error(
                    QueryErrorCode::CorruptSelectedCarrier,
                    "source node carrier size is invalid",
                )
            })?;
            charged_bytes = charged_bytes
                .checked_add(1 + id_value.len() as u64 + 64 + 8)
                .ok_or_else(|| {
                    query_error(
                        QueryErrorCode::BudgetExceeded,
                        "source node byte count overflow",
                    )
                })?;
            if id_value != id || size > self.limits.max_row_bytes || size > max_carrier_bytes {
                return Err(query_error(
                    QueryErrorCode::BudgetExceeded,
                    "source node carrier exceeds byte budget",
                ));
            }
            let raw: Option<String> = row.get("row_json");
            let raw = raw
                .ok_or_else(|| {
                    query_error(
                        QueryErrorCode::BudgetExceeded,
                        "source node carrier exceeds byte budget",
                    )
                })?
                .into_bytes();
            charged_bytes = charged_bytes.checked_add(raw.len() as u64).ok_or_else(|| {
                query_error(
                    QueryErrorCode::BudgetExceeded,
                    "source node byte count overflow",
                )
            })?;
            let sha256 = parse_digest(&sha_value)?;
            if Digest256::of_bytes(&raw) != sha256 {
                return Err(query_error(
                    QueryErrorCode::CorruptSelectedCarrier,
                    "source node digest differs",
                ));
            }
            let value = parse_json(
                &raw,
                JsonMode::PublishedStrict,
                JsonLimits {
                    max_bytes: max_carrier_bytes.max(1),
                    max_depth: 128,
                    max_visits: 200_000,
                    max_integer_digits: 128,
                },
            )
            .map_err(|_| {
                query_error(
                    QueryErrorCode::CorruptSelectedCarrier,
                    "source node JSON is invalid",
                )
            })?;
            if value
                .root()
                .object_get("node_id")
                .and_then(tos_foundation::JsonValue::as_str)
                != Some(id)
            {
                return Err(query_error(
                    QueryErrorCode::CorruptSelectedCarrier,
                    "source node identity differs",
                ));
            }
            if let Some((old_visible, old_sha256, old_raw)) = effective.as_ref() {
                if *old_visible != visible || *old_sha256 != sha256 || old_raw != &raw {
                    return Err(query_error(
                        QueryErrorCode::IndexIncomplete,
                        "source node ID has conflicting projected contributions",
                    ));
                }
            } else {
                effective = Some((visible, sha256, raw.clone()));
            }
            if visible && found.is_none() {
                found = Some(RawRecord {
                    binding: self.binding.clone(),
                    raw,
                    sha256,
                });
            }
        }
        if charged_bytes > max_bytes as u64 {
            return Err(query_error(
                QueryErrorCode::BudgetExceeded,
                "source node result exceeds byte budget",
            ));
        }
        self.check_current()?;
        Ok((
            found,
            self.charge(1, candidates.len() as u64, charged_bytes),
        ))
    }

    fn adjacency_certificate(
        &mut self,
        node_id: &str,
    ) -> Result<(u64, Digest256, u64), QueryError> {
        let cert =
            crate::durable_adapter::durable_projection::read_navigation_adjacency_certificate_v1(
                self.tx,
                self.domain,
                self.consumer_id,
                node_id,
                self.guard.bootstrap_root(),
            )
            .map_err(|_| {
                query_error(
                    QueryErrorCode::IndexIncomplete,
                    "source adjacency certificate unavailable",
                )
            })?
            .ok_or_else(|| {
                query_error(
                    QueryErrorCode::IndexIncomplete,
                    "source adjacency certificate absent",
                )
            })?;
        Ok((cert.edge_count(), cert.edge_digest(), 1))
    }
}

impl<P: ManagedSelectedProof> ReadModel for SourceNavigationPgReadModel<'_, '_, '_, '_, '_, P> {
    fn check_interrupt(&mut self) -> Result<(), QueryError> {
        self.check_current()
    }

    fn selected_binding(&mut self) -> Result<Binding, QueryError> {
        self.check_current()?;
        Ok(self.binding.clone())
    }

    fn exact_visible_node(
        &mut self,
        id: &str,
        max_bytes: usize,
        max_carrier_bytes: usize,
        _max_vm_steps: u64,
        max_work_probes: u64,
        max_work_rows: u64,
    ) -> Result<ExactNode, QueryError> {
        let (record, charged) = self.node_lookup(
            id,
            max_bytes,
            max_carrier_bytes,
            max_work_probes,
            max_work_rows,
        )?;
        Ok(ExactNode {
            binding: self.binding.clone(),
            record,
            complete_unique_lookup: true,
            charged,
        })
    }

    fn visible_outgoing(
        &mut self,
        from_id: &str,
        after_edge_id: Option<&str>,
        max_rows: usize,
        max_bytes: usize,
        max_carrier_bytes: usize,
        _max_vm_steps: u64,
        max_work_probes: u64,
        max_work_rows: u64,
    ) -> Result<AdjacencyPage, QueryError> {
        checked_id(from_id)?;
        if let Some(after) = after_edge_id {
            checked_id(after)?;
        }
        if max_work_probes < 2 || max_work_rows == 0 || max_rows == 0 {
            return Err(query_error(
                QueryErrorCode::BudgetExceeded,
                "source adjacency probe or row budget exceeded",
            ));
        }
        self.check_current()?;
        let (expected_count, expected_digest, cert_rows) = self.adjacency_certificate(from_id)?;
        let limit = max_rows.checked_add(1).ok_or_else(|| {
            query_error(
                QueryErrorCode::BudgetExceeded,
                "source adjacency row limit overflow",
            )
        })?;
        let distinct_limit_i64 = checked_u64(limit, "source adjacency row limit overflow")?;
        let work_limit_i64 = i64::try_from(max_work_rows)
            .ok()
            .and_then(|rows| rows.checked_add(1))
            .ok_or_else(|| {
                query_error(
                    QueryErrorCode::BudgetExceeded,
                    "source adjacency work row budget overflow",
                )
            })?;
        let carrier_cap = checked_u64(max_carrier_bytes, "source adjacency byte limit overflow")?;
        let rows = self.tx.query(
            "WITH physical AS (
                 SELECT edge_id,from_id,to_id,visible,row_sha256,row_json
                 FROM (
                     SELECT row_id AS edge_id,from_id,to_id,visible,row_sha256,row_json
                     FROM cmd2_projection_navigation_bootstrap_row
                     WHERE domain=$1 AND consumer_id=$2 AND row_kind='edge' AND active
                       AND from_id=$3 AND ($4::text IS NULL OR edge_id COLLATE \"C\" > $4 COLLATE \"C\")
                     UNION ALL
                     SELECT edge_id,from_id,to_id,visible,row_sha256,row_json
                     FROM cmd2_projection_navigation_edge
                     WHERE domain=$1 AND consumer_id=$2
                       AND from_id=$3 AND ($4::text IS NULL OR edge_id COLLATE \"C\" > $4 COLLATE \"C\")
                 ) effective
             ), candidate_ids AS (
                 SELECT edge_id FROM physical
                 GROUP BY edge_id HAVING bool_or(visible)
                 ORDER BY edge_id COLLATE \"C\" LIMIT $5
             ), candidates AS (
                 SELECT physical.edge_id,physical.from_id,physical.to_id,physical.visible,
                        physical.row_sha256,octet_length(physical.row_json)::bigint AS size,
                        physical.row_json
                 FROM physical JOIN candidate_ids USING(edge_id)
             )
             SELECT edge_id,from_id,to_id,visible,row_sha256,size,
                    CASE WHEN size <= $6 THEN row_json END AS row_json,
                    count(*) OVER () AS candidate_count
             FROM candidates ORDER BY edge_id COLLATE \"C\",row_sha256 COLLATE \"C\"
             LIMIT $7",
            &[&self.domain, &self.consumer_id, &from_id, &after_edge_id,
              &distinct_limit_i64, &carrier_cap, &work_limit_i64],
        ).map_err(|_| query_error(QueryErrorCode::CorruptSelectedCarrier, "source adjacency index query failed"))?;
        let physical_rows = rows
            .first()
            .map(|row| row.get::<_, i64>("candidate_count"))
            .unwrap_or(0);
        let physical_rows = usize::try_from(physical_rows).map_err(|_| {
            query_error(
                QueryErrorCode::CorruptSelectedCarrier,
                "source adjacency candidate count is invalid",
            )
        })?;
        if physical_rows as u64 > max_work_rows {
            return Err(query_error(
                QueryErrorCode::BudgetExceeded,
                "source adjacency physical row budget exceeded",
            ));
        }
        let rows_total = physical_rows
            .checked_add(cert_rows as usize)
            .ok_or_else(|| {
                query_error(
                    QueryErrorCode::BudgetExceeded,
                    "source adjacency row count overflow",
                )
            })?;
        if rows.len() > work_limit_i64 as usize || rows_total as u64 > max_work_rows {
            return Err(query_error(
                QueryErrorCode::BudgetExceeded,
                "source adjacency row budget exceeded",
            ));
        }
        let exhausted = rows
            .iter()
            .map(|row| row.get::<_, String>("edge_id"))
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            <= max_rows;
        let mut edges = Vec::with_capacity(max_rows.min(rows.len()));
        let mut charged_bytes = 8u64 + 64;
        let mut effective =
            std::collections::BTreeMap::<String, (String, String, bool, Digest256, Vec<u8>)>::new();
        for row in &rows {
            let edge_id: String = row.get("edge_id");
            let from: String = row.get("from_id");
            let to: String = row.get("to_id");
            let visible: bool = row.get("visible");
            let sha_value: String = row.get("row_sha256");
            let size = usize::try_from(row.get::<_, i64>("size")).map_err(|_| {
                query_error(
                    QueryErrorCode::CorruptSelectedCarrier,
                    "source edge carrier size is invalid",
                )
            })?;
            if from != from_id || size > self.limits.max_row_bytes || size > max_carrier_bytes {
                return Err(query_error(
                    QueryErrorCode::BudgetExceeded,
                    "source edge carrier exceeds byte budget",
                ));
            }
            let raw: Option<String> = row.get("row_json");
            let raw = raw
                .ok_or_else(|| {
                    query_error(
                        QueryErrorCode::BudgetExceeded,
                        "source edge carrier exceeds byte budget",
                    )
                })?
                .into_bytes();
            charged_bytes = charged_bytes
                .checked_add(
                    edge_id.len() as u64
                        + from.len() as u64
                        + to.len() as u64
                        + 1
                        + 64
                        + 8
                        + raw.len() as u64,
                )
                .ok_or_else(|| {
                    query_error(
                        QueryErrorCode::BudgetExceeded,
                        "source adjacency byte count overflow",
                    )
                })?;
            let sha256 = parse_digest(&sha_value)?;
            if Digest256::of_bytes(&raw) != sha256 {
                return Err(query_error(
                    QueryErrorCode::CorruptSelectedCarrier,
                    "source edge digest differs",
                ));
            }
            let value = parse_json(
                &raw,
                JsonMode::PublishedStrict,
                JsonLimits {
                    max_bytes: max_carrier_bytes.max(1),
                    max_depth: 128,
                    max_visits: 200_000,
                    max_integer_digits: 128,
                },
            )
            .map_err(|_| {
                query_error(
                    QueryErrorCode::CorruptSelectedCarrier,
                    "source edge JSON is invalid",
                )
            })?;
            if value
                .root()
                .object_get("edge_id")
                .and_then(tos_foundation::JsonValue::as_str)
                != Some(edge_id.as_str())
                || value
                    .root()
                    .object_get("from_id")
                    .and_then(tos_foundation::JsonValue::as_str)
                    != Some(from.as_str())
                || value
                    .root()
                    .object_get("to_id")
                    .and_then(tos_foundation::JsonValue::as_str)
                    != Some(to.as_str())
            {
                return Err(query_error(
                    QueryErrorCode::CorruptSelectedCarrier,
                    "source edge endpoints differ",
                ));
            }
            if let Some((old_from, old_to, old_visible, old_sha256, old_raw)) =
                effective.get(&edge_id)
            {
                if old_from != &from
                    || old_to != &to
                    || *old_visible != visible
                    || *old_sha256 != sha256
                    || old_raw != &raw
                {
                    return Err(query_error(
                        QueryErrorCode::IndexIncomplete,
                        "source edge ID has conflicting projected contributions",
                    ));
                }
            } else {
                effective.insert(edge_id, (from, to, visible, sha256, raw));
            }
        }
        for (_, (_, _, visible, sha256, raw)) in effective.into_iter().take(max_rows) {
            if !visible {
                continue;
            }
            edges.push(RawRecord {
                binding: self.binding.clone(),
                raw,
                sha256,
            });
        }
        if charged_bytes > max_bytes as u64 {
            return Err(query_error(
                QueryErrorCode::BudgetExceeded,
                "source adjacency result exceeds byte budget",
            ));
        }
        self.check_current()?;
        Ok(AdjacencyPage {
            binding: self.binding.clone(),
            from_id: from_id.to_owned(),
            after_edge_id: after_edge_id.map(str::to_owned),
            edges,
            exhausted,
            expected_count,
            expected_digest,
            charged: self.charge(2, rows_total as u64, charged_bytes),
        })
    }

    fn authorize_current(&mut self, record: &RawRecord) -> Result<Charged, QueryError> {
        self.check_current()?;
        if record.binding != self.binding || Digest256::of_bytes(&record.raw) != record.sha256 {
            return Err(query_error(
                QueryErrorCode::StaleSelection,
                "source policy input binding differs",
            ));
        }
        self.policy
            .authorize_current(&self.disclosure_scope, record)
    }

    fn authority_boundary(&mut self, max_bytes: usize) -> Result<RawRecord, QueryError> {
        self.check_current()?;
        let raw = self.guard.header_raw();
        if raw.len() > self.limits.max_row_bytes || raw.len() > max_bytes {
            return Err(query_error(
                QueryErrorCode::BudgetExceeded,
                "source authority header exceeds byte budget",
            ));
        }
        let parsed = parse_json(
            raw,
            JsonMode::PublishedStrict,
            JsonLimits {
                max_bytes,
                max_depth: 128,
                max_visits: 100_000,
                max_integer_digits: 128,
            },
        )
        .map_err(|_| {
            query_error(
                QueryErrorCode::CorruptSelectedCarrier,
                "source authority header is invalid",
            )
        })?;
        let authority = parsed
            .root()
            .object_get("authority_boundary")
            .and_then(tos_foundation::JsonValue::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                query_error(
                    QueryErrorCode::CorruptSelectedCarrier,
                    "source authority boundary is absent",
                )
            })?;
        if authority != self.guard.owner_binding().selected_authority_boundary {
            return Err(query_error(
                QueryErrorCode::StaleSelection,
                "source authority boundary differs",
            ));
        }
        let raw = serde_json::to_vec(authority).map_err(|_| {
            query_error(
                QueryErrorCode::CorruptSelectedCarrier,
                "source authority boundary encoding failed",
            )
        })?;
        Ok(RawRecord {
            binding: self.binding.clone(),
            sha256: Digest256::of_bytes(&raw),
            raw,
        })
    }

    fn check_pin(&mut self, binding: &Binding) -> Result<(), QueryError> {
        self.check_current()?;
        if binding != &self.binding
            || self.guard.published_sequence() != binding.through_commit_seq
            || self.guard.published_root() != binding.index_root
        {
            return Err(query_error(
                QueryErrorCode::StaleSelection,
                "source navigation pin differs",
            ));
        }
        Ok(())
    }

    fn acquire_disclosure(
        &mut self,
        binding: &Binding,
        selected: &[RawRecord],
    ) -> Result<Box<dyn DisclosureLease + '_>, QueryError> {
        self.check_pin(binding)?;
        if selected
            .iter()
            .any(|row| row.binding != self.binding || Digest256::of_bytes(&row.raw) != row.sha256)
        {
            return Err(query_error(
                QueryErrorCode::StaleSelection,
                "source disclosure rows differ from selected binding",
            ));
        }
        self.disclosure_scope
            .validate_for_selected_model(binding, &self.guard.owner_binding().owner_receipt_id)?;
        let inner = self
            .policy
            .acquire_disclosure(&self.disclosure_scope, selected)?;
        let mut lease = ManagedPgDisclosureLease {
            inner,
            source: self.source,
            proof: self.proof,
        };
        lease.recheck()?;
        Ok(Box::new(lease))
    }
}

fn _durable_result_contract(_: DurableResult<()>) {}
