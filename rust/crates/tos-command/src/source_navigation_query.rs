//! Bounded PostgreSQL read model for the current managed source-navigation
//! projection. Its admitted Original receipt authenticates immutable bootstrap
//! metadata against the protected selection; no SQLite file is reopened.
//! A live PG/source lease holds publication and current rights through delivery.

use std::{
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

use crate::source_current_cut::ManagedCurrentSourceGeneration;
use postgres::Transaction;
use tos_compiler::KnowledgeSelectedExpectation;
use tos_compiler::managed_source::ManagedProducerProof;
use tos_foundation::{Digest256, JsonLimits, JsonMode, parse_json};
use tos_query::{
    AbortProbe, AbortReason, AdjacencyPage, Binding, Charged, CurrentPolicy, DisclosureLease,
    DisclosureScope, ExactNode, QueryError, QueryErrorCode, RawRecord, ReadModel,
};

use crate::{
    DurableNavigationEventLimits, DurableNavigationPublishedGenerationGuardV2,
    DurableProjectionProfile,
    durable_adapter::durable_projection::DurableNavigationPublishedGenerationV2,
    source_managed_selection::{
        ManagedSelectionError, ManagedSelectionResult, SourceNavigationCurrentLeaseV1,
    },
    source_navigation_cli::{
        NavigationProducerHeldOwnerIdentity, NavigationProducerOwnerSelection,
    },
    source_navigation_policy::{self, SourceNavigationPolicyReceiptV1},
};

const QUERY_TIMEOUT_CAP: Duration = Duration::from_secs(15);
const MAX_ID_BYTES: usize = 256;

fn query_error(code: QueryErrorCode, message: &'static str) -> QueryError {
    QueryError { code, message }
}

fn source_error(source: &SourceNavigationCurrentLeaseV1<'_>) -> Result<(), QueryError> {
    source.check_current().map_err(|_| {
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

fn verify_admitted_original_metadata(
    owner: &crate::durable_adapter::durable_projection::DurableNavigationCurrentIndexOwnerBindingV1,
    expected: &KnowledgeSelectedExpectation,
) -> Result<(), QueryError> {
    let invalid = || {
        query_error(
            QueryErrorCode::StaleSelection,
            "published navigation admission metadata differs; explicit bootstrap required",
        )
    };
    let receipt = owner
        .navigation_original_receipt
        .as_ref()
        .ok_or_else(invalid)?;
    receipt
        .verify_selected_metadata(expected)
        .map_err(|_| invalid())?;
    let admitted =
        crate::durable_adapter::durable_projection::navigation_selected_expectation_digest(
            expected,
        )
        .map_err(|_| invalid())?;
    if !expected.complete
        || owner.selected_expectation_digest != Some(admitted)
        || owner.navigation_original_root != parse_digest(&receipt.component_root_sha256)?
        || owner.source_graph != receipt.source_graph
        || owner.navigation_header_sha256 != parse_digest(&receipt.header_sha256)?
        || owner.navigation_node_input_root != parse_digest(&receipt.node_input_root_sha256)?
        || owner.navigation_edge_input_root != parse_digest(&receipt.edge_input_root_sha256)?
        || owner.navigation_nodes != receipt.nodes
        || owner.navigation_edges != receipt.edges
        || owner.navigation_rights != receipt.rights
        || owner.navigation_projected_rights > receipt.rights
    {
        return Err(invalid());
    }
    Ok(())
}

pub(crate) fn verify_selected_binding(
    generation: &ManagedCurrentSourceGeneration,
    expected: &KnowledgeSelectedExpectation,
    proof: &tos_compiler::managed_source::ManagedSourceProofV2,
    live: &SourceNavigationCurrentLeaseV1<'_>,
    facts: &NavigationProducerHeldOwnerIdentity,
    guard: &DurableNavigationPublishedGenerationGuardV2,
    owner_selection: &NavigationProducerOwnerSelection<'_>,
) -> Result<Binding, QueryError> {
    let owner = guard.owner_binding();
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
    let proof_root = proof.root_sha256().map_err(|_| {
        query_error(
            QueryErrorCode::StaleSelection,
            "managed source proof is invalid",
        )
    })?;
    let proof_digest = Digest256::from_hex(&proof_root).map_err(|_| {
        query_error(
            QueryErrorCode::StaleSelection,
            "managed source proof digest is invalid",
        )
    })?;
    let addressed_tree = Digest256::from_hex(&proof.generation.addressed_current_tree_sha256)
        .map_err(|_| {
            query_error(
                QueryErrorCode::StaleSelection,
                "managed source anchor tree commitment is invalid",
            )
        })?;
    let domain_digest = Digest256::from_hex(&proof.generation.domain_sha256).map_err(|_| {
        query_error(
            QueryErrorCode::StaleSelection,
            "managed source domain digest is invalid",
        )
    })?;
    let bootstrap_revision = Digest256::from_hex(&proof.generation.bootstrap_source_revision)
        .map_err(|_| {
            query_error(
                QueryErrorCode::StaleSelection,
                "managed source bootstrap revision is invalid",
            )
        })?;
    let bootstrap_membership = Digest256::from_hex(&proof.generation.bootstrap_membership_sha256)
        .map_err(|_| {
        query_error(
            QueryErrorCode::StaleSelection,
            "managed source bootstrap membership is invalid",
        )
    })?;
    let source_definition =
        Digest256::from_hex(&proof.generation.definition_sha256).map_err(|_| {
            query_error(
                QueryErrorCode::StaleSelection,
                "managed source definition digest is invalid",
            )
        })?;
    let schema_profile =
        Digest256::from_hex(&proof.generation.schema_profile_sha256).map_err(|_| {
            query_error(
                QueryErrorCode::StaleSelection,
                "managed source schema profile digest is invalid",
            )
        })?;
    verify_admitted_original_metadata(owner, expected)?;
    if live.require_managed_v2_basis(proof).is_err()
        || !live.rights_allowed()
        || live.head_sequence() != live.source_generation()
        || live.domain() != owner_selection.domain
        || live.domain() != proof.generation.domain
        || live.store_id() != proof.generation.store_id
        || live.source_epoch() != proof.generation.epoch
        || live.bootstrap_revision().0 != bootstrap_revision
        || live.bootstrap_membership().count != proof.generation.bootstrap_members
        || live.bootstrap_membership().digest != bootstrap_membership
        || live.source_definition_digest() != source_definition
        || live.schema_profile_digest() != schema_profile
        || live.anchor_commit_sequence() != generation.commit_seq()
        || facts.domain != owner_selection.domain
        || facts.proof_sha256 != proof_digest
        || facts.domain_sha256 != domain_digest
        || facts.store_id != proof.generation.store_id
        || facts.database_oid != live.database_oid()
        || facts.database_oid != proof.generation.database_oid
        || facts.anchor_addressed_tree_commitment != addressed_tree
        || facts.source_head_sequence != live.head_sequence()
        || facts.source_generation != live.source_generation()
        || facts.selected_audit_generation != live.audit_generation()
        || facts.bootstrap_membership_root != bootstrap_membership
        || facts.bootstrap_membership_count != proof.generation.bootstrap_members
        || facts.current_selected_generation_digest != live.selected_generation_digest()
        || facts.current_source_projection_digest != live.source_projection_digest()
        || facts.rule_version != live.rule_version()
        || facts.rule_version != owner_selection.rule_version
        || facts.rights_version != live.rights_version()
        || facts.rights_version != owner_selection.rights_version
        || facts.source_contract != live.source_contract()
        || facts.source_contract != owner_selection.source_contract
        || facts.schema_profile_digest != live.schema_profile_digest()
        || facts.owner_configuration_sha256 != owner_selection.owner_configuration_sha256
        || facts.job_id_sha256 != Digest256::of_bytes(owner_selection.job_id.as_bytes())
        || facts.job_fence != owner_selection.job_fence
        || facts.source_epoch != proof.generation.epoch
        || facts.managed_generation_digest != generation.digest()
        || facts.anchor_member_count != generation.member_count()
        || facts.anchor_projection_digest
            != Digest256::from_hex(&proof.inventory_projection()).map_err(|_| {
                query_error(
                    QueryErrorCode::StaleSelection,
                    "managed source projection digest is invalid",
                )
            })?
        || facts.model_sha256 != model_digest
        || facts.model_membership_root != membership
        || expected.through_commit_seq != generation.commit_seq()
        || owner.domain != owner_selection.domain
        || owner.owner_receipt_id != expected.owner_receipt_id
        || owner.managed_generation_digest != generation.digest()
        || owner.managed_generation_member_count != generation.member_count()
        || owner.managed_generation_commit_sequence != generation.commit_seq()
        || owner.managed_agent_definition_digest != generation.agent_definition_digest()
        || owner.source_membership_root != bootstrap_membership
        || owner.source_membership_count != proof.generation.bootstrap_members
        || owner.source_projection_digest
            != Digest256::from_hex(proof.native_inventory_root()).map_err(|_| {
                query_error(
                    QueryErrorCode::StaleSelection,
                    "managed source projection digest is invalid",
                )
            })?
        || owner.selected_model_digest != model_digest
        || owner.selected_model_abi != expected.model_abi
        || !tos_foundation::is_managed_knowledge_model_abi(&expected.model_abi)
        || owner.selected_descriptor_digest != parse_digest(&expected.descriptor_sha256)?
        || owner.selected_selection_profile != tos_query::SOURCE_NAVIGATION_SELECTION_PROFILE
        || owner.selected_source_cut != expected.source_cut
        || owner.selected_membership_root != membership
        || owner.selected_index_generation != expected.index_generation
        || owner.selected_route_map_version != expected.route_map_version
        || owner.selected_reader_abi != expected.reader_abi
        || owner.selected_authority_boundary != expected.authority_boundary
        || owner.selected_software_profile_id != owner_selection.profile.id
        || owner.selected_software_profile_version != owner_selection.profile.version
        || owner.selected_software_contract_digest != owner_selection.source_contract
        || live.source_contract() != owner_selection.source_contract
        || generation.commit_seq() > guard.published_sequence()
        || guard.domain_head_sequence() != live.head_sequence()
        || guard.published_sequence() > guard.domain_head_sequence()
        || !guard.rights_allowed()
        || guard.rights_allowed() != live.rights_allowed()
        || guard.rights_version() != live.rights_version()
        || guard.audit_generation() != live.audit_generation()
        || guard.profile_id() != owner_selection.profile.id
        || guard.profile_version() != owner_selection.profile.version
        || guard.source_contract_digest() != owner_selection.source_contract
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
        projection_root: facts.anchor_projection_digest,
        index_root: guard.published_root(),
        index_generation: owner.selected_index_generation.clone(),
        route_map_version: owner.selected_route_map_version.clone(),
        reader_abi: owner.selected_reader_abi.clone(),
        model_abi: owner.selected_model_abi.clone(),
        selection_profile: owner.selected_selection_profile.clone(),
        source_navigation_proof_v1: Some(tos_query::SourceNavigationProofV1 {
            bootstrap_root: guard.bootstrap_root(),
            anchor_through_commit_seq: generation.commit_seq(),
            anchor_membership_root: owner.selected_membership_root,
            anchor_projection_root: facts.anchor_projection_digest,
            published_through_commit_seq: guard.published_sequence(),
            observed_source_head_commit_seq: live.head_sequence(),
            coverage_root: guard.coverage_root(),
            adjacency_root: guard.adjacency_root(),
            published_index_root: guard.published_root(),
        }),
    };
    Ok(binding)
}

/// Result of checking the exact last-published source-navigation generation.
/// Missing publication remains non-ready. A lagging publication can serve
/// unchanged dependency scopes under the independently held current policy.
pub enum SourceNavigationPublishedReadV2<T> {
    Ready(T),
    NotReady,
}

/// Execute a query against the immutable selected navigation anchor and the
/// current PostgreSQL source lease. The caller already holds both the source
/// and job fences in the same repeatable-read transaction. `consume` must
/// finish any transport write before it returns.
#[allow(clippy::too_many_arguments)]
pub(crate) fn with_published_source_navigation_v2<'lease, 'tx_ref, 'tx, 'request, T, E>(
    source: &'lease SourceNavigationCurrentLeaseV1<'lease>,
    generation: &ManagedCurrentSourceGeneration,
    proof: &tos_compiler::managed_source::ManagedSourceProofV2,
    expected: &KnowledgeSelectedExpectation,
    facts: &NavigationProducerHeldOwnerIdentity,
    tx: &'tx_ref mut Transaction<'tx>,
    owner_selection: &'request NavigationProducerOwnerSelection<'request>,
    projection_profile: &DurableProjectionProfile,
    limits: DurableNavigationEventLimits,
    deadline: Instant,
    cancelled: &'request AtomicBool,
    consume: impl FnOnce(SourceNavigationPgReadModel<'lease, 'tx_ref, 'tx, 'request>) -> Result<T, E>,
) -> ManagedSelectionResult<SourceNavigationPublishedReadV2<Result<T, E>>> {
    let mut limits = limits;
    limits.statement_timeout =
        remaining_timeout(deadline, limits.statement_timeout).map_err(|_| {
            ManagedSelectionError::Durable(crate::DurableError::Refused(
                "source navigation deadline exhausted",
            ))
        })?;
    let isolation_row = tx
        .query_one("SHOW transaction_isolation", &[])
        .map_err(|error| ManagedSelectionError::Durable(error.into()))?;
    let isolation: String = isolation_row.get(0);
    if isolation != "repeatable read" {
        return Err(ManagedSelectionError::Durable(
            crate::DurableError::Conflict("source navigation read requires repeatable read"),
        ));
    }
    source
        .require_managed_v2_basis(proof)
        .map_err(ManagedSelectionError::Source)?;
    tx.query_opt(
        "SELECT cursor_seq FROM cmd2_projection_navigation_consumer
         WHERE domain=$1 AND consumer_id=$2 FOR SHARE",
        &[&owner_selection.domain, &owner_selection.consumer_id],
    )
    .map_err(|error| ManagedSelectionError::Durable(error.into()))?;
    let guard =
        crate::durable_adapter::durable_projection::read_navigation_published_guard_with_tx_v2(
            tx,
            owner_selection.domain,
            owner_selection.consumer_id,
            projection_profile,
            limits,
        )
        .map_err(ManagedSelectionError::Durable)?;
    let guard = match guard {
        DurableNavigationPublishedGenerationV2::Ready(guard) => guard,
        DurableNavigationPublishedGenerationV2::NotReady(_) => {
            return Ok(SourceNavigationPublishedReadV2::NotReady);
        }
    };
    let binding = verify_selected_binding(
        generation,
        expected,
        proof,
        source,
        facts,
        &guard,
        owner_selection,
    )
    .map_err(|_| {
        ManagedSelectionError::Durable(crate::DurableError::Conflict(
            "published navigation index differs from the immutable source anchor or live owner",
        ))
    })?;
    let policy_receipt = SourceNavigationPolicyReceiptV1::parse(
        owner_selection.policy_receipt_value,
    )
    .map_err(|_| {
        ManagedSelectionError::Durable(crate::DurableError::Refused(
            "explicit source navigation policy receipt is invalid",
        ))
    })?;
    let expected_export_id = source_navigation_policy::selected_export_receipt_id(
        &guard.owner_binding().navigation_original_root.to_hex(),
    )
    .map_err(|_| {
        ManagedSelectionError::Durable(crate::DurableError::Refused(
            "selected source export receipt is invalid",
        ))
    })?;
    if policy_receipt.0.export_receipt_id != expected_export_id
        || policy_receipt.0.model_receipt_id != expected.owner_receipt_id
        || policy_receipt.0.model_abi != expected.model_abi
        || policy_receipt.0.selection_profile != tos_query::SOURCE_NAVIGATION_SELECTION_PROFILE
    {
        return Err(ManagedSelectionError::Durable(
            crate::DurableError::Refused("source-owner policy receipt differs from selected model"),
        ));
    }
    let (disclosure_scope, policy) = source_navigation_policy::read_locked_v1(
        tx,
        owner_selection.domain,
        owner_selection.consumer_id,
        owner_selection.owner_configuration_sha256,
        &policy_receipt.0,
        policy_receipt.1,
        &binding,
        &expected.owner_receipt_id,
        &expected.source_cut,
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
    source_error(source).map_err(|_| {
        ManagedSelectionError::Durable(crate::DurableError::Conflict(
            "source navigation current owner changed",
        ))
    })?;
    let read_model = SourceNavigationPgReadModel {
        tx,
        domain: owner_selection.domain,
        consumer_id: owner_selection.consumer_id,
        guard,
        binding,
        disclosure_scope,
        policy: Box::new(policy),
        source,
        deadline,
        cancelled,
        abort_probe: None,
        limits,
    };
    Ok(SourceNavigationPublishedReadV2::Ready(consume(read_model)))
}

/// Indexed QRY adapter over the authenticated baseline/event overlay.
/// Construction is private to the owner callback above.
pub struct SourceNavigationPgReadModel<'lease, 'tx_ref, 'tx, 'request> {
    tx: &'tx_ref mut Transaction<'tx>,
    domain: &'request str,
    consumer_id: &'request str,
    guard: DurableNavigationPublishedGenerationGuardV2,
    binding: Binding,
    disclosure_scope: DisclosureScope,
    policy: Box<dyn CurrentPolicy>,
    source: &'lease SourceNavigationCurrentLeaseV1<'lease>,
    deadline: Instant,
    cancelled: &'request AtomicBool,
    abort_probe: Option<Arc<dyn AbortProbe>>,
    limits: DurableNavigationEventLimits,
}

struct ManagedPgDisclosureLease<'lease> {
    inner: Box<dyn DisclosureLease>,
    source: &'lease SourceNavigationCurrentLeaseV1<'lease>,
}

impl DisclosureLease for ManagedPgDisclosureLease<'_> {
    fn recheck(&mut self) -> Result<(), QueryError> {
        self.inner.recheck()?;
        source_error(self.source)?;
        self.inner.recheck()?;
        Ok(())
    }
}

impl SourceNavigationPgReadModel<'_, '_, '_, '_> {
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
        source_error(self.source)
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

    /// A current policy receipt authorizes the selected metadata carrier; it
    /// does not establish that a source dependency retained its disclosure
    /// conditions after publication. Until such a changed scope is republished,
    /// only unchanged scopes can be served from the older coherent version.
    /// The surrounding source/PG fences hold every observation through flush.
    fn authorize_published_dependencies(
        &mut self,
        record: &RawRecord,
    ) -> Result<Charged, QueryError> {
        if self.guard.published_sequence() == self.source.head_sequence() {
            return Ok(Charged::default());
        }
        let parsed = parse_json(
            &record.raw,
            JsonMode::PublishedStrict,
            JsonLimits {
                max_bytes: self.limits.max_row_bytes,
                max_depth: 128,
                max_visits: 200_000,
                max_integer_digits: 128,
            },
        )
        .map_err(|_| {
            query_error(
                QueryErrorCode::CorruptSelectedCarrier,
                "published disclosure record is invalid",
            )
        })?;
        let value = parsed.root();
        let (kind, id) = if let Some(id) = value.object_get("node_id").and_then(|v| v.as_str()) {
            ("node", id)
        } else if let Some(id) = value.object_get("edge_id").and_then(|v| v.as_str()) {
            ("edge", id)
        } else {
            // The authenticated authority header is checked by authority_boundary
            // and the current owner policy; it has no source object contribution.
            return Ok(self.charge(0, 0, record.raw.len() as u64));
        };
        checked_id(id)?;
        let cap = self.limits.max_dependencies;
        let row_cap = cap
            .checked_add(1)
            .and_then(|n| i64::try_from(n).ok())
            .ok_or_else(|| {
                query_error(
                    QueryErrorCode::BudgetExceeded,
                    "disclosure dependency bound",
                )
            })?;
        let digest = record.sha256.to_hex();
        let rows = self.tx.query(
            "WITH owners AS MATERIALIZED (
                 (SELECT owner_source_path AS source_path,owner_record_id AS record_id,true AS baseline
                  FROM cmd2_projection_navigation_bootstrap_row
                  WHERE domain=$1 AND consumer_id=$2 AND row_kind=$3 AND row_id=$4
                    AND row_sha256=$5 AND active LIMIT $6)
                 UNION ALL
                 (SELECT source_path,record_id,false AS baseline
                  FROM cmd2_projection_navigation_node
                  WHERE domain=$1 AND consumer_id=$2 AND $3='node' AND node_id=$4
                    AND row_sha256=$5 LIMIT $6)
                 UNION ALL
                 (SELECT source_path,record_id,false AS baseline
                  FROM cmd2_projection_navigation_edge
                  WHERE domain=$1 AND consumer_id=$2 AND $3='edge' AND edge_id=$4
                    AND row_sha256=$5 LIMIT $6)
                 LIMIT $6
             ), dependencies AS MATERIALIZED (
                 SELECT source_path AS path,true AS is_required FROM owners
                 UNION ALL
                 SELECT d.dependency_path,false AS is_required
                 FROM owners o JOIN cmd2_projection_navigation_bootstrap_dependency d
                   ON d.domain=$1 AND d.consumer_id=$2
                  AND d.owner_source_path=o.source_path AND d.owner_record_id=o.record_id
                 WHERE o.baseline
                 UNION ALL
                 SELECT d.dependency_path,d.observation_state='present' AS is_required
                 FROM owners o JOIN cmd2_projection_navigation_dependency d
                   ON d.domain=$1 AND d.consumer_id=$2
                  AND d.source_path=o.source_path AND d.record_id=o.record_id
                 WHERE NOT o.baseline
                 LIMIT $6
             )
             SELECT d.path,d.is_required,c.commit_seq,
                    CASE WHEN c.subject IS NULL THEN EXISTS (
                        SELECT 1 FROM cmd2_history h WHERE h.domain=$1 AND h.subject=d.path
                    ) ELSE false END AS missing_retained_member,
                    (SELECT count(*) FROM owners) AS owner_count
             FROM dependencies d LEFT JOIN cmd2_current c ON c.domain=$1 AND c.subject=d.path",
            &[&self.domain, &self.consumer_id, &kind, &id, &digest, &row_cap],
        ).map_err(|_| query_error(QueryErrorCode::CorruptSelectedCarrier, "published disclosure dependencies unavailable"))?;
        if rows.is_empty() {
            return Err(query_error(
                QueryErrorCode::CorruptSelectedCarrier,
                "published disclosure owner is absent",
            ));
        }
        if rows.len() > cap || rows[0].get::<_, i64>("owner_count") > cap as i64 {
            return Err(query_error(
                QueryErrorCode::BudgetExceeded,
                "published disclosure dependency budget",
            ));
        }
        let mut bytes = record.raw.len() as u64;
        for row in &rows {
            let path: &str = row.get("path");
            bytes = bytes.checked_add(path.len() as u64 + 18).ok_or_else(|| {
                query_error(
                    QueryErrorCode::BudgetExceeded,
                    "published disclosure byte overflow",
                )
            })?;
            let sequence: Option<i64> = row.get("commit_seq");
            let required: bool = row.get("is_required");
            let missing: bool = row.get("missing_retained_member");
            if sequence.is_some_and(|n| n < 0 || n as u64 > self.guard.published_sequence())
                || (sequence.is_none() && (required || missing))
            {
                return Err(query_error(
                    QueryErrorCode::PublicationPending,
                    "published source dependencies changed; await their publication",
                ));
            }
        }
        if bytes > self.limits.max_total_bytes {
            return Err(query_error(
                QueryErrorCode::BudgetExceeded,
                "published disclosure byte budget",
            ));
        }
        self.check_current()?;
        Ok(self.charge(1 + 2 * rows.len() as u64, rows.len() as u64, bytes))
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

impl ReadModel for SourceNavigationPgReadModel<'_, '_, '_, '_> {
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
                       AND from_id=$3 AND ($4::text IS NULL OR row_id COLLATE \"C\" > $4 COLLATE \"C\")
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
        let policy = self
            .policy
            .authorize_current(&self.disclosure_scope, record)?;
        let dependencies = self.authorize_published_dependencies(record)?;
        Ok(Charged {
            probes: policy.probes.saturating_add(dependencies.probes),
            rows: policy.rows.saturating_add(dependencies.rows),
            bytes: policy.bytes.saturating_add(dependencies.bytes),
            cpu_steps: policy.cpu_steps.saturating_add(dependencies.cpu_steps),
            cpu_meter: tos_query::CpuStepsMeter::Unavailable,
        })
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
        // This is the source navigation carrier's note. The normalized graph
        // selection has its own authority envelope; both are independently
        // bound by the verified owner binding and need not have equal text.
        if Some(authority)
            != self
                .guard
                .owner_binding()
                .navigation_header
                .get("authority_boundary")
                .and_then(serde_json::Value::as_str)
        {
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
        };
        lease.recheck()?;
        Ok(Box::new(lease))
    }
}
