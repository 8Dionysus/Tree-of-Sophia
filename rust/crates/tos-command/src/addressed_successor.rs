//! Opt-in V2 source selection. V1 transcript meanings remain unchanged.
//! The immutable tree proves membership; current rights and publication are
//! always checked by the existing coordinator/owner fences.
use super::super::audit_delta;
use super::*;
use tos_segment_store::{
    AuthenticatedTreeDeltaV1, AuthenticatedTreeDescriptorV2, AuthenticatedTreeEntryV1,
    AuthenticatedTreeLimitsV1, AuthenticatedTreeWorkV1, decode_placement_tree_row,
    encode_placement_tree_row,
};

const OBJECT_KIND: &[u8] = b"cmd2-managed-addressed-selection-v2";
const METADATA_KIND: &[u8] = b"cmd2-private-row-commitments-v2";
const HISTORY_KIND: &[u8] = b"cmd2-history-addressed-v2";
const CURRENT_KIND: &[u8] = b"cmd2-current-addressed-v2";
const MAX_DESCRIPTOR: usize = 65536;

// A query_raw result stays active on the server while a slow tree builder
// consumes it. A bounded portal suspends the SQL between pages, so the SQL
// statement timeout measures a database fetch, not client-side packing. The
// caller keeps the same transaction, snapshot, order and owner fences.
pub(super) struct BoundedPgRows<'a, 'connection> {
    tx: &'a mut Transaction<'connection>,
    portal: postgres::Portal,
    page: std::vec::IntoIter<postgres::Row>,
    ended: bool,
}

pub(super) fn bounded_pg_rows<'a, 'connection>(
    tx: &'a mut Transaction<'connection>,
    query: &str,
    params: &[&(dyn postgres::types::ToSql + Sync)],
) -> Result<BoundedPgRows<'a, 'connection>, postgres::Error> {
    let portal = tx.bind(query, params)?;
    Ok(BoundedPgRows {
        tx,
        portal,
        page: Vec::new().into_iter(),
        ended: false,
    })
}

impl Iterator for BoundedPgRows<'_, '_> {
    type Item = Result<postgres::Row, postgres::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(row) = self.page.next() {
            return Some(Ok(row));
        }
        if self.ended {
            return None;
        }
        match self.tx.query_portal(&self.portal, 128) {
            Ok(rows) => {
                self.ended = rows.len() < 128;
                self.page = rows.into_iter();
                self.page.next().map(Ok)
            }
            Err(error) => {
                self.ended = true;
                Some(Err(error))
            }
        }
    }
}

pub(super) fn build_mixed_inventory_from_retained_projections(
    tx: &mut Transaction<'_>,
    store: &SegmentStore,
    domain: &str,
    generation: u64,
    limits: AuthenticatedTreeLimitsV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<super::addressed_inventory::AddressedMixedInventoryV1> {
    active(deadline, cancelled)?;
    let admission = format!(
        "WITH members AS ({RETAINED_PROJECTIONS}) SELECT count(*),coalesce(sum(octet_length(inventory_projection)),0),count(*) FILTER(WHERE inventory_projection IS NULL) FROM members"
    );
    let sequence = as_i64(generation)?;
    let admitted = tx.query_one(&admission, &[&domain, &sequence])?;
    let expected_count = as_u64(admitted.get(0))?;
    if expected_count > limits.max_rows
        || as_u64(admitted.get(1))? > 64 * 1024 * 1024
        || admitted.get::<_, i64>(2) != 0
    {
        return Err(DurableError::Refused(
            "mixed source projection exceeds the cold inventory envelope or is incomplete",
        ));
    }

    let sql = format!(
        "WITH members AS ({RETAINED_PROJECTIONS}) SELECT * FROM members ORDER BY subject COLLATE \"C\""
    );
    let mut rows = bounded_pg_rows(tx, &sql, &[&domain, &sequence])?;
    let projections = std::iter::from_fn(|| match rows.next().transpose() {
        Ok(Some(row)) => {
            let result = (|| -> DurableResult<(String, Vec<u8>)> {
                active(deadline, cancelled)?;
                let subject: String = row.get("subject");
                let raw = row
                    .get::<_, Option<Vec<u8>>>("inventory_projection")
                    .ok_or(DurableError::Refused(
                        "managed mixed inventory projection incomplete; FullOnly required",
                    ))?;
                super::projection_value(&row)?;
                Ok((subject, raw))
            })();
            Some(result)
        }
        Ok(None) => None,
        Err(error) => Some(Err(DurableError::Database(error))),
    });
    let result =
        super::addressed_inventory::AddressedMixedInventoryV1::build_from_assessed_projections(
            store,
            projections,
            expected_count,
            limits,
            deadline,
            cancelled,
            &mut AuthenticatedTreeWorkV1::default(),
        );
    drop(rows);
    result
}

fn tree_callback_state_bytes(limits: AuthenticatedTreeLimitsV1) -> DurableResult<usize> {
    usize::try_from(limits.max_total_bytes)
        .map_err(|_| DurableError::Refused("addressed tree state bound"))
}

// Both compatibility migration and direct streamed cold assessment consume the
// SAME verified membership carrier. This helper creates no cold/source grant:
// the caller must already own its complete receipt/history/pin assessment.
pub(super) fn build_addressed_membership_trees(
    store: &SegmentStore,
    membership: &MembershipInstallation<'_>,
    limits: AuthenticatedTreeLimitsV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<(AuthenticatedTreeDescriptorV2, AuthenticatedTreeDescriptorV2)> {
    let mut owner_check = || true;
    build_addressed_membership_trees_with_callback(
        store,
        membership,
        limits,
        deadline,
        cancelled,
        &mut owner_check,
    )
}

pub(super) fn build_addressed_membership_trees_with_callback(
    store: &SegmentStore,
    membership: &MembershipInstallation<'_>,
    limits: AuthenticatedTreeLimitsV1,
    deadline: Instant,
    cancelled: &AtomicBool,
    owner_check: &mut dyn FnMut() -> bool,
) -> DurableResult<(AuthenticatedTreeDescriptorV2, AuthenticatedTreeDescriptorV2)> {
    membership.audited_root.require_store(store)?;
    if !owner_check() {
        return Err(DurableError::Conflict(
            "addressed membership owner fence changed",
        ));
    }
    let state_bytes = tree_callback_state_bytes(limits)?;
    let mut build = |namespace, kind| -> DurableResult<AuthenticatedTreeDescriptorV2> {
        let mut cursor = membership.cursor(namespace)?;
        let entries = std::iter::from_fn(|| match cursor.next_row() {
            Ok(Some(row)) => Some((|| {
                active(deadline, cancelled)
                    .map_err(|_| source_tree_error("cold membership deadline"))?;
                Ok(AuthenticatedTreeEntryV1 {
                    value: encode_placement_tree_row(&row, limits.max_value_bytes)?,
                    key: row.key,
                })
            })()),
            Ok(None) => None,
            Err(_) => Some(Err(source_tree_error(
                "cold verified membership cursor failed",
            ))),
        });
        let (tree, _tree_work) = store
            .build_authenticated_tree_v2_with_work_and_io_and_state_and_callback(
                kind,
                entries,
                limits,
                None,
                state_bytes,
                0,
                deadline,
                cancelled,
                owner_check,
            )
            .map_err(DurableError::Storage)?;
        cursor.finish()?;
        if tree.semantic.entries != membership.count(namespace) {
            return Err(DurableError::Corrupt(
                "addressed bootstrap EOF counts differ",
            ));
        }
        active(deadline, cancelled)?;
        Ok(tree)
    };
    let history = build(GenerationNamespaceV1::History, HISTORY_KIND)?;
    let current = build(GenerationNamespaceV1::Current, CURRENT_KIND)?;
    if history.entries != membership.descriptor_cut.historical_members
        || current.entries != membership.descriptor_cut.current_members
    {
        return Err(DurableError::Corrupt(
            "addressed bootstrap descriptor counts differ",
        ));
    }
    Ok((history, current))
}

// This is a physical builder only. Its caller owns the same held source/audit
// snapshot and complete semantic assessment; the tree never grants admission.
pub(super) fn build_addressed_metadata_tree(
    tx: &mut Transaction<'_>,
    store: &SegmentStore,
    domain: &str,
    limits: AuthenticatedTreeLimitsV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<AuthenticatedTreeDescriptorV2> {
    let mut owner_check = || true;
    build_addressed_metadata_tree_with_callback(
        tx,
        store,
        domain,
        limits,
        deadline,
        cancelled,
        &mut owner_check,
    )
}

pub(super) fn build_addressed_metadata_tree_with_callback(
    tx: &mut Transaction<'_>,
    store: &SegmentStore,
    domain: &str,
    limits: AuthenticatedTreeLimitsV1,
    deadline: Instant,
    cancelled: &AtomicBool,
    owner_check: &mut dyn FnMut() -> bool,
) -> DurableResult<AuthenticatedTreeDescriptorV2> {
    if !owner_check() {
        return Err(DurableError::Conflict(
            "addressed metadata owner fence changed",
        ));
    }
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
    let mut rows = bounded_pg_rows(tx, &sql, &[&domain])?;
    let entries = std::iter::from_fn(|| match rows.next().transpose() {
        Ok(Some(row)) => Some((|| {
            active(deadline, cancelled).map_err(|_| source_tree_error("cold metadata deadline"))?;
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
    let state_bytes = tree_callback_state_bytes(limits)?;
    let (metadata, _metadata_work) = store
        .build_authenticated_tree_v2_with_work_and_io_and_state_and_callback(
            METADATA_KIND,
            entries,
            limits,
            None,
            state_bytes,
            0,
            deadline,
            cancelled,
            owner_check,
        )
        .map_err(DurableError::Storage)?;
    drop(rows);
    active(deadline, cancelled)?;
    if !owner_check() {
        return Err(DurableError::Conflict(
            "addressed metadata owner fence changed",
        ));
    }
    Ok(metadata)
}

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
    pub inventory: super::addressed_inventory::AddressedMixedInventoryV1,
}
#[derive(Clone, Debug)]
pub(crate) struct VerifiedAddressedGeneration {
    pub(crate) audited_root: AuditedStoreRoot,
    pub(crate) store: SegmentStore,
    pub(crate) cut: AddressedCutV2,
    pub(crate) digest: Digest256,
    pub(crate) state_digest: Digest256,
    pub(crate) selected_audit_generation: u64,
    /// Present only when this process has just exhaustively verified the
    /// OID-bound transcript. A retained descriptor proves its digest, not the
    /// internal streaming hasher state needed to extend a later successor.
    pub(crate) log_frontier: Option<VerifiedLogFrontier>,
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
            inventory: super::addressed_inventory::AddressedMixedInventoryV1::decode(&bytes(
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
        if self.state_profile_digest != audit_delta::audit_delta_schema_digest() {
            return Err(DurableError::Refused(
                "addressed metadata coverage changed; explicit cold reopen required",
            ));
        }
        self.require_store_profile(store, audit_delta::audit_delta_schema_digest())
    }
    fn require_store_profile(
        &self,
        store: &SegmentStore,
        expected_profile: Digest256,
    ) -> DurableResult<()> {
        self.inventory.require_store(store)?;
        if self.store_id != store.store_id()
            || self.domain_digest != store.domain_digest()
            || self.domain.as_bytes() != store.custody_domain()
            || self.schema_profile_digest != schema_profile_digest()
            || !audit_delta::profiles_are_compatible(self.state_profile_digest, expected_profile)
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

/// Verify that the currently published selector still points at its exact V2
/// descriptor and that its authenticated inventory is freshly reconstructed
/// from source. `Ok(false)` only means the V2 parser did not identify the
/// selected bytes; callers must positively open the selected V1 generation
/// before treating it as a legacy transition. A descriptor exceeding the V2
/// bound must first decode as V1 under its own declared profile. Other
/// read/decode failures remain fail-closed.
pub(super) fn verify_published_addressed_inventory(
    store: &SegmentStore,
    row: &postgres::Row,
    domain: &str,
    head: u64,
    database_oid: u64,
    context: &CommandContext,
    projections: &ProjectionRows,
    allow_database_rebind: bool,
    observed_profile: Digest256,
    v1_limits: GenerationReadLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<bool> {
    let Some(selected_digest) = row.get::<_, Option<String>>("selected_generation_digest") else {
        return Ok(false);
    };
    let selected_digest = Digest256::from_hex(selected_digest.trim_end())
        .map_err(|_| DurableError::Corrupt("addressed selected generation digest"))?;
    let raw = match store.read_authenticated_object_v1(
        OBJECT_KIND,
        selected_digest,
        MAX_DESCRIPTOR,
        deadline,
        cancelled,
    ) {
        Ok(raw) => raw,
        // V1 descriptors use the generation format rather than this V2
        // authenticated-object envelope. This is only a tentative dispatch;
        // the caller must cold-open the exact V1 descriptor and cut.
        Err(error)
            if error.code == tos_segment_store::SegmentErrorCode::InvalidFormat
                && error.detail == "authenticated object header differs" =>
        {
            return Ok(false);
        }
        // Another authenticated object kind also needs positive V1 validation
        // by the caller. The kind mismatch alone grants no legacy selection.
        Err(error)
            if error.code == tos_segment_store::SegmentErrorCode::InvalidReceipt
                && error.detail == "authenticated object kind differs" =>
        {
            return Ok(false);
        }
        // A streamed V1 descriptor can exceed the compact V2 descriptor cap.
        // Keep each format's own limit: positively identify the exact V1
        // digest/domain/store here, then require the caller's complete cold
        // verification of its cut and membership before any re-selection.
        Err(error)
            if error.code == tos_segment_store::SegmentErrorCode::BudgetExceeded
                && error.detail == "authenticated object exceeds read limit" =>
        {
            active(deadline, cancelled)?;
            store.inspect_generation_descriptor_v1(selected_digest, v1_limits)?;
            active(deadline, cancelled)?;
            return Ok(false);
        }
        Err(error) => return Err(DurableError::Storage(error)),
    };
    let cut = AddressedCutV2::decode(&raw)?;
    if observed_profile != audit_delta::audit_delta_schema_digest()
        && !(allow_database_rebind && audit_delta::profile_is_supported(observed_profile))
    {
        return Err(DurableError::Corrupt("cold addressed profile unsupported"));
    }
    cut.require_store_profile(store, observed_profile)?;
    let state_digest = cut.state_digest()?;
    let selected_audit_generation =
        cut.audit_generation
            .checked_add(1)
            .ok_or(DurableError::Corrupt(
                "addressed selected audit generation overflow",
            ))?;
    let projection_root = cut.inventory.root().to_hex();
    let complete_cut_digest =
        row.get::<_, Option<String>>("complete_cut_digest")
            .ok_or(DurableError::Corrupt(
                "addressed complete cut digest absent",
            ))?;
    let source_projection_digest = row
        .get::<_, Option<String>>("source_projection_digest")
        .ok_or(DurableError::Corrupt(
            "addressed source projection digest absent",
        ))?;
    let schema_digest = row
        .get::<_, Option<String>>("schema_profile_digest")
        .ok_or(DurableError::Corrupt(
            "addressed schema profile digest absent",
        ))?;
    // A full-audit restore/reselection may authenticate a retained descriptor
    // whose database identity belongs to the source database. The caller must
    // then clear it and build a new selection bound to the current OID; this
    // old descriptor is never returned as current authority.
    if cut.domain != domain
        || cut.through_seq != head
        || (!allow_database_rebind && cut.database_oid != database_oid)
        || row.get::<_, i64>("head_seq") != as_i64(head)?
        || row.get::<_, i64>("published_seq") != as_i64(head)?
        || row.get::<_, Option<i64>>("source_generation") != Some(as_i64(head)?)
        || row.get::<_, Option<i64>>("complete_cut_generation")
            != Some(as_i64(selected_audit_generation)?)
        || complete_cut_digest.trim_end() != state_digest.to_hex()
        || source_projection_digest.trim_end() != projection_root
        || schema_digest.trim_end() != cut.schema_profile_digest.to_hex()
        || !audit_delta::profiles_are_compatible(cut.state_profile_digest, observed_profile)
        || cut.schema_profile_digest != schema_profile_digest()
    {
        return Err(DurableError::Corrupt(
            "published addressed source selection binding differs",
        ));
    }
    cut.inventory.verify_source_projections(
        store,
        context,
        projections,
        addressed_replay_limits(),
        deadline,
        cancelled,
    )?;
    Ok(true)
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

pub(super) fn delta_error(error: audit_delta::AuditDeltaError) -> DurableError {
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

#[cfg(all(test, feature = "postgres-lab"))]
#[test]
#[ignore = "requires explicit isolated TOS_CMD_POSTGRES_URL"]
fn addressed_cold_pages_suspend_sql_during_slow_consumer_and_keep_snapshot() {
    let url = std::env::var("TOS_CMD_POSTGRES_URL").expect("explicit isolated PG URL");
    let mut client = postgres::Client::connect(&url, postgres::NoTls).unwrap();
    let mut observer = postgres::Client::connect(&url, postgres::NoTls).unwrap();
    client
        .batch_execute(
            "CREATE SCHEMA tos_addressed_page_test;
         CREATE TABLE tos_addressed_page_test.rows(id integer PRIMARY KEY);
         INSERT INTO tos_addressed_page_test.rows SELECT generate_series(1,1025)",
        )
        .unwrap();
    let pid: i32 = client
        .query_one("SELECT pg_backend_pid()", &[])
        .unwrap()
        .get(0);
    let mut tx = client
        .build_transaction()
        .isolation_level(postgres::IsolationLevel::RepeatableRead)
        .start()
        .unwrap();
    tx.batch_execute("SET LOCAL statement_timeout='500ms'")
        .unwrap();
    let mut rows = bounded_pg_rows(
        &mut tx,
        "SELECT id,repeat('x',32768) FROM tos_addressed_page_test.rows ORDER BY id",
        &[],
    )
    .unwrap();
    let first = rows.next().unwrap().unwrap();
    assert_eq!(first.get::<_, i32>(0), 1);
    // The source is larger than transport buffers. A client-side packing pause
    // exceeds the SQL timeout, while the actual bounded fetches remain short.
    assert_eq!(
        observer
            .query_one("SELECT state FROM pg_stat_activity WHERE pid=$1", &[&pid])
            .unwrap()
            .get::<_, String>(0),
        "idle in transaction"
    );
    observer
        .execute("INSERT INTO tos_addressed_page_test.rows VALUES(1026)", &[])
        .unwrap();
    std::thread::sleep(Duration::from_millis(750));
    for expected in 2..=1025 {
        let row = rows.next().unwrap().unwrap();
        assert_eq!(row.get::<_, i32>(0), expected);
        assert_eq!(row.get::<_, String>(1).len(), 32768);
    }
    assert!(rows.next().is_none());
    drop(rows);
    assert_eq!(
        tx.query_one("SELECT count(*) FROM tos_addressed_page_test.rows", &[])
            .unwrap()
            .get::<_, i64>(0),
        1025
    );
    tx.commit().unwrap();
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM tos_addressed_page_test.rows", &[])
            .unwrap()
            .get::<_, i64>(0),
        1026
    );
    let mut tx = client.transaction().unwrap();
    let mut rows =
        bounded_pg_rows(&mut tx, "SELECT 1/(n-1) FROM generate_series(1,1) n", &[]).unwrap();
    assert!(rows.next().unwrap().is_err());
    assert!(rows.next().is_none());
    drop(rows);
    tx.rollback().unwrap();
    client
        .batch_execute("DROP SCHEMA tos_addressed_page_test CASCADE")
        .unwrap();
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

    /// Require the addressed audit companion without installing or changing
    /// schema. Existing-schema bootstrap paths use this before any source or
    /// store writes; only the explicitly authorized initialization route may
    /// call `enable_addressed_audit_protocol`.
    pub(crate) fn require_addressed_audit_protocol(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<()> {
        active(deadline, cancelled)?;
        let deadline = deadline.min(Instant::now() + Duration::from_secs(5));
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        audit_delta::verify_sql_binding_types(
            &mut tx,
            audit_delta::AuditDeltaControl {
                deadline,
                cancelled,
            },
        )
        .map_err(delta_error)?;
        active(deadline, cancelled)?;
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
        let metadata =
            build_addressed_metadata_tree(&mut tx, store, domain, limits, deadline, cancelled)?;
        let (history_tree, current_tree) =
            build_addressed_membership_trees(store, &legacy, limits, deadline, cancelled)?;
        let inventory = build_mixed_inventory_from_retained_projections(
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
            VerifiedLogFrontier(
                generation
                    .selected()
                    .log_frontier()
                    .ok_or(DurableError::Refused(
                        "addressed migration requires a live verified log frontier",
                    ))?
                    .clone(),
            ),
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

    pub(super) fn install_addressed_selection(
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
            log_frontier: Some(frontier),
            limits,
        })
    }

    pub(super) fn select_addressed_selection(
        &mut self,
        store: &SegmentStore,
        candidate: VerifiedAddressedGeneration,
        cohort: &ManagedSourceCohort,
        owner: Option<&CreationOwnerFence<'_>>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<VerifiedAddressedGeneration> {
        self.select_addressed_selection_controlled(
            store, candidate, cohort, owner, None, deadline, cancelled,
        )
    }

    pub(super) fn select_addressed_selection_controlled(
        &mut self,
        store: &SegmentStore,
        candidate: VerifiedAddressedGeneration,
        cohort: &ManagedSourceCohort,
        owner: Option<&CreationOwnerFence<'_>>,
        max_statement_ms: Option<u64>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<VerifiedAddressedGeneration> {
        self.select_addressed_selection_controlled_with_callback(
            store,
            candidate,
            cohort,
            owner,
            max_statement_ms,
            deadline,
            cancelled,
            None,
        )
    }

    pub(super) fn select_addressed_selection_controlled_with_callback(
        &mut self,
        store: &SegmentStore,
        mut candidate: VerifiedAddressedGeneration,
        cohort: &ManagedSourceCohort,
        owner: Option<&CreationOwnerFence<'_>>,
        max_statement_ms: Option<u64>,
        deadline: Instant,
        cancelled: &AtomicBool,
        protected_check: Option<&dyn Fn() -> DurableResult<()>>,
    ) -> DurableResult<VerifiedAddressedGeneration> {
        active(deadline, cancelled)?;
        candidate.audited_root.require_store(store)?;
        candidate.cut.require_store(store)?;
        let cut = &candidate.cut;
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        if let Some(milliseconds) = max_statement_ms {
            super::source_cohort_streamed::set_streamed_pg_limits(
                &mut tx,
                milliseconds,
                deadline,
                cancelled,
            )?;
        }
        let audit = lock_audit_fence(&mut tx, &cut.domain)?;
        if let Some(milliseconds) = max_statement_ms {
            super::source_cohort_streamed::set_streamed_pg_limits(
                &mut tx,
                milliseconds,
                deadline,
                cancelled,
            )?;
        }
        let row = tx.query_one(
            "SELECT * FROM cmd2_domain WHERE domain=$1 FOR UPDATE",
            &[&cut.domain],
        )?;
        cohort_matches(&row, cohort, true)?;
        if audit != cut.audit_generation
            || as_u64(row.get("head_seq"))? != cut.through_seq
            || {
                if let Some(milliseconds) = max_statement_ms {
                    super::source_cohort_streamed::set_streamed_pg_limits(
                        &mut tx,
                        milliseconds,
                        deadline,
                        cancelled,
                    )?;
                }
                database_oid(&mut tx)? != cut.database_oid
            }
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
        if let Some(milliseconds) = max_statement_ms {
            super::source_cohort_streamed::set_streamed_pg_limits(
                &mut tx,
                milliseconds,
                deadline,
                cancelled,
            )?;
        }
        if let Some(check) = protected_check {
            check()?;
        }
        tx.execute("UPDATE cmd2_domain SET published_seq=$2,complete_cut_digest=$3,complete_cut_generation=$4,selected_generation_digest=$5,source_projection_digest=$6 WHERE domain=$1",
            &[&cut.domain,&as_i64(cut.through_seq)?,&candidate.state_digest.to_hex(),&as_i64(anticipated)?,
              &candidate.digest.to_hex(),&cut.inventory.root().to_hex()])?;
        if let Some(milliseconds) = max_statement_ms {
            super::source_cohort_streamed::set_streamed_pg_limits(
                &mut tx,
                milliseconds,
                deadline,
                cancelled,
            )?;
        }
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
        if let Some(milliseconds) = max_statement_ms {
            super::source_cohort_streamed::set_streamed_pg_limits(
                &mut tx,
                milliseconds,
                deadline,
                cancelled,
            )?;
        }
        if let Some(check) = protected_check {
            check()?;
        }
        tx.commit()?;
        candidate.selected_audit_generation = observed;
        Ok(candidate)
    }
}

impl DurablePgCoordinator {
    /// Reopen the immutable addressed descriptor named by a persisted source
    /// navigation proof without treating its old PG selection as current.
    /// The descriptor, proof and model establish only the bootstrap anchor;
    /// the navigation-specific live lease checks today's PG owner state at
    /// every disclosure.
    pub(crate) fn reopen_persisted_addressed_navigation_anchor(
        &mut self,
        store: &SegmentStore,
        proof: &tos_compiler::managed_source::ManagedSourceProofV2,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<crate::source_current_cut::ManagedCurrentSourceGeneration> {
        active(deadline, cancelled)?;
        proof
            .validate()
            .map_err(|_| DurableError::Refused("persisted managed source proof is invalid"))?;
        let described = &proof.generation;
        let digest = Digest256::from_hex(&described.installed_generation_sha256)
            .map_err(|_| DurableError::Corrupt("persisted addressed generation digest"))?;
        let parse_digest = |value: &str| {
            Digest256::from_hex(value)
                .map_err(|_| DurableError::Corrupt("persisted addressed cohort digest"))
        };
        let cohort = ManagedSourceCohort {
            domain: described.domain.clone(),
            store_id: described.store_id,
            initial_revision: SourceRevision(parse_digest(&described.bootstrap_source_revision)?),
            initial_membership: SourceMembershipV1 {
                digest: parse_digest(&described.bootstrap_membership_sha256)?,
                count: described.bootstrap_members,
            },
            epoch: described.epoch,
            definition: parse_digest(&described.definition_sha256)?,
            generation: described.through_commit_seq,
        };
        if cohort.store_id != store.store_id()
            || parse_digest(&described.domain_sha256)? != store.domain_digest()
            || cohort.domain.as_bytes() != store.custody_domain()
        {
            return Err(DurableError::Conflict(
                "persisted addressed source store/domain differs",
            ));
        }

        // This authenticated immutable object is deliberately opened without
        // consulting cmd2_domain. A later ordinary write advances that live
        // owner row while the bootstrap descriptor remains the model anchor.
        let raw = store.read_authenticated_object_v1(
            OBJECT_KIND,
            digest,
            MAX_DESCRIPTOR,
            deadline,
            cancelled,
        )?;
        let cut = AddressedCutV2::decode(&raw)?;
        cut.require_store(store)?;
        let state_digest = cut.state_digest()?;
        if cut.domain != cohort.domain
            || cut.through_seq != described.through_commit_seq
            || cut.audit_generation.checked_add(1) != Some(described.selected_audit_generation)
            || cut.database_oid != described.database_oid
            || cut.schema_profile_digest.to_hex() != described.schema_profile_sha256
            || cut.state_profile_digest.to_hex() != described.state_profile_sha256
            || cut.metadata.commitment.to_hex() != described.addressed_metadata_tree_sha256
            || cut.metadata.entries != described.addressed_metadata_members
            || cut.inventory.root().to_hex() != described.addressed_inventory_root_sha256
            || cut.current.commitment.to_hex() != described.addressed_current_tree_sha256
            || cut.current.entries != described.current_members
            || cut.history.commitment.to_hex() != described.addressed_history_tree_sha256
            || cut.history.entries != described.history_members
            || cut.log_digest.to_hex() != described.log_sha256
        {
            return Err(DurableError::Conflict(
                "persisted addressed descriptor differs from its navigation anchor",
            ));
        }
        let selected = VerifiedAddressedGeneration {
            audited_root: store.hold_audit_root()?,
            store: store.clone(),
            cut,
            digest,
            state_digest,
            selected_audit_generation: described.selected_audit_generation,
            log_frontier: None,
            limits,
        };
        let generation =
            crate::source_current_cut::ManagedCurrentSourceGeneration::from_verified_addressed(
                store, cohort, selected,
            );
        let projection = parse_digest(&described.inventory_projection_sha256)?;
        if Self::managed_model_generation_description_v2(&generation, projection)? != *described {
            return Err(DurableError::Conflict(
                "persisted addressed navigation anchor differs from its proof",
            ));
        }
        active(deadline, cancelled)?;
        Ok(generation)
    }

    /// Reuse an already assessed current V2 generation for explicit export.
    /// This reads the authenticated descriptor under the same live fences as
    /// persisted proof reopening; it neither reassesses source bodies nor
    /// selects a new generation. None requests an explicit cold assessment
    /// for a stale selection or restored database; corrupt bindings remain errors.
    pub(crate) fn try_reopen_current_addressed_source_selection(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        initial_revision: SourceRevision,
        initial_membership: SourceMembershipV1,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<Option<crate::source_current_cut::ManagedCurrentSourceGeneration>> {
        active(deadline, cancelled)?;
        if domain.as_bytes() != store.custody_domain() {
            return Err(DurableError::Conflict(
                "addressed source store/domain differs",
            ));
        }
        let audited_root = store.hold_audit_root()?;
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .start()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        let audit = tx.query_opt(
            "SELECT generation,maintenance_state FROM cmd2_audit_fence WHERE domain=$1 FOR SHARE",
            &[&domain],
        )?.ok_or(DurableError::Corrupt("domain audit fence absent"))?;
        if audit.get::<_, String>(1) != "normal" {
            return Err(DurableError::Refused("physical maintenance in progress"));
        }
        let audit_generation = as_u64(audit.get(0))?;
        let row = tx
            .query_opt(
                "SELECT * FROM cmd2_domain WHERE domain=$1 FOR SHARE",
                &[&domain],
            )?
            .ok_or(DurableError::Corrupt("managed source domain absent"))?;
        let head = as_u64(row.get("head_seq"))?;
        let cohort = ManagedSourceCohort {
            domain: domain.into(),
            store_id: store.store_id(),
            initial_revision,
            initial_membership,
            epoch: as_u64(
                row.get::<_, Option<i64>>("source_epoch")
                    .ok_or(DurableError::Corrupt("source epoch absent"))?,
            )?,
            definition: parse_hex(
                row.get::<_, Option<String>>("source_definition_digest")
                    .ok_or(DurableError::Corrupt("source definition absent"))?,
            )?,
            generation: head,
        };
        cohort_matches(&row, &cohort, true)?;
        if !row.get::<_, bool>("rights_allowed") {
            return Err(DurableError::Refused("addressed source rights withdrawn"));
        }
        if row.get::<_, Option<i64>>("source_generation") != Some(as_i64(head)?)
            || as_u64(row.get("published_seq"))? != head
            || row.get::<_, Option<i64>>("complete_cut_generation")
                != Some(as_i64(audit_generation)?)
        {
            tx.commit()?;
            return Ok(None);
        }
        let digest = parse_hex(
            row.get::<_, Option<String>>("selected_generation_digest")
                .ok_or(DurableError::Conflict("addressed source selection absent"))?,
        )?;
        let raw = store.read_authenticated_object_v1(
            OBJECT_KIND,
            digest,
            MAX_DESCRIPTOR,
            deadline,
            cancelled,
        )?;
        let cut = AddressedCutV2::decode(&raw)?;
        cut.require_store(store)?;
        let state_digest = cut.state_digest()?;
        if cut.domain != domain
            || cut.through_seq != head
            || cut.audit_generation.checked_add(1) != Some(audit_generation)
            || row.get::<_, Option<String>>("schema_profile_digest")
                != Some(cut.schema_profile_digest.to_hex())
            || row.get::<_, Option<String>>("complete_cut_digest") != Some(state_digest.to_hex())
            || row.get::<_, Option<String>>("source_projection_digest")
                != Some(cut.inventory.root().to_hex())
        {
            return Err(DurableError::Conflict(
                "addressed descriptor differs from current selection",
            ));
        }
        if cut.database_oid != database_oid(&mut tx)? {
            tx.commit()?;
            return Ok(None);
        }
        let selected = VerifiedAddressedGeneration {
            audited_root,
            store: store.clone(),
            cut,
            digest,
            state_digest,
            selected_audit_generation: audit_generation,
            log_frontier: None,
            limits,
        };
        active(deadline, cancelled)?;
        tx.commit()?;
        Ok(Some(
            crate::source_current_cut::ManagedCurrentSourceGeneration::from_verified_addressed(
                store, cohort, selected,
            ),
        ))
    }

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
        let mut log_frontier = parent
            .log_frontier
            .as_ref()
            .ok_or(DurableError::Refused(
                "addressed successor requires a live verified log frontier",
            ))?
            .0
            .clone();
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
        cut.inventory.select_agent(
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
