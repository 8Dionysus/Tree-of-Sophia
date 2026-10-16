//! Mixed source inventory roots backed by STO authenticated trees.
//!
//! The projection tree commits every current ToS subject to the exact SHA-256
//! of its canonical inventory projection. A second bounded tree commits the
//! coherent source-profile union needed to rebuild the Agent dependency
//! template. The trees are mechanics custody only; they do not admit source.

use super::*;
use tos_segment_store::{
    AuthenticatedTreeDeltaV1, AuthenticatedTreeDescriptorV2, AuthenticatedTreeEntryV1,
    AuthenticatedTreeLimitsV1, AuthenticatedTreeWorkV1, SegmentError, SegmentErrorCode,
    SegmentStore,
};

const MIXED_PROJECTION_KIND: &[u8] = b"tos.cmd.managed-mixed-source-inventory-projection-v2";
const MIXED_PROFILE_KIND: &[u8] = b"tos.cmd.managed-mixed-source-profile-v2";
const MIXED_AGENT_PROFILE_KIND: &[u8] = b"tos.cmd.managed-mixed-agent-source-profile-v1";
const MIXED_INVENTORY_MAGIC: &[u8] = b"TOS-MI-V2\0";
const MIXED_AGENT_BASELINE: [&str; 2] = [
    "ToS/doctrine/semantic-interchange/entity-types.v1.json",
    "ToS/contracts/semantic-entity-type-registry.schema.json",
];
const MAX_INVENTORY_DESCRIPTOR_BYTES: usize = 65_536;
const MAX_PROJECTION_BYTES: u64 = 64 * 1024 * 1024;
// A successful Agent selection already has this bound on its selected source
// files. Enforcing the same ceiling here makes profile materialization
// independent of the number of cohort members.
const MAX_PROFILE_COUNT: u64 = cmd::SELECTED_SOURCE_MAX_FILES as u64;

fn tree_callback_state_bytes(limits: AuthenticatedTreeLimitsV1) -> DurableResult<usize> {
    usize::try_from(limits.max_total_bytes)
        .map_err(|_| DurableError::Refused("authenticated inventory tree state bound"))
}

/// Mixed source inventory custody. Its first two trees describe the complete
/// mixed projection/profile surface; the third tree contains only the common
/// Agent registry baseline and schemas contributed by Agent metadata/forms.
/// The Agent-only predecessor descriptor is never interpreted as a mixed inventory.
#[derive(Clone, Debug)]
pub(super) struct AddressedMixedInventoryV1 {
    projections: AuthenticatedTreeDescriptorV2,
    profiles: AuthenticatedTreeDescriptorV2,
    agent_profiles: AuthenticatedTreeDescriptorV2,
}

impl AddressedMixedInventoryV1 {
    pub(super) fn build_from_assessed_projections<I>(
        store: &SegmentStore,
        projections: I,
        expected_count: u64,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
        work: &mut AuthenticatedTreeWorkV1,
    ) -> DurableResult<Self>
    where
        I: IntoIterator<Item = DurableResult<(String, Vec<u8>)>>,
    {
        let mut owner_check = || true;
        Self::build_from_assessed_projections_with_callback(
            store,
            projections,
            expected_count,
            limits,
            deadline,
            cancelled,
            work,
            &mut owner_check,
        )
    }

    pub(super) fn build_from_assessed_projections_with_callback<I>(
        store: &SegmentStore,
        projections: I,
        expected_count: u64,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
        work: &mut AuthenticatedTreeWorkV1,
        owner_check: &mut dyn FnMut() -> bool,
    ) -> DurableResult<Self>
    where
        I: IntoIterator<Item = DurableResult<(String, Vec<u8>)>>,
    {
        active(deadline, cancelled)?;
        if !owner_check() {
            return Err(DurableError::Conflict(
                "streamed mixed inventory owner fence changed",
            ));
        }
        if expected_count > limits.max_rows {
            return Err(DurableError::Refused("streamed mixed inventory row bound"));
        }
        let mut input = projections.into_iter();
        let mut profiles = BTreeMap::<String, Digest256>::new();
        let mut agent_profiles = BTreeMap::<String, Digest256>::new();
        let mut after = None::<String>;
        let mut count = 0u64;
        let mut failure = None;
        let mut ended = false;
        let entries = std::iter::from_fn(|| {
            if ended {
                return None;
            }
            let next = match input.next() {
                Some(next) => next,
                None => {
                    ended = true;
                    return None;
                }
            };
            let entry = (|| -> DurableResult<AuthenticatedTreeEntryV1> {
                active(deadline, cancelled)?;
                let (path, raw) = next?;
                if !path.starts_with("ToS/")
                    || after.as_ref().is_some_and(|prior| path <= *prior)
                    || path.len() > limits.max_key_bytes
                {
                    return Err(DurableError::Corrupt("streamed mixed inventory path/order"));
                }
                let value = parse_mixed_projection(&path, &raw, None)?;
                collect_profiles(&value, &mut profiles)?;
                collect_mixed_agent_profiles(&path, &value, &mut agent_profiles)?;
                count = count
                    .checked_add(1)
                    .filter(|n| *n <= expected_count)
                    .ok_or(DurableError::Corrupt("streamed mixed inventory extra rows"))?;
                let digest = Digest256::of_bytes(&raw);
                let key = path.as_bytes().to_vec();
                after = Some(path);
                active(deadline, cancelled)?;
                Ok(AuthenticatedTreeEntryV1 {
                    key,
                    value: digest.as_bytes().to_vec(),
                })
            })();
            match entry {
                Ok(entry) => Some(Ok(entry)),
                Err(error) => {
                    ended = true;
                    failure = Some(error);
                    Some(Err(SegmentError::new(
                        SegmentErrorCode::CorruptBytes,
                        "streamed mixed inventory assessment cursor failed",
                    )))
                }
            }
        });
        let state_bytes = tree_callback_state_bytes(limits)?;
        let projection_result = store
            .build_authenticated_tree_v2_with_work_and_io_and_state_and_callback(
                MIXED_PROJECTION_KIND,
                entries,
                limits,
                None,
                state_bytes,
                0,
                deadline,
                cancelled,
                owner_check,
            );
        if let Some(error) = failure {
            return Err(error);
        }
        let (projection_tree, projection_work) = projection_result.map_err(tree_error)?;
        add_tree_work(work, projection_work)?;
        if !ended || count != expected_count || projection_tree.entries != count {
            return Err(DurableError::Corrupt(
                "streamed mixed inventory EOF/count differs",
            ));
        }
        if !owner_check() {
            return Err(DurableError::Conflict(
                "streamed mixed inventory owner fence changed",
            ));
        }
        let (profile_tree, profile_work) = store
            .build_authenticated_tree_v2_with_work_and_io_and_state_and_callback(
                MIXED_PROFILE_KIND,
                profiles.into_iter().map(|(path, digest)| {
                    Ok(AuthenticatedTreeEntryV1 {
                        key: path.into_bytes(),
                        value: digest.as_bytes().to_vec(),
                    })
                }),
                limits,
                None,
                state_bytes,
                0,
                deadline,
                cancelled,
                owner_check,
            )
            .map_err(tree_error)?;
        add_tree_work(work, profile_work)?;
        if !owner_check() {
            return Err(DurableError::Conflict(
                "streamed mixed inventory owner fence changed",
            ));
        }
        let (agent_profile_tree, agent_profile_work) = store
            .build_authenticated_tree_v2_with_work_and_io_and_state_and_callback(
                MIXED_AGENT_PROFILE_KIND,
                agent_profiles.into_iter().map(|(path, digest)| {
                    Ok(AuthenticatedTreeEntryV1 {
                        key: path.into_bytes(),
                        value: digest.as_bytes().to_vec(),
                    })
                }),
                limits,
                None,
                state_bytes,
                0,
                deadline,
                cancelled,
                owner_check,
            )
            .map_err(tree_error)?;
        add_tree_work(work, agent_profile_work)?;
        if !owner_check() {
            return Err(DurableError::Conflict(
                "streamed mixed inventory owner fence changed",
            ));
        }
        let result = Self {
            projections: projection_tree,
            profiles: profile_tree,
            agent_profiles: agent_profile_tree,
        };
        result.require_store(store)?;
        result.check_descriptor_set()?;
        active(deadline, cancelled)?;
        Ok(result)
    }

    pub(super) fn apply_changed_with_work(
        &self,
        store: &SegmentStore,
        changed: &ProjectionRows,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
        work: &mut AuthenticatedTreeWorkV1,
    ) -> DurableResult<Self> {
        self.require_store(store)?;
        if changed.is_empty() {
            return Ok(self.clone());
        }
        let changed_rows = u64::try_from(changed.len())
            .map_err(|_| DurableError::Refused("mixed inventory delta row count overflow"))?;
        // AuthenticatedTreeLimitsV1::max_rows bounds rows consumed by this
        // delta call, not the final tree's total entry count.
        if changed_rows > limits.max_rows {
            return Err(DurableError::Refused(
                "mixed inventory delta row limit exceeded",
            ));
        }
        let (projection_delta, profile_delta, agent_profile_delta, next_count) =
            prepare_inventory_projection_delta(
                store,
                &self.projections,
                changed,
                limits,
                deadline,
                cancelled,
                work,
                |subject, raw| {
                    let value = parse_mixed_projection(subject, raw, None)?;
                    let mut profiles = BTreeMap::new();
                    let mut agent_profiles = BTreeMap::new();
                    collect_profiles(&value, &mut profiles)?;
                    collect_mixed_agent_profiles(subject, &value, &mut agent_profiles)?;
                    Ok((profiles, agent_profiles))
                },
            )?;

        let profile_changes = checked_profile_changes(
            store,
            &self.profiles,
            profile_delta,
            limits,
            deadline,
            cancelled,
            work,
            "mixed source profile contributions disagree",
        )?;
        let agent_profile_changes = checked_profile_changes(
            store,
            &self.agent_profiles,
            agent_profile_delta,
            limits,
            deadline,
            cancelled,
            work,
            "mixed Agent profile contributions disagree",
        )?;
        let (projection_tree, projection_work) = store
            .apply_authenticated_tree_delta_v2_with_work(
                &self.projections,
                projection_delta.into_iter().map(Ok),
                limits,
                deadline,
                cancelled,
            )
            .map_err(tree_error)?;
        add_tree_work(work, projection_work)?;
        if projection_tree.entries != next_count {
            return Err(DurableError::Corrupt(
                "mixed inventory projection result count differs",
            ));
        }
        let profile_tree = apply_inventory_profile_changes(
            store,
            &self.profiles,
            profile_changes,
            limits,
            deadline,
            cancelled,
            work,
        )?;
        let agent_profile_tree = apply_inventory_profile_changes(
            store,
            &self.agent_profiles,
            agent_profile_changes,
            limits,
            deadline,
            cancelled,
            work,
        )?;
        let result = Self {
            projections: projection_tree,
            profiles: profile_tree,
            agent_profiles: agent_profile_tree,
        };
        result.require_store(store)?;
        result.check_descriptor_set()?;
        Ok(result)
    }

    pub(super) fn root(&self) -> Digest256 {
        let mut hash = Digest256Hasher::new();
        part(&mut hash, b"tos-managed-mixed-source-inventory-root-v1");
        part(&mut hash, &1u16.to_be_bytes());
        part(&mut hash, MIXED_PROJECTION_KIND);
        part(&mut hash, self.projections.commitment.as_bytes());
        part(&mut hash, &self.projections.entries.to_be_bytes());
        part(&mut hash, MIXED_PROFILE_KIND);
        part(&mut hash, self.profiles.commitment.as_bytes());
        part(&mut hash, &self.profiles.entries.to_be_bytes());
        part(&mut hash, MIXED_AGENT_PROFILE_KIND);
        part(&mut hash, self.agent_profiles.commitment.as_bytes());
        part(&mut hash, &self.agent_profiles.entries.to_be_bytes());
        hash.finalize()
    }


    pub(super) fn projection_digest_with_work(
        &self,
        store: &SegmentStore,
        path: &RelativePath,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
        work: &mut AuthenticatedTreeWorkV1,
    ) -> DurableResult<Option<Digest256>> {
        self.require_store(store)?;
        let (raw, lookup_work) = store
            .lookup_authenticated_tree_v2_with_work(
                &self.projections,
                path.as_str().as_bytes(),
                limits,
                deadline,
                cancelled,
            )
            .map_err(tree_error)?;
        add_tree_work(work, lookup_work)?;
        let Some(raw) = raw else {
            return Ok(None);
        };
        let digest: [u8; 32] = raw
            .try_into()
            .map_err(|_| DurableError::Corrupt("mixed selected projection digest length"))?;
        Ok(Some(Digest256::from_bytes(digest)))
    }

    pub(super) fn projection_entries(&self) -> u64 {
        self.projections.entries
    }

    pub(super) fn checked_projection(
        subject: &str,
        raw: &[u8],
        content: &str,
    ) -> DurableResult<tos_foundation::JsonValue> {
        parse_mixed_projection(subject, raw, Some(content))
    }

    pub(super) fn select_agent(
        &self,
        store: &SegmentStore,
        ctx: &CommandContext,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<ManagedAgentInventory> {
        self.require_store(store)?;
        let profiles = read_inventory_profile_tree(
            store,
            &self.agent_profiles,
            MAX_PROFILE_COUNT,
            limits,
            deadline,
            cancelled,
            "mixed Agent source profile",
        )?;
        let root = self.root();
        Ok(ManagedAgentInventory {
            commitment_version: 3,
            root,
            dependencies: mixed_agent_dependency_commitment_v1(
                ctx, &profiles, deadline, cancelled,
            )?,
        })
    }

    pub(super) fn verify_source_projections(
        &self,
        store: &SegmentStore,
        ctx: &CommandContext,
        projections: &ProjectionRows,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<()> {
        self.require_store(store)?;
        if self.projections.entries != projections.len() as u64 {
            return Err(DurableError::Corrupt(
                "mixed source projection coverage differs",
            ));
        }
        let mut expected_profiles = BTreeMap::new();
        let mut expected_agent_profiles = BTreeMap::new();
        for (path, raw) in projections {
            let value = parse_mixed_projection(path, raw, None)?;
            collect_profiles(&value, &mut expected_profiles)?;
            collect_mixed_agent_profiles(path, &value, &mut expected_agent_profiles)?;
        }
        verify_inventory_tree(
            store,
            &self.projections,
            projections,
            limits,
            deadline,
            cancelled,
            "mixed source projection",
        )?;
        verify_inventory_profile_tree(
            store,
            &self.profiles,
            &expected_profiles,
            limits,
            deadline,
            cancelled,
            "mixed source profile",
        )?;
        verify_inventory_profile_tree(
            store,
            &self.agent_profiles,
            &expected_agent_profiles,
            limits,
            deadline,
            cancelled,
            "mixed Agent source profile",
        )?;
        self.select_agent(store, ctx, limits, deadline, cancelled)?;
        Ok(())
    }

    pub(super) fn encode(&self) -> DurableResult<Vec<u8>> {
        self.check_descriptor_set()?;
        encode_inventory_descriptors(
            MIXED_INVENTORY_MAGIC,
            &[&self.projections, &self.profiles, &self.agent_profiles],
        )
    }

    pub(super) fn decode(raw: &[u8]) -> DurableResult<Self> {
        let mut descriptors = decode_inventory_descriptors(raw, MIXED_INVENTORY_MAGIC, 3)?;
        let result = Self {
            agent_profiles: descriptors
                .pop()
                .ok_or(DurableError::Corrupt("mixed inventory descriptor fields"))?,
            profiles: descriptors
                .pop()
                .ok_or(DurableError::Corrupt("mixed inventory descriptor fields"))?,
            projections: descriptors
                .pop()
                .ok_or(DurableError::Corrupt("mixed inventory descriptor fields"))?,
        };
        result.check_descriptor_set()?;
        if result.encode()? != raw {
            return Err(DurableError::Corrupt(
                "mixed inventory descriptor noncanonical encoding",
            ));
        }
        Ok(result)
    }

    pub(super) fn require_store(&self, store: &SegmentStore) -> DurableResult<()> {
        self.check_descriptor_set()?;
        for descriptor in [&self.projections, &self.profiles, &self.agent_profiles] {
            if descriptor.store_id != store.store_id()
                || descriptor.domain_digest != store.domain_digest()
            {
                return Err(DurableError::Conflict(
                    "mixed inventory tree custody identity differs",
                ));
            }
        }
        Ok(())
    }

    fn check_descriptor_set(&self) -> DurableResult<()> {
        if self.projections.kind != MIXED_PROJECTION_KIND
            || self.profiles.kind != MIXED_PROFILE_KIND
            || self.agent_profiles.kind != MIXED_AGENT_PROFILE_KIND
            || self.projections.store_id != self.profiles.store_id
            || self.projections.store_id != self.agent_profiles.store_id
            || self.projections.domain_digest != self.profiles.domain_digest
            || self.projections.domain_digest != self.agent_profiles.domain_digest
            || self.profiles.entries > MAX_PROFILE_COUNT
            || self.agent_profiles.entries > MAX_PROFILE_COUNT
            || self.projections.root.as_ref().map(|root| root.entries)
                != (self.projections.entries != 0).then_some(self.projections.entries)
            || self.profiles.root.as_ref().map(|root| root.entries)
                != (self.profiles.entries != 0).then_some(self.profiles.entries)
            || self.agent_profiles.root.as_ref().map(|root| root.entries)
                != (self.agent_profiles.entries != 0).then_some(self.agent_profiles.entries)
        {
            return Err(DurableError::Corrupt(
                "mixed inventory tree descriptors disagree",
            ));
        }
        Ok(())
    }
}

fn prepare_inventory_projection_delta<F>(
    store: &SegmentStore,
    current: &AuthenticatedTreeDescriptorV2,
    changed: &ProjectionRows,
    limits: AuthenticatedTreeLimitsV1,
    deadline: Instant,
    cancelled: &AtomicBool,
    work: &mut AuthenticatedTreeWorkV1,
    mut contributions: F,
) -> DurableResult<(
    Vec<AuthenticatedTreeDeltaV1>,
    BTreeMap<String, Digest256>,
    BTreeMap<String, Digest256>,
    u64,
)>
where
    F: FnMut(
        &str,
        &[u8],
    ) -> DurableResult<(BTreeMap<String, Digest256>, BTreeMap<String, Digest256>)>,
{
    if changed.len() > MAX_MEMBERS {
        return Err(DurableError::Refused(
            "inventory projection delta exceeds member bound",
        ));
    }
    let next_count =
        current
            .entries
            .checked_add(changed.len() as u64)
            .ok_or(DurableError::Refused(
                "inventory projection tree entry count overflow",
            ))?;
    let mut projection_delta = Vec::with_capacity(changed.len());
    let mut profiles = BTreeMap::<String, Digest256>::new();
    let mut agent_profiles = BTreeMap::<String, Digest256>::new();
    let mut total_key_bytes = 0usize;
    let mut total_projection_bytes = 0usize;
    for (subject, raw) in changed {
        active(deadline, cancelled)?;
        let path = RelativePath::parse(subject)
            .map_err(|_| DurableError::Corrupt("inventory delta projection path"))?;
        if !path.as_str().starts_with("ToS/") || raw.len() > 1_048_576 {
            return Err(DurableError::Refused(
                "inventory delta projection exceeds its member envelope",
            ));
        }
        total_key_bytes = total_key_bytes
            .checked_add(path.as_str().len())
            .filter(|n| *n <= MAX_TOTAL_MEMBERSHIP_KEY_BYTES)
            .ok_or(DurableError::Refused(
                "inventory delta keys exceed authenticated tree envelope",
            ))?;
        total_projection_bytes = total_projection_bytes
            .checked_add(raw.len())
            .filter(|n| *n <= MAX_PROJECTION_BYTES as usize)
            .ok_or(DurableError::Refused(
                "inventory delta projections exceed cold metadata envelope",
            ))?;
        if path.as_str().len() > limits.max_key_bytes {
            return Err(DurableError::Refused(
                "inventory delta key exceeds authenticated tree bound",
            ));
        }
        let (existing, lookup_work) = store
            .lookup_authenticated_tree_v2_with_work(
                current,
                path.as_str().as_bytes(),
                limits,
                deadline,
                cancelled,
            )
            .map_err(tree_error)?;
        add_tree_work(work, lookup_work)?;
        if existing.is_some() {
            return Err(DurableError::Conflict(
                "inventory delta rewrites an addressed projection",
            ));
        }
        let (member_profiles, member_agent_profiles) = contributions(subject, raw)?;
        for (path, digest) in member_profiles {
            merge_profile(&mut profiles, &path, digest)?;
        }
        for (path, digest) in member_agent_profiles {
            merge_profile(&mut agent_profiles, &path, digest)?;
        }
        projection_delta.push(AuthenticatedTreeDeltaV1 {
            key: path.as_str().as_bytes().to_vec(),
            value: Some(Digest256::of_bytes(raw).as_bytes().to_vec()),
        });
    }
    Ok((projection_delta, profiles, agent_profiles, next_count))
}

fn checked_profile_changes(
    store: &SegmentStore,
    descriptor: &AuthenticatedTreeDescriptorV2,
    additions: BTreeMap<String, Digest256>,
    limits: AuthenticatedTreeLimitsV1,
    deadline: Instant,
    cancelled: &AtomicBool,
    work: &mut AuthenticatedTreeWorkV1,
    conflict_message: &'static str,
) -> DurableResult<Vec<AuthenticatedTreeDeltaV1>> {
    let mut changes = Vec::new();
    for (path, digest) in additions {
        let (existing, lookup_work) = store
            .lookup_authenticated_tree_v2_with_work(
                descriptor,
                path.as_bytes(),
                limits,
                deadline,
                cancelled,
            )
            .map_err(tree_error)?;
        add_tree_work(work, lookup_work)?;
        if let Some(existing) = existing {
            if existing.as_slice() != digest.as_bytes() {
                return Err(DurableError::Corrupt(conflict_message));
            }
        } else {
            changes.push(AuthenticatedTreeDeltaV1 {
                key: path.into_bytes(),
                value: Some(digest.as_bytes().to_vec()),
            });
        }
    }
    if descriptor
        .entries
        .checked_add(changes.len() as u64)
        .is_none_or(|count| count > MAX_PROFILE_COUNT)
    {
        return Err(DurableError::Refused(
            "source profile union exceeds selected file bound",
        ));
    }
    Ok(changes)
}

fn apply_inventory_profile_changes(
    store: &SegmentStore,
    descriptor: &AuthenticatedTreeDescriptorV2,
    changes: Vec<AuthenticatedTreeDeltaV1>,
    limits: AuthenticatedTreeLimitsV1,
    deadline: Instant,
    cancelled: &AtomicBool,
    work: &mut AuthenticatedTreeWorkV1,
) -> DurableResult<AuthenticatedTreeDescriptorV2> {
    if changes.is_empty() {
        return Ok(descriptor.clone());
    }
    let (result, operation_work) = store
        .apply_authenticated_tree_delta_v2_with_work(
            descriptor,
            changes.into_iter().map(Ok),
            limits,
            deadline,
            cancelled,
        )
        .map_err(tree_error)?;
    add_tree_work(work, operation_work)?;
    Ok(result)
}

fn read_inventory_profile_tree(
    store: &SegmentStore,
    descriptor: &AuthenticatedTreeDescriptorV2,
    max_count: u64,
    limits: AuthenticatedTreeLimitsV1,
    deadline: Instant,
    cancelled: &AtomicBool,
    label: &'static str,
) -> DurableResult<BTreeMap<String, Digest256>> {
    if descriptor.entries > max_count {
        return Err(DurableError::Refused(
            "source profile union exceeds selected file bound",
        ));
    }
    let mut stream = store
        .stream_authenticated_tree_v2(descriptor, limits)
        .map_err(tree_error)?;
    let mut profiles = BTreeMap::<String, Digest256>::new();
    let mut previous = String::new();
    while let Some(entry) = stream.next_row(deadline, cancelled).map_err(tree_error)? {
        active(deadline, cancelled)?;
        let path = String::from_utf8(entry.key)
            .map_err(|_| DurableError::Corrupt("source profile tree key"))?;
        if path <= previous {
            return Err(DurableError::Corrupt(
                "source profile tree ordering differs",
            ));
        }
        RelativePath::parse(&path)
            .map_err(|_| DurableError::Corrupt("source profile tree path"))?;
        let bytes: [u8; 32] = entry
            .value
            .try_into()
            .map_err(|_| DurableError::Corrupt("source profile tree digest"))?;
        profiles.insert(path.clone(), Digest256::from_bytes(bytes));
        previous = path;
    }
    if profiles.len() as u64 != descriptor.entries || profiles.len() as u64 > max_count {
        return Err(DurableError::Corrupt(match label {
            "Agent source profile" => "Agent source profile tree coverage differs",
            "mixed Agent source profile" => "mixed Agent source profile tree coverage differs",
            "mixed source profile" => "mixed source profile tree coverage differs",
            _ => "source profile tree coverage differs",
        }));
    }
    Ok(profiles)
}

fn verify_inventory_profile_tree(
    store: &SegmentStore,
    descriptor: &AuthenticatedTreeDescriptorV2,
    expected: &BTreeMap<String, Digest256>,
    limits: AuthenticatedTreeLimitsV1,
    deadline: Instant,
    cancelled: &AtomicBool,
    _label: &'static str,
) -> DurableResult<()> {
    if descriptor.entries != expected.len() as u64 {
        return Err(DurableError::Corrupt(
            "addressed source profile coverage differs",
        ));
    }
    let mut rows = store
        .stream_authenticated_tree_v2(descriptor, limits)
        .map_err(tree_error)?;
    let mut expected = expected.iter();
    let mut previous = String::new();
    let mut count = 0u64;
    while let Some(entry) = rows.next_row(deadline, cancelled).map_err(tree_error)? {
        active(deadline, cancelled)?;
        let path = String::from_utf8(entry.key)
            .map_err(|_| DurableError::Corrupt("addressed source profile key"))?;
        if path <= previous {
            return Err(DurableError::Corrupt(
                "addressed source profile ordering differs",
            ));
        }
        let (expected_path, expected_digest) = expected.next().ok_or(DurableError::Corrupt(
            "addressed source profile has extra member",
        ))?;
        if path.as_str() != expected_path.as_str()
            || entry.value.as_slice() != &expected_digest.as_bytes()[..]
        {
            return Err(DurableError::Conflict(
                "addressed source profile differs from verified projections",
            ));
        }
        previous = path;
        count = count.checked_add(1).ok_or(DurableError::Corrupt(
            "addressed source profile count overflow",
        ))?;
    }
    if count != descriptor.entries || expected.next().is_some() {
        return Err(DurableError::Corrupt(
            "addressed source profile EOF differs",
        ));
    }
    Ok(())
}

fn verify_inventory_tree(
    store: &SegmentStore,
    descriptor: &AuthenticatedTreeDescriptorV2,
    projections: &ProjectionRows,
    limits: AuthenticatedTreeLimitsV1,
    deadline: Instant,
    cancelled: &AtomicBool,
    _label: &'static str,
) -> DurableResult<()> {
    if descriptor.entries != projections.len() as u64 {
        return Err(DurableError::Corrupt(
            "addressed source projection coverage differs",
        ));
    }
    let mut rows = store
        .stream_authenticated_tree_v2(descriptor, limits)
        .map_err(tree_error)?;
    let mut expected = projections.iter();
    let mut previous = String::new();
    let mut count = 0u64;
    while let Some(entry) = rows.next_row(deadline, cancelled).map_err(tree_error)? {
        active(deadline, cancelled)?;
        let path = String::from_utf8(entry.key)
            .map_err(|_| DurableError::Corrupt("addressed source projection key"))?;
        if path <= previous {
            return Err(DurableError::Corrupt(
                "addressed source projection ordering differs",
            ));
        }
        let (expected_path, raw) = expected.next().ok_or(DurableError::Corrupt(
            "addressed source projection has extra member",
        ))?;
        let digest = Digest256::of_bytes(raw);
        if path.as_str() != expected_path.as_str()
            || entry.value.as_slice() != &digest.as_bytes()[..]
        {
            return Err(DurableError::Conflict(
                "addressed source projection differs from verified source",
            ));
        }
        previous = path;
        count = count.checked_add(1).ok_or(DurableError::Corrupt(
            "addressed source projection count overflow",
        ))?;
    }
    if count != descriptor.entries || expected.next().is_some() {
        return Err(DurableError::Corrupt(
            "addressed source projection EOF differs",
        ));
    }
    Ok(())
}

fn collect_mixed_agent_profiles(
    subject: &str,
    value: &tos_foundation::JsonValue,
    agent_profiles: &mut BTreeMap<String, Digest256>,
) -> DurableResult<()> {
    for (path, digest) in collect_agent_profile_contributions(subject, value)? {
        merge_profile(agent_profiles, &path, digest)?;
    }
    Ok(())
}

/// Return the per-member contribution to the typed Agent profile tree. The
/// parent cohort owner uses this exact rule when recomputing retained hashes.
pub(super) fn collect_agent_profile_contributions(
    subject: &str,
    value: &tos_foundation::JsonValue,
) -> DurableResult<BTreeMap<String, Digest256>> {
    let profiles = collect_profile_map(value)?;
    let mut contribution = BTreeMap::new();
    for baseline in MIXED_AGENT_BASELINE {
        let digest = profiles
            .get(baseline)
            .copied()
            .ok_or(DurableError::Corrupt(
                "mixed Agent registry baseline absent",
            ))?;
        merge_profile(&mut contribution, baseline, digest)?;
    }
    let records = cmd::field(value, "records")
        .map_err(source_error)?
        .as_object()
        .ok_or(DurableError::Corrupt("mixed inventory records map"))?;
    let has_agent_record = records
        .iter()
        .find(|(kind, _)| kind.as_str() == Some("agent"))
        .and_then(|(_, records)| records.as_array())
        .is_some_and(|records| !records.is_empty());
    let is_agent_form = subject.ends_with("/agent.human-forms.json")
        && cmd::field(value, "form")
            .map_err(source_error)?
            .as_object()
            .is_some();
    if has_agent_record || is_agent_form {
        for (path, digest) in profiles {
            merge_profile(&mut contribution, &path, digest)?;
        }
    }
    Ok(contribution)
}

/// Bind only the inputs initial Agent creation reads: raw configuration and the
/// canonical creation template, including the selected Agent profile/schema
/// map, contracts and implementation inputs. Existing Agent records, events,
/// anchors and forms are not creation references/evidence. The complete mixed
/// root and current head/epoch/rights/audit fences are checked separately, and
/// durable uniqueness/absence predicates still protect global IDs and homes.
/// Bind the fixed registry/schema baseline from the selected context directly
/// so it remains present even when the current mixed cut contains no Agents.
pub(super) fn mixed_agent_dependency_commitment_v1(
    ctx: &CommandContext,
    profiles: &BTreeMap<String, Digest256>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<String> {
    let mut selected_profiles = profiles.clone();
    for baseline in MIXED_AGENT_BASELINE {
        let path = RelativePath::parse(baseline)
            .map_err(|_| DurableError::Corrupt("mixed Agent baseline path"))?;
        let raw = ctx
            .file(&path)
            .map_err(source_error)?
            .ok_or(DurableError::Conflict(
                "mixed Agent baseline resource not selected",
            ))?;
        let digest = Digest256::of_bytes(raw);
        if selected_profiles
            .get(baseline)
            .is_some_and(|prior| *prior != digest)
        {
            return Err(DurableError::Conflict(
                "mixed Agent baseline differs from selected profile tree",
            ));
        }
        selected_profiles.insert(baseline.to_owned(), digest);
    }
    let mut profile_values = Vec::with_capacity(selected_profiles.len());
    for (path, digest) in &selected_profiles {
        active(deadline, cancelled)?;
        let relative = RelativePath::parse(path)
            .map_err(|_| DurableError::Corrupt("mixed Agent profile path"))?;
        let raw = ctx
            .file(&relative)
            .map_err(source_error)?
            .ok_or(DurableError::Conflict(
                "mixed Agent profile resource not selected",
            ))?;
        if Digest256::of_bytes(raw) != *digest {
            return Err(DurableError::Conflict(
                "mixed Agent profile resource digest differs",
            ));
        }
        profile_values.push((path.as_str(), cmd::string(&digest.to_hex())));
    }
    let profile_map = cmd::object(profile_values);
    let template = crate::source_creation::agent_dependency_template(ctx, profile_map)
        .map_err(source_error)?;
    let template_bytes = cmd::canonical(&template).map_err(source_error)?;
    let mut dependency_hash = Digest256Hasher::new();
    part(
        &mut dependency_hash,
        b"tos-managed-mixed-agent-inventory-dependencies-v1",
    );
    part(&mut dependency_hash, &1u16.to_be_bytes());
    part(&mut dependency_hash, MIXED_AGENT_PROFILE_KIND);
    part(&mut dependency_hash, b"configuration-raw-sha256-v1");
    part(
        &mut dependency_hash,
        Digest256::of_bytes(&ctx.configuration_raw).as_bytes(),
    );
    part(
        &mut dependency_hash,
        b"agent-dependency-template-canonical-v1",
    );
    part(&mut dependency_hash, &template_bytes);
    Ok(dependency_hash.finalize().to_prefixed())
}

fn collect_profile_map(
    value: &tos_foundation::JsonValue,
) -> DurableResult<BTreeMap<String, Digest256>> {
    let source_profiles = cmd::field(value, "source_profiles").map_err(source_error)?;
    let source_profiles = source_profiles
        .as_object()
        .ok_or(DurableError::Corrupt("source profile map"))?;
    let mut profiles = BTreeMap::new();
    for (path, value) in source_profiles {
        let path = path
            .as_str()
            .ok_or(DurableError::Corrupt("source profile locator"))?;
        RelativePath::parse(path).map_err(|_| DurableError::Corrupt("source profile path"))?;
        let text = value
            .as_str()
            .ok_or(DurableError::Corrupt("source profile digest"))?;
        let digest = Digest256::from_hex(text)
            .map_err(|_| DurableError::Corrupt("source profile digest"))?;
        profiles.insert(path.to_owned(), digest);
    }
    if profiles.len() as u64 > MAX_PROFILE_COUNT {
        return Err(DurableError::Refused(
            "source profile map exceeds selected file bound",
        ));
    }
    Ok(profiles)
}

fn merge_profile(
    profiles: &mut BTreeMap<String, Digest256>,
    path: &str,
    digest: Digest256,
) -> DurableResult<()> {
    if profiles.get(path).is_some_and(|prior| prior != &digest) {
        return Err(DurableError::Corrupt(
            "source profile contributions disagree",
        ));
    }
    if !profiles.contains_key(path) && profiles.len() as u64 >= MAX_PROFILE_COUNT {
        return Err(DurableError::Refused(
            "source profile union exceeds selected file bound",
        ));
    }
    profiles.insert(path.to_owned(), digest);
    Ok(())
}

fn parse_mixed_projection(
    subject: &str,
    raw: &[u8],
    expected_content_digest: Option<&str>,
) -> DurableResult<tos_foundation::JsonValue> {
    if raw.len() > 1_048_576 {
        return Err(DurableError::Refused(
            "mixed projection exceeds cold metadata bound",
        ));
    }
    let value = cmd::parse(raw).map_err(source_error)?;
    cmd::exact_keys(
        &value,
        tos_compiler::managed_source::MANAGED_MIXED_INVENTORY_MEMBER_FIELDS_V2,
    )
    .map_err(source_error)?;
    let raw_digest = cmd::text(&value, "raw_sha256").map_err(source_error)?;
    let parsed_digest = Digest256::from_hex(raw_digest)
        .map_err(|_| DurableError::Corrupt("mixed projection source digest"))?;
    let records = cmd::field(&value, "records")
        .map_err(source_error)?
        .as_object()
        .ok_or(DurableError::Corrupt("mixed projection records map"))?;
    for kind in crate::source_claims::MIXED_INVENTORY_RECORD_KINDS {
        if !records.iter().any(|(key, _)| key.as_str() == Some(*kind)) {
            return Err(DurableError::Corrupt(
                "mixed projection record kind coverage",
            ));
        }
    }
    if records.iter().any(|(_, rows)| rows.as_array().is_none()) {
        return Err(DurableError::Corrupt("mixed projection record rows"));
    }
    // The source owner validates the selected registry and exact record/schema
    // before rendering. Its registry-defined families are not a fixed enum.
    // Here bind any extension to this one member and its selected inputs;
    // this structural check does not grant registration or source admission.
    let mut extension = None;
    for (key, rows) in records {
        let kind = key
            .as_str()
            .filter(|kind| !kind.is_empty())
            .ok_or(DurableError::Corrupt("mixed projection record family key"))?;
        if !crate::source_claims::MIXED_INVENTORY_RECORD_KINDS.contains(&kind) {
            if extension.replace((kind, rows)).is_some() {
                return Err(DurableError::Corrupt(
                    "mixed projection has multiple member profile families",
                ));
            }
        }
    }
    if let Some((kind, rows)) = extension {
        let rows = rows.as_array().ok_or(DurableError::Corrupt(
            "mixed projection profile record rows",
        ))?;
        if rows.len() != 1
            || !subject.starts_with("ToS/source-witnesses/")
            || subject.rsplit('/').next() != Some(format!("{kind}.json").as_str())
            || records.iter().any(|(key, rows)| {
                key.as_str() != Some(kind) && rows.as_array().is_none_or(|rows| !rows.is_empty())
            })
        {
            return Err(DurableError::Corrupt(
                "mixed projection profile record coverage differs",
            ));
        }
        let row = &rows[0];
        let schema = cmd::text(row, "source_schema_ref").map_err(source_error)?;
        let profiles = collect_profile_map(&value)?;
        if cmd::text(row, "schema_version").map_err(source_error)?
            != "tos_source_witness_catalog_entry_v1"
            || cmd::text(row, "record_type").map_err(source_error)? != kind
            || cmd::text(row, "source_record_ref").map_err(source_error)? != subject
            || cmd::text(row, "record_id")
                .map_err(source_error)?
                .is_empty()
            || Digest256::from_hex(cmd::text(row, "record_sha256").map_err(source_error)?).is_err()
            || !schema.starts_with("ToS/contracts/")
            || !schema.ends_with(".schema.json")
            || !profiles.contains_key(schema)
            || MIXED_AGENT_BASELINE
                .iter()
                .any(|path| !profiles.contains_key(*path))
        {
            return Err(DurableError::Corrupt(
                "mixed projection profile record binding differs",
            ));
        }
    }
    let form_required = crate::source_claims::nonprojecting_custody_kind(subject).is_none()
        && subject.starts_with("ToS/source-witnesses/")
        && subject
            .rsplit('/')
            .next()
            .is_some_and(|basename| basename.ends_with(".human-forms.json"));
    if form_required
        && value
            .object_get("form")
            .is_none_or(|form| form.as_object().is_none())
    {
        return Err(DurableError::Corrupt(
            "mixed projection HumanForm contribution is absent",
        ));
    }
    if let Some(form) = value.object_get("form") {
        if form.as_object().is_some() {
            cmd::exact_keys(form, &["raw_sha256", "form_ids"]).map_err(source_error)?;
            if is_claim_form_path(subject) {
                validate_claim_form_projection(form, subject, &parsed_digest.to_hex())?;
            } else {
                validate_record_form_projection(form, subject, &parsed_digest.to_hex())?;
            }
        } else if form != &tos_foundation::JsonValue::Null {
            return Err(DurableError::Corrupt("mixed projection form shape"));
        }
    }
    let claim_owner = cmd::field(&value, "claim_owner").map_err(source_error)?;
    if crate::source_claims::nonprojecting_custody_kind(subject).is_some() {
        validate_nonprojecting_source_custody(claim_owner, subject)?;
        let events = cmd::field(&value, "events").map_err(source_error)?;
        let anchors = cmd::field(&value, "anchors").map_err(source_error)?;
        let form = cmd::field(&value, "form").map_err(source_error)?;
        let public_text_owner = cmd::field(&value, "public_text_owner").map_err(source_error)?;
        if records
            .iter()
            .any(|(_, rows)| rows.as_array().is_none_or(|rows| !rows.is_empty()))
            || events.as_object().is_none_or(|rows| !rows.is_empty())
            || anchors.as_object().is_none_or(|rows| !rows.is_empty())
            || form != &tos_foundation::JsonValue::Null
            || public_text_owner != &tos_foundation::JsonValue::Null
        {
            return Err(DurableError::Corrupt(
                "nonprojecting source custody carries projected content",
            ));
        }
    } else if is_claim_stream_path(subject) {
        validate_claim_owner(claim_owner, subject, raw_digest)?;
    } else if claim_owner != &tos_foundation::JsonValue::Null {
        return Err(DurableError::Corrupt(
            "Claim owner contribution occurs on a non-Claim member",
        ));
    }
    validate_public_text_owner(
        cmd::field(&value, "public_text_owner").map_err(source_error)?,
        subject,
        raw_digest,
    )?;
    if cmd::text(&value, "schema_version").map_err(source_error)?
        != tos_compiler::managed_source::MANAGED_MIXED_INVENTORY_MEMBER_SCHEMA_V2
        || cmd::text(&value, "path").map_err(source_error)? != subject
        || expected_content_digest.is_some_and(|expected| expected != parsed_digest.to_hex())
        || cmd::canonical(&value).map_err(source_error)? != raw
    {
        return Err(DurableError::Corrupt(
            "mixed projection raw/classification binding differs",
        ));
    }
    RelativePath::parse(subject)
        .map_err(|_| DurableError::Corrupt("mixed projection path invalid"))?;
    collect_profile_map(&value)?;
    Ok(value)
}

fn validate_claim_form_projection(
    form: &tos_foundation::JsonValue,
    subject: &str,
    raw_sha256: &str,
) -> DurableResult<()> {
    let basename = subject.rsplit('/').next().unwrap_or(subject);
    let stem = ["source-claims.", "historical-claims."]
        .iter()
        .find_map(|prefix| basename.strip_prefix(prefix))
        .and_then(|name| name.strip_suffix(".human-forms.json"))
        .ok_or(DurableError::Corrupt("Claim form projection path"))?;
    let _ = Digest256::from_hex(stem)
        .map_err(|_| DurableError::Corrupt("Claim form projection identity path"))?;
    if !subject.starts_with("ToS/source-witnesses/")
        || owner_digest(form, "raw_sha256")?
            != Digest256::from_hex(raw_sha256)
                .map_err(|_| DurableError::Corrupt("Claim form source digest"))?
    {
        return Err(DurableError::Corrupt(
            "Claim form projection source binding",
        ));
    }
    let ids = cmd::field(form, "form_ids")
        .map_err(source_error)?
        .as_array()
        .ok_or(DurableError::Corrupt("Claim form projection identity list"))?;
    let mut seen = BTreeSet::new();
    for id in ids {
        let id = id
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or(DurableError::Corrupt("Claim form projection identity"))?;
        if !seen.insert(id.to_owned()) {
            return Err(DurableError::Corrupt(
                "Claim form projection duplicate identity",
            ));
        }
    }
    Ok(())
}

fn is_claim_form_path(subject: &str) -> bool {
    let basename = subject.rsplit('/').next().unwrap_or(subject);
    subject.starts_with("ToS/source-witnesses/")
        && (basename.starts_with("source-claims.") || basename.starts_with("historical-claims."))
        && basename.ends_with(".human-forms.json")
}

fn is_claim_stream_path(subject: &str) -> bool {
    let basename = subject.rsplit('/').next().unwrap_or(subject);
    // Match the source owner's declared stream families. Laboratory packets
    // such as gold-set graph-claims.jsonl retain their exact source bytes but
    // are not maintained Claim packages or imported catalog streams merely
    // because their filename has the same suffix.
    (subject.starts_with("ToS/source-witnesses/") && basename == crate::source_claims::CLAIM_STREAM)
        || crate::source_claims::is_imported_legacy_claim_path(subject)
        || crate::source_claims::is_historical_claim_stream_path(subject)
}

fn validate_record_form_projection(
    form: &tos_foundation::JsonValue,
    subject: &str,
    raw_sha256: &str,
) -> DurableResult<()> {
    let basename = subject.rsplit('/').next().unwrap_or(subject);
    let stem = basename
        .strip_suffix(".human-forms.json")
        .filter(|stem| !stem.is_empty())
        .ok_or(DurableError::Corrupt("record form projection path"))?;
    let record_path = format!(
        "{}/{}.json",
        subject.rsplit_once('/').map_or("", |(parent, _)| parent),
        stem
    );
    if !subject.starts_with("ToS/source-witnesses/")
        || RelativePath::parse(&record_path).is_err()
        || owner_digest(form, "raw_sha256")?
            != Digest256::from_hex(raw_sha256)
                .map_err(|_| DurableError::Corrupt("record form source digest"))?
    {
        return Err(DurableError::Corrupt("record form projection binding"));
    }
    let ids = cmd::field(form, "form_ids")
        .map_err(source_error)?
        .as_array()
        .ok_or(DurableError::Corrupt("record form projection identities"))?;
    if ids.is_empty() {
        return Err(DurableError::Corrupt("record form projection is empty"));
    }
    let mut seen = BTreeSet::new();
    for id in ids {
        let id = id
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or(DurableError::Corrupt("record form projection identity"))?;
        if !seen.insert(id.to_owned()) {
            return Err(DurableError::Corrupt(
                "record form projection duplicate identity",
            ));
        }
    }
    Ok(())
}

fn owner_digest(owner: &tos_foundation::JsonValue, key: &str) -> DurableResult<Digest256> {
    Digest256::from_prefixed(cmd::text(owner, key).map_err(source_error)?)
        .map_err(|_| DurableError::Corrupt("mixed owner contribution digest"))
}

fn validate_owner_member_rows(
    members: &tos_foundation::JsonValue,
    expected_prefix: &str,
    allowed_roles: &[&str],
) -> DurableResult<BTreeMap<String, (String, u64, Digest256)>> {
    let rows = members
        .as_array()
        .ok_or(DurableError::Corrupt("mixed owner member rows"))?;
    let mut result = BTreeMap::new();
    let mut previous: Option<&str> = None;
    for member in rows {
        cmd::exact_keys(member, &["ref", "role", "sha256", "bytes"]).map_err(source_error)?;
        let path = cmd::text(member, "ref").map_err(source_error)?;
        let role = cmd::text(member, "role").map_err(source_error)?;
        let digest = Digest256::from_prefixed(cmd::text(member, "sha256").map_err(source_error)?)
            .map_err(|_| DurableError::Corrupt("mixed owner member digest"))?;
        let bytes = cmd::integer(member, "bytes").map_err(source_error)?;
        let tail = path
            .strip_prefix(expected_prefix)
            .ok_or(DurableError::Corrupt("mixed owner member prefix"))?;
        if tail.is_empty()
            || tail.contains('/')
            || RelativePath::parse(path).is_err()
            || previous.is_some_and(|prior| prior.as_bytes() >= path.as_bytes())
            || !allowed_roles.contains(&role)
            || bytes > 8_388_608
        {
            return Err(DurableError::Corrupt("mixed owner member row binding"));
        }
        previous = Some(path);
        if result
            .insert(path.to_owned(), (role.to_owned(), bytes, digest))
            .is_some()
        {
            return Err(DurableError::Corrupt("mixed owner member duplicate"));
        }
    }
    Ok(result)
}

fn validate_owner_closure(owner: &tos_foundation::JsonValue) -> DurableResult<()> {
    let object = owner
        .as_object()
        .ok_or(DurableError::Corrupt("mixed owner closure object"))?;
    let mut core = object
        .iter()
        .filter(|(key, _)| key.as_str() != Some("closure_sha256"))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<Vec<_>>();
    core.sort_by(|left, right| left.0.as_str().cmp(&right.0.as_str()));
    if cmd::record_digest(&tos_foundation::JsonValue::Object(core)).map_err(source_error)?
        != owner_digest(owner, "closure_sha256")?
    {
        return Err(DurableError::Corrupt("mixed owner closure root differs"));
    }
    Ok(())
}

fn growth_receipt_name(schema: &str) -> Option<&'static str> {
    Some(match schema {
        "tos_managed_collection_membership_claim_owner_inventory_v1" => {
            "membership-attachment-receipt.json"
        }
        "tos_managed_expression_edition_claim_owner_inventory_v1" => {
            "expression-edition-receipt.json"
        }
        "tos_managed_expression_responsibility_claim_owner_inventory_v1" => {
            "responsibility-attachment-receipt.json"
        }
        "tos_managed_work_expression_claim_owner_inventory_v1" => "work-expression-receipt.json",
        "tos_managed_object_link_claim_owner_inventory_v1" => "object-link-creation-receipt.json",
        _ => return None,
    })
}

fn validate_growth_claim_owner(
    owner: &tos_foundation::JsonValue,
    subject: &str,
    raw_sha256: &str,
    schema: &str,
    allowed_roles: &[&str],
    expected_claim_count: usize,
) -> DurableResult<()> {
    cmd::exact_keys(
        owner,
        &[
            "schema_version",
            "package_home",
            "source_path",
            "stream_sha256",
            "direct_members",
            "claims",
            "closure_sha256",
        ],
    )
    .map_err(source_error)?;
    let home = subject
        .strip_suffix("/source-claims.jsonl")
        .ok_or(DurableError::Corrupt("Growth Claim owner source path"))?;
    let stream_digest = Digest256::from_hex(raw_sha256)
        .map_err(|_| DurableError::Corrupt("Growth Claim stream digest"))?;
    if cmd::text(owner, "schema_version").map_err(source_error)? != schema
        || cmd::text(owner, "package_home").map_err(source_error)? != home
        || cmd::text(owner, "source_path").map_err(source_error)? != subject
        || owner_digest(owner, "stream_sha256")? != stream_digest
        || !home.starts_with("ToS/source-witnesses/")
    {
        return Err(DurableError::Corrupt("Growth Claim owner source binding"));
    }
    let direct = validate_owner_member_rows(
        cmd::field(owner, "direct_members").map_err(source_error)?,
        &format!("{home}/"),
        allowed_roles,
    )?;
    let receipt_name =
        growth_receipt_name(schema).ok_or(DurableError::Corrupt("Growth Claim owner schema"))?;
    for (name, role) in [
        ("source-claims.jsonl", "claim-stream"),
        ("source-create-request.json", "creation-request"),
        ("source-create-environment.json", "creation-environment"),
        ("source-create-provenance.jsonl", "creation-event"),
        (receipt_name, "creation-receipt"),
    ] {
        if direct
            .get(&format!("{home}/{name}"))
            .is_none_or(|(actual, _, digest)| {
                actual != role || name == "source-claims.jsonl" && *digest != stream_digest
            })
        {
            return Err(DurableError::Corrupt(
                "Growth Claim owner required member absent",
            ));
        }
    }
    for (path, (role, _, _)) in &direct {
        let basename = path.rsplit('/').next().unwrap_or(path);
        let expected_role = match basename {
            "source-claims.jsonl" => Some("claim-stream"),
            "source-create-request.json" => Some("creation-request"),
            "source-create-environment.json" => Some("creation-environment"),
            "source-create-provenance.jsonl" => Some("creation-event"),
            "claim-revision-history.json" => Some("revision-history"),
            name if name == receipt_name => Some("creation-receipt"),
            name if name.starts_with("source-claims.") && name.ends_with(".human-forms.json") => {
                Some("human-form-set")
            }
            _ => None,
        };
        if expected_role != Some(role.as_str()) {
            return Err(DurableError::Corrupt(
                "Growth Claim owner member role differs",
            ));
        }
    }
    let mut required_paths = BTreeSet::from([
        subject.to_owned(),
        format!("{home}/source-create-request.json"),
        format!("{home}/source-create-environment.json"),
        format!("{home}/source-create-provenance.jsonl"),
        format!("{home}/{receipt_name}"),
    ]);
    let history_path = format!("{home}/claim-revision-history.json");
    if direct.contains_key(&history_path) {
        required_paths.insert(history_path);
    }
    let claims = cmd::field(owner, "claims")
        .map_err(source_error)?
        .as_array()
        .ok_or(DurableError::Corrupt("Growth Claim owner claims"))?;
    if claims.len() != expected_claim_count || claims.is_empty() {
        return Err(DurableError::Corrupt(
            "Growth Claim owner claim count differs",
        ));
    }
    let expected_predicate = match schema {
        "tos_managed_collection_membership_claim_owner_inventory_v1" => Some("contains_work"),
        "tos_managed_expression_edition_claim_owner_inventory_v1" => Some("embodied_by"),
        "tos_managed_expression_responsibility_claim_owner_inventory_v1" => Some("translated_by"),
        "tos_managed_work_expression_claim_owner_inventory_v1" => Some("has_expression"),
        _ => None,
    };
    let object_link = schema == "tos_managed_object_link_claim_owner_inventory_v1";
    let mut claim_ids = BTreeSet::new();
    let mut expected_forms = BTreeSet::new();
    for claim in claims {
        let expected_claim_schema = if object_link {
            "tos_object_link_claim_v2"
        } else {
            "tos_source_relation_claim_v1"
        };
        if cmd::text(claim, "schema_version").map_err(source_error)? != expected_claim_schema {
            return Err(DurableError::Corrupt("Growth Claim source schema differs"));
        }
        let claim_id = cmd::text(claim, "claim_id").map_err(source_error)?;
        let claim_version = cmd::integer(claim, "claim_version").map_err(source_error)?;
        let predicate = cmd::text(claim, "predicate").map_err(source_error)?;
        if !claim_id.starts_with("tos.claim.")
            || claim_id.len() > 4_096
            || claim_version == 0
            || predicate.is_empty()
            || !cmd::text(claim, "subject_ref")
                .map_err(source_error)?
                .starts_with("tos.")
            || !cmd::text(claim, "object")
                .map_err(source_error)?
                .starts_with("tos.")
            || !cmd::text(claim, "provenance_event_ref")
                .map_err(source_error)?
                .starts_with("tos.")
            || cmd::text(claim, "claim_type").map_err(source_error)? != "relation"
            || expected_predicate.is_some_and(|expected| predicate != expected)
            || (object_link
                && (cmd::text(claim, "claim_type").map_err(source_error)? != "relation"
                    || cmd::text(claim, "assertion_layer").map_err(source_error)?
                        != "forensic_observation"
                    || ![
                        "described_by",
                        "metadata_at",
                        "downloadable_at",
                        "rights_statement_at",
                    ]
                    .contains(&predicate)
                    || !cmd::text(claim, "subject_ref")
                        .map_err(source_error)?
                        .starts_with("tos.")
                    || !cmd::text(claim, "object")
                        .map_err(source_error)?
                        .starts_with("tos.link.")
                    || cmd::text(claim, "review_status").map_err(source_error)? != "unreviewed"))
            || !claim_ids.insert(claim_id.to_owned())
        {
            return Err(DurableError::Corrupt(
                "Growth Claim identity or relation differs",
            ));
        }
        let form_path = format!(
            "{home}/source-claims.{}.human-forms.json",
            Digest256::of_bytes(claim_id.as_bytes()).to_hex()
        );
        if direct
            .get(&form_path)
            .is_none_or(|(role, _, _)| role != "human-form-set")
        {
            return Err(DurableError::Corrupt("Growth Claim form member absent"));
        }
        expected_forms.insert(form_path);
    }
    let direct_forms = direct
        .iter()
        .filter(|(_, (role, _, _))| role == "human-form-set")
        .map(|(path, _)| path.clone())
        .collect::<BTreeSet<_>>();
    if direct_forms != expected_forms {
        return Err(DurableError::Corrupt(
            "Growth Claim form membership differs",
        ));
    }
    required_paths.extend(expected_forms);
    if direct.keys().cloned().collect::<BTreeSet<_>>() != required_paths {
        return Err(DurableError::Corrupt(
            "Growth Claim direct package closure differs",
        ));
    }
    validate_owner_closure(owner)
}

fn validate_item_claim_owner(
    owner: &tos_foundation::JsonValue,
    subject: &str,
    raw_sha256: &str,
) -> DurableResult<()> {
    cmd::exact_keys(
        owner,
        &[
            "schema_version",
            "package_home",
            "source_path",
            "stream_sha256",
            "direct_members",
            "claims",
            "lineage",
            "closure_sha256",
        ],
    )
    .map_err(source_error)?;
    let home = subject
        .strip_suffix("/source-claims.jsonl")
        .ok_or(DurableError::Corrupt("Item Claim owner source path"))?;
    let stream_digest = Digest256::from_hex(raw_sha256)
        .map_err(|_| DurableError::Corrupt("Item Claim stream digest"))?;
    if cmd::text(owner, "schema_version").map_err(source_error)?
        != "tos_managed_item_adoption_claim_owner_inventory_v1"
        || cmd::text(owner, "package_home").map_err(source_error)? != home
        || cmd::text(owner, "source_path").map_err(source_error)? != subject
        || owner_digest(owner, "stream_sha256")? != stream_digest
        || !is_managed_item_home(home)
    {
        return Err(DurableError::Corrupt("Item Claim owner source binding"));
    }
    let direct = validate_owner_member_rows(
        cmd::field(owner, "direct_members").map_err(source_error)?,
        &format!("{home}/"),
        &[
            "item-record",
            "item-form-set",
            "claim-stream",
            "creation-request",
            "creation-environment",
            "creation-event",
            "compound-receipt",
            "file-manifest",
            "rights-record",
            "resource-inventory",
            "fixity",
            "forensic-report",
            "provenance",
            "deposit-receipt",
            "claim-form-set",
        ],
    )?;
    let expected_names = [
        ("item.json", "item-record"),
        ("item.human-forms.json", "item-form-set"),
        ("source-claims.jsonl", "claim-stream"),
        ("source-create-request.json", "creation-request"),
        ("source-create-environment.json", "creation-environment"),
        ("source-create-provenance.jsonl", "creation-event"),
        ("edition-item-receipt.json", "compound-receipt"),
        ("item.manifest.json", "file-manifest"),
        ("rights.json", "rights-record"),
        ("resource-inventory.json", "resource-inventory"),
        ("fixity.sha256", "fixity"),
        ("forensic-report.md", "forensic-report"),
        ("provenance.jsonl", "provenance"),
        ("item-deposit-receipt.json", "deposit-receipt"),
    ];
    if direct.len() != 15 {
        return Err(DurableError::Corrupt(
            "Item Claim owner member count differs",
        ));
    }
    for (name, role) in expected_names {
        if direct
            .get(&format!("{home}/{name}"))
            .is_none_or(|(actual, _, digest)| {
                actual != role || name == "source-claims.jsonl" && *digest != stream_digest
            })
        {
            return Err(DurableError::Corrupt(
                "Item Claim owner required member absent",
            ));
        }
    }
    for (path, (role, _, _)) in &direct {
        let basename = path.rsplit('/').next().unwrap_or(path);
        let expected_role = expected_names
            .iter()
            .find(|(name, _)| *name == basename)
            .map(|(_, role)| *role)
            .or_else(|| {
                (basename.starts_with("source-claims.") && basename.ends_with(".human-forms.json"))
                    .then_some("claim-form-set")
            });
        if expected_role != Some(role.as_str()) {
            return Err(DurableError::Corrupt(
                "Item Claim owner member role differs",
            ));
        }
    }
    let claims = cmd::field(owner, "claims")
        .map_err(source_error)?
        .as_array()
        .ok_or(DurableError::Corrupt("Item Claim owner claims"))?;
    if claims.len() != 1 {
        return Err(DurableError::Corrupt(
            "Item Claim owner claim count differs",
        ));
    }
    let claim_row = &claims[0];
    cmd::exact_keys(
        claim_row,
        &[
            "source_ref",
            "source_line",
            "claim_id",
            "claim_version",
            "claim_sha256",
            "claim",
            "form",
        ],
    )
    .map_err(source_error)?;
    let claim = cmd::field(claim_row, "claim").map_err(source_error)?;
    let claim_id = cmd::text(claim_row, "claim_id").map_err(source_error)?;
    let claim_version = cmd::integer(claim_row, "claim_version").map_err(source_error)?;
    let claim_digest = owner_digest(claim_row, "claim_sha256")?;
    let claim_form = cmd::field(claim_row, "form").map_err(source_error)?;
    cmd::exact_keys(claim_form, &["ref", "sha256", "form_ids", "subject"]).map_err(source_error)?;
    let form_path = format!(
        "{home}/source-claims.{}.human-forms.json",
        Digest256::of_bytes(claim_id.as_bytes()).to_hex()
    );
    let form_digest = owner_digest(claim_form, "sha256")?;
    let form_ids = cmd::field(claim_form, "form_ids")
        .map_err(source_error)?
        .as_array()
        .ok_or(DurableError::Corrupt("Item Claim form identities"))?;
    let expected_form_subject =
        cmd::reference(claim, "claim_id", "claim_version").map_err(source_error)?;
    let mut form_id_set = BTreeSet::new();
    if cmd::text(claim_row, "source_ref").map_err(source_error)? != subject
        || cmd::integer(claim_row, "source_line").map_err(source_error)? == 0
        || claim_id.is_empty()
        || claim_version != 1
        || cmd::text(claim, "schema_version").map_err(source_error)?
            != "tos_source_relation_claim_v1"
        || cmd::text(claim, "claim_id").map_err(source_error)? != claim_id
        || cmd::integer(claim, "claim_version").map_err(source_error)? != claim_version
        || cmd::text(claim, "predicate").map_err(source_error)? != "exemplified_by"
        || cmd::record_digest(claim).map_err(source_error)? != claim_digest
        || cmd::text(claim_form, "ref").map_err(source_error)? != form_path
        || direct
            .get(&form_path)
            .is_none_or(|(role, _, digest)| role != "claim-form-set" || *digest != form_digest)
        || !cmd::same(
            cmd::field(claim_form, "subject").map_err(source_error)?,
            &expected_form_subject,
        )
        .map_err(source_error)?
        || form_ids.is_empty()
        || form_ids.iter().any(|id| {
            id.as_str().is_none_or(str::is_empty)
                || !form_id_set.insert(id.as_str().unwrap_or_default().to_owned())
        })
    {
        return Err(DurableError::Corrupt(
            "Item Claim/form projection binding differs",
        ));
    }
    let mut expected_direct = expected_names
        .iter()
        .map(|(name, _)| format!("{home}/{name}"))
        .collect::<BTreeSet<_>>();
    expected_direct.insert(form_path);
    if direct.keys().cloned().collect::<BTreeSet<_>>() != expected_direct {
        return Err(DurableError::Corrupt(
            "Item Claim direct package closure differs",
        ));
    }
    let lineage = cmd::field(owner, "lineage").map_err(source_error)?;
    validate_item_lineage(lineage, &direct, home)?;
    let scope = cmd::field(lineage, "scope").map_err(source_error)?;
    if cmd::text(claim, "subject_ref").map_err(source_error)?
        != cmd::text(scope, "edition_id").map_err(source_error)?
        || cmd::text(claim, "object").map_err(source_error)?
            != cmd::text(scope, "item_id").map_err(source_error)?
        || cmd::text(claim, "provenance_event_ref").map_err(source_error)?
            != cmd::text(scope, "provenance_event_id").map_err(source_error)?
    {
        return Err(DurableError::Corrupt("Item Claim scope projection differs"));
    }
    validate_owner_closure(owner)
}

fn is_managed_item_home(home: &str) -> bool {
    if RelativePath::parse(home).is_err() {
        return false;
    }
    let parts = home.split('/').collect::<Vec<_>>();
    if parts.len() == 4
        && parts[0] == "ToS"
        && parts[1] == "source-witnesses"
        && parts[2] == "items"
        && !parts[3].is_empty()
    {
        return true;
    }
    parts.len() >= 8
        && parts[0] == "ToS"
        && parts[1] == "source-witnesses"
        && (2..parts.len().saturating_sub(3)).any(|index| {
            parts[index] == "editions"
                && !parts[index + 1].is_empty()
                && parts[index + 2] == "items"
                && index + 3 == parts.len() - 1
                && !parts[index + 3].is_empty()
        })
}

fn validate_item_claim_owner_v2(
    owner: &tos_foundation::JsonValue,
    subject: &str,
    raw_sha256: &str,
) -> DurableResult<()> {
    cmd::exact_keys(
        owner,
        &[
            "schema_version",
            "package_home",
            "source_path",
            "adoption_v1",
            "adoption_item_source",
            "current",
            "closure_sha256",
        ],
    )
    .map_err(source_error)?;
    let home = subject
        .strip_suffix("/source-claims.jsonl")
        .ok_or(DurableError::Corrupt("Item Claim current source path"))?;
    let current_stream_digest = Digest256::from_hex(raw_sha256)
        .map_err(|_| DurableError::Corrupt("Item Claim current stream digest"))?;
    if cmd::text(owner, "schema_version").map_err(source_error)?
        != "tos_managed_item_adoption_claim_owner_inventory_v2"
        || cmd::text(owner, "package_home").map_err(source_error)? != home
        || cmd::text(owner, "source_path").map_err(source_error)? != subject
        || !is_managed_item_home(home)
    {
        return Err(DurableError::Corrupt("Item Claim v2 source binding"));
    }
    let adoption = cmd::field(owner, "adoption_v1").map_err(source_error)?;
    let adoption_stream_digest = owner_digest(adoption, "stream_sha256")?.to_hex();
    validate_item_claim_owner(adoption, subject, &adoption_stream_digest)?;
    let current = cmd::field(owner, "current").map_err(source_error)?;
    let adoption_item_source = cmd::field(owner, "adoption_item_source").map_err(source_error)?;
    validate_item_record_ref(adoption_item_source)?;
    cmd::exact_keys(current, &["edition", "item"]).map_err(source_error)?;
    let edition = cmd::field(current, "edition").map_err(source_error)?;
    let item = cmd::field(current, "item").map_err(source_error)?;
    cmd::exact_keys(
        edition,
        &[
            "source_path",
            "source",
            "history_ref",
            "history_sha256",
            "history_head_source",
            "history_head_sha256",
            "history_chain",
            "direct_members",
        ],
    )
    .map_err(source_error)?;
    cmd::exact_keys(
        item,
        &[
            "source_path",
            "source",
            "claim_id",
            "stream_sha256",
            "history_ref",
            "history_sha256",
            "history_head_source",
            "history_head_sha256",
            "history_chain",
            "direct_members",
        ],
    )
    .map_err(source_error)?;

    let adoption_claims = cmd::field(adoption, "claims")
        .map_err(source_error)?
        .as_array()
        .ok_or(DurableError::Corrupt("Item Claim adoption rows"))?;
    let adoption_claim = adoption_claims
        .first()
        .and_then(|row| row.object_get("claim"))
        .ok_or(DurableError::Corrupt("Item Claim adoption claim"))?;
    let item_id = cmd::text(adoption_claim, "object").map_err(source_error)?;
    let edition_id = cmd::text(adoption_claim, "subject_ref").map_err(source_error)?;
    if cmd::text(adoption_item_source, "id").map_err(source_error)? != item_id
        || cmd::integer(adoption_item_source, "version").map_err(source_error)? != 1
    {
        return Err(DurableError::Corrupt(
            "Item Claim adoption Item v1 reference",
        ));
    }
    let adoption_lineage = cmd::field(adoption, "lineage").map_err(source_error)?;
    let parent_after = cmd::field(adoption_lineage, "parent_after").map_err(source_error)?;
    let expected_edition_home = cmd::text(
        cmd::field(adoption_lineage, "scope").map_err(source_error)?,
        "edition_source_path",
    )
    .map_err(source_error)?
    .strip_suffix("/edition.json")
    .ok_or(DurableError::Corrupt("Item Claim adoption Edition path"))?;
    let edition_path = format!("{expected_edition_home}/edition.json");
    let item_path = format!("{home}/item.json");
    let item_home_prefix = format!("{expected_edition_home}/items/");
    if home
        .strip_prefix(&item_home_prefix)
        .is_none_or(|slug| slug.is_empty() || slug.contains('/'))
    {
        return Err(DurableError::Corrupt(
            "Item Claim current Edition/Item path relation",
        ));
    }
    let edition_source = cmd::field(edition, "source").map_err(source_error)?;
    let item_source = cmd::field(item, "source").map_err(source_error)?;
    let edition_history_head = cmd::field(edition, "history_head_source").map_err(source_error)?;
    let item_history_head = cmd::field(item, "history_head_source").map_err(source_error)?;
    validate_item_record_ref(edition_source)?;
    validate_item_record_ref(item_source)?;
    validate_item_record_ref(edition_history_head)?;
    if cmd::text(edition, "source_path").map_err(source_error)? != edition_path
        || cmd::text(item, "source_path").map_err(source_error)? != item_path
        || cmd::text(edition_source, "id").map_err(source_error)? != edition_id
        || cmd::text(item_source, "id").map_err(source_error)? != item_id
        || cmd::integer(item_source, "version").map_err(source_error)? == 0
        || !cmd::same(edition_source, edition_history_head).map_err(source_error)?
        || cmd::integer(edition_source, "version").map_err(source_error)?
            < cmd::integer(parent_after, "version").map_err(source_error)?
        || cmd::text(edition, "history_ref").map_err(source_error)?
            != format!("{expected_edition_home}/source-revision-history.json")
        || owner_digest(item, "stream_sha256")? != current_stream_digest
    {
        return Err(DurableError::Corrupt(
            "Item Claim current record/history binding",
        ));
    }
    let edition_history_digest = owner_digest(edition, "history_sha256")?;
    let edition_head_digest = owner_digest(edition, "history_head_sha256")?;
    validate_owner_revision_chain(
        cmd::field(edition, "history_chain").map_err(source_error)?,
        cmd::text(edition_source, "id").map_err(source_error)?,
        edition_source,
        edition_head_digest,
        true,
    )?;
    let item_chain = cmd::field(item, "history_chain").map_err(source_error)?;
    let item_chain_rows = item_chain
        .as_array()
        .ok_or(DurableError::Corrupt("Item Claim Item revision chain"))?;
    let item_history_ref = cmd::field(item, "history_ref").map_err(source_error)?;
    let item_history_sha = cmd::field(item, "history_sha256").map_err(source_error)?;
    let item_head_source = cmd::field(item, "history_head_source").map_err(source_error)?;
    let item_head_sha = cmd::field(item, "history_head_sha256").map_err(source_error)?;
    if item_chain_rows.is_empty() {
        if item_history_ref != &tos_foundation::JsonValue::Null
            || item_history_sha != &tos_foundation::JsonValue::Null
            || item_head_source != &tos_foundation::JsonValue::Null
            || item_head_sha != &tos_foundation::JsonValue::Null
            || cmd::integer(item_source, "version").map_err(source_error)? != 1
            || !cmd::same(item_source, adoption_item_source).map_err(source_error)?
        {
            return Err(DurableError::Corrupt(
                "unrevised Item history binding differs",
            ));
        }
    } else {
        if cmd::text(item, "history_ref").map_err(source_error)?
            != format!("{home}/source-revision-history.json")
            || !cmd::same(item_source, item_history_head).map_err(source_error)?
            || !item_chain_rows.first().is_some_and(|row| {
                row.object_get("previous_source")
                    .is_some_and(|source| cmd::same(source, adoption_item_source).unwrap_or(false))
            })
        {
            return Err(DurableError::Corrupt(
                "Item revision history digest differs",
            ));
        }
        validate_item_record_ref(item_head_source)?;
        validate_owner_revision_chain(
            item_chain,
            cmd::text(item_source, "id").map_err(source_error)?,
            item_source,
            owner_digest(item, "history_head_sha256")?,
            false,
        )?;
    }
    let adoption_lineage = cmd::field(adoption, "lineage").map_err(source_error)?;
    let adoption_parent_before =
        cmd::field(adoption_lineage, "parent_before").map_err(source_error)?;
    let adoption_parent_after =
        cmd::field(adoption_lineage, "parent_after").map_err(source_error)?;
    let adoption_transition_sha = owner_digest(adoption_lineage, "parent_transition_sha256")?;
    let adoption_transition_text = adoption_transition_sha.to_prefixed();
    let adoption_archive =
        cmd::text(adoption_lineage, "parent_archive_ref").map_err(source_error)?;
    let matching_adoptions = cmd::field(edition, "history_chain")
        .map_err(source_error)?
        .as_array()
        .ok_or(DurableError::Corrupt("Edition Claim revision chain"))?
        .iter()
        .filter(|row| {
            row.object_get("operation")
                .and_then(tos_foundation::JsonValue::as_str)
                == Some("item.adopt")
                && row
                    .object_get("archive_path")
                    .and_then(tos_foundation::JsonValue::as_str)
                    == Some(adoption_archive)
                && row
                    .object_get("receipt_sha256")
                    .and_then(tos_foundation::JsonValue::as_str)
                    == Some(adoption_transition_text.as_str())
                && row.object_get("previous_source").is_some_and(|source| {
                    cmd::same(source, adoption_parent_before).unwrap_or(false)
                })
                && row
                    .object_get("source")
                    .is_some_and(|source| cmd::same(source, adoption_parent_after).unwrap_or(false))
        })
        .count();
    if matching_adoptions != 1 {
        return Err(DurableError::Corrupt(
            "current Edition history does not retain exact Item adoption",
        ));
    }
    let edition_members = validate_owner_member_rows(
        cmd::field(edition, "direct_members").map_err(source_error)?,
        &format!("{expected_edition_home}/"),
        &[
            "edition-record",
            "edition-form-set",
            "revision-history",
            "compound-receipt",
            "claim-stream",
            "claim-form-set",
            "creation-request",
            "creation-environment",
            "creation-event",
        ],
    )?;
    let item_members = validate_owner_member_rows(
        cmd::field(item, "direct_members").map_err(source_error)?,
        &format!("{home}/"),
        &[
            "item-record",
            "item-form-set",
            "claim-stream",
            "creation-request",
            "creation-environment",
            "creation-event",
            "compound-receipt",
            "file-manifest",
            "rights-record",
            "resource-inventory",
            "fixity",
            "forensic-report",
            "provenance",
            "deposit-receipt",
            "claim-form-set",
            "revision-history",
        ],
    )?;
    let claim_id = cmd::text(item, "claim_id").map_err(source_error)?;
    let form_path = format!(
        "{home}/source-claims.{}.human-forms.json",
        Digest256::of_bytes(claim_id.as_bytes()).to_hex()
    );
    let mut expected_item_names = [
        "item.json",
        "item.human-forms.json",
        "source-claims.jsonl",
        "source-create-request.json",
        "source-create-environment.json",
        "source-create-provenance.jsonl",
        "edition-item-receipt.json",
        "item.manifest.json",
        "rights.json",
        "resource-inventory.json",
        "fixity.sha256",
        "forensic-report.md",
        "provenance.jsonl",
        "item-deposit-receipt.json",
    ]
    .into_iter()
    .map(|name| format!("{home}/{name}"))
    .chain(std::iter::once(form_path.clone()))
    .collect::<BTreeSet<_>>();
    if !item_chain_rows.is_empty() {
        expected_item_names.insert(format!("{home}/source-revision-history.json"));
    }
    let edition_form_paths = edition_members
        .keys()
        .filter(|path| {
            path.rsplit('/').next().is_some_and(|name| {
                name.starts_with("source-claims.") && name.ends_with(".human-forms.json")
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    if edition_form_paths.len() != 1 {
        return Err(DurableError::Corrupt(
            "current Edition Claim form-set count differs",
        ));
    }
    let expected_edition_names = [
        "edition.json",
        "edition.human-forms.json",
        "source-revision-history.json",
        "expression-edition-receipt.json",
        "source-claims.jsonl",
        "source-create-environment.json",
        "source-create-provenance.jsonl",
        "source-create-request.json",
    ]
    .into_iter()
    .map(|name| format!("{expected_edition_home}/{name}"))
    .chain(edition_form_paths.iter().cloned())
    .collect::<BTreeSet<_>>();
    let expected_roles = [
        ("item.json", "item-record"),
        ("item.human-forms.json", "item-form-set"),
        ("source-claims.jsonl", "claim-stream"),
        ("source-create-request.json", "creation-request"),
        ("source-create-environment.json", "creation-environment"),
        ("source-create-provenance.jsonl", "creation-event"),
        ("edition-item-receipt.json", "compound-receipt"),
        ("item.manifest.json", "file-manifest"),
        ("rights.json", "rights-record"),
        ("resource-inventory.json", "resource-inventory"),
        ("fixity.sha256", "fixity"),
        ("forensic-report.md", "forensic-report"),
        ("provenance.jsonl", "provenance"),
        ("item-deposit-receipt.json", "deposit-receipt"),
    ];
    if edition_members.len() != 9
        || edition_members.keys().cloned().collect::<BTreeSet<_>>() != expected_edition_names
        || item_members.keys().cloned().collect::<BTreeSet<_>>() != expected_item_names
    {
        return Err(DurableError::Corrupt(
            "Item Claim current package membership differs",
        ));
    }
    for (name, role) in expected_roles {
        if item_members
            .get(&format!("{home}/{name}"))
            .is_none_or(|(actual, _, _)| actual != role)
        {
            return Err(DurableError::Corrupt(
                "Item Claim current member role differs",
            ));
        }
    }
    if item_members
        .get(&form_path)
        .is_none_or(|(actual, _, _)| actual != "claim-form-set")
    {
        return Err(DurableError::Corrupt(
            "Item Claim current form role differs",
        ));
    }
    let edition_member_roles = [
        ("edition.json", "edition-record"),
        ("edition.human-forms.json", "edition-form-set"),
        ("source-revision-history.json", "revision-history"),
        ("expression-edition-receipt.json", "compound-receipt"),
        ("source-claims.jsonl", "claim-stream"),
        ("source-create-environment.json", "creation-environment"),
        ("source-create-provenance.jsonl", "creation-event"),
        ("source-create-request.json", "creation-request"),
    ];
    for (name, role) in edition_member_roles {
        if edition_members
            .get(&format!("{expected_edition_home}/{name}"))
            .is_none_or(|(actual, _, _)| actual != role)
        {
            return Err(DurableError::Corrupt(
                "Item Claim current Edition role differs",
            ));
        }
    }
    if edition_members
        .get(&edition_form_paths[0])
        .is_none_or(|(actual, _, _)| actual != "claim-form-set")
    {
        return Err(DurableError::Corrupt(
            "Item Claim current Edition form role differs",
        ));
    }
    let item_history_member = item_members.get(&format!("{home}/source-revision-history.json"));
    let item_history_matches = if item_chain_rows.is_empty() {
        item_history_member.is_none()
    } else {
        item_history_member.is_some_and(|(_, _, digest)| {
            owner_digest(item, "history_sha256").is_ok_and(|expected| *digest == expected)
        })
    };
    if item_members
        .get(subject)
        .is_none_or(|(role, _, digest)| role != "claim-stream" || *digest != current_stream_digest)
        || !item_history_matches
        || edition_members
            .get(&format!(
                "{expected_edition_home}/source-revision-history.json"
            ))
            .is_none_or(|(_, _, digest)| *digest != edition_history_digest)
    {
        return Err(DurableError::Corrupt(
            "Item Claim current fixity binding differs",
        ));
    }
    validate_owner_closure(owner)
}

fn validate_owner_member_refs(
    members: &tos_foundation::JsonValue,
    expected_prefix: &str,
) -> DurableResult<BTreeMap<String, (u64, Digest256)>> {
    let rows = members
        .as_array()
        .ok_or(DurableError::Corrupt("Item lineage member rows"))?;
    let mut result = BTreeMap::new();
    let mut previous: Option<&str> = None;
    for member in rows {
        cmd::exact_keys(member, &["ref", "bytes", "sha256"]).map_err(source_error)?;
        let path = cmd::text(member, "ref").map_err(source_error)?;
        let tail = path
            .strip_prefix(expected_prefix)
            .ok_or(DurableError::Corrupt("Item lineage member prefix"))?;
        let bytes = cmd::integer(member, "bytes").map_err(source_error)?;
        let digest = owner_digest(member, "sha256")?;
        if tail.is_empty()
            || tail.contains('/')
            || RelativePath::parse(path).is_err()
            || previous.is_some_and(|prior| prior.as_bytes() >= path.as_bytes())
            || bytes > 8_388_608
            || result.insert(path.to_owned(), (bytes, digest)).is_some()
        {
            return Err(DurableError::Corrupt("Item lineage member binding differs"));
        }
        previous = Some(path);
    }
    if result.is_empty() {
        return Err(DurableError::Corrupt("Item lineage member set is empty"));
    }
    Ok(result)
}

fn validate_item_record_ref(value: &tos_foundation::JsonValue) -> DurableResult<()> {
    cmd::exact_keys(value, &["id", "version", "digest"]).map_err(source_error)?;
    if cmd::text(value, "id").map_err(source_error)?.is_empty()
        || cmd::integer(value, "version").map_err(source_error)? == 0
    {
        return Err(DurableError::Corrupt("Item lineage record reference"));
    }
    let _ = owner_digest(value, "digest")?;
    Ok(())
}

fn validate_owner_revision_chain(
    value: &tos_foundation::JsonValue,
    record_id: &str,
    current_source: &tos_foundation::JsonValue,
    head_digest: Digest256,
    edition_history: bool,
) -> DurableResult<()> {
    let rows = value
        .as_array()
        .ok_or(DurableError::Corrupt("Item Claim revision chain rows"))?;
    if rows.is_empty() {
        return Err(DurableError::Corrupt(
            "Item Claim required revision chain is empty",
        ));
    }
    let mut previous_source: Option<&tos_foundation::JsonValue> = None;
    let mut last_source: Option<&tos_foundation::JsonValue> = None;
    let mut last_digest = None;
    for row in rows {
        cmd::exact_keys(
            row,
            &[
                "operation",
                "archive_path",
                "receipt_sha256",
                "previous_source",
                "source",
            ],
        )
        .map_err(source_error)?;
        let operation = cmd::text(row, "operation").map_err(source_error)?;
        if operation != "record.revise" && !(edition_history && operation == "item.adopt") {
            return Err(DurableError::Corrupt("Item Claim revision chain operation"));
        }
        let archive_ref = cmd::text(row, "archive_path").map_err(source_error)?;
        let archive_path = RelativePath::parse(archive_ref)
            .map_err(|_| DurableError::Corrupt("Item Claim revision archive path"))?;
        if !archive_path
            .as_str()
            .starts_with("ToS/source-witnesses/.record-revisions/")
        {
            return Err(DurableError::Corrupt(
                "Item Claim revision archive namespace",
            ));
        }
        let digest = owner_digest(row, "receipt_sha256")?;
        let before = cmd::field(row, "previous_source").map_err(source_error)?;
        let after = cmd::field(row, "source").map_err(source_error)?;
        validate_item_record_ref(before)?;
        validate_item_record_ref(after)?;
        let before_version = cmd::integer(before, "version").map_err(source_error)?;
        let after_version = cmd::integer(after, "version").map_err(source_error)?;
        let expected_after_version = before_version.checked_add(1);
        if cmd::text(before, "id").map_err(source_error)? != record_id
            || cmd::text(after, "id").map_err(source_error)? != record_id
            || expected_after_version != Some(after_version)
            || previous_source.is_some_and(|previous| !cmd::same(previous, before).unwrap_or(false))
        {
            return Err(DurableError::Corrupt(
                "Item Claim revision chain continuity",
            ));
        }
        previous_source = Some(after);
        last_source = Some(after);
        last_digest = Some(digest);
    }
    if !last_source.is_some_and(|source| cmd::same(source, current_source).unwrap_or(false))
        || last_digest != Some(head_digest)
    {
        return Err(DurableError::Corrupt(
            "Item Claim revision chain tip differs",
        ));
    }
    Ok(())
}

fn validate_item_lineage(
    lineage: &tos_foundation::JsonValue,
    direct: &BTreeMap<String, (String, u64, Digest256)>,
    item_home: &str,
) -> DurableResult<()> {
    cmd::exact_keys(
        lineage,
        &[
            "transaction_id",
            "operation",
            "command_id",
            "request_digest",
            "owner_configuration",
            "recorded_at",
            "scope",
            "dependencies",
            "parent_before",
            "parent_after",
            "parent_archive_ref",
            "parent_transition_sha256",
            "parent_transition",
            "predecessor_members",
            "successor_members",
            "native_receipt_sha256",
            "native_receipt",
        ],
    )
    .map_err(source_error)?;
    let transaction_id = cmd::text(lineage, "transaction_id").map_err(source_error)?;
    let command_id = cmd::text(lineage, "command_id").map_err(source_error)?;
    let parent_before = cmd::field(lineage, "parent_before").map_err(source_error)?;
    let parent_after = cmd::field(lineage, "parent_after").map_err(source_error)?;
    validate_item_record_ref(parent_before)?;
    validate_item_record_ref(parent_after)?;
    if transaction_id.is_empty()
        || cmd::text(lineage, "operation").map_err(source_error)? != "item.adopt"
        || command_id.is_empty()
        || cmd::text(lineage, "recorded_at")
            .map_err(source_error)?
            .is_empty()
        || cmd::field(lineage, "scope")
            .map_err(source_error)?
            .as_object()
            .is_none()
        || cmd::text(parent_before, "id").map_err(source_error)?
            != cmd::text(parent_after, "id").map_err(source_error)?
    {
        return Err(DurableError::Corrupt("Item lineage identity differs"));
    }
    for key in [
        "request_digest",
        "owner_configuration",
        "parent_transition_sha256",
        "dependencies",
    ] {
        let _ = owner_digest(lineage, key)?;
    }
    let parent_archive_ref = cmd::text(lineage, "parent_archive_ref").map_err(source_error)?;
    let archive_path = RelativePath::parse(parent_archive_ref)
        .map_err(|_| DurableError::Corrupt("Item lineage archive path"))?;
    if !archive_path
        .as_str()
        .starts_with("ToS/source-witnesses/.record-revisions/")
    {
        return Err(DurableError::Corrupt("Item lineage archive namespace"));
    }
    let parent_home = cmd::field(lineage, "scope")
        .map_err(source_error)?
        .object_get("edition_source_path")
        .and_then(tos_foundation::JsonValue::as_str)
        .and_then(|path| path.rsplit_once('/').map(|(parent, _)| parent))
        .ok_or(DurableError::Corrupt("Item lineage Edition source path"))?;
    if parent_home == item_home {
        return Err(DurableError::Corrupt(
            "Item lineage parent/child homes collide",
        ));
    }
    let scope = cmd::field(lineage, "scope").map_err(source_error)?;
    if cmd::text(scope, "edition_source_path").map_err(source_error)?
        != format!("{parent_home}/edition.json")
        || cmd::text(scope, "item_source_path").map_err(source_error)?
            != format!("{item_home}/item.json")
        || cmd::text(scope, "edition_id").map_err(source_error)?
            != cmd::text(parent_after, "id").map_err(source_error)?
        || cmd::text(scope, "edition_id").map_err(source_error)?
            != cmd::text(parent_before, "id").map_err(source_error)?
        || cmd::text(scope, "item_id")
            .map_err(source_error)?
            .is_empty()
        || cmd::text(scope, "provenance_event_id")
            .map_err(source_error)?
            .is_empty()
    {
        return Err(DurableError::Corrupt("Item lineage source scope differs"));
    }
    let predecessor = validate_owner_member_refs(
        cmd::field(lineage, "predecessor_members").map_err(source_error)?,
        &format!("{parent_home}/"),
    )?;
    let successor = validate_owner_member_refs(
        cmd::field(lineage, "successor_members").map_err(source_error)?,
        &format!("{parent_home}/"),
    )?;
    if !predecessor.contains_key(&format!("{parent_home}/edition.json"))
        || !successor.contains_key(&format!("{parent_home}/edition.json"))
        || !successor.contains_key(&format!("{parent_home}/source-revision-history.json"))
    {
        return Err(DurableError::Corrupt(
            "Item lineage parent package incomplete",
        ));
    }
    let parent_transition = cmd::field(lineage, "parent_transition").map_err(source_error)?;
    if cmd::record_digest(parent_transition).map_err(source_error)?
        != owner_digest(lineage, "parent_transition_sha256")?
        || cmd::text(parent_transition, "command_id").map_err(source_error)? != command_id
        || cmd::text(parent_transition, "request_digest").map_err(source_error)?
            != cmd::text(lineage, "request_digest").map_err(source_error)?
        || cmd::text(parent_transition, "owner_configuration").map_err(source_error)?
            != cmd::text(lineage, "owner_configuration").map_err(source_error)?
        || !cmd::same(
            cmd::field(parent_transition, "source").map_err(source_error)?,
            parent_after,
        )
        .map_err(source_error)?
        || !cmd::same(
            cmd::field(parent_transition, "previous_source").map_err(source_error)?,
            parent_before,
        )
        .map_err(source_error)?
    {
        return Err(DurableError::Corrupt(
            "Item lineage parent transition differs",
        ));
    }
    let receipt = cmd::field(lineage, "native_receipt").map_err(source_error)?;
    if cmd::text(receipt, "schema_version").map_err(source_error)? != "tos_edition_item_receipt_v1"
        || cmd::text(receipt, "operation").map_err(source_error)? != "item.adopt"
        || cmd::text(receipt, "transaction_id").map_err(source_error)? != transaction_id
        || cmd::text(receipt, "command_id").map_err(source_error)? != command_id
        || cmd::record_digest(receipt).map_err(source_error)?
            != owner_digest(lineage, "native_receipt_sha256")?
        || cmd::text(receipt, "parent_archive_ref").map_err(source_error)? != parent_archive_ref
        || cmd::text(receipt, "parent_transition_sha256").map_err(source_error)?
            != cmd::text(lineage, "parent_transition_sha256").map_err(source_error)?
    {
        return Err(DurableError::Corrupt("Item native receipt binding differs"));
    }
    for (left, right) in [
        ("request_digest", "request_digest"),
        ("owner_configuration", "owner_configuration"),
        ("recorded_at", "recorded_at"),
        ("scope", "scope"),
        ("dependencies", "dependencies"),
        ("parent_before", "parent_before"),
        ("parent_after", "parent_after"),
    ] {
        if !cmd::same(
            cmd::field(lineage, left).map_err(source_error)?,
            cmd::field(receipt, right).map_err(source_error)?,
        )
        .map_err(source_error)?
        {
            return Err(DurableError::Corrupt("Item receipt lineage fields differ"));
        }
    }
    let files = cmd::field(receipt, "files")
        .map_err(source_error)?
        .as_object()
        .ok_or(DurableError::Corrupt("Item native receipt member map"))?;
    let mut expected_files = BTreeMap::<String, (u64, Digest256)>::new();
    let compound_receipt_path = format!("{item_home}/edition-item-receipt.json");
    for (path, (_, bytes, digest)) in direct {
        if path != &compound_receipt_path {
            expected_files.insert(path.clone(), (*bytes, *digest));
        }
    }
    expected_files.extend(successor);
    if files.len() != expected_files.len()
        || expected_files.iter().any(|(path, (bytes, digest))| {
            let Some(entry) = receipt
                .object_get("files")
                .and_then(|map| map.object_get(path))
            else {
                return true;
            };
            cmd::exact_keys(entry, &["sha256", "bytes"]).is_err()
                || cmd::integer(entry, "bytes").ok() != Some(*bytes)
                || owner_digest(entry, "sha256").ok() != Some(*digest)
        })
    {
        return Err(DurableError::Corrupt(
            "Item native receipt member closure differs",
        ));
    }
    let receipt_before = cmd::field(receipt, "parent_before_files")
        .map_err(source_error)?
        .as_object()
        .ok_or(DurableError::Corrupt("Item receipt predecessor file map"))?;
    if receipt_before.len() != predecessor.len()
        || predecessor.iter().any(|(path, (bytes, digest))| {
            let basename = path.rsplit('/').next().unwrap_or(path);
            let Some(entry) = receipt_before
                .iter()
                .find(|(name, _)| name.as_str() == Some(basename))
                .map(|(_, entry)| entry)
            else {
                return true;
            };
            cmd::exact_keys(entry, &["sha256", "bytes"]).is_err()
                || cmd::integer(entry, "bytes").ok() != Some(*bytes)
                || owner_digest(entry, "sha256").ok() != Some(*digest)
        })
    {
        return Err(DurableError::Corrupt(
            "Item receipt predecessor member closure differs",
        ));
    }
    Ok(())
}

fn validate_historical_claim_owner(
    owner: &tos_foundation::JsonValue,
    subject: &str,
    raw_sha256: &str,
) -> DurableResult<()> {
    cmd::exact_keys(
        owner,
        &[
            "schema_version",
            "package_home",
            "source_path",
            "source_profiles",
            "stream_sha256",
            "direct_members",
            "history",
            "archives",
            "claims",
            "creation_receipt_sha256",
            "creation_receipt_command_id",
            "closure_sha256",
        ],
    )
    .map_err(source_error)?;
    let home = subject
        .strip_suffix("/historical-claims.jsonl")
        .ok_or(DurableError::Corrupt("Historical Claim owner source path"))?;
    let stream_digest = Digest256::from_hex(raw_sha256)
        .map_err(|_| DurableError::Corrupt("Historical Claim stream digest"))?;
    if cmd::text(owner, "schema_version").map_err(source_error)?
        != "tos_managed_historical_claim_owner_inventory_v1"
        || cmd::text(owner, "package_home").map_err(source_error)? != home
        || cmd::text(owner, "source_path").map_err(source_error)? != subject
        || owner_digest(owner, "stream_sha256")? != stream_digest
    {
        return Err(DurableError::Corrupt(
            "Historical Claim owner source binding",
        ));
    }
    collect_profile_map(owner)?;
    let direct = validate_owner_member_rows(
        cmd::field(owner, "direct_members").map_err(source_error)?,
        &format!("{home}/"),
        &[
            "claim-stream",
            "creation-request",
            "creation-environment",
            "creation-event",
            "creation-receipt",
            "revision-history",
            "human-form-set",
            "operational-lock",
        ],
    )?;
    if direct
        .get(subject)
        .is_none_or(|(role, _, digest)| role != "claim-stream" || *digest != stream_digest)
    {
        return Err(DurableError::Corrupt(
            "Historical Claim owner stream member absent",
        ));
    }
    for (name, role) in [
        ("source-create-request.json", "creation-request"),
        ("source-create-environment.json", "creation-environment"),
        ("source-create-provenance.jsonl", "creation-event"),
        ("source-create-receipt.json", "creation-receipt"),
    ] {
        if direct
            .get(&format!("{home}/{name}"))
            .is_none_or(|(actual, _, _)| actual != role)
        {
            return Err(DurableError::Corrupt(
                "Historical Claim creation package is incomplete",
            ));
        }
    }
    let receipt_path = format!("{home}/source-create-receipt.json");
    let receipt_digest = owner_digest(owner, "creation_receipt_sha256")?;
    if cmd::text(owner, "creation_receipt_command_id")
        .map_err(source_error)?
        .is_empty()
        || direct
            .get(&receipt_path)
            .is_none_or(|(_, _, digest)| *digest != receipt_digest)
    {
        return Err(DurableError::Corrupt(
            "Historical Claim creation receipt binding",
        ));
    }

    let history = cmd::field(owner, "history").map_err(source_error)?;
    cmd::exact_keys(history, &["ref", "present", "sha256", "receipts"]).map_err(source_error)?;
    let history_path = format!("{home}/claim-revision-history.json");
    let history_present = cmd::field(history, "present")
        .map_err(source_error)?
        .as_bool()
        .ok_or(DurableError::Corrupt("Historical Claim history presence"))?;
    let history_digest = cmd::field(history, "sha256").map_err(source_error)?;
    if cmd::text(history, "ref").map_err(source_error)? != history_path
        || history_present != direct.contains_key(&history_path)
        || (!history_present && history_digest != &tos_foundation::JsonValue::Null)
        || (history_present
            && direct.get(&history_path).is_none_or(|(role, _, digest)| {
                role != "revision-history"
                    || history_digest
                        .as_str()
                        .and_then(|value| Digest256::from_prefixed(value).ok())
                        != Some(*digest)
            }))
    {
        return Err(DurableError::Corrupt(
            "Historical Claim history member binding",
        ));
    }

    let archives = cmd::field(owner, "archives")
        .map_err(source_error)?
        .as_array()
        .ok_or(DurableError::Corrupt("Historical Claim archive rows"))?;
    let mut archive_by_path = BTreeMap::<String, String>::new();
    let mut archive_order = Vec::with_capacity(archives.len());
    for archive in archives {
        cmd::exact_keys(archive, &["path", "manifest_sha256", "revision", "members"])
            .map_err(source_error)?;
        let path = cmd::text(archive, "path").map_err(source_error)?;
        if !path.starts_with("ToS/source-witnesses/.record-revisions/")
            || RelativePath::parse(path).is_err()
        {
            return Err(DurableError::Corrupt(
                "Historical Claim archive path binding",
            ));
        }
        owner_digest(archive, "manifest_sha256")?;
        let revision = cmd::text(archive, "revision").map_err(source_error)?;
        if revision.is_empty() {
            return Err(DurableError::Corrupt(
                "Historical Claim archive revision is empty",
            ));
        }
        let members = cmd::field(archive, "members")
            .map_err(source_error)?
            .as_array()
            .ok_or(DurableError::Corrupt("Historical Claim archive members"))?;
        let mut previous_name: Option<&str> = None;
        let mut names = BTreeSet::new();
        for member in members {
            cmd::exact_keys(member, &["name", "sha256", "bytes"]).map_err(source_error)?;
            let name = cmd::text(member, "name").map_err(source_error)?;
            let bytes = cmd::integer(member, "bytes").map_err(source_error)?;
            let _ = owner_digest(member, "sha256")?;
            if name.is_empty()
                || name.contains('/')
                || name == "."
                || name == ".."
                || bytes > 8_388_608
                || previous_name.is_some_and(|prior| prior.as_bytes() >= name.as_bytes())
                || !names.insert(name.to_owned())
            {
                return Err(DurableError::Corrupt(
                    "Historical Claim archive member binding",
                ));
            }
            previous_name = Some(name);
        }
        if !names.contains("historical-claims.jsonl") {
            return Err(DurableError::Corrupt(
                "Historical Claim archive omits its stream",
            ));
        }
        if archive_by_path
            .insert(path.to_owned(), revision.to_owned())
            .is_some()
        {
            return Err(DurableError::Corrupt(
                "Historical Claim archive path is duplicated",
            ));
        }
        archive_order.push(path.to_owned());
    }

    let receipts = cmd::field(history, "receipts")
        .map_err(source_error)?
        .as_array()
        .ok_or(DurableError::Corrupt("Historical Claim history receipts"))?;
    if receipts.len() != archive_by_path.len()
        || receipts.len() > 128
        || (!history_present && !receipts.is_empty())
    {
        return Err(DurableError::Corrupt(
            "Historical Claim receipt/archive count differs",
        ));
    }
    let mut used_archives = BTreeSet::new();
    for (index, receipt) in receipts.iter().enumerate() {
        cmd::exact_keys(
            receipt,
            &[
                "command_id",
                "request_digest",
                "previous_source",
                "source",
                "previous_revision",
                "archive_path",
                "receipt_sha256",
            ],
        )
        .map_err(source_error)?;
        let command_id = cmd::text(receipt, "command_id").map_err(source_error)?;
        let archive_path = cmd::text(receipt, "archive_path").map_err(source_error)?;
        let previous_revision = cmd::text(receipt, "previous_revision").map_err(source_error)?;
        if archive_order.get(index).map(String::as_str) != Some(archive_path) {
            return Err(DurableError::Corrupt(
                "Historical Claim archive order differs from history",
            ));
        }
        let archive_revision = archive_by_path
            .get(archive_path)
            .ok_or(DurableError::Corrupt(
                "Historical Claim receipt archive is absent",
            ))?;
        let previous_source = cmd::field(receipt, "previous_source").map_err(source_error)?;
        let source = cmd::field(receipt, "source").map_err(source_error)?;
        if command_id.is_empty()
            || Digest256::from_prefixed(cmd::text(receipt, "request_digest").map_err(source_error)?)
                .is_err()
            || Digest256::from_prefixed(cmd::text(receipt, "receipt_sha256").map_err(source_error)?)
                .is_err()
            || cmd::text(previous_source, "id").map_err(source_error)?
                != cmd::text(source, "id").map_err(source_error)?
            || cmd::text(source, "id").map_err(source_error)?.is_empty()
            || previous_revision != archive_revision.as_str()
            || !used_archives.insert(archive_path.to_owned())
        {
            return Err(DurableError::Corrupt(
                "Historical Claim receipt/archive binding",
            ));
        }
    }
    if used_archives != archive_by_path.keys().cloned().collect() {
        return Err(DurableError::Corrupt(
            "Historical Claim archive closure differs",
        ));
    }

    let claims = cmd::field(owner, "claims")
        .map_err(source_error)?
        .as_array()
        .ok_or(DurableError::Corrupt("Historical Claim identity rows"))?;
    let mut previous_claim: Option<&str> = None;
    let mut claim_ids = BTreeSet::new();
    let mut form_ids = BTreeSet::new();
    let mut expected_forms = BTreeSet::new();
    for claim in claims {
        cmd::exact_keys(
            claim,
            &[
                "claim_id",
                "claim_version",
                "claim_sha256",
                "form_ref",
                "form_present",
                "form_sha256",
                "form_ids",
            ],
        )
        .map_err(source_error)?;
        let claim_id = cmd::text(claim, "claim_id").map_err(source_error)?;
        let form_hash = Digest256::of_bytes(claim_id.as_bytes()).to_hex();
        let form_path = format!("{home}/historical-claims.{form_hash}.human-forms.json");
        let form_present = cmd::field(claim, "form_present")
            .map_err(source_error)?
            .as_bool()
            .ok_or(DurableError::Corrupt("Historical Claim form presence"))?;
        let form_digest = cmd::field(claim, "form_sha256").map_err(source_error)?;
        let ids = cmd::field(claim, "form_ids")
            .map_err(source_error)?
            .as_array()
            .ok_or(DurableError::Corrupt("Historical Claim form IDs"))?;
        if claim_id.is_empty()
            || previous_claim.is_some_and(|previous| previous.as_bytes() >= claim_id.as_bytes())
            || !claim_ids.insert(claim_id.to_owned())
            || cmd::integer(claim, "claim_version").map_err(source_error)? == 0
            || (!history_present
                && cmd::integer(claim, "claim_version").map_err(source_error)? != 1)
            || Digest256::from_prefixed(cmd::text(claim, "claim_sha256").map_err(source_error)?)
                .is_err()
            || cmd::text(claim, "form_ref").map_err(source_error)? != form_path
            || form_present != direct.contains_key(&form_path)
            || (form_present
                && (form_digest == &tos_foundation::JsonValue::Null
                    || direct.get(&form_path).is_none_or(|(role, _, digest)| {
                        role != "human-form-set"
                            || form_digest
                                .as_str()
                                .and_then(|value| Digest256::from_prefixed(value).ok())
                                != Some(*digest)
                    })))
            || (!form_present
                && (form_digest != &tos_foundation::JsonValue::Null || !ids.is_empty()))
            || ids.iter().any(|id| {
                id.as_str()
                    .is_none_or(|id| id.is_empty() || !form_ids.insert(id.to_owned()))
            })
        {
            return Err(DurableError::Corrupt(
                "Historical Claim identity/form binding",
            ));
        }
        if form_present {
            expected_forms.insert(form_path);
        }
        previous_claim = Some(claim_id);
    }
    let direct_forms = direct
        .iter()
        .filter(|(_, (role, _, _))| role == "human-form-set")
        .map(|(path, _)| path.clone())
        .collect::<BTreeSet<_>>();
    if direct_forms != expected_forms {
        return Err(DurableError::Corrupt(
            "Historical Claim form package closure differs",
        ));
    }
    let mut expected_direct = BTreeSet::from([
        subject.to_owned(),
        format!("{home}/source-create-request.json"),
        format!("{home}/source-create-environment.json"),
        format!("{home}/source-create-provenance.jsonl"),
        receipt_path,
    ]);
    if history_present {
        expected_direct.insert(history_path);
    }
    expected_direct.extend(expected_forms);
    for (path, (role, _, _)) in &direct {
        if role == "operational-lock" {
            if direct.get(path).is_none_or(|(_, bytes, _)| *bytes != 0) {
                return Err(DurableError::Corrupt(
                    "Historical Claim operational lock is not empty",
                ));
            }
            let basename = path.rsplit('/').next().unwrap_or(path);
            if !direct.iter().any(|(form_path, (form_role, _, _))| {
                form_role == "human-form-set"
                    && basename
                        == format!(
                            ".{}.writer.lock",
                            form_path.rsplit('/').next().unwrap_or(form_path)
                        )
            }) {
                return Err(DurableError::Corrupt(
                    "Historical Claim orphan operational lock",
                ));
            }
            expected_direct.insert(path.clone());
        }
    }
    if direct.keys().cloned().collect::<BTreeSet<_>>() != expected_direct {
        return Err(DurableError::Corrupt(
            "Historical Claim direct package closure differs",
        ));
    }
    validate_owner_closure(owner)
}

fn validate_claim_owner(
    owner: &tos_foundation::JsonValue,
    subject: &str,
    raw_sha256: &str,
) -> DurableResult<()> {
    let schema = cmd::text(owner, "schema_version").map_err(source_error)?;
    match schema {
        "tos_managed_imported_legacy_claim_owner_inventory_v1" => {
            return validate_imported_legacy_claim_owner(owner, subject, raw_sha256);
        }
        "tos_managed_historical_claim_owner_inventory_v1" => {
            return validate_historical_claim_owner(owner, subject, raw_sha256);
        }
        "tos_managed_item_adoption_claim_owner_inventory_v1" => {
            return validate_item_claim_owner(owner, subject, raw_sha256);
        }
        "tos_managed_item_adoption_claim_owner_inventory_v2" => {
            return validate_item_claim_owner_v2(owner, subject, raw_sha256);
        }
        "tos_managed_collection_membership_claim_owner_inventory_v1" => {
            return validate_growth_claim_owner(
                owner,
                subject,
                raw_sha256,
                schema,
                &[
                    "claim-stream",
                    "creation-request",
                    "creation-environment",
                    "creation-event",
                    "creation-receipt",
                    "human-form-set",
                ],
                1,
            );
        }
        "tos_managed_object_link_claim_owner_inventory_v1"
        | "tos_managed_work_expression_claim_owner_inventory_v1"
        | "tos_managed_expression_edition_claim_owner_inventory_v1"
        | "tos_managed_expression_responsibility_claim_owner_inventory_v1" => {
            return validate_growth_claim_owner(
                owner,
                subject,
                raw_sha256,
                schema,
                &[
                    "claim-stream",
                    "creation-request",
                    "creation-environment",
                    "creation-event",
                    "creation-receipt",
                    "revision-history",
                    "human-form-set",
                ],
                1,
            );
        }
        _ => {}
    }
    cmd::exact_keys(
        owner,
        &[
            "schema_version",
            "package_home",
            "source_path",
            "stream_sha256",
            "direct_members",
            "history",
            "archives",
            "claims",
            "creation_receipt_sha256",
            "creation_receipt_command_id",
            "closure_sha256",
        ],
    )
    .map_err(source_error)?;
    let home = subject
        .strip_suffix(&format!("/{}", crate::source_claims::CLAIM_STREAM))
        .ok_or(DurableError::Corrupt("Claim owner source path"))?;
    let expected_stream_digest = Digest256::from_hex(raw_sha256)
        .map_err(|_| DurableError::Corrupt("Claim stream projection digest"))?;
    if cmd::text(owner, "schema_version").map_err(source_error)?
        != "tos_managed_global_claim_owner_inventory_v1"
        || cmd::text(owner, "package_home").map_err(source_error)? != home
        || cmd::text(owner, "source_path").map_err(source_error)? != subject
        || owner_digest(owner, "stream_sha256")? != expected_stream_digest
    {
        return Err(DurableError::Corrupt("Claim owner source binding"));
    }
    let direct = validate_owner_member_rows(
        cmd::field(owner, "direct_members").map_err(source_error)?,
        &format!("{home}/"),
        &[
            "claim-stream",
            "creation-request",
            "creation-environment",
            "creation-event",
            "creation-receipt",
            "revision-history",
            "human-form-set",
        ],
    )?;
    if direct
        .get(subject)
        .is_none_or(|(role, _, digest)| role != "claim-stream" || *digest != expected_stream_digest)
    {
        return Err(DurableError::Corrupt("Claim owner stream member absent"));
    }
    for (name, role) in [
        ("source-create-request.json", "creation-request"),
        ("source-create-environment.json", "creation-environment"),
        ("source-create-provenance.jsonl", "creation-event"),
        ("source-create-receipt.json", "creation-receipt"),
    ] {
        if direct
            .get(&format!("{home}/{name}"))
            .map(|(actual, _, _)| actual.as_str())
            != Some(role)
        {
            return Err(DurableError::Corrupt("Claim owner creation member absent"));
        }
    }
    let receipt_path = format!("{home}/source-create-receipt.json");
    let expected_receipt_digest = owner_digest(owner, "creation_receipt_sha256")?;
    if cmd::text(owner, "creation_receipt_command_id")
        .map_err(source_error)?
        .is_empty()
        || direct
            .get(&receipt_path)
            .is_none_or(|(_, _, digest)| *digest != expected_receipt_digest)
    {
        return Err(DurableError::Corrupt("Claim owner receipt binding"));
    }
    let history = cmd::field(owner, "history").map_err(source_error)?;
    cmd::exact_keys(history, &["ref", "present", "sha256", "receipts"]).map_err(source_error)?;
    let history_path = format!("{home}/{}", crate::source_claims::CLAIM_HISTORY);
    let history_present = cmd::field(history, "present")
        .map_err(source_error)?
        .as_bool()
        .ok_or(DurableError::Corrupt("Claim history presence flag"))?;
    let history_digest = cmd::field(history, "sha256").map_err(source_error)?;
    if cmd::text(history, "ref").map_err(source_error)? != history_path
        || history_present != direct.contains_key(&history_path)
        || (history_present
            && (history_digest == &tos_foundation::JsonValue::Null
                || Digest256::from_prefixed(cmd::text(history, "sha256").map_err(source_error)?)
                    .is_err()))
        || (!history_present && history_digest != &tos_foundation::JsonValue::Null)
        || (history_present
            && direct.get(&history_path).is_none_or(|(role, _, digest)| {
                role != "revision-history"
                    || history_digest
                        .as_str()
                        .and_then(|value| Digest256::from_prefixed(value).ok())
                        != Some(*digest)
            }))
    {
        return Err(DurableError::Corrupt("Claim history membership binding"));
    }
    let archives = cmd::field(owner, "archives")
        .map_err(source_error)?
        .as_array()
        .ok_or(DurableError::Corrupt("Claim archive rows"))?;
    let mut archive_by_path = BTreeMap::new();
    let mut previous_archive: Option<&str> = None;
    for archive in archives {
        cmd::exact_keys(archive, &["archive_path", "manifest_sha256", "members"])
            .map_err(source_error)?;
        let path = cmd::text(archive, "archive_path").map_err(source_error)?;
        if !path.starts_with("ToS/source-witnesses/.record-revisions/")
            || previous_archive.is_some_and(|prior| prior.as_bytes() >= path.as_bytes())
        {
            return Err(DurableError::Corrupt("Claim archive path ordering"));
        }
        previous_archive = Some(path);
        let members = validate_owner_member_rows(
            cmd::field(archive, "members").map_err(source_error)?,
            &format!("{path}/"),
            &["archive-manifest", "archive-blob"],
        )?;
        if members
            .get(&format!("{path}/manifest.json"))
            .map(|(role, _, _)| role.as_str())
            != Some("archive-manifest")
        {
            return Err(DurableError::Corrupt("Claim archive manifest row absent"));
        }
        let manifest_digest = owner_digest(archive, "manifest_sha256")?;
        if members
            .get(&format!("{path}/manifest.json"))
            .is_none_or(|(_, _, digest)| *digest != manifest_digest)
        {
            return Err(DurableError::Corrupt(
                "Claim archive manifest digest differs",
            ));
        }
        archive_by_path.insert(path.to_owned(), manifest_digest);
    }
    let receipts = cmd::field(history, "receipts")
        .map_err(source_error)?
        .as_array()
        .ok_or(DurableError::Corrupt("Claim history receipt rows"))?;
    let mut used_archives = BTreeSet::new();
    for receipt in receipts {
        cmd::exact_keys(
            receipt,
            &[
                "command_id",
                "claim_id",
                "previous_revision",
                "archive_path",
                "archive_manifest_sha256",
                "forms",
            ],
        )
        .map_err(source_error)?;
        let archive_path = cmd::text(receipt, "archive_path").map_err(source_error)?;
        let digest = archive_by_path
            .get(archive_path)
            .ok_or(DurableError::Corrupt("Claim history archive row absent"))?;
        if cmd::text(receipt, "command_id")
            .map_err(source_error)?
            .is_empty()
            || cmd::text(receipt, "claim_id")
                .map_err(source_error)?
                .is_empty()
            || digest != &owner_digest(receipt, "archive_manifest_sha256")?
            || !used_archives.insert(archive_path.to_owned())
        {
            return Err(DurableError::Corrupt("Claim history archive binding"));
        }
    }
    if used_archives != archive_by_path.keys().cloned().collect() {
        return Err(DurableError::Corrupt(
            "Claim history archive closure differs",
        ));
    }
    let claims = cmd::field(owner, "claims")
        .map_err(source_error)?
        .as_array()
        .ok_or(DurableError::Corrupt("Claim owner identity rows"))?;
    let mut previous_claim: Option<&str> = None;
    let mut claim_ids = BTreeSet::new();
    for claim in claims {
        cmd::exact_keys(
            claim,
            &[
                "claim_id",
                "claim_version",
                "claim_sha256",
                "form_ref",
                "form_present",
                "form_sha256",
                "form_ids",
            ],
        )
        .map_err(source_error)?;
        let claim_id = cmd::text(claim, "claim_id").map_err(source_error)?;
        let hash = tos_foundation::Digest256::of_bytes(claim_id.as_bytes())
            .to_prefixed()
            .trim_start_matches("sha256:")
            .to_owned();
        let form_path = format!("{home}/source-claims.{hash}.human-forms.json");
        let form_present = cmd::field(claim, "form_present")
            .map_err(source_error)?
            .as_bool()
            .ok_or(DurableError::Corrupt("Claim form presence flag"))?;
        let form_sha = cmd::field(claim, "form_sha256").map_err(source_error)?;
        let expected_form_digest = if form_present {
            Some(owner_digest(claim, "form_sha256")?)
        } else {
            None
        };
        let ids = cmd::field(claim, "form_ids")
            .map_err(source_error)?
            .as_array()
            .ok_or(DurableError::Corrupt("Claim owner form ID rows"))?;
        if claim_id.is_empty()
            || previous_claim.is_some_and(|previous| previous.as_bytes() >= claim_id.as_bytes())
            || !claim_ids.insert(claim_id.to_owned())
            || cmd::integer(claim, "claim_version").map_err(source_error)? == 0
            || Digest256::from_prefixed(cmd::text(claim, "claim_sha256").map_err(source_error)?)
                .is_err()
            || cmd::text(claim, "form_ref").map_err(source_error)? != form_path
            || form_present != direct.contains_key(&form_path)
            || (form_present
                && (form_sha == &tos_foundation::JsonValue::Null
                    || direct.get(&form_path).is_none_or(|(role, _, digest)| {
                        role != "human-form-set" || expected_form_digest != Some(*digest)
                    })))
            || (!form_present && (form_sha != &tos_foundation::JsonValue::Null || !ids.is_empty()))
            || ids.iter().any(|id| id.as_str().is_none_or(str::is_empty))
        {
            return Err(DurableError::Corrupt("Claim owner identity/form binding"));
        }
        previous_claim = Some(claim_id);
    }
    let direct_forms = direct
        .iter()
        .filter(|(_, (role, _, _))| role == "human-form-set")
        .map(|(path, _)| path.clone())
        .collect::<BTreeSet<_>>();
    let claim_forms = claims
        .iter()
        .filter_map(|claim| {
            (cmd::field(claim, "form_present").ok()?.as_bool() == Some(true))
                .then(|| cmd::text(claim, "form_ref").ok().map(str::to_owned))
                .flatten()
        })
        .collect::<BTreeSet<_>>();
    if direct_forms != claim_forms {
        return Err(DurableError::Corrupt("Claim owner form membership differs"));
    }
    let mut expected_direct = BTreeSet::from([
        subject.to_owned(),
        format!("{home}/source-create-request.json"),
        format!("{home}/source-create-environment.json"),
        format!("{home}/source-create-provenance.jsonl"),
        format!("{home}/source-create-receipt.json"),
    ]);
    if history_present {
        expected_direct.insert(history_path);
    }
    expected_direct.extend(claim_forms);
    expected_direct.extend(
        direct
            .iter()
            .filter(|(_, (role, _, _))| role == "operational-lock")
            .map(|(path, _)| path.clone()),
    );
    if direct.keys().cloned().collect::<BTreeSet<_>>() != expected_direct {
        return Err(DurableError::Corrupt(
            "Claim owner direct package closure differs",
        ));
    }
    let mut core = owner
        .as_object()
        .ok_or(DurableError::Corrupt("Claim owner object"))?
        .iter()
        .filter(|(key, _)| key.as_str() != Some("closure_sha256"))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<Vec<_>>();
    // Canonical JSON sorts keys, so preserve a stable object regardless of
    // insertion order when verifying the owner closure root.
    core.sort_by(|left, right| left.0.as_str().cmp(&right.0.as_str()));
    let core = tos_foundation::JsonValue::Object(core);
    if cmd::record_digest(&core)
        .map_err(source_error)?
        .to_prefixed()
        != cmd::text(owner, "closure_sha256").map_err(source_error)?
    {
        return Err(DurableError::Corrupt("Claim owner closure root differs"));
    }
    let _ = raw_sha256;
    Ok(())
}

fn validate_imported_legacy_claim_owner(
    owner: &tos_foundation::JsonValue,
    subject: &str,
    raw_sha256: &str,
) -> DurableResult<()> {
    cmd::exact_keys(
        owner,
        &[
            "schema_version",
            "source_path",
            "owner_mode",
            "stream_sha256",
            "form_refs",
            "claim_history_ref",
            "claims",
            "closure_sha256",
        ],
    )
    .map_err(source_error)?;
    let stream_digest = Digest256::from_hex(raw_sha256)
        .map_err(|_| DurableError::Corrupt("imported legacy Claim stream digest"))?;
    if !crate::source_claims::is_imported_legacy_claim_path(subject)
        || cmd::text(owner, "schema_version").map_err(source_error)?
            != "tos_managed_imported_legacy_claim_owner_inventory_v1"
        || cmd::text(owner, "source_path").map_err(source_error)? != subject
        || cmd::text(owner, "owner_mode").map_err(source_error)? != "imported_read_only"
        || owner_digest(owner, "stream_sha256")? != stream_digest
        || cmd::field(owner, "form_refs")
            .map_err(source_error)?
            .as_array()
            .is_none_or(|forms| !forms.is_empty())
        || cmd::field(owner, "claim_history_ref").map_err(source_error)?
            != &tos_foundation::JsonValue::Null
    {
        return Err(DurableError::Corrupt(
            "imported legacy Claim owner source or sidecar binding",
        ));
    }
    let claims = cmd::field(owner, "claims")
        .map_err(source_error)?
        .as_array()
        .ok_or(DurableError::Corrupt("imported legacy Claim rows"))?;
    let mut previous: Option<&str> = None;
    for row in claims {
        cmd::exact_keys(
            row,
            &[
                "claim_id",
                "source_claim_line",
                "claim_sha256",
                "catalogue_entry",
            ],
        )
        .map_err(source_error)?;
        let claim_id = cmd::text(row, "claim_id").map_err(source_error)?;
        let line = cmd::integer(row, "source_claim_line").map_err(source_error)?;
        let digest = owner_digest(row, "claim_sha256")?;
        let entry = cmd::field(row, "catalogue_entry").map_err(source_error)?;
        if claim_id.is_empty()
            || line == 0
            || previous.is_some_and(|prior| prior.as_bytes() >= claim_id.as_bytes())
            || cmd::text(entry, "schema_version").map_err(source_error)?
                != "tos_source_witness_claim_catalog_entry_v1"
            || cmd::text(entry, "source_claim_file_ref").map_err(source_error)? != subject
            || cmd::integer(entry, "source_claim_line").map_err(source_error)? != line
            || cmd::text(entry, "claim_id").map_err(source_error)? != claim_id
            || cmd::text(entry, "claim_sha256").map_err(source_error)? != digest.to_hex()
            || !matches!(
                cmd::text(entry, "visibility").map_err(source_error)?,
                "public" | "public_metadata_only"
            )
        {
            return Err(DurableError::Corrupt(
                "imported legacy Claim row binding differs",
            ));
        }
        previous = Some(claim_id);
    }
    validate_owner_closure(owner)
}

fn validate_nonprojecting_source_custody(
    owner: &tos_foundation::JsonValue,
    subject: &str,
) -> DurableResult<()> {
    cmd::exact_keys(
        owner,
        &[
            "schema_version",
            "source_path",
            "custody_kind",
            "content_projected",
        ],
    )
    .map_err(source_error)?;
    let expected_kind = crate::source_claims::nonprojecting_custody_kind(subject)
        .ok_or(DurableError::Corrupt("source custody path classification"))?;
    if cmd::text(owner, "schema_version").map_err(source_error)?
        != "tos_managed_nonprojecting_source_custody_v1"
        || cmd::text(owner, "source_path").map_err(source_error)? != subject
        || cmd::text(owner, "custody_kind").map_err(source_error)? != expected_kind
        || cmd::field(owner, "content_projected").map_err(source_error)?
            != &tos_foundation::JsonValue::Bool(false)
    {
        return Err(DurableError::Corrupt(
            "nonprojecting source custody marker differs",
        ));
    }
    Ok(())
}

fn validate_public_text_owner(
    owner: &tos_foundation::JsonValue,
    subject: &str,
    raw_sha256: &str,
) -> DurableResult<()> {
    if owner == &tos_foundation::JsonValue::Null {
        // Shared package names (including source-create-*.json) do not select
        // the public Text family. Original and other owned source packages
        // legitimately have no public Text contribution. The typed revision
        // boundary requires this contribution for every public Text write;
        // any contribution present here still has its full closure checked.
        return Ok(());
    }
    cmd::exact_keys(
        owner,
        &[
            "schema_version",
            "package_home",
            "member_ref",
            "member_role",
            "member_sha256",
            "package_manifest_ref",
            "package_manifest_sha256",
            "package_members",
            "layer_ref",
            "layer_sha256",
            "layer_id",
            "packet_ref",
            "packet_sha256",
            "packet_id",
            "bindings_ref",
            "bindings_sha256",
        ],
    )
    .map_err(source_error)?;
    let home = cmd::text(owner, "package_home").map_err(source_error)?;
    let member_ref = cmd::text(owner, "member_ref").map_err(source_error)?;
    let role = cmd::text(owner, "member_role").map_err(source_error)?;
    let expected_role = subject
        .strip_prefix(&format!("{home}/"))
        .and_then(crate::source_public_text_entry::public_text_member_role);
    if cmd::text(owner, "schema_version").map_err(source_error)?
        != "tos_managed_public_text_inventory_member_v1"
        || RelativePath::parse(home)
            .map(|path| path.as_str() != home)
            .unwrap_or(true)
        || !home.starts_with("ToS/source-witnesses/")
        || home.split('/').any(|part| part == "owner-local")
        || !member_ref.starts_with(&format!("{home}/"))
        || member_ref != subject
        || expected_role != Some(role)
        || owner_digest(owner, "member_sha256")?
            != Digest256::from_hex(raw_sha256)
                .map_err(|_| DurableError::Corrupt("public Text member projection digest"))?
        || cmd::text(owner, "package_manifest_ref").map_err(source_error)?
            != format!("{home}/source-create-receipt.json")
    {
        return Err(DurableError::Corrupt("public Text member owner binding"));
    }
    let members = cmd::field(owner, "package_members")
        .map_err(source_error)?
        .as_array()
        .ok_or(DurableError::Corrupt("public Text package member rows"))?;
    if members.len() != 12 {
        return Err(DurableError::Corrupt("public Text package closure count"));
    }
    let mut expected_refs = crate::source_public_text_entry::FILES
        .iter()
        .map(|name| format!("{home}/{name}"))
        .collect::<BTreeSet<_>>();
    if expected_refs.len() != 12 {
        return Err(DurableError::Corrupt("public Text package file set"));
    }
    let mut previous: Option<&str> = None;
    let mut refs = BTreeMap::new();
    for member in members {
        cmd::exact_keys(member, &["ref", "sha256"]).map_err(source_error)?;
        let path = cmd::text(member, "ref").map_err(source_error)?;
        let digest = Digest256::from_prefixed(cmd::text(member, "sha256").map_err(source_error)?)
            .map_err(|_| DurableError::Corrupt("public Text package member digest"))?;
        if !path.starts_with(&format!("{home}/"))
            || RelativePath::parse(path)
                .map(|parsed| parsed.as_str() != path)
                .unwrap_or(true)
            || previous.is_some_and(|prior| prior.as_bytes() >= path.as_bytes())
            || !expected_refs.remove(path)
            || refs.insert(path.to_owned(), digest).is_some()
        {
            return Err(DurableError::Corrupt("public Text package member binding"));
        }
        previous = Some(path);
    }
    let manifest_ref = cmd::text(owner, "package_manifest_ref").map_err(source_error)?;
    let layer_ref = cmd::text(owner, "layer_ref").map_err(source_error)?;
    let packet_ref = cmd::text(owner, "packet_ref").map_err(source_error)?;
    let bindings_ref = cmd::text(owner, "bindings_ref").map_err(source_error)?;
    let member_digest = |name: &str| -> DurableResult<Digest256> {
        refs.get(&format!("{home}/{name}"))
            .copied()
            .ok_or(DurableError::Corrupt(
                "public Text package dependency absent",
            ))
    };
    if !expected_refs.is_empty()
        || refs.get(subject)
            != Some(
                &Digest256::from_hex(raw_sha256)
                    .map_err(|_| DurableError::Corrupt("public Text member projection digest"))?,
            )
        || manifest_ref != format!("{home}/source-create-receipt.json")
        || layer_ref != format!("{home}/source-text-layer.v1.json")
        || packet_ref != format!("{home}/source-text-unit.v1.json")
        || bindings_ref != format!("{home}/native-bindings.json")
        || owner_digest(owner, "package_manifest_sha256")?
            != member_digest("source-create-receipt.json")?
        || owner_digest(owner, "layer_sha256")? != member_digest("source-text-layer.v1.json")?
        || owner_digest(owner, "packet_sha256")? != member_digest("source-text-unit.v1.json")?
        || owner_digest(owner, "bindings_sha256")? != member_digest("native-bindings.json")?
        || cmd::text(owner, "layer_id")
            .map_err(source_error)?
            .is_empty()
        || cmd::text(owner, "packet_id")
            .map_err(source_error)?
            .is_empty()
    {
        return Err(DurableError::Corrupt("public Text package closure binding"));
    }
    Ok(())
}

fn collect_profiles(
    value: &tos_foundation::JsonValue,
    profiles: &mut BTreeMap<String, Digest256>,
) -> DurableResult<()> {
    let source_profiles = cmd::field(value, "source_profiles").map_err(source_error)?;
    let source_profiles = source_profiles
        .as_object()
        .ok_or(DurableError::Corrupt("Agent source profile map"))?;
    for (path, value) in source_profiles {
        let path = path
            .as_str()
            .ok_or(DurableError::Corrupt("Agent profile locator"))?;
        RelativePath::parse(path).map_err(|_| DurableError::Corrupt("Agent profile path"))?;
        let text = value
            .as_str()
            .ok_or(DurableError::Corrupt("Agent profile digest"))?;
        let digest =
            Digest256::from_hex(text).map_err(|_| DurableError::Corrupt("Agent profile digest"))?;
        if profiles.get(path).is_some_and(|prior| prior != &digest) {
            return Err(DurableError::Corrupt(
                "Agent source profile contributions disagree",
            ));
        }
        if !profiles.contains_key(path) && profiles.len() as u64 >= MAX_PROFILE_COUNT {
            return Err(DurableError::Refused(
                "Agent source profile union exceeds selected file bound",
            ));
        }
        profiles.insert(path.to_owned(), digest);
    }
    Ok(())
}

fn encode_inventory_descriptors(
    magic: &[u8],
    descriptors: &[&AuthenticatedTreeDescriptorV2],
) -> DurableResult<Vec<u8>> {
    let encoded = descriptors
        .iter()
        .map(|descriptor| {
            descriptor
                .encode(MAX_INVENTORY_DESCRIPTOR_BYTES)
                .map_err(tree_error)
        })
        .collect::<DurableResult<Vec<_>>>()?;
    let total = encoded.iter().try_fold(magic.len(), |total, field| {
        total
            .checked_add(4)
            .and_then(|size| size.checked_add(field.len()))
            .filter(|size| *size <= MAX_INVENTORY_DESCRIPTOR_BYTES)
            .ok_or(DurableError::Refused(
                "inventory descriptor exceeds byte bound",
            ))
    })?;
    let mut output = Vec::with_capacity(total);
    output.extend_from_slice(magic);
    for field in encoded {
        append_bytes(&mut output, &field)?;
    }
    Ok(output)
}

fn decode_inventory_descriptors(
    raw: &[u8],
    magic: &[u8],
    field_count: usize,
) -> DurableResult<Vec<AuthenticatedTreeDescriptorV2>> {
    if raw.len() > MAX_INVENTORY_DESCRIPTOR_BYTES || !raw.starts_with(magic) {
        return Err(DurableError::Corrupt("inventory descriptor version/size"));
    }
    let mut offset = magic.len();
    let mut descriptors = Vec::with_capacity(field_count);
    for _ in 0..field_count {
        let field = take_bytes(raw, &mut offset)?;
        descriptors.push(
            AuthenticatedTreeDescriptorV2::decode(field, MAX_INVENTORY_DESCRIPTOR_BYTES)
                .map_err(tree_error)?,
        );
    }
    if offset != raw.len() {
        return Err(DurableError::Corrupt("inventory descriptor trailing bytes"));
    }
    Ok(descriptors)
}

fn append_bytes(output: &mut Vec<u8>, value: &[u8]) -> DurableResult<()> {
    let length = u32::try_from(value.len())
        .map_err(|_| DurableError::Refused("Agent inventory descriptor field too large"))?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(value);
    Ok(())
}

fn take_bytes<'a>(raw: &'a [u8], offset: &mut usize) -> DurableResult<&'a [u8]> {
    let header_end = offset
        .checked_add(4)
        .filter(|end| *end <= raw.len())
        .ok_or(DurableError::Corrupt(
            "Agent inventory descriptor truncated",
        ))?;
    let length = u32::from_be_bytes(raw[*offset..header_end].try_into().unwrap()) as usize;
    let end = header_end
        .checked_add(length)
        .filter(|end| *end <= raw.len())
        .ok_or(DurableError::Corrupt(
            "Agent inventory descriptor truncated",
        ))?;
    *offset = end;
    Ok(&raw[header_end..end])
}

fn add_tree_work(
    total: &mut AuthenticatedTreeWorkV1,
    operation: AuthenticatedTreeWorkV1,
) -> DurableResult<()> {
    let read_nodes =
        total
            .read_nodes
            .checked_add(operation.read_nodes)
            .ok_or(DurableError::Refused(
                "authenticated tree work counter overflow",
            ))?;
    let read_bytes =
        total
            .read_bytes
            .checked_add(operation.read_bytes)
            .ok_or(DurableError::Refused(
                "authenticated tree work counter overflow",
            ))?;
    let written_nodes = total
        .written_nodes
        .checked_add(operation.written_nodes)
        .ok_or(DurableError::Refused(
            "authenticated tree work counter overflow",
        ))?;
    let written_bytes = total
        .written_bytes
        .checked_add(operation.written_bytes)
        .ok_or(DurableError::Refused(
            "authenticated tree work counter overflow",
        ))?;
    total.read_nodes = read_nodes;
    total.read_bytes = read_bytes;
    total.written_nodes = written_nodes;
    total.written_bytes = written_bytes;
    Ok(())
}

fn tree_error(error: SegmentError) -> DurableError {
    match error.code {
        SegmentErrorCode::BudgetExceeded => {
            DurableError::Refused("authenticated Agent inventory tree budget")
        }
        SegmentErrorCode::Cancelled | SegmentErrorCode::DeadlineExceeded => {
            DurableError::Refused("source operation deadline or cancellation")
        }
        SegmentErrorCode::PinConflict => {
            DurableError::Conflict("authenticated Agent inventory tree custody differs")
        }
        _ => DurableError::Corrupt("authenticated Agent inventory tree is invalid"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn historical_member(path: &str, role: &str, raw: &[u8]) -> tos_foundation::JsonValue {
        let digest = Digest256::of_bytes(raw).to_prefixed();
        cmd::object(vec![
            ("ref", cmd::string(path)),
            ("role", cmd::string(role)),
            ("sha256", cmd::string(&digest)),
            ("bytes", cmd::number(raw.len() as u64)),
        ])
    }

    fn seal_historical_owner(mut owner: tos_foundation::JsonValue) -> tos_foundation::JsonValue {
        let core = owner
            .as_object()
            .unwrap()
            .iter()
            .filter(|(key, _)| key.as_str() != Some("closure_sha256"))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<Vec<_>>();
        let digest = cmd::record_digest(&tos_foundation::JsonValue::Object(core))
            .unwrap()
            .to_prefixed();
        cmd::set(&mut owner, "closure_sha256", cmd::string(&digest)).unwrap();
        owner
    }

    fn historical_owner_fixture(
        claim_version: u64,
        history_raw: Option<&[u8]>,
        archived_stream_name: &str,
    ) -> (String, String, tos_foundation::JsonValue) {
        let home = "ToS/source-witnesses/history/synthetic/episode";
        let source_path = format!("{home}/historical-claims.jsonl");
        let stream = if claim_version == 1 {
            b"{\"record_version\":1}\n".as_slice()
        } else {
            b"{\"record_version\":2}\n".as_slice()
        };
        let request = b"{}\n";
        let environment = b"{}\n";
        let event = b"{}\n";
        let creation_receipt = b"{}\n";
        let history_path = format!("{home}/claim-revision-history.json");
        let archive_path = "ToS/source-witnesses/.record-revisions/synthetic-historical-claim-r1";
        let claim_id = "tos.claim.synthetic.historical-example";
        let claim_hash =
            Digest256::of_bytes(format!("claim-version-{claim_version}").as_bytes()).to_prefixed();
        let form_hash = Digest256::of_bytes(claim_id.as_bytes()).to_hex();
        let form_path = format!("{home}/historical-claims.{form_hash}.human-forms.json");
        let mut direct_members = vec![
            historical_member(&source_path, "claim-stream", stream),
            historical_member(
                &format!("{home}/source-create-request.json"),
                "creation-request",
                request,
            ),
            historical_member(
                &format!("{home}/source-create-environment.json"),
                "creation-environment",
                environment,
            ),
            historical_member(
                &format!("{home}/source-create-provenance.jsonl"),
                "creation-event",
                event,
            ),
            historical_member(
                &format!("{home}/source-create-receipt.json"),
                "creation-receipt",
                creation_receipt,
            ),
        ];
        if let Some(history_raw) = history_raw {
            direct_members.push(historical_member(
                &history_path,
                "revision-history",
                history_raw,
            ));
        }
        direct_members.sort_by(|left, right| {
            cmd::text(left, "ref")
                .unwrap()
                .as_bytes()
                .cmp(cmd::text(right, "ref").unwrap().as_bytes())
        });

        let archives = if history_raw.is_some() {
            let archived_stream = b"{\"record_version\":1}\n";
            let archived_digest = Digest256::of_bytes(archived_stream).to_prefixed();
            let manifest_digest = Digest256::of_bytes(b"archive manifest").to_prefixed();
            tos_foundation::JsonValue::Array(vec![cmd::object(vec![
                ("path", cmd::string(archive_path)),
                ("manifest_sha256", cmd::string(&manifest_digest)),
                ("revision", cmd::string("1")),
                (
                    "members",
                    tos_foundation::JsonValue::Array(vec![cmd::object(vec![
                        ("name", cmd::string(archived_stream_name)),
                        ("sha256", cmd::string(&archived_digest)),
                        ("bytes", cmd::number(archived_stream.len() as u64)),
                    ])]),
                ),
            ])])
        } else {
            tos_foundation::JsonValue::Array(Vec::new())
        };
        let receipts = history_raw.map_or_else(Vec::new, |_| {
            let request_digest = Digest256::of_bytes(b"revision request").to_prefixed();
            let receipt_digest = Digest256::of_bytes(b"revision receipt").to_prefixed();
            vec![cmd::object(vec![
                ("command_id", cmd::string("synthetic-historical-revision")),
                ("request_digest", cmd::string(&request_digest)),
                (
                    "previous_source",
                    cmd::object(vec![("id", cmd::string(claim_id))]),
                ),
                ("source", cmd::object(vec![("id", cmd::string(claim_id))])),
                ("previous_revision", cmd::string("1")),
                ("archive_path", cmd::string(archive_path)),
                ("receipt_sha256", cmd::string(&receipt_digest)),
            ])]
        });
        let history_present = history_raw.is_some();
        let history_digest = history_raw
            .map(|raw| cmd::string(&Digest256::of_bytes(raw).to_prefixed()))
            .unwrap_or(tos_foundation::JsonValue::Null);
        let history = cmd::object(vec![
            ("ref", cmd::string(&history_path)),
            ("present", tos_foundation::JsonValue::Bool(history_present)),
            ("sha256", history_digest),
            ("receipts", tos_foundation::JsonValue::Array(receipts)),
        ]);
        let claim = cmd::object(vec![
            ("claim_id", cmd::string(claim_id)),
            ("claim_version", cmd::number(claim_version)),
            ("claim_sha256", cmd::string(&claim_hash)),
            ("form_ref", cmd::string(&form_path)),
            ("form_present", tos_foundation::JsonValue::Bool(false)),
            ("form_sha256", tos_foundation::JsonValue::Null),
            ("form_ids", tos_foundation::JsonValue::Array(Vec::new())),
        ]);
        let stream_digest = Digest256::of_bytes(stream).to_hex();
        let creation_receipt_digest = Digest256::of_bytes(creation_receipt).to_prefixed();
        let owner = cmd::object(vec![
            (
                "schema_version",
                cmd::string("tos_managed_historical_claim_owner_inventory_v1"),
            ),
            ("package_home", cmd::string(home)),
            ("source_path", cmd::string(&source_path)),
            ("source_profiles", cmd::object(Vec::new())),
            (
                "stream_sha256",
                cmd::string(&Digest256::of_bytes(stream).to_prefixed()),
            ),
            (
                "direct_members",
                tos_foundation::JsonValue::Array(direct_members),
            ),
            ("history", history),
            ("archives", archives),
            ("claims", tos_foundation::JsonValue::Array(vec![claim])),
            (
                "creation_receipt_sha256",
                cmd::string(&creation_receipt_digest),
            ),
            (
                "creation_receipt_command_id",
                cmd::string("synthetic-historical-create"),
            ),
        ]);
        (source_path, stream_digest, seal_historical_owner(owner))
    }

    fn profiled_projection(kind: &str) -> (String, Vec<u8>, tos_foundation::JsonValue) {
        let path = format!("ToS/source-witnesses/profiles/example/{kind}.json");
        let schema = "ToS/contracts/fixture-profile.schema.json";
        let record = cmd::object(vec![
            ("record_type", cmd::string(kind)),
            ("record_id", cmd::string(&format!("tos.{kind}.example"))),
        ]);
        // Formatting changes raw source bytes, but not the catalog's canonical
        // record digest. Both bindings have to survive the transport.
        let mut source = cmd::canonical(&record).unwrap();
        source.push(b'\n');
        let row =
            crate::source_claims::pg_catalogue_record_value(&record, &path, Some(schema)).unwrap();
        let mut records = cmd::object(
            crate::source_claims::MIXED_INVENTORY_RECORD_KINDS
                .iter()
                .map(|kind| (*kind, tos_foundation::JsonValue::Array(Vec::new())))
                .collect(),
        );
        cmd::set(
            &mut records,
            kind,
            tos_foundation::JsonValue::Array(vec![row]),
        )
        .unwrap();
        let profiles = cmd::object(
            MIXED_AGENT_BASELINE
                .into_iter()
                .chain([schema])
                .map(|path| {
                    (
                        path,
                        cmd::string(&Digest256::of_bytes(path.as_bytes()).to_hex()),
                    )
                })
                .collect(),
        );
        let projection = cmd::object(vec![
            (
                "schema_version",
                cmd::string(tos_compiler::managed_source::MANAGED_MIXED_INVENTORY_MEMBER_SCHEMA_V2),
            ),
            ("path", cmd::string(&path)),
            (
                "raw_sha256",
                cmd::string(&Digest256::of_bytes(&source).to_hex()),
            ),
            ("records", records),
            ("source_profiles", profiles),
            ("events", cmd::object(Vec::new())),
            ("anchors", cmd::object(Vec::new())),
            ("form", tos_foundation::JsonValue::Null),
            ("claim_owner", tos_foundation::JsonValue::Null),
            ("public_text_owner", tos_foundation::JsonValue::Null),
        ]);
        (path, source, projection)
    }

    #[test]
    fn profile_record_rows_survive_cold_and_revision_projection_transport() {
        for kind in ["community", "intellectual-movement", "fixture-new-profile"] {
            let (path, source, projection) = profiled_projection(kind);
            let raw = cmd::canonical(&projection).unwrap();
            let digest = Digest256::of_bytes(&source).to_hex();
            let parsed =
                AddressedMixedInventoryV1::checked_projection(&path, &raw, &digest).unwrap();
            // JsonValue keeps insertion order; compare the complete canonical
            // transport bytes rather than the in-memory object construction order.
            assert_eq!(cmd::canonical(&parsed).unwrap(), raw);
            let contribution = cmd::object(
                [
                    "records",
                    "source_profiles",
                    "events",
                    "anchors",
                    "form",
                    "claim_owner",
                ]
                .into_iter()
                .map(|key| (key, cmd::field(&projection, key).unwrap().clone()))
                .chain([(
                    "schema_version",
                    cmd::string("tos_pg_source_inventory_member_contribution_v1"),
                )])
                .collect(),
            );
            let writes = [SourceRevisionWrite {
                subject: path.clone(),
                expected_predecessor: None,
                proposed_revision: 1,
                exact_bytes: source,
            }];
            // This exercises the owner's envelope adapter, not registry or
            // semantic admission, which precedes these sealed contributions.
            let normalized = super::super::revision_owner_projections(
                "tos_local_native_metadata_revision_owner_v1:fixture",
                &writes,
                BTreeMap::from([(path.clone(), contribution)]),
            )
            .unwrap();
            assert_eq!(normalized.get(&path), Some(&raw));
        }
    }

    #[test]
    fn profile_record_transport_refuses_missing_or_cross_member_bindings() {
        let (path, source, projection) = profiled_projection("community");
        let raw = cmd::canonical(&projection).unwrap();
        let valid: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        for fault in [
            "missing-baseline",
            "empty-extension",
            "extra-extension",
            "baseline-row",
            "wrong-kind",
            "wrong-source",
            "wrong-schema",
            "missing-registry",
            "invalid-digest",
        ] {
            let mut bad = valid.clone();
            match fault {
                "missing-baseline" => {
                    bad["records"].as_object_mut().unwrap().remove("agent");
                }
                "empty-extension" => bad["records"]["community"] = serde_json::json!([]),
                "extra-extension" => {
                    bad["records"]["another-profile"] = bad["records"]["community"].clone()
                }
                "baseline-row" => bad["records"]["agent"] = bad["records"]["community"].clone(),
                "wrong-kind" => {
                    bad["records"]["community"][0]["record_type"] = serde_json::json!("other")
                }
                "wrong-source" => {
                    bad["records"]["community"][0]["source_record_ref"] =
                        serde_json::json!("ToS/source-witnesses/other/community.json")
                }
                "wrong-schema" => {
                    bad["records"]["community"][0]["source_schema_ref"] =
                        serde_json::json!("ToS/contracts/unselected.schema.json")
                }
                "missing-registry" => {
                    bad["source_profiles"]
                        .as_object_mut()
                        .unwrap()
                        .remove(MIXED_AGENT_BASELINE[0]);
                }
                "invalid-digest" => {
                    bad["records"]["community"][0]["record_sha256"] = serde_json::json!("invalid")
                }
                _ => unreachable!(),
            }
            let bad = cmd::parse(&serde_json::to_vec(&bad).unwrap()).unwrap();
            assert!(
                AddressedMixedInventoryV1::checked_projection(
                    &path,
                    &cmd::canonical(&bad).unwrap(),
                    &Digest256::of_bytes(&source).to_hex(),
                )
                .is_err(),
                "accepted {fault}"
            );
        }
    }

    #[test]
    fn shared_creation_metadata_keeps_its_family_and_public_text_stays_required() {
        let home = "ToS/source-witnesses/relations/formation-research";
        for basename in [
            "source-create-request.json",
            "source-create-environment.json",
            "source-create-provenance.jsonl",
            "source-create-receipt.json",
        ] {
            let path = format!("{home}/{basename}");
            let source = b"{}\n".to_vec();
            let digest = Digest256::of_bytes(&source).to_hex();
            let projection = cmd::object(vec![
                (
                    "schema_version",
                    cmd::string(
                        tos_compiler::managed_source::MANAGED_MIXED_INVENTORY_MEMBER_SCHEMA_V2,
                    ),
                ),
                ("path", cmd::string(&path)),
                ("raw_sha256", cmd::string(&digest)),
                (
                    "records",
                    cmd::object(
                        crate::source_claims::MIXED_INVENTORY_RECORD_KINDS
                            .iter()
                            .map(|kind| (*kind, tos_foundation::JsonValue::Array(Vec::new())))
                            .collect(),
                    ),
                ),
                ("source_profiles", cmd::object(Vec::new())),
                ("events", tos_foundation::JsonValue::Array(Vec::new())),
                ("anchors", tos_foundation::JsonValue::Array(Vec::new())),
                ("form", tos_foundation::JsonValue::Null),
                ("claim_owner", tos_foundation::JsonValue::Null),
                ("public_text_owner", tos_foundation::JsonValue::Null),
            ]);
            let raw = cmd::canonical(&projection).unwrap();
            AddressedMixedInventoryV1::checked_projection(&path, &raw, &digest).unwrap();
            let writes = [SourceRevisionWrite {
                subject: path.clone(),
                expected_predecessor: None,
                proposed_revision: 1,
                exact_bytes: source,
            }];
            let projections = BTreeMap::from([(path, raw)]);
            super::super::validate_revision_owner_projections(
                "tos-pg-global-claims-v1:regression",
                &writes,
                &projections,
            )
            .unwrap();
            assert!(matches!(
                super::super::validate_revision_owner_projections(
                    "tos-pg-public-native-text-v1:regression",
                    &writes,
                    &projections,
                ),
                Err(DurableError::Corrupt(
                    "source revision public Text owner projection differs"
                ))
            ));
        }
    }

    #[test]
    fn bootstrap_contract_addressed_claim_routes_match_declared_source_families() {
        let laboratory = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/graph-claims.jsonl";
        let source = b"retained laboratory packet\n";
        let digest = Digest256::of_bytes(source).to_hex();
        let envelope = |path: &str, owner| {
            cmd::canonical(&cmd::object(vec![
                (
                    "schema_version",
                    cmd::string(
                        tos_compiler::managed_source::MANAGED_MIXED_INVENTORY_MEMBER_SCHEMA_V2,
                    ),
                ),
                ("path", cmd::string(path)),
                ("raw_sha256", cmd::string(&digest)),
                (
                    "records",
                    cmd::object(
                        crate::source_claims::MIXED_INVENTORY_RECORD_KINDS
                            .iter()
                            .map(|kind| (*kind, tos_foundation::JsonValue::Array(Vec::new())))
                            .collect(),
                    ),
                ),
                ("source_profiles", cmd::object(Vec::new())),
                ("events", cmd::object(Vec::new())),
                ("anchors", cmd::object(Vec::new())),
                ("form", tos_foundation::JsonValue::Null),
                ("claim_owner", owner),
                ("public_text_owner", tos_foundation::JsonValue::Null),
            ]))
            .unwrap()
        };
        let raw = envelope(laboratory, tos_foundation::JsonValue::Null);
        let decoded = AddressedMixedInventoryV1::checked_projection(laboratory, &raw, &digest)
            .expect("retained laboratory source is not a command Claim owner");
        assert_eq!(cmd::text(&decoded, "path").unwrap(), laboratory);
        assert_eq!(
            cmd::field(&decoded, "claim_owner").unwrap(),
            &tos_foundation::JsonValue::Null
        );
        assert!(
            AddressedMixedInventoryV1::checked_projection(
                laboratory,
                &raw,
                &Digest256::of_bytes(b"changed").to_hex()
            )
            .is_err()
        );
        assert!(
            AddressedMixedInventoryV1::checked_projection(
                laboratory,
                &envelope(laboratory, cmd::object(Vec::new())),
                &digest,
            )
            .is_err()
        );

        // The native, historical and all declared nonhistorical imported
        // streams still require a real owner contribution, even if empty.
        for basename in [
            "source-claims.jsonl",
            "membership-claims.jsonl",
            "responsibility-claims.jsonl",
            "publication-claims.jsonl",
            "provision-activity-claims.jsonl",
            "work-chronology-claims.jsonl",
            "work-expression-claims.jsonl",
            "expression-edition-claims.jsonl",
            "edition-item-claims.jsonl",
            "expression-derivation-claims.jsonl",
            "object-link-claims.jsonl",
            "historical-claims.jsonl",
        ] {
            let path = format!("ToS/source-witnesses/history/example/{basename}");
            assert!(is_claim_stream_path(&path), "{path}");
            assert!(
                AddressedMixedInventoryV1::checked_projection(
                    &path,
                    &envelope(&path, tos_foundation::JsonValue::Null),
                    &digest,
                )
                .is_err(),
                "missing owner was accepted for {path}"
            );
        }
    }

    #[test]
    fn private_source_custody_rejects_semantic_record_rows() {
        let path = "ToS/source-witnesses/works/example/private/item.json";
        let empty_records = || {
            cmd::object(
                crate::source_claims::MIXED_INVENTORY_RECORD_KINDS
                    .iter()
                    .map(|kind| (*kind, tos_foundation::JsonValue::Array(Vec::new())))
                    .collect(),
            )
        };
        let envelope = |records| {
            cmd::object(vec![
                (
                    "schema_version",
                    cmd::string(
                        tos_compiler::managed_source::MANAGED_MIXED_INVENTORY_MEMBER_SCHEMA_V2,
                    ),
                ),
                ("path", cmd::string(path)),
                (
                    "raw_sha256",
                    cmd::string(&Digest256::of_bytes(b"private source").to_hex()),
                ),
                ("records", records),
                ("source_profiles", cmd::object(Vec::new())),
                ("events", cmd::object(Vec::new())),
                ("anchors", cmd::object(Vec::new())),
                ("form", tos_foundation::JsonValue::Null),
                (
                    "claim_owner",
                    crate::source_claims::pg_inventory::nonprojecting_custody_owner_contribution(
                        path,
                    )
                    .unwrap(),
                ),
                ("public_text_owner", tos_foundation::JsonValue::Null),
            ])
        };
        let empty = cmd::canonical(&envelope(empty_records())).unwrap();
        parse_mixed_projection(path, &empty, None).unwrap();

        let mut projected_records = empty_records();
        cmd::set(
            &mut projected_records,
            crate::source_claims::MIXED_INVENTORY_RECORD_KINDS[0],
            tos_foundation::JsonValue::Array(vec![cmd::object(Vec::new())]),
        )
        .unwrap();
        let projected = cmd::canonical(&envelope(projected_records)).unwrap();
        assert!(parse_mixed_projection(path, &projected, None).is_err());
    }

    #[test]
    fn historical_claim_owner_initial_and_revised_closure_is_stream_specific() {
        let (initial_path, initial_digest, initial) =
            historical_owner_fixture(1, None, "historical-claims.jsonl");
        assert!(validate_historical_claim_owner(&initial, &initial_path, &initial_digest).is_ok());

        let (revised_path, revised_digest, revised) = historical_owner_fixture(
            2,
            Some(b"{\"schema_version\":\"tos_claim_revision_history_v1\"}\n"),
            "historical-claims.jsonl",
        );
        assert!(validate_historical_claim_owner(&revised, &revised_path, &revised_digest).is_ok());

        let (v2_without_history_path, v2_without_history_digest, v2_without_history) =
            historical_owner_fixture(2, None, "historical-claims.jsonl");
        assert!(
            validate_historical_claim_owner(
                &v2_without_history,
                &v2_without_history_path,
                &v2_without_history_digest
            )
            .is_err()
        );

        let (wrong_archive_path, wrong_archive_digest, wrong_archive) = historical_owner_fixture(
            2,
            Some(b"{\"schema_version\":\"tos_claim_revision_history_v1\"}\n"),
            "source-claims.jsonl",
        );
        assert!(
            validate_historical_claim_owner(
                &wrong_archive,
                &wrong_archive_path,
                &wrong_archive_digest
            )
            .is_err()
        );
    }

    #[test]
    fn managed_item_home_paths_are_direct_and_relative() {
        assert!(is_managed_item_home("ToS/source-witnesses/items/example"));
        assert!(is_managed_item_home(
            "ToS/source-witnesses/works/tree-of-sophia/expressions/example/editions/edition-1/items/item-1"
        ));
        for path in [
            "ToS/source-witnesses/items/example/extra",
            "ToS/source-witnesses/works/example/items/item-1",
            "ToS/source-witnesses/works/example/editions/edition-1/items/item-1/extra",
            "ToS/source-witnesses/works/example/editions/../items/item-1",
        ] {
            assert!(!is_managed_item_home(path), "accepted {path}");
        }
    }
}
