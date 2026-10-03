//! CMD-owned immutable V2 source roots and their atomic-selection descriptor.
//!
//! These roots are derived from a completed native candidate. Their logical
//! membership and physical tree commitments remain separate from the V1
//! manifest digest and from NativeAdmissionComplete.
use std::io;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::Instant;
use tos_foundation::{Digest256, RelativePath, SourceRevision};
use tos_segment_store::{
    AuthenticatedTreeDescriptorV2, AuthenticatedTreeEntryV1, AuthenticatedTreeIoLedgerV1,
    AuthenticatedTreeLimitsV1, AuthenticatedTreeWorkV1, SegmentLimits, SegmentStore,
};
use tos_source_store::SourceMembershipV1;

const ROOTSET_SCHEMA: &str = "tos-native-source-rootset-v2";
const ROOTSET_MAX_BYTES: usize = 65_536;
const TREE_DESCRIPTOR_MAX_BYTES: usize = 12_288;
pub(crate) const MEMBERS_KIND: &[u8] = b"source-members-v2";
pub(crate) const IDENTITIES_KIND: &[u8] = b"source-identities-v2";
pub(crate) const DEPENDENCIES_KIND: &[u8] = b"source-dependencies-v2";
pub(crate) const RETIREMENTS_KIND: &[u8] = b"source-retirements-v2";
pub(crate) const HISTORY_KIND: &[u8] = b"source-history-v2";
pub(crate) const SOURCE_ADMISSION_V2_DOMAIN: &[u8] = b"tos-native-admission-source-v2";

fn validate_tree_binding(
    tree: &AuthenticatedTreeDescriptorV2,
    store_id: [u8; 16],
    domain_digest: Digest256,
    kind: &[u8],
    entries: u64,
) -> io::Result<()> {
    if tree.store_id != store_id
        || tree.domain_digest != domain_digest
        || tree.kind.as_slice() != kind
        || tree.entries != entries
    {
        return Err(invalid("source root tree binding differs"));
    }
    // Reuse the physical owner's descriptor shape and commitment checks.
    let raw = tree_bytes(tree)?;
    if AuthenticatedTreeDescriptorV2::decode(&raw, TREE_DESCRIPTOR_MAX_BYTES)
        .map_err(|_| invalid("source root tree commitment is invalid"))?
        != *tree
    {
        return Err(invalid("source root tree roundtrip differs"));
    }
    Ok(())
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn digest(value: &serde_json::Value) -> io::Result<Digest256> {
    let text = value
        .as_str()
        .ok_or_else(|| invalid("source rootset digest field is not text"))?;
    Digest256::from_hex(text).map_err(|_| invalid("source rootset digest encoding differs"))
}

fn optional_revision(value: &serde_json::Value) -> io::Result<Option<SourceRevision>> {
    if value.is_null() {
        Ok(None)
    } else {
        digest(value).map(|revision| Some(SourceRevision(revision)))
    }
}

fn number(value: &serde_json::Value) -> io::Result<u64> {
    value
        .as_u64()
        .ok_or_else(|| invalid("source rootset count is not unsigned"))
}

fn tree_bytes(tree: &AuthenticatedTreeDescriptorV2) -> io::Result<Vec<u8>> {
    tree.encode(TREE_DESCRIPTOR_MAX_BYTES)
        .map_err(|_| invalid("source rootset tree descriptor exceeds profile"))
}

fn tree(value: &serde_json::Value) -> io::Result<AuthenticatedTreeDescriptorV2> {
    let bytes: Vec<u8> = serde_json::from_value(value.clone())
        .map_err(|_| invalid("source rootset tree descriptor is not bytes"))?;
    AuthenticatedTreeDescriptorV2::decode(&bytes, TREE_DESCRIPTOR_MAX_BYTES)
        .map_err(|_| invalid("source rootset tree descriptor is invalid"))
}

/// One revision's current persistent roots. V1 membership remains its exact
/// historical digest; V2 tree commitments are separately named descriptors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceRevisionRootsV2 {
    pub revision: SourceRevision,
    pub base_revision: Option<SourceRevision>,
    pub validator_sha256: Digest256,
    pub manifest_sha256: Digest256,
    pub membership_v1: SourceMembershipV1,
    pub source_bytes: u64,
    pub member_count: u64,
    pub identity_count: u64,
    pub dependency_source_count: u64,
    pub dependency_count: u64,
    pub retirement_count: u64,
    pub members: AuthenticatedTreeDescriptorV2,
    pub identities: AuthenticatedTreeDescriptorV2,
    pub dependencies: AuthenticatedTreeDescriptorV2,
    pub retirements: AuthenticatedTreeDescriptorV2,
}

impl SourceRevisionRootsV2 {
    pub(crate) fn validate_store_binding(
        &self,
        store_id: [u8; 16],
        domain_digest: Digest256,
    ) -> io::Result<()> {
        if self.member_count != self.membership_v1.count
            || self.base_revision == Some(self.revision)
            || self.dependency_source_count > self.dependency_count
            || (self.dependency_source_count == 0) != (self.dependency_count == 0)
        {
            return Err(invalid("source revision root logical counts differ"));
        }
        for (root, kind, count) in [
            (&self.members, MEMBERS_KIND, self.member_count),
            (&self.identities, IDENTITIES_KIND, self.identity_count),
            (&self.dependencies, DEPENDENCIES_KIND, self.dependency_count),
            (&self.retirements, RETIREMENTS_KIND, self.retirement_count),
        ] {
            validate_tree_binding(root, store_id, domain_digest, kind, count)?;
        }
        Ok(())
    }

    /// Canonical tuple stored as an authenticated history-tree value. The
    /// caller still owns the shared state allowance for these bounded bytes.
    pub(crate) fn encode(&self) -> io::Result<Vec<u8>> {
        let raw = serde_json::to_vec(&self.wire_value()?)
            .map_err(|_| invalid("source revision root serialization failed"))?;
        if raw.is_empty() || raw.len() > ROOTSET_MAX_BYTES {
            return Err(invalid("source revision root byte profile exceeded"));
        }
        Ok(raw)
    }

    pub(crate) fn decode(raw: &[u8]) -> io::Result<Self> {
        if raw.is_empty() || raw.len() > ROOTSET_MAX_BYTES {
            return Err(invalid("source revision root byte profile exceeded"));
        }
        let value = serde_json::from_slice(raw)
            .map_err(|_| invalid("source revision root JSON is invalid"))?;
        let result = Self::from_wire(&value)?;
        if result.encode()?.as_slice() != raw {
            return Err(invalid("source revision root encoding is not canonical"));
        }
        Ok(result)
    }

    fn wire_value(&self) -> io::Result<serde_json::Value> {
        self.validate_store_binding(self.members.store_id, self.members.domain_digest)?;
        Ok(serde_json::json!([
            "tos-native-source-revision-roots-v2",
            self.revision.0.to_hex(),
            self.base_revision.map(|revision| revision.0.to_hex()),
            self.validator_sha256.to_hex(),
            self.manifest_sha256.to_hex(),
            self.membership_v1.count,
            self.membership_v1.digest.to_hex(),
            self.source_bytes,
            self.member_count,
            self.identity_count,
            self.dependency_source_count,
            self.dependency_count,
            self.retirement_count,
            tree_bytes(&self.members)?,
            tree_bytes(&self.identities)?,
            tree_bytes(&self.dependencies)?,
            tree_bytes(&self.retirements)?
        ]))
    }

    fn from_wire(value: &serde_json::Value) -> io::Result<Self> {
        let fields = value
            .as_array()
            .filter(|fields| fields.len() == 17)
            .ok_or_else(|| invalid("source revision root tuple shape differs"))?;
        if fields[0].as_str() != Some("tos-native-source-revision-roots-v2") {
            return Err(invalid("source revision root version differs"));
        }
        let result = Self {
            revision: SourceRevision(digest(&fields[1])?),
            base_revision: optional_revision(&fields[2])?,
            validator_sha256: digest(&fields[3])?,
            manifest_sha256: digest(&fields[4])?,
            membership_v1: SourceMembershipV1 {
                count: number(&fields[5])?,
                digest: digest(&fields[6])?,
            },
            source_bytes: number(&fields[7])?,
            member_count: number(&fields[8])?,
            identity_count: number(&fields[9])?,
            dependency_source_count: number(&fields[10])?,
            dependency_count: number(&fields[11])?,
            retirement_count: number(&fields[12])?,
            members: tree(&fields[13])?,
            identities: tree(&fields[14])?,
            dependencies: tree(&fields[15])?,
            retirements: tree(&fields[16])?,
        };
        if result.wire_value()? != *value {
            return Err(invalid("source revision root encoding is not canonical"));
        }
        Ok(result)
    }
}

/// The one immutable selection object named by current.json. It binds all
/// current roots and the append-history root under a single CAS selector.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceRootSetV2 {
    pub current: SourceRevisionRootsV2,
    pub history: AuthenticatedTreeDescriptorV2,
}

impl SourceRootSetV2 {
    pub(crate) fn validate_store_binding(
        &self,
        store_id: [u8; 16],
        domain_digest: Digest256,
    ) -> io::Result<()> {
        self.current
            .validate_store_binding(store_id, domain_digest)?;
        if self.history.entries == 0 {
            return Err(invalid("source history root has no current revision"));
        }
        validate_tree_binding(
            &self.history,
            store_id,
            domain_digest,
            HISTORY_KIND,
            self.history.entries,
        )
    }

    /// Apply only to a row returned by the authenticated history reader. This
    /// comparison binds the selected current tuple; it does not prove that a
    /// caller-supplied byte slice occurs in that tree.
    pub(crate) fn verify_current_history_row(&self, key: &[u8], raw: &[u8]) -> io::Result<()> {
        if key != self.current.revision.0.as_bytes()
            || SourceRevisionRootsV2::decode(raw)? != self.current
        {
            return Err(invalid("source history current revision binding differs"));
        }
        Ok(())
    }

    pub(crate) fn encode(&self) -> io::Result<Vec<u8>> {
        self.validate_store_binding(
            self.current.members.store_id,
            self.current.members.domain_digest,
        )?;
        let value = serde_json::json!([
            ROOTSET_SCHEMA,
            self.current.wire_value()?,
            tree_bytes(&self.history)?
        ]);
        let raw = serde_json::to_vec(&value)
            .map_err(|_| invalid("source rootset serialization failed"))?;
        if raw.is_empty() || raw.len() > ROOTSET_MAX_BYTES {
            return Err(invalid("source rootset byte profile exceeded"));
        }
        Ok(raw)
    }

    pub(crate) fn decode(raw: &[u8]) -> io::Result<Self> {
        if raw.is_empty() || raw.len() > ROOTSET_MAX_BYTES {
            return Err(invalid("source rootset byte profile exceeded"));
        }
        let value: serde_json::Value =
            serde_json::from_slice(raw).map_err(|_| invalid("source rootset JSON is invalid"))?;
        let fields = value
            .as_array()
            .filter(|fields| fields.len() == 3)
            .ok_or_else(|| invalid("source rootset tuple shape differs"))?;
        if fields[0].as_str() != Some(ROOTSET_SCHEMA) {
            return Err(invalid("source rootset version differs"));
        }
        let result = Self {
            current: SourceRevisionRootsV2::from_wire(&fields[1])?,
            history: tree(&fields[2])?,
        };
        if result.encode()?.as_slice() != raw {
            return Err(invalid("source rootset encoding is not canonical"));
        }
        Ok(result)
    }

    pub(crate) fn digest(&self) -> io::Result<Digest256> {
        self.encode().map(|raw| Digest256::of_bytes(&raw))
    }
}

/// IO and incremental persistent-allocation adapter for one completed native
/// V2 writer. The complete reservation is selected before this object can
/// touch the physical store; each new pack is additionally precharged by the
/// SegmentStore install path before it is staged.
pub(crate) struct NativeV2TreeIo {
    io: tos_source_store::PinnedSqliteIoBudget,
    custody: Arc<tos_source_store::PinnedSqliteSpaceReservation>,
    max_allocated_bytes: u64,
    allocation_unit_bytes: u64,
    reserved: AtomicU64,
    actual: AtomicU64,
}

impl NativeV2TreeIo {
    pub(crate) fn new(
        io: tos_source_store::PinnedSqliteIoBudget,
        custody: Arc<tos_source_store::PinnedSqliteSpaceReservation>,
        max_allocated_bytes: u64,
        allocation_unit_bytes: u64,
    ) -> io::Result<Arc<Self>> {
        if max_allocated_bytes == 0
            || max_allocated_bytes == u64::MAX
            || allocation_unit_bytes == 0
            || allocation_unit_bytes == u64::MAX
        {
            return Err(invalid("V2 allocation accountant profile is invalid"));
        }
        Ok(Arc::new(Self {
            io,
            custody,
            max_allocated_bytes,
            allocation_unit_bytes,
            reserved: AtomicU64::new(0),
            actual: AtomicU64::new(0),
        }))
    }

    pub(crate) fn from_budget(
        budget: &super::source_foundation_admission::NativeSegmentV2Budget,
    ) -> Arc<Self> {
        Arc::clone(&budget.allocation_accountant)
    }

    pub(crate) fn reserve_file_allocation(&self, bytes: u64) -> io::Result<u64> {
        let upper = allocation_upper_bound(bytes, self.allocation_unit_bytes)?;
        if !self.reserve_allocated_bytes(upper) {
            return Err(invalid("V2 persistent allocation precharge refused"));
        }
        Ok(upper)
    }

    pub(crate) fn reconcile_file_allocation(&self, reserved: u64, actual: u64) -> io::Result<()> {
        if !self.reconcile_allocated_bytes(reserved, actual) {
            return Err(invalid("V2 persistent allocation reconciliation refused"));
        }
        Ok(())
    }

    pub(crate) fn release_file_allocation(&self, reserved: u64) -> io::Result<()> {
        let result = self
            .reserved
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current.checked_sub(reserved)
            });
        result
            .map(|_| ())
            .map_err(|_| invalid("V2 persistent allocation precharge regressed"))
    }

    pub(crate) fn selected_allocation_unit_bytes(&self) -> u64 {
        self.allocation_unit_bytes
    }

    pub(crate) fn actual_allocated_bytes(&self) -> u64 {
        self.actual.load(Ordering::Acquire)
    }

    pub(crate) fn io_budget(&self) -> &tos_source_store::PinnedSqliteIoBudget {
        &self.io
    }

    pub(crate) fn custody_reservation(
        &self,
    ) -> Arc<tos_source_store::PinnedSqliteSpaceReservation> {
        Arc::clone(&self.custody)
    }
}

impl AuthenticatedTreeIoLedgerV1 for NativeV2TreeIo {
    fn charge_read(&self, bytes: u64) -> bool {
        self.io.charge_read(bytes).is_ok()
    }
    fn record_read_returned(&self, bytes: u64) -> bool {
        self.io.record_read_returned(bytes).is_ok()
    }
    fn charge_write(&self, bytes: u64) -> bool {
        self.io.charge_write(bytes).is_ok()
    }
    fn record_write_returned(&self, bytes: u64) -> bool {
        self.io.record_write_returned(bytes).is_ok()
    }
    fn reserve_allocated_bytes(&self, bytes: u64) -> bool {
        let Ok(next) = self
            .reserved
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current
                    .checked_add(bytes)
                    .filter(|sum| *sum <= self.max_allocated_bytes)
            })
        else {
            return false;
        };
        let _ = next;
        true
    }
    fn reconcile_allocated_bytes(&self, reserved: u64, actual: u64) -> bool {
        let Ok(previous) =
            self.actual
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                    current.checked_add(actual)
                })
        else {
            return false;
        };
        let next = previous + actual;
        let within_file_precharge = actual <= reserved;
        let within_total_cap = next <= self.max_allocated_bytes;
        self.custody.update_actual_allocated(next).is_ok()
            && within_file_precharge
            && within_total_cap
    }
    fn allocation_unit_bytes(&self) -> u64 {
        self.allocation_unit_bytes
    }
}

fn allocation_upper_bound(bytes: u64, unit: u64) -> io::Result<u64> {
    if unit == 0 || unit == u64::MAX {
        return Err(invalid("V2 allocation quantum is invalid"));
    }
    bytes
        .checked_add(unit - 1)
        .and_then(|n| n.checked_div(unit))
        .and_then(|n| n.checked_mul(unit))
        .and_then(|n| n.checked_add(unit))
        .ok_or_else(|| invalid("V2 allocation precharge overflow"))
}

pub(crate) struct BuiltInitialRootSetV2 {
    pub(crate) roots: SourceRootSetV2,
    pub(crate) bytes: Vec<u8>,
    pub(crate) sha256: Digest256,
    pub(crate) tree_io: Arc<NativeV2TreeIo>,
    pub(crate) segment_store: SegmentStore,
    pub(crate) work: AuthenticatedTreeWorkV1,
}

/// The first CMD writer uses the real validated candidate and maintained
/// native index cursors. It is an initial import only; warm successors must
/// use COW deltas and retained history roots.
pub(crate) fn build_initial_rootset_v2(
    store: &super::source_admission_store::AdmissionStore,
    candidate: &super::source_admission_spooled_candidate::SpoolCandidate<'_>,
    index: &super::source_admission_spooled_index::IndexView<'_>,
    revision: SourceRevision,
    manifest_sha256: Digest256,
    deadline: Instant,
    cancelled: &std::sync::atomic::AtomicBool,
) -> io::Result<BuiltInitialRootSetV2> {
    if candidate.base_revision().is_some() {
        return Err(invalid("V2 initial root builder requires an empty base"));
    }
    if store.has_v2_segments()? {
        return Err(invalid(
            "initial V2 writer refuses an existing or interrupted segment namespace",
        ));
    }
    let profile = index
        .segment_v2_budget()
        .ok_or_else(|| invalid("V2 root builder lacks the native completion profile"))?;
    let rootset_state_bound = ROOTSET_MAX_BYTES
        .checked_mul(4)
        .ok_or_else(|| invalid("V2 rootset state bound overflow"))?;
    if rootset_state_bound > profile.max_working_state_bytes {
        return Err(invalid("V2 rootset state preflight exceeds profile"));
    }
    index.verify_candidate()?;
    let tree_io = NativeV2TreeIo::from_budget(profile);
    if !store.has_v2_allocation_accountant(&tree_io) {
        return Err(invalid(
            "V2 writer and object-ingest allocation account are not shared",
        ));
    }
    store.retain_v2_store_custody(tree_io.custody_reservation());
    let segment_bytes = profile.max_allocated_bytes;
    if segment_bytes < 65_536 {
        return Err(invalid(
            "V2 persistent store profile is below metadata floor",
        ));
    }
    let segment_limits = SegmentLimits {
        max_segment_bytes: segment_bytes,
        max_frame_bytes: segment_bytes.min(4 * 1024 * 1024).max(1),
        max_frames: u32::try_from(profile.tree_limits.max_nodes.min(u32::MAX as u64))
            .map_err(|_| invalid("V2 segment frame limit exceeds range"))?
            .max(1),
        max_journal_bytes: profile
            .max_working_state_bytes
            .min(4 * 1024 * 1024)
            .max(128),
    };
    let tree_ledger: Arc<dyn AuthenticatedTreeIoLedgerV1> = tree_io.clone();
    let segment = store.segment_store_v2_with_io(
        SOURCE_ADMISSION_V2_DOMAIN,
        segment_limits,
        tree_ledger,
        deadline,
        cancelled,
    )?;
    if segment.custody_domain() != SOURCE_ADMISSION_V2_DOMAIN {
        return Err(invalid("V2 segment store domain differs"));
    }
    let base_limits = profile.tree_limits;
    let mut used = AuthenticatedTreeWorkV1::default();

    let member_rows = {
        let mut after: Option<RelativePath> = None;
        let candidate = candidate;
        std::iter::from_fn(move || {
            match candidate
                .member_after_bounded(after.as_ref(), profile.max_working_state_bytes / 8)
            {
                Ok(Some(member)) => {
                    after = Some(member.path.clone());
                    let mut value = Vec::with_capacity(44);
                    value.extend_from_slice(member.sha256.as_bytes());
                    value.extend_from_slice(&member.size_bytes.to_be_bytes());
                    value.extend_from_slice(&member.mode.to_le_bytes());
                    Some(Ok(AuthenticatedTreeEntryV1 {
                        key: member.path.as_str().as_bytes().to_vec(),
                        value,
                    }))
                }
                Ok(None) => None,
                Err(error) => Some(Err(tree_io_error(error))),
            }
        })
    };
    let (members, member_work) = segment
        .build_authenticated_tree_v2_with_work_and_io(
            MEMBERS_KIND,
            member_rows,
            remaining_tree_limits(base_limits, used)?,
            Some(tree_io.clone()),
            deadline,
            cancelled,
        )
        .map_err(tree_io_error)?;
    add_tree_work(&mut used, member_work, base_limits)?;

    let identity_rows = {
        let mut after: Option<String> = None;
        let index = index;
        std::iter::from_fn(move || match index.identities_after(after.as_deref()) {
            Ok(Some((id, path))) => {
                after = Some(id.clone());
                Some(Ok(AuthenticatedTreeEntryV1 {
                    key: id.into_bytes(),
                    value: path.as_str().as_bytes().to_vec(),
                }))
            }
            Ok(None) => None,
            Err(error) => Some(Err(tree_io_error(error))),
        })
    };
    let (identities, identity_work) = segment
        .build_authenticated_tree_v2_with_work_and_io(
            IDENTITIES_KIND,
            identity_rows,
            remaining_tree_limits(base_limits, used)?,
            Some(tree_io.clone()),
            deadline,
            cancelled,
        )
        .map_err(tree_io_error)?;
    add_tree_work(&mut used, identity_work, base_limits)?;

    let dependency_rows = {
        use super::source_admission_index::NativeDependencyDirectionV1::Forward;
        let mut after: Option<(RelativePath, RelativePath)> = None;
        let index = index;
        std::iter::from_fn(move || {
            match index.dependency_pair_after(Forward, after.as_ref().map(|(s, t)| (s, t))) {
                Ok(Some((source, target))) => {
                    after = Some((source.clone(), target.clone()));
                    let pair = match length_prefixed_pair(&source, &target) {
                        Ok(pair) => pair,
                        Err(error) => return Some(Err(tree_io_error(error))),
                    };
                    let key_len = source
                        .as_str()
                        .len()
                        .checked_add(1)
                        .and_then(|n| n.checked_add(target.as_str().len()));
                    let Some(key_len) = key_len else {
                        return Some(Err(tree_error("V2 dependency key length overflow")));
                    };
                    if key_len > profile.tree_limits.max_key_bytes {
                        return Some(Err(tree_error("V2 dependency key exceeds profile")));
                    }
                    // RelativePath excludes NUL, so this delimiter produces
                    // a unique bytewise source/target ordering and permits
                    // source-prefix seeks in later warm delta readers.
                    let mut key = Vec::new();
                    if key.try_reserve_exact(key_len).is_err() {
                        return Some(Err(tree_error("V2 dependency key allocation failed")));
                    }
                    key.extend_from_slice(source.as_str().as_bytes());
                    key.push(0);
                    key.extend_from_slice(target.as_str().as_bytes());
                    Some(Ok(AuthenticatedTreeEntryV1 { key, value: pair }))
                }
                Ok(None) => None,
                Err(error) => Some(Err(tree_io_error(error))),
            }
        })
    };
    let (dependencies, dependency_work) = segment
        .build_authenticated_tree_v2_with_work_and_io(
            DEPENDENCIES_KIND,
            dependency_rows,
            remaining_tree_limits(base_limits, used)?,
            Some(tree_io.clone()),
            deadline,
            cancelled,
        )
        .map_err(tree_io_error)?;
    add_tree_work(&mut used, dependency_work, base_limits)?;

    let retirement_rows = {
        let mut ordinal = 0u64;
        let count = candidate.retirement_count();
        let candidate = candidate;
        std::iter::from_fn(move || {
            if ordinal >= count {
                return None;
            }
            let row = candidate.retirement_at_bounded(ordinal, profile.max_working_state_bytes / 8);
            ordinal += 1;
            match row {
                Ok(Some(row)) => Some(encode_retirement_entry(ordinal - 1, row)),
                Ok(None) => Some(Err(tree_error("V2 retirement ordinal ended early"))),
                Err(error) => Some(Err(tree_io_error(error))),
            }
        })
    };
    let (retirements, retirement_work) = segment
        .build_authenticated_tree_v2_with_work_and_io(
            RETIREMENTS_KIND,
            retirement_rows,
            remaining_tree_limits(base_limits, used)?,
            Some(tree_io.clone()),
            deadline,
            cancelled,
        )
        .map_err(tree_io_error)?;
    add_tree_work(&mut used, retirement_work, base_limits)?;

    let fence = index.fence();
    let (member_count, source_bytes) = candidate.membership_counts();
    if member_count != fence.membership.count || source_bytes != fence.source_bytes {
        return Err(invalid(
            "V2 member binding differs from completed candidate",
        ));
    }
    let current = SourceRevisionRootsV2 {
        revision,
        base_revision: None,
        validator_sha256: fence.validator_sha256,
        manifest_sha256,
        membership_v1: fence.membership,
        source_bytes,
        member_count,
        identity_count: index.identity_count(),
        dependency_source_count: index.dependency_source_count(),
        dependency_count: index.dependency_count(),
        retirement_count: candidate.retirement_count(),
        members,
        identities,
        dependencies,
        retirements,
    };
    current.validate_store_binding(segment.store_id(), segment.domain_digest())?;
    let current_row = current.encode()?;
    let history_rows = [Ok(AuthenticatedTreeEntryV1 {
        key: revision.0.as_bytes().to_vec(),
        value: current_row,
    })];
    let (history, history_work) = segment
        .build_authenticated_tree_v2_with_work_and_io(
            HISTORY_KIND,
            history_rows,
            remaining_tree_limits(base_limits, used)?,
            Some(tree_io.clone()),
            deadline,
            cancelled,
        )
        .map_err(tree_io_error)?;
    add_tree_work(&mut used, history_work, base_limits)?;
    let roots = SourceRootSetV2 { current, history };
    roots.validate_store_binding(segment.store_id(), segment.domain_digest())?;
    let (current_row, history_read_work) = segment
        .lookup_authenticated_tree_v2_with_work_and_io(
            &roots.history,
            revision.0.as_bytes(),
            remaining_tree_limits(base_limits, used)?,
            Some(tree_io.clone()),
            deadline,
            cancelled,
        )
        .map_err(tree_io_error)?;
    add_tree_work(&mut used, history_read_work, base_limits)?;
    roots.verify_current_history_row(
        revision.0.as_bytes(),
        current_row
            .as_deref()
            .ok_or_else(|| invalid("V2 current history row is absent"))?,
    )?;
    index.verify_candidate()?;
    let bytes = roots.encode()?;
    let simultaneous_rootset_state = bytes
        .len()
        .checked_mul(4)
        .ok_or_else(|| invalid("V2 rootset state bound overflow"))?;
    if simultaneous_rootset_state > profile.max_working_state_bytes {
        return Err(invalid("V2 rootset state profile exceeded"));
    }
    let sha256 = Digest256::of_bytes(&bytes);
    Ok(BuiltInitialRootSetV2 {
        roots,
        bytes,
        sha256,
        tree_io,
        segment_store: segment,
        work: used,
    })
}

fn remaining_tree_limits(
    base: AuthenticatedTreeLimitsV1,
    used: AuthenticatedTreeWorkV1,
) -> io::Result<AuthenticatedTreeLimitsV1> {
    let mut remaining = base;
    let used_nodes = used
        .read_nodes
        .checked_add(used.written_nodes)
        .ok_or_else(|| invalid("V2 cumulative tree node overflow"))?;
    let used_bytes = used
        .read_bytes
        .checked_add(used.written_bytes)
        .ok_or_else(|| invalid("V2 cumulative tree byte overflow"))?;
    remaining.max_nodes = base
        .max_nodes
        .checked_sub(used_nodes)
        .filter(|n| *n > 0)
        .ok_or_else(|| invalid("V2 cumulative tree node profile exceeded"))?;
    remaining.max_total_bytes = base
        .max_total_bytes
        .checked_sub(used_bytes)
        .filter(|n| *n > 0)
        .ok_or_else(|| invalid("V2 cumulative tree byte profile exceeded"))?;
    Ok(remaining)
}

fn add_tree_work(
    used: &mut AuthenticatedTreeWorkV1,
    next: AuthenticatedTreeWorkV1,
    limits: AuthenticatedTreeLimitsV1,
) -> io::Result<()> {
    used.read_nodes = used
        .read_nodes
        .checked_add(next.read_nodes)
        .ok_or_else(|| invalid("V2 cumulative tree node overflow"))?;
    used.written_nodes = used
        .written_nodes
        .checked_add(next.written_nodes)
        .ok_or_else(|| invalid("V2 cumulative tree node overflow"))?;
    used.read_bytes = used
        .read_bytes
        .checked_add(next.read_bytes)
        .ok_or_else(|| invalid("V2 cumulative tree byte overflow"))?;
    used.written_bytes = used
        .written_bytes
        .checked_add(next.written_bytes)
        .ok_or_else(|| invalid("V2 cumulative tree byte overflow"))?;
    used.allocated_bytes = used
        .allocated_bytes
        .checked_add(next.allocated_bytes)
        .ok_or_else(|| invalid("V2 cumulative allocation overflow"))?;
    if used
        .read_nodes
        .checked_add(used.written_nodes)
        .is_none_or(|n| n > limits.max_nodes)
        || used
            .read_bytes
            .checked_add(used.written_bytes)
            .is_none_or(|n| n > limits.max_total_bytes)
    {
        return Err(invalid("V2 cumulative tree work exceeded"));
    }
    Ok(())
}

fn length_prefixed_pair(source: &RelativePath, target: &RelativePath) -> io::Result<Vec<u8>> {
    let source_len = u32::try_from(source.as_str().len())
        .map_err(|_| invalid("V2 dependency source exceeds range"))?;
    let target_len = u32::try_from(target.as_str().len())
        .map_err(|_| invalid("V2 dependency target exceeds range"))?;
    let cap = 8usize
        .checked_add(source.as_str().len())
        .and_then(|n| n.checked_add(target.as_str().len()))
        .ok_or_else(|| invalid("V2 dependency pair size overflow"))?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(cap)
        .map_err(|_| invalid("V2 dependency pair allocation failed"))?;
    result.extend_from_slice(&source_len.to_be_bytes());
    result.extend_from_slice(source.as_str().as_bytes());
    result.extend_from_slice(&target_len.to_be_bytes());
    result.extend_from_slice(target.as_str().as_bytes());
    Ok(result)
}

fn encode_retirement_entry(
    ordinal: u64,
    row: tos_source_store::RetirementMetadata,
) -> tos_segment_store::Result<AuthenticatedTreeEntryV1> {
    let mut value = Vec::new();
    let path_len = u32::try_from(row.path.as_str().len())
        .map_err(|_| tree_error("V2 retirement path exceeds range"))?;
    let event_len = u32::try_from(row.event_ref.as_str().len())
        .map_err(|_| tree_error("V2 retirement event path exceeds range"))?;
    let capacity = 4usize
        .checked_add(row.path.as_str().len())
        .and_then(|n| n.checked_add(32 + 4))
        .and_then(|n| n.checked_add(row.event_ref.as_str().len()))
        .and_then(|n| n.checked_add(32 + 8))
        .ok_or_else(|| tree_error("V2 retirement tuple size overflow"))?;
    value
        .try_reserve_exact(capacity)
        .map_err(|_| tree_error("V2 retirement tuple allocation failed"))?;
    value.extend_from_slice(&path_len.to_be_bytes());
    value.extend_from_slice(row.path.as_str().as_bytes());
    value.extend_from_slice(row.sha256.as_bytes());
    value.extend_from_slice(&event_len.to_be_bytes());
    value.extend_from_slice(row.event_ref.as_str().as_bytes());
    value.extend_from_slice(row.event_sha256.as_bytes());
    value.extend_from_slice(&row.event_size_bytes.to_be_bytes());
    Ok(AuthenticatedTreeEntryV1 {
        key: ordinal.to_be_bytes().to_vec(),
        value,
    })
}

fn tree_error(message: &'static str) -> tos_segment_store::SegmentError {
    tos_segment_store::SegmentError::new(
        tos_segment_store::SegmentErrorCode::InvalidFormat,
        message,
    )
}

fn tree_io_error(error: io::Error) -> tos_segment_store::SegmentError {
    tos_segment_store::SegmentError::io("CMD source cursor failed while building V2 roots", error)
}
