//! Opt-in V2 source selection. V1 transcript meanings remain unchanged.
//! The immutable tree proves membership; current rights and publication are
//! always checked by the existing coordinator/owner fences.
use super::super::audit_delta;
use super::*;
use tos_segment_store::{
    AuthenticatedTreeDeltaV1, AuthenticatedTreeDescriptorV2, AuthenticatedTreeEntryV1,
    AuthenticatedTreeLimitsV1, decode_placement_tree_row, encode_placement_tree_row,
};

const OBJECT_KIND: &[u8] = b"cmd2-managed-addressed-selection-v2";
const METADATA_KIND: &[u8] = b"cmd2-private-row-commitments-v2";
const HISTORY_KIND: &[u8] = b"cmd2-history-addressed-v2";
const CURRENT_KIND: &[u8] = b"cmd2-current-addressed-v2";
const MAX_DESCRIPTOR: usize = 65536;

#[derive(Clone, Debug)]
pub(crate) struct AddressedCutV2 {
    pub store_id: [u8; 16],
    pub domain_digest: Digest256,
    pub domain: String,
    pub through_seq: u64,
    pub audit_generation: u64,
    pub database_oid: u64,
    pub schema_profile_digest: Digest256,
    pub state_profile_digest: Digest256,
    pub log_digest: Digest256,
    pub metadata: AuthenticatedTreeDescriptorV2,
    pub history: AuthenticatedTreeDescriptorV2,
    pub current: AuthenticatedTreeDescriptorV2,
    pub inventory: super::addressed_inventory::AddressedInventoryV2,
}
#[derive(Clone, Debug)]
pub(crate) struct VerifiedAddressedGeneration {
    pub(crate) audited_root: AuditedStoreRoot,
    pub(crate) store: SegmentStore,
    pub(crate) cut: AddressedCutV2,
    pub(crate) digest: Digest256,
    pub(crate) state_digest: Digest256,
    pub(crate) selected_audit_generation: u64,
    pub(crate) log_frontier: VerifiedLogFrontier,
    pub(crate) limits: AuthenticatedTreeLimitsV1,
}

impl AddressedCutV2 {
    fn encode(&self) -> DurableResult<Vec<u8>> {
        let inventory = self.inventory.encode()?;
        let tuple = serde_json::json!([
            "cmd2-managed-addressed-selection-v2",
            self.store_id,
            self.domain_digest.to_hex(),
            self.domain,
            self.through_seq,
            self.audit_generation,
            self.database_oid,
            self.schema_profile_digest.to_hex(),
            self.state_profile_digest.to_hex(),
            self.log_digest.to_hex(),
            self.metadata.encode(MAX_DESCRIPTOR)?,
            self.history.encode(MAX_DESCRIPTOR)?,
            self.current.encode(MAX_DESCRIPTOR)?,
            inventory
        ]);
        let raw = serde_json::to_vec(&tuple)
            .map_err(|_| DurableError::Corrupt("addressed descriptor encoding"))?;
        if raw.len() > MAX_DESCRIPTOR {
            return Err(DurableError::Refused("addressed descriptor byte budget"));
        }
        Ok(raw)
    }
    fn decode(raw: &[u8]) -> DurableResult<Self> {
        if raw.len() > MAX_DESCRIPTOR {
            return Err(DurableError::Refused("addressed descriptor byte budget"));
        }
        let value: serde_json::Value = serde_json::from_slice(raw)
            .map_err(|_| DurableError::Corrupt("addressed descriptor encoding"))?;
        let fields = retained_tuple(&value, 14)?;
        if retained_text(&fields[0])? != "cmd2-managed-addressed-selection-v2" {
            return Err(DurableError::Corrupt("addressed descriptor version"));
        }
        let bytes = |field: &serde_json::Value| -> DurableResult<Vec<u8>> {
            serde_json::from_value(field.clone())
                .map_err(|_| DurableError::Corrupt("addressed descriptor byte field"))
        };
        let number = |field: &serde_json::Value| {
            field
                .as_u64()
                .ok_or(DurableError::Corrupt("addressed descriptor integer"))
        };
        let digest = |field: &serde_json::Value| parse_hex(retained_text(field)?.into());
        let store_id: [u8; 16] = bytes(&fields[1])?
            .try_into()
            .map_err(|_| DurableError::Corrupt("addressed descriptor store identity"))?;
        let result = Self {
            store_id,
            domain_digest: digest(&fields[2])?,
            domain: retained_text(&fields[3])?.into(),
            through_seq: number(&fields[4])?,
            audit_generation: number(&fields[5])?,
            database_oid: number(&fields[6])?,
            schema_profile_digest: digest(&fields[7])?,
            state_profile_digest: digest(&fields[8])?,
            log_digest: digest(&fields[9])?,
            metadata: AuthenticatedTreeDescriptorV2::decode(&bytes(&fields[10])?, MAX_DESCRIPTOR)?,
            history: AuthenticatedTreeDescriptorV2::decode(&bytes(&fields[11])?, MAX_DESCRIPTOR)?,
            current: AuthenticatedTreeDescriptorV2::decode(&bytes(&fields[12])?, MAX_DESCRIPTOR)?,
            inventory: super::addressed_inventory::AddressedInventoryV2::decode(&bytes(
                &fields[13],
            )?)?,
        };
        if result.encode()? != raw {
            return Err(DurableError::Corrupt(
                "addressed descriptor noncanonical encoding",
            ));
        }
        Ok(result)
    }
    pub(crate) fn state_digest(&self) -> DurableResult<Digest256> {
        // Selected digest and complete_cut bookkeeping never enter this object.
        // All semantic metadata/header changes enter the metadata tree instead.
        Ok(Digest256::of_bytes(&self.encode()?))
    }
    pub(crate) fn require_store(&self, store: &SegmentStore) -> DurableResult<()> {
        self.inventory.require_store(store)?;
        if self.store_id != store.store_id()
            || self.domain_digest != store.domain_digest()
            || self.domain.as_bytes() != store.custody_domain()
            || self.schema_profile_digest != schema_profile_digest()
            || self.state_profile_digest != audit_delta::audit_delta_schema_digest()
            || self.metadata.kind != METADATA_KIND
            || self.history.kind != HISTORY_KIND
            || self.current.kind != CURRENT_KIND
            || self.inventory.projection_entries() != self.current.entries
            || self.history.entries < self.current.entries
            || [&self.metadata, &self.history, &self.current]
                .iter()
                .any(|tree| {
                    tree.store_id != store.store_id() || tree.domain_digest != store.domain_digest()
                })
        {
            return Err(DurableError::Conflict(
                "addressed selection identity/profile differs",
            ));
        }
        Ok(())
    }
}
impl VerifiedAddressedGeneration {
    pub(crate) fn lookup_current(
        &self,
        domain: &str,
        path: &RelativePath,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> DurableResult<Option<PlacementGenerationRowV1>> {
        self.lookup_current_with_work(
            domain,
            path,
            deadline,
            cancel,
            &mut tos_segment_store::AuthenticatedTreeWorkV1::default(),
        )
    }
    pub(crate) fn lookup_current_with_work(
        &self,
        domain: &str,
        path: &RelativePath,
        deadline: Instant,
        cancel: &AtomicBool,
        work: &mut tos_segment_store::AuthenticatedTreeWorkV1,
    ) -> DurableResult<Option<PlacementGenerationRowV1>> {
        let store = &self.store;
        self.audited_root.require_store(store)?;
        self.cut.require_store(store)?;
        if domain != self.cut.domain {
            return Err(DurableError::Conflict("addressed domain differs"));
        }
        let key = membership_key(CURRENT_KEY_TAG, domain, path.as_str(), None)?;
        let (value, observed) = store.lookup_authenticated_tree_v2_with_work(
            &self.cut.current,
            &key,
            self.limits,
            deadline,
            cancel,
        )?;
        ManagedSourceWorkV1::charge(&mut work.read_nodes, observed.read_nodes)?;
        ManagedSourceWorkV1::charge(&mut work.read_bytes, observed.read_bytes)?;
        ManagedSourceWorkV1::charge(&mut work.written_nodes, observed.written_nodes)?;
        ManagedSourceWorkV1::charge(&mut work.written_bytes, observed.written_bytes)?;
        value
            .map(|raw| {
                let row = decode_placement_tree_row(&raw, self.limits.max_value_bytes)?;
                if row.key != key {
                    return Err(DurableError::Corrupt(
                        "addressed placement value key differs",
                    ));
                }
                Ok(row)
            })
            .transpose()
    }
}

impl VerifiedAddressedGeneration {
    pub(crate) fn facts(&self) -> SourceSelectionFacts<'_> {
        SourceSelectionFacts {
            audited_root: &self.audited_root,
            domain: &self.cut.domain,
            descriptor_cut: SelectedCutFacts {
                commitment_version: 2,
                store_id: self.cut.store_id,
                domain_digest: self.cut.domain_digest,
                through_seq: self.cut.through_seq,
                audit_generation: self.cut.audit_generation,
                database_oid: self.cut.database_oid,
                schema_profile_digest: self.cut.schema_profile_digest,
                state_digest: self.state_digest,
                log_digest: self.cut.log_digest,
                historical_members: self.cut.history.entries,
                current_members: self.cut.current.entries,
                history_membership_root: self.cut.history.commitment,
                current_membership_root: self.cut.current.commitment,
            },
        }
    }
}

fn delta_error(error: audit_delta::AuditDeltaError) -> DurableError {
    match error {
        audit_delta::AuditDeltaError::Database(error) => DurableError::Database(error),
        _ => DurableError::Refused("addressed audit interval lost; explicit cold reopen required"),
    }
}
fn metadata_tree_key(
    table: audit_delta::MetadataTable,
    key: Vec<u8>,
    cap: usize,
) -> DurableResult<Vec<u8>> {
    audit_delta::StableRowKey::parse(table, key, cap)
        .map_err(delta_error)?
        .framed_tree_key()
        .map_err(delta_error)
}
fn source_tree_error(message: &'static str) -> SegmentError {
    SegmentError::new(tos_segment_store::SegmentErrorCode::CorruptBytes, message)
}

impl DurablePgCoordinator {
    /// Explicit opt-in schema route. Call only in the owner-selected isolated
    /// private coordinator before a fresh exhaustive cold source baseline.
    /// This does not modify the V1 schema installer or a live installed release.
    pub fn enable_addressed_audit_protocol(&mut self) -> DurableResult<()> {
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        tx.batch_execute(include_str!("audit_delta_schema_v1.sql"))?;
        let cancelled = AtomicBool::new(false);
        audit_delta::verify_sql_binding_types(
            &mut tx,
            audit_delta::AuditDeltaControl {
                deadline: Instant::now() + Duration::from_secs(5),
                cancelled: &cancelled,
            },
        )
        .map_err(delta_error)?;
        tx.commit()?;
        Ok(())
    }

    /// Bootstrap after complete source+custody cold verification. The borrowed
    /// parent supplies the exact anchored root and V1 EOF membership certificate;
    /// after return the caller replaces that parent with the new addressed handle.
    pub fn migrate_current_source_generation_addressed(
        &mut self,
        store: &SegmentStore,
        generation: &crate::source_current_cut::ManagedCurrentSourceGeneration,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<crate::source_current_cut::ManagedCurrentSourceGeneration> {
        active(deadline, cancelled)?;
        let legacy = generation
            .selected()
            .legacy_view()
            .ok_or(DurableError::Refused(
                "addressed bootstrap requires freshly cold-verified V1 source baseline",
            ))?;
        // A warm legacy parent is not a substitute for the independently
        // completed cold source/index assessment required by the opt-in route.
        if !matches!(generation.selected(), SelectedSourceGeneration::Cold(_)) {
            return Err(DurableError::Refused(
                "addressed bootstrap requires cold source assessment",
            ));
        }
        legacy.audited_root.require_store(store)?;
        // Activation only after the exact cold source baseline; it neither
        // mutates semantic rows nor advances the baseline fence generation.
        let mut activation = self.client.transaction()?;
        activation.batch_execute("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ; SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        audit_delta::activate_domain(
            &mut activation,
            generation.cohort().domain(),
            generation.audit_generation(),
            audit_delta::audit_delta_schema_digest(),
        )
        .map_err(delta_error)?;
        held_generation_metadata(&mut activation, store, generation)?;
        activation.commit()?;
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .start()?;
        tx.batch_execute("SET LOCAL statement_timeout='60s'; SET LOCAL work_mem='4MB'")?;
        held_generation_metadata(&mut tx, store, generation)?;
        let domain = generation.cohort().domain();
        let mut clauses = Vec::new();
        for table in audit_delta::MetadataTable::ALL {
            clauses.push(format!(
                "SELECT {}::smallint AS table_id,cmd2_audit_delta_v1_row_key({}::smallint,row_to_json(t)) AS row_key,cmd2_audit_delta_v1_row_commitment({}::smallint,row_to_json(t)) AS row_commitment FROM {} t WHERE domain=$1",
                table.id(), table.id(), table.id(), table.sql_name()));
        }
        let union = clauses.join(" UNION ALL ");
        let sql = format!(
            "SELECT table_id,row_key,row_commitment FROM ({union}) rows ORDER BY table_id,octet_length(row_key),row_key"
        );
        let mut rows = tx.query_raw(&sql, &[&domain])?;
        let entries = std::iter::from_fn(|| match rows.next() {
            Ok(Some(row)) => Some((|| {
                active(deadline, cancelled)
                    .map_err(|_| source_tree_error("cold metadata deadline"))?;
                let table = audit_delta::MetadataTable::from_id(row.get(0))
                    .ok_or_else(|| source_tree_error("cold metadata unknown table"))?;
                let key = metadata_tree_key(table, row.get(1), limits.max_key_bytes)
                    .map_err(|_| source_tree_error("cold metadata stable key"))?;
                let value: Vec<u8> = row.get(2);
                if value.len() != 32 {
                    return Err(source_tree_error("cold metadata row commitment"));
                }
                Ok(AuthenticatedTreeEntryV1 { key, value })
            })()),
            Ok(None) => None,
            Err(_) => Some(Err(source_tree_error(
                "cold metadata database cursor failed",
            ))),
        });
        let metadata = store.build_authenticated_tree_v2(
            METADATA_KIND,
            entries,
            limits,
            deadline,
            cancelled,
        )?;
        drop(rows);
        let mut history = legacy.cursor(GenerationNamespaceV1::History)?;
        let history_entries = std::iter::from_fn(|| match history.next_row() {
            Ok(Some(row)) => Some(encode_placement_tree_row(&row, limits.max_value_bytes).map(
                |value| AuthenticatedTreeEntryV1 {
                    key: row.key,
                    value,
                },
            )),
            Ok(None) => None,
            Err(_) => Some(Err(source_tree_error(
                "cold verified history cursor failed",
            ))),
        });
        let history_tree = store.build_authenticated_tree_v2(
            HISTORY_KIND,
            history_entries,
            limits,
            deadline,
            cancelled,
        )?;
        history.finish()?;
        let mut current = legacy.cursor(GenerationNamespaceV1::Current)?;
        let current_entries = std::iter::from_fn(|| match current.next_row() {
            Ok(Some(row)) => Some(encode_placement_tree_row(&row, limits.max_value_bytes).map(
                |value| AuthenticatedTreeEntryV1 {
                    key: row.key,
                    value,
                },
            )),
            Ok(None) => None,
            Err(_) => Some(Err(source_tree_error(
                "cold verified current cursor failed",
            ))),
        });
        let current_tree = store.build_authenticated_tree_v2(
            CURRENT_KIND,
            current_entries,
            limits,
            deadline,
            cancelled,
        )?;
        current.finish()?;
        if history_tree.entries != legacy.descriptor_cut.historical_members
            || current_tree.entries != legacy.descriptor_cut.current_members
        {
            return Err(DurableError::Corrupt(
                "addressed bootstrap EOF counts differ",
            ));
        }
        let inventory = super::addressed_inventory::AddressedInventoryV2::build(
            &mut tx,
            store,
            domain,
            generation.commit_seq(),
            limits,
            deadline,
            cancelled,
        )?;
        let cut = AddressedCutV2 {
            store_id: store.store_id(),
            domain_digest: store.domain_digest(),
            domain: domain.into(),
            through_seq: generation.commit_seq(),
            audit_generation: generation.audit_generation(),
            database_oid: legacy.descriptor_cut.database_oid,
            schema_profile_digest: schema_profile_digest(),
            state_profile_digest: audit_delta::audit_delta_schema_digest(),
            log_digest: legacy.descriptor_cut.log_digest,
            metadata,
            history: history_tree,
            current: current_tree,
            inventory,
        };
        tx.commit()?;
        let candidate = self.install_addressed_selection(
            store,
            cut,
            VerifiedLogFrontier(generation.selected().log_frontier().clone()),
            limits,
            deadline,
            cancelled,
        )?;
        let selected = self.select_addressed_selection(
            store,
            candidate,
            generation.cohort(),
            None,
            deadline,
            cancelled,
        )?;
        Ok(
            crate::source_current_cut::ManagedCurrentSourceGeneration::from_verified_addressed(
                store,
                generation.cohort().clone(),
                selected,
            ),
        )
    }

    fn install_addressed_selection(
        &mut self,
        store: &SegmentStore,
        cut: AddressedCutV2,
        frontier: VerifiedLogFrontier,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<VerifiedAddressedGeneration> {
        active(deadline, cancelled)?;
        cut.require_store(store)?;
        if frontier.0.clone().finalize() != cut.log_digest {
            return Err(DurableError::Corrupt("addressed log frontier differs"));
        }
        let raw = cut.encode()?;
        let state_digest = cut.state_digest()?;
        let digest = store.install_authenticated_object_v1(
            OBJECT_KIND,
            &raw,
            MAX_DESCRIPTOR,
            deadline,
            cancelled,
        )?;
        let readback = store.read_authenticated_object_v1(
            OBJECT_KIND,
            digest,
            MAX_DESCRIPTOR,
            deadline,
            cancelled,
        )?;
        if readback != raw {
            return Err(DurableError::Corrupt(
                "addressed descriptor readback differs",
            ));
        }
        Ok(VerifiedAddressedGeneration {
            audited_root: store.hold_audit_root()?,
            store: store.clone(),
            cut,
            digest,
            state_digest,
            selected_audit_generation: 0,
            log_frontier: frontier,
            limits,
        })
    }

    fn select_addressed_selection(
        &mut self,
        store: &SegmentStore,
        mut candidate: VerifiedAddressedGeneration,
        cohort: &ManagedSourceCohort,
        owner: Option<&CreationOwnerFence<'_>>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<VerifiedAddressedGeneration> {
        active(deadline, cancelled)?;
        candidate.audited_root.require_store(store)?;
        candidate.cut.require_store(store)?;
        let cut = &candidate.cut;
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        let audit = lock_audit_fence(&mut tx, &cut.domain)?;
        let row = tx.query_one(
            "SELECT * FROM cmd2_domain WHERE domain=$1 FOR UPDATE",
            &[&cut.domain],
        )?;
        cohort_matches(&row, cohort, true)?;
        if audit != cut.audit_generation
            || as_u64(row.get("head_seq"))? != cut.through_seq
            || database_oid(&mut tx)? != cut.database_oid
            || !row.get::<_, bool>("rights_allowed")
            || row.get::<_, Option<String>>("schema_profile_digest")
                != Some(cut.schema_profile_digest.to_hex())
        {
            return Err(DurableError::Conflict(
                "addressed selection raced metadata/policy",
            ));
        }
        if let Some(owner) = owner {
            owner
                .verify_current(deadline, cancelled)
                .map_err(source_error)?;
        }
        let anticipated = audit
            .checked_add(1)
            .ok_or(DurableError::Corrupt("addressed audit overflow"))?;
        tx.execute("UPDATE cmd2_domain SET published_seq=$2,complete_cut_digest=$3,complete_cut_generation=$4,selected_generation_digest=$5,source_projection_digest=$6 WHERE domain=$1",
            &[&cut.domain,&as_i64(cut.through_seq)?,&candidate.state_digest.to_hex(),&as_i64(anticipated)?,
              &candidate.digest.to_hex(),&cut.inventory.root().to_hex()])?;
        let observed = lock_audit_fence(&mut tx, &cut.domain)?;
        if observed != anticipated {
            return Err(DurableError::Corrupt("addressed publication audit differs"));
        }
        if let Some(owner) = owner {
            owner
                .verify_current(deadline, cancelled)
                .map_err(source_error)?;
        }
        active(deadline, cancelled)?;
        tx.commit()?;
        candidate.selected_audit_generation = observed;
        Ok(candidate)
    }
}

impl DurablePgCoordinator {
    pub(super) fn continue_addressed_creation(
        &mut self,
        store: &SegmentStore,
        cohort: &ManagedSourceCohort,
        attempt: &SourceCreationAttempt,
        package: CreationPackage<'_>,
        filesystem: &CreationFilesystem,
        deadline: Instant,
        cancelled: &AtomicBool,
        source_work: &mut ManagedSourceWorkV1,
    ) -> DurableResult<crate::source_current_cut::ManagedCurrentSourceGeneration> {
        active(deadline, cancelled)?;
        ManagedSourceWorkV1::charge(&mut source_work.addressed_continuations_attempted, 1)?;
        let continuation = attempt.continuation.as_ref().ok_or(DurableError::Refused(
            "addressed creation chain absent; explicit cold reopen required",
        ))?;
        let parent = match &continuation.parent {
            SelectedSourceGeneration::Addressed(parent) => parent,
            _ => return Err(DurableError::Refused("addressed parent selection absent")),
        };
        parent.audited_root.require_store(store)?;
        parent.cut.require_store(store)?;
        let committed = continuation.committed.borrow();
        let committed = committed
            .as_ref()
            .ok_or(DurableError::Refused("addressed commit not observed"))?;
        let head = committed.commit_seq;
        if attempt.domain != cohort.domain
            || attempt.epoch != cohort.epoch
            || attempt.definition != cohort.definition
            || parent.cut.through_seq.checked_add(1) != Some(head)
            || registered_source_delta(package, &attempt.reads, &attempt.projections)?
                != attempt.delta
        {
            return Err(DurableError::Conflict(
                "addressed creation original basis changed",
            ));
        }
        let owner = filesystem
            .hold_creation_owner(package, deadline, cancelled)
            .map_err(source_error)?;
        owner
            .verify_current(deadline, cancelled)
            .map_err(source_error)?;
        drop(owner);
        let domain = &attempt.domain;
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()?;
        tx.batch_execute("SET LOCAL statement_timeout='60s'; SET LOCAL work_mem='4MB'")?;
        let row = tx.query_one("SELECT d.*,f.generation AS audit_generation,f.maintenance_state FROM cmd2_domain d JOIN cmd2_audit_fence f USING(domain) WHERE d.domain=$1", &[&domain])?;
        cohort_matches(&row, cohort, true)?;
        if as_u64(row.get("audit_generation"))? != committed.audit_generation
            || as_u64(row.get("head_seq"))? != head
            || row.get::<_, Option<i64>>("source_generation") != Some(as_i64(head)?)
            || row.get::<_, String>("maintenance_state") != "normal"
            || !row.get::<_, bool>("rights_allowed")
            || row.get::<_, Option<String>>("schema_profile_digest")
                != Some(schema_profile_digest().to_hex())
            || database_oid(&mut tx)? != parent.cut.database_oid
        {
            return Err(DurableError::Conflict(
                "addressed metadata chain lost; explicit cold reopen required",
            ));
        }
        let registered = tx.query_one(
            "SELECT * FROM cmd2_attempt WHERE domain=$1 AND prepare_id=$2",
            &[&domain, &attempt.prepare_id],
        )?;
        let receipt = receipt_from_committed_attempt(&mut tx, &registered)?;
        if receipt.commit_seq != head
            || receipt.delta_digest != attempt.delta
            || registered.get::<_, Option<Vec<u8>>>("source_reads")
                != Some(reads_bytes(&attempt.reads)?)
            || registered.get::<_, Option<Vec<u8>>>("source_indexes")
                != Some(indexes_bytes(&attempt.indexes)?)
            || registered.get::<_, Option<Vec<u8>>>("source_projections")
                != Some(projections_bytes(&attempt.projections)?)
        {
            return Err(DurableError::Corrupt("addressed committed attempt differs"));
        }
        let members = tx.query(
            "SELECT * FROM cmd2_member WHERE domain=$1 AND prepare_id=$2 ORDER BY member_slot",
            &[&domain, &attempt.prepare_id],
        )?;
        if members.is_empty()
            || members.len() > MAX_MEMBERS
            || members.len() != package.changes().len()
            || members.len() != committed.receipts.len()
        {
            return Err(DurableError::Corrupt(
                "addressed committed member coverage differs",
            ));
        }
        // The complete changed interval is bounded before materialization. Its
        // first old value must prove membership (or absence) in the parent tree;
        // repeated changes/key moves/deletes then replay in strict fence order.
        let delta_limits = audit_delta::AuditDeltaLimits {
            max_rows: 4096,
            max_bytes: 4 * 1024 * 1024,
            max_key_bytes: MAX_MEMBERSHIP_KEY_BYTES,
        };
        let interval = audit_delta::load_interval_controlled_with_work(
            &mut tx,
            domain,
            parent.cut.audit_generation,
            delta_limits,
            Some(audit_delta::AuditDeltaControl {
                deadline,
                cancelled,
            }),
            &mut source_work.audit,
        )
        .map_err(delta_error)?;
        if interval.through_generation() != committed.audit_generation {
            return Err(DurableError::Conflict("addressed journal head differs"));
        }
        for (key, expected) in interval.initial_commitments() {
            active(deadline, cancelled)?;
            let framed = key.framed_tree_key().map_err(delta_error)?;
            let (prior, tree_work) = store.lookup_authenticated_tree_v2_with_work(
                &parent.cut.metadata,
                &framed,
                parent.limits,
                deadline,
                cancelled,
            )?;
            source_work.charge_tree(tree_work)?;
            if prior.as_deref() != expected.as_ref().map(|digest| digest.0.as_slice()) {
                return Err(DurableError::Corrupt(
                    "addressed journal predecessor membership differs",
                ));
            }
        }
        interval
            .verify_final_rows_controlled_with_work(
                &mut tx,
                domain,
                delta_limits,
                Some(audit_delta::AuditDeltaControl {
                    deadline,
                    cancelled,
                }),
                &mut source_work.audit,
            )
            .map_err(delta_error)?;
        // The commit proved the exact parent publication under its held
        // sequencer/audit locks before apply_source_change invalidated it.
        // This snapshot must contain that exact committed invalidation, not a
        // replacement publication; semantic journal membership was proved above.
        let selector = audit_delta::read_final_selector_controlled(
            &mut tx,
            domain,
            Some(audit_delta::AuditDeltaControl {
                deadline,
                cancelled,
            }),
        )
        .map_err(delta_error)?;
        if selector.domain != *domain
            || selector.head_seq != head
            || selector.published_seq != parent.cut.through_seq
            || selector.complete_cut_digest.is_some()
            || selector.complete_cut_generation.is_some()
            || selector.selected_generation_digest.is_some()
            || selector.source_projection_digest.is_some()
        {
            return Err(DurableError::Conflict(
                "addressed committed publication invalidation differs",
            ));
        }
        let mut metadata_changes = interval
            .expected_final_commitments()
            .iter()
            .map(|(key, value)| {
                Ok(AuthenticatedTreeDeltaV1 {
                    key: key
                        .framed_tree_key()
                        .map_err(|_| source_tree_error("addressed metadata key"))?,
                    value: value.map(|digest| digest.0.to_vec()),
                })
            })
            .collect::<tos_segment_store::Result<Vec<_>>>()?;
        metadata_changes.sort_by(|a, b| a.key.cmp(&b.key));
        let (metadata, tree_work) = store.apply_authenticated_tree_delta_v2_with_work(
            &parent.cut.metadata,
            metadata_changes.into_iter().map(Ok),
            parent.limits,
            deadline,
            cancelled,
        )?;
        source_work.charge_tree(tree_work)?;
        let paths = package
            .changes()
            .iter()
            .map(|c| c.path.as_str())
            .collect::<Vec<_>>();
        let histories = tx.query("SELECT * FROM cmd2_history WHERE domain=$1 AND commit_seq=$2 AND prepare_id=$3 ORDER BY member_slot", &[&domain,&as_i64(head)?,&attempt.prepare_id])?;
        let current = tx
            .query(
                "SELECT * FROM cmd2_current WHERE domain=$1 AND subject=ANY($2)",
                &[&domain, &paths],
            )?
            .into_iter()
            .map(|row| (row.get::<_, String>("subject"), row))
            .collect::<BTreeMap<_, _>>();
        if histories.len() != members.len() || current.len() != members.len() {
            return Err(DurableError::Corrupt(
                "addressed changed metadata coverage differs",
            ));
        }
        let carriers = creation_metadata(package);
        let mut member_root = Digest256Hasher::new();
        part(&mut member_root, b"cmd2-member-root-v1");
        part(&mut member_root, &(members.len() as u64).to_be_bytes());
        let mut history_changes = Vec::new();
        let mut current_changes = Vec::new();
        for ((member, history), change) in members.iter().zip(&histories).zip(package.changes()) {
            active(deadline, cancelled)?;
            let subject = change.path.as_str();
            let selected = committed
                .receipts
                .iter()
                .find(|r| r.binding().member_slot == member.get::<_, i32>("member_slot") as u32)
                .ok_or(DurableError::Corrupt("addressed verified frame absent"))?;
            check_member_row(member, selected, subject, 1)?;
            check_history_locator(history, selected, domain, subject, 1)?;
            let current = current
                .get(subject)
                .ok_or(DurableError::Corrupt("addressed current row absent"))?;
            if selected.binding().profile_id != CREATION
                || selected.coordinate().sha256
                    != Digest256::of_bytes(change.after.as_ref().unwrap())
                || row_source_metadata(member)? != carriers[subject]
                || row_source_metadata(history)? != carriers[subject]
                || row_source_metadata(current)? != carriers[subject]
                || metadata_locator_digest(history) != metadata_locator_digest(current)
                || history
                    .get::<_, Option<Vec<u8>>>("inventory_projection")
                    .as_ref()
                    != attempt.projections.get(subject)
                || current
                    .get::<_, Option<Vec<u8>>>("inventory_projection")
                    .as_ref()
                    != attempt.projections.get(subject)
            {
                return Err(DurableError::Corrupt(
                    "addressed changed source bindings differ",
                ));
            }
            update_member_root(&mut member_root, member);
            for (tag, revision, tree, changes) in [
                (
                    HISTORY_KEY_TAG,
                    Some(1),
                    &parent.cut.history,
                    &mut history_changes,
                ),
                (
                    CURRENT_KEY_TAG,
                    None,
                    &parent.cut.current,
                    &mut current_changes,
                ),
            ] {
                let key = membership_key(tag, domain, subject, revision)?;
                let (prior, tree_work) = store.lookup_authenticated_tree_v2_with_work(
                    tree,
                    &key,
                    parent.limits,
                    deadline,
                    cancelled,
                )?;
                source_work.charge_tree(tree_work)?;
                if prior.is_some() {
                    return Err(DurableError::Corrupt(
                        "addressed creation predecessor not absent",
                    ));
                }
                let row = PlacementGenerationRowV1 {
                    key: key.clone(),
                    logical_digest: selected.coordinate().sha256,
                    logical_length: selected.coordinate().size_bytes,
                    placement: selected.placement(),
                };
                changes.push(AuthenticatedTreeDeltaV1 {
                    key,
                    value: Some(encode_placement_tree_row(
                        &row,
                        parent.limits.max_value_bytes,
                    )?),
                });
            }
        }
        if member_root.finalize() != receipt.member_root {
            return Err(DurableError::Corrupt("addressed member root differs"));
        }
        history_changes.sort_by(|a, b| a.key.cmp(&b.key));
        current_changes.sort_by(|a, b| a.key.cmp(&b.key));
        let (history, tree_work) = store.apply_authenticated_tree_delta_v2_with_work(
            &parent.cut.history,
            history_changes.into_iter().map(Ok),
            parent.limits,
            deadline,
            cancelled,
        )?;
        source_work.charge_tree(tree_work)?;
        let (current, tree_work) = store.apply_authenticated_tree_delta_v2_with_work(
            &parent.cut.current,
            current_changes.into_iter().map(Ok),
            parent.limits,
            deadline,
            cancelled,
        )?;
        source_work.charge_tree(tree_work)?;
        if parent.cut.history.entries.checked_add(members.len() as u64) != Some(history.entries)
            || parent.cut.current.entries.checked_add(members.len() as u64) != Some(current.entries)
        {
            return Err(DurableError::Corrupt(
                "addressed successor membership counts differ",
            ));
        }
        let inventory = parent.cut.inventory.apply_changed_with_work(
            store,
            &attempt.projections,
            parent.limits,
            deadline,
            cancelled,
            &mut source_work.tree,
        )?;
        let mut log_frontier = parent.log_frontier.0.clone();
        if log_frontier.clone().finalize() != parent.cut.log_digest {
            return Err(DurableError::Corrupt("addressed log prefix differs"));
        }
        let log = tx.query_one("SELECT commit_seq,event_kind,command_id,delta_digest,members_root FROM cmd2_log WHERE domain=$1 AND commit_seq=$2", &[&domain,&as_i64(head)?])?;
        let kind: String = log.get(1);
        let command: String = log.get(2);
        let delta: String = log.get(3);
        let root: String = log.get(4);
        if as_u64(log.get(0))? != head
            || kind != "command"
            || command != receipt.command_id
            || delta != receipt.delta_digest.to_hex()
            || root != receipt.member_root.to_hex()
        {
            return Err(DurableError::Corrupt("addressed final log differs"));
        }
        for bytes in [
            &as_i64(head)?.to_be_bytes()[..],
            kind.as_bytes(),
            command.as_bytes(),
            delta.as_bytes(),
            root.as_bytes(),
        ] {
            part(&mut log_frontier, bytes);
        }
        let cut = AddressedCutV2 {
            store_id: store.store_id(),
            domain_digest: store.domain_digest(),
            domain: domain.clone(),
            through_seq: head,
            audit_generation: committed.audit_generation,
            database_oid: parent.cut.database_oid,
            schema_profile_digest: schema_profile_digest(),
            state_profile_digest: audit_delta::audit_delta_schema_digest(),
            log_digest: log_frontier.clone().finalize(),
            metadata,
            history,
            current,
            inventory,
        };
        tx.commit()?;
        let candidate = self.install_addressed_selection(
            store,
            cut,
            VerifiedLogFrontier(log_frontier),
            parent.limits,
            deadline,
            cancelled,
        )?;
        let owner = filesystem
            .hold_creation_owner(package, deadline, cancelled)
            .map_err(source_error)?;
        let mut successor_cohort = cohort.clone();
        successor_cohort.generation = head;
        let selected = self.select_addressed_selection(
            store,
            candidate,
            &successor_cohort,
            Some(&owner),
            deadline,
            cancelled,
        )?;
        Ok(
            crate::source_current_cut::ManagedCurrentSourceGeneration::from_verified_addressed(
                store,
                successor_cohort,
                selected,
            ),
        )
    }
}

pub(super) fn addressed_replay_limits() -> AuthenticatedTreeLimitsV1 {
    AuthenticatedTreeLimitsV1 {
        max_key_bytes: MAX_MEMBERSHIP_KEY_BYTES,
        max_value_bytes: 1_048_576,
        max_kind_bytes: 128,
        max_node_bytes: 1_048_576,
        max_children: 16,
        max_nodes: 4096,
        max_total_bytes: 64 * 1024 * 1024,
        max_rows: 4096,
    }
}
impl DurablePgCoordinator {
    pub(super) fn replay_addressed_inventory(
        store: &SegmentStore,
        basis: &ManagedCreationBasis,
        context: &CommandContext,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<ManagedAgentInventory> {
        active(deadline, cancelled)?;
        // The basis comes from the already committed exact attempt's original
        // SourceReads, not from an arbitrary caller-supplied STO object digest.
        let raw = store.read_authenticated_object_v1(
            OBJECT_KIND,
            basis.digest,
            MAX_DESCRIPTOR,
            deadline,
            cancelled,
        )?;
        let cut = AddressedCutV2::decode(&raw)?;
        cut.require_store(store)?;
        if cut.domain != basis.domain || cut.through_seq != basis.generation {
            return Err(DurableError::Corrupt(
                "retained addressed inventory basis differs",
            ));
        }
        cut.inventory.select(
            store,
            context,
            addressed_replay_limits(),
            deadline,
            cancelled,
        )
    }
}

impl VerifiedAddressedGeneration {
    pub(crate) fn lookup_history(
        &self,
        domain: &str,
        path: &RelativePath,
        revision: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<Option<PlacementGenerationRowV1>> {
        self.audited_root.require_store(&self.store)?;
        self.cut.require_store(&self.store)?;
        if domain != self.cut.domain {
            return Err(DurableError::Conflict("addressed history domain differs"));
        }
        let key = membership_key(HISTORY_KEY_TAG, domain, path.as_str(), Some(revision))?;
        let value = self.store.lookup_authenticated_tree_v2(
            &self.cut.history,
            &key,
            self.limits,
            deadline,
            cancelled,
        )?;
        value
            .map(|raw| {
                let row = decode_placement_tree_row(&raw, self.limits.max_value_bytes)?;
                if row.key != key {
                    return Err(DurableError::Corrupt(
                        "addressed placement value key differs",
                    ));
                }
                Ok(row)
            })
            .transpose()
    }
}
impl DurablePgCoordinator {
    pub(crate) fn read_generation_historical_source_member(
        &mut self,
        store: &SegmentStore,
        generation: &crate::source_current_cut::ManagedCurrentSourceGeneration,
        path: &RelativePath,
        revision: u64,
        max_bytes: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<ManagedCurrentMember> {
        active(deadline, cancelled)?;
        if revision == 0 || max_bytes == 0 || max_bytes > 8_388_608 {
            return Err(DurableError::Refused("historical source read envelope"));
        }
        generation
            .selected()
            .facts()
            .audited_root
            .require_store(store)?;
        let domain = generation.cohort().domain();
        let selected = generation
            .selected()
            .lookup_history(domain, path, revision, deadline, cancelled)?
            .ok_or(DurableError::Conflict(
                "selected historical source member absent",
            ))?;
        if selected.logical_length > max_bytes {
            return Err(DurableError::Refused(
                "historical selected member byte bound",
            ));
        }
        let mut observation = self.client.transaction()?;
        observation
            .batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        held_generation_metadata(&mut observation, store, generation)?;
        let observed=observation.query_one("SELECT h.*,a.attempt_fence AS source_attempt_fence FROM cmd2_history h JOIN cmd2_attempt a USING(domain,prepare_id) WHERE h.domain=$1 AND h.subject=$2 AND h.revision=$3", &[&domain,&path.as_str(),&as_i64(revision)?])?;
        if as_u64(observed.get("commit_seq"))? > generation.commit_seq() {
            return Err(DurableError::Conflict(
                "historical member after selected generation",
            ));
        }
        let prepare: Vec<u8> = observed.get("prepare_id");
        let fence = as_u64(observed.get("source_attempt_fence"))?;
        let observed_digest = metadata_locator_digest(&observed);
        observation.commit()?;
        let receipts = match store.recover_attempt_fenced(&prepare, fence, 0)? {
            Some(AttemptRecovery::Sealed { receipts }) => receipts,
            _ => {
                return Err(DurableError::Corrupt(
                    "historical source sealed intent absent",
                ));
            }
        };
        let receipt = receipts
            .iter()
            .find(|r| r.receipt_id() == selected.placement.receipt_id())
            .ok_or(DurableError::Corrupt("historical selected receipt absent"))?;
        if selected.placement != receipt.placement() {
            return Err(DurableError::Corrupt(
                "historical physical selection differs",
            ));
        }
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        held_generation_metadata(&mut tx, store, generation)?;
        let row = tx.query_one(
            "SELECT * FROM cmd2_history WHERE domain=$1 AND subject=$2 AND revision=$3",
            &[&domain, &path.as_str(), &as_i64(revision)?],
        )?;
        check_history_locator(&row, receipt, domain, path.as_str(), revision)?;
        if observed_digest != metadata_locator_digest(&row)
            || row.get::<_, String>("content_digest") != selected.logical_digest.to_hex()
            || as_u64(row.get("content_length"))? != selected.logical_length
        {
            return Err(DurableError::Conflict(
                "historical metadata selection differs",
            ));
        }
        let (metadata, dependency_claims) = expose_metadata(&row, path)?;
        let mut raw = Vec::new();
        store.read_selected(receipt, max_bytes, &mut raw)?;
        active(deadline, cancelled)?;
        held_generation_metadata(&mut tx, store, generation)?;
        tx.commit()?;
        Ok(ManagedCurrentMember {
            path: path.clone(),
            custody_revision: revision,
            current_generation: generation.commit_seq(),
            commit_seq: as_u64(row.get("commit_seq"))?,
            raw,
            metadata,
            dependency_claims,
            placement: receipt.placement(),
        })
    }
}
